mod channel;
mod error;
mod registry;

pub use channel::{Channel, Mailbox, Receiver, ReceiverSlot, Sender};
pub use error::ChannelError;
pub use registry::Registry;
