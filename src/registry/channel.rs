use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

use super::{AgentIdentity, ChannelError};

pub type Sender = mpsc::Sender<Value>;
pub type Receiver = mpsc::Receiver<Value>;
pub type ReceiverSlot = Arc<tokio::sync::Mutex<Receiver>>;

pub struct Mailbox {
    tx: Sender,
    rx: ReceiverSlot,
}

pub struct Channel {
    mailboxes: HashMap<Uuid, Mailbox>,
    identities: HashMap<Uuid, AgentIdentity>,
    available: Option<Uuid>,
}

impl Channel {
    pub(crate) fn new(buffer: usize, identity: AgentIdentity) -> (Self, Uuid) {
        let endpoint1 = Uuid::new_v4();
        let endpoint2 = Uuid::new_v4();

        // Cross-wire: each endpoint's tx delivers into the peer's rx so an
        // agent cannot read back its own messages.
        let (tx_1to2, rx_1to2) = mpsc::channel(buffer);
        let (tx_2to1, rx_2to1) = mpsc::channel(buffer);

        let mut mailboxes = HashMap::new();
        mailboxes.insert(
            endpoint1,
            Mailbox {
                tx: tx_1to2,
                rx: Arc::new(Mutex::new(rx_2to1)),
            },
        );
        mailboxes.insert(
            endpoint2,
            Mailbox {
                tx: tx_2to1,
                rx: Arc::new(Mutex::new(rx_1to2)),
            },
        );

        let mut identities = HashMap::new();
        identities.insert(endpoint1, identity);

        (
            Channel {
                mailboxes,
                identities,
                available: Some(endpoint2),
            },
            endpoint1,
        )
    }

    pub(crate) fn join(&mut self, identity: AgentIdentity) -> Result<Uuid, ChannelError> {
        let endpoint = self.available.take().ok_or(ChannelError::ChannelFull)?;
        self.identities.insert(endpoint, identity);
        Ok(endpoint)
    }

    pub(crate) fn peer_identity_for(&self, endpoint: Uuid) -> Result<AgentIdentity, ChannelError> {
        let peer_id = self
            .mailboxes
            .keys()
            .find(|&&id| id != endpoint)
            .ok_or(ChannelError::EndpointNotFound)?;
        self.identities
            .get(peer_id)
            .cloned()
            .ok_or(ChannelError::PeerNotYetJoined)
    }

    pub(crate) fn sender_for(&self, endpoint: Uuid) -> Result<Sender, ChannelError> {
        self.mailboxes
            .get(&endpoint)
            .map(|m| m.tx.clone())
            .ok_or(ChannelError::EndpointNotFound)
    }

    pub(crate) fn receiver_slot_for(&self, endpoint: Uuid) -> Result<ReceiverSlot, ChannelError> {
        self.mailboxes
            .get(&endpoint)
            .map(|m| m.rx.clone())
            .ok_or(ChannelError::EndpointNotFound)
    }
}
