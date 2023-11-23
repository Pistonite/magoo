use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use crate::git::IndexObject;
use crate::print::{println_verbose, print_progress, self, println_info};

use super::{GitError, GitmodulesSubmodule, GitConfigSubmodule, GitModule};

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
    pub fn try_from<S>(working_dir: S) -> Result<Self, GitError> where S: AsRef<Path> {
        if which::which("git").is_err() {
            return Err(GitError::NotInstalled);
        }
        Ok(Self {
            working_dir: working_dir.as_ref().canonicalize().map_err(|e| GitError::WorkingDir(format!("canonicalize failed: {e}")))?,
            git_dir_cell: OnceCell::new(),
            top_level_cell: OnceCell::new(),
        })
    }
}

impl GitContext {

    pub fn print_version_info(&self) -> Result<(), GitError> {
        println_info!("The officially supported git versions are: {}", super::SUPPORTED_GIT_VERSIONS);
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
        let git_dir_path = output
            .first()
            .ok_or_else(|| GitError::GitDir("git did not return the .git directory".to_string()))?;
        let path = self.working_dir.join(git_dir_path)
            .canonicalize()
            .map_err(|_| GitError::GitDir("canonicalize failed".to_string()))?;

        self.git_dir_cell.set(path).unwrap();
        Ok(self.git_dir_cell.get().unwrap())
    }


    /// Get the absolute path to the top level directory
    pub fn top_level_dir(&self) -> Result<&PathBuf, GitError> {
        if let Some(top_level) = self.top_level_cell.get() {
            return Ok(top_level);
        }

        let output = self.run_git_command(&["rev-parse", "--show-toplevel"], PrintMode::Quiet)?;
        let top_dir_path = output
            .first()
            .ok_or_else(|| GitError::TopLevelDir("git did not return the top level directory".to_string()))?;
        let path = self.working_dir.join(top_dir_path)
            .canonicalize()
            .map_err(|_| GitError::TopLevelDir("canonicalize failed".to_string()))?;

        self.top_level_cell.set(path).unwrap();
        Ok(self.top_level_cell.get().unwrap())
    }

    /// Read the .gitmodules file from this repo and return a map of submodule names to
    /// [`GitmodulesSubmodule`]
    ///
    /// Returns an error if any git command fails. Use `.unwrap_or_default()` to get an empty map.
    pub fn read_dot_gitmodules(&self) -> Result<BTreeMap<String, GitmodulesSubmodule>, GitError> {
        let top_level_dir = self.top_level_dir()?;
        let dot_gitmodules_path = top_level_dir.join(".gitmodules");

        let config_entries = self.read_submodule_from_config(&dot_gitmodules_path.display().to_string())?;

        let mut submodules = BTreeMap::new();

        for (key, value) in config_entries {
            if let Some(x) = key.strip_suffix(".path") {
                let entry = submodules.entry(x.to_string()).or_insert_with( || {
                    GitmodulesSubmodule::with_name(x)
                });
                entry.path = Some(value);
            } else if let Some(x) = key.strip_suffix(".url") {
                let entry = submodules.entry(x.to_string()).or_insert_with( || {
                    GitmodulesSubmodule::with_name(x)
                });
                entry.url = Some(value);
            } else if let Some(x) = key.strip_suffix(".branch") {
                let entry = submodules.entry(x.to_string()).or_insert_with( || {
                    GitmodulesSubmodule::with_name(x)
                });
                entry.branch = Some(value);
            }

        }

        println_verbose!("Found {} submodules in .gitmodules: {:?}", submodules.len(), submodules.keys().collect::<Vec<_>>());
        Ok(submodules)
    }

    /// Read the .git/config file and return a map of submodule names to [`GitConfigSubmodule`].
    ///
    /// Returns an error if any git command fails. Use `.unwrap_or_default()` to get an empty map.
    pub fn read_dot_git_config(&self) -> Result<BTreeMap<String, GitConfigSubmodule>, GitError> {
        let git_dir = self.git_dir()?;
        let dot_git_config_path = git_dir.join("config");

        let config_entries = self.read_submodule_from_config(&dot_git_config_path.display().to_string())?;

        let mut submodules = BTreeMap::new();

        for (key, value) in config_entries {
            if let Some(x) = key.strip_suffix(".url") {
                let entry = submodules.entry(x.to_string()).or_insert_with( || {
                    GitConfigSubmodule::with_name(x)
                });
                entry.url = Some(value);
            }
        }

        println_verbose!("Found {} submodules in .git/config: {:?}", submodules.len(), submodules.keys().collect::<Vec<_>>());
        Ok(submodules)
    }

    /// Use `git ls-files` to list submodules stored in the index.
    ///
    /// Returns a map of path to submodule [`IndexObject`].
    pub fn read_submodules_in_index(&self) -> Result<BTreeMap<String, IndexObject>, GitError> {
        let output = self.run_git_command(&["ls-files", r#"--format=%(objectmode) %(objectname) %(path)"#], PrintMode::Progress)?;
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
            let sha = parts.next().ok_or_else(|| GitError::InvalidIndex("missing commit hash in output".to_string()))?;
            let path = parts.next().ok_or_else(|| GitError::InvalidIndex("missing path in output".to_string()))?;
            submodules.insert(path.to_string(), IndexObject {
                sha: sha.to_string(),
                path: path.to_string(),
            });

        }

        println_verbose!("Found {} submodules in index: {:?}", submodules.len(), submodules.keys().collect::<Vec<_>>());
        Ok(submodules)
    }

    /// Read .git/modules and find all entries.
    pub fn find_all_git_modules(&self) -> Result<BTreeMap<String, GitModule>, GitError> {
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

    fn find_git_modules_recursively(&self, name: Option<&str>, dir_path: &Path, modules: &mut BTreeMap<String, GitModule>) {
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
                        println_verbose!("Failed to read directory entry in `{}`: {e}", dir_path.display());
                        continue;
                    }
                    Ok(entry) => entry,
                };
                let full_path = entry.path();
                if full_path.is_dir() {
                    let entry_file_name = entry.file_name();
                    let entry_name_utf8 = match entry_file_name.to_str() {
                        None => {
                            println_verbose!("File name is not unicode: `{}`", entry_file_name.to_string_lossy());
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
    pub fn read_git_module(&self, name: &str) -> Result<GitModule, GitError> {
        let git_dir = self.git_dir()?;
        let module_dir = git_dir.join("modules").join(name);
        if !module_dir.exists() {
            println_verbose!("Module `{name}` not found in .git/modules");
            return Err(GitError::ModuleNotFound(name.to_string()));
        }

        let config_path = module_dir.join("config");
        let worktree = 
        self.run_git_command(&["config", "-f", &config_path.display().to_string(), "--get", "core.worktree"], PrintMode::Quiet)
            .map(|x|x.into_iter().next())
            .unwrap_or_default();

        match worktree {
            None => {
                Ok(GitModule {
                    name: name.to_string(),
                    worktree: None,
                    head_sha: None,
                    git_dir: None,
                })
            },
            Some(worktree) => {
            let path = module_dir.join(&worktree);
                let sub_git = match Self::try_from(&path).ok() {
                    Some(sub_git) => sub_git,
                    None => {
                        return
                        Ok(GitModule {
                            name: name.to_string(),
                            worktree: Some(worktree),
                            head_sha: None,
                            git_dir: None,
                        })
                    }
                };
            let head_sha = sub_git.run_git_command(&["rev-parse", "HEAD"], PrintMode::Quiet)
                    .ok()
                .and_then(|x|x.into_iter().next());
            let git_dir = sub_git.run_git_command(&["rev-parse", "--git-dir"], PrintMode::Quiet)
                    .ok()
                .and_then(|x|x.into_iter().next());

                Ok(GitModule {
                    name: name.to_string(),
                    worktree: Some(worktree),
                    head_sha,
                    git_dir,
                })
            }
        }

    }

    /// Read the git config and return key-value pairs that starts with "submodule.". This prefix is
    /// removed for the returned keys.
    fn read_submodule_from_config(&self, config_path: &str) -> Result<Vec<(String, String)>, GitError> {
        let name_and_values = self.run_git_command(&["config", "-f", config_path, "--get-regexp", "submodule"], PrintMode::Quiet)?;
        let name_only = self.run_git_command(&["config", "-f", config_path, "--name-only", "--get-regexp", "submodule"], PrintMode::Quiet)?;

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
                    return Err(GitError::InvalidConfig("unexpected config key mismatch in git output.".to_string()));
                }
            }
        }

        Ok(name_values)
    }

    /// Run the git command from self's working directory
    ///
    /// The output of the command will be returned as a vector of lines.
    pub fn run_git_command(&self, args: &[&str], print: PrintMode) -> Result<Vec<String>, GitError> {
        println_verbose!("Running `git {}`", args.iter().map(|x| if x.contains(' ') { format!("\"{x}\"") } else { x.to_string() }).collect::<Vec<_>>().join(" "));

        let mut child = Command::new("git")
            .args(args)
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn().map_err(|e| GitError::SpawnFailed(e.to_string()))?;

        let mut output = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let line = line.map_err(|e| GitError::OutputFailed(e.to_string()))?;
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
        let status = child.wait().map_err(|e| GitError::NoReturn(e.to_string()))?;
        if print == PrintMode::Progress {
            print::progress_done();
        }
        println_verbose!(
            "Git command finished: {}",
            status
        );
        if status.success() {
            Ok(output)
        } else {
            Err(GitError::ExitFail(status))
        }
    }

    /// Get the command to `git add <arg>` that can be ran in the working directory of the current
    /// process. `arg` is the path from the top level directory.
    pub fn get_git_add_command(&self, arg: &str) -> Result<String, GitError> {
        let top_level_dir = self.top_level_dir()?;
        let arg = if arg.contains(' ') {
            format!("\'{arg}\'")
        } else {
            arg.to_string()
            };

        let command = match Path::new(".").canonicalize() {
            Ok(cwd) => {
                if &cwd == top_level_dir {
                    format!("git add {arg}")
                } else {
                    let diff = pathdiff::diff_paths(
                        top_level_dir, &cwd)
                        .unwrap_or(top_level_dir.to_path_buf()).display().to_string();
                    if diff.contains(' ') {
                        format!("git -C '{diff}' add {arg}")
                    } else {
                        format!("git -C {diff} add {arg}")
                    }
                }
            }
            Err(_) => {
                let top_level = top_level_dir.display().to_string();
                if top_level.contains(' ') {
                    format!("git -C '{top_level}' add {arg}")
                } else {
                    format!("git -C {top_level} add {arg}")
                }
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
    Normal
}
