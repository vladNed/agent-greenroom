# Agent Greenroom - Spec

## Intent

A registry of channels for agents to communicate using the channels in Rust.
This is the same principle as in Golang channels or asyncio Queue in Python.

Agents should never be able after sending a message, to read their own messages.
Channels have a mailbox system where to each participant a channel is assigned
at creation/joining

## Requirements

### R1: Channel mailbox

When a channel is created, it will contain two mailboxes.
Each mailbox will be further used to make agents only send and receive messages
from a mailbox.
This prevents the agent to read their own messages.

- A struct called Mailbox that contains a receive and sender (`rx`, `tx`) is needed
- The mailboxes will be added to the Channel struct

There should be a way for when an agent creates a channel, to know which
mailbox is for it.

### R2: An agent can join a channel

Given an already created channel, an agent can join the channel and receive an endpoint_id
that will give it the mailbox.

- The MCP server clearly has this configuration in `channles_join` tool

### R3: Agent has identity

Given a channel and two agents that connect to a channel, just like in browsers
each agent should have a fingerprint of its own so that it can be recognised.

An agent can be a different model, and CLI.

Examples:

- Claude Code
- OpenCode with DeepSeek model
- Codex with GPT-5.5

An identity is part of the protocol, it cointains the agent data + the endpoint
An identity is provided at creation and joining and is mainly used to keep a
room focused on one topic and agent conversation only.

## API

### Channels Create

Outcome:

- `channel_id`
- `endpoint_id`

### Channels Join

tool name: `channles_join`

payload mandatory field:

- `channel_id`

response:

- `endpoint_id`

### Channels Recv

tool name: `channels_recv`

payload:

- `channel_id` (required)
- `endpoint_id` (required)
- `wait_ms` (optional, default `50_000`, max `600_000`) — how long the server may
  block before returning. Used to keep the call under MCP client request
  timeouts; callers should loop on `timed_out` to wait indefinitely.

response (one of):

- `{ "message": <json> }` — a message was received.
- `{ "closed": true }` — the channel was closed.
- `{ "timed_out": true }` — `wait_ms` elapsed with no message. The receiver
  remains parked; the caller should issue another `channels_recv` to keep
  waiting.

#### Cancel-safety

`channels_recv` MUST be cancel-safe. If the MCP request future is dropped
mid-await (client disconnect, client-side timeout, task cancellation), the
underlying receiver is NOT lost: a subsequent `channels_recv` on the same
endpoint will succeed and continue receiving from the same queue.

`recv already in flight` is only returned when a concurrent `channels_recv`
for the same endpoint is genuinely still executing on the server.
