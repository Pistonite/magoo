//! # Magoo
//!
//! A wrapper for `git submodule` commands.
//!
//! ## CLI Usage
//! ```bash
//! cargo install magoo
//! magoo --help
//! ```
//! See [README](https://github.com/Pistonite/magoo) for more information.
//!
//! ## Library Usage
//! ```bash
//! cargo add magoo
//! ```
//! If you don't need `clap` for parsing arguments, you can add `--no-default-features` to
//! exclude the dependency.
//!
//! ### Examples
//! #### Run a command
//! ```rust
//! use magoo::{StatusCommand, PrintOptions};
//!
//! let command = magoo::StatusCommand {
//!     git: true,
//!     fix: false,
//!     long: false,
//!     options: PrintOptions {
//!         verbose: false,
//!         quiet: false,
//!         color: None,
//!     },
//!     delete: false,
//! };
//!
//! // don't need this if you don't need output to stdout
//! command.set_print_options();
//! // runs `magoo status --git` in the current directory
//! command.run("."); //.unwrap();
//! ```
//! #### Use `clap` to parse arguments
//! ```rust
//! use magoo::Magoo;
//! use clap::Parser;
//!
//! // for assertion below only
//! use magoo::{Command, StatusCommand, PrintOptions};
//!
//! let magoo = Magoo::try_parse_from(["magoo", "--dir", "my/repo", "status", "--long", "--verbose"]).unwrap();
//!
//! assert_eq!(magoo, Magoo {
//!     subcmd: Command::Status(StatusCommand {
//!         git: false,
//!         fix: false,
//!         long: true,
//!         options: PrintOptions {
//!             verbose: true,
//!             quiet: false,
//!             color: None,
//!         },
//!         delete: false,
//!     }),
//!     dir: "my/repo".to_string(),
//! });
//!
//! magoo.set_print_options();
//! magoo.run(); //.unwrap();
//! ```
//! You can also look at [main.rs](https://github.com/Pistonite/magoo/blob/master/src/main.rs) for
//! reference.
//!

pub mod git;
pub use git::SUPPORTED_GIT_VERSIONS;
use git::{GitContext, GitError};

pub mod print;
pub mod status;
pub mod submodule;
use status::Status;

use crate::print::{println_error, println_hint, println_info, println_verbose, println_warn};

/// The main entry point for the library
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "cli", derive(clap::Parser))]
#[cfg_attr(
    feature = "cli",
    command(author, about, version, arg_required_else_help(true))
)]
pub struct Magoo {
    /// Command to run
    #[clap(subcommand)]
    pub subcmd: Command,
    /// Set the working directory of commands. Useful if not running inside a git repository.
    #[cfg_attr(feature = "cli", clap(long, short('C'), default_value(".")))]
    pub dir: String,
}

impl Magoo {
    /// Run the command
    pub fn run(&self) -> Result<(), GitError> {
        self.subcmd.run(&self.dir)
    }

    /// Apply the print options
    pub fn set_print_options(&self) {
        self.subcmd.set_print_options();
    }
}

/// Subcommands
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "cli", derive(clap::Parser))]
pub enum Command {
    /// Print the status of the submodules in the current git repository.
    Status(StatusCommand),
    /// Add or install dependencies
    ///
    /// Installs dependencies if no arguments are provided.
    /// Otherwise, adds the provided dependency as a git submodule.
    Install(InstallCommand),
    /// Updates all dependencies or the specified dependency.
    ///
    /// Dependencies will be updated to the branch (specified when adding the dependency) from the
    /// remote.
    Update(UpdateCommand),
    /// Remove a dependency
    Remove(RemoveCommand),
}

impl Command {
    /// Apply the print options
    pub fn set_print_options(&self) {
        match self {
            Command::Status(cmd) => cmd.set_print_options(),
            Command::Install(cmd) => cmd.set_print_options(),
            Command::Update(cmd) => cmd.set_print_options(),
            Command::Remove(cmd) => cmd.set_print_options(),
        }
    }

    /// Run the command in the given directory.
    pub fn run(&self, dir: &str) -> Result<(), GitError> {
        match self {
            Command::Status(cmd) => {
                cmd.run(dir)?;
            }
            Command::Install(cmd) => {
                cmd.run(dir)?;
            }
            Command::Update(cmd) => {
                cmd.run(dir)?;
            }
            Command::Remove(cmd) => {
                cmd.run(dir)?;
            }
        }

        Ok(())
    }
}

/// The `status` command
#[derive(Debug, Default, Clone, PartialEq)]
#[cfg_attr(feature = "cli", derive(clap::Parser))]
pub struct StatusCommand {
    /// Show the current git version and if it is supported
    #[cfg_attr(feature = "cli", clap(long))]
    pub git: bool,

    /// Show more information in a longer format
    #[cfg_attr(feature = "cli", clap(long, short))]
    pub long: bool,

    /// Fix the submodules to be in a consistent state. (CAUTION - you should never have to do this if you let magoo manage the submodules, be sure to read the details in `magoo status --help` before using!)
    ///
    /// If any submodule appears to be broken (likely due to changing
    /// the git files manually), this will attempt to bring the submodule back
    /// to a consistent state by de-initializing it.
    ///
    /// USE WITH CAUTION - If the submodule state is so broken that there's not enough information
    /// to fix it, it will be removed from existence.
    /// This may delete local files and directories that look like submodules because they are referenced by git files.
    ///
    #[cfg_attr(feature = "cli", clap(long, short))]
    pub fix: bool,

    /// Prefers deleting the submodule instead of installing it when fixing
    #[cfg_attr(feature = "cli", clap(long, requires("fix")))]
    pub delete: bool,

    /// Print options
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub options: PrintOptions,
}

impl StatusCommand {
    /// Apply the print options
    pub fn set_print_options(&self) {
        self.options.apply();
    }

    /// Run the command and return the status as a [`Status`] struct.
    pub fn run(&self, dir: &str) -> Result<Status, GitError> {
        let context = GitContext::try_from(dir)?;
        let _guard = context.lock()?;
        if self.git {
            context.print_version_info()?;
            return Ok(Status::default());
        }

        let mut status = Status::read_from(&context)?;
        let mut flat_status = status.flattened_mut();
        if flat_status.is_empty() {
            println!("No submodules found");
            return Ok(status);
        }
        if self.fix {
            for submodule in flat_status.iter_mut() {
                submodule.fix(&context, self.delete)?;
            }
            return Ok(status);
        }

        let dir_switch = if dir == "." {
            "".to_string()
        } else {
            format!(" --dir {dir}")
        };

        for submodule in &flat_status {
            submodule.print(&context, &dir_switch, self.long)?;
        }
        Ok(status)
    }
}

/// The `install` command
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "cli", derive(clap::Parser))]
pub struct InstallCommand {
    /// URL of the git repository to add
    ///
    /// See the `add` command of <https://git-scm.com/docs/git-submodule> for what formats are
    /// supported.
    pub url: Option<String>,

    /// Local path to clone the git submodule to
    ///
    /// Unlike the path specified with `git submodule add`, this path should be relative from
    /// the top level (root) of the git repository.
    #[cfg_attr(feature = "cli", arg(requires("url")))]
    pub path: Option<String>,

    /// Branch to checkout and track
    ///
    /// This is the branch reference that will be used when updating the submodule.
    /// If not specified, the behavior is the same as `git submodule add` without `--branch`
    /// (`HEAD` is used)
    #[cfg_attr(feature = "cli", clap(long, short))]
    #[cfg_attr(feature = "cli", arg(requires("url")))]
    pub branch: Option<String>,

    /// Name of the submodule
    ///
    /// If not specified, the name of the submodule is the same as the path.
    #[cfg_attr(feature = "cli", clap(long))]
    #[cfg_attr(feature = "cli", arg(requires("url")))]
    pub name: Option<String>,

    /// Depth to clone the submodule
    #[cfg_attr(feature = "cli", clap(long))]
    #[cfg_attr(feature = "cli", arg(requires("url")))]
    pub depth: Option<usize>,

    /// Whether to force the submodule to be added
    ///
    /// This will pass the `--force` flag to `git submodule add` and `git submodule update`.
    #[cfg_attr(feature = "cli", clap(long, short))]
    pub force: bool,

    /// Don't install submodules recursively, only top-level
    ///
    /// By default, submodules are installed recursively with `git submodule update --recursive`.
    #[cfg_attr(feature = "cli", clap(long))]
    pub no_recursive: bool,

    /// Print options
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub options: PrintOptions,
}

impl InstallCommand {
    /// Apply the print options
    pub fn set_print_options(&self) {
        self.options.apply();
    }

    /// Run the command in the given directory
    pub fn run(&self, dir: &str) -> Result<(), GitError> {
        let context = GitContext::try_from(dir)?;
        let _guard = context.lock()?;

        let mut status = Status::read_from(&context)?;
        for submodule in status.flattened_mut() {
            submodule.fix(&context, false)?;
        }

        match &self.url {
            Some(url) => {
                println_verbose!("Adding submodule from url: {url}");
                context.submodule_add(
                    url,
                    self.path.as_deref(),
                    self.branch.as_deref(),
                    self.name.as_deref(),
                    self.depth.as_ref().copied(),
                    self.force,
                )?;
            }
            None => {
                println_verbose!("Installing submodules");
                context.submodule_init(None)?;
                context.submodule_sync(None, !self.no_recursive)?;
                context.submodule_update(None, self.force, false, !self.no_recursive)?;
            }
        }

        Ok(())
    }
}

/// The `update` command
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "cli", derive(clap::Parser))]
pub struct UpdateCommand {
    /// Name of the submodule to update
    ///
    /// If not specified, all submodules will be updated.
    pub name: Option<String>,

    /// Change the branch of the submodule
    #[cfg_attr(feature = "cli", clap(long, short))]
    #[cfg_attr(feature = "cli", arg(requires("name")))]
    pub branch: Option<String>,

    /// Unset the update branch of the submodule
    #[cfg_attr(feature = "cli", clap(long))]
    #[cfg_attr(feature = "cli", arg(requires("name"), conflicts_with("branch")))]
    pub unset_branch: bool,

    /// Change the url of the submodule
    #[cfg_attr(feature = "cli", clap(long, short))]
    #[cfg_attr(feature = "cli", arg(requires("name")))]
    pub url: Option<String>,

    /// Whether to force the submodule to be updated
    ///
    /// This will pass the `--force` flag to `git submodule update`.
    #[cfg_attr(feature = "cli", clap(long, short))]
    pub force: bool,

    /// Bypass warnings in the submodule state
    #[cfg_attr(feature = "cli", clap(long))]
    pub bypass: bool,

    /// Print options
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub options: PrintOptions,
}

impl UpdateCommand {
    /// Apply the print options
    pub fn set_print_options(&self) {
        self.options.apply();
    }

    /// Run the command in the given directory
    pub fn run(&self, dir: &str) -> Result<(), GitError> {
        let context = GitContext::try_from(dir)?;
        let _guard = context.lock()?;

        match &self.name {
            Some(name) => {
                println_verbose!("Updating submodule: {name}");
                let status = Status::read_from(&context)?;
                let submodule = match status.modules.get(name) {
                    Some(submodule) => submodule,
                    None => {
                        println_error!("Submodule `{name}` not found!");
                        // maybe user passed in path instead of name?
                        println_verbose!("Trying to search for a path matching `{name}`");
                        for submodule in status.flattened() {
                            if let Some(other_name) = submodule.name() {
                                if let Some(path) = submodule.path() {
                                    if path == name {
                                        println_hint!("  however, there is a submodule \"{other_name}\" with path \"{path}\"");
                                        println_hint!("  if you meant to update this submodule, use `magoo update {other_name}`");
                                        break;
                                    }
                                }
                            }
                        }

                        return Err(GitError::NeedFix(false));
                    }
                };
                if !submodule.is_healthy(&context)? {
                    if !self.bypass {
                        println_error!("Submodule `{name}` is not healthy!");
                        println_hint!("  run `magoo status` to investigate. Some issues might be fixable with `magoo status --fix`.");
                        println_hint!("  alternatively, use the `--bypass` flag to ignore and continue anyway.");
                        return Err(GitError::NeedFix(false));
                    }
                    println_warn!("Bypassing warnings from unhealthy submodule `{name}`");
                }

                let path = match submodule.path() {
                    Some(x) => x,
                    None => {
                        println_error!("Submodule `{name}` does not have a path!");
                        println_hint!("  run `magoo status` to investigate.");
                        println_hint!("  if you are unsure of the problem, try hard removing the submodule with `magoo remove {name} --force` and then re-adding it");
                        return Err(GitError::NeedFix(false));
                    }
                };
                context.submodule_init(Some(path))?;
                if self.unset_branch {
                    context.submodule_set_branch(path, None)?;
                } else if let Some(branch) = &self.branch {
                    context.submodule_set_branch(path, Some(branch))?;
                }

                if let Some(url) = &self.url {
                    context.submodule_set_url(path, url)?;
                }

                context.submodule_sync(Some(path), false)?;
                context.submodule_update(Some(path), self.force, true, false)?;
            }
            None => {
                println_verbose!("Updating submodules");
                context.submodule_init(None)?;
                context.submodule_sync(None, false)?;
                context.submodule_update(None, self.force, true, false)?;
            }
        }

        println_info!();
        println_info!("Submodules updated successfully.");
        println_hint!(
            "  run `git status` to check the changes and run `git add ...` to stage them"
        );
        println_hint!("  run `magoo status` to check the status of the submodules");
        Ok(())
    }
}

/// The `remove` command
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "cli", derive(clap::Parser))]
pub struct RemoveCommand {
    /// The name of the submodule to remove
    pub name: String,

    /// Force remove the submodule. Will delete any local changes to the submodule
    #[cfg_attr(feature = "cli", clap(long, short))]
    pub force: bool,

    /// Pass the `--force` flag to `git submobule deinit`
    ///
    /// Cannot be used together with `--force`, since `--force` skips de-initializing.
    #[cfg_attr(feature = "cli", clap(long))]
    #[cfg_attr(feature = "cli", arg(conflicts_with("force")))]
    pub force_deinit: bool,

    /// Print options
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub options: PrintOptions,
}

impl RemoveCommand {
    /// Apply the print options
    pub fn set_print_options(&self) {
        self.options.apply();
    }

    /// Run the command in the given directory
    pub fn run(&self, dir: &str) -> Result<(), GitError> {
        let context = GitContext::try_from(dir)?;
        let _guard = context.lock()?;

        let name = &self.name;

        println_verbose!("Removing submodule: {name}");
        let mut status = Status::read_from(&context)?;
        let submodule = match status.modules.get_mut(name) {
            Some(submodule) => submodule,
            None => {
                println_error!("Submodule `{name}` not found!");
                // maybe user passed in path instead of name?
                println_verbose!("Trying to search for a path matching `{name}`");
                for submodule in status.flattened() {
                    if let Some(other_name) = submodule.name() {
                        if let Some(path) = submodule.path() {
                            if path == name {
                                println_hint!("  however, there is a submodule \"{other_name}\" with path \"{path}\"");
                                println_hint!("  if you meant to remove this submodule, use `magoo remove {other_name}`");
                                break;
                            }
                        }
                    }
                }

                return Err(GitError::NeedFix(false));
            }
        };

        if self.force {
            println_verbose!("Removing (force): {name}");
            submodule.force_delete(&context)?;
        } else {
            let path = match submodule.path() {
                Some(x) => x,
                None => {
                    println_error!("Submodule `{name}` does not have a path!");
                    println_hint!("  run `magoo status` to investigate.");
                    println_hint!("  if you are unsure of the problem, try hard removing the submodule with `magoo remove {name} --force`");
                    return Err(GitError::NeedFix(false));
                }
            };
            if let Err(e) = context.submodule_deinit(Some(path), self.force_deinit) {
                println_error!("Failed to deinitialize submodule `{name}`: {e}");
                println_hint!(
                    "  try running with `--force-deinit` to force deinitialize the module"
                );
                println_hint!(
                    "  alternatively, running with `--force` will remove the module anyway."
                );
                return Err(GitError::NeedFix(false));
            }

            submodule.force_remove_module_dir(&context)?;
            submodule.force_remove_config(&context)?;
            submodule.force_remove_from_dot_gitmodules(&context)?;
            submodule.force_remove_from_index(&context)?;
        }

        println_info!();
        println_info!("Submodules removed successfully.");
        println_hint!("  run `git status` to check the changes");
        Ok(())
    }
}

/// Printing options for all commands
#[derive(Debug, Default, Clone, PartialEq)]
#[cfg_attr(feature = "cli", derive(clap::Parser))]
pub struct PrintOptions {
    /// Enable verbose output
    ///
    /// Display more information about what is happening, for example which git commands are
    /// executed and their output
    #[cfg_attr(feature = "cli", clap(long))]
    pub verbose: bool,

    /// Disable output to stdout and stderr
    #[cfg_attr(feature = "cli", clap(long, short))]
    pub quiet: bool,

    /// Color options
    ///
    /// `Some(true)` and `Some(false)` to always/never use color in output.
    /// `None` to read the color setting from git config.
    #[cfg_attr(feature = "cli", clap(skip))]
    pub color: Option<bool>,
}

impl PrintOptions {
    /// Apply the options
    pub fn apply(&self) {
        print::set_options(self.verbose, self.quiet, self.color);
    }
}
