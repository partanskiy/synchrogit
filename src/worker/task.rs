use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span, warn};

use crate::error::Result;
use crate::git::{CycleParams, Git, sync_cycle};
use crate::worker::debouncer;
use crate::worker::fs_watch;
use crate::worker::handle::{KickReason, WorkerHandle};

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub name: String,
    pub path: PathBuf,
    pub interval: Duration,
    pub debounce: Duration,
    pub commit_template: String,
    pub auto_push: bool,
    pub auto_pull: bool,
}

pub fn spawn(cfg: WorkerConfig) -> Result<WorkerHandle> {
    let cancel = CancellationToken::new();
    let (kick_tx, kick_rx) = mpsc::channel::<KickReason>(8);
    let cancel_inner = cancel.clone();
    let name = cfg.name.clone();

    let join = tokio::spawn(async move {
        let span = info_span!("repo", name = %name);
        async move {
            if let Err(e) = run(cfg, cancel_inner, kick_rx).await {
                warn!(error = %e, "worker exited with error");
            }
        }
        .instrument(span)
        .await
    });

    Ok(WorkerHandle {
        cancel,
        kick_tx,
        join,
    })
}

async fn run(
    cfg: WorkerConfig,
    cancel: CancellationToken,
    mut kick_rx: mpsc::Receiver<KickReason>,
) -> Result<()> {
    let git = Git::new(&cfg.path);
    let mut fs = fs_watch::spawn(&cfg.path)?;
    let mut ticker = interval(cfg.interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let lock = Arc::new(Mutex::new(()));

    info!(
        path = %cfg.path.display(),
        interval = ?cfg.interval,
        debounce = ?cfg.debounce,
        "worker started"
    );

    do_cycle(&git, &cfg, &lock, "startup").await;

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            Some(_) = fs.rx.recv() => {
                debouncer::drain(&mut fs.rx, cfg.debounce).await;
                do_cycle(&git, &cfg, &lock, "fs-change").await;
            }
            _ = ticker.tick() => {
                do_cycle(&git, &cfg, &lock, "timer").await;
            }
            Some(reason) = kick_rx.recv() => {
                let label = match reason {
                    KickReason::Manual => "kick-manual",
                    KickReason::External => "kick-external",
                };
                do_cycle(&git, &cfg, &lock, label).await;
            }
        }
    }
    info!("worker stopped");
    Ok(())
}

async fn do_cycle(git: &Git, cfg: &WorkerConfig, lock: &Arc<Mutex<()>>, reason: &str) {
    let _guard = lock.lock().await;
    let params = CycleParams {
        commit_template: &cfg.commit_template,
        auto_push: cfg.auto_push,
        auto_pull: cfg.auto_pull,
    };
    let report = sync_cycle(git, &params).await;
    let outcome = report.summary();
    if let Some(err) = &report.error {
        warn!(reason, ?outcome, error = %err, "cycle ended with error");
    } else {
        info!(reason, ?outcome, "cycle ok");
    }
}
