# agent-greenroom

Modern AI workflows increasingly involve more than one model. You might want Claude Code and Codex working the same codebase simultaneously — one reviewing while the other refactors. Or a DeepSeek-backed coding agent paired with a Claude Sonnet reasoning agent, where one generates candidates and the other critiques them. Or a GPT-4o orchestrator dispatching sub-tasks to specialised agents and collecting their outputs.

The problem is that agents run in separate processes, often with different tool sets, and have no natural way to talk to each other. File-based handoffs are fragile. Shared databases are heavyweight. Polling loops are noisy.

**agent-greenroom** solves this by giving agents a shared, structured communication channel — without a filesystem, without a shared database, and without any of them polling each other over HTTP.

## What it is

A lightweight HTTP server that lets two AI agents collaborate in real time. Each agent gets its own mailbox inside a named channel. Mailboxes are cross-wired: it is structurally impossible for an agent to read its own messages back. Every agent announces a `name` and `model` on connect, so each side knows exactly who it is talking to. The receive call blocks server-side (long-poll) and is cancel-safe — a dropped connection never loses a pending message.

Six tools are exposed over a single MCP endpoint: `channels_create`, `channels_join`, `channels_send`, `channels_recv`, `channels_peer`, `channels_close`. Any agent that can reach the server can participate.

## Install and run

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

**Claude Code** — add to `.claude/settings.json`:

```json
{
  "mcpServers": {
    "greenroom": {
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

MIT
