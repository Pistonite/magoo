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
//!     all: false,
//!     fix: false,
//!     options: PrintOptions {
//!         verbose: false,
//!         quiet: false,
//!         color: None,
//!     },
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
//! let magoo = Magoo::try_parse_from(["magoo", "--dir", "my/repo", "status", "--all", "--verbose"]).unwrap();
//!
//! assert_eq!(magoo, Magoo {
//!     subcmd: Command::Status(StatusCommand {
//!         git: false,
//!         all: true,
//!         fix: false,
//!         options: PrintOptions {
//!             verbose: true,
//!             quiet: false,
//!             color: None,
//!         },
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

mod git;
pub use git::SUPPORTED_GIT_VERSIONS;
use git::{GitContext, GitError};

mod print;
mod status;
mod submodule;
use status::Status;

use crate::print::println_verbose;

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
    pub fn set_print_options(&self) {
        match self {
            Command::Status(cmd) => cmd.set_print_options(),
            Command::Install(cmd) => cmd.set_print_options(),
            _ => todo!(),
            // Command::Update(cmd) => cmd.set_print_options(),
            // Command::Remove(cmd) => cmd.set_print_options(),
        }
    }
    pub fn run(&self, dir: &str) -> Result<(), GitError> {
        match self {
            Command::Status(cmd) => {
                cmd.run(dir)?;
            }
            Command::Install(cmd) => {
                cmd.run(dir)?;
            }
            _ => todo!(),
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
        self.options.apply();
    }
    pub fn run(&self, dir: &str) -> Result<Status, GitError> {
        let context = GitContext::try_from(dir)?;
        let _guard = context.lock()?;
        if self.git {
            context.print_version_info()?;
            return Ok(Status::default());
        }

        let mut status = Status::read_from(&context, self.all)?;
        let mut flat_status = status.flattened_mut();
        if flat_status.is_empty() {
            println!("No submodules found");
            return Ok(status);
        }
        if self.fix {
            for submodule in flat_status.iter_mut() {
                submodule.fix(&context)?;
            }
            return Ok(status);
        }

        let dir_switch = if dir == "." {
            "".to_string()
        } else {
            format!(" --dir {dir}")
        };

        let all_switch = if self.all { " --all" } else { "" };

        for submodule in &flat_status {
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

    #[cfg_attr(feature = "cli", clap(flatten))]
    pub options: PrintOptions,
}

impl InstallCommand {
    pub fn set_print_options(&self) {
        self.options.apply();
    }
    pub fn run(&self, dir: &str) -> Result<(), GitError> {
        let context = GitContext::try_from(dir)?;
        let _guard = context.lock()?;

        let mut status = Status::read_from(&context, true)?;
        for submodule in status.flattened_mut() {
            submodule.fix(&context)?;
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
                context.submodule_sync(None)?;
                context.submodule_update(None, self.force)?;
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
