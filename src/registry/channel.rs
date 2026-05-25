use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

use super::ChannelError;

pub type Sender = mpsc::Sender<Value>;
pub type Receiver = mpsc::Receiver<Value>;
pub type ReceiverSlot = Arc<tokio::sync::Mutex<Option<Receiver>>>;

pub struct Mailbox {
    tx: Sender,
    rx: ReceiverSlot,
}

pub struct Channel {
    mailboxes: HashMap<Uuid, Mailbox>,
    available: Option<Uuid>,
}

impl Channel {
    pub(crate) fn new(buffer: usize) -> (Self, Uuid) {
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
                rx: Arc::new(Mutex::new(Some(rx_2to1))),
            },
        );
        mailboxes.insert(
            endpoint2,
            Mailbox {
                tx: tx_2to1,
                rx: Arc::new(Mutex::new(Some(rx_1to2))),
            },
        );

        (
            Channel {
                mailboxes,
                available: Some(endpoint2),
            },
            endpoint1,
        )
    }

    pub(crate) fn join(&mut self) -> Result<Uuid, ChannelError> {
        self.available.take().ok_or(ChannelError::ChannelFull)
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
