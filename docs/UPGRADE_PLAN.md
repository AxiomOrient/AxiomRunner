# axiomAi Upgrade Plan

작성일: 2026-02-24
기준: 전체 파일 감사 + 제품 동작 검증 결과
현재 점수: **6.5 / 10** (빌드/테스트/CLI 동작, stub 미해소)
목표 점수: **9 / 10** (실제 프로덕션 서비스 가능)

---

## 현재 상태 요약

| 항목 | 상태 |
|------|------|
| Release 빌드 | PASS |
| 테스트 (49 suite) | PASS (0 failed) |
| CLI / doctor | PASS (6개 체크 all pass) |
| Clippy 경고 | 경고 13개 (dead_code, 스타일) |
| provider_model | `mock-local` — 실제 LLM 미연결 |
| memory_adapter | `enabled=false` — 환경변수 미설정 |
| tool_adapter | `enabled=false` — 환경변수 미설정 |
| 채널 어댑터 | Discord/Slack/IRC/Matrix/WhatsApp stub, Telegram semi-live |

---

## Phase A: 코드 정리 & 불용 파일 삭제 (즉시 — 1일)

목표: 저장소 노이즈 제거, 경고 해소, 감사 기반 정립.

### A-1: 불필요 문서 삭제

아래 파일들은 이미 완료된 계획/리포트이거나 활성 문서(AXIOM_COMPLETE_REVIEW.md)로 대체되었음.
삭제 전 `git rm` 로 이력 보존.

| 파일 | 삭제 사유 |
|------|-----------|
| `docs/REMEDIATION_PLAN.md` | 완료된 수정 계획 |
| `docs/IMPLEMENTATION_PLAN.md` | 완료된 구현 계획 |
| `docs/AXIOM_ANALYSIS_REPORT.md` | `AXIOM_COMPLETE_REVIEW.md`로 대체 |
| `docs/migration-h1-report.md` | 완료된 마이그레이션 리포트 |
| `docs/h2-parallel-verification.md` | 완료된 마이그레이션 리포트 |
| `docs/h3-rollback-recovery.md` | 완료된 마이그레이션 리포트 |
| `docs/h4-transition-rehearsal.md` | 완료된 마이그레이션 리포트 |
| `docs/g1-dev-mitigation-path-inventory.md` | 17줄, 실질 내용 없음 |
| `docs/transition-gates.md` | 완료된 전환 게이트 |

검증 기준: `ls docs/` 에서 위 파일 미존재 확인.

### A-2: Clippy 경고 해소

현재 경고 목록 (`cargo clippy 2>&1`):

| 경고 유형 | 위치 | 조치 |
|-----------|------|------|
| `method halt is never used` | `apps/src/estop.rs:14` | `#[allow(dead_code)]` 또는 daemon 연동 후 사용 (Phase C-2) |
| `this if statement can be collapsed` | `apps/src/gateway.rs:324`, `agent_loop.rs:148,252` | 중첩 if → `if let`/`&&` 통합 |
| `this impl can be derived` | `adapters/src/memory.rs:101`, `apps/src/integrations.rs:84` | `#[derive(Default)]` 추가 |
| `manual implementation of is_multiple_of` | `apps/src/agent_loop.rs:87`, `apps/src/agent_loop.rs:265` | `.is_multiple_of()` 교체 |
| `using write!() with newline` | `adapters/src/tool_memory.rs:120` | `writeln!()` 교체 |
| `calls to push immediately after creation` | `adapters/src/memory_sqlite.rs:235` | `vec![...]` 리터럴 교체 |
| `iter().copied().collect()` | `adapters/src/channel_telegram.rs:323` | `.to_vec()` 교체 |
| `if statement can be collapsed` | `adapters/src/memory_axiomme.rs:38`, `apps/src/gateway_signature.rs:45` | 통합 |

검증 기준: `cargo clippy 2>&1 | grep "^warning\[" | wc -l` = 0

### A-3: cargo-audit 취약점 확인

```bash
cargo install cargo-audit
cargo audit --manifest-path Cargo.toml
```

검증 기준: `Vulnerabilities found: 0` 또는 각 항목 RUSTSEC 번호 기록 후 업데이트 계획 수립.

---

## Phase B: 실제 어댑터 연결 (1주)

목표: doctor에서 `enabled=true`, 실제 I/O 동작 확인.

### B-1: Memory 어댑터 기본 활성화

**현재 동작**
- `apps/src/runtime_compose.rs:111` — `memory_path: env_path(ENV_RUNTIME_MEMORY_PATH)`
- `ENV_RUNTIME_MEMORY_PATH = "AXIOM_RUNTIME_MEMORY_PATH"` (runtime_compose.rs:13)
- 환경변수 미설정 시 `None` → `RuntimeComposeInitState::Disabled`

**구현 스펙**
- `runtime_compose.rs:111` 근처에 fallback 추가:
  ```rust
  memory_path: env_path(ENV_RUNTIME_MEMORY_PATH)
      .or_else(|| dirs::home_dir().map(|h| h.join(".axiom/memory.db"))),
  ```
- 기본 백엔드: SQLite (`adapters/src/memory_sqlite.rs`)
- 기본 경로: `~/.axiom/memory.db`
- `build_contract_memory` 호출 시 확장자 `.db` → `"sqlite"` 분기 필요 (`adapters/src/memory.rs:31-34`)

**검증 기준**
```
axiom doctor → memory_adapter: enabled=true path=~/.axiom/memory.db
```

### B-2: Tool 어댑터 기본 활성화

**현재 동작**
- `apps/src/runtime_compose.rs:112` — `tool_workspace: env_path(ENV_RUNTIME_TOOL_WORKSPACE)`
- `ENV_RUNTIME_TOOL_WORKSPACE = "AXIOM_RUNTIME_TOOL_WORKSPACE"` (runtime_compose.rs:14)
- 미설정 시 `None` → disabled

**구현 스펙**
- fallback 추가:
  ```rust
  tool_workspace: env_path(ENV_RUNTIME_TOOL_WORKSPACE)
      .or_else(|| dirs::home_dir().map(|h| h.join(".axiom/workspace"))),
  ```
- 디렉토리 자동 생성: `std::fs::create_dir_all`
- `adapters/src/tool.rs:140-165` `WorkspaceTool::new` 는 이미 canonicalize + 검증 포함

**검증 기준**
```
axiom doctor → tool_adapter: enabled=true workspace=~/.axiom/workspace/
```

### B-3: Telegram 실제 연결 확인

**현재 동작**
- `adapters/src/channel_telegram.rs:554줄` — Live 모드 코드 존재
- `send()` 메서드: offline 시 `VecDeque.push_back` (line ~191), online 시 `sendMessage` API 호출 (line ~209)
- `getUpdates` 폴링: line ~272 구현됨
- `channel.rs:507` 주석: "Does NOT start polling or make network calls" — 연결 레이어 없음

**구현 스펙**
- `AXIOM_CHANNEL_TELEGRAM_TOKEN` 환경변수 설정 경로 문서화
- doctor `channel` 체크에서 Telegram adapter health 반환 확인
- `channel_registry.rs` 에서 Telegram 어댑터 초기화 경로 점검

**검증 기준**
```
AXIOM_CHANNEL_TELEGRAM_TOKEN=xxx axiom doctor → channel: telegram enabled=true
```

### B-4: Provider OpenAI 실제 연결

**현재 동작**
- `adapters/src/provider_registry.rs:5` — `DEFAULT_PROVIDER_ID = "mock-local"`
- `adapters/src/provider_registry.rs:27-34` — `"openai"` 분기에서 `OPENAI_API_KEY` env var 사용
- `AXIOM_RUNTIME_PROVIDER = "AXIOM_RUNTIME_PROVIDER"` (runtime_compose.rs:16)

**구현 스펙**
- 환경변수 설정으로 즉시 활성화 가능:
  ```bash
  export OPENAI_API_KEY=sk-...
  export AXIOM_RUNTIME_PROVIDER=openai
  export AXIOM_RUNTIME_PROVIDER_MODEL=gpt-4o-mini
  ```
- `provider_registry.rs:27` 에서 `OPENAI_API_KEY` 사용 → 별도 `AXIOM_OPENAI_KEY` alias 추가 고려 (일관성)
- 필요 시 `openrouter` 분기도 동일 방식

**검증 기준**
```
axiom doctor → provider_model: openai/gpt-4o-mini
axiom run "hello" → 실제 LLM 응답 반환 (non-mock)
```

---

## Phase C: 핵심 기능 완성 (2주)

목표: 실제 메시지 흐름 동작 — 채널 입력 → 에이전트 처리 → 채널 출력.

### C-1: Channel Start 실제 폴링 루프 구현

**현재 동작**
- `apps/src/channel.rs:507` 주석: "Does NOT start polling or make network calls"
- `channel.rs:210-244` `ChannelAction::Start` 처리: `channel.running = true` 플래그만 설정
- 실제 네트워크 연결/폴링 코드 없음

**구현 스펙**
- `channel.rs` `Start` 핸들러에서 백그라운드 스레드 또는 async task 생성
- Telegram 폴링 루프 (`channel_telegram.rs:272` `getUpdates` 기반):
  ```rust
  // 의사 코드
  loop {
      let updates = adapter.poll_updates(offset)?;
      for update in updates {
          let msg = ChannelMessage::from_update(update);
          // 채널 수신 큐에 push
          inbox_tx.send(msg)?;
          offset = update.id + 1;
      }
      sleep(Duration::from_secs(1));
  }
  ```
- 수신 큐: `std::sync::mpsc` 또는 `tokio::sync::mpsc`
- EStop 연동: `estop.rs:14` `halt()` 시 루프 종료

**검증 기준**
- Telegram 봇에 메시지 전송 → `axiom channel list` 에서 수신 카운트 증가
- EStop 발생 시 폴링 루프 정상 종료

### C-2: Daemon → Channel 연동

**현재 동작**
- `apps/src/daemon.rs:88-115` `execute_daemon_run`: `DaemonLoop.tick()` 반복
- `apps/src/daemon_supervisor.rs:15` 에 `SupervisorComponentKind::Channels` 존재하나 실제 channel drain 없음
- `estop.rs:14` `halt()` 는 현재 daemon에서 미사용 (Clippy 경고 원인)

**구현 스펙**
- `DaemonLoop::tick` 에 channel inbox drain 추가:
  ```rust
  // daemon tick에서
  while let Ok(msg) = channel_inbox_rx.try_recv() {
      let response = agent.process(msg.body)?;
      channel_adapter.send(ChannelMessage::reply(msg, response))?;
  }
  ```
- `EStop::halt()` 를 daemon shutdown 경로에 연결 (dead_code 경고 해소)
- `DaemonRunInput` 에 channel inbox receiver 필드 추가

**검증 기준**
- Telegram 메시지 → daemon tick → agent 응답 → Telegram 송신 E2E 동작
- `cargo clippy` 에서 `halt is never used` 경고 제거

### C-3: RuntimeComposeState 책임 분리

**현재 동작**
- `apps/src/runtime_compose.rs:180-192` `RuntimeComposeState`: config, memory, provider, tool, bootstrap, channel, context 전부 보유
- 초기화(`new`)와 실행 상태가 단일 구조체에 혼재

**구현 스펙**
- `RuntimeComposeConfig` (입력 파라미터, line 29) / `RuntimeComposeState` (초기화 결과, line 180) 분리는 이미 됨
- 추가 분리 대상: 런타임 실행 핸들 (채널 inbox sender, EStop arc) 을 별도 `RuntimeHandle` 구조체로 추출
  ```rust
  pub struct RuntimeHandle {
      pub estop: Arc<EStop>,
      pub channel_inbox: Option<mpsc::Sender<ChannelMessage>>,
  }
  ```
- `RuntimeComposeState::activate() -> (RuntimeComposeState, RuntimeHandle)` 패턴

**검증 기준**
- 기존 49개 테스트 전부 통과
- `RuntimeHandle` 을 통해 daemon과 channel이 통신 가능

---

## Phase D: 프로덕션 준비 (1개월)

목표: 실제 배포 가능한 상태, 외부 모니터링 연동.

### D-1: 채널 어댑터 실구현 (Discord, Slack 우선)

**현재 동작**
- `adapters/src/channel_discord.rs:173줄` — `send()` line 74-77: `queue.push_back(message)` stub만 존재
- `adapters/src/channel_slack.rs:173줄` — 동일 패턴

**구현 스펙 (Discord)**
- Webhook 방식 (Bot Token 방식보다 단순, 읽기 불필요 시 충분):
  ```
  POST https://discord.com/api/webhooks/{webhook_id}/{webhook_token}
  Content-Type: application/json
  {"content": "<message>"}
  ```
- `DiscordConfig` 에 `webhook_url: Option<String>` 추가
- `send()` 에서 webhook URL 있으면 HTTP POST, 없으면 queue fallback 유지
- 환경변수: `AXIOM_CHANNEL_DISCORD_WEBHOOK`

**구현 스펙 (Slack)**
- Incoming Webhook 방식:
  ```
  POST https://hooks.slack.com/services/T.../B.../xxx
  Content-Type: application/json
  {"text": "<message>"}
  ```
- 환경변수: `AXIOM_CHANNEL_SLACK_WEBHOOK`

**우선순위 이유**: Webhook은 수신(polling) 불필요, send-only로 단계적 구현 가능.
IRC / Matrix / WhatsApp은 프로토콜 복잡도 높음 → Phase D 후반부.

**검증 기준**
- `AXIOM_CHANNEL_DISCORD_WEBHOOK=https://... axiom channel send discord "test"` → Discord 채널에 메시지 수신
- `cargo test` 49개 이상 통과 (신규 integration test 추가)

### D-2: 배포 문서 작성

**대상 파일**: `docs/DEPLOYMENT.md` (신규 작성)

**포함 내용**:
1. **필수 환경변수 가이드**
   ```
   AXIOM_RUNTIME_PROVIDER=openai
   OPENAI_API_KEY=sk-...
   AXIOM_RUNTIME_PROVIDER_MODEL=gpt-4o-mini
   AXIOM_RUNTIME_MEMORY_PATH=~/.axiom/memory.db   # 생략 시 자동 기본값
   AXIOM_RUNTIME_TOOL_WORKSPACE=~/.axiom/workspace # 생략 시 자동 기본값
   AXIOM_CHANNEL_TELEGRAM_TOKEN=xxx               # 선택
   AXIOM_CHANNEL_DISCORD_WEBHOOK=https://...      # 선택
   ```
2. **Docker 컨테이너 설정** — `Dockerfile` + `docker-compose.yml` 예시
3. **systemd 서비스 파일** — `axiom-daemon.service` 예시
4. **첫 실행 체크리스트**:
   - `axiom doctor` — 전 항목 green 확인
   - `axiom identity init` — bootstrap 완료
   - `axiom daemon start` — 데몬 기동

**검증 기준**: 문서만 따라 fresh 환경에서 `axiom doctor` all-green 재현 가능.

### D-3: 모니터링 연동 (metrics 외부 내보내기)

**현재 동작**
- `apps/src/metrics.rs:91` `render_dashboard()` — 내부 텍스트 포맷만 존재
- `MetricsSnapshot` 구조체: queue, lock, copy 지표 보유
- 외부 export 경로 없음

**구현 스펙**
- HTTP endpoint (최소 구현): `GET /metrics` → Prometheus text format
  ```
  axiom_queue_depth{} 0
  axiom_lock_wait_count{} 0
  axiom_copy_out_bytes{} 0
  ```
- 구현 위치: `apps/src/status.rs` 또는 신규 `apps/src/metrics_http.rs`
- 환경변수: `AXIOM_METRICS_PORT=9090` (미설정 시 disabled)
- 라이브러리 추가: `tiny_http` 또는 `axum` (이미 의존성 있으면 활용)

**검증 기준**
```bash
AXIOM_METRICS_PORT=9090 axiom daemon start &
curl http://localhost:9090/metrics | grep axiom_queue_depth
```

### D-4: cargo-audit CI 통합

**구현 스펙**
- GitHub Actions 또는 로컬 Makefile 타겟 추가:
  ```yaml
  # .github/workflows/audit.yml
  - name: Security audit
    run: cargo audit --deny warnings
  ```
- `Cargo.lock` 커밋 필수 (audit은 lock 파일 기반)
- 주간 스케줄 실행: `schedule: cron: '0 0 * * 1'`

**검증 기준**: CI audit 잡 green, `RUSTSEC` 취약점 0건.

---

## 우선순위 요약

```
즉시 (A)  →  1주 (B)  →  2주 (C)  →  1개월 (D)
  A-1          B-1          C-1          D-1
  A-2          B-2          C-2          D-2
  A-3          B-3          C-3          D-3
               B-4                       D-4
```

Phase B 완료 시점 예상 점수: **7.5 / 10**
Phase C 완료 시점 예상 점수: **8.5 / 10**
Phase D 완료 시점 예상 점수: **9.0 / 10**

---

## 핵심 파일 참조 색인

| 파일 | 관련 Phase | 주요 라인 |
|------|-----------|-----------|
| `apps/src/runtime_compose.rs` | B-1, B-2, C-3 | L13-22 (env vars), L111-112 (memory/tool), L180-192 (State struct) |
| `apps/src/channel.rs` | C-1 | L507 (stub 주석), L210-244 (Start 핸들러) |
| `apps/src/daemon.rs` | C-2 | L88-115 (execute_daemon_run), L345-349 (tick) |
| `apps/src/estop.rs` | C-2, A-2 | L14 (halt — dead_code 경고) |
| `adapters/src/provider_registry.rs` | B-4 | L5 (DEFAULT_PROVIDER_ID), L27-34 (openai 분기) |
| `adapters/src/channel_telegram.rs` | B-3, C-1 | L185-245 (send), L272 (getUpdates) |
| `adapters/src/channel_discord.rs` | D-1 | L74-77 (stub send) |
| `adapters/src/channel_slack.rs` | D-1 | L74-77 (stub send) |
| `adapters/src/memory.rs` | B-1, A-2 | L31-34 (backend 분기), L101 (derive 경고) |
| `adapters/src/tool.rs` | B-2 | L140-165 (WorkspaceTool::new) |
| `apps/src/metrics.rs` | D-3 | L91 (render_dashboard) |
| `apps/src/daemon_supervisor.rs` | C-2 | L15 (Channels component) |
