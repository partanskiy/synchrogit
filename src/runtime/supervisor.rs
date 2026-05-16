use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::config::{Config, LoadedConfig, ResolvedRepoConfig, load_from_path};
use crate::error::Result;
use crate::ipc::protocol::RepoStatus;
use crate::worker::{KickReason, WorkerHandle, spawn};

pub struct Supervisor {
    inner: Arc<RwLock<SupervisorInner>>,
    reload_lock: Arc<tokio::sync::Mutex<()>>,
}

struct SupervisorInner {
    config_path: Option<PathBuf>,
    workers: BTreeMap<String, WorkerSlot>,
}

struct WorkerSlot {
    config: ResolvedRepoConfig,
    handle: WorkerHandle,
}

#[derive(Clone)]
pub struct SupervisorControl {
    inner: Arc<RwLock<SupervisorInner>>,
    reload_lock: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Debug, Clone)]
pub struct ReloadReport {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub restarted: Vec<String>,
    pub unchanged: usize,
}

impl Supervisor {
    pub fn spawn(config: Config) -> Result<Self> {
        Self::from_config(None, config)
    }

    pub fn spawn_loaded(loaded: LoadedConfig) -> Result<Self> {
        Self::from_config(Some(loaded.path), loaded.config)
    }

    fn from_config(config_path: Option<PathBuf>, config: Config) -> Result<Self> {
        let repos = config.resolved_repos();
        let mut workers = BTreeMap::new();

        for repo in &repos {
            match spawn_slot(repo) {
                Ok(slot) => {
                    workers.insert(repo.name.clone(), slot);
                }
                Err(e) => {
                    cancel_slots(workers.into_values().collect());
                    return Err(e);
                }
            }
        }

        Ok(Self {
            inner: Arc::new(RwLock::new(SupervisorInner {
                config_path,
                workers,
            })),
            reload_lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    pub fn worker_count(&self) -> usize {
        self.inner
            .read()
            .expect("supervisor lock poisoned")
            .workers
            .len()
    }

    pub fn control(&self) -> SupervisorControl {
        SupervisorControl {
            inner: self.inner.clone(),
            reload_lock: self.reload_lock.clone(),
        }
    }

    pub async fn shutdown(self) {
        let slots = {
            let mut inner = self.inner.write().expect("supervisor lock poisoned");
            std::mem::take(&mut inner.workers)
        };
        stop_slots(slots.into_values().collect()).await;
    }
}

impl SupervisorControl {
    pub fn status(&self) -> Vec<RepoStatus> {
        self.inner
            .read()
            .expect("supervisor lock poisoned")
            .workers
            .values()
            .map(|slot| RepoStatus {
                name: slot.config.name.clone(),
                path: slot.config.path.display().to_string(),
            })
            .collect()
    }

    pub async fn sync(&self, repo_name: Option<&str>) -> std::result::Result<Vec<String>, String> {
        let selected: Vec<_> = {
            let inner = self.inner.read().expect("supervisor lock poisoned");
            inner
                .workers
                .values()
                .filter(|slot| repo_name.is_none_or(|name| name == slot.config.name))
                .map(|slot| (slot.config.name.clone(), slot.handle.kick_tx.clone()))
                .collect()
        };

        if selected.is_empty() {
            return Err(match repo_name {
                Some(name) => format!("unknown repo `{name}`"),
                None => "no repos configured".to_string(),
            });
        }

        let mut queued = Vec::with_capacity(selected.len());
        for (name, kick_tx) in selected {
            kick_tx
                .send(KickReason::Manual)
                .await
                .map_err(|_| format!("repo `{name}` worker is not running"))?;
            queued.push(name);
        }

        Ok(queued)
    }

    pub async fn reload(&self) -> std::result::Result<ReloadReport, String> {
        let _guard = self.reload_lock.lock().await;
        let path = {
            let inner = self.inner.read().expect("supervisor lock poisoned");
            inner.config_path.clone()
        }
        .ok_or_else(|| "supervisor was not started from a config file".to_string())?;

        let loaded = load_from_path(&path).map_err(|e| e.to_string())?;
        let new_repos = loaded.config.resolved_repos();
        let new_names: BTreeSet<_> = new_repos.iter().map(|repo| repo.name.clone()).collect();

        let mut to_stop = Vec::new();
        let mut to_start = Vec::new();
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut restarted = Vec::new();
        let mut unchanged = 0;

        {
            let mut inner = self.inner.write().expect("supervisor lock poisoned");
            let old_names: Vec<_> = inner.workers.keys().cloned().collect();

            for name in old_names {
                if !new_names.contains(&name)
                    && let Some(slot) = inner.workers.remove(&name)
                {
                    removed.push(name);
                    to_stop.push(slot);
                }
            }

            for repo in new_repos {
                match inner.workers.get(&repo.name) {
                    Some(slot) if slot.config == repo => unchanged += 1,
                    Some(_) => {
                        if let Some(slot) = inner.workers.remove(&repo.name) {
                            restarted.push(repo.name.clone());
                            to_stop.push(slot);
                            to_start.push(repo);
                        }
                    }
                    None => {
                        added.push(repo.name.clone());
                        to_start.push(repo);
                    }
                }
            }
        }

        stop_slots(to_stop).await;

        let mut started = BTreeMap::new();
        for repo in &to_start {
            match spawn_slot(repo) {
                Ok(slot) => {
                    started.insert(repo.name.clone(), slot);
                }
                Err(e) => {
                    cancel_slots(started.into_values().collect());
                    return Err(e.to_string());
                }
            }
        }

        {
            let mut inner = self.inner.write().expect("supervisor lock poisoned");
            inner.workers.extend(started);
        }

        Ok(ReloadReport {
            added,
            removed,
            restarted,
            unchanged,
        })
    }
}

impl ReloadReport {
    pub fn message(&self) -> String {
        format!(
            "reloaded config: {} added, {} removed, {} restarted, {} unchanged",
            self.added.len(),
            self.removed.len(),
            self.restarted.len(),
            self.unchanged
        )
    }
}

fn spawn_slot(config: &ResolvedRepoConfig) -> Result<WorkerSlot> {
    let handle = spawn(config.into())?;
    Ok(WorkerSlot {
        config: config.clone(),
        handle,
    })
}

fn cancel_slots(slots: Vec<WorkerSlot>) {
    for slot in slots {
        slot.handle.cancel.cancel();
    }
}

async fn stop_slots(slots: Vec<WorkerSlot>) {
    for slot in &slots {
        slot.handle.cancel.cancel();
    }
    for slot in slots {
        let _ = slot.handle.join.await;
    }
}
