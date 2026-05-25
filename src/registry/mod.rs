mod channel;
mod error;

pub use error::ChannelError;

use std::{collections::HashMap, sync::Mutex};

use uuid::Uuid;

use channel::{Channel, ReceiverSlot, Sender};

pub struct Registry {
    channels: Mutex<HashMap<Uuid, Channel>>,
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

    pub fn create(&self, buffer: usize) -> (Uuid, Uuid) {
        let channel_id = Uuid::new_v4();
        let (channel, endpoint_id) = Channel::new(buffer);
        self.channels.lock().expect("registry lock poisoned").insert(channel_id, channel);
        (channel_id, endpoint_id)
    }

    pub fn sender_for(&self, channel_id: Uuid, endpoint: Uuid) -> Result<Sender, ChannelError> {
        let channels = self.channels.lock().expect("registry lock poisoned");
        let channel = channels
            .get(&channel_id)
            .ok_or(ChannelError::ChannelNotFound)?;
        channel.sender_for(endpoint)
    }

    pub fn receiver_slot_for(
        &self,
        channel_id: Uuid,
        endpoint: Uuid,
    ) -> Result<ReceiverSlot, ChannelError> {
        let channels = self.channels.lock().expect("registry lock poisoned");
        let channel = channels
            .get(&channel_id)
            .ok_or(ChannelError::ChannelNotFound)?;
        channel.receiver_slot_for(endpoint)
    }

    pub fn join(&self, channel_id: Uuid) -> Result<Uuid, ChannelError> {
        let mut channels = self.channels.lock().expect("registry lock poisoned");
        let channel = channels
            .get_mut(&channel_id)
            .ok_or(ChannelError::ChannelNotFound)?;
        channel.join()
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
    use crate::registry::ChannelError;

    #[tokio::test]
    async fn test_send_unknown_channel() {
        let reg = Registry::new();
        let fake = Uuid::new_v4();
        assert!(matches!(
            reg.sender_for(fake, Uuid::new_v4()),
            Err(ChannelError::ChannelNotFound)
        ));
    }

    #[tokio::test]
    async fn test_join_success() {
        let reg = Registry::new();
        let (id, ep1) = reg.create(1024);

        let ep2 = reg.join(id).unwrap();

        assert_ne!(ep1, ep2);
        assert!(reg.sender_for(id, ep2).is_ok());
    }

    #[tokio::test]
    async fn test_join_nonexistent_channel() {
        let reg = Registry::new();
        let fake = Uuid::new_v4();

        assert!(matches!(
            reg.join(fake),
            Err(ChannelError::ChannelNotFound)
        ));
    }

    #[tokio::test]
    async fn test_join_twice_fails() {
        let reg = Registry::new();
        let (id, _) = reg.create(1024);

        let _ep2 = reg.join(id).unwrap();
        assert!(matches!(reg.join(id), Err(ChannelError::ChannelFull)));
    }

    #[tokio::test]
    async fn test_join_endpoints_independent() {
        let reg = Registry::new();
        let (id, ep1) = reg.create(2);

        let ep2 = reg.join(id).unwrap();

        let tx1 = reg.sender_for(id, ep1).unwrap();
        let tx2 = reg.sender_for(id, ep2).unwrap();

        tx1.try_send(json!("from_a")).unwrap();
        tx2.try_send(json!("from_b")).unwrap();

        // Each endpoint reads what the OTHER sent, not its own message.
        let slot1 = reg.receiver_slot_for(id, ep1).unwrap();
        let mut rx1 = slot1.lock().await.take().unwrap();
        assert_eq!(rx1.recv().await, Some(json!("from_b")));

        let slot2 = reg.receiver_slot_for(id, ep2).unwrap();
        let mut rx2 = slot2.lock().await.take().unwrap();
        assert_eq!(rx2.recv().await, Some(json!("from_a")));
    }

    #[tokio::test]
    async fn test_recv_already_in_flight() {
        let reg = Registry::new();
        let (id, ep) = reg.create(1024);
        let slot = reg.receiver_slot_for(id, ep).unwrap();

        let _rx = slot.lock().await.take().unwrap();

        assert!(slot.lock().await.take().is_none());
    }

    #[tokio::test]
    async fn test_close_unblocks_parked_recv() {
        let reg = Arc::new(Registry::new());
        let (id, ep) = reg.create(1024);
        let slot = reg.receiver_slot_for(id, ep).unwrap();

        let mut guard = slot.lock().await;
        let mut rx = guard.take().unwrap();
        drop(guard);

        let reg2 = reg.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1)).await;
            reg2.close(id).unwrap();
        });

        assert!(rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn test_buffer_full() {
        let reg = Registry::new();
        let (id, ep) = reg.create(2);
        let sender = reg.sender_for(id, ep).unwrap();

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
