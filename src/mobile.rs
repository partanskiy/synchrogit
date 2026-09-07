//! Synchronous boundary used by mobile UI threads via Dispatchers.IO. All
//! repository workers, timers and filesystem watching remain in the Rust core.
use crate::config::{load_from_path, parse_str};
use crate::git::embedded::{self, Credentials};
use crate::runtime::Supervisor;
use crate::{Result, SynchrogitError};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    DecodeConfig {
        config: String,
    },
    EncodeConfig {
        settings: Value,
    },
    Start {
        path: PathBuf,
    },
    Once {
        path: PathBuf,
    },
    Stop,
    Status,
    Sync,
    Credentials {
        path: PathBuf,
        url: String,
        username: String,
        password: String,
    },
    Clone {
        path: PathBuf,
        url: String,
        name: String,
        email: String,
    },
    Identity {
        path: PathBuf,
        name: String,
        email: String,
        remote: Option<String>,
        url: Option<String>,
    },
}

pub struct Engine {
    runtime: tokio::runtime::Runtime,
    supervisor: Option<Supervisor>,
}
impl Engine {
    pub fn new() -> Result<Self> {
        crate::clock::init_local_offset();
        Ok(Self {
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()?,
            supervisor: None,
        })
    }
    pub fn call(&mut self, request: Request) -> Result<Value> {
        match request {
            Request::DecodeConfig { config } => {
                // Import may contain paths from another OS. Decode first so
                // the user can edit them; saving and starting validate fully.
                let value: toml::Value =
                    toml::from_str(&config).map_err(|e| SynchrogitError::Config(e.to_string()))?;
                Ok(serde_json::to_value(value)?)
            }
            Request::EncodeConfig { settings } => {
                let value: toml::Value = serde_json::from_value(settings)?;
                let source = toml::to_string_pretty(&value)
                    .map_err(|e| SynchrogitError::Config(e.to_string()))?;
                parse_str(&source)?;
                Ok(json!({"config": source}))
            }
            Request::Start { path } => {
                let config = load_from_path(path)?; // Validate before stopping.
                self.stop();
                let _guard = self.runtime.enter();
                self.supervisor = Some(Supervisor::spawn_loaded(config)?);
                Ok(json!({"running": true}))
            }
            Request::Once { path } => {
                if self.supervisor.is_some() {
                    return Ok(json!({"continuous": true}));
                }
                let config = load_from_path(path)?.config;
                self.runtime.block_on(async {
                    for repo in config.resolved_repos() {
                        let git = crate::git::Git::embedded(&repo.path, repo.git_timeout);
                        let report = crate::git::sync_cycle(
                            &git,
                            &crate::git::CycleParams {
                                commit_template: &repo.commit_template,
                                auto_push: repo.auto_push,
                                auto_pull: repo.auto_pull,
                                branch: repo.branch.as_deref(),
                                remote: repo.remote.as_deref(),
                                ignore: &repo.ignore,
                            },
                        )
                        .await;
                        if let Some(error) = report.failure_message() {
                            return Err(SynchrogitError::Other(format!("{}: {error}", repo.name)));
                        }
                    }
                    Ok(json!({"synced": true}))
                })
            }
            Request::Stop => {
                self.stop();
                Ok(json!({"running": false}))
            }
            Request::Status => Ok(
                json!({"running": self.supervisor.is_some(), "repos": self.supervisor.as_ref().map(|s| s.control().status()).unwrap_or_default()}),
            ),
            Request::Sync => {
                let control = self
                    .supervisor
                    .as_ref()
                    .ok_or_else(|| SynchrogitError::Other("synchronization is stopped".into()))?
                    .control();
                let queued = self
                    .runtime
                    .block_on(control.sync(None))
                    .map_err(SynchrogitError::Other)?;
                Ok(json!({"queued": queued}))
            }
            Request::Credentials {
                path,
                url,
                username,
                password,
            } => {
                // The app accepts HTTPS only. Never allow a token to be used on
                // plaintext HTTP or an unrelated redirected repository URL.
                if !url.starts_with("https://") {
                    return Err(SynchrogitError::Config(
                        "Android authentication requires an HTTPS URL".into(),
                    ));
                }
                embedded::set_credentials(
                    path,
                    if password.is_empty() {
                        None
                    } else {
                        Some(Credentials {
                            url,
                            username,
                            password,
                        })
                    },
                );
                Ok(json!({}))
            }
            Request::Clone {
                path,
                url,
                name,
                email,
            } => {
                if self.supervisor.is_some() {
                    return Err(SynchrogitError::Other(
                        "stop synchronization before cloning".into(),
                    ));
                }
                if !url.starts_with("https://") {
                    return Err(SynchrogitError::Config(
                        "Android cloning requires an HTTPS URL".into(),
                    ));
                }
                validate_identity(&name, &email)?;
                embedded::clone_repository(&url, &path, &name, &email, Duration::from_secs(120))?;
                Ok(json!({}))
            }
            Request::Identity {
                path,
                name,
                email,
                remote,
                url,
            } => {
                if self.supervisor.is_some() {
                    return Err(SynchrogitError::Other(
                        "stop synchronization before changing repository identity".into(),
                    ));
                }
                validate_identity(&name, &email)?;
                let repo = git2::Repository::open(path)
                    .map_err(|e| SynchrogitError::Other(e.to_string()))?;
                let mut config = repo
                    .config()
                    .map_err(|e| SynchrogitError::Other(e.to_string()))?;
                config
                    .set_str("user.name", &name)
                    .and_then(|()| config.set_str("user.email", &email))
                    .map_err(|e| SynchrogitError::Other(e.to_string()))?;
                if let Some(url) = url.filter(|url| !url.is_empty()) {
                    if !url.starts_with("https://") {
                        return Err(SynchrogitError::Config(
                            "Android connections require HTTPS".into(),
                        ));
                    }
                    let remote = remote
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| "origin".into());
                    match repo.find_remote(&remote) {
                        Ok(_) => repo.remote_set_url(&remote, &url),
                        Err(error) if error.code() == git2::ErrorCode::NotFound => {
                            repo.remote(&remote, &url).map(|_| ())
                        }
                        Err(error) => Err(error),
                    }
                    .map_err(|e| SynchrogitError::Other(e.to_string()))?;
                }
                Ok(json!({}))
            }
        }
    }
    fn stop(&mut self) {
        if let Some(supervisor) = self.supervisor.take() {
            self.runtime.block_on(supervisor.shutdown());
        }
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}
fn validate_identity(name: &str, email: &str) -> Result<()> {
    if name.trim().is_empty() || email.trim().is_empty() {
        return Err(SynchrogitError::Config(
            "commit author name and email are required".into(),
        ));
    }
    git2::Signature::now(name, email).map_err(|e| SynchrogitError::Config(e.to_string()))?;
    Ok(())
}
