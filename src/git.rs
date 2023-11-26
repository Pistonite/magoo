//! Low level integration with git
use std::borrow::Cow;
use std::cell::OnceCell;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::Duration;

use fs4::FileExt;

use crate::print::{self, println_info, println_verbose, println_warn};

/// The semver notation of the officially supported git versions
///
/// The version is not checked at run time, since unsupported versions might work fine.
pub const SUPPORTED_GIT_VERSIONS: &str = ">=2.35.0";

/// Context for running git commands
pub struct GitContext {
    /// The absolute path of the working directory to run the git commands
    working_dir: PathBuf,

    /// The path to the .git directory
    ///
    /// This is retrieved from `git rev-parse --git-dir` and cached.
    git_dir_cell: OnceCell<PathBuf>,

    /// The path to the top level directory
    ///
    /// This is retrieved from `git rev-parse --show-toplevel` and cached.
    top_level_cell: OnceCell<PathBuf>,
}

/// Implementation for basic operations
impl GitContext {
    /// Create a new GitContext for running git commands in the given working directory
    pub fn try_from<S>(working_dir: S) -> Result<Self, GitError>
    where
        S: AsRef<Path>,
    {
        if which::which("git").is_err() {
            return Err(GitError::NotInstalled);
        }
        Ok(Self {
            working_dir: working_dir.as_ref().canonicalize_git()?,
            git_dir_cell: OnceCell::new(),
            top_level_cell: OnceCell::new(),
        })
    }

    /// Return a guard that locks the repository until dropped. Other magoo processes cannot access
    /// the repository while the guard is alive.
    pub fn lock(&self) -> Result<Guard, GitError> {
        let git_dir = self.git_dir()?;
        let lock_path = git_dir.join("magoo.lock");
        Guard::new(lock_path)
    }

    /// Print the supported git version info and current git version into
    pub fn print_version_info(&self) -> Result<(), GitError> {
        println_info!(
            "The officially supported git versions are: {}",
            super::SUPPORTED_GIT_VERSIONS
        );
        println_info!("Your `git --version` is:");
        self.run_git_command(&["--version"], true)?;
        Ok(())
    }

    /// Get the absolute path to the .git directory
    pub fn git_dir(&self) -> Result<&PathBuf, GitError> {
        if let Some(git_dir) = self.git_dir_cell.get() {
            return Ok(git_dir);
        }

        let git_dir_path = match self.git_dir_raw()? {
            Some(x) => x,
            None => {
                return Err(GitError::UnexpectedOutput(
                    "git did not return the .git directory".to_string(),
                ));
            }
        };
        let path = self.working_dir.join(git_dir_path).canonicalize_git()?;

        self.git_dir_cell.set(path).unwrap();
        Ok(self.git_dir_cell.get().unwrap())
    }

    /// Return the raw output of `git rev-parse --git-dir`
    pub fn git_dir_raw(&self) -> Result<Option<String>, GitError> {
        let output = self.run_git_command(&["rev-parse", "--git-dir"], false)?;
        Ok(output.into_iter().next())
    }

    /// Get the absolute path to the top level directory
    pub fn top_level_dir(&self) -> Result<&PathBuf, GitError> {
        if let Some(top_level) = self.top_level_cell.get() {
            return Ok(top_level);
        }

        let output = self.run_git_command(&["rev-parse", "--show-toplevel"], false)?;
        let top_dir_path = output.first().ok_or_else(|| {
            GitError::UnexpectedOutput("git did not return the top level directory".to_string())
        })?;
        let path = self.working_dir.join(top_dir_path).canonicalize_git()?;

        self.top_level_cell.set(path).unwrap();
        Ok(self.top_level_cell.get().unwrap())
    }

    /// Get the path in `git -C <path> ...` to run the command in the top level directory
    pub fn get_top_level_switch(&self) -> Result<Option<String>, GitError> {
        let top_level_dir = self.top_level_dir()?;

        let command = match Path::new(".").canonicalize() {
            Ok(cwd) => {
                if &cwd == top_level_dir {
                    None
                } else {
                    let path = pathdiff::diff_paths(top_level_dir, &cwd)
                        .unwrap_or(top_level_dir.to_path_buf());
                    let diff = path.to_cmd_arg();
                    Some(quote_arg(&diff).to_string())
                }
            }
            Err(_) => {
                let top_level = top_level_dir.to_cmd_arg();
                Some(quote_arg(&top_level).to_string())
            }
        };

        Ok(command)
    }

    /// Run the git command from self's working directory. The output of the command will be returned as a vector of lines.
    fn run_git_command(&self, args: &[&str], print: bool) -> Result<Vec<String>, GitError> {
        let args_str = args
            .iter()
            .map(|x| {
                if x.contains(' ') {
                    format!("'{x}'")
                } else {
                    x.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let command = format!("git {args_str}");
        println_verbose!("Running `{command}`");

        let mut child = Command::new("git")
            .args(args)
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(if !print {
                Stdio::piped()
            } else {
                Stdio::inherit()
            })
            .spawn()
            .map_err(|e| {
                GitError::CommandFailed(command.clone(), "failed to spawn process".to_string(), e)
            })?;

        let mut output = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let line = line.map_err(|e| {
                    GitError::CommandFailed(command.clone(), "failed to read output".to_string(), e)
                })?;
                if print {
                    println_info!("{line}");
                }
                output.push(line);
            }
        }

        if print::is_verbose() {
            if let Some(stderr) = child.stderr.take() {
                let reader = BufReader::new(stderr);
                for line in reader.lines().flatten() {
                    println_verbose!("{line}");
                }
            }
        }
        let status = child.wait().map_err(|e| {
            GitError::CommandFailed(
                command.clone(),
                "command did not finish normally".to_string(),
                e,
            )
        })?;
        println_verbose!("Git command finished: {}", status);
        if status.success() {
            Ok(output)
        } else {
            Err(GitError::ExitStatus(command, status))
        }
    }
}

/// Wrapper implementation for git commands
impl GitContext {
    /// Run `git status` and print the status
    pub fn status(&self) -> Result<(), GitError> {
        self.run_git_command(&["status"], true)?;
        Ok(())
    }

    /// Run `git -C top_level ls-files ...`
    pub fn ls_files(&self, extra_args: &[&str]) -> Result<Vec<String>, GitError> {
        let top_level_dir = self.top_level_dir()?.to_cmd_arg();
        let mut args = vec!["-C", &top_level_dir, "ls-files"];
        args.extend_from_slice(extra_args);
        self.run_git_command(&args, false)
    }

    /// Run `git describe --all <commit>` and return the first output
    pub fn describe(&self, commit: &str) -> Option<String> {
        self.run_git_command(&["describe", "--all", commit], false)
            .ok()
            .and_then(|x| x.into_iter().next())
    }

    /// Run `git rev-parse HEAD`
    pub fn head(&self) -> Result<Option<String>, GitError> {
        let output = self.run_git_command(&["rev-parse", "HEAD"], false)?;
        Ok(output.into_iter().next())
    }

    /// Run `git config -f config_path --get key`
    ///
    /// The config path is resolved relative to the working directory of this context.
    pub fn get_config<S>(&self, config_path: S, key: &str) -> Result<Option<String>, GitError>
    where
        S: AsRef<Path>,
    {
        let config_path = config_path.to_cmd_arg();
        let value = self
            .run_git_command(&["config", "-f", &config_path, "--get", key], false)?
            .into_iter()
            .next();
        Ok(value)
    }

    /// Calls `git config -f config_path ... --get-regexp regexp` to get (key, value) pairs in the config file
    ///
    /// The config path is resolved relative to the working directory of this context.
    pub fn get_config_regexp<S>(
        &self,
        config_path: S,
        regexp: &str,
    ) -> Result<Vec<(String, String)>, GitError>
    where
        S: AsRef<Path>,
    {
        let config_path = config_path.to_cmd_arg();
        let name_and_values = self.run_git_command(
            &["config", "-f", &config_path, "--get-regexp", regexp],
            false,
        )?;
        let name_only = self.run_git_command(
            &[
                "config",
                "-f",
                &config_path,
                "--name-only",
                "--get-regexp",
                regexp,
            ],
            false,
        )?;

        let mut name_values = Vec::new();
        for (name, name_and_value) in name_only.iter().zip(name_and_values.iter()) {
            match name_and_value.strip_prefix(name) {
                Some(value) => {
                    name_values.push((name.trim().to_string(), value.trim().to_string()));
                }
                None => {
                    return Err(GitError::InvalidConfig(
                        "unexpected config key mismatch in git output.".to_string(),
                    ));
                }
            }
        }

        Ok(name_values)
    }

    /// Calls `git config` to set or remove a config from a config file.
    ///
    /// The config path is resolved relative to the working directory of this context.
    pub fn set_config<S>(
        &self,
        config_path: S,
        key: &str,
        value: Option<&str>,
    ) -> Result<(), GitError>
    where
        S: AsRef<Path>,
    {
        let config_path = config_path.to_cmd_arg();
        let mut args = vec!["config", "-f", &config_path];
        match value {
            Some(v) => {
                args.push(key);
                args.push(v);
            }
            None => {
                args.push("--unset");
                args.push(key);
            }
        }
        self.run_git_command(&args, false)?;
        Ok(())
    }

    /// Remove a config section from a config file.
    ///
    /// The config path is resolved relative to the working directory of this context.
    pub fn remove_config_section<S>(&self, config_path: S, section: &str) -> Result<(), GitError>
    where
        S: AsRef<Path>,
    {
        let config_path = config_path.to_cmd_arg();
        self.run_git_command(
            &["config", "-f", &config_path, "--remove-section", section],
            false,
        )?;
        Ok(())
    }

    /// Remove an object from the index and stage the change. The path should be relative from repo top level
    pub fn remove_from_index(&self, path: &str) -> Result<(), GitError> {
        let top_level_dir = self.top_level_dir()?.to_cmd_arg();

        // ignore the error because the file might not be in the index
        let _ = self.run_git_command(&["-C", &top_level_dir, "rm", path], false);

        let _ = self.run_git_command(&["-C", &top_level_dir, "add", path], false);
        Ok(())
    }

    /// Run `git add`
    pub fn add(&self, path: &str) -> Result<(), GitError> {
        let top_level_dir = self.top_level_dir()?.to_cmd_arg();

        self.run_git_command(&["-C", &top_level_dir, "add", path], false)?;
        Ok(())
    }

    /// Runs `git submodule deinit [-- <path>]`. Path should be from top level
    pub fn submodule_deinit(&self, path: Option<&str>, force: bool) -> Result<(), GitError> {
        let top_level_dir = self.top_level_dir()?.to_cmd_arg();
        let mut args = vec!["-C", &top_level_dir, "submodule", "deinit"];

        if force {
            args.push("--force");
        }

        if let Some(path) = path {
            args.push("--");
            args.push(path);
        } else {
            args.push("--all");
        }
        self.run_git_command(&args, true)?;

        Ok(())
    }

    /// Runs `git submodule init [-- <path>]`. Path should be from top level
    pub fn submodule_init(&self, path: Option<&str>) -> Result<(), GitError> {
        let top_level_dir = self.top_level_dir()?.to_cmd_arg();
        let mut args = vec!["-C", &top_level_dir, "submodule", "init"];

        if let Some(path) = path {
            args.push("--");
            args.push(path);
        }
        self.run_git_command(&args, true)?;

        Ok(())
    }

    /// Runs `git submodule sync [-- <path>]`. Path should be from top level
    pub fn submodule_sync(&self, path: Option<&str>) -> Result<(), GitError> {
        let top_level_dir = self.top_level_dir()?.to_cmd_arg();
        let mut args = vec!["-C", &top_level_dir, "submodule", "sync"];

        if let Some(path) = path {
            args.push("--");
            args.push(path);
        }
        self.run_git_command(&args, true)?;

        Ok(())
    }

    /// Runs `git submodule set-branch`. Path should be from top level
    pub fn submodule_set_branch(&self, path: &str, branch: Option<&str>) -> Result<(), GitError> {
        let top_level_dir = self.top_level_dir()?.to_cmd_arg();
        let mut args = vec!["-C", &top_level_dir, "submodule", "set-branch"];
        match branch {
            Some(branch) => {
                args.push("--branch");
                args.push(branch);
            }
            None => {
                args.push("--default");
            }
        }
        args.push("--");
        args.push(path);
        self.run_git_command(&args, true)?;
        Ok(())
    }

    /// Runs `git submodule set-url`. Path should be from top level
    pub fn submodule_set_url(&self, path: &str, url: &str) -> Result<(), GitError> {
        let top_level_dir = self.top_level_dir()?.to_cmd_arg();
        self.run_git_command(
            &[
                "-C",
                &top_level_dir,
                "submodule",
                "set-url",
                "--",
                path,
                url,
            ],
            true,
        )?;
        Ok(())
    }

    /// Runs `git submodule update [-- <path>]`. Path should be from top level
    pub fn submodule_update(
        &self,
        path: Option<&str>,
        force: bool,
        remote: bool,
    ) -> Result<(), GitError> {
        let top_level_dir = self.top_level_dir()?.to_cmd_arg();
        let mut args = vec!["-C", &top_level_dir, "submodule", "update"];

        if force {
            args.push("--force");
        }

        if remote {
            args.push("--remote");
        }

        if let Some(path) = path {
            args.push("--");
            args.push(path);
        }
        self.run_git_command(&args, true)?;

        Ok(())
    }

    /// Runs `git submodule add`. Path should be from top level
    pub fn submodule_add(
        &self,
        url: &str,
        path: Option<&str>,
        branch: Option<&str>,
        name: Option<&str>,
        depth: Option<usize>,
        force: bool,
    ) -> Result<(), GitError> {
        let top_level_dir = self.top_level_dir()?.to_cmd_arg();
        let mut args = vec!["-C", &top_level_dir, "submodule", "add"];
        if force {
            args.push("--force");
        }
        if let Some(branch) = branch {
            args.push("--branch");
            args.push(branch);
        }
        if let Some(name) = name {
            args.push("--name");
            args.push(name);
        }
        let depth = depth.map(|x| x.to_string());
        if let Some(depth) = &depth {
            args.push("--depth");
            args.push(depth);
        }
        args.push("--");
        args.push(url);
        if let Some(path) = path {
            args.push(path);
        }
        self.run_git_command(&args, true)?;
        Ok(())
    }
}

/// Guard that uses file locking to ensure only one process are manipulating
/// the submodules at a time.
#[derive(Debug)]
pub struct Guard(pub File, pub PathBuf);

impl Guard {
    /// Create a new guard with the given path as the file lock. Will block until
    /// the lock can be acquired.
    pub fn new<P>(path: P) -> Result<Self, GitError>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        if path.exists() {
            println_warn!("Waiting on file lock. If you are sure no other magoo processes are running, you can remove the lock file `{}`", path.to_cmd_arg());
        }
        while path.exists() {
            println_verbose!("Waiting for lock file...");
            std::thread::sleep(Duration::from_millis(1000));
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .map_err(|e| GitError::LockFailed(path.to_cmd_arg(), e))?;
        file.lock_exclusive()
            .map_err(|e| GitError::LockFailed(path.to_cmd_arg(), e))?;
        println_verbose!("Acquired lock file `{}`", path.to_cmd_arg());
        Ok(Self(file, path.to_path_buf()))
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let path = &self.1.to_cmd_arg();
        println_verbose!("Releasing lock file `{path}`");
        if self.0.unlock().is_err() {
            println_verbose!("Failed to unlock file `{path}`");
        }
        if std::fs::remove_file(&self.1).is_err() {
            println_verbose!("Failed to remove file `{path}`");
        }
    }
}

/// Error type for the program
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("operation was successful")]
    Success,

    #[error("git is not installed or not in PATH")]
    NotInstalled,

    #[error("fail to read `{0}`: {1}")]
    CanonicalizeFail(String, std::io::Error),

    #[error("unexpected output: {0}")]
    UnexpectedOutput(String),

    #[error("failed to execute `{0}`: {1}: {2}")]
    CommandFailed(String, String, std::io::Error),

    #[error("command `{0}` finished with {1}")]
    ExitStatus(String, ExitStatus),

    #[error("cannot process config: {0}")]
    InvalidConfig(String),

    #[error("cannot process index: {0}")]
    InvalidIndex(String),

    #[error("cannot find module `{0}`")]
    ModuleNotFound(String),

    #[error("cannot lock `{0}`: {1}")]
    LockFailed(String, std::io::Error),

    #[error("fix the issues above and try again.")]
    NeedFix(bool /* should show fatal */),
}

/// Helper trait to canonicalize a path and return a [`GitError`] if failed
pub trait GitCanonicalize {
    fn canonicalize_git(&self) -> Result<PathBuf, GitError>;
}

impl<S> GitCanonicalize for S
where
    S: AsRef<Path>,
{
    fn canonicalize_git(&self) -> Result<PathBuf, GitError> {
        let s = self.as_ref();
        s.canonicalize()
            .map_err(|e| GitError::CanonicalizeFail(s.display().to_string(), e))
    }
}

/// Helper trait to clean a path to be used as command line argument
pub trait GitCmdPath {
    fn to_cmd_arg(&self) -> String;
}

impl<S> GitCmdPath for S
where
    S: AsRef<Path>,
{
    #[cfg(not(windows))]
    fn to_cmd_arg(&self) -> String {
        self.as_ref().display().to_string()
    }

    #[cfg(windows)]
    fn to_cmd_arg(&self) -> String {
        let s = self.as_ref().display().to_string();
        match s.strip_prefix(r"\\?\") {
            Some(x) => x.to_string(),
            None => s,
        }
    }
}

/// Quote the argument for shell.
pub fn quote_arg(s: &str) -> Cow<'_, str> {
    // note that this implementation doesn't work in a few edge cases
    // but atm I don't have enough time to thoroughly test it
    if s.is_empty() {
        Cow::Borrowed("''")
    } else if s.contains(' ') || s.contains('\'') {
        Cow::Owned(format!("'{s}'"))
    } else {
        Cow::Borrowed(s)
    }
}
