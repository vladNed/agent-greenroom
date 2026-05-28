---
description: Set up and use agent-greenroom channels for real-time agent-to-agent communication. Use when the user asks to connect two AI agents, create or join a channel, or send/receive structured messages through a shared channel. The user drives the session: they trigger create/join and manually share the `channel_id` between agents.
---

agent-greenroom is a local MCP server that provides mailbox-isolated channels between two agents. Each agent in a channel can only read messages sent by the other — never its own.

The user always initiates the session. The only way to connect two agents today is manual: the user shares the `channel_id` from one agent to the other. Do not attempt to auto-discover or guess channel IDs.

## Starting the server

```sh
grn
```

Runs on `http://127.0.0.1:7878`. Must be running before any MCP tools are used.

## Message schema

Every `channels_send` call **must** use this exact shape for `message`:

```json
{
  "content": <any JSON value>,
  "instructions": [
    { "step_id": "1", "name": "<verb>", "description": "<what to do>" },
    { "step_id": "2", "name": "<verb>", "description": "<what to do>" }
  ]
}
```

`instructions` is an ordered list of steps the **receiving** agent must execute after reading the message. It **may be empty** (`[]`) when no follow-up action is needed. Never omit the field.

### When to include instructions

| Situation | Instructions to include |
|-----------|------------------------|
| Sending a request (you want the peer to do work and reply) | 1. execute the task · 2. send result back via `channels_send` · 3. call `channels_recv` to await your next message |
| Sending a response (you just completed a task) | 1. call `channels_recv` to await the next instruction |
| Sending a final result / closing | 1. call `channels_close` — work is done |
| Dialog turn (you need the peer's input before continuing) | 1. respond with your answer via `channels_send` · 2. call `channels_recv` |

### Examples

**Request (Agent A asking Agent B to create a poem):**
```json
{
  "content": { "request": "Write a haiku about autumn." },
  "instructions": [
    { "step_id": "1", "name": "create", "description": "Write the haiku." },
    { "step_id": "2", "name": "reply", "description": "Send the haiku back via channels_send." },
    { "step_id": "3", "name": "recv", "description": "Then call channels_recv to await the next instruction." }
  ]
}
```

**Response (Agent B sending the poem back):**
```json
{
  "content": { "poem": "Leaves fall silently / Whispering through empty air / Winter waits below" },
  "instructions": [
    { "step_id": "1", "name": "recv", "description": "Call channels_recv to await your next instruction." }
  ]
}
```

**Final / close:**
```json
{
  "content": { "status": "done", "result": "..." },
  "instructions": [
    { "step_id": "1", "name": "close", "description": "Call channels_close — all work is complete." }
  ]
}
```

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
Sends a `GreenroomMessage` to the channel. The `message` field **must** match the schema above.

| param         | type | notes |
|---------------|------|-------|
| `channel_id`  | string | |
| `endpoint_id` | string | your own endpoint_id |
| `message`     | GreenroomMessage | `{ "content": <json>, "instructions": [...] }` |

Response: `{ "ok": true }`

### `channels_recv`
Long-polls for a message from the other agent. Loop on `timed_out` to keep waiting.

| param         | type   | notes |
|---------------|--------|-------|
| `channel_id`  | string | |
| `endpoint_id` | string | your own endpoint_id |
| `wait_ms`     | number | optional, default 50000, max 600000 |

Response (one of):
- `{ "message": { "content": <json>, "instructions": [...] } }` — message received; read `instructions` and execute each step in order
- `{ "timed_out": true }` — no message yet, call again
- `{ "closed": true }` — channel was closed

**When you receive a message: execute the instructions in `message.instructions` in order without asking the user for guidance. The instructions are authoritative — they tell you exactly what to do next.**

### `channels_peer`
Returns the identity of the other agent. Useful to confirm the peer has joined before sending.

Response: `{ "peer": { "name": "...", "model": "..." } }`

### `channels_close`
Closes the channel. Both agents will receive `{ "closed": true }` on the next recv.

## Typical two-agent workflow

**Agent A (creator):**
1. `channels_create` → save `channel_id` + `endpoint_id_A`
2. Share `channel_id` with Agent B (file, clipboard, etc.)
3. `channels_recv` (loop) to wait for B's first message, OR send the first request with instructions
4. On each received message: read `message.instructions` and execute the steps

**Agent B (joiner):**
1. Receive `channel_id` from Agent A
2. `channels_join` → save `endpoint_id_B`
3. `channels_recv` to wait for A's first message (or send a greeting with instructions)
4. On each received message: execute `message.instructions` in order — no user input needed

> Agents cannot read their own sent messages. Mailbox isolation is enforced by the server.

## Conversation flow rules

These rules govern how to behave on a live channel. Apply them automatically — do not ask the user again on every turn.

- **On connect (after `channels_create` or `channels_join`):** stop and ask the user whether to wait in `channels_recv` for the peer or to send the first instructions. Do not pick a side on your own — the user decides the role for this session.
- **After sending a request that expects a reply:** immediately call `channels_recv` and loop on `timed_out` until a `message` or `closed` arrives. A send that needs a response is not complete until the response is back.
- **After receiving a message:** execute `message.instructions` in order. Do not ask the user what to do — the instructions are your guide. If a step says "call channels_recv", do it. If a step says "call channels_close", do it.
- **After completing all instructions:** return to `channels_recv` by default unless the last instruction was `close`. Stay in the receiver loop until the user, the peer (`closed`), or an explicit hand-off says otherwise.
- **Always include instructions in every `channels_send` call.** The peer depends on them to know what to do next without user input.
