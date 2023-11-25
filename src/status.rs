use std::collections::BTreeMap;
use std::path::Path;

use crate::git::{GitContext, GitError};
use crate::print::println_verbose;
use crate::submodule::*;

/// Data returned from [`GitContext::submodule_status`]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Status {
    /// The submodule status map from name to [`Submodule`]
    pub modules: BTreeMap<String, Submodule>,
    /// The submodules that only exist in the index (thus don't have a name, only a path and a
    /// SHA-1)
    pub nameless: Vec<Submodule>,
}

macro_rules! insert_with_name {
    ($modules:expr, $name:ident) => {{
        let m = $modules;
        if let Some(s) = m.get_mut($name) {
            s.in_gitmodules.as_mut().unwrap()
        } else {
            m.insert(
                $name.to_string(),
                Submodule {
                    in_gitmodules: Some(InGitmodules::with_name($name)),
                    in_config: None,
                    in_index: None,
                    in_modules: None,
                },
            );
            m.get_mut($name).unwrap().in_gitmodules.as_mut().unwrap()
        }
    }};
}

impl Status {
    /// Return a flattened view of all the submodules
    ///
    /// If the status was created with the `--all` flag, it will also include the nameless
    /// submodules
    pub fn flattened(&self) -> Vec<&Submodule> {
        let mut modules = self.modules.values().collect::<Vec<_>>();
        for index_obj in &self.nameless {
            modules.push(index_obj);
        }
        modules
    }

    /// Return a flattened view of all the submodules
    ///
    /// If the status was created with the `--all` flag, it will also include the nameless
    /// submodules
    pub fn flattened_mut(&mut self) -> Vec<&mut Submodule> {
        let mut modules = self.modules.values_mut().collect::<Vec<_>>();
        for index_obj in self.nameless.iter_mut() {
            modules.push(index_obj);
        }
        modules
    }

    /// Flattens the submodules into a vector of [`Submodule`]
    ///
    /// If the status was created with the `--all` flag, it will also include the nameless
    /// submodules
    pub fn into_flattened(self) -> Vec<Submodule> {
        let mut modules = self.modules.into_values().collect::<Vec<_>>();
        for index_obj in self.nameless {
            modules.push(index_obj);
        }
        modules
    }

    /// Get a view of the submodules that only exist in the index
    pub fn nameless_objects(&self) -> Vec<&IndexObject> {
        self.nameless
            .iter()
            .map(|s| s.in_index.as_ref().unwrap())
            .collect()
    }

    pub fn is_healthy(&self, context: &GitContext) -> Result<bool, GitError> {
        for submodule in self.flattened() {
            if !submodule.is_healthy(context)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Get the submodule status in the repository.
    ///
    /// If `all` is false, it will not include submodules that are only in the index and in
    /// `.git/modules`
    pub fn read_from(context: &GitContext, all: bool) -> Result<Self, GitError> {
        let mut status = Self::default();
        status.read_dot_gitmodules(context)?;
        status.read_dot_git_config(context)?;
        // read .git/modules
        if all {
            status.find_all_git_modules(context)?;
        } else {
            for (name, submodule) in status.modules.iter_mut() {
                if let Ok(module) = Self::read_git_module(name, context) {
                    submodule.in_modules = Some(module);
                }
            }
        };
        status.read_submodules_in_index(context, all)?;

        Ok(status)
    }

    /// Read the `.gitmodules` data into self
    fn read_dot_gitmodules(&mut self, context: &GitContext) -> Result<(), GitError> {
        let top_level_dir = context.top_level_dir()?;
        let dot_gitmodules_path = top_level_dir.join(".gitmodules");

        let config_entries =
            Self::read_submodule_from_config(context, &dot_gitmodules_path.display().to_string())?;

        for (key, value) in config_entries {
            let name = if let Some(name) = key.strip_suffix(".path") {
                insert_with_name!(&mut self.modules, name).path = Some(value);
                name
            } else if let Some(name) = key.strip_suffix(".url") {
                insert_with_name!(&mut self.modules, name).url = Some(value);
                name
            } else if let Some(name) = key.strip_suffix(".branch") {
                insert_with_name!(&mut self.modules, name).branch = Some(value);
                name
            } else {
                continue;
            };

            println_verbose!("Found submodule in .gitmodules: {name}");
        }
        Ok(())
    }

    /// Read the `.git/config` data into self
    fn read_dot_git_config(&mut self, context: &GitContext) -> Result<(), GitError> {
        let git_dir = context.git_dir()?;
        let dot_git_config_path = git_dir.join("config");

        let config_entries = match Self::read_submodule_from_config(
            context,
            &dot_git_config_path.display().to_string(),
        ) {
            Ok(entries) => entries,
            Err(e) => {
                println_verbose!("Git error when reading submodules from .git/config, assuming no submodules: {e}");
                return Ok(());
            }
        };

        for (key, value) in config_entries {
            if let Some(name) = key.strip_suffix(".url") {
                println_verbose!("Found submodule in .git/config: {}", name);
                let submodule = InGitConfig {
                    name: name.to_string(),
                    url: value,
                };

                if let Some(s) = self.modules.get_mut(name) {
                    s.in_config = Some(submodule);
                } else {
                    self.modules.insert(
                        name.to_string(),
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

        Ok(())
    }

    /// Read the git config and return key-value pairs that starts with "submodule.". This prefix is
    /// removed for the returned keys.
    fn read_submodule_from_config(
        context: &GitContext,
        config_path: &str,
    ) -> Result<Vec<(String, String)>, GitError> {
        let name_values = context.get_config_regexp(config_path, "submodule")?;
        let name_values = name_values
            .into_iter()
            .filter_map(|(name, value)| {
                let name = name.strip_prefix("submodule.")?;
                println_verbose!("Found submodule config: {} => {}", name, value);
                Some((name.to_string(), value))
            })
            .collect::<Vec<_>>();

        Ok(name_values)
    }

    /// Read .git/modules and find all entries and put them in self
    fn find_all_git_modules(&mut self, context: &GitContext) -> Result<(), GitError> {
        let git_dir = context.git_dir()?;
        let module_dir = git_dir.join("modules");
        if !module_dir.exists() {
            println_verbose!(".git/modules does not exist");
        } else {
            self.find_git_modules_recursively(context, None, &module_dir);
        }
        Ok(())
    }

    fn find_git_modules_recursively(
        &mut self,
        context: &GitContext,
        name: Option<&str>,
        dir_path: &Path,
    ) {
        println_verbose!("Scanning for git modules in `{}`", dir_path.display());
        let config_path = dir_path.join("config");
        if config_path.is_file() {
            if let Some(name) = name {
                // dir_path is a git module
                match Self::read_git_module(name, context) {
                    Err(e) => {
                        println_verbose!("Failed to read git module `{name}`: {e}");
                    }
                    Ok(module) => {
                        println_verbose!("Found git module `{name}`");
                        if let Some(s) = self.modules.get_mut(name) {
                            s.in_modules = Some(module);
                        } else {
                            self.modules.insert(
                                name.to_string(),
                                Submodule {
                                    in_gitmodules: None,
                                    in_config: None,
                                    in_index: None,
                                    in_modules: Some(module),
                                },
                            );
                        }
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
                    self.find_git_modules_recursively(context, Some(&next_name), &full_path);
                }
            }
        }
    }

    /// Read `.git/modules/<name>`
    fn read_git_module(name: &str, context: &GitContext) -> Result<InGitModule, GitError> {
        let git_dir = context.git_dir()?;
        let module_dir = git_dir.join("modules").join(name);
        if !module_dir.exists() {
            println_verbose!("Module `{name}` not found in .git/modules");
            return Err(GitError::ModuleNotFound(name.to_string()));
        }

        let config_path = module_dir.join("config");
        let worktree = context
            .get_config(config_path, "core.worktree")
            .unwrap_or_default();

        match worktree {
            None => Ok(InGitModule {
                name: name.to_string(),
                worktree: None,
                head_sha: None,
                git_dir: None,
            }),
            Some(worktree) => {
                let path = module_dir.join(&worktree);
                let sub_git = match GitContext::try_from(path).ok() {
                    Some(sub_git) => sub_git,
                    None => {
                        return Ok(InGitModule {
                            name: name.to_string(),
                            worktree: Some(worktree),
                            head_sha: None,
                            git_dir: None,
                        });
                    }
                };
                let head_sha = sub_git.head().unwrap_or_default();
                let git_dir = sub_git.git_dir_raw().unwrap_or_default();

                Ok(InGitModule {
                    name: name.to_string(),
                    worktree: Some(worktree),
                    head_sha,
                    git_dir,
                })
            }
        }
    }

    /// Use `git ls-files` to list submodules stored in the index into self
    fn read_submodules_in_index(
        &mut self,
        context: &GitContext,
        all: bool,
    ) -> Result<(), GitError> {
        let index_list = context.ls_files(&[r#"--format=%(objectmode) %(objectname) %(path)"#])?;

        let mut path_to_index_object = BTreeMap::new();

        for line in index_list {
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

            path_to_index_object.insert(
                path.to_string(),
                IndexObject {
                    sha: sha.to_string(),
                    path: path.to_string(),
                },
            );
        }

        for submodule in self.modules.values_mut() {
            let path = match submodule.path() {
                Some(path) => path,
                None => continue,
            };
            if let Some(index_obj) = path_to_index_object.remove(path) {
                println_verbose!(
                    "Connect index path `{}` to submodule `{}`",
                    path,
                    submodule.name().unwrap_or_default()
                );
                submodule.in_index = Some(index_obj);
            }
        }

        if all {
            for index_obj in path_to_index_object.into_values() {
                self.nameless.push(Submodule {
                    in_gitmodules: None,
                    in_config: None,
                    in_index: Some(index_obj),
                    in_modules: None,
                });
            }
        }
        Ok(())
    }
}
