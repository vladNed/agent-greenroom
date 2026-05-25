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
