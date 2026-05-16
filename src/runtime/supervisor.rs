use crate::config::Config;
use crate::error::Result;
use crate::worker::{WorkerHandle, spawn};

pub struct Supervisor {
    workers: Vec<WorkerHandle>,
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

    pub async fn shutdown(self) {
        for handle in &self.workers {
            handle.cancel.cancel();
        }
        for handle in self.workers {
            let _ = handle.join.await;
        }
    }
}
