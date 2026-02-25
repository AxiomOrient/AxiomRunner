# axiomAi

Event-sourcing AI agent framework with multi-channel messaging support.

![Rust Edition](https://img.shields.io/badge/edition-2024-orange)
![Build](https://img.shields.io/badge/build-passing-brightgreen)

---

## Features

- **Multi-channel messaging** — Discord, Slack, Telegram, IRC, Matrix, WhatsApp adapters
- **Event sourcing core** — Pure `Intent → Policy → Decision → Effect → Projection` pipeline, no I/O in `core/`
- **SQLite memory backend** — Persistent semantic memory with configurable path
- **Agent loop with RAG** — Context enrichment via AxiomMe RAG before each LLM call
- **Daemon mode** — Background polling with Prometheus metrics export
- **Emergency stop (EStop)** — `freeze` / `halt` intents for graceful shutdown
- **OTP / TOTP gate** — Optional CLI 2FA with `AXIOM_OTP_SECRET`
- **CLI-first design** — Single binary with structured subcommands

---

## Quick Start

```bash
# Build release binary
cargo build --release

# Or use make
make build

# System health check
./target/release/axiom_apps doctor

# Initialize identity (first run only)
./target/release/axiom_apps onboard

# Single agent call (mock-local provider by default)
./target/release/axiom_apps agent run

# Run with OpenAI
AXIOM_RUNTIME_PROVIDER=openai \
OPENAI_API_KEY=sk-... \
./target/release/axiom_apps agent run

# Run Telegram channel polling (foreground)
AXIOM_CHANNEL_TELEGRAM_TOKEN=<token> \
./target/release/axiom_apps channel serve telegram
```

---

## Architecture

```
axiomAi/
├── core/       Pure event sourcing — Intent → Policy → Decision → Effect → Projection (no I/O)
├── apps/       CLI binary, agent loop, daemon, channel serve, doctor, cron, service
├── adapters/   Channel adapters | Memory (SQLite/Markdown) | Tools | Providers
├── infra/      Infrastructure utilities (hex, env, time, parse helpers)
└── schema/     Shared data schema definitions
```

The `core/` crate has zero I/O dependencies. All side effects are handled in `apps/` and `adapters/`.

---

## CLI Commands

| Command | Description |
|---------|-------------|
| `onboard` | Initialize agent identity (run once) |
| `doctor` | System health check — provider, memory, tools, channels |
| `status` | Current agent state summary |
| `agent run` | Start agent loop |
| `channel list` | List registered channels and their status |
| `channel add <type> <name>` | Register a new channel |
| `channel serve <name>` | Start channel polling in foreground |
| `channel doctor` | Diagnose channel adapters |
| `cron list` | List scheduled cron tasks |
| `cron add <expr> <cmd>` | Add a cron task |
| `service install\|start\|stop\|status` | Manage system service |
| `skills list\|install\|remove` | Manage agent skills |
| `serve --mode=gateway\|daemon` | Start gateway or daemon server |
| `read <key>` | Read a memory key |
| `write <key> <value>` | Write a memory key |
| `freeze` | Suspend agent (EStop soft) |
| `halt` | Stop agent (EStop hard) |

---

## Configuration

Configuration priority: CLI flags → Environment variables → Config file.

### Core Runtime

| Variable | Default | Description |
|----------|---------|-------------|
| `AXIOM_PROFILE` | `prod` | Runtime profile name |
| `AXIOM_ENDPOINT` | `http://127.0.0.1:8080` | Gateway endpoint URL |
| `AXIOM_RUNTIME_PROVIDER` | `mock-local` | Provider ID (`mock-local` / `openai` / `openrouter`) |
| `AXIOM_RUNTIME_PROVIDER_MODEL` | `gpt-4o-mini` | Model name |
| `AXIOM_RUNTIME_MAX_TOKENS` | `4096` | Max response tokens |

### Provider API Keys

| Variable | Required when |
|----------|--------------|
| `OPENAI_API_KEY` | `AXIOM_RUNTIME_PROVIDER=openai` |
| `OPENROUTER_API_KEY` | `AXIOM_RUNTIME_PROVIDER=openrouter` |

### Memory and Tools

| Variable | Default | Description |
|----------|---------|-------------|
| `AXIOM_RUNTIME_MEMORY_PATH` | `~/.axiom/memory.db` | SQLite memory database path |
| `AXIOM_RUNTIME_TOOL_WORKSPACE` | `~/.axiom/workspace` | Tool workspace directory |
| `AXIOM_CONTEXT_ROOT` | — | RAG context root (disabled if unset) |

### Channel Configuration

| Channel | Environment Variable |
|---------|---------------------|
| Telegram | `AXIOM_CHANNEL_TELEGRAM_TOKEN` |
| Discord | `AXIOM_DISCORD_BOT_TOKEN` + `AXIOM_CHANNEL_DISCORD_WEBHOOK` |
| Slack | `AXIOM_SLACK_BOT_TOKEN` + `AXIOM_CHANNEL_SLACK_WEBHOOK` |
| IRC | `AXIOM_IRC_SERVER`, `AXIOM_IRC_CHANNEL`, `AXIOM_IRC_NICK` |

Daemon channel polling:
```bash
AXIOM_RUNTIME_CHANNEL=discord \
AXIOM_DISCORD_BOT_TOKEN=<token> \
AXIOM_CHANNEL_DISCORD_WEBHOOK=https://discord.com/api/webhooks/... \
./target/release/axiom_apps serve --mode=daemon
```

### Security

| Variable | Description |
|----------|-------------|
| `AXIOM_GATEWAY_SECRET` | HMAC signing secret for HTTP gateway (disabled if unset) |
| `AXIOM_OTP_SECRET` | Base32 TOTP secret for CLI 2FA (disabled if unset) |
| `AXIOM_METRICS_PORT` | Prometheus metrics HTTP port (e.g. `9090`, disabled if unset) |

---

## Development

```bash
cargo test --workspace        # Run all tests
cargo clippy --workspace      # Lint
make check                    # Build + test + clippy
make audit                    # Security audit (cargo-audit)
```

### Doctor Output Example

```
profile     : prod
endpoint    : http://127.0.0.1:8080

[pass] provider        mock-local (mock)
[pass] memory          enabled — ~/.axiom/memory.db
[pass] tool            enabled — ~/.axiom/workspace
[info] context         not configured (RAG disabled)
[warn] gateway-secret  AXIOM_GATEWAY_SECRET not set — HTTP signatures disabled
[warn] otp-secret      AXIOM_OTP_SECRET not set — CLI OTP disabled
```

`warn` items indicate optional features. They do not block operation.

---

## Deployment

See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) for the full deployment guide including Docker, systemd, and security hardening.
