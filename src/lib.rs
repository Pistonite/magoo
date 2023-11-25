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
//! fn main() {
//!     let command = magoo::StatusCommand {
//!         git: true,
//!         all: false,
//!         fix: false,
//!         options: PrintOptions {
//!             verbose: false,
//!             quiet: false,
//!             color: None,
//!         },
//!     }
//!
//!     // don't need this if you don't need output to stdout
//!     command.set_print_options();
//!     // runs `magoo status --git` in the current directory
//!     command.run(".").unwrap();
//! }
//! ```
//! #### Use `clap` to parse arguments
//! ```rust
//! use magoo::Magoo;
//!
//! // for assertion below only
//! use magoo::{Command, StatusCommand};
//!
//! fn main() {
//!     let magoo = Magoo::try_parse_from("magoo --dir my/repo status --all --verbose").unwrap();
//!
//!     assert_eq!(magoo, Magoo {
//!         subcmd: Command::Status(StatusCommand {
//!             git: false,
//!             all: true,
//!             fix: false,
//!             options: PrintOptions {
//!                 verbose: true,
//!                 quiet: false,
//!                 color: None,
//!             },
//!         },
//!         dir: "my/repo".to_string(),
//!     });
//!
//!     magoo.set_print_options();
//!     magoo.run().unwrap();
//! }
//! ```
//! You can also look at [main.rs](https://github.com/Pistonite/magoo/blob/master/src/main.rs) for
//! reference.
//!

use std::collections::{BTreeMap, BTreeSet};

// mod error;
mod git;
pub use git::SUPPORTED_GIT_VERSIONS;
use git::{GitContext, GitError};

mod print;
mod submodule;
use submodule::Submodule;

/// The main entry point for the library
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "cli", derive(clap::Parser))]
#[cfg_attr(
    feature = "cli",
    command(author, about, version, arg_required_else_help(true))
)]
pub struct Magoo {
    #[clap(subcommand)]
    pub subcmd: Command,
    /// Set the working directory of commands. Useful if not running inside a git repository.
    #[cfg_attr(feature = "cli", clap(long, default_value(".")))]
    pub dir: String,
}

impl Magoo {
    /// Run the command
    pub fn run(&self) -> Result<(), GitError> {
        self.subcmd.run(&self.dir)
    }

    pub fn set_print_options(&self) {
        self.subcmd.set_print_options();
    }
}

/// The command to run
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "cli", derive(clap::Parser))]
pub enum Command {
    /// Print the status of the submodules in the current git repository.
    Status(StatusCommand),
    /// Add or install dependencies
    ///
    /// Installs dependencies if no arguments are provided.
    /// Otherwise, adds the provided dependency as a git submodule.
    Install(AddCommand),
    /// Updates all dependencies or the specified dependency.
    ///
    /// Dependencies will be updated to the branch (specified when adding the dependency) from the
    /// remote.
    Update(UpdateCommand),
    /// Remove a dependency
    Remove(RemoveCommand),
}

impl Command {
    pub fn set_print_options(&self) {
        match self {
            Command::Status(cmd) => cmd.set_print_options(),
            _ => todo!(),
            // Command::Install(cmd) => cmd.set_print_options(),
            // Command::Update(cmd) => cmd.set_print_options(),
            // Command::Remove(cmd) => cmd.set_print_options(),
        }
    }
    pub fn run(&self, dir: &str) -> Result<(), GitError> {
        match self {
            Command::Status(cmd) => {
                cmd.run(dir)?;
            }
            _ => todo!(),
            // Command::Install(cmd) => cmd.run(dir),
            // Command::Update(cmd) => cmd.run(dir),
            // Command::Remove(cmd) => cmd.run(dir),
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

    /// Show every trace of submodules found.
    ///
    /// This includes modules found in `.git/modules`, but not in anywhere else.
    #[cfg_attr(feature = "cli", clap(long, short))]
    pub all: bool,

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

    #[cfg_attr(feature = "cli", clap(flatten))]
    pub options: PrintOptions,
}

impl StatusCommand {
    pub fn set_print_options(&self) {
        print::set_options(self.options.verbose, self.options.quiet, None);
    }
    pub fn run(&self, dir: &str) -> Result<Vec<Submodule>, GitError> {
        let context = GitContext::try_from(dir)?;
        let _guard = context.lock()?;
        if self.git {
            context.print_version_info()?;
            return Ok(vec![]);
        }

        let mut status_map = BTreeMap::new();
        let mut index = Vec::new();

        context.get_submodule_status(&mut status_map, &mut index, self.all)?;

        let mut status = status_map.into_iter().map(|(_, v)| v).collect::<Vec<_>>();
        if self.all {
            index.into_iter().for_each(|v| {
                status.push(Submodule {
                    in_gitmodules: None,
                    in_config: None,
                    in_index: Some(v),
                    in_modules: None,
                })
            });
        }
        if self.fix {
            for submodule in &mut status {
                submodule.fix(&context)?;
            }
            return Ok(status);
        }

        if status.is_empty() {
            println!("No submodules found");
            return Ok(vec![]);
        }

        let dir_switch = if dir == "." {
            "".to_string()
        } else {
            format!(" --dir {dir}")
        };

        let all_switch = if self.all { " --all" } else { "" };

        for submodule in &status {
            submodule.print(&context, &dir_switch, all_switch)?;
        }
        Ok(status)
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

    /// Disable output to stdout
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
    pub fn apply(&self) {
        print::set_options(self.verbose, self.quiet, self.color);
    }
}

/// The `add` command
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "cli", derive(clap::Parser))]
pub struct AddCommand {
    /// URL of the git repository to add
    ///
    /// See the `add` command of https://git-scm.com/docs/git-submodule for what formats are
    /// supported.
    pub url: String,

    /// Local path to clone the git submodule to
    pub path: String,

    /// Branch to checkout and track
    ///
    /// This is the branch reference that will be used when updating the submodule.
    /// If not specified, the behavior is the same as `git submodule add` without `--branch`
    /// (`HEAD` is used)
    #[cfg_attr(feature = "cli", clap(long, short))]
    pub branch: Option<String>,

    /// Name of the submodule
    ///
    /// If not specified, the name of the submodule is the same as the path.
    #[cfg_attr(feature = "cli", clap(long))]
    pub name: Option<String>,

    /// Depth to clone the submodule
    #[cfg_attr(feature = "cli", clap(long))]
    pub depth: Option<usize>,

    /// Whether to force the submodule to be added
    ///
    /// This is the same as the `--force` flag of `git submodule add`. The submodule will be
    /// added even if one with the same name or path already existed.
    #[cfg_attr(feature = "cli", clap(long, short))]
    pub force: bool,
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

    /// Change the url of the submodule
    #[cfg_attr(feature = "cli", clap(long, short))]
    #[cfg_attr(feature = "cli", arg(requires("name")))]
    pub url: Option<String>,
}

/// The `remove` command
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "cli", derive(clap::Parser))]
pub struct RemoveCommand {
    /// The name of the submodule to remove
    pub name: String,

    /// Whether to force the submodule to be removed
    ///
    /// The submodule will be removed even if it has local changes. (`git submodule deinit -f`)
    #[cfg_attr(feature = "cli", clap(long, short))]
    pub force: bool,
}
