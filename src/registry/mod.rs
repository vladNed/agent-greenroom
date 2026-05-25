use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde_json::Value;
use thiserror::Error;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum ChannelError {
    #[error("channel not found")]
    ChannelNotFound,
    #[error("buffer full")]
    BufferFull,
    #[error("recv already in flight")]
    RecvAlreadyInFlight,
    #[error("invalid channel id")]
    InvalidChannelId,
}

impl ChannelError {
    pub fn to_mcp_error(self) -> rmcp::ErrorData {
        rmcp::ErrorData::invalid_params(self.to_string(), None)
    }
}

type Sender = mpsc::Sender<Value>;
type Receiver = mpsc::Receiver<Value>;
pub type ReceiverSlot = Arc<tokio::sync::Mutex<Option<Receiver>>>;

struct ChannelState {
    sender: Sender,
    receiver: ReceiverSlot,
}

pub struct Registry {
    channels: Mutex<HashMap<Uuid, ChannelState>>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Self {
            channels: Mutex::new(HashMap::new()),
        }
    }

    pub fn create(&self, buffer: usize) -> Uuid {
        let id = Uuid::new_v4();
        let (tx, rx) = mpsc::channel(buffer);
        self.channels.lock().unwrap().insert(
            id,
            ChannelState {
                sender: tx,
                receiver: Arc::new(tokio::sync::Mutex::new(Some(rx))),
            },
        );
        id
    }

    pub fn sender_for(&self, id: Uuid) -> Result<Sender, ChannelError> {
        self.channels
            .lock()
            .unwrap()
            .get(&id)
            .map(|s| s.sender.clone())
            .ok_or(ChannelError::ChannelNotFound)
    }

    pub fn receiver_slot(&self, id: Uuid) -> Result<ReceiverSlot, ChannelError> {
        self.channels
            .lock()
            .unwrap()
            .get(&id)
            .map(|s| s.receiver.clone())
            .ok_or(ChannelError::ChannelNotFound)
    }

    pub fn close(&self, id: Uuid) -> Result<(), ChannelError> {
        self.channels
            .lock()
            .unwrap()
            .remove(&id)
            .map(|_| ())
            .ok_or(ChannelError::ChannelNotFound)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn test_round_trip() {
        let reg = Registry::new();
        let id = reg.create(1024);

        let sender = reg.sender_for(id).unwrap();
        sender.send(json!("hello")).await.unwrap();

        let slot = reg.receiver_slot(id).unwrap();
        let mut guard = slot.lock().await;
        let mut rx = guard.take().unwrap();
        drop(guard);

        let msg = rx.recv().await.unwrap();
        assert_eq!(msg, json!("hello"));

        *slot.lock().await = Some(rx);
    }

    #[tokio::test]
    async fn test_send_unknown_channel() {
        let reg = Registry::new();
        let fake = Uuid::new_v4();
        assert!(matches!(
            reg.sender_for(fake),
            Err(ChannelError::ChannelNotFound)
        ));
    }

    #[tokio::test]
    async fn test_recv_already_in_flight() {
        let reg = Registry::new();
        let id = reg.create(1024);
        let slot = reg.receiver_slot(id).unwrap();

        // First recv takes the receiver out of the slot.
        let _rx = slot.lock().await.take().unwrap();

        // Second recv sees None.
        assert!(slot.lock().await.take().is_none());
    }

    #[tokio::test]
    async fn test_close_unblocks_parked_recv() {
        let reg = Arc::new(Registry::new());
        let id = reg.create(1024);
        let slot = reg.receiver_slot(id).unwrap();

        let mut guard = slot.lock().await;
        let mut rx = guard.take().unwrap();
        drop(guard);

        let reg2 = reg.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            reg2.close(id).unwrap();
        });

        // Sender is dropped when close() removes the entry → recv returns None.
        assert!(rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn test_buffer_full() {
        let reg = Registry::new();
        let id = reg.create(2);
        let sender = reg.sender_for(id).unwrap();

        sender.try_send(json!(1)).unwrap();
        sender.try_send(json!(2)).unwrap();

        assert!(
            matches!(
                sender.try_send(json!(3)),
                Err(tokio::sync::mpsc::error::TrySendError::Full(_))
            ),
            "expected TrySendError::Full on a full buffer"
        );
    }
}
