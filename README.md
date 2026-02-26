# axiomAi

**Multi-channel AI agent framework built on event sourcing.**

Deploy an AI agent that talks to users across Discord, Slack, Telegram, IRC, Matrix, and WhatsApp — with persistent memory, tool execution, scheduled tasks, and production-grade security.

![Rust Edition](https://img.shields.io/badge/edition-2024-orange)
![License](https://img.shields.io/badge/license-MIT-blue)
![Build](https://img.shields.io/badge/build-passing-brightgreen)
![Tests](https://img.shields.io/badge/tests-500%2B%20passing-brightgreen)

---

## Features

| Category | Capability |
|----------|-----------|
| **Agent** | Claude Sonnet 4 via coclai · RAG memory enrichment · Skills |
| **Channels** | Telegram · Discord · Slack · IRC · Matrix · WhatsApp |
| **Memory** | SQLite (WAL) · Markdown · AxiomMe semantic indexing |
| **Tools** | Shell · File I/O · Browser automation · Composio |
| **Security** | HMAC-SHA256 gateway · TOTP OTP gate · Shell allowlist |
| **Ops** | Prometheus metrics · Daemon mode · Cron · systemd |
| **CLI** | 18 commands · Onboard wizard · Doctor · Migrate |

---

## Quick Start

```bash
# 1. Build
cargo build --release

# 2. Health check
./target/release/axiom_apps doctor

# 3. Run agent (mock provider, no API key needed)
./target/release/axiom_apps agent

# 4. Run with real LLM (OpenAI)
AXIOM_RUNTIME_PROVIDER=openai \
OPENAI_API_KEY=sk-... \
./target/release/axiom_apps agent

# 5. Start Telegram channel
AXIOM_TELEGRAM_BOT_TOKEN=<token> \
./target/release/axiom_apps channel serve telegram
```

---

## Architecture

```
Intent → Policy → Decision → Effect → Projection
```

```
axiomAi/
├── core/       Pure event sourcing pipeline (zero I/O)
├── apps/       CLI · agent loop · daemon · channels · doctor · cron
├── adapters/   Channel adapters · Memory backends · Tools · Providers
├── infra/      Shared utilities
└── schema/     Data schema definitions
```

The `core/` crate has **zero I/O dependencies**. All side effects are isolated in `apps/` and `adapters/`.

---

## Channels

### Telegram

```bash
AXIOM_TELEGRAM_BOT_TOKEN=<bot-token> \
./target/release/axiom_apps channel serve telegram
```

Get a token from [@BotFather](https://t.me/BotFather). Supports long-polling with offset persistence.

### Discord

```bash
AXIOM_DISCORD_BOT_TOKEN=<bot-token> \
AXIOM_CHANNEL_DISCORD_WEBHOOK=https://discord.com/api/webhooks/... \
./target/release/axiom_apps channel serve discord
```

Optional: `AXIOM_DISCORD_GUILD_ID` to restrict to a specific server.

### Slack

```bash
AXIOM_SLACK_BOT_TOKEN=xoxb-... \
AXIOM_CHANNEL_SLACK_WEBHOOK=https://hooks.slack.com/services/... \
./target/release/axiom_apps channel serve slack
```

Optional: `AXIOM_SLACK_CHANNEL_ID` to restrict to a specific channel.

### IRC

```bash
AXIOM_IRC_SERVER=irc.libera.chat:6667 \
AXIOM_IRC_CHANNEL=#axiom \
AXIOM_IRC_NICK=axiom-bot \
./target/release/axiom_apps channel serve irc
```

Uses raw TCP with automatic PING/PONG keep-alive.

### Matrix

```bash
AXIOM_MATRIX_ACCESS_TOKEN=<access-token> \
AXIOM_MATRIX_HOMESERVER=https://matrix.org \
AXIOM_MATRIX_ROOM_ID=!abc123:matrix.org \
./target/release/axiom_apps channel serve matrix
```

Uses `/_matrix/client/v3/sync` polling with `next_batch` persistence.

### WhatsApp (send-only)

```bash
AXIOM_WHATSAPP_API_TOKEN=<token> \
AXIOM_WHATSAPP_PHONE_NUMBER_ID=<phone-id> \
./target/release/axiom_apps channel serve whatsapp
```

Sends via Meta Cloud API v17.0. Receive requires webhook (platform limitation).

### Daemon with Channel

```bash
AXIOM_RUNTIME_CHANNEL=telegram \
AXIOM_TELEGRAM_BOT_TOKEN=<token> \
AXIOM_METRICS_PORT=9090 \
./target/release/axiom_apps serve --mode=daemon
```

---

## Memory Backends

| Backend | Env var override | Description |
|---------|-----------------|-------------|
| `sqlite` | `AXIOM_RUNTIME_MEMORY_PATH` | SQLite WAL — recommended for production |
| `markdown` | `AXIOM_RUNTIME_MEMORY_PATH` | Plain text, human-readable |
| `axiomme` | `AXIOM_CONTEXT_ROOT` | Semantic indexing + BM25 RAG |

Default: `~/.axiom/memory.db` (SQLite).

The agent automatically enriches each turn with memory context (RAG) when `AXIOM_CONTEXT_ROOT` is set.

---

## CLI Reference

### Global Options

```
axiom_apps [--config-file <path>] [--profile=<name>] [--endpoint=<url>] [--actor=<id>] <command>
```

### Commands

| Command | Description |
|---------|-------------|
| `onboard` | Interactive setup wizard (provider, memory, channels) |
| `agent` | Start interactive AI agent session |
| `agent --message/-m <text>` | Single-turn agent call |
| `doctor` | System health check (6 components) |
| `status` | Runtime state summary |
| `health` | Quick reachability check |
| `batch <intent>...` | Execute multiple commands in one invocation |
| `read <key>` | Read a memory key (current session) |
| `write <key> <value>` | Write a memory key (current session) |
| `remove <key>` | Delete a memory key |
| `freeze` | Switch to read-only mode (EStop soft) |
| `halt` | Full stop (EStop hard) |
| `cron list` | List scheduled tasks |
| `cron add <expr> <cmd>` | Add a cron task |
| `cron remove <id>` | Remove a cron task |
| `channel list` | List registered channels |
| `channel add <type> <name>` | Register a channel |
| `channel serve <name>` | Start channel polling |
| `channel doctor` | Diagnose channel adapters |
| `integrations info <name>` | Show integration details |
| `integrations list` | List all 23 integrations |
| `skills list` | List installed skills |
| `skills install <source>` | Install a skill |
| `service install\|start\|stop\|status\|uninstall` | Manage systemd service |
| `migrate --legacy-root <path> --target-root <path>` | Migrate from legacy format |
| `serve --mode=gateway\|daemon` | Start server mode |

### State & Memory

Facts (key-value data) are scoped to a single CLI invocation. Use `batch` to
write and read within the same call:

```bash
# Correct: write + read in one invocation
axiom_apps batch "write:config=prod" "read:config"
# output: read key=config value=prod

# Note: separate calls do not share state
axiom_apps write config "prod"   # effects=1
axiom_apps read config           # value=<none>  (expected — session isolated)
```

For persistent cross-session state, use the `agent` session or the daemon.

---

## Configuration

Priority: **CLI flags → Environment variables → Config file**

### Core Runtime

| Variable | Default | Description |
|----------|---------|-------------|
| `AXIOM_PROFILE` | `prod` | Runtime profile name |
| `AXIOM_ENDPOINT` | `http://127.0.0.1:8080` | Gateway endpoint |
| `AXIOM_RUNTIME_PROVIDER` | `mock-local` | Provider ID |
| `AXIOM_RUNTIME_PROVIDER_MODEL` | `gpt-4o-mini` | Model name |
| `AXIOM_RUNTIME_MAX_TOKENS` | `4096` | Max response tokens |

### AI Providers

| Provider ID | Required variable | Notes |
|------------|------------------|-------|
| `mock-local` | — | No API key, for testing |
| `openai` | `OPENAI_API_KEY` | GPT-4o, GPT-4o-mini, etc. |
| `openrouter` | `OPENROUTER_API_KEY` | 100+ models via one key |
| `anthropic` | via OpenRouter | Use `openrouter` provider |

> **Note:** The agent itself runs via `coclai` (Claude Sonnet 4). Provider config controls tool-log annotations.

### Memory & Tools

| Variable | Default | Description |
|----------|---------|-------------|
| `AXIOM_RUNTIME_MEMORY_PATH` | `~/.axiom/memory.db` | SQLite memory path |
| `AXIOM_RUNTIME_TOOL_WORKSPACE` | `~/.axiom/workspace` | Tool workspace directory |
| `AXIOM_CONTEXT_ROOT` | — | RAG context root (AxiomMe, disabled if unset) |
| `COMPOSIO_API_KEY` | — | Composio tool execution API key |

### Channels

| Variable | Channel | Required |
|----------|---------|---------|
| `AXIOM_TELEGRAM_BOT_TOKEN` | Telegram | Yes |
| `AXIOM_DISCORD_BOT_TOKEN` | Discord | Yes |
| `AXIOM_CHANNEL_DISCORD_WEBHOOK` | Discord | Yes (send) |
| `AXIOM_DISCORD_GUILD_ID` | Discord | No |
| `AXIOM_SLACK_BOT_TOKEN` | Slack | Yes |
| `AXIOM_CHANNEL_SLACK_WEBHOOK` | Slack | Yes (send) |
| `AXIOM_SLACK_CHANNEL_ID` | Slack | No |
| `AXIOM_IRC_SERVER` | IRC | Yes |
| `AXIOM_IRC_CHANNEL` | IRC | No |
| `AXIOM_IRC_NICK` | IRC | No (default: `axiom-bot`) |
| `AXIOM_MATRIX_ACCESS_TOKEN` | Matrix | Yes |
| `AXIOM_MATRIX_HOMESERVER` | Matrix | No (default: matrix.org) |
| `AXIOM_MATRIX_ROOM_ID` | Matrix | No |
| `AXIOM_WHATSAPP_API_TOKEN` | WhatsApp | Yes |
| `AXIOM_WHATSAPP_PHONE_NUMBER_ID` | WhatsApp | No |
| `AXIOM_RUNTIME_CHANNEL` | daemon | Channel ID for daemon mode |

### Security

| Variable | Description |
|----------|-------------|
| `AXIOM_GATEWAY_SECRET` | HMAC-SHA256 signing key for HTTP gateway (opt-in) |
| `AXIOM_OTP_SECRET` | Base32 TOTP secret for CLI 2FA, ≥128 bits (opt-in) |

When `AXIOM_OTP_SECRET` is set, the `agent` command requires:
```bash
AXIOM_OTP_CODE=123456 axiom_apps agent
```

### Daemon & Metrics

| Variable | Default | Description |
|----------|---------|-------------|
| `AXIOM_METRICS_PORT` | — | Prometheus metrics port (e.g. `9090`) |
| `AXIOM_DAEMON_MAX_TICKS` | `32` | Max daemon work iterations |
| `AXIOM_DAEMON_IDLE_SECS` | — | Keep daemon alive N seconds after work completes |
| `AXIOM_DAEMON_WORK_ITEMS` | `startup-check` | Comma-separated work item IDs |

---

## Security

### HMAC Gateway Signatures

Set `AXIOM_GATEWAY_SECRET` to enable per-request HMAC-SHA256 signing. All unsigned requests return HTTP 401. Uses constant-time XOR comparison to prevent timing attacks.

### TOTP OTP Gate

Set `AXIOM_OTP_SECRET` (Base32-encoded, ≥128 bits) to require a 6-digit TOTP code before each agent invocation:

```bash
export AXIOM_OTP_SECRET=<your-base32-secret>
export AXIOM_OTP_CODE=$(oathtool --totp --base32 $AXIOM_OTP_SECRET)
axiom_apps agent
```

### Shell Execution Safety

Shell tools enforce:
1. Metacharacter detection (blocks `;`, `&&`, `|`, `` ` ``, `$()`, etc.)
2. Binary allowlist (`ALLOWED_SHELL_PROGRAMS`)
3. Direct `Command::new(binary).args()` — no `sh -c` passthrough

---

## Monitoring

Start the Prometheus metrics server:

```bash
AXIOM_METRICS_PORT=9090 \
AXIOM_DAEMON_IDLE_SECS=3600 \
./target/release/axiom_apps serve --mode=daemon
```

Scrape at `http://localhost:9090/metrics`:

```
# HELP axiom_queue_current_depth Current number of items in the work queue.
# TYPE axiom_queue_current_depth gauge
axiom_queue_current_depth 0

# HELP axiom_lock_wait_count Total number of lock-wait events recorded.
# TYPE axiom_lock_wait_count counter
axiom_lock_wait_count 0
```

Available metrics: `axiom_queue_current_depth`, `axiom_queue_peak_depth`, `axiom_lock_wait_count`, `axiom_lock_wait_ns_total`, `axiom_copy_in_bytes_total`, `axiom_copy_out_bytes_total`

---

## Integrations

```bash
# List all 23 integrations
axiom_apps integrations list

# Get details
axiom_apps integrations info telegram
axiom_apps integrations info composio
```

Categories: Chat (6) · AI Models (14) · Platform (2) · Productivity (1 coming soon)

---

## Skills

Skills extend the agent with additional capabilities:

```bash
# List installed skills
axiom_apps skills list

# Install from a source
axiom_apps skills install https://github.com/your-org/axiom-skill-example

# Remove
axiom_apps skills remove skill-name
```

Skills are stored in `AXIOM_SKILLS_DIR` (default: `~/.axiom/skills`).

---

## Cron

```bash
# Add a scheduled task (cron expression + intent spec)
axiom_apps cron add "0 9 * * *" "write:daily_check=true"

# List tasks
axiom_apps cron list

# Remove
axiom_apps cron remove <id>
```

---

## Migration

Migrate data from a legacy axiomAi installation:

```bash
# Dry run first
axiom_apps migrate \
  --legacy-root ~/.axiom-old \
  --target-root ~/.axiom \
  --dry-run

# Apply
axiom_apps migrate \
  --legacy-root ~/.axiom-old \
  --target-root ~/.axiom
```

---

## Development

```bash
cargo test --workspace --all-features   # Run all 500+ tests
cargo clippy --workspace -- -D warnings # Lint
make check                              # Build + test + clippy
make audit                              # cargo audit security scan
make doctor                             # Build + runtime health check
```

### Doctor Output

```
doctor ok=true profile=prod endpoint=http://127.0.0.1:8080 mode=active checks=6
doctor check=endpoint_scheme   level=pass
doctor check=runtime_mode      level=pass   detail=mode=active
doctor check=provider_model    level=pass   detail=provider_model=mock-local
doctor check=memory_adapter    level=info   detail=enabled=true
doctor check=tool_adapter      level=info   detail=enabled=true
doctor check=daemon_health     level=pass   detail=status=ok completed=1 failed=0
```

`info` items are optional features. They do not block operation.

---

## Deployment

See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) for:
- Docker and docker-compose setup
- systemd service configuration
- Environment variable reference (30+ variables)
- First-run checklist
- Security hardening guide

---

## License

MIT — see [LICENSE](LICENSE)

## Changelog

See [CHANGELOG.md](CHANGELOG.md)
