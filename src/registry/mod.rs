mod channel;
mod error;
mod identity;

pub use error::ChannelError;
pub use identity::AgentIdentity;

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

    pub fn create(&self, buffer: usize, identity: AgentIdentity) -> (Uuid, Uuid) {
        let channel_id = Uuid::new_v4();
        let (channel, endpoint_id) = Channel::new(buffer, identity);
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

    pub fn join(&self, channel_id: Uuid, identity: AgentIdentity) -> Result<(Uuid, AgentIdentity), ChannelError> {
        let mut channels = self.channels.lock().expect("registry lock poisoned");
        let channel = channels
            .get_mut(&channel_id)
            .ok_or(ChannelError::ChannelNotFound)?;
        let endpoint_id = channel.join(identity)?;
        let peer_identity = channel.peer_identity_for(endpoint_id)?;
        Ok((endpoint_id, peer_identity))
    }

    pub fn peer_identity_for(&self, channel_id: Uuid, endpoint: Uuid) -> Result<AgentIdentity, ChannelError> {
        let channels = self.channels.lock().expect("registry lock poisoned");
        let channel = channels
            .get(&channel_id)
            .ok_or(ChannelError::ChannelNotFound)?;
        channel.peer_identity_for(endpoint)
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

    fn stub(name: &str) -> AgentIdentity {
        AgentIdentity { name: name.to_string(), model: "test-model".to_string() }
    }

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
        let (id, ep1) = reg.create(1024, stub("creator"));

        let (ep2, _peer) = reg.join(id, stub("joiner")).unwrap();

        assert_ne!(ep1, ep2);
        assert!(reg.sender_for(id, ep2).is_ok());
    }

    #[tokio::test]
    async fn test_join_nonexistent_channel() {
        let reg = Registry::new();
        let fake = Uuid::new_v4();

        assert!(matches!(
            reg.join(fake, stub("a")),
            Err(ChannelError::ChannelNotFound)
        ));
    }

    #[tokio::test]
    async fn test_join_twice_fails() {
        let reg = Registry::new();
        let (id, _) = reg.create(1024, stub("creator"));

        let _ = reg.join(id, stub("joiner")).unwrap();
        assert!(matches!(reg.join(id, stub("late")), Err(ChannelError::ChannelFull)));
    }

    #[tokio::test]
    async fn test_join_endpoints_independent() {
        let reg = Registry::new();
        let (id, ep1) = reg.create(2, stub("creator"));

        let (ep2, _) = reg.join(id, stub("joiner")).unwrap();

        let tx1 = reg.sender_for(id, ep1).unwrap();
        let tx2 = reg.sender_for(id, ep2).unwrap();

        tx1.try_send(json!("from_a")).unwrap();
        tx2.try_send(json!("from_b")).unwrap();

        // Each endpoint reads what the OTHER sent, not its own message.
        let slot1 = reg.receiver_slot_for(id, ep1).unwrap();
        assert_eq!(slot1.lock().await.recv().await, Some(json!("from_b")));

        let slot2 = reg.receiver_slot_for(id, ep2).unwrap();
        assert_eq!(slot2.lock().await.recv().await, Some(json!("from_a")));
    }

    #[tokio::test]
    async fn test_recv_in_flight_detected_via_try_lock() {
        let reg = Registry::new();
        let (id, ep) = reg.create(1024, stub("creator"));
        let slot = reg.receiver_slot_for(id, ep).unwrap();

        let _guard = slot.lock().await;

        assert!(slot.try_lock().is_err());
    }

    #[tokio::test]
    async fn test_recv_cancel_safe() {
        // Aborting an in-flight recv must not lose the receiver: a subsequent
        // recv on the same slot must succeed.
        let reg = Arc::new(Registry::new());
        let (id, ep1) = reg.create(1024, stub("creator"));
        let (ep2, _) = reg.join(id, stub("joiner")).unwrap();

        let slot = reg.receiver_slot_for(id, ep1).unwrap();

        let slot_for_task = slot.clone();
        let task = tokio::spawn(async move {
            let mut guard = slot_for_task.lock().await;
            let _ = guard.recv().await;
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        task.abort();
        let _ = task.await;

        // Peer sends; ep1 must still be able to receive (receiver not lost on cancel).
        reg.sender_for(id, ep2).unwrap().try_send(json!("hello")).unwrap();

        let got = tokio::time::timeout(Duration::from_millis(100), async {
            slot.lock().await.recv().await
        })
        .await
        .expect("recv timed out — receiver was lost on cancel");
        assert_eq!(got, Some(json!("hello")));
    }

    #[tokio::test]
    async fn test_close_unblocks_parked_recv() {
        let reg = Arc::new(Registry::new());
        let (id, ep) = reg.create(1024, stub("creator"));
        let slot = reg.receiver_slot_for(id, ep).unwrap();

        let reg2 = reg.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1)).await;
            reg2.close(id).unwrap();
        });

        assert!(slot.lock().await.recv().await.is_none());
    }

    #[tokio::test]
    async fn test_buffer_full() {
        let reg = Registry::new();
        let (id, ep) = reg.create(2, stub("creator"));
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

    #[tokio::test]
    async fn test_create_and_peer_identity() {
        let reg = Registry::new();
        let creator = AgentIdentity { name: "Claude Code".to_string(), model: "claude-sonnet-4-6".to_string() };
        let joiner = AgentIdentity { name: "OpenCode".to_string(), model: "deepseek-chat".to_string() };

        let (id, ep1) = reg.create(1024, creator);
        let (ep2, peer_seen_by_joiner) = reg.join(id, joiner).unwrap();

        assert_eq!(peer_seen_by_joiner.name, "Claude Code");

        let peer_seen_by_creator = reg.peer_identity_for(id, ep1).unwrap();
        assert_eq!(peer_seen_by_creator.name, "OpenCode");

        assert_ne!(ep1, ep2);
    }

    #[tokio::test]
    async fn test_peer_not_yet_joined() {
        let reg = Registry::new();
        let (id, ep1) = reg.create(1024, stub("creator"));

        assert!(matches!(
            reg.peer_identity_for(id, ep1),
            Err(ChannelError::PeerNotYetJoined)
        ));
    }
}
