use std::sync::Arc;

use crate::config::Config;
use crate::error::Result;
use crate::ipc::protocol::RepoStatus;
use crate::worker::{KickReason, WorkerHandle, spawn};

pub struct Supervisor {
    workers: Vec<WorkerHandle>,
}

#[derive(Debug, Clone)]
pub struct SupervisorControl {
    repos: Arc<Vec<RepoControl>>,
}

#[derive(Debug, Clone)]
struct RepoControl {
    name: String,
    path: String,
    kick_tx: tokio::sync::mpsc::Sender<KickReason>,
}

impl Supervisor {
    pub fn spawn(config: Config) -> Result<Self> {
        let repos = config.resolved_repos();
        let mut workers = Vec::with_capacity(repos.len());

        for repo in &repos {
            match spawn(repo.into()) {
                Ok(handle) => workers.push(handle),
                Err(e) => {
                    for handle in &workers {
                        handle.cancel.cancel();
                    }
                    return Err(e);
                }
            }
        }

        Ok(Self { workers })
    }

    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    pub fn control(&self) -> SupervisorControl {
        SupervisorControl {
            repos: Arc::new(
                self.workers
                    .iter()
                    .map(|worker| RepoControl {
                        name: worker.name.clone(),
                        path: worker.path.display().to_string(),
                        kick_tx: worker.kick_tx.clone(),
                    })
                    .collect(),
            ),
        }
    }

    pub async fn shutdown(self) {
        for handle in &self.workers {
            handle.cancel.cancel();
        }
        for handle in self.workers {
            let _ = handle.join.await;
        }
    }
}

impl SupervisorControl {
    pub fn status(&self) -> Vec<RepoStatus> {
        self.repos
            .iter()
            .map(|repo| RepoStatus {
                name: repo.name.clone(),
                path: repo.path.clone(),
            })
            .collect()
    }

    pub async fn sync(&self, repo_name: Option<&str>) -> std::result::Result<Vec<String>, String> {
        let selected: Vec<_> = self
            .repos
            .iter()
            .filter(|repo| repo_name.is_none_or(|name| name == repo.name))
            .collect();

        if selected.is_empty() {
            return Err(match repo_name {
                Some(name) => format!("unknown repo `{name}`"),
                None => "no repos configured".to_string(),
            });
        }

        let mut queued = Vec::with_capacity(selected.len());
        for repo in selected {
            repo.kick_tx
                .send(KickReason::Manual)
                .await
                .map_err(|_| format!("repo `{}` worker is not running", repo.name))?;
            queued.push(repo.name.clone());
        }

        Ok(queued)
    }
}
