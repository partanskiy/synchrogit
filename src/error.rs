use std::path::PathBuf;

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
    GitSpawn(std::io::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("filesystem watcher error: {0}")]
    Watch(#[from] notify::Error),

    #[error("config file not found; checked: {checked}")]
    ConfigNotFound { checked: String },

    #[error("failed to read config {path}: {source}")]
    ConfigRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid config: {0}")]
    Config(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, SynchrogitError>;
