use std::sync::Arc;

use rmcp::{ErrorData, handler::server::wrapper::Parameters, tool, tool_router};
use uuid::Uuid;

use crate::registry::{ChannelError, Registry};

use super::params::{ChannelEndpointParams, SendParams};

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

fn parse_ids(channel_str: &str, endpoint_str: &str) -> Result<(Uuid, Uuid), ErrorData> {
    let channel_id = channel_str
        .parse::<Uuid>()
        .map_err(|_| ChannelError::InvalidChannelId.to_mcp_error())?;
    let endpoint_id = endpoint_str
        .parse::<Uuid>()
        .map_err(|_| ChannelError::InvalidEndpoint.to_mcp_error())?;
    Ok((channel_id, endpoint_id))
}

#[tool_router(server_handler)]
impl ChannelsServer {
    #[tool(
        description = "Create a new channel; returns { \"channel_id\": \"<uuid>\", \"endpoint_id\": \"<uuid>\" }"
    )]
    async fn channels_create(&self) -> Result<String, ErrorData> {
        let (channel_id, endpoint_id) = self.registry.create(self.buffer_size);
        Ok(serde_json::json!({ "channel_id": channel_id, "endpoint_id": endpoint_id }).to_string())
    }

    #[tool(description = "Send a JSON message to a channel; returns { \"ok\": true }")]
    async fn channels_send(
        &self,
        Parameters(SendParams {
            channel_id,
            endpoint_id,
            message,
        }): Parameters<SendParams>,
    ) -> Result<String, ErrorData> {
        let (channel_id, endpoint_id) = parse_ids(&channel_id, &endpoint_id)?;
        let sender = self
            .registry
            .sender_for(channel_id, endpoint_id)
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
        description = "Receive a message from a channel using your endpoint; blocks until a message arrives or the channel is closed. Returns { \"message\": <json> } or { \"closed\": true }"
    )]
    async fn channels_recv(
        &self,
        Parameters(ChannelEndpointParams {
            channel_id,
            endpoint_id,
        }): Parameters<ChannelEndpointParams>,
    ) -> Result<String, ErrorData> {
        let (channel_id, endpoint_id) = parse_ids(&channel_id, &endpoint_id)?;
        let slot = self
            .registry
            .receiver_slot_for(channel_id, endpoint_id)
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
        Parameters(ChannelEndpointParams {
            channel_id,
            endpoint_id,
        }): Parameters<ChannelEndpointParams>,
    ) -> Result<String, ErrorData> {
        let (channel_id, _) = parse_ids(&channel_id, &endpoint_id)?;
        self.registry
            .close(channel_id)
            .map_err(ChannelError::to_mcp_error)?;
        Ok(serde_json::json!({ "ok": true }).to_string())
    }
}
