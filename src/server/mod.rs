use std::sync::Arc;

use rmcp::{ErrorData, handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::registry::{ChannelError, Registry};

pub struct ChannelsServer {
    pub registry: Arc<Registry>,
    pub buffer_size: usize,
}

impl ChannelsServer {
    pub fn new(registry: Arc<Registry>, buffer_size: usize) -> Self {
        Self {
            registry,
            buffer_size,
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
struct SendParams {
    channel_id: String,
    message: Value,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct ChannelIdParam {
    channel_id: String,
}

fn parse_id(s: &str) -> Result<Uuid, ErrorData> {
    s.parse::<Uuid>()
        .map_err(|_| ChannelError::InvalidChannelId.to_mcp_error())
}

#[tool_router(server_handler)]
impl ChannelsServer {
    #[tool(description = "Create a new channel; returns { \"channel_id\": \"<uuid>\" }")]
    async fn channels_create(&self) -> Result<String, ErrorData> {
        let id = self.registry.create(self.buffer_size);
        Ok(serde_json::json!({ "channel_id": id }).to_string())
    }

    #[tool(description = "Send a JSON message to a channel; returns { \"ok\": true }")]
    async fn channels_send(
        &self,
        Parameters(SendParams {
            channel_id,
            message,
        }): Parameters<SendParams>,
    ) -> Result<String, ErrorData> {
        let id = parse_id(&channel_id)?;
        let sender = self
            .registry
            .sender_for(id)
            .map_err(ChannelError::to_mcp_error)?;
        sender.try_send(message).map_err(|e| match e {
            tokio::sync::mpsc::error::TrySendError::Full(_) => {
                ChannelError::BufferFull.to_mcp_error()
            }
            tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                ChannelError::ChannelNotFound.to_mcp_error()
            }
        })?;
        Ok(serde_json::json!({ "ok": true }).to_string())
    }

    #[tool(
        description = "Receive a message from a channel; blocks until a message arrives or the channel is closed. Returns { \"message\": <json> } or { \"closed\": true }"
    )]
    async fn channels_recv(
        &self,
        Parameters(ChannelIdParam { channel_id }): Parameters<ChannelIdParam>,
    ) -> Result<String, ErrorData> {
        let id = parse_id(&channel_id)?;
        let slot = self
            .registry
            .receiver_slot(id)
            .map_err(ChannelError::to_mcp_error)?;

        let mut guard = slot.lock().await;
        let mut rx = guard
            .take()
            .ok_or_else(|| ChannelError::RecvAlreadyInFlight.to_mcp_error())?;
        drop(guard);

        match rx.recv().await {
            Some(v) => {
                *slot.lock().await = Some(rx);
                Ok(serde_json::json!({ "message": v }).to_string())
            }
            None => Ok(serde_json::json!({ "closed": true }).to_string()),
        }
    }

    #[tool(description = "Close a channel; returns { \"ok\": true }")]
    async fn channels_close(
        &self,
        Parameters(ChannelIdParam { channel_id }): Parameters<ChannelIdParam>,
    ) -> Result<String, ErrorData> {
        let id = parse_id(&channel_id)?;
        self.registry
            .close(id)
            .map_err(ChannelError::to_mcp_error)?;
        Ok(serde_json::json!({ "ok": true }).to_string())
    }
}
