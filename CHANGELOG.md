# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.0.1] - 2026-02-25

### Added

#### Core Framework
- Event sourcing architecture: `Intent → Policy → Decision → Effect → Projection`
- Pure functional core (`core/`) — zero I/O dependencies
- Policy engine with freeze/halt EStop (read-only and full-stop modes)
- `batch` command for multi-intent processing in a single invocation

#### AI Agent
- Interactive agent loop powered by `coclai` (Claude Sonnet 4)
- RAG context enrichment via AxiomMe before each LLM turn
- Agent execution context with structured dependency injection
- Agent skills registry (git clone / link / copy install modes)

#### Multi-Channel Messaging
- **Telegram** — long-polling bot adapter with offset persistence
- **Discord** — webhook send + gateway receive adapter
- **Slack** — webhook send + Web API adapter
- **IRC** — TCP transport with PING/PONG keep-alive and PRIVMSG parsing
- **Matrix** — `/_matrix/client/v3/sync` polling with `next_batch` persistence
- **WhatsApp** — Meta Cloud API send adapter (webhook-only receive)
- Daemon channel polling thread integration (`AXONRUNNER_RUNTIME_CHANNEL`)

#### Memory & Storage
- SQLite memory backend with WAL mode and connection pooling
- Markdown memory backend for lightweight text-based storage
- AxiomMe semantic memory integration with BM25 + semantic ranking
- Hybrid memory adapter (SQLite primary + Markdown fallback)
- Per-key full-text term indexing (`memory_term_index`)

#### Security
- HMAC-SHA256 request fingerprinting for HTTP gateway (`AXONRUNNER_GATEWAY_SECRET`)
- Constant-time XOR comparison to prevent timing attacks
- TOTP/OTP gate for CLI agent access (`AXONRUNNER_OTP_SECRET`, SHA1, 30s window)
- Shell command allowlist + metacharacter detection (no `sh -c` injection)
- Token masking in error logs (Telegram, Composio, OpenAI)

#### Operations & Observability
- Prometheus metrics HTTP server (`AXONRUNNER_METRICS_PORT`)
  - `axonrunner_queue_current_depth`, `axonrunner_queue_peak_depth` (gauge)
  - `axonrunner_lock_wait_count`, `axonrunner_lock_wait_ns_total` (counter)
  - `axonrunner_copy_in_bytes_total`, `axonrunner_copy_out_bytes_total` (counter)
- Daemon mode (`serve --mode=daemon`) with health file and supervisor
- Gateway mode (`serve --mode=gateway`) for HTTP intent processing
- `doctor` command with 6 component health checks
- `AXONRUNNER_DAEMON_IDLE_SECS` for controlled daemon lifetime in test/dev environments

#### Integrations Catalog (23 entries)
- **Chat**: Telegram, Discord, Slack, Matrix, WhatsApp, IRC
- **AI Models**: OpenAI, OpenRouter, Anthropic, DeepSeek, Groq, Mistral, Fireworks, Together, Perplexity, XAI, Moonshot, Qwen, OpenAI-compatible
- **Platform**: Browser (headless automation), Composio (tool execution)
- **Productivity**: GitHub (coming soon)

#### Tool Execution
- Delegate tool adapter (recursive agent calls with depth tracking)
- Memory tool adapter (`memory.store`, `memory.recall`, `memory.forget`)
- Browser tool adapter (headless automation)
- Composio tool adapter (REST API, `COMPOSIO_API_KEY`)

#### CLI
- 18 top-level commands: `onboard`, `agent`, `read`, `write`, `remove`, `freeze`,
  `halt`, `status`, `health`, `doctor`, `batch`, `cron`, `service`, `channel`,
  `integrations`, `skills`, `migrate`, `serve`
- Global options: `--config-file`, `--profile`, `--endpoint`, `--actor`
- `onboard` wizard (interactive, channels-only, provider, memory backend selection)
- `migrate` tool for legacy data migration with dry-run support
- `service` subcommand for systemd integration (install/start/stop/uninstall)

#### Developer Experience
- `make check` — clippy + full test suite with `--all-features`
- `make doctor` — build + runtime health check
- `make audit` — `cargo audit` security scan
- GitHub Actions CI: security audit job + full test job
- 500+ unit and integration tests, 0 failures

### Technical
- Rust edition 2024, `unsafe_code = "forbid"`
- 75 source files, ~21,000 lines of Rust
- All channel features enabled by default in release build
- Workspace version synchronization across 5 crates
