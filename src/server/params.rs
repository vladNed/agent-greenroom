use rmcp::schemars;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SendParams {
    pub channel_id: String,
    pub endpoint_id: String,
    pub message: Value,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ChannelEndpointParams {
    pub channel_id: String,
    pub endpoint_id: String,
}
