//! Integration with git commands
use std::borrow::Cow;
use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};


// use crate::{Options, GitOutput, SubmoduleStatus};
// use crate::error::MagooError;
// use crate::log::{verbose, log, LOG_LEVEL, self};

mod context;
pub use context::GitContext;
/// The semver notation of the officially supported git versions
///
/// The version is not checked at run time, since unsupported versions might work fine.
pub const SUPPORTED_GIT_VERSIONS: &str = ">=2.35.0";

pub fn test() {
    let context = context::GitContext::try_from("./temp/Gitplay").unwrap();
    let submodules = context.read_dot_gitmodules().unwrap_or_default();
    let submodules2 = context.read_dot_git_config().unwrap_or_default();
    let submodules3 = context.read_submodules_in_index().unwrap_or_default();

    println!("{:#?}", submodules);
    println!("{:#?}", submodules2);
    println!("{:#?}", submodules3);

    let mut submodules4 = BTreeMap::new();
    for name in submodules.keys() {
        let module = context.read_git_module(name).ok();
        submodules4.insert(name.clone(), module);
    }
    println!("{:#?}", submodules4);

}


#[derive(Debug)]
pub struct SubmoduleStatus {
    /// Data of this submodule in .gitmodules
    pub in_gitmodules: Option<GitmodulesSubmodule>,
    /// Data of this submodule in .git/config
    pub in_config: Option<GitConfigSubmodule>,
    /// Data of this submodule in .git/modules/<name>
    pub in_modules: Option<GitModule>,
    /// Data of this submodule in the index
    pub in_index: Option<IndexObject>,
}

/// Data of a submodule stored in .gitmodules
#[derive(Debug)]
pub struct GitmodulesSubmodule {
    /// Name of the submodule, stored in the section name
    pub name: String,
    /// Path of the submodule, stored as `submodule.<name>.path`
    pub path: Option<String>,
    /// URL of the submodule, stored as `submodule.<name>.url`
    pub url: Option<String>,
    /// Branch of the submodule to update, stored as `submodule.<name>.branch`
    pub branch: Option<String>,
}

impl GitmodulesSubmodule {
    pub fn with_name(name: &str) -> Self {
        Self {
            name: name.to_string(),
            path: None,
            url: None,
            branch: None,
        }
    }
}

#[derive(Debug)]
pub struct IndexObject {
    pub path: String,
    pub sha: String,
}

/// Data of a submodule stored in .git/config
#[derive(Debug)]
pub struct GitConfigSubmodule {
    /// Name of the submodule, stored in the section name
    pub name: String,
    /// URL of the submodule, stored as `submodule.<name>.url`
    pub url: Option<String>,
}

impl GitConfigSubmodule {
    pub fn with_name(name: &str) -> Self {
        Self {
            name: name.to_string(),
            url: None,
        }
    }
}

/// Data of submodule stored in .git/modules
#[derive(Debug)]
pub struct GitModule {
    /// Name of the submodule, which is the part in the path after `.git/modules/`
    pub name: String,
    /// Path of the submodule worktree, stored in `core.worktree` in `.git/modules/<name>/config`
    pub worktree: Option<String>,
    /// Currently checked out commit. (`git rev-parse HEAD` in the submodule)
    pub head_sha: Option<String>,
    /// Git dir of the submodule (`git rev-parse --git-dir` in the submodule)
    pub git_dir: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("git is not installed or not in PATH")]
    NotInstalled,

    #[error("cannot set the working directory: {0}")]
    WorkingDir(String),

    #[error("cannot find the .git directory: {0}")]
    GitDir(String),

    #[error("cannot find the top level directory: {0}")]
    TopLevelDir(String),

    #[error("failed to spawn git command: {0}")]
    SpawnFailed(String),

    #[error("failed to read git command output: {0}")]
    OutputFailed(String),

    #[error("git command did not finish: {0}")]
    NoReturn(String),

    #[error("git command failed: {0}")]
    ExitFail(ExitStatus),

    #[error("cannot process config: {0}")]
    InvalidConfig(String),

    #[error("cannot process index: {0}")]
    InvalidIndex(String),

    #[error("cannot find module `{0}`")]
    ModuleNotFound(String),



    #[error("git: {0}")]
    Fail(Cow<'static, str>),
}

// #[derive(Debug)]
// pub struct Git<'a> {
//     options: &'a Options,
//     /// The path to the .git directory
//     git_dir_cell: OnceCell<PathBuf>,
// }
// impl<'a> Git<'a> {
//     pub fn new(options: &Options) -> Self {
//         Self {
//             options,
//             git_dir_cell: OnceCell::new(),
//         }
//     }
//
//     pub fn check_version(&self) -> Result<String, MagooError> {
//         let req = SUPPORTED_GIT_VERSIONS;
//         let min_ver = VersionReq::parse(req).unwrap();
//         let output = self.run_git_command(&["--version"], true)?;
//         for line in output {
//             if let Some(version) = line.strip_prefix("git version") {
//                 let version_str = version.trim();
//                 let version = Version::parse(version_str)
//                     .map_err(|_| GitError::InvalidVersion(version_str.to_string()))?;
//                 if !min_ver.matches(&version) {
//                     return Err(MagooError::from(GitError::UnsupportedVersion(
//                         version_str.to_string(),
//                         req,
//                     )));
//                 }
//                 println!("git version is supported ({req})");
//                 return Ok(version_str.to_string());
//             }
//         }
//
//         return Err(MagooError::from(GitError::InvalidVersion("git did not return the version".to_string())))
//     }
//
//     pub fn submodule_status(&self) -> Result<BTreeMap<String, SubmoduleStatus>, MagooError> {
//         let git_dir = self.git_dir()?;
//         let gitmodules = git_dir.join("../.gitmodules");
//         if !gitmodules.exists() {
//             log!("no .gitmodules file found");
//             return Ok(Vec::new());
//         }
//         let gitmodules_arg = gitmodules.display().to_string();
//         let name_paths = self.run_git_command(&["config", "-f", &gitmodules_arg, "--get-regexp", "path"], false)?;
//         let names = self.run_git_command(&["config", "-f", &gitmodules_arg, "--name-only", "--get-regexp", "path"], false)?;
//         let paths = Vec::new();
//
//         for (name, name_path) in names.iter().zip(name_paths.iter()) {
//         }
//
//         let output = self.run_git_command(&["submodule", "status"], false)?;
//         for line in output {
//             println!("{}", line);
//         }
//         Ok(())
//     }
//
//     pub fn git_dir(&self) -> Result<&PathBuf, GitError> {
//         if let Some(git_dir) = self.git_dir_cell.get() {
//             return Ok(git_dir);
//         }
//
//         let output = self.run_git_command(&["rev-parse", "--git-dir"], false)?;
//         let path_raw = output
//             .first()
//             .ok_or_else(|| GitError::GitDir(Cow::from("git did not return the .git directory")))?;
//         let path = Path::new(path_raw)
//             .canonicalize()
//             .map_err(|_| GitError::GitDir(Cow::from("canonicalize failed")))?;
//
//         self.git_dir_cell.set(path).unwrap();
//         Ok(self.git_dir_cell.get().unwrap())
//     }
//
//     pub fn run_git_command(&self, args: &[&str], print: bool) -> Result<Vec<String>, GitError> {
//         verbose!("magoo: run: git {}", args.join(" "));
//         if which::which("git").is_err() {
//             return Err(GitError::NotInstalled);
//         }
//
//         let mut child = Command::new("git")
//             .args(args)
//             .stdout(Stdio::piped())
//             .stderr(Stdio::inherit())
//             .spawn()?;
//
//         let mut output = Vec::new();
//         if let Some(stdout) = child.stdout.take() {
//             let reader = BufReader::new(stdout);
//             for line in reader.lines() {
//                 let line = line?;
//                 if self.options.verbose {
//                     log!("git: {line}");
//                 } else {
//                     if print && !log::is_quiet() {
//                         print!("git: {line}\r");
//                         let _ = std::io::stdout().flush();
//                     }
//                 }
//                 output.push(line);
//             }
//         }
//         if print && !self.options.verbose {
//             log!();
//         }
//
//         let status = child.wait()?;
//         verbose!(
//             "magoo: git command finished: {}",
//             status
//         );
//         if status.success() {
//             Ok(output)
//         } else {
//             Err(GitError::ExitFail(status))
//         }
//     }
// }
//
// fn parse_submodule_status(line: &str) -> Option<SubmoduleStatus> {
//     if line.len() < 41 {
//         return None;
//     }
//     let status = line.chars().next()?;
//     let commit = &line[1..41].to_string();
//     let rest = line[41..].trim();
//     if status == '-' {
//         Some(SubmoduleStatus {
//             name: rest.to_string(),
//             path: rest.to_string(),
//             commit: commit.to_string(),
//             status,
//             reference: None,
//         })
//     }
//     // branch names cannot contain spaces, so we can split on whitespace
//     let mut parts = rest.rsplitn(2, ' ');
//     let mut parts = line.split_whitespace();
//     let commit = parts.next()?;
//     let status = parts.next()?;
//     let path = parts.next()?;
//     let name = parts.next()?;
//     let reference = parts.next();
//
//     Some(SubmoduleStatus {
//         name: name.to_string(),
//         path: path.to_string(),
//         commit: commit.to_string(),
//         status: status.chars().next()?,
//         reference: reference.map(|s| s.to_string()),
//     })
// }
