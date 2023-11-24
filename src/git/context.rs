use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use crate::print::{self, print_progress, println_info, println_verbose};
use crate::submodule::{InGitConfig, InGitModule, InGitmodules, IndexObject, Submodule};

use super::{quote_arg, GitCanonicalize, GitError};

/// Context for running git commands
///
/// Some command results are cached to avoid running git commands multiple times.
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
}

impl GitContext {
    pub fn print_version_info(&self) -> Result<(), GitError> {
        println_info!(
            "The officially supported git versions are: {}",
            super::SUPPORTED_GIT_VERSIONS
        );
        println_info!("You `git --version` is:");
        self.run_git_command(&["--version"], PrintMode::Normal)?;
        Ok(())
    }

    /// Get the absolute path to the .git directory
    pub fn git_dir(&self) -> Result<&PathBuf, GitError> {
        if let Some(git_dir) = self.git_dir_cell.get() {
            return Ok(git_dir);
        }

        let output = self.run_git_command(&["rev-parse", "--git-dir"], PrintMode::Quiet)?;
        let git_dir_path = output.first().ok_or_else(|| {
            GitError::UnexpectedOutput("git did not return the .git directory".to_string())
        })?;
        let path = self.working_dir.join(git_dir_path).canonicalize_git()?;

        self.git_dir_cell.set(path).unwrap();
        Ok(self.git_dir_cell.get().unwrap())
    }

    /// Get the absolute path to the top level directory
    pub fn top_level_dir(&self) -> Result<&PathBuf, GitError> {
        if let Some(top_level) = self.top_level_cell.get() {
            return Ok(top_level);
        }

        let output = self.run_git_command(&["rev-parse", "--show-toplevel"], PrintMode::Quiet)?;
        let top_dir_path = output.first().ok_or_else(|| {
            GitError::UnexpectedOutput("git did not return the top level directory".to_string())
        })?;
        let path = self.working_dir.join(top_dir_path).canonicalize_git()?;

        self.top_level_cell.set(path).unwrap();
        Ok(self.top_level_cell.get().unwrap())
    }

    pub fn describe(&self, commit: &str) -> Option<String> {
        self.run_git_command(&["describe", "--all", commit], PrintMode::Quiet)
            .ok()
            .and_then(|x| x.into_iter().next())
    }

    /// Get the submodule status in the repository.
    ///
    /// `status_map` gets filled with submodules with names, while `lone_index` gets
    /// filled with submodules in the index that are not found anywhere else.
    ///
    /// If `all` is false, it will not include submodules that are only in the index and in
    /// .git/modules
    pub fn get_submodule_status(
        &self,
        status_map: &mut BTreeMap<String, Submodule>,
        lone_index: &mut Vec<IndexObject>,
        all: bool,
    ) -> Result<(), GitError> {
        status_map.clear();
        lone_index.clear();
        // read .gitmodules
        {
            let map = self.read_dot_gitmodules().unwrap_or_default();
            for (name, submodule) in map {
                status_map.insert(
                    name,
                    Submodule {
                        in_gitmodules: Some(submodule),
                        in_config: None,
                        in_index: None,
                        in_modules: None,
                    },
                );
            }
        }
        // read .git/config
        {
            let map = self.read_dot_git_config().unwrap_or_default();
            for (name, submodule) in map {
                if let Some(s) = status_map.get_mut(&name) {
                    s.in_config = Some(submodule);
                } else {
                    status_map.insert(
                        name,
                        Submodule {
                            in_gitmodules: None,
                            in_config: Some(submodule),
                            in_index: None,
                            in_modules: None,
                        },
                    );
                }
            }
        }
        // read .git/modules
        if all {
            let map = self.find_all_git_modules().unwrap_or_default();
            for (name, module) in map {
                if let Some(s) = status_map.get_mut(&name) {
                    s.in_modules = Some(module);
                } else {
                    status_map.insert(
                        name,
                        Submodule {
                            in_gitmodules: None,
                            in_config: None,
                            in_index: None,
                            in_modules: Some(module),
                        },
                    );
                }
            }
        } else {
            for (name, submodule) in status_map.iter_mut() {
                if let Some(module) = self.read_git_module(name).ok() {
                    submodule.in_modules = Some(module);
                }
            }
        };
        let mut index = self.read_submodules_in_index().unwrap_or_default();

        for submodule in status_map.values_mut() {
            let path = match submodule.path() {
                Some(path) => path,
                None => continue,
            };
            if let Some(index_obj) = index.remove(path) {
                submodule.in_index = Some(index_obj);
            }
        }

        if all {
            for (_, index_obj) in index {
                lone_index.push(index_obj);
            }
        }

        Ok(())
    }

    /// Read the .gitmodules file from this repo and return a map of submodule names to
    /// [`GitmodulesSubmodule`]
    ///
    /// Returns an error if any git command fails. Use `.unwrap_or_default()` to get an empty map.
    pub fn read_dot_gitmodules(&self) -> Result<BTreeMap<String, InGitmodules>, GitError> {
        let top_level_dir = self.top_level_dir()?;
        let dot_gitmodules_path = top_level_dir.join(".gitmodules");

        let config_entries =
            self.read_submodule_from_config(&dot_gitmodules_path.display().to_string())?;

        let mut submodules = BTreeMap::new();

        for (key, value) in config_entries {
            if let Some(x) = key.strip_suffix(".path") {
                let entry = submodules
                    .entry(x.to_string())
                    .or_insert_with(|| InGitmodules::with_name(x));
                entry.path = Some(value);
            } else if let Some(x) = key.strip_suffix(".url") {
                let entry = submodules
                    .entry(x.to_string())
                    .or_insert_with(|| InGitmodules::with_name(x));
                entry.url = Some(value);
            } else if let Some(x) = key.strip_suffix(".branch") {
                let entry = submodules
                    .entry(x.to_string())
                    .or_insert_with(|| InGitmodules::with_name(x));
                entry.branch = Some(value);
            }
        }

        println_verbose!(
            "Found {} submodules in .gitmodules: {:?}",
            submodules.len(),
            submodules.keys().collect::<Vec<_>>()
        );
        Ok(submodules)
    }

    /// Read the .git/config file and return a map of submodule names to [`GitConfigSubmodule`].
    ///
    /// Returns an error if any git command fails. Use `.unwrap_or_default()` to get an empty map.
    pub fn read_dot_git_config(&self) -> Result<BTreeMap<String, InGitConfig>, GitError> {
        let git_dir = self.git_dir()?;
        let dot_git_config_path = git_dir.join("config");

        let config_entries =
            self.read_submodule_from_config(&dot_git_config_path.display().to_string())?;

        let mut submodules = BTreeMap::new();

        for (key, value) in config_entries {
            if let Some(x) = key.strip_suffix(".url") {
                let entry = submodules
                    .entry(x.to_string())
                    .or_insert_with(|| InGitConfig::with_name(x));
                entry.url = Some(value);
            }
        }

        println_verbose!(
            "Found {} submodules in .git/config: {:?}",
            submodules.len(),
            submodules.keys().collect::<Vec<_>>()
        );
        Ok(submodules)
    }

    /// Use `git ls-files` to list submodules stored in the index.
    ///
    /// Returns a map of path to submodule [`IndexObject`].
    pub fn read_submodules_in_index(&self) -> Result<BTreeMap<String, IndexObject>, GitError> {
        let output = self.run_git_command(
            &[
                "ls-files",
                r#"--format=%(objectmode) %(objectname) %(path)"#,
            ],
            PrintMode::Progress,
        )?;
        let mut submodules = BTreeMap::new();

        for line in output {
            // mode 160000 is submodule
            let line = match line.strip_prefix("160000 ") {
                Some(line) => line,
                None => {
                    continue;
                }
            };
            println_verbose!("Found submodule in index: {}", line);
            let mut parts = line.splitn(2, ' ');
            let sha = parts.next().ok_or_else(|| {
                GitError::InvalidIndex("missing commit hash in output".to_string())
            })?;
            let path = parts
                .next()
                .ok_or_else(|| GitError::InvalidIndex("missing path in output".to_string()))?;
            submodules.insert(
                path.to_string(),
                IndexObject {
                    sha: sha.to_string(),
                    path: path.to_string(),
                },
            );
        }

        println_verbose!(
            "Found {} submodules in index: {:?}",
            submodules.len(),
            submodules.keys().collect::<Vec<_>>()
        );
        Ok(submodules)
    }

    /// Read .git/modules and find all entries.
    pub fn find_all_git_modules(&self) -> Result<BTreeMap<String, InGitModule>, GitError> {
        let mut modules = BTreeMap::new();
        let git_dir = self.git_dir()?;
        let module_dir = git_dir.join("modules");
        if !module_dir.exists() {
            println_verbose!(".git/modules does not exist");
        } else {
            self.find_git_modules_recursively(None, &module_dir, &mut modules);
        }
        Ok(modules)
    }

    fn find_git_modules_recursively(
        &self,
        name: Option<&str>,
        dir_path: &Path,
        modules: &mut BTreeMap<String, InGitModule>,
    ) {
        println_verbose!("Scanning for git modules in `{}`", dir_path.display());
        let config_path = dir_path.join("config");
        if config_path.is_file() {
            if let Some(name) = name {
                // dir_path is a git module
                match self.read_git_module(name) {
                    Err(e) => {
                        println_verbose!("Failed to read git module `{name}`: {e}");
                    }
                    Ok(module) => {
                        println_verbose!("Found git module `{name}`");
                        modules.insert(name.to_string(), module);
                    }
                }
            }
        } else {
            // dir_path is not a module, recurse
            let dir = match dir_path.read_dir() {
                Err(e) => {
                    println_verbose!("Failed to read directory `{}`: {e}", dir_path.display());
                    return;
                }
                Ok(dir) => dir,
            };
            for entry in dir {
                let entry = match entry {
                    Err(e) => {
                        println_verbose!(
                            "Failed to read directory entry in `{}`: {e}",
                            dir_path.display()
                        );
                        continue;
                    }
                    Ok(entry) => entry,
                };
                let full_path = entry.path();
                if full_path.is_dir() {
                    let entry_file_name = entry.file_name();
                    let entry_name_utf8 = match entry_file_name.to_str() {
                        None => {
                            println_verbose!(
                                "File name is not unicode: `{}`",
                                entry_file_name.to_string_lossy()
                            );
                            continue;
                        }
                        Some(name) => name,
                    };
                    let next_name = match name {
                        Some(name) => format!("{name}/{entry_name_utf8}"),
                        None => entry_name_utf8.to_string(),
                    };
                    self.find_git_modules_recursively(Some(&next_name), &full_path, modules);
                }
            }
        }
    }

    /// Read .git/modules/<name> and return a [`GitModule`]
    pub fn read_git_module(&self, name: &str) -> Result<InGitModule, GitError> {
        let git_dir = self.git_dir()?;
        let module_dir = git_dir.join("modules").join(name);
        if !module_dir.exists() {
            println_verbose!("Module `{name}` not found in .git/modules");
            return Err(GitError::ModuleNotFound(name.to_string()));
        }

        let config_path = module_dir.join("config");
        let worktree = self
            .run_git_command(
                &[
                    "config",
                    "-f",
                    &config_path.display().to_string(),
                    "--get",
                    "core.worktree",
                ],
                PrintMode::Quiet,
            )
            .map(|x| x.into_iter().next())
            .unwrap_or_default();

        match worktree {
            None => {
                Ok(InGitModule {
                    name: name.to_string(),
                    worktree: None,
                    head_sha: None,
                    git_dir: None,
                    // describe: None,
                })
            }
            Some(worktree) => {
                let path = module_dir.join(&worktree);
                let sub_git = match Self::try_from(&path).ok() {
                    Some(sub_git) => sub_git,
                    None => {
                        return Ok(InGitModule {
                            name: name.to_string(),
                            worktree: Some(worktree),
                            head_sha: None,
                            git_dir: None,
                            // describe: None,
                        });
                    }
                };
                let head_sha = sub_git
                    .run_git_command(&["rev-parse", "HEAD"], PrintMode::Quiet)
                    .ok()
                    .and_then(|x| x.into_iter().next());
                let git_dir = sub_git
                    .run_git_command(&["rev-parse", "--git-dir"], PrintMode::Quiet)
                    .ok()
                    .and_then(|x| x.into_iter().next());
                // let describe = head_sha.as_ref().and_then(|head_sha| {
                //         sub_git.run_git_command(&["describe", "--all", head_sha], PrintMode::Quiet)
                //             .ok()
                //     .and_then(|x|x.into_iter().next())
                //     });

                Ok(InGitModule {
                    name: name.to_string(),
                    worktree: Some(worktree),
                    head_sha,
                    git_dir,
                    // describe,
                })
            }
        }
    }

    /// Read the git config and return key-value pairs that starts with "submodule.". This prefix is
    /// removed for the returned keys.
    fn read_submodule_from_config(
        &self,
        config_path: &str,
    ) -> Result<Vec<(String, String)>, GitError> {
        let name_and_values = self.run_git_command(
            &["config", "-f", config_path, "--get-regexp", "submodule"],
            PrintMode::Quiet,
        )?;
        let name_only = self.run_git_command(
            &[
                "config",
                "-f",
                config_path,
                "--name-only",
                "--get-regexp",
                "submodule",
            ],
            PrintMode::Quiet,
        )?;

        let mut name_values = Vec::new();
        for (name, name_and_value) in name_only.iter().zip(name_and_values.iter()) {
            match name_and_value.strip_prefix(name) {
                Some(value) => {
                    let name = match name.trim().strip_prefix("submodule.") {
                        Some(name) => name,
                        None => {
                            continue;
                        }
                    };
                    let value = value.trim();
                    println_verbose!("Found submodule config: {} => {}", name, value);
                    name_values.push((name.to_string(), value.to_string()));
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

    pub fn unset_config<S>(&self, config_path: S, key: &str) -> Result<(), GitError>
    where
        S: AsRef<Path>,
    {
        let config_path = config_path.as_ref().display().to_string();
        self.run_git_command(
            &["config", "-f", &config_path, "--unset", key],
            PrintMode::Normal,
        )?;
        Ok(())
    }

    /// Run the git command from self's working directory
    ///
    /// The output of the command will be returned as a vector of lines.
    pub fn run_git_command(
        &self,
        args: &[&str],
        print: PrintMode,
    ) -> Result<Vec<String>, GitError> {
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
            .stderr(if print == PrintMode::Quiet {
                Stdio::null()
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
                match print {
                    PrintMode::Normal => {
                        println!("{}", line);
                    }
                    PrintMode::Progress => {
                        print_progress!("{}", line);
                    }
                    PrintMode::Quiet => {}
                }
                output.push(line);
            }
        }
        let status = child.wait().map_err(|e| {
            GitError::CommandFailed(
                command.clone(),
                "command did not finish normally".to_string(),
                e,
            )
        })?;
        if print == PrintMode::Progress {
            print::progress_done();
        }
        println_verbose!("Git command finished: {}", status);
        if status.success() {
            Ok(output)
        } else {
            Err(GitError::ExitStatus(command, status))
        }
    }

    /// Get the part in `git -C <path> ...` to run the command in the top level directory
    pub fn get_top_level_switch(&self) -> Result<Option<String>, GitError> {
        let top_level_dir = self.top_level_dir()?;

        let command = match Path::new(".").canonicalize() {
            Ok(cwd) => {
                if &cwd == top_level_dir {
                    None
                } else {
                    let path = pathdiff::diff_paths(top_level_dir, &cwd)
                        .unwrap_or(top_level_dir.to_path_buf());
                    let diff = path.display().to_string();
                    Some(quote_arg(&diff).to_string())
                }
            }
            Err(_) => {
                let top_level = top_level_dir.display().to_string();
                Some(quote_arg(&top_level).to_string())
            }
        };

        Ok(command)
    }
}

/// Print mode for git commands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintMode {
    /// Don't print git output
    Quiet,
    /// Print git output as progress (only show last line)
    Progress,
    /// Print git output as normal
    Normal,
}
