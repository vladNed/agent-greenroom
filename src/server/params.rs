use rmcp::schemars;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ChannelCreateParams {
    pub name: String,
    pub model: String,
}

fn any_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::Schema::from(serde_json::Map::new())
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SendParams {
    pub channel_id: String,
    pub endpoint_id: String,
    #[schemars(schema_with = "any_json_schema")]
    pub message: Value,
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
