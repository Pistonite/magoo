use crate::git::GitError;

#[derive(Debug, thiserror::Error)]
pub enum MagooError {
    #[error("{0}")]
    Git(#[from] GitError),
}
