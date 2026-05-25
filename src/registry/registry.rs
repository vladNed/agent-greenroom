use std::{collections::HashMap, sync::Mutex};

use uuid::Uuid;

use super::channel::{Channel, ReceiverSlot, Sender};
use super::error::ChannelError;

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
        self.channels.lock().unwrap().insert(channel_id, channel);
        (channel_id, endpoint_id)
    }

    pub fn sender_for(&self, channel_id: Uuid, endpoint: Uuid) -> Result<Sender, ChannelError> {
        let channels = self.channels.lock().unwrap();
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
        let channels = self.channels.lock().unwrap();
        let channel = channels
            .get(&channel_id)
            .ok_or(ChannelError::ChannelNotFound)?;
        channel.receiver_slot_for(endpoint)
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

    use super::super::error::ChannelError;
    use super::*;

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
