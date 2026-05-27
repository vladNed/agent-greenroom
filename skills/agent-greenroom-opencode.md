# agent-greenroom — OpenCode instructions

agent-greenroom is a local MCP server providing mailbox-isolated channels for agent-to-agent communication. Start it with `grn` before using any tools.

The user always initiates the session. Channel pairing is manual today — the user shares the `channel_id` between the two agents. Do not guess or auto-discover channel IDs.

## MCP tools (registered at `http://127.0.0.1:7878/mcp`)

### `channels_create` — create a channel
Params: `name` (string), `model` (string)
Returns: `{ "channel_id": "<uuid>", "endpoint_id": "<uuid>" }`
Share `channel_id` with the other agent.

### `channels_join` — join an existing channel
Params: `channel_id` (string), `name` (string), `model` (string)
Returns: `{ "endpoint_id": "<uuid>", "peer": { "name": "...", "model": "..." } }`

### `channels_send` — send a message
Params: `channel_id` (string), `endpoint_id` (string), `message` (any JSON)
Returns: `{ "ok": true }`

### `channels_recv` — receive a message (long-poll)
Params: `channel_id` (string), `endpoint_id` (string), `wait_ms` (optional, default 50000)
Returns one of:
- `{ "message": <json> }` — received
- `{ "timed_out": true }` — retry
- `{ "closed": true }` — channel closed

### `channels_peer` — get peer identity
Params: `channel_id` (string), `endpoint_id` (string)

### `channels_close` — close the channel
Params: `channel_id` (string)

## Key rule
Each agent reads only messages sent by the other. You cannot read your own sent messages.

## Workflow
1. Agent A calls `channels_create`, shares `channel_id` with Agent B
2. Agent B calls `channels_join` with that `channel_id`
3. Both agents loop `channels_recv` to receive and `channels_send` to reply

## Conversation flow rules
- **On connect** (after `channels_create` or `channels_join`): ask the user whether to wait in `channels_recv` or to send the first instructions. Do not pick a side on your own.
- **After sending instructions that expect a reply**: immediately call `channels_recv` and loop on `timed_out` until a `message` or `closed` arrives.
- **After receiving and acting on a peer message**: return to `channels_recv` by default — stay in the receiver role until the user, the peer (`closed`), or an explicit hand-off says otherwise.
