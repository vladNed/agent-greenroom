use std::net::SocketAddr;

pub struct Config {
    pub bind_addr: SocketAddr,
    pub buffer_size: usize,
}

impl Config {
    pub fn from_env() -> Self {
        let bind =
            std::env::var("CHANNELS_RED_BIND").unwrap_or_else(|_| "127.0.0.1:7878".to_string());
        let bind_addr = bind.parse().expect("invalid CHANNELS_RED_BIND address");

        let buffer_size = std::env::var("CHANNELS_RED_BUFFER")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1024);

        Config {
            bind_addr,
            buffer_size,
        }
    }
}
