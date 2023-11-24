use std::path::Path;

use crate::git::{quote_arg, GitCanonicalize, GitContext, GitError, PrintMode};
use crate::print::{println_dimmed, println_info, println_verbose, println_warn};

/// Collection of data of a submodule with the same name as identifier
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Submodule {
    /// Data of this submodule in .gitmodules
    pub in_gitmodules: Option<InGitmodules>,
    /// Data of this submodule in .git/config
    pub in_config: Option<InGitConfig>,
    /// Data of this submodule in .git/modules/<name>
    pub in_modules: Option<InGitModule>,
    /// Data of this submodule in the index
    pub in_index: Option<IndexObject>,
}

impl Submodule {
    /// Get the name of the submodule with the best effort.
    ///
    /// Follows the order:
    /// 1. Name in .gitmodules
    /// 2. Name in .git/config
    /// 3. Name as in .git/modules/<name>
    pub fn name(&self) -> Option<&str> {
        if let Some(gitmodules) = &self.in_gitmodules {
            return Some(gitmodules.name.as_str());
        }
        if let Some(config) = &self.in_config {
            return Some(config.name.as_str());
        }
        if let Some(modules) = &self.in_modules {
            return Some(modules.name.as_str());
        }
        None
    }

    /// Get the path of the submodule with the best effort.
    ///
    /// Follows the order:
    /// 1. Path in .gitmodules
    /// 2. Path in the index object
    /// 3. Path in .git/modules/<name>/config (core.worktree)
    pub fn path(&self) -> Option<&str> {
        if let Some(gitmodules) = &self.in_gitmodules {
            if let Some(path) = &gitmodules.path {
                return Some(path.as_str());
            }
        }
        if let Some(index) = &self.in_index {
            return Some(index.path.as_str());
        }
        if let Some(modules) = &self.in_modules {
            if let Some(worktree) = &modules.worktree {
                return Some(worktree.as_str());
            }
        }
        None
    }

    /// Get the paths stored in different places are consistent
    pub fn are_paths_consistent(&self) -> bool {
        todo!()
    }

    // /// Return the path stored in .gitmodules
    // pub fn simple_path(&self) -> Option<&str> {
    //     self.in_gitmodules?.path.as_ref().map(|s| s.as_str())
    // }

    /// Get the URL of the submodule with the best effort.
    ///
    /// Follows the order:
    /// 1. URL in .git/config (first because this is the resolved one)
    /// 2. URL in .gitmodules
    pub fn url(&self) -> Option<&str> {
        if let Some(config) = &self.in_config {
            if let Some(url) = &config.url {
                return Some(url.as_str());
            }
        }
        if let Some(gitmodules) = &self.in_gitmodules {
            if let Some(url) = &gitmodules.url {
                return Some(url.as_str());
            }
        }
        None
    }

    /// Get the update branch of the submodule defined in .gitmodules
    pub fn branch(&self) -> Option<&str> {
        if let Some(gitmodules) = &self.in_gitmodules {
            if let Some(branch) = &gitmodules.branch {
                return Some(branch.as_str());
            }
        }
        None
    }

    /// Get the commit of the submodule in the index
    pub fn index_commit(&self) -> Option<&str> {
        if let Some(index) = &self.in_index {
            return Some(index.sha.as_str());
        }
        None
    }

    pub fn index_commit_short(&self) -> Option<&str> {
        self.index_commit().map(|s| &s[..7])
    }

    /// Get the commit currently checked out
    pub fn head_commit(&self) -> Option<&str> {
        if let Some(modules) = &self.in_modules {
            if let Some(head_sha) = &modules.head_sha {
                return Some(head_sha.as_str());
            }
        }
        None
    }

    pub fn head_commit_short(&self) -> Option<&str> {
        self.head_commit().map(|s| &s[..7])
    }

    pub fn print(&self, context: &GitContext, dir_switch: &str) -> Result<(), GitError> {
        let name = match self.name() {
            Some(name) => format!("\"{name}\""),
            None => "<unknown>".to_string(),
        };
        println_info!("submodule {name}:");
        if let Some(url) = self.url() {
            println_info!("  from {url}");
        }
        if let Some(branch) = self.branch() {
            println_info!("  update branch is {branch}");
        }
        let path = self.path();
        if let Some(index_commit) = self.index_commit() {
            let index_commit_short = &index_commit[..7];
            match path {
                Some(path) => {
                    let describe = {
                        let mut x = None;
                        if let Ok(top_level_dir) = context.top_level_dir() {
                            if let Ok(context) = GitContext::try_from(top_level_dir.join(path)) {
                                x = context.describe(index_commit);
                            }
                        }
                        x
                    };

                    match describe {
                        Some(describe) => {
                            println_info!("  {index_commit_short} \"{path}\" ({describe})");
                        }
                        None => {
                            println_info!("  {index_commit_short} \"{path}\"");
                        }
                    }
                }
                None => {
                    println_info!("  {index_commit_short} <unknown path>");
                }
            };
        }
        if let Some(head_commit) = self.head_commit() {
            if let Some(index_commit) = self.index_commit() {
                if head_commit != index_commit {
                    let head_commit_short = &head_commit[..7];
                    let mut describe = String::new();
                    if let Some(path) = path {
                        if let Ok(top_level_dir) = context.top_level_dir() {
                            if let Ok(context) = GitContext::try_from(top_level_dir.join(path)) {
                                if let Some(x) = context.describe(head_commit) {
                                    describe = format!(" ({x})");
                                }
                            }
                        }
                    }
                    println_warn!("! checked out {head_commit_short}{describe}");
                    println_dimmed!(
                        "    run `magoo{dir_switch} install` to revert all submodules to index"
                    );
                    if let Some(path) = path {
                        let path = quote_arg(path);
                        let git_c = match context.get_top_level_switch()? {
                            Some(x) => format!("git -C {x}"),
                            None => "git".to_string(),
                        };

                        println_dimmed!("    run `{git_c} submodule update --init -- {path}` to revert this submodule to index");
                        println_dimmed!("    run `{git_c} add {path}` update the index to {head_commit_short}{describe}");
                    }
                }
            }
        } else {
            // not initialized
        }
        println_info!();

        Ok(())
    }

    /// Get if the module data and the submodule's worktree is consistent
    ///
    /// Checks that:
    /// - if `worktree` is set, `head_sha` and `git_dir` should also be set
    /// - `git_dir` should resolve to `.git/modules/<name>`
    ///
    /// If `self.in_module` is None, this function returns [`true`] vacuously.
    pub fn is_module_consistent(&self, context: &GitContext) -> Result<bool, GitError> {
        let in_module = match &self.in_modules {
            Some(in_module) => in_module,
            None => return Ok(true),
        };

        if let Some(worktree) = &in_module.worktree {
            let consistent = in_module.head_sha.is_some() && in_module.git_dir.is_some();
            if !consistent {
                return Ok(false);
            }
        }

        Ok(in_module.is_git_dir_consistent(context)?)
    }

    /// Fix inconsistencies with the module data stored in .git/modules/<name>
    ///
    /// Checks that:
    /// - if `worktree` is set and `head_sha` and `git_dir` are both not set, delete
    /// `core.worktree` in `.git/modules/<name>/config`
    /// - if `git_dir` does not resolve to `.git/modules/<name>`, run `git submodule absorbgitdirs`
    pub fn make_module_consistent(&mut self, context: &GitContext) -> Result<FixResult, GitError> {
        let in_module = match &mut self.in_modules {
            Some(in_module) => in_module,
            None => return Ok(FixResult::Clean),
        };

        let mut result = FixResult::Clean;

        if let Some(worktree) = &in_module.worktree {
            let has_head_sha = in_module.head_sha.is_some();
            let has_git_dir = in_module.git_dir.is_some();
            match (has_head_sha, has_git_dir) {
                (true, true) => {}
                (false, false) => {
                    // fix
                    let config_path = context
                        .top_level_dir()?
                        .join(".git/modules")
                        .join(&in_module.name)
                        .join("config");
                    println_warn!(
                        "! Fixing: deleting core.worktree in `{}`",
                        config_path.display()
                    );
                    context.unset_config(&config_path, "core.worktree")?;
                    in_module.worktree = None;
                }
                _ => {
                    println_verbose!(
                        "Cannot auto fix module inconsistency! One of head_sha and git_dir is set."
                    );
                }
            }
        }

        if !in_module.is_git_dir_consistent(context)? {
            // fix
            println_warn!("! Fixing: running `git submodule absorbgitdirs`");
            context.run_git_command(&["submodule", "absorbgitdirs"], PrintMode::Quiet)?;
            result = FixResult::Dirty;
        }

        Ok(result)
    }
}

/// Data of a submodule stored in .gitmodules
#[derive(Debug, Default, Clone, PartialEq)]
pub struct InGitmodules {
    /// Name of the submodule, stored in the section name
    pub name: String,
    /// Path of the submodule, stored as `submodule.<name>.path`
    pub path: Option<String>,
    /// URL of the submodule, stored as `submodule.<name>.url`
    pub url: Option<String>,
    /// Branch of the submodule to update, stored as `submodule.<name>.branch`
    pub branch: Option<String>,
}

impl InGitmodules {
    /// Create a new submodule with the given name and [`None`] for all other fields
    pub fn with_name(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }
}

/// Data of a submodule stored in the index
#[derive(Debug, Default, Clone, PartialEq)]
pub struct IndexObject {
    /// Path of the index object
    pub path: String,
    /// SHA-1 of the index object
    pub sha: String,
}

/// Data of a submodule stored in .git/config
#[derive(Debug, Default, Clone, PartialEq)]
pub struct InGitConfig {
    /// Name of the submodule, stored in the section name
    pub name: String,
    /// URL of the submodule, stored as `submodule.<name>.url`
    pub url: Option<String>,
}

impl InGitConfig {
    pub fn with_name(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }
}

/// Data of submodule stored in .git/modules
#[derive(Debug, Default, Clone, PartialEq)]
pub struct InGitModule {
    /// Name of the submodule, which is the part in the path after `.git/modules/`
    pub name: String,
    /// Path of the submodule worktree, stored in `core.worktree` in `.git/modules/<name>/config`
    pub worktree: Option<String>,
    /// Currently checked out commit. (`git rev-parse HEAD` in the submodule)
    pub head_sha: Option<String>,
    /// Git dir of the submodule (`git rev-parse --git-dir` in the submodule)
    pub git_dir: Option<String>,
    // /// Result of running `git describe --all <head_sha>` in the submodule
    // pub describe: Option<String>
}

impl InGitModule {
    /// Return if `self.git_dir` resolves to `.git/modules/<name>`
    ///
    /// Returns `true` if it's None
    pub fn is_git_dir_consistent(&self, context: &GitContext) -> Result<bool, GitError> {
        if let Some(git_dir) = &self.git_dir {
            let git_dir = Path::new(git_dir).canonicalize_git()?;
            let expected_git_dir = context
                .top_level_dir()?
                .join(".git/modules")
                .join(&self.name);
            if git_dir != expected_git_dir {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

// /A submodule issue that can be fixed with `status --fix`
// ///
// /## Notation
// /Some issues are only possible with certain patterns of submodule data.
// /We have the following notation:
// /- `G`: submodule data in .gitmodules is [`Some`]
// /- `C`: submodule data in .git/config is [`Some`]
// /- `M`: submodule data in .git/modules/<name> is [`Some`]
// /- `I`: submodule data in the index is [`Some`]
// ///
// /Also:
// /- `I.path` is the path in the index
// /- `G.path` is the path in .gitmodules
// /- `M.path` is the path in .git/modules/<name>/config
// ///
// /Each variant has a list of these letters, which are the scenarios the issue corresponds to

/// An issue in the paths in different places
pub struct SubmodulePaths {
    pub in_gitmodules: Option<String>,
    pub in_index: String,
    pub in_modules: Option<String>,
}

/// Result of fixing some issues in the submodule
pub enum FixResult {
    /// No refresh needed after the fix
    Clean,
    /// The issue was fixed, but the data could be dirty and requires the status to be refreshed
    Dirty,
}
