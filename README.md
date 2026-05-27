# agent-greenroom

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.80+-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-Enabled-orange.svg)](https://modelcontextprotocol.io)
[![Spec-Driven](https://img.shields.io/badge/Spec--Driven-Development-8B5CF6.svg)](spec/SPEC.md)
[![GitHub Release](https://img.shields.io/github/v/release/vladNed/agent-greenroom?include_prereleases)](https://github.com/vladNed/agent-greenroom/releases)
[![Tests](https://img.shields.io/github/actions/workflow/status/vladNed/agent-greenroom/rust.yml?label=tests)](https://github.com/vladNed/agent-greenroom/actions/workflows/rust.yml)

Modern AI workflows increasingly involve more than one model. You might want Claude Code and Codex working the same codebase simultaneously — one reviewing while the other refactors. Or a DeepSeek-backed coding agent paired with a Claude Sonnet reasoning agent, where one generates candidates and the other critiques them. Or a GPT-4o orchestrator dispatching sub-tasks to specialised agents and collecting their outputs.

The problem is that agents run in separate processes, often with different tool sets, and have no natural way to talk to each other. File-based handoffs are fragile. Shared databases are heavyweight. Polling loops are noisy.

**agent-greenroom** solves this by giving agents a shared, structured communication channel — without a filesystem, without a shared database, and without any of them polling each other over HTTP.

## What it is

A lightweight HTTP server that lets two AI agents collaborate in real time. Each agent gets its own mailbox inside a named channel. Mailboxes are cross-wired: it is structurally impossible for an agent to read its own messages back. Every agent announces a `name` and `model` on connect, so each side knows exactly who it is talking to. The receive call blocks server-side (long-poll) and is cancel-safe — a dropped connection never loses a pending message.

Six tools are exposed over a single MCP endpoint: `channels_create`, `channels_join`, `channels_send`, `channels_recv`, `channels_peer`, `channels_close`. Any agent that can reach the server can participate.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/vladNed/agent-greenroom/main/scripts/install.sh | sh
```

This installs the `grn` binary to `~/.local/bin` and registers the MCP server.

### From source (alternative)

**Prerequisites:** Rust toolchain (stable ≥ 1.80, edition 2024).

```bash
cargo build --release
./target/release/agent-greenroom
# listening on 127.0.0.1:7878
```

| Variable              | Default          | Description                              |
|-----------------------|------------------|------------------------------------------|
| `CHANNELS_RED_BIND`   | `127.0.0.1:7878` | Bind address for the HTTP server         |
| `CHANNELS_RED_BUFFER` | `1024`           | Per-channel message buffer depth         |

Point each agent at `http://127.0.0.1:7878/mcp`.

**Claude Code** — the installer automatically registers it. The config looks like this:

```json
{
  "mcpServers": {
    "agent-greenroom": {
      "type": "http",
      "url": "http://127.0.0.1:7878/mcp"
    }
  }
}
```

**Any Python agent:**

```python
from mcp import ClientSession
from mcp.client.streamable_http import streamablehttp_client

async with streamablehttp_client("http://127.0.0.1:7878/mcp") as (r, w, _):
    async with ClientSession(r, w) as session:
        await session.initialize()
        result = await session.call_tool("channels_create", {
            "name": "my-agent",
            "model": "gpt-4o"
        })
```

## Example scenarios

- **Claude Code + Codex 5.5 code-review loop** — one proposes changes, the other critiques, they iterate until the reviewer closes the channel.
- **DeepSeek generator + Claude Sonnet critic** — fast candidate generation paired with deep reasoning review, coordinated turn by turn.
- **GPT-4o orchestrator with specialised sub-agents** — orchestrator dispatches sub-tasks over separate channels and collects structured outputs.
- **Two Claude Code instances on the same repo** — one handles backend refactors while the other writes tests, coordinating on shared interfaces in real time.

---

Apache License 2.0

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting any Pull Requests. This project follows **Spec-Driven Development** — all changes must be preceded by updates to the specification.
