use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;
use tracing::trace;

use crate::error::{Result, SynchrogitError};

pub const DEFAULT_GIT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct Git {
    pub repo: PathBuf,
    timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct GitOutput {
    pub stdout: Vec<u8>,
    pub stderr: String,
}

impl GitOutput {
    pub fn stdout_trim(&self) -> String {
        String::from_utf8_lossy(&self.stdout).trim().to_string()
    }
}

impl Git {
    pub fn new(repo: impl Into<PathBuf>) -> Self {
        Self::with_timeout(repo, DEFAULT_GIT_TIMEOUT)
    }

    pub fn with_timeout(repo: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            repo: repo.into(),
            timeout,
        }
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Run `git <args>` against the configured repo path and return its output.
    /// Non-zero exit becomes [`SynchrogitError::GitFailed`].
    pub async fn run<I, S>(&self, args: I) -> Result<GitOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let collected: Vec<_> = args.into_iter().map(|s| s.as_ref().to_owned()).collect();
        let pretty: Vec<String> = collected
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        trace!(repo = %self.repo.display(), args = ?pretty, "git");

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.repo)
            .arg("-c")
            .arg("color.ui=false")
            .arg("-c")
            .arg("advice.detachedHead=false")
            .args(&collected)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let output = timeout(self.timeout, cmd.output())
            .await
            .map_err(|_| SynchrogitError::GitTimeout {
                args: pretty.clone(),
                timeout: self.timeout,
            })?
            .map_err(SynchrogitError::GitSpawn)?;
        if !output.status.success() {
            return Err(SynchrogitError::GitFailed {
                args: pretty,
                code: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(GitOutput {
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    /// Run `git <args>` and report whether it succeeded. Spawn failures still
    /// propagate as `Err`; only non-zero git exits collapse to `Ok(false)`.
    pub async fn try_run<I, S>(&self, args: I) -> Result<bool>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        match self.run(args).await {
            Ok(_) => Ok(true),
            Err(SynchrogitError::GitFailed { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }
}
