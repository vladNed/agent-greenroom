use thiserror::Error;

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
    #[error("invalid endpoint")]
    InvalidEndpoint,
}

impl ChannelError {
    pub fn to_mcp_error(self) -> rmcp::ErrorData {
        rmcp::ErrorData::invalid_params(self.to_string(), None)
    }
}
