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

---

# Installation

## Requirements

### IR1: Zero prerequisites beyond Claude Code

A user who has only Claude Code installed must be able to install agent-greenroom
with a single command. No Rust toolchain, no git, no manual PATH editing, no
secondary package manager required.

### IR2: Single-command install

Installation is triggered by one shell command, e.g.:

```sh
curl -fsSL https://raw.githubusercontent.com/<owner>/agent-greenroom/main/install.sh | sh
```

### IR3: Binary placed on PATH

The `agent-greenroom` binary must be available as a command after install
without the user modifying their shell config manually. The installer places
the binary in `~/.local/bin/` and appends the necessary export to
`.bashrc`/`.zshrc` if that directory is not already on `$PATH`.

### IR4: Skill auto-installed

The Claude Code skill file is downloaded and placed in the correct Claude Code
skills directory automatically. The user must not copy any files manually.

### IR5: MCP server auto-registered

The installer patches `~/.claude/settings.json` to register the MCP server
under `mcpServers` so Claude Code can start it without manual configuration.

### IR6: Cross-platform binaries

Prebuilt binaries must be available for:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`

## Design

### Distribution: GitHub Releases + `install.sh`

Prebuilt binaries are uploaded to a **draft** GitHub Release on every version tag
via a GitHub Actions workflow. The release is published manually (or via a follow-up job)
after all 4 matrix jobs complete successfully. This avoids the "immutable release"
error when multiple jobs try to upload assets concurrently.

`install.sh` performs four steps in order:

1. **Detect platform** — determine OS and CPU architecture; select the correct
   binary asset from the latest GitHub Release.
2. **Download binary** — fetch the binary and place it at
   `~/.local/bin/agent-greenroom`; ensure `~/.local/bin` is on `$PATH`.
3. **Download skill file** — fetch `skills/agent-greenroom.md` from the release
   assets and place it in the Claude Code skills directory.
4. **Register MCP server** — patch `~/.claude/settings.json` to add
   `agent-greenroom` under `mcpServers`, pointing to the installed binary.

The binary itself requires no changes — it already exposes a single runnable
server process. No subcommands are added solely for installation.

### Artifacts required

| Artifact | Purpose |
|---|---|
| `.github/workflows/release.yml` | Cross-compile matrix, upload to GitHub Release assets on tag push |
| `install.sh` | Zero-prereq installer (~80 lines) |
| `skills/agent-greenroom.md` | Claude Code skill definition shipped alongside the binary |

### Secondary install path

Once binaries exist on GitHub Releases, `cargo binstall agent-greenroom`
works automatically for users who already have Rust. Document in README only;
not a primary path.

## Acceptance Criteria

- [ ] Running the install command on a machine with only Claude Code (no Rust,
      no git) completes without error.
- [ ] After install, `agent-greenroom` runs and prints a listening address
      without any extra setup.
- [ ] Claude Code lists the skill without the user having placed any files
      manually.
- [ ] Claude Code can connect to the MCP server without the user editing any
      config file.
- [ ] The installer is idempotent: running it a second time updates the binary
      and skill file without corrupting settings.
- [ ] All four platform targets (Linux x86\_64, Linux arm64, macOS x86\_64,
      macOS arm64) have a corresponding binary on each GitHub Release.
- [ ] The install script fails fast and prints a clear error if the platform is
      unsupported or the network is unavailable.
