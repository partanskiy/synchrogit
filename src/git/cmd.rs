use super::operation::Operation;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;
use tracing::trace;

use crate::error::{Result, SynchrogitError};

pub const DEFAULT_GIT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
enum Backend {
    External(PathBuf),
    #[cfg(feature = "embedded-git")]
    Embedded,
}

#[derive(Debug, Clone)]
pub struct Git {
    pub repo: PathBuf,
    timeout: Duration,
    backend: Arc<OnceLock<std::result::Result<Backend, String>>>,
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
    fn backend(&self) -> Result<&Backend> {
        self.backend
            .get_or_init(select_backend)
            .as_ref()
            .map_err(|message| SynchrogitError::Other(message.clone()))
    }

    pub fn backend_name(&self) -> Result<String> {
        Ok(match self.backend()? {
            Backend::External(path) => format!("external ({})", path.display()),
            #[cfg(feature = "embedded-git")]
            Backend::Embedded => "embedded (libgit2)".into(),
        })
    }

    #[cfg(feature = "embedded-git")]
    pub fn embedded(repo: impl Into<PathBuf>, timeout: Duration) -> Self {
        let git = Self::with_timeout(repo, timeout);
        let _ = git.backend.set(Ok(Backend::Embedded));
        git
    }

    pub(crate) async fn execute(&self, operation: Operation) -> Result<GitOutput> {
        match self.backend()? {
            Backend::External(_) => {
                if let Operation::KeepRemote(path) = &operation {
                    if self
                        .try_run(["rev-parse", "--verify", &format!(":3:{path}")])
                        .await?
                    {
                        self.run(operation.args()).await?;
                        self.run(["add", "--", path]).await
                    } else {
                        self.run(["rm", "--", path]).await
                    }
                } else {
                    self.run(operation.args()).await
                }
            }
            #[cfg(feature = "embedded-git")]
            Backend::Embedded => {
                let path = self.repo.clone();
                let duration = self.timeout;
                // Await completion, including cancellation. Dropping a timed-out
                // spawn_blocking handle would let Git keep mutating the repository
                // concurrently with the next cycle. Network callbacks enforce the
                // deadline cooperatively instead.
                tokio::task::spawn_blocking(move || {
                    super::embedded::execute(&path, operation, duration)
                })
                .await
                .map_err(|e| SynchrogitError::Other(format!("Git worker failed: {e}")))?
            }
        }
    }

    pub fn new(repo: impl Into<PathBuf>) -> Self {
        Self::with_timeout(repo, DEFAULT_GIT_TIMEOUT)
    }

    pub fn with_timeout(repo: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            repo: repo.into(),
            timeout,
            backend: Arc::new(OnceLock::new()),
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

        let executable = match self.backend()? {
            Backend::External(path) => path,
            #[cfg(feature = "embedded-git")]
            Backend::Embedded => {
                return Err(SynchrogitError::Other(
                    "raw Git commands require an external Git executable".into(),
                ));
            }
        };
        let mut cmd = Command::new(executable);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
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

fn select_backend() -> std::result::Result<Backend, String> {
    let preference = std::env::var("SYNCHROGIT_GIT_BACKEND").unwrap_or_else(|_| "auto".into());
    if !matches!(preference.as_str(), "auto" | "external" | "embedded") {
        return Err("SYNCHROGIT_GIT_BACKEND must be auto, external, or embedded".into());
    }
    if preference != "embedded" {
        if let Some(path) = std::env::var_os("SYNCHROGIT_GIT") {
            // An explicit override is authoritative, including a broken path.
            return Ok(Backend::External(path.into()));
        }
        if !cfg!(target_os = "android")
            && let Some(path) = find_external_git()
        {
            return Ok(Backend::External(path));
        }
        if preference == "external" {
            return Err("Git executable not found; install Git or set SYNCHROGIT_GIT".into());
        }
    }
    #[cfg(feature = "embedded-git")]
    {
        Ok(Backend::Embedded)
    }
    #[cfg(not(feature = "embedded-git"))]
    {
        Err("Git executable not found and this build has no embedded-git feature".into())
    }
}

fn find_external_git() -> Option<PathBuf> {
    let name = if cfg!(windows) { "git.exe" } else { "git" };
    let mut candidates: Vec<_> = std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .filter(|p| p.is_absolute())
                .map(|p| p.join(name))
                .collect()
        })
        .unwrap_or_default();
    #[cfg(windows)]
    {
        for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(path) = std::env::var_os(variable) {
                candidates.push(PathBuf::from(path).join("Git/cmd/git.exe"));
            }
        }
        if let Some(path) = std::env::var_os("LOCALAPPDATA") {
            candidates.push(PathBuf::from(path).join("Programs/Git/cmd/git.exe"));
        }
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join(if cfg!(windows) {
            "git/cmd/git.exe"
        } else {
            "git/bin/git"
        }));
    }
    candidates.into_iter().find(|path| {
        let Ok(metadata) = path.metadata() else {
            return false;
        };
        if !metadata.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            true
        }
    })
}
