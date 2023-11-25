//! Integration with git commands
use std::borrow::Cow;
use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

mod context;
pub use context::*;
/// The semver notation of the officially supported git versions
///
/// The version is not checked at run time, since unsupported versions might work fine.
pub const SUPPORTED_GIT_VERSIONS: &str = ">=2.35.0";

#[derive(Debug, thiserror::Error)]
pub enum GitError {
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
