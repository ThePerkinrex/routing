use flume::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
use tracing::warn;

pub struct CtrlCHandler {
    disconnect: Sender<()>,
    notify: Arc<Notify>,
}

impl CtrlCHandler {
    pub async fn next(&self) {
        self.notify.notified().await
    }
}

impl Drop for CtrlCHandler {
    fn drop(&mut self) {
        self.disconnect.send(()).unwrap()
    }
}

type Handles = Vec<(Receiver<()>, Arc<Notify>)>;

pub struct CtrlC {
    stack: Arc<Mutex<Handles>>,
}

fn handle_ctrlc(lock: &mut Handles) {
    lock.last()
        .map(|(d_rx, n)| match d_rx.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => true,
            Err(TryRecvError::Empty) => {
                n.notify_one();
                false
            }
        })
        .map_or_else(
            || {
                warn!("No ctrl-c handlers");
            },
            |should_disc| {
                if should_disc {
                    lock.pop();
                    handle_ctrlc(lock);
                }
            },
        );
}

impl CtrlC {
    pub fn new() -> Result<Self, ::ctrlc::Error> {
        let stack = Arc::new(Mutex::new(Handles::new()));
        let mutex_clone = stack.clone();
        ::ctrlc::set_handler(move || {
            let mut lock = stack.lock().unwrap();
            handle_ctrlc(&mut lock);
            drop(lock)
        })?;
        Ok(Self { stack: mutex_clone })
    }

    pub async fn add_handler(&self) -> CtrlCHandler {
        let stack = self.stack.clone();
        let (tx, rx) = flume::bounded(1);
        let n = Arc::new(Notify::new());
        let n_clone = n.clone();
        let _: Result<Result<(), ()>, tokio::task::JoinError> =
            tokio::task::spawn_blocking(move || {
                stack.lock().map_err(|_| ())?.push((rx, n_clone));
                Ok(())
            })
            .await;
        CtrlCHandler {
            disconnect: tx,
            notify: n,
        }
    }
}
