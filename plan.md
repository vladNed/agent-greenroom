# Plan: channels-red — MCP server for agent-to-agent channels

## Context

`channels-red` is a fresh Rust binary crate (edition 2024, no deps yet). The goal is an MCP server exposing four tools — `channels_create`, `channels_send`, `channels_recv`, `channels_close` — so multiple agents can pass JSON messages to each other without touching the filesystem.

Confirmed design choices (from clarifying questions):

- **Topology**: a *single shared* long-running MCP server. All agents connect to the same process. Streamable-HTTP transport is required because stdio gives every client its own process and they couldn't share state.
- **Send when no receiver**: messages are buffered FIFO in a bounded per-channel queue.
- **Recv**: blocks forever until a message arrives or the channel is closed. At most one in-flight `channels_recv` per channel; a concurrent second call errors.
- **Identity & payload**: `channels_create` returns a server-generated UUIDv4. Message payloads are arbitrary JSON values.

Defaults baked in (callable out if you want different):

- Bind `127.0.0.1:7878` (configurable via `CHANNELS_RED_BIND` env var). Localhost-only, no auth in v1.
- Per-channel buffer size = 1024 messages. `channels_send` returns an error if the buffer is full (no backpressure on the caller — fail-fast is friendlier for agents).
- Closing a channel while a recv is parked returns `{ "closed": true }` to that recv rather than an error.

## Architecture

One process, one tokio runtime, one Axum-backed HTTP server (provided by rmcp's `transport-streamable-http-server`). The MCP service struct holds an `Arc<Registry>`. The registry is a `Mutex<HashMap<Uuid, ChannelState>>`.

Each `ChannelState`:

```text
ChannelState {
    sender:   mpsc::Sender<serde_json::Value>,            // bounded(1024), Clone
    receiver: Arc<tokio::sync::Mutex<Option<Receiver>>>,  // take()'d while recv parked
}
```

Why this shape:

- `sender` is `Clone`, so `channels_send` clones it out of the registry under a brief lock and calls `try_send` *after* the registry lock is released — no global contention on the hot path.
- The receiver lives behind `Arc<Mutex<Option<_>>>`. `channels_recv` takes the `Option`'s contents; a second concurrent recv sees `None` and errors with `RecvAlreadyInFlight`. After the message is delivered, the receiver is put back so future recvs can use it.
- The `Arc` keeps the receiver alive even after `channels_close` removes the entry from the registry. The parked recv's `.await` resolves `None` (sender dropped) → translated to `{closed: true}`.

### Tools (rmcp macro-based router)

| Tool             | Input                                 | Output                                            |
|------------------|---------------------------------------|---------------------------------------------------|
| `channels_create`| `{}`                                  | `{ "channel_id": "<uuid>" }`                      |
| `channels_send`  | `{ "channel_id": "...", "message": <json> }` | `{ "ok": true }`                          |
| `channels_recv`  | `{ "channel_id": "..." }`             | `{ "message": <json> }` or `{ "closed": true }`   |
| `channels_close` | `{ "channel_id": "..." }`             | `{ "ok": true }`                                  |

Error cases (returned as tool errors, not panics):

- Unknown `channel_id` → `ChannelNotFound`
- Buffer full on send → `BufferFull`
- Concurrent recv → `RecvAlreadyInFlight`
- Invalid UUID string → `InvalidChannelId`

## Crates

To be added in `Cargo.toml` (let `cargo add` pin exact minor versions during impl):

```toml
[dependencies]
rmcp                  = { version = "*", features = ["server", "macros", "transport-streamable-http-server"] }
tokio                 = { version = "1", features = ["macros", "rt-multi-thread", "sync", "signal"] }
serde                 = { version = "1", features = ["derive"] }
serde_json            = "1"
schemars              = "0.8"
uuid                  = { version = "1", features = ["v4", "serde"] }
thiserror             = "1"
anyhow                = "1"
tracing               = "0.1"
tracing-subscriber    = { version = "0.3", features = ["env-filter"] }
```

`rmcp`'s feature flags shift between minor versions — during implementation, run `cargo add rmcp --features server,macros,transport-streamable-http-server` and adjust to whatever the resolver reports.

## File layout

```
src/
  main.rs        # bin entry: parse env, install tracing, start server, wait for Ctrl-C
  lib.rs         # pub mod registry; pub mod server; pub mod config;
  config.rs      # Config::from_env() → bind addr + buffer size
  registry.rs    # Registry, ChannelState, ChannelError (thiserror enum)
  server.rs      # ChannelsServer struct, #[tool_router] impl with the 4 tools
```

Keep the binary thin (`main.rs` < 60 lines). All real logic in `lib.rs` modules so it's unit-testable.

## Implementation notes per module

### `registry.rs`

- `Registry::new() -> Self`
- `Registry::create(&self, buffer: usize) -> Uuid`
- `Registry::sender_for(&self, id: Uuid) -> Result<Sender, ChannelError>` — clones out under lock
- `Registry::receiver_slot(&self, id: Uuid) -> Result<Arc<Mutex<Option<Receiver>>>, ChannelError>` — clones the Arc out under lock
- `Registry::close(&self, id: Uuid) -> Result<(), ChannelError>` — removes entry, dropping the sender
- `ChannelError` derives `thiserror::Error` and a helper `to_mcp_error()` that maps each variant to an MCP error code + human message.

### `server.rs`

- `ChannelsServer { registry: Arc<Registry>, buffer_size: usize }`
- `#[tool_router]` impl with the four `#[tool(...)]` methods.
- Each tool method calls into `Registry`, awaits as needed, and returns `Result<CallToolResult, McpError>`.
- `channels_recv` does:
  1. `slot = registry.receiver_slot(id)?`
  2. `let mut guard = slot.lock().await;`
  3. `let mut rx = guard.take().ok_or(RecvAlreadyInFlight)?;`
  4. `drop(guard);` ← release the slot lock so close can still proceed.
  5. `match rx.recv().await { Some(v) => { *slot.lock().await = Some(rx); reply with message } None => { reply with closed: true } }`

### `main.rs`

- Read `CHANNELS_RED_BIND` (default `127.0.0.1:7878`) and `CHANNELS_RED_BUFFER` (default `1024`).
- Init tracing.
- Build `ChannelsServer`, hand it to rmcp's streamable-HTTP server starter.
- Wait on `tokio::signal::ctrl_c()` and shut down cleanly.

## Verification

Run from the repo root:

1. **Build & lint**: `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt -- --check`.
2. **Unit tests** (in `registry.rs`): cover
   - create → send → recv round-trip
   - send to unknown channel returns `ChannelNotFound`
   - second recv while first is parked returns `RecvAlreadyInFlight`
   - close unblocks parked recv with `closed: true`
   - send when buffer is full returns `BufferFull` (set buffer size to 2 for the test)
   Run with `cargo test`.
3. **End-to-end smoke**:
   - `cargo run` in one terminal — server logs "listening on 127.0.0.1:7878".
   - From a second terminal, `curl -s -X POST http://127.0.0.1:7878/mcp -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'` should return the four tools.
   - With an MCP client (e.g. `mcp-cli` or a small Python script using the `mcp` SDK), exercise the happy path: `channels_create` → background `channels_recv` → `channels_send` → recv returns the message → `channels_close`.
4. **Manual concurrency check**: open two recv calls on the same channel from two clients; second must error. Close the channel while one recv is parked; that recv must resolve with `{closed: true}`.

## Out of scope for v1 (track as follow-ups)

- Auth / TLS — bind is localhost-only.
- Channel listing / introspection tool.
- Multi-consumer fan-out (broadcast) channels.
- Persistence across server restarts.
- Configurable per-channel buffer size at create time (currently global).
- Metrics / Prometheus endpoint.
