use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KickReason {
    Manual,
    External,
}

pub struct WorkerHandle {
    pub cancel: CancellationToken,
    pub kick_tx: mpsc::Sender<KickReason>,
    pub join: JoinHandle<()>,
}
