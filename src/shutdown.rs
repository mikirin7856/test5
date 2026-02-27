// src/shutdown.rs
use tokio::sync::watch;

#[derive(Clone)]
pub struct Shutdown {
    rx: watch::Receiver<bool>,
}

pub struct ShutdownTrigger {
    tx: watch::Sender<bool>,
}

pub fn shutdown_channel() -> (ShutdownTrigger, Shutdown) {
    let (tx, rx) = watch::channel(false);
    (ShutdownTrigger { tx }, Shutdown { rx })
}

impl ShutdownTrigger {
    pub fn trigger(self) {
        let _ = self.tx.send(true);
    }
}

impl Shutdown {
    pub async fn cancelled(&mut self) {
        while !*self.rx.borrow() {
            if self.rx.changed().await.is_err() {
                break;
            }
        }
    }

    pub fn is_cancelled(&self) -> bool {
        *self.rx.borrow()
    }
}
