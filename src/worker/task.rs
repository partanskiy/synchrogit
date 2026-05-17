use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::time::{Instant, sleep_until};
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span, warn};

use crate::clock::now_local;
use crate::error::Result;
use crate::git::{CycleParams, Git, sync_cycle};
use crate::state::{CycleReport, RepoState};
use crate::worker::debouncer;
use crate::worker::fs_watch;
use crate::worker::handle::{KickReason, WorkerHandle};

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub name: String,
    pub path: PathBuf,
    pub interval: Duration,
    pub debounce: Duration,
    pub backoff_min: Duration,
    pub backoff_max: Duration,
    pub git_timeout: Duration,
    pub branch: Option<String>,
    pub remote: Option<String>,
    pub commit_template: String,
    pub auto_push: bool,
    pub auto_pull: bool,
    pub ignore: Vec<String>,
}

pub fn spawn(cfg: WorkerConfig) -> Result<WorkerHandle> {
    let cancel = CancellationToken::new();
    let (kick_tx, kick_rx) = mpsc::channel::<KickReason>(8);
    let cancel_inner = cancel.clone();
    let name = cfg.name.clone();
    let handle_name = cfg.name.clone();
    let handle_path = cfg.path.clone();
    let initial_state = RepoState::new(cfg.name.clone(), cfg.path.clone());
    let (state_tx, state_rx) = watch::channel(initial_state);

    let join = tokio::spawn(async move {
        let span = info_span!("repo", name = %name);
        async move {
            if let Err(e) = run(cfg, cancel_inner, kick_rx, state_tx).await {
                warn!(error = %e, "worker exited with error");
            }
        }
        .instrument(span)
        .await
    });

    Ok(WorkerHandle {
        name: handle_name,
        path: handle_path,
        cancel,
        kick_tx,
        state_rx,
        join,
    })
}

async fn run(
    cfg: WorkerConfig,
    cancel: CancellationToken,
    mut kick_rx: mpsc::Receiver<KickReason>,
    state_tx: watch::Sender<RepoState>,
) -> Result<()> {
    let git = Git::with_timeout(&cfg.path, cfg.git_timeout);
    let mut fs = fs_watch::spawn(&cfg.path)?;
    let lock = Arc::new(Mutex::new(()));
    let mut state = state_tx.borrow().clone();

    info!(
        path = %cfg.path.display(),
        interval = ?cfg.interval,
        debounce = ?cfg.debounce,
        "worker started"
    );

    do_cycle(&git, &cfg, &lock, &mut state, &state_tx, "startup").await;
    let timer = sleep_until(Instant::now() + next_timer_delay(&cfg, &state));
    tokio::pin!(timer);

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            Some(_) = fs.rx.recv() => {
                debouncer::drain(&mut fs.rx, cfg.debounce).await;
                do_cycle(&git, &cfg, &lock, &mut state, &state_tx, "fs-change").await;
                timer.as_mut().reset(Instant::now() + next_timer_delay(&cfg, &state));
            }
            _ = &mut timer => {
                do_cycle(&git, &cfg, &lock, &mut state, &state_tx, "timer").await;
                timer.as_mut().reset(Instant::now() + next_timer_delay(&cfg, &state));
            }
            Some(reason) = kick_rx.recv() => {
                let label = match reason {
                    KickReason::Manual => "kick-manual",
                    KickReason::External => "kick-external",
                };
                do_cycle(&git, &cfg, &lock, &mut state, &state_tx, label).await;
                timer.as_mut().reset(Instant::now() + next_timer_delay(&cfg, &state));
            }
        }
    }
    state.running = false;
    let _ = state_tx.send_replace(state);
    info!("worker stopped");
    Ok(())
}

async fn do_cycle(
    git: &Git,
    cfg: &WorkerConfig,
    lock: &Arc<Mutex<()>>,
    state: &mut RepoState,
    state_tx: &watch::Sender<RepoState>,
    reason: &str,
) {
    let _guard = lock.lock().await;
    state.running = true;
    refresh_refs(git, state).await;
    let _ = state_tx.send_replace(state.clone());

    let params = CycleParams {
        commit_template: &cfg.commit_template,
        auto_push: cfg.auto_push,
        auto_pull: cfg.auto_pull,
        branch: cfg.branch.as_deref(),
        remote: cfg.remote.as_deref(),
        ignore: &cfg.ignore,
    };
    let report = sync_cycle(git, &params).await;
    let outcome = report.summary();
    apply_report(state, &report);
    refresh_refs(git, state).await;
    state.running = false;
    let _ = state_tx.send_replace(state.clone());

    if let Some(err) = &state.last_sync.last_error {
        warn!(reason, ?outcome, error = %err, "cycle ended with error");
    } else {
        info!(reason, ?outcome, "cycle ok");
    }
}

async fn refresh_refs(git: &Git, state: &mut RepoState) {
    state.current_branch = git.current_branch().await.ok();
    state.upstream = git.upstream_name().await.ok();
}

fn apply_report(state: &mut RepoState, report: &CycleReport) {
    let ts = status_timestamp();
    state.last_sync.last_cycle_at = Some(ts.clone());
    state.last_sync.last_outcome = report.summary();

    if report.committed {
        state.last_sync.committed_at = Some(ts.clone());
    }
    if report.pulled {
        state.last_sync.pulled_at = Some(ts.clone());
    }
    if report.pushed {
        state.last_sync.pushed_at = Some(ts.clone());
    }

    state.last_sync.last_error = report.failure_message();
    if state.last_sync.last_error.is_some() {
        state.last_sync.consecutive_failures =
            state.last_sync.consecutive_failures.saturating_add(1);
    } else {
        state.last_sync.consecutive_failures = 0;
    }
}

fn next_timer_delay(cfg: &WorkerConfig, state: &RepoState) -> Duration {
    let failures = state.last_sync.consecutive_failures;
    if failures == 0 {
        return cfg.interval;
    }

    let shift = failures.saturating_sub(1).min(31);
    let factor = 1_u32 << shift;
    cfg.backoff_min.saturating_mul(factor).min(cfg.backoff_max)
}

fn status_timestamp() -> String {
    now_local()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> WorkerConfig {
        WorkerConfig {
            name: "repo".to_string(),
            path: PathBuf::from("/tmp/repo"),
            interval: Duration::from_secs(15),
            debounce: Duration::from_secs(2),
            backoff_min: Duration::from_secs(10),
            backoff_max: Duration::from_secs(60),
            git_timeout: Duration::from_secs(60),
            branch: None,
            remote: None,
            commit_template: "{ts} ({host})".to_string(),
            auto_push: true,
            auto_pull: true,
            ignore: Vec::new(),
        }
    }

    #[test]
    fn timer_delay_uses_exponential_backoff() {
        let cfg = cfg();
        let mut state = RepoState::new("repo".to_string(), PathBuf::from("/tmp/repo"));
        assert_eq!(next_timer_delay(&cfg, &state), Duration::from_secs(15));

        state.last_sync.consecutive_failures = 1;
        assert_eq!(next_timer_delay(&cfg, &state), Duration::from_secs(10));

        state.last_sync.consecutive_failures = 3;
        assert_eq!(next_timer_delay(&cfg, &state), Duration::from_secs(40));

        state.last_sync.consecutive_failures = 10;
        assert_eq!(next_timer_delay(&cfg, &state), Duration::from_secs(60));
    }
}
