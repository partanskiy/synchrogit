use std::path::PathBuf;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::state::RepoState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KickReason {
    Manual,
    External,
}

pub struct WorkerHandle {
    pub name: String,
    pub path: PathBuf,
    pub cancel: CancellationToken,
    pub kick_tx: mpsc::Sender<KickReason>,
    pub state_rx: watch::Receiver<RepoState>,
    pub join: JoinHandle<()>,
}
