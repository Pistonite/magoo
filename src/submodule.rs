//! Submodule data and operations

use std::path::{Path, PathBuf};

use crate::git::{quote_arg, GitCanonicalize, GitCmdPath, GitContext, GitError};
use crate::print::{
    print_info, print_warn, println_error, println_hint, println_info, println_verbose,
    println_warn,
};

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

    /// Get the URL of the submodule with the best effort.
    ///
    /// Follows the order:
    /// 1. URL in .git/config (first because this is the resolved one)
    /// 2. URL in .gitmodules
    pub fn url(&self) -> Option<&str> {
        if let Some(config) = &self.in_config {
            return Some(config.url.as_str());
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

    /// Get the short version (8 digits) of the commit in the index
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

    /// Get the short version (8 digits) of the commit currently checked out
    pub fn head_commit_short(&self) -> Option<&str> {
        self.head_commit().map(|s| &s[..7])
    }

    /// Print status
    pub fn print(
        &self,
        context: &GitContext,
        dir_switch: &str,
        long: bool,
    ) -> Result<(), GitError> {
        let name = match self.name() {
            Some(name) => format!("\"{name}\""),
            None => "<unknown>".to_string(),
        };

        if long {
            println_info!("submodule {name}:");
            if let Some(url) = self.url() {
                println_info!("  from {url}");
            }
            if let Some(branch) = self.branch() {
                println_info!("  update branch is {branch}");
            }
        } else {
            print_info!("{name:<15}");
        }

        let path = self.path();
        if let Some(index_commit) = self.index_commit() {
            let index_commit_short = &index_commit[..7];
            if long {
                print_info!("  {index_commit_short}");
            } else {
                print_info!(" at {index_commit_short}");
            }
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

                    print_info!(" \"{path}\"");
                    if let Some(describe) = describe {
                        print_info!(" ({describe})");
                    }
                }
                None => {
                    print_warn!("<unknown path>");
                }
            };
            if long {
                println_info!();
            }
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
                    if long {
                        println_warn!("! checked out {head_commit_short}{describe}");
                        if let Some(path) = path {
                            let path = quote_arg(path);
                            let git_c = match context.get_top_level_switch()? {
                                Some(x) => format!("git -C {x}"),
                                None => "git".to_string(),
                            };

                            println_hint!("    run `{git_c} submodule update -- {path}` to revert this submodule to index (`magoo{dir_switch} install` to revert all)");
                            println_hint!("    run `{git_c} add {path}` update the index to {head_commit_short}{describe}");
                        } else {
                            println_hint!(
                            "    run `magoo{dir_switch} install` to revert all submodules to index"
                        );
                        }
                    } else {
                        print_warn!(", checked out {head_commit_short}{describe}");
                    }
                }
            }
        } else {
            // not initialized
            if let Some(path) = path {
                if long {
                    println_warn!("! not initialized");
                    let path = quote_arg(path);
                    let git_c = match context.get_top_level_switch()? {
                        Some(x) => format!("git -C {x}"),
                        None => "git".to_string(),
                    };

                    println_hint!(
                        "    run `magoo{dir_switch} install` to initialize all submodules"
                    );
                    println_hint!("    run `{git_c} submodule update --init -- {path}` to initialize only this submodule");
                } else {
                    print_warn!(", not initialized");
                }
            }
        }

        if !long {
            println_info!();
        }

        if !self.is_module_consistent(context)? {
            println_error!("! submodule has residue");
            println_hint!("    run `magoo{dir_switch} status --fix` to fix all submodules");
        }
        if !self.resolved_paths(context)?.is_consistent() {
            println_error!("! inconsistent paths");
            println_hint!("    run `magoo{dir_switch} status --fix` to fix all submodules");
        }
        let issue = self.find_issue();
        if issue != PartsIssue::None {
            println_error!("! inconsistent state ({})", issue.describe());
            println_hint!("    run `magoo{dir_switch} status --fix` to fix all submodules");
        }

        if long {
            println_info!();
        }

        Ok(())
    }

    /// Return false if the submodule has issues that can be fixed with [`fix`]
    pub fn is_healthy(&self, context: &GitContext) -> Result<bool, GitError> {
        if !self.is_module_consistent(context)? {
            return Ok(false);
        }
        if !self.resolved_paths(context)?.is_consistent() {
            return Ok(false);
        }
        if self.find_issue() != PartsIssue::None {
            return Ok(false);
        }
        Ok(true)
    }

    /// Get if the module data and the submodule's worktree is consistent, see [`InGitModule::is_consistent`]
    pub fn is_module_consistent(&self, context: &GitContext) -> Result<bool, GitError> {
        let in_module = match &self.in_modules {
            Some(in_module) => in_module,
            None => return Ok(true),
        };

        in_module.is_consistent(context)
    }

    /// Resolves the paths stored in various places and return them
    pub fn resolved_paths(&self, context: &GitContext) -> Result<SubmodulePaths, GitError> {
        let mut path_in_gitmodules = None;
        if let Some(in_gitmodules) = &self.in_gitmodules {
            if let Some(path) = &in_gitmodules.path {
                let top_level_dir = context.top_level_dir()?;
                if let Ok(path) = top_level_dir.join(path).canonicalize() {
                    path_in_gitmodules = Some(path);
                }
            }
        }

        let mut path_in_index = None;
        if let Some(in_index) = &self.in_index {
            let top_level_dir = context.top_level_dir()?;
            if let Ok(path) = top_level_dir.join(&in_index.path).canonicalize() {
                path_in_index = Some(path);
            }
        }

        let mut path_in_module = None;
        if let Some(in_module) = &self.in_modules {
            if let Some(worktree) = &in_module.worktree {
                let git_dir = context.git_dir()?;
                if let Ok(path) = git_dir
                    .join("modules")
                    .join(&in_module.name)
                    .join(worktree)
                    .canonicalize()
                {
                    path_in_module = Some(path);
                }
            }
        }

        Ok(SubmodulePaths {
            in_gitmodules: path_in_gitmodules,
            in_index: path_in_index,
            in_modules: path_in_module,
        })
    }

    /// Fix the submodule
    ///
    /// This will result in one of the following states:
    /// 1. The submodule is healthy and initialized.
    /// 2. The submodule is healthy but not initialized.
    /// 3. The submodule is deleted.
    pub fn fix(&mut self, context: &GitContext) -> Result<(), GitError> {
        // the submodule can be in any shape or form
        // here are some notations:
        // - `G`: submodule data in .gitmodules is [`Some`]
        // - `C`: submodule data in .git/config is [`Some`]
        // - `M`: submodule data in .git/modules/<name> is [`Some`]
        // - `I`: submodule data in the index is [`Some`]

        // First, we want to be in a state where, if a component exists, it is consistent internally and with others
        if !self.is_module_consistent(context)? {
            self.force_remove_module_dir(context)?;
        }

        // make sure all paths are consistent
        // The paths are in G, M and I
        let resolved_paths = self.resolved_paths(context)?;
        if !resolved_paths.is_consistent() {
            // we only fix the paths if index exists
            // if index doesn't exist, the submodule will be deleted anyway
            if let Some(in_index) = &self.in_index {
                let index_path = in_index.path.clone();
                // path exists in index
                if self.in_modules.is_some() && resolved_paths.in_index != resolved_paths.in_modules
                {
                    // module has different path, delete it
                    self.force_remove_module_dir(context)?;
                }
                if let Some(in_gitmodules) = &self.in_gitmodules {
                    if resolved_paths.in_index != resolved_paths.in_gitmodules {
                        let name = &in_gitmodules.name;
                        // gitmodules has different path, update it
                        let top_level_dir = context.top_level_dir()?;
                        context.set_config(
                            top_level_dir.join(".gitmodules"),
                            &format!("submodule.\"{name}\".path"),
                            Some(&index_path),
                        )?;
                    }
                }
            }
        }

        self.fix_issue(self.find_issue(), context)?;
        Ok(())
    }

    fn fix_issue(&mut self, issue: PartsIssue, context: &GitContext) -> Result<(), GitError> {
        match issue {
            PartsIssue::None => {
                // submodule is healthy
            }
            PartsIssue::Residue => {
                // submodule is not initialized but module dir exists
                println_verbose!("Fix: removing uninitialized submodule directory and worktree");
                self.force_remove_config(context)?;
                self.force_remove_module_dir(context)?;
            }
            PartsIssue::MissingIndex => {
                // index is missing, delete it
                println_verbose!("Fix: deleting submodule missing in index");
                self.force_delete(context)?;
            }
            PartsIssue::MissingInGitModules => {
                // submodule is not in .gitmodules
                // delete it
                println_verbose!("Fix: deleting submodule missing in .gitmodules");
                self.force_delete(context)?;
            }
        };
        Ok(())
    }

    fn find_issue(&self) -> PartsIssue {
        match (
            &self.in_gitmodules,
            &self.in_config,
            &self.in_modules,
            &self.in_index,
        ) {
            (None, None, None, None) => {
                // submodule doesn't exist
                PartsIssue::None
            }
            (Some(_), Some(_), Some(_), Some(_)) => {
                // initialized and all good
                PartsIssue::None
            }
            (Some(_), None, None, Some(_)) => {
                // submodule is in .gitmodules and index (not initialized)
                // nothing to fix
                PartsIssue::None
            }
            (Some(_), Some(_), None, Some(_)) => {
                // there are remains after submodule is deinitialized
                PartsIssue::Residue
            }
            (Some(_), None, Some(_), Some(_)) => PartsIssue::Residue,
            (_, _, _, None) => {
                // index is missing
                PartsIssue::MissingIndex
            }
            (None, _, _, _) => {
                // submodule is not in .gitmodules
                PartsIssue::MissingInGitModules
            }
        }
    }

    /// Delete the submodule by removing the configuration and directories that reference it
    pub fn force_delete(&mut self, context: &GitContext) -> Result<(), GitError> {
        self.force_remove_from_index(context)?;
        self.force_remove_module_dir(context)?;
        self.force_remove_config(context)?;
        self.force_remove_from_dot_gitmodules(context)?;

        Ok(())
    }

    /// Delete the submodule section in .gitmodules
    pub fn force_remove_from_dot_gitmodules(
        &mut self,
        context: &GitContext,
    ) -> Result<(), GitError> {
        if let Some(in_gitmodules) = &self.in_gitmodules {
            let top_level_dir = context.top_level_dir()?;
            let name = &in_gitmodules.name;
            println_info!("Deleting submodule `{name}` in .gitmodules");
            let _ = context.remove_config_section(
                top_level_dir.join(".gitmodules"),
                &format!("submodule.{name}"),
            );
            context.add(".gitmodules")?;
        }
        self.in_gitmodules = None;
        Ok(())
    }

    /// Remove the submodule from index
    pub fn force_remove_from_index(&mut self, context: &GitContext) -> Result<(), GitError> {
        if let Some(in_index) = &self.in_index {
            println_info!("Deleting `{}` in index", in_index.path);
            context.remove_from_index(&in_index.path)?;
            context.add(".gitmodules")?;
        }
        self.in_index = None;
        Ok(())
    }

    /// Delete the submodule in .git/modules/<name> and its worktree if present
    pub fn force_remove_module_dir(&mut self, context: &GitContext) -> Result<(), GitError> {
        if let Some(in_module) = &self.in_modules {
            let name = &in_module.name;
            let git_dir = context.git_dir()?;
            let module_dir = git_dir.join("modules").join(name);
            if module_dir.exists() {
                // delete worktree directory if exists
                if let Some(worktree) = &in_module.worktree {
                    let worktree_path = module_dir.join(worktree);
                    if worktree_path.exists() {
                        println_info!(
                            "Deleting the worktree of submodule `{name}` at `{}`",
                            worktree_path.to_cmd_arg()
                        );
                        let _ = std::fs::remove_dir_all(worktree_path);
                    }
                }
                // delete the module directory
                println_info!("Deleting `.git/modules/{name}`");
                let _ = std::fs::remove_dir_all(module_dir);
            }
        }
        self.in_modules = None;
        Ok(())
    }

    /// Delete the submodule in .git/config
    pub fn force_remove_config(&mut self, context: &GitContext) -> Result<(), GitError> {
        if let Some(in_config) = &self.in_config {
            let git_dir = context.git_dir()?;
            let name = &in_config.name;
            println_info!("Deleting submodule `{name}` in .git/config");
            let _ =
                context.remove_config_section(git_dir.join("config"), &format!("submodule.{name}"));
        }
        self.in_config = None;
        Ok(())
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
    pub url: String,
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
    /// Return if data is consistent internally
    ///
    /// For the data to be consistent:
    /// - if `worktree` is set, `head_sha` and `git_dir` should also be set
    /// - `git_dir` should resolve to `.git/modules/<name>`
    pub fn is_consistent(&self, context: &GitContext) -> Result<bool, GitError> {
        if self.worktree.is_some() {
            let consistent = self.head_sha.is_some() && self.git_dir.is_some();
            if !consistent {
                return Ok(false);
            }
        }
        if let Some(git_dir) = &self.git_dir {
            let git_dir = Path::new(git_dir).canonicalize_git()?;
            let expected_git_dir = context.git_dir()?.join("modules").join(&self.name);
            if git_dir != expected_git_dir {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

/// An issue in the paths in different places
pub struct SubmodulePaths {
    pub in_gitmodules: Option<PathBuf>,
    pub in_index: Option<PathBuf>,
    pub in_modules: Option<PathBuf>,
}

impl SubmodulePaths {
    pub fn is_consistent(&self) -> bool {
        if self.in_gitmodules == self.in_index && self.in_index == self.in_modules {
            return true;
        }
        if self.in_gitmodules == self.in_index && self.in_modules.is_none() {
            return true;
        }
        false
    }
}

#[derive(Debug, Clone, PartialEq)]
enum PartsIssue {
    None,
    Residue,
    MissingIndex,
    MissingInGitModules,
}
impl PartsIssue {
    pub fn describe(&self) -> &'static str {
        match self {
            PartsIssue::None => "none",
            PartsIssue::Residue => "has residue from removal",
            PartsIssue::MissingIndex => "missing in index",
            PartsIssue::MissingInGitModules => "not in .gitmodules",
        }
    }
}
