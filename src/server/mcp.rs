use std::sync::Arc;
use std::time::Duration;

use rmcp::{ErrorData, handler::server::wrapper::Parameters, tool, tool_router};
use uuid::Uuid;

use crate::registry::{AgentIdentity, ChannelError, Registry};

use super::params::{
    ChannelCloseParams, ChannelCreateParams, ChannelEndpointParams, ChannelJoinParams, RecvParams,
    SendParams,
};

const DEFAULT_RECV_WAIT_MS: u64 = 50_000;
const MAX_RECV_WAIT_MS: u64 = 600_000;

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

fn parse_channel_id(s: &str) -> Result<Uuid, ErrorData> {
    s.parse::<Uuid>()
        .map_err(|_| ChannelError::InvalidChannelId.to_mcp_error())
}

fn parse_ids(channel_str: &str, endpoint_str: &str) -> Result<(Uuid, Uuid), ErrorData> {
    let channel_id = parse_channel_id(channel_str)?;
    let endpoint_id = endpoint_str
        .parse::<Uuid>()
        .map_err(|_| ChannelError::InvalidEndpointId.to_mcp_error())?;
    Ok((channel_id, endpoint_id))
}

#[tool_router(server_handler)]
impl ChannelsServer {
    #[tool(
        description = "Create a new channel; returns { \"channel_id\": \"<uuid>\", \"endpoint_id\": \"<uuid>\" }"
    )]
    async fn channels_create(
        &self,
        Parameters(ChannelCreateParams { name, model }): Parameters<ChannelCreateParams>,
    ) -> Result<String, ErrorData> {
        let identity = AgentIdentity { name, model };
        let (channel_id, endpoint_id) = self.registry.create(self.buffer_size, identity);
        Ok(serde_json::json!({ "channel_id": channel_id, "endpoint_id": endpoint_id }).to_string())
    }

    #[tool(
        description = "Join an existing channel; returns { \"endpoint_id\": \"<uuid>\", \"peer\": { \"name\": \"...\", \"model\": \"...\" } }"
    )]
    async fn channels_join(
        &self,
        Parameters(ChannelJoinParams {
            channel_id,
            name,
            model,
        }): Parameters<ChannelJoinParams>,
    ) -> Result<String, ErrorData> {
        let channel_id = parse_channel_id(&channel_id)?;
        let identity = AgentIdentity { name, model };
        let (endpoint_id, peer) = self
            .registry
            .join(channel_id, identity)
            .map_err(ChannelError::to_mcp_error)?;
        Ok(serde_json::json!({
            "endpoint_id": endpoint_id,
            "peer": { "name": peer.name, "model": peer.model }
        })
        .to_string())
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
        description = "Receive a message from a channel using your endpoint; long-polls up to wait_ms (default 50000, max 600000). Returns { \"message\": <json> }, { \"closed\": true }, or { \"timed_out\": true } — re-call on timed_out to keep waiting. Cancel-safe: if the call is aborted, the receiver is not lost."
    )]
    async fn channels_recv(
        &self,
        Parameters(RecvParams {
            channel_id,
            endpoint_id,
            wait_ms,
        }): Parameters<RecvParams>,
    ) -> Result<String, ErrorData> {
        let (channel_id, endpoint_id) = parse_ids(&channel_id, &endpoint_id)?;
        let slot = self
            .registry
            .receiver_slot_for(channel_id, endpoint_id)
            .map_err(ChannelError::to_mcp_error)?;

        let mut guard = slot
            .try_lock()
            .map_err(|_| ChannelError::RecvAlreadyInFlight.to_mcp_error())?;

        let wait = wait_ms
            .unwrap_or(DEFAULT_RECV_WAIT_MS)
            .min(MAX_RECV_WAIT_MS);

        match tokio::time::timeout(Duration::from_millis(wait), guard.recv()).await {
            Ok(Some(v)) => Ok(serde_json::json!({ "message": v }).to_string()),
            Ok(None) => Ok(serde_json::json!({ "closed": true }).to_string()),
            Err(_) => Ok(serde_json::json!({ "timed_out": true }).to_string()),
        }
    }

    #[tool(
        description = "Query the peer's identity for your endpoint; returns { \"peer\": { \"name\": \"...\", \"model\": \"...\" } } or an error if the peer has not joined yet"
    )]
    async fn channels_peer(
        &self,
        Parameters(ChannelEndpointParams {
            channel_id,
            endpoint_id,
        }): Parameters<ChannelEndpointParams>,
    ) -> Result<String, ErrorData> {
        let (channel_id, endpoint_id) = parse_ids(&channel_id, &endpoint_id)?;
        let peer = self
            .registry
            .peer_identity_for(channel_id, endpoint_id)
            .map_err(ChannelError::to_mcp_error)?;
        Ok(serde_json::json!({ "peer": { "name": peer.name, "model": peer.model } }).to_string())
    }

    #[tool(description = "Close a channel; returns { \"ok\": true }")]
    async fn channels_close(
        &self,
        Parameters(ChannelCloseParams { channel_id }): Parameters<ChannelCloseParams>,
    ) -> Result<String, ErrorData> {
        let channel_id = parse_channel_id(&channel_id)?;
        self.registry
            .close(channel_id)
            .map_err(ChannelError::to_mcp_error)?;
        Ok(serde_json::json!({ "ok": true }).to_string())
    }
}
