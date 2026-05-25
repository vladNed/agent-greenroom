---
description: Set up and use agent-greenroom channels for real-time agent-to-agent communication. Use when coordinating two AI agents via message passing, or when an agent needs to send/receive structured messages through a shared channel.
---

agent-greenroom is a local MCP server that provides mailbox-isolated channels between two agents. Each agent in a channel can only read messages sent by the other — never its own.

## Starting the server

```sh
grn
```

Runs on `http://127.0.0.1:7878`. Must be running before any MCP tools are used.

## MCP tools

### `channels_create`
Creates a new channel. Returns the `channel_id` to share with the other agent, and this agent's `endpoint_id`.

| param  | type   | notes |
|--------|--------|-------|
| `name`  | string | agent display name, e.g. `"claude-code"` |
| `model` | string | model identifier, e.g. `"claude-sonnet-4-6"` |

Response: `{ "channel_id": "<uuid>", "endpoint_id": "<uuid>" }`

### `channels_join`
Joins an existing channel using a `channel_id` shared by the creator.

| param        | type   | notes |
|--------------|--------|-------|
| `channel_id` | string | UUID from `channels_create` |
| `name`       | string | this agent's display name |
| `model`      | string | this agent's model |

Response: `{ "endpoint_id": "<uuid>", "peer": { "name": "...", "model": "..." } }`

### `channels_send`
Sends a JSON message to the channel. The message is delivered to the other agent's mailbox.

| param         | type | notes |
|---------------|------|-------|
| `channel_id`  | string | |
| `endpoint_id` | string | your own endpoint_id |
| `message`     | any JSON | structured payload to send |

Response: `{ "ok": true }`

### `channels_recv`
Long-polls for a message from the other agent. Loop on `timed_out` to keep waiting.

| param         | type   | notes |
|---------------|--------|-------|
| `channel_id`  | string | |
| `endpoint_id` | string | your own endpoint_id |
| `wait_ms`     | number | optional, default 50000, max 600000 |

Response (one of):
- `{ "message": <json> }` — message received
- `{ "timed_out": true }` — no message yet, call again
- `{ "closed": true }` — channel was closed

### `channels_peer`
Returns the identity of the other agent. Useful to confirm the peer has joined before sending.

Response: `{ "peer": { "name": "...", "model": "..." } }`

### `channels_close`
Closes the channel. Both agents will receive `{ "closed": true }` on the next recv.

## Typical two-agent workflow

**Agent A (creator):**
1. `channels_create` → save `channel_id` + `endpoint_id_A`
2. Share `channel_id` with Agent B (file, clipboard, etc.)
3. `channels_recv` (loop) to wait for B's first message
4. `channels_send` to reply

**Agent B (joiner):**
1. Receive `channel_id` from Agent A
2. `channels_join` → save `endpoint_id_B`
3. `channels_send` initial message
4. `channels_recv` (loop) to wait for replies

> Agents cannot read their own sent messages. Mailbox isolation is enforced by the server.
