# axiomAi

**Single-binary, multi-channel AI agent framework built on event sourcing.**

Deploy a production-grade AI agent that responds to users across Telegram, Discord, Slack, IRC, Matrix, and WhatsApp — with persistent memory, tool execution, scheduled tasks, HMAC gateway signing, and TOTP authentication.

![Rust Edition](https://img.shields.io/badge/edition-2024-orange)
![License](https://img.shields.io/badge/license-MIT-blue)
![Build](https://img.shields.io/badge/build-passing-brightgreen)
![Tests](https://img.shields.io/badge/tests-500%2B%20passing-brightgreen)
![Clippy](https://img.shields.io/badge/clippy-0%20warnings-brightgreen)
![Audit](https://img.shields.io/badge/audit-0%20vulnerabilities-brightgreen)

---

## Features

| Category | Capability |
|---|---|
| **Agent** | Claude Sonnet 4 via coclai · RAG memory enrichment per turn · Skills registry |
| **Channels** | Telegram · Discord · Slack · IRC · Matrix · WhatsApp (6 adapters) |
| **Memory** | SQLite WAL (default) · Markdown · AxiomMe semantic indexing + BM25 RAG |
| **Tools** | Shell (allowlist) · FileRead · FileWrite · Memory · Composio · Delegate (depth 3) |
| **Security** | HMAC-SHA256 gateway · TOTP OTP gate · Shell metachar detection · no `sh -c` |
| **Ops** | Prometheus metrics · Daemon mode · Cron scheduler · systemd integration |
| **CLI** | 18 commands · Onboard wizard · Doctor · Migrate |
| **Architecture** | Pure event-sourcing core (zero I/O) · `unsafe_code = "forbid"` |

---

## Quick Start

```bash
# 1. Build the release binary
cargo build --release

# 2. Run system health check
./target/release/axiom_apps doctor

# 3. Initialize agent identity (first run only)
./target/release/axiom_apps onboard

# 4. Run the agent with mock provider (no API key required)
./target/release/axiom_apps agent

# 5. Run with a real LLM provider (OpenAI example)
AXIOM_RUNTIME_PROVIDER=openai \
OPENAI_API_KEY=sk-... \
./target/release/axiom_apps agent
```

---

## Architecture

```
User Input
    |
    v
[ Intent ]  ──>  [ Policy ]  ──>  [ Decision ]  ──>  [ Effect ]  ──>  [ Projection ]
                                                                            |
                                                                     Persistent State
```

```
axiomAi/
├── core/       Pure event sourcing pipeline — zero I/O dependencies
├── apps/       CLI · agent loop · daemon · channels · doctor · cron
├── adapters/   Channel adapters · Memory backends · Tools · Providers
├── infra/      Shared error types + retry policies
└── schema/     Data schema definitions
```

The `core/` crate has **zero I/O dependencies**. Every side effect — network calls, disk reads, LLM requests — is confined to `apps/` and `adapters/`. This boundary makes the business logic fully unit-testable without mocking I/O primitives. The agent loop ingests a user intent, runs it through the policy engine, receives a decision, executes effects (tool calls, memory writes, channel sends), and emits a projection back to the caller.

---

## Channels

Channel selection is always done via the `AXIOM_RUNTIME_CHANNEL` environment variable. The `channel serve` command reads this variable at startup.

### Telegram

Long-polling adapter. Offset is persisted to disk so no messages are replayed across restarts.

```bash
AXIOM_RUNTIME_CHANNEL=telegram \
AXIOM_TELEGRAM_BOT_TOKEN=<bot-token> \
./target/release/axiom_apps channel serve
```

Get a token from [@BotFather](https://t.me/BotFather).

### Discord

Send via webhook. Receive requires gateway events (not yet polled; see [Limitations](#limitations)).

```bash
AXIOM_RUNTIME_CHANNEL=discord \
AXIOM_DISCORD_BOT_TOKEN=<bot-token> \
AXIOM_CHANNEL_DISCORD_WEBHOOK=https://discord.com/api/webhooks/... \
./target/release/axiom_apps channel serve
```

### Slack

Send via Incoming Webhook. Receive requires Slack Event API (not yet polled; see [Limitations](#limitations)).

```bash
AXIOM_RUNTIME_CHANNEL=slack \
AXIOM_SLACK_BOT_TOKEN=xoxb-... \
AXIOM_CHANNEL_SLACK_WEBHOOK=https://hooks.slack.com/services/... \
./target/release/axiom_apps channel serve
```

### IRC

Raw TCP transport. Handles `PING`/`PONG` keep-alive automatically. No TLS, no SASL (see [Limitations](#limitations)).

```bash
AXIOM_RUNTIME_CHANNEL=irc \
AXIOM_IRC_SERVER=irc.libera.chat:6667 \
AXIOM_IRC_CHANNEL='#axiom' \
AXIOM_IRC_NICK=axiom-bot \
./target/release/axiom_apps channel serve
```

### Matrix

Polls `/_matrix/client/v3/sync`. The `next_batch` token is persisted across restarts.

```bash
AXIOM_RUNTIME_CHANNEL=matrix \
AXIOM_MATRIX_ACCESS_TOKEN=<access-token> \
AXIOM_MATRIX_HOMESERVER=https://matrix.org \
AXIOM_MATRIX_ROOM_ID='!abc123:matrix.org' \
./target/release/axiom_apps channel serve
```

### WhatsApp (send-only)

Sends via Meta Cloud API v17.0. Receiving messages requires a Meta webhook endpoint (platform limitation; see [Limitations](#limitations)).

```bash
AXIOM_RUNTIME_CHANNEL=whatsapp \
AXIOM_WHATSAPP_API_TOKEN=<api-token> \
AXIOM_WHATSAPP_PHONE_NUMBER_ID=<phone-number-id> \
./target/release/axiom_apps channel serve
```

### Daemon with Channel

Run a persistent daemon that polls a channel alongside scheduled work items:

```bash
AXIOM_RUNTIME_CHANNEL=telegram \
AXIOM_TELEGRAM_BOT_TOKEN=<token> \
AXIOM_METRICS_PORT=9090 \
AXIOM_DAEMON_IDLE_SECS=3600 \
./target/release/axiom_apps serve --mode=daemon
```

---

## Memory Backends

| Backend | Key variable | Description |
|---|---|---|
| `sqlite` (default) | `AXIOM_RUNTIME_MEMORY_PATH` | SQLite WAL mode — recommended for production. Default path: `~/.axiom/memory.db` |
| `markdown` | `AXIOM_RUNTIME_MEMORY_PATH` | Plain text files. Human-readable and diff-friendly |
| `axiomme` | `AXIOM_CONTEXT_ROOT` | AxiomMe semantic indexing with BM25 + semantic ranking. RAG enrichment per agent turn |

When `AXIOM_CONTEXT_ROOT` is set, the agent automatically retrieves relevant context from the AxiomMe store before each LLM turn. Unset this variable to disable RAG entirely.

---

## CLI Reference

### Global Options

```
axiom_apps [--config-file <path>] [--profile <name>] [--endpoint <url>] [--actor <id>] <command>
```

### Commands

| Command | Description |
|---|---|
| `onboard` | Interactive setup wizard (provider, memory backend, channels) |
| `agent` | Start an interactive multi-turn agent session |
| `agent -m <text>` | Single-turn agent call (alias: `--message`) |
| `agent --model <model>` | Override the model for this invocation |
| `read <key>` | Read a memory key in the current session |
| `write <key> <value>` | Write a memory key in the current session |
| `remove <key>` | Delete a memory key |
| `freeze` | Switch to read-only mode (EStop soft) |
| `halt` | Full stop — terminate agent and daemon (EStop hard) |
| `status` | Print runtime state summary |
| `health` | Quick reachability check |
| `doctor` | System health check across 6 components |
| `batch <intent>...` | Execute multiple intents in a single invocation |
| `cron list` | List all scheduled tasks |
| `cron add <expr> <intent>` | Add a cron task (standard cron expression) |
| `cron remove <id>` | Remove a scheduled task by ID |
| `service install` | Install axiomAi as a systemd service |
| `service start \| stop \| status \| uninstall` | Manage the systemd service lifecycle |
| `channel list` | List registered channel adapters |
| `channel add <type> <name>` | Register a new channel |
| `channel serve` | Start channel polling (reads `AXIOM_RUNTIME_CHANNEL`) |
| `channel doctor` | Diagnose channel adapter health |
| `channel remove <name>` | Remove a registered channel |
| `integrations list` | List all 23 catalog entries |
| `integrations info <name>` | Show details for a specific integration |
| `integrations install <name>` | Print installation instructions |
| `integrations remove <name>` | Remove an installed integration |
| `skills list` | List installed skills |
| `skills install <source>` | Install a skill from a git URL, path, or archive |
| `skills remove <name>` | Remove an installed skill |
| `migrate --legacy-root <path> --target-root <path> [--dry-run]` | Migrate data from a legacy installation |
| `serve --mode=gateway` | Start HTTP gateway mode for intent processing |
| `serve --mode=daemon` | Start daemon mode with optional channel polling |

---

## Session Isolation

Memory reads and writes are scoped to a single CLI invocation. Two separate calls do not share in-memory state:

```bash
# This does NOT work as expected — separate sessions
./target/release/axiom_apps write config prod
./target/release/axiom_apps read config
# output: value=<none>  (expected — session isolated)
```

Use `batch` to read and write within the same invocation:

```bash
# Correct — single invocation, shared state
./target/release/axiom_apps batch "write:config=prod" "read:config"
# output: read key=config value=prod
```

For persistent cross-session state, use the interactive `agent` session or run in `serve --mode=daemon`.

---

## Configuration

Priority order: **CLI flags > Environment variables > Config file**

### Core Runtime

| Variable | Default | Description |
|---|---|---|
| `AXIOM_PROFILE` | `prod` | Runtime profile name |
| `AXIOM_ENDPOINT` | `http://127.0.0.1:8080` | Gateway endpoint URL |
| `AXIOM_RUNTIME_PROVIDER` | `mock-local` | Provider ID (see AI Providers table below) |
| `AXIOM_RUNTIME_PROVIDER_MODEL` | `gpt-4o-mini` | Model name for the selected provider |
| `AXIOM_RUNTIME_MAX_TOKENS` | `4096` | Maximum response tokens |

### AI Providers

| Provider ID | Required variable | Notes |
|---|---|---|
| `mock-local` | — | No API key. For local testing only |
| `openai` | `OPENAI_API_KEY` | GPT-4o, GPT-4o-mini, o1, etc. Status: active |
| `openrouter` | `OPENROUTER_API_KEY` | 100+ models via one key |
| `anthropic` | `ANTHROPIC_API_KEY` | Anthropic-compatible endpoint |
| `deepseek` | `DEEPSEEK_API_KEY` | DeepSeek models |
| `groq` | `GROQ_API_KEY` | Groq inference endpoint |
| `mistral` | `MISTRAL_API_KEY` | Mistral models |
| `fireworks` | `FIREWORKS_API_KEY` | Fireworks AI |
| `together` | `TOGETHER_API_KEY` | Together AI |
| `perplexity` | `PERPLEXITY_API_KEY` | Perplexity models |
| `xai` | `XAI_API_KEY` | xAI Grok models |
| `moonshot` | `MOONSHOT_API_KEY` | Moonshot AI |
| `qwen` | `QWEN_API_KEY` | Alibaba Qwen models |
| `openai-compatible` | Custom | Any OpenAI-compatible endpoint |

> The agent itself always runs via `coclai` (Claude Sonnet 4). The provider config controls tool-log annotations and auxiliary model calls.

### Memory and Tools

| Variable | Default | Description |
|---|---|---|
| `AXIOM_RUNTIME_MEMORY_PATH` | `~/.axiom/memory.db` | SQLite or Markdown memory path |
| `AXIOM_RUNTIME_TOOL_WORKSPACE` | `~/.axiom/workspace/` | Tool execution workspace directory |
| `AXIOM_CONTEXT_ROOT` | — | AxiomMe RAG root directory. Unset disables RAG |
| `COMPOSIO_API_KEY` | — | Composio API key for the Composio tool adapter |
| `AXIOM_RUNTIME_TOOLS` | — | Comma-separated list of tools to activate |

### Channels

| Variable | Channel | Required |
|---|---|---|
| `AXIOM_RUNTIME_CHANNEL` | all (daemon) | Required to specify which channel to poll |
| `AXIOM_TELEGRAM_BOT_TOKEN` | Telegram | Yes |
| `AXIOM_DISCORD_BOT_TOKEN` | Discord | Yes |
| `AXIOM_CHANNEL_DISCORD_WEBHOOK` | Discord | Yes (for sending) |
| `AXIOM_DISCORD_GUILD_ID` | Discord | No |
| `AXIOM_SLACK_BOT_TOKEN` | Slack | Yes |
| `AXIOM_CHANNEL_SLACK_WEBHOOK` | Slack | Yes (for sending) |
| `AXIOM_SLACK_CHANNEL_ID` | Slack | No |
| `AXIOM_IRC_SERVER` | IRC | Yes (format: `host:port`) |
| `AXIOM_IRC_CHANNEL` | IRC | No |
| `AXIOM_IRC_NICK` | IRC | No (default: `axiom-bot`) |
| `AXIOM_MATRIX_ACCESS_TOKEN` | Matrix | Yes |
| `AXIOM_MATRIX_HOMESERVER` | Matrix | No (default: `https://matrix.org`) |
| `AXIOM_MATRIX_ROOM_ID` | Matrix | No |
| `AXIOM_WHATSAPP_API_TOKEN` | WhatsApp | Yes |
| `AXIOM_WHATSAPP_PHONE_NUMBER_ID` | WhatsApp | Yes |

### Security

| Variable | Description |
|---|---|
| `AXIOM_GATEWAY_SECRET` | HMAC-SHA256 signing secret for the HTTP gateway (opt-in) |
| `AXIOM_OTP_SECRET` | Base32-encoded TOTP secret, minimum 128 bits (opt-in) |
| `AXIOM_OTP_CODE` | 6-digit TOTP code. Required when `AXIOM_OTP_SECRET` is set |

### Daemon and Metrics

| Variable | Default | Description |
|---|---|---|
| `AXIOM_METRICS_PORT` | — | Prometheus metrics port (e.g. `9090`). Unset disables metrics |
| `AXIOM_DAEMON_MAX_TICKS` | `32` | Maximum daemon work iterations before exit |
| `AXIOM_DAEMON_IDLE_SECS` | — | Keep daemon alive N seconds after work completes |
| `AXIOM_DAEMON_WORK_ITEMS` | `startup-check` | Comma-separated work item IDs |

---

## Security

### HMAC Gateway Signatures

Set `AXIOM_GATEWAY_SECRET` to enable per-request HMAC-SHA256 request fingerprinting on the HTTP gateway. All requests without a valid signature return HTTP 401. Comparison uses constant-time XOR to prevent timing attacks.

```bash
export AXIOM_GATEWAY_SECRET=your-secret-here
./target/release/axiom_apps serve --mode=gateway
```

### TOTP OTP Gate

Set `AXIOM_OTP_SECRET` (Base32-encoded, minimum 128 bits) to require a valid 6-digit TOTP code before each `agent` invocation. Uses SHA1, 30-second window (RFC 6238 compatible).

```bash
export AXIOM_OTP_SECRET=JBSWY3DPEHPK3PXP  # example base32 secret
export AXIOM_OTP_CODE=$(oathtool --totp --base32 "$AXIOM_OTP_SECRET")
./target/release/axiom_apps agent
```

Any standard TOTP app (Google Authenticator, Authy, 1Password) works as the code source.

### Shell Execution Safety

The shell tool enforces three layers of protection:

1. **Metacharacter detection** — blocks `;`, `&&`, `||`, `|`, `` ` ``, `$()`, `>`, `<`, and other shell expansion characters
2. **Binary allowlist** (`ALLOWED_SHELL_PROGRAMS`) — only `ls`, `cat`, `grep`, `find`, `ps`, `curl`, `wget`, `jq`, `sed`, `awk`, `wc` and a small set of safe binaries are permitted
3. **Direct execution** — uses `Command::new(binary).args(...)` with no `sh -c` passthrough, eliminating shell injection entirely

---

## Monitoring

Start the Prometheus metrics endpoint alongside the daemon:

```bash
AXIOM_METRICS_PORT=9090 \
AXIOM_DAEMON_IDLE_SECS=3600 \
./target/release/axiom_apps serve --mode=daemon
```

Scrape endpoint: `GET http://localhost:9090/metrics`

```
# HELP axiom_queue_current_depth Current number of items in the work queue.
# TYPE axiom_queue_current_depth gauge
axiom_queue_current_depth 0

# HELP axiom_queue_peak_depth Peak number of items observed in the work queue.
# TYPE axiom_queue_peak_depth gauge
axiom_queue_peak_depth 2

# HELP axiom_lock_wait_count Total number of lock-wait events recorded.
# TYPE axiom_lock_wait_count counter
axiom_lock_wait_count 0

# HELP axiom_lock_wait_ns_total Total nanoseconds spent waiting for locks.
# TYPE axiom_lock_wait_ns_total counter
axiom_lock_wait_ns_total 0

# HELP axiom_copy_in_bytes_total Total bytes received across all channel inputs.
# TYPE axiom_copy_in_bytes_total counter
axiom_copy_in_bytes_total 1024

# HELP axiom_copy_out_bytes_total Total bytes sent across all channel outputs.
# TYPE axiom_copy_out_bytes_total counter
axiom_copy_out_bytes_total 2048
```

---

## Integrations

23 integrations across 4 categories:

| Category | Integrations |
|---|---|
| **Chat** (6) | telegram · discord · slack · matrix · whatsapp · irc |
| **AI Models** (13) | openai (active) · openrouter · anthropic · deepseek · groq · mistral · fireworks · together · perplexity · xai · moonshot · qwen · openai-compatible |
| **Platform** (3) | browser · composio · cron |
| **Productivity** (1) | github (coming soon) |

```bash
# List all integrations
./target/release/axiom_apps integrations list

# Show details for a specific integration
./target/release/axiom_apps integrations info telegram
./target/release/axiom_apps integrations info composio

# Show installation instructions
./target/release/axiom_apps integrations install openai
```

---

## Skills

Skills extend the agent with additional domain-specific capabilities. They are installed from git repositories, local paths, or archives and stored under `AXIOM_SKILLS_DIR` (default: `~/.axiom/skills/`).

```bash
# List installed skills
./target/release/axiom_apps skills list

# Install a skill from a git repository
./target/release/axiom_apps skills install https://github.com/your-org/axiom-skill-example

# Remove a skill
./target/release/axiom_apps skills remove skill-name
```

---

## Cron

Schedule recurring intents using standard cron expressions:

```bash
# Add a task that fires at 09:00 every day
./target/release/axiom_apps cron add "0 9 * * *" "write:daily_check=true"

# List all scheduled tasks
./target/release/axiom_apps cron list

# Remove a task by ID
./target/release/axiom_apps cron remove <id>
```

Cron tasks are stored persistently and survive restarts.

---

## Limitations

The following are known limitations as of v0.1.0:

- **Discord receive**: send-only via webhook. Receiving messages from Discord requires gateway polling, which is not yet implemented.
- **Slack receive**: send-only via Incoming Webhook. Receiving messages requires the Slack Event API, which is not yet polled.
- **WhatsApp receive**: send-only via Meta Cloud API v17.0. Receiving requires a Meta-registered webhook endpoint; polling is not supported by the platform.
- **IRC TLS**: the IRC adapter uses plain TCP. No TLS and no SASL authentication are supported in the current implementation.
- **Browser tool**: the browser adapter is a stub. Headless browser automation is listed in the integration catalog but real execution is not yet implemented.
- **Session isolation**: `read` and `write` CLI commands do not share state across separate invocations. Use `batch` or the interactive `agent` session for stateful workflows.

---

## Development

```bash
# Build release binary
cargo build --release

# Run all 500+ tests (all features enabled)
cargo test --workspace --all-features

# Lint (zero warnings policy)
cargo clippy --workspace -- -D warnings

# Security audit
cargo audit

# Build + test + clippy in one step
make check

# Build + runtime health check
make doctor

# Security scan only
make audit
```

### Makefile Targets

| Target | Description |
|---|---|
| `make build` | `cargo build --release` |
| `make test` | `cargo test --workspace --all-features` |
| `make clippy` | `cargo clippy --workspace -- -D warnings` |
| `make audit` | `cargo audit` |
| `make doctor` | Build release binary and run `axiom_apps doctor` |
| `make check` | Run clippy + full test suite |
| `make clean` | `cargo clean` |

---

## Doctor Output

`doctor` checks 6 components and reports `pass`, `info`, or `warn` for each. `info` items indicate optional features that are not configured; they do not block operation.

```
doctor ok=true profile=prod endpoint=http://127.0.0.1:8080 mode=active checks=6
doctor check=endpoint_scheme   level=pass
doctor check=runtime_mode      level=pass   detail=mode=active
doctor check=provider_model    level=pass   detail=provider_model=mock-local
doctor check=memory_adapter    level=info   detail=enabled=true
doctor check=tool_adapter      level=info   detail=enabled=true
doctor check=daemon_health     level=pass   detail=status=ok completed=1 failed=0
```

---

## Deployment

See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) for:

- Docker and docker-compose configuration
- systemd service setup (`service install` / `service start`)
- Complete environment variable reference (30+ variables)
- First-run checklist (build → doctor → onboard → serve)
- Security hardening guide

---

## License

MIT — see [LICENSE](LICENSE)

Copyright (c) 2026 axiomAi Contributors

---

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for the full release history.

**v0.1.0** (2026-02-25) — Initial release.
- Event sourcing framework with 6 channel adapters
- HMAC gateway, TOTP OTP gate, shell allowlist
- SQLite WAL + AxiomMe semantic memory + BM25 RAG
- Prometheus metrics, daemon mode, cron scheduler, 23-entry integrations catalog
- 563 tests, 0 failures · clippy 0 warnings · audit 0 vulnerabilities
