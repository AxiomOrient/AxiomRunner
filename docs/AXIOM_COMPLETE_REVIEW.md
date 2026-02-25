# axiomAi 종합 코드 리뷰 및 분석 보고서

작성일: 2026-02-23
리뷰어: Claude Sonnet 4.6 (Rust 전문가 코드 리뷰)
대상 커밋: HEAD (전체 소스 트리 완전 분석)

---

## 1. 프로젝트 전체 구조

### 1.1 워크스페이스 개요

```
axiomAi/
├── core/         순수 도메인 로직 (no I/O, no external deps)
├── schema/       타입 스키마 및 호환성 계층
├── adapters/     외부 시스템 어댑터 (채널, 메모리, 런타임, 도구)
├── infra/        인프라 유틸리티
└── apps/         실행 바이너리 및 CLI 오케스트레이터
```

axiomAi는 **Event Sourcing 아키텍처**를 채택한 Rust 워크스페이스입니다. ZeroClaw에서 파생되어 "zeroclaw" 경로 패턴을 레거시로 취급하고 "axiom" 네임스페이스로 전환 중입니다.

---

### 1.2 크레이트별 역할과 경계

#### `core/` — 순수 도메인 계층

| 파일 | 역할 |
|------|------|
| `intent.rs` | 사용자 의도 표현 (ReadFact / WriteFact / RemoveFact / FreezeWrites / Halt) |
| `policy.rs` | 정책 평가 함수 (`evaluate_policy`). 순수 함수, I/O 없음 |
| `policy_codes.rs` | `PolicyCode` enum (Allowed / ActorMissing / RuntimeHalted / ReadOnlyMutation / UnauthorizedControl / PayloadTooLarge) |
| `decision.rs` | 정책 결과로부터 결정 도출 (`decide`) |
| `effect.rs` | 결정의 부작용 표현 (PutFact / RemoveFact / SetMode) |
| `event.rs` | `DomainEvent` — 4가지 이벤트 타입, payload 직렬화/역직렬화 |
| `reducer.rs` | 이벤트 적용 → 새 `AgentState` 반환. 순수 함수 |
| `projection.rs` | 이벤트 스트림 → 최종 상태 폴딩 (`project`, `project_from`) |
| `state.rs` | `AgentState` (revision, mode, facts, 카운터) + 불변 검사 |
| `audit.rs` | `PolicyAuditRecord` 생성 및 감사 라인 포매팅 |
| `error.rs` | `CoreError` (InvalidIntent, PolicyViolation) |

**핵심 불변식**: `denied_count <= audit_count <= revision`

#### `schema/` — 타입 스키마 및 호환성

| 파일 | 역할 |
|------|------|
| `config.rs` | `ConfigSource` (Default/File/Environment/Cli) 우선순위 병합 |
| `legacy.rs` | ZeroClaw → axiom 레거시 경로 파싱/분류. `SchemaVersion` 파싱 |
| `compat.rs` | `CompatLevel` (Exact/Compatible/LegacyBridge/Incompatible) 체크 |
| `dev_mode.rs` | `DevModePolicy` — 개발 모드 완화 정책. 3-요소 튜플 검증 |

#### `adapters/` — 외부 어댑터 계층

| 파일 | 역할 |
|------|------|
| `contracts.rs` (lib.rs) | 5개 어댑터 Trait 정의: `MemoryAdapter`, `ProviderAdapter`, `ChannelAdapter`, `ToolAdapter`, `RuntimeAdapter` |
| `memory.rs` | `MarkdownMemoryAdapter`, `SqliteMemoryAdapter` — 실제 파일/DB 기반 구현 |
| `tool_memory.rs` | `MemoryToolAdapter` — 자체 `BTreeMap` 기반 인메모리 구현 (MemoryAdapter와 단절) |
| `tool_browser.rs` | `BrowserToolAdapter` — URL 허용 목록 기반 브라우저 에뮬레이션 |
| `runtime.rs` | `NativeRuntimeAdapter` — 서브프로세스 실행 |
| `channel.rs` | `CliChannel`, `WebhookChannel` 입력 검증 |
| `channel_discord.rs` | DiscordChannelAdapter — VecDeque stub |
| `channel_telegram.rs` | TelegramChannelAdapter — VecDeque stub |
| `channel_slack.rs` | SlackChannelAdapter — VecDeque stub |
| `channel_irc.rs` | IrcChannelAdapter — VecDeque stub |
| `channel_matrix.rs` | MatrixChannelAdapter — VecDeque stub |
| `channel_whatsapp.rs` | WhatsAppChannelAdapter — VecDeque stub |
| `provider_registry.rs` | 15개 프로바이더 mock 레지스트리 (mock-local만 동작) |
| `error.rs` | `AdapterError` (InvalidInput/NotFound/Unavailable/Failed/PolicyViolation) |

#### `infra/` — 인프라 유틸리티

| 파일 | 역할 |
|------|------|
| `error.rs` | `InfraError` 기본 타입 |
| `lib.rs` | re-export |

#### `apps/` — 실행 계층

**진입점 바이너리:**
- `main.rs` → CLI 파싱, 서브커맨드 디스패치
- `bin/migrate.rs` → 레거시 마이그레이션 바이너리
- `bin/perf_suite.rs` → 성능 벤치마크 바이너리
- `bin/h2_verify.rs` → 병렬 검증 바이너리

**주요 모듈:**

| 파일 | 역할 |
|------|------|
| `agent_loop.rs` | 에이전트 메인 루프. `coclai::Client` 외부 크레이트 사용, `MockAgent`는 test-only |
| `runtime_compose.rs` | 런타임 합성 플랜 (`RuntimeComposePlan`) |
| `gateway.rs` | HTTP 요청 검증, IP 허용 목록, `GatewayRuntime` |
| `cli_command.rs` | 커맨드 파싱 및 라우팅 |
| `config_loader.rs` | 파일/환경변수/CLI 설정 병합 |
| `channel.rs` | `channel start/stop/status/list` 커맨드 처리 |
| `service.rs` | `service start/stop/status` 처리 |
| `cron.rs` | 크론 작업 관리 |
| `skills.rs` | 스킬 로드/실행 |
| `integrations.rs` | 외부 통합 (Info만 구현) |
| `onboard.rs` | 온보딩 플로우 |
| `metrics.rs` | 메트릭 스냅샷 및 대시보드 렌더링 |
| `heartbeat.rs` | 하트비트 루프 |
| `daemon_supervisor.rs` | 슈퍼바이저 컴포넌트 백오프/재시도 |
| `daemon_types.rs` | 데몬 루프 타입 |
| `identity_bootstrap.rs` | 신원 부트스트랩 |
| `doctor.rs` | 시스템 진단 |
| `status.rs` | 상태 표시 |
| `display.rs` | 출력 포매팅 |
| `dev_guard.rs` | 개발 모드 가드 |
| `migrate_*.rs` | 마이그레이션 타입/러너/리포트/메모리/IO |
| `hex_util.rs` / `env_util.rs` / `parse_util.rs` / `time_util.rs` | 공통 유틸리티 |

**스크립트 (tests/ 의존):**
- `scripts/rollback_recovery.sh` → H3 롤백 복구 시뮬레이션. 3개 핵심 인수(workspace-root, runtime-root, snapshot-root)와 환경변수(RECOVERY_MAX_RETRIES, RECOVERY_BACKOFF_MS, RECOVERY_TIMEOUT_MS) 수신.
- `scripts/run_h4_transition_rehearsal.sh` → H4 전환 리허설. apps 바이너리와 h2_verify 바이너리 경로를 인수로 받아 전체 전환 시나리오를 오케스트레이션.

---

## 2. 크리티컬 구조적 문제 (P0)

### P0-1: 채널 어댑터 6종 전체 Stub — 실제 API 호출 없음

**위치:**
- `/Users/axient/repository/axiomAi/adapters/src/channel_discord.rs`
- `/Users/axient/repository/axiomAi/adapters/src/channel_telegram.rs`
- `/Users/axient/repository/axiomAi/adapters/src/channel_slack.rs`
- `/Users/axient/repository/axiomAi/adapters/src/channel_irc.rs`
- `/Users/axient/repository/axiomAi/adapters/src/channel_matrix.rs`
- `/Users/axient/repository/axiomAi/adapters/src/channel_whatsapp.rs`

**근거 코드 (Telegram 예시):**
```rust
// channel_telegram.rs
fn send(&mut self, message: ChannelMessage) -> AdapterResult<ChannelSendReceipt> {
    self.sequence += 1;
    self.queue.push_back(message); // VecDeque에 쌓기만 함
    Ok(ChannelSendReceipt { sequence: self.sequence, accepted: true })
}
```

**패턴:**
- 모든 6개 어댑터가 동일하게 `VecDeque.push_back()`만 수행
- `health()` 함수는 토큰 형식 검사만으로 Healthy/Degraded/Unavailable 판단 (실제 연결 없음)
  - Discord: `"discord_token"` 접두어 없으면 Unavailable
  - Slack: `"xoxb-"` 또는 `"xoxp-"` 접두어 없으면 Unavailable
  - IRC: `"irc."` 포함 여부로 판단
- `drain()` 호출 시에만 메시지 반환 (실제 전송 없음)

**영향:**
- `channel start` 커맨드를 실행해도 실제 Discord/Telegram/Slack 메시지가 전송되지 않음
- 사용자에게 "성공" 응답이 반환되지만 아무 동작도 일어나지 않음
- 프로덕션 사용 불가

---

### P0-2: `channel start` 액션 — 어댑터 연결 없이 `running=true`만 설정

**위치:** `/Users/axient/repository/axiomAi/apps/src/channel.rs`

**근거 코드:**
```rust
// channel.rs - handle_start()
ChannelAction::Start { .. } => {
    state.running = true; // 상태 플래그만 변경
    // 실제 어댑터 초기화 / 연결 코드 없음
}
```

**영향:**
- `axiom channel start discord` 실행 시 `running=true`만 설정되고 종료
- 채널 상태가 "Running"으로 표시되지만 실제로는 아무 연결도 없음

---

### P0-3: `MemoryToolAdapter` — 영구 메모리 어댑터와 단절

**위치:** `/Users/axient/repository/axiomAi/adapters/src/tool_memory.rs`

**근거 코드:**
```rust
// tool_memory.rs
pub struct MemoryToolAdapter {
    store: BTreeMap<String, String>, // 자체 인메모리 저장소
}
// MarkdownMemoryAdapter, SqliteMemoryAdapter와 전혀 연결 없음
```

**영향:**
- 에이전트가 `memory.store` 도구를 사용해 저장한 데이터가 프로세스 종료 시 모두 사라짐
- `MemoryAdapter` 트레잇을 구현하는 `MarkdownMemoryAdapter`, `SqliteMemoryAdapter`가 존재하지만 tool 실행 경로에 주입되지 않음
- 메모리 지속성 기능이 실질적으로 동작하지 않음

---

## 3. 높은 우선순위 문제 (P1)

### P1-1: `release_gate_s2.rs` + `release_security_gate.rs` 테스트 중복

**위치:**
- `/Users/axient/repository/axiomAi/apps/tests/release_gate_s2.rs`
- `/Users/axient/repository/axiomAi/apps/tests/release_security_gate.rs`

**근거:** 두 파일 모두 7개 테스트를 포함하며 `dev_guard.rs`의 `enforce_release_gate`를 테스트합니다. 핵심 시나리오가 동일합니다:
- 릴리즈 빌드에서 `profile=dev` 차단 검증
- 비-dev 프로필은 허용
- 대소문자 무관 비교 (DEV, Dev)
- `enforce_current_build` 함수 (cfg!(debug_assertions) 기반)
- 레거시 플래그 거부
- 파일 기반 우회 거부

**영향:**
- 빌드 시간 증가 (동일 테스트 이중 실행)
- 한쪽 파일만 업데이트 시 두 파일 불일치 위험
- 코드베이스 유지보수 복잡도 증가

---

### P1-2: `config_loader.rs` 파싱 불일치

**위치:** `/Users/axient/repository/axiomAi/apps/src/config_loader.rs`

**근거:**
```rust
// parse_file_config — inline split 사용
fn parse_file_config(input: &str) -> ... {
    for part in line.split(',') { ... } // 인라인 구현
}

// parse_env_config / parse_cli_config — parse_util 사용
fn parse_env_config(...) -> ... {
    parse_tools_list(raw) // parse_util::parse_tools_list
}
```

**영향:**
- 쉼표 처리 방식 불일치 가능성 (공백 처리, 빈 항목 처리 등)
- `parse_util.rs`의 중앙화 정책과 불일치
- `tools` 설정 파싱 결과가 소스에 따라 달라질 수 있음

---

### P1-3: `DevModeMitigationInput` 중복 정의

**위치:**
- `/Users/axient/repository/axiomAi/core/src/policy.rs` (line 13)
- `/Users/axient/repository/axiomAi/schema/src/dev_mode.rs` (line 13)

**근거:** 동일한 이름의 구조체가 두 크레이트에 각각 정의되어 있습니다. `policy.rs`의 버전은 `from_env_values` 메서드를 포함하고, `dev_mode.rs`의 버전은 `DevModeEnvFlags`를 별도로 분리합니다.

**영향:**
- 개발자 혼란 (어느 타입을 사용해야 하는지 불명확)
- 동기화 부담 (한쪽 변경 시 다른 쪽도 변경 필요)
- core 계층이 schema 계층의 개념을 중복 보유

---

### P1-4: `migrate_h1.rs` 테스트의 `hex_encode` / `hex_decode` 중복

**위치:** `/Users/axient/repository/axiomAi/apps/tests/migrate_h1.rs` (line 139-173)

**근거:** 테스트 파일 내에 `hex_encode`, `hex_decode`, `hex_nibble` 함수가 인라인으로 구현되어 있습니다. `apps/src/hex_util.rs`에 동일한 함수가 존재합니다.

**영향:**
- `hex_util.rs` 변경 시 테스트 인라인 구현도 함께 업데이트해야 함
- Round 4 리팩터링의 의도(중복 제거)와 불일치

---

## 4. 중간 우선순위 문제 (P2)

### P2-1: `integrations.rs` — `Info` 액션만 구현

**위치:** `/Users/axient/repository/axiomAi/apps/src/integrations.rs`

**근거:**
```rust
pub fn handle_integration(action: IntegrationsAction) -> IntegrationResult {
    match action {
        IntegrationsAction::Info { name } => { /* 구현됨 */ }
        IntegrationsAction::Install { .. } => Err("install not yet implemented"),
        IntegrationsAction::Remove { .. } => Err("remove not yet implemented"),
    }
}
```

**영향:**
- `axiom integrations install <name>` 실행 시 오류 반환
- openClaw의 ClawHub 마켓플레이스에 해당하는 기능 미구현

---

### P2-2: `skills.rs` — 마켓플레이스 없음, 로컬 파일 시스템만

**위치:** `/Users/axient/repository/axiomAi/apps/src/skills.rs`

**근거:**
- 스킬은 로컬 파일 시스템의 YAML/Markdown 파일로만 정의
- 커뮤니티 공유 플랫폼(ClawHub 상당) 없음
- `SkillSource::Remote` 타입은 있으나 실제 원격 로딩 없음

---

### P2-3: `provider_registry.rs` — 단일 `mock-local` 프로바이더만 동작

**위치:** `/Users/axient/repository/axiomAi/adapters/src/provider_registry.rs`

**근거:**
- 15개 프로바이더 항목이 등록되어 있으나 (`openai`, `anthropic`, `gemini` 등)
- 실제로는 `mock-local`만 `build_contract_provider`에서 동작
- 나머지 14개는 "not supported" 오류 반환

**영향:**
- 실제 LLM 프로바이더 직접 연동 불가 (coclai 외부 크레이트 통해서만 가능)
- 프로바이더 등록 구조가 있지만 비기능 상태

---

### P2-4: `ensure_not_blank` 함수 5중 중복

**위치:**
- `core/src/intent.rs`
- `core/src/decision.rs`
- `core/src/effect.rs`
- `core/src/event.rs`
- 기타

**근거:** 각 모듈이 `fn ensure_not_blank(value: &str, error: T) -> Result<(), T>` 함수를 독립적으로 정의합니다. core 계층이 순수해야 하는 제약이 있으나, 모듈 간 헬퍼 공유 패턴이 없음.

---

### P2-5: `h2_verify` 바이너리 — 하드코딩된 19개 시나리오

**위치:** `/Users/axient/repository/axiomAi/apps/tests/h2_parallel.rs`

**근거:**
```rust
assert_eq!(extract_u32_field(&stdout, "scenario_count"), Some(19));
```

시나리오 수가 하드코딩되어 있어, 시나리오 추가/제거 시 테스트 수동 업데이트 필요.

---

### P2-6: `rollback_recovery_h3.rs` / `transition_rehearsal_h4.rs` — 쉘 스크립트 의존

**위치:**
- `/Users/axient/repository/axiomAi/apps/tests/rollback_recovery_h3.rs`
- `/Users/axient/repository/axiomAi/apps/tests/transition_rehearsal_h4.rs`

**근거:** 두 테스트 파일이 외부 쉘 스크립트(`../scripts/rollback_recovery.sh`, `../scripts/transition_rehearsal.sh`)를 `Command::new("sh")`로 실행합니다. 스크립트 경로가 상대 경로로 하드코딩되어 있어 실행 환경에 따라 실패할 수 있습니다.

---

## 5. 코드 품질 및 패턴 평가

### 5.1 잘 된 부분

**Event Sourcing 패턴의 완성도**
- `Intent → PolicyVerdict → Decision → Effects → DomainEvent → Reducer → AgentState` 흐름이 완전히 구현됨
- 모든 변환 함수가 순수 함수 (side-effect 없음)
- `project_from(&state, &events)` = `events.fold(state, reduce)` 동일성을 테스트로 검증
- `#[forbid(unsafe_code)]`를 core 계층에 적용

**상태 불변식 검증**
```rust
pub fn invariants_hold(&self) -> bool {
    self.denied_count <= self.audit_count && self.audit_count <= self.revision
}
```
256개 seed로 생성된 랜덤 이벤트 스트림에 대해 모든 prefix에서 불변식이 유지됨을 검증.

**포화 산술 (Saturating Arithmetic)**
```rust
next.revision = next.revision.saturating_add(1);
next.audit_count = next.audit_count.saturating_add(1);
```
u64 오버플로를 안전하게 처리.

**어댑터 트레잇 설계**
- `MemoryAdapter`, `ProviderAdapter` 등 5개 트레잇이 명확한 계약 정의
- `AdapterHealth` (Healthy/Degraded/Unavailable) 3단계 헬스 모델

**Payload 직렬화 계층**
- 모든 도메인 타입이 `Payload` 변형을 가지며, `to_payload()` / `try_from_payload()` 양방향 변환 지원
- 빈 문자열 검증이 `try_from_payload` 단계에서 수행됨

**WorkspaceTool 보안**
```rust
// channel_tool.rs
fn workspace_escape_is_blocked_for_symlinked_paths() { ... }
```
경로 탈출(path traversal) 및 심볼릭 링크 탈출을 `canonical_path` 기반으로 차단.

**SqliteMemoryAdapter 하이브리드 리콜**
- `hybrid_recall` 함수: 벡터 유사도(0.7) + 키워드(0.3) 혼합 검색
- 성능 회귀 검사: `avg_ns_per_iteration < 30ms` 조건을 테스트에서 검증

**마이그레이션 시스템**
- Markdown ↔ SQLite 양방향 마이그레이션 지원
- 중복 키는 `updated_at` 타임스탬프 기준으로 최신 값 선택
- `dry_run` 모드 지원, JSON 보고서 출력

**Dev Mode 보안**
- 3-요소 튜플 (`dev_mode=true AND bind=127.0.0.1 AND allow_remote=false`)이 모두 충족될 때만 완화
- "localhost" 문자열 별칭도 거부 (반드시 "127.0.0.1")

**구조화된 에러 타입**
- `AdapterError` → `RetryClass` (Retryable/NonRetryable/PolicyDenied)로 재시도 전략 인코딩
- `PolicyCode` 각 변형이 `as_str()` 및 `is_rejection()` 메서드 보유

### 5.2 개선 필요 부분

**채널 어댑터 건강 판단 로직**
현재 토큰 형식 기반 건강 판단은 실질적 의미가 없습니다:
```rust
// Slack: "xoxb-" 또는 "xoxp-" 없으면 Unavailable
// Discord: "discord_token" 문자열이 있으면 Unavailable (역설적)
```
실제 연결 없이 토큰 형식만으로 상태를 결정하는 것은 오해를 유발합니다.

**`ensure_not_blank` 반복 정의**
core 내에서 5회 이상 동일 헬퍼가 반복됩니다. `core::util` 내부 모듈로 통합할 수 있습니다.

**`#[path]` 파일 기반 테스트 패턴의 불완전성**
`config_priority.rs`, `release_gate_s2.rs` 등이 `#[path = "../src/..."]`로 소스 파일을 직접 포함합니다. 이는 컴파일 유닛 분리를 우회하며, 모듈 의존성 그래프를 흐립니다. 테스트 대상 함수를 `pub(crate)`로 노출하거나 통합 테스트로 전환이 바람직합니다.

**`BenchmarkReport::to_json()` 수동 JSON 직렬화**
`serde_json` 의존성이 없어 JSON 직렬화를 수동으로 구현했습니다. 특수문자 이스케이프는 구현되어 있지만 유지보수 부담이 있습니다.

**성능 임계치 하드코딩**
```rust
assert!(elapsed < Duration::from_secs(10)); // sqlite 80 records
assert!(elapsed < Duration::from_secs(8));  // markdown 120 records
```
CI 환경에 따라 플래키 테스트가 될 가능성이 있습니다.

---

## 6. 테스트 커버리지 평가

### 6.1 테스트 파일별 커버 범위

#### `core/tests/` (7개 파일, 약 40개 테스트)

| 파일 | 커버 범위 |
|------|-----------|
| `domain_flow.rs` | 완전한 파이프라인 E2E (Intent→Policy→Decision→Effect→Project), ReadOnly 차단 |
| `policy_decision.rs` | 정책 규칙 순서, 경계값 (MAX_KEY_LEN/MAX_VALUE_LEN), 순수성 검증 |
| `projection_replay.rs` | 결정론적 재생, `project` = fold 동일성, 입력 불변성 |
| `reducer_cases.rs` | 각 이벤트 타입별 state 변환, 포화 산술 |
| `schema_boundaries.rs` | Payload 직렬화/역직렬화 라운드트립, 오류 전파 |
| `state_invariants.rs` | 256+256 시드 랜덤 스트림에 대한 불변식 검증 |
| `policy_codes.rs` | PolicyCode 문자열 매핑, 거부 카탈로그 |

**평가: 우수.** core 계층은 가장 밀도 높은 테스트 커버리지를 보유합니다.

#### `adapters/tests/` (7개 파일, 약 35개 테스트)

| 파일 | 커버 범위 |
|------|-----------|
| `contracts.rs` | 5개 어댑터 트레잇 인터페이스 계약 검증 |
| `memory.rs` | Sqlite/Markdown 전체 API (store/get/recall/list/delete/count), 성능 smoke |
| `memory_hybrid.rs` | hybrid recall 성능 회귀, SQLite retention job |
| `tool_surface.rs` | memory/browser 도구 라운드트립, 허용 목록, 오류 경계 |
| `channel_tool.rs` | CliChannel/WebhookChannel 검증, WorkspaceTool 정책/경계/탈출 차단 |
| `channel_external.rs` | 6개 채널 어댑터 health matrix, VecDeque 큐 동작 |
| `error.rs` | AdapterError 분류 및 RetryClass 매핑 |

**평가: 양호.** 단, `channel_external.rs`의 테스트는 stub 동작(VecDeque)을 검증하는 것으로, 실제 API 연동을 커버하지 않습니다.

#### `schema/tests/` (4개 파일, 약 20개 테스트)

| 파일 | 커버 범위 |
|------|-----------|
| `schema_compat.rs` | 호환성 수준 (Exact/Compatible/LegacyBridge/Incompatible) |
| `legacy_spec.rs` | SchemaVersion 파싱 (v접두어, legacy: 접두어, 부분 세그먼트) |
| `config_source_merge.rs` | 우선순위 병합 (Default < File < Environment < CLI) |
| `dev_mode_relax_guard.rs` | DevModePolicy 완화 조건 매트릭스 |

**평가: 우수.** 경계 조건 및 매트릭스 테스트가 철저합니다.

#### `apps/tests/` (약 15개 파일, 약 60개 테스트)

| 파일 | 커버 범위 |
|------|-----------|
| `gateway_e2e.rs` | IP 허용 목록, 요청 크기, 메서드 검증 |
| `release_gate_s2.rs` | 릴리즈 게이트 보안 (중복 — P1-1 참조) |
| `release_security_gate.rs` | 릴리즈 보안 게이트 (중복 — P1-1 참조) |
| `config_priority.rs` | CLI > ENV > File > Default 우선순위 |
| `daemon_loop.rs` | DaemonLoop 실패/재시도/완료 시나리오 |
| `daemon_supervisor.rs` | 슈퍼바이저 백오프/복구/예산 소진 |
| `heartbeat_loop.rs` | HeartbeatLoop 주기/정지 조건 |
| `metrics_dashboard.rs` | 메트릭 수집, 스냅샷 병합, 대시보드 렌더링 |
| `policy_audit.rs` | PolicyAuditRecord 필드 및 감사 라인 포매팅 |
| `legacy_paths.rs` | ZeroClaw 레거시 경로 파싱/분류/정규화 |
| `migrate_h1.rs` | Markdown/SQLite 마이그레이션, dry-run, 중복 키 해결 |
| `h2_parallel.rs` | h2_verify 바이너리 19개 시나리오 전부 diff=0 검증 |
| `e2e_cli.rs` | ~25개 CLI 커맨드 E2E (status, service, channel, doctor 등) |
| `perf_suite_smoke.rs` | perf_suite 바이너리 실행 연기 |
| `rollback_recovery_h3.rs` | 쉘 스크립트 기반 롤백 시뮬레이션 |
| `transition_rehearsal_h4.rs` | 쉘 스크립트 기반 전환 리허설 |

**평가: 보통.** 기능적 커버리지는 넓으나, 채널/통합 E2E는 stub에 의존합니다.

### 6.2 미달 영역

1. **채널 어댑터 실제 API 통합 테스트**: Discord/Telegram/Slack/IRC/Matrix/WhatsApp 실제 전송 없음
2. **MemoryToolAdapter → MemoryAdapter 연동 경로**: 영구 저장소 연결이 없어 테스트 불가능
3. **`integrations.rs` install/remove 경로**: 미구현이므로 테스트도 없음
4. **에이전트 루프 실제 LLM 호출**: coclai 외부 크레이트 의존으로 단위 테스트 불가
5. **동시성 테스트**: tokio 런타임 기반 경쟁 조건 테스트 없음
6. **오류 복구 E2E**: 실제 프로세스 재시작/롤백 시나리오 (쉘 스크립트 의존만 존재)

---

## 7. openClaw vs ZeroClaw vs axiomAi 비교

### 7.1 3자 비교 표

| 항목 | openClaw | ZeroClaw | axiomAi |
|------|----------|----------|---------|
| **언어** | TypeScript/Node.js | Rust/tokio | Rust/tokio |
| **코드량** | 430K 라인 | ~50K 라인 (추정) | ~30K 라인 (추정) |
| **바이너리 크기** | N/A (Node.js) | ~3.4MB | 미측정 |
| **메모리 사용** | 높음 (V8 런타임) | <5MB | 유사 (<10MB 추정) |
| **아키텍처** | Hub-and-Spoke 4계층 | 13개 Trait 기반 | Event Sourcing 5계층 |
| **에이전트 루프** | 5단계 (pi-agent-core) | 6단계 (agent_turn) | 외부 coclai 크레이트 위임 |
| **메모리 시스템** | sqlite-vec + FTS5 하이브리드 | hybrid vector(0.7)+keyword(0.3) | Markdown + SQLite + hybrid recall |
| **채널** | 다수 (실제 API) | 미확인 | 6개 (모두 Stub) |
| **스킬/플러그인** | ClawHub 마켓플레이스, YAML-frontmatter MD | 미확인 | 로컬 파일 시스템만 |
| **보안** | pairing/Cedar 정책 | ChaCha20-Poly1305, OTP, E-Stop | PolicyCode 5종, DevMode 미티게이션 |
| **보안 수준** | 엔터프라이즈 | 군사급 암호화 | 기본 (PolicyCode 5종, DevMode 미티게이션) |
| **외부 통합** | 풍부 | Composio | Info만 구현 |
| **서브에이전트** | 미확인 | delegate 지원 | 미구현 |
| **자율성 수준** | 미확인 | readonly/supervised/full | Active/ReadOnly/Halted |
| **테스트 품질** | N/A | N/A | core 계층 우수, 어댑터 stub 의존 |
| **이벤트 소싱** | 없음 | 없음 | 완전 구현 |
| **레거시 마이그레이션** | N/A | N/A | ZeroClaw → axiom 전용 도구 |

### 7.2 axiomAi의 차별점

**강점:**
1. **Event Sourcing 패턴**: 프로젝트 전체에서 유일하게 완전한 Event Sourcing (Intent→Policy→Decision→Effect→Projection)을 구현
2. **순수 core 계층**: `#[forbid(unsafe_code)]`와 완전한 I/O 격리. 모든 도메인 로직이 결정론적으로 테스트 가능
3. **강력한 불변식 검증**: 256개 랜덤 시드로 상태 불변식 검증
4. **스키마 호환성**: `CompatLevel` 시스템으로 버전 업그레이드/다운그레이드 관리
5. **마이그레이션 도구**: ZeroClaw → axiomAi 전환을 위한 완전한 마이그레이션 파이프라인
6. **메모리 하이브리드 리콜**: sqlite-vec 없이도 키워드+유사도 혼합 검색 구현

**격차 (ZeroClaw 대비):**
1. 암호화 없음 (ChaCha20-Poly1305 미구현, API 자격증명 평문 저장)
2. OTP 게이팅 없음
3. E-Stop 메커니즘 없음
4. 채널 실제 API 연동 없음 (6개 모두 stub)
5. delegate 서브에이전트 없음
6. Composio 통합 없음

**격차 (openClaw 대비):**
1. ClawHub 마켓플레이스 없음 (스킬 로컬 파일만)
2. Cedar 정책 엔진 없음
3. 실제 LLM 멀티프로바이더 없음 (coclai 위임)
4. 외부 통합 미완성 (install/remove 없음)

---

## 8. 구현 계획 (로드맵)

### Phase 1: Critical Fixes (크리티컬 수정)

**목표: P0 문제 해결 → 실제 기능 동작 가능 상태**

#### 1-A: MemoryToolAdapter를 MemoryAdapter와 연결

**대상 파일:**
- `/Users/axient/repository/axiomAi/adapters/src/tool_memory.rs`
- `/Users/axient/repository/axiomAi/adapters/src/contracts.rs`

**변경 내용:**
```rust
// 현재
pub struct MemoryToolAdapter {
    store: BTreeMap<String, String>,
}

// 변경 후
pub struct MemoryToolAdapter {
    backend: Box<dyn MemoryAdapter>,
}

impl MemoryToolAdapter {
    pub fn with_markdown(path: &Path) -> Result<Self, AdapterError> { ... }
    pub fn with_sqlite(path: &Path) -> Result<Self, AdapterError> { ... }
}
```

**추가 필요:** `build_contract_tool` 함수가 백엔드 경로를 인수로 받도록 수정

#### 1-B: 채널 어댑터 실제 API 연결

**대상 파일:**
- `/Users/axient/repository/axiomAi/adapters/src/channel_telegram.rs`
- `/Users/axient/repository/axiomAi/adapters/src/channel_discord.rs`
- `/Users/axient/repository/axiomAi/adapters/src/channel_slack.rs`
- 나머지 3개 채널

**변경 내용:**
- `reqwest` (async) 또는 `ureq` (sync) 의존성 추가
- 각 채널에 실제 HTTP API 호출 구현
- `send()` 성공 시 실제 메시지 ID를 `ChannelSendReceipt`에 포함
- `VecDeque`는 오프라인 큐잉용으로만 유지 (실패 시 fallback)

**우선순위 권장:** Telegram → Slack → Discord 순 (API 단순도 기준)

#### 1-C: `channel start` 실제 어댑터 연결

**대상 파일:**
- `/Users/axient/repository/axiomAi/apps/src/channel.rs`

**변경 내용:**

`ChannelStore`에 `adapter` 필드를 직접 추가하는 것은 큰 구조 변경입니다. 대신 다음 경로로 수정합니다:

1. `ChannelEntry`에 `connection_token: Option<String>` 필드 추가 — 채널 시작 시 발급된 토큰으로 연결 상태를 추적
2. 실제 채널 연결 수명 주기는 별도 채널 관리자(예: `ChannelManager`)에서 처리

```rust
// 의사코드 — 실제 구현은 채널 관리자 설계 이후 확정
ChannelAction::Start { name, config_path } => {
    let token = channel_manager.connect(&name, &config_path)?;
    entry.running = true;
    entry.connection_token = Some(token);
}
```

---

### Phase 2: Core Features (핵심 기능)

**목표: 경쟁 제품 수준의 기본 기능 확보**

#### 2-A: 중복 테스트 통합

**대상 파일:**
- `/Users/axient/repository/axiomAi/apps/tests/release_gate_s2.rs`
- `/Users/axient/repository/axiomAi/apps/tests/release_security_gate.rs`

**변경 내용:** 공통 헬퍼를 `tests/common/gateway_helpers.rs`로 추출하고, 두 파일 중 하나 제거 또는 각 파일의 고유 테스트만 유지

#### 2-B: `config_loader.rs` 파싱 통일

**대상 파일:**
- `/Users/axient/repository/axiomAi/apps/src/config_loader.rs`

**변경 내용:** `parse_file_config`의 인라인 tools 파싱을 `parse_util::parse_tools_list`로 교체

#### 2-C: `integrations.rs` install/remove 구현

**대상 파일:**
- `/Users/axient/repository/axiomAi/apps/src/integrations.rs`

**변경 내용:**
- 통합 레지스트리 파일 (`~/.axiom/integrations.toml`) 정의
- `install`: 레지스트리에 추가, 설정 파일 생성
- `remove`: 레지스트리에서 제거, 설정 파일 삭제

#### 2-D: `DevModeMitigationInput` 중복 제거

**대상 파일:**
- `/Users/axient/repository/axiomAi/core/src/policy.rs`
- `/Users/axient/repository/axiomAi/schema/src/dev_mode.rs`

**아키텍처 원칙:** `core`는 워크스페이스 최하위 계층으로 `schema`에 의존해서는 안 됩니다. `core::policy`에서 `axiom_schema`를 import하면 계층 역전이 발생합니다.

**올바른 해결 방법:**
- `DevModeMitigationInput`은 `core::policy` 모듈 내에서만 정의 (현재 위치 유지)
- `schema::dev_mode::DevModePolicy`는 `DevModeMitigationInput`에 의존하는 대신, 평가에 필요한 최소 인터페이스(bool 플래그 3개: `dev_mode`, `bind_localhost`, `allow_remote`)를 직접 수신하도록 변경
- `schema::dev_mode`의 `DevModeMitigationInput` 정의는 삭제

**변경 방향 (의사코드):**
```rust
// schema/src/dev_mode.rs — DevModeMitigationInput 제거, 평가 함수 시그니처 변경
pub fn evaluate_dev_mode(dev_mode: bool, bind_localhost: bool, allow_remote: bool) -> DevModeVerdict { ... }

// core/src/policy.rs — DevModeMitigationInput 단독 소유
pub struct DevModeMitigationInput { pub dev_mode: bool, pub bind_localhost: bool, pub allow_remote: bool }
```

#### 2-E: `migrate_h1.rs` hex 유틸리티 중복 제거

**대상 파일:**
- `/Users/axient/repository/axiomAi/apps/tests/migrate_h1.rs`

**변경 내용:** `#[path = "../src/hex_util.rs"]` 패턴으로 `hex_util` 모듈 직접 포함

#### 2-F: 멀티프로바이더 지원

**대상 파일:**
- `/Users/axient/repository/axiomAi/adapters/src/provider_registry.rs`

**변경 내용:**
- openai, anthropic, gemini 등 실제 API 클라이언트 구현 (reqwest 기반)
- `ProviderAdapter::complete()` 실제 HTTP 호출

---

### Phase 3: Advanced Features (고급 기능)

**목표: ZeroClaw/openClaw 경쟁력 확보**

#### 3-A: 암호화 레이어

**대상 파일 (신규):**
- `/Users/axient/repository/axiomAi/infra/src/crypto.rs`

**변경 내용:**
- `ring` 또는 `chacha20poly1305` 크레이트로 ChaCha20-Poly1305 구현
- 게이트웨이 요청 서명 HMAC-SHA256 신규 구현 (현재 서명 없음)
- 메모리 저장 시 API 자격증명 암호화

#### 3-B: Skills 마켓플레이스

**대상 파일:**
- `/Users/axient/repository/axiomAi/apps/src/skills.rs`
- 신규: `/Users/axient/repository/axiomAi/apps/src/skill_registry.rs`

**변경 내용:**
- 원격 스킬 저장소 (`https://registry.axiom.ai/skills`) 정의
- YAML-frontmatter 기반 스킬 패키지 형식 표준화
- `axiom skills install <name>` 원격 다운로드/검증 구현

#### 3-C: 서브에이전트 (Delegate)

**대상 파일:**
- `/Users/axient/repository/axiomAi/apps/src/agent_loop.rs`
- 신규: `/Users/axient/repository/axiomAi/apps/src/agent_delegate.rs`

**변경 내용:**
- `DelegateRequest` 타입 정의
- 자율성 수준에 따른 서브에이전트 생성/감독
- 서브에이전트 결과를 부모 에이전트 상태에 통합

#### 3-D: OTP 게이팅 및 E-Stop

**대상 파일:**
- `/Users/axient/repository/axiomAi/apps/src/gateway.rs`
- 신규: `/Users/axient/repository/axiomAi/apps/src/estop.rs`

**변경 내용:**
- OTP(TOTP/HOTP) 기반 고위험 명령어 게이팅
- E-Stop 신호 핸들러 (SIGTERM → `Intent::halt("estop", "system")`)
- `HaltCommand` 감사 로그 필수화

#### 3-E: Composio 통합

**대상 파일:**
- `/Users/axient/repository/axiomAi/apps/src/integrations.rs`

**변경 내용:**
- Composio API 클라이언트 추가
- `IntegrationsAction::Composio { tool_id, args }` 변형 추가
- 200+ Composio 도구를 `ToolAdapter` 래퍼로 노출

---

## 9. 결론

axiomAi는 **탄탄한 도메인 아키텍처**와 **불완전한 어댑터 구현**이 공존하는 프로젝트입니다.

### 강점 요약

`core/` 계층은 업계 수준의 Event Sourcing 구현을 보여줍니다. 256개 랜덤 시드로 검증된 불변식, Payload 직렬화 라운드트립, 포화 산술, 순수 함수 체계 — 이 모든 것이 잘 설계되어 있습니다. `schema/` 계층의 버전 호환성 시스템과 마이그레이션 파이프라인도 프로덕션 품질에 가깝습니다.

### 핵심 위험

**세 가지 P0 문제가 프로젝션 사용을 차단합니다:**

1. 채널 어댑터 6종 전체가 VecDeque stub — 메시지가 실제로 전송되지 않음
2. `channel start`가 플래그만 설정하고 어댑터를 연결하지 않음
3. `MemoryToolAdapter`가 `MarkdownMemoryAdapter`/`SqliteMemoryAdapter`와 단절 — 메모리가 지속되지 않음

이 세 가지 문제를 해결하지 않으면 axiomAi는 **테스트는 통과하지만 실제로는 아무 동작도 하지 않는** 시스템입니다.

### 권장 우선순위

```
Phase 1 (Critical): P0 수정 — 실제 채널 전송 + 메모리 지속성
Phase 2 (Core): P1/P2 수정 + 멀티프로바이더 + 통합 완성
Phase 3 (Advanced): 암호화 + 마켓플레이스 + 서브에이전트
```

ZeroClaw 대비 axiomAi의 Event Sourcing 아키텍처는 독창적인 차별점입니다. 감사 추적(audit trail)의 완전성과 상태 재생(replay) 능력은 엔터프라이즈 컴플라이언스 요구사항에서 강점이 될 수 있습니다. 단, 이 이점을 실현하려면 P0 문제 해결이 선행되어야 합니다.
