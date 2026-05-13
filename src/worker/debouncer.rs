use std::time::Duration;

use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

/// Drain pending events from `rx` for up to `window`. Returns once the channel
/// has been quiet for that long, or `rx` has been closed.
pub async fn drain<T>(rx: &mut UnboundedReceiver<T>, window: Duration) {
    while let Ok(Some(_)) = timeout(window, rx.recv()).await {}
}
