use thiserror::Error;

#[derive(Debug, Error)]
pub enum SynchrogitError {
    #[error("git {args:?} failed with status {code}: {stderr}")]
    GitFailed {
        args: Vec<String>,
        code: i32,
        stderr: String,
    },

    #[error("failed to spawn git: {0}")]
    GitSpawn(#[from] std::io::Error),

    #[error("filesystem watcher error: {0}")]
    Watch(#[from] notify::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, SynchrogitError>;
