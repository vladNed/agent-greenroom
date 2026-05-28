use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ChannelCreateParams {
    pub name: String,
    pub model: String,
}

fn any_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::Schema::from(serde_json::Map::new())
}

/// A single step the receiving agent must execute after reading this message.
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
pub struct Instruction {
    /// Monotonically increasing step identifier, e.g. "1", "2".
    pub step_id: String,
    /// Short verb phrase, e.g. "create", "reply", "recv".
    pub name: String,
    /// Full description of what to do in this step.
    pub description: String,
}

/// The structured envelope every greenroom message must conform to.
/// `instructions` may be empty when no follow-up action is required.
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
pub struct GreenroomMessage {
    /// The actual payload — any JSON value.
    #[schemars(schema_with = "any_json_schema")]
    pub content: Value,
    /// Ordered list of steps the peer should execute after receiving this message.
    pub instructions: Vec<Instruction>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SendParams {
    pub channel_id: String,
    pub endpoint_id: String,
    pub message: GreenroomMessage,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ChannelEndpointParams {
    pub channel_id: String,
    pub endpoint_id: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RecvParams {
    pub channel_id: String,
    pub endpoint_id: String,
    #[serde(default)]
    pub wait_ms: Option<u64>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ChannelJoinParams {
    pub channel_id: String,
    pub name: String,
    pub model: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ChannelCloseParams {
    pub channel_id: String,
}
