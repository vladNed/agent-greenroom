use std::sync::Arc;

use anyhow::Result;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};

use agent_greenroom::{config::Config, registry::Registry, server::ChannelsServer};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env();
    let registry = Arc::new(Registry::new());
    let buffer_size = config.buffer_size;

    let service = StreamableHttpService::new(
        {
            let registry = registry.clone();
            move || Ok(ChannelsServer::new(registry.clone(), buffer_size))
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;

    tracing::info!("listening on {}", config.bind_addr);

    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;

    Ok(())
}
