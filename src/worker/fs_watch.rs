use std::path::{Path, PathBuf};
use std::sync::mpsc as stdmpsc;
use std::thread;
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::error::Result;

/// Handle for a filesystem watcher thread. Dropping the handle signals the
/// background thread to exit.
pub struct FsWatchHandle {
    _stop: stdmpsc::Sender<()>,
    pub rx: mpsc::UnboundedReceiver<()>,
}

pub fn spawn(root: &Path) -> Result<FsWatchHandle> {
    let (event_tx, event_rx) = mpsc::unbounded_channel::<()>();
    let (stop_tx, stop_rx) = stdmpsc::channel::<()>();
    let root = root.to_path_buf();

    thread::Builder::new()
        .name(format!("synchrogit-fswatch-{}", root.display()))
        .spawn(move || run_watcher(root, event_tx, stop_rx))?;

    Ok(FsWatchHandle {
        _stop: stop_tx,
        rx: event_rx,
    })
}

fn run_watcher(root: PathBuf, tx: mpsc::UnboundedSender<()>, stop: stdmpsc::Receiver<()>) {
    let (raw_tx, raw_rx) = stdmpsc::channel::<notify::Result<Event>>();
    let mut watcher: RecommendedWatcher = match notify::recommended_watcher(raw_tx) {
        Ok(w) => w,
        Err(e) => {
            warn!(error = %e, "failed to construct fs watcher");
            return;
        }
    };
    if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
        warn!(error = %e, root = %root.display(), "failed to watch root");
        return;
    }

    loop {
        match stop.try_recv() {
            Ok(_) | Err(stdmpsc::TryRecvError::Disconnected) => break,
            Err(stdmpsc::TryRecvError::Empty) => {}
        }
        match raw_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(event)) => {
                if interesting(&event, &root) && tx.send(()).is_err() {
                    break;
                }
            }
            Ok(Err(e)) => warn!(error = %e, "watcher error"),
            Err(stdmpsc::RecvTimeoutError::Timeout) => continue,
            Err(stdmpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    debug!("fs watcher thread exiting");
}

fn interesting(event: &Event, root: &Path) -> bool {
    if !matches!(
        event.kind,
        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
    ) {
        return false;
    }
    let git_dir = root.join(".git");
    event.paths.iter().any(|p| !p.starts_with(&git_dir))
}
