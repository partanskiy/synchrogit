use std::path::{Path, PathBuf};
use std::sync::mpsc as stdmpsc;
use std::thread;
use std::time::Duration;

use notify::event::ModifyKind;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::error::Result;

pub struct ConfigWatchHandle {
    _stop: stdmpsc::Sender<()>,
    pub rx: mpsc::UnboundedReceiver<()>,
}

pub fn spawn(config_path: &Path) -> Result<ConfigWatchHandle> {
    let (event_tx, event_rx) = mpsc::unbounded_channel::<()>();
    let (stop_tx, stop_rx) = stdmpsc::channel::<()>();
    let path = config_path.to_path_buf();

    thread::Builder::new()
        .name(format!("synchrogit-config-watch-{}", path.display()))
        .spawn(move || run_watcher(path, event_tx, stop_rx))?;

    Ok(ConfigWatchHandle {
        _stop: stop_tx,
        rx: event_rx,
    })
}

fn run_watcher(path: PathBuf, tx: mpsc::UnboundedSender<()>, stop: stdmpsc::Receiver<()>) {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    // FSEvents on macOS reports event paths with symlinks resolved (e.g.
    // $TMPDIR lives under /var, a symlink to /private/var), so watch and
    // compare against the canonical parent or events never match the target.
    let parent = parent.canonicalize().unwrap_or(parent);
    let path = match path.file_name() {
        Some(name) => parent.join(name),
        None => path,
    };

    let (raw_tx, raw_rx) = stdmpsc::channel::<notify::Result<Event>>();
    let mut watcher: RecommendedWatcher = match notify::recommended_watcher(raw_tx) {
        Ok(w) => w,
        Err(e) => {
            warn!(error = %e, "failed to construct config watcher");
            return;
        }
    };

    if let Err(e) = watcher.watch(&parent, RecursiveMode::NonRecursive) {
        warn!(error = %e, path = %path.display(), "failed to watch config parent");
        return;
    }

    loop {
        match stop.try_recv() {
            Ok(_) | Err(stdmpsc::TryRecvError::Disconnected) => break,
            Err(stdmpsc::TryRecvError::Empty) => {}
        }

        match raw_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(event)) => {
                if interesting(&event, &path) && tx.send(()).is_err() {
                    break;
                }
            }
            Ok(Err(e)) => warn!(error = %e, "config watcher error"),
            Err(stdmpsc::RecvTimeoutError::Timeout) => continue,
            Err(stdmpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    debug!("config watcher thread exiting");
}

fn interesting(event: &Event, target: &Path) -> bool {
    if !matches!(
        event.kind,
        EventKind::Any
            | EventKind::Create(_)
            | EventKind::Modify(ModifyKind::Any | ModifyKind::Data(_) | ModifyKind::Name(_))
            | EventKind::Remove(_)
    ) {
        return false;
    }

    event.paths.iter().any(|p| same_target(p, target))
}

fn same_target(path: &Path, target: &Path) -> bool {
    if path == target {
        return true;
    }

    path.file_name() == target.file_name()
        && path.parent().unwrap_or_else(|| Path::new(""))
            == target.parent().unwrap_or_else(|| Path::new(""))
}
