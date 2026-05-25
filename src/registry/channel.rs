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

impl Mailbox {
    pub fn new(buffer: usize) -> Self {
        let (tx, rx) = mpsc::channel(buffer);
        Mailbox {
            tx,
            rx: Arc::new(Mutex::new(Some(rx))),
        }
    }
}

pub struct Channel {
    mailboxes: HashMap<Uuid, Mailbox>,
}

impl Channel {
    pub(crate) fn new(buffer: usize) -> (Self, Uuid) {
        let endpoint1 = Uuid::new_v4();
        let endpoint2 = Uuid::new_v4();

        let mailbox_1 = Mailbox::new(buffer);
        let mailbox_2 = Mailbox::new(buffer);

        let mut mailboxes = HashMap::new();
        mailboxes.insert(endpoint1, mailbox_1);
        mailboxes.insert(endpoint2, mailbox_2);

        (Channel { mailboxes }, endpoint1)
    }

    pub(crate) fn sender_for(&self, endpoint: Uuid) -> Result<Sender, ChannelError> {
        self.mailboxes
            .get(&endpoint)
            .map(|m| m.tx.clone())
            .ok_or(ChannelError::InvalidEndpoint)
    }

    pub(crate) fn receiver_slot_for(&self, endpoint: Uuid) -> Result<ReceiverSlot, ChannelError> {
        self.mailboxes
            .get(&endpoint)
            .map(|m| m.rx.clone())
            .ok_or(ChannelError::InvalidEndpoint)
    }
}
