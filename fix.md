전체 프로젝트 종합 분석 보고서

유닛별 점수 요약

┌───────────────────────────────────────┬────────┐  
 │ 유닛 │ 점수 │  
 ├───────────────────────────────────────┼────────┤
│ core/ — 도메인 모델 │ 6.5/10 │  
 ├───────────────────────────────────────┼────────┤  
 │ schema/ — 설정 스키마 │ 4.0/10 │
├───────────────────────────────────────┼────────┤
│ adapters/ contracts + channels │ 5.5/10 │
├───────────────────────────────────────┼────────┤
│ adapters/ memory + tools + providers │ 6.5/10 │
├───────────────────────────────────────┼────────┤
│ apps/ entry + agent_loop + runtime │ 7.0/10 │
├───────────────────────────────────────┼────────┤
│ apps/ daemon + gateway + infra + util │ 6.5/10 │
├───────────────────────────────────────┼────────┤
│ 전체 종합 │ 6.0/10 │
└───────────────────────────────────────┴────────┘

---

P0 — Critical (즉시 수정 필요)

총 11개 발견. 프로덕션 데이터 경로에 직접 영향.

┌─────┬────────────────────────────┬───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ # │ 위치 │ 내용 │
├─────┼────────────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ 1 │ channel_irc.rs │ AdapterFuture async 인터페이스 내 동기 blocking TCP I/O → tokio 런타임 스레드 블로킹 │
├─────┼────────────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ 2 │ async_http_bridge.rs │ 어댑터마다 독립 multi-thread tokio 런타임 생성 → 스레드 폭발. Default::default() 실패 시 panic │
├─────┼────────────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ 3 │ agent_coclai.rs │ 매 run() 호출마다 신규 연결 생성+종료 — 세션 재사용 없음 │
├─────┼────────────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ 4 │ memory_markdown.rs:171-181 │ store 실패 시 인메모리 state ↔ 디스크 diverge → 어댑터 손상 상태로 계속 동작 │
├─────┼────────────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ 5 │ memory_axiomme.rs:207-213 │ recall이 full value 대신 검색 스니펫(abstract_text) 반환 — MemoryAdapter 트레잇 계약 위반 │
├─────┼────────────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ 6 │ gateway.rs:283 │ 요청별 서명 데이터를 전역 환경변수(std::env::var)로 처리 → HttpBoundaryRequest에 서명 필드 없음. 멀티테넌트/테스트 환경에서 서명 │
│ │ │ 계약 파괴 │
├─────┼────────────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ 7 │ hex_util.rs │ hex_decode가 임의 바이너리에 UTF-8 가정 → migrate_memory.rs 경로에서 데이터 손상 │
├─────┼────────────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ 8 │ migrate_runner.rs:76 │ read_workspace_schema_hint에서 디렉터리 경로에 read_to_string 호출 → 항상 에러 → 스키마 호환성 체크가 실제로 절대 실행되지 않음 │
│ │ │ (사일런트 버그) │
├─────┼────────────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ 9 │ event.rs │ audit.rs 정의 타입(PolicyAuditRecord)의 to/from_payload impl이 event.rs에 구현 — 책임 분산 │
├─────┼────────────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ 10 │ legacy.rs │ path 관련 타입/함수 전체가 lib.rs re-export 없음 → crate 외부 접근 불가능한 dead code │
├─────┼────────────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ 11 │ SchemaVersion 타입 │ legacy.rs 내부에 정의되어 있으나 실제로는 일반 semver 타입 — 이름과 위치 모두 오해 유발 │
└─────┴────────────────────────────┴───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘

---

P1 — Important (스프린트 내 수정 권장)

[아키텍처 구조]

- sync-over-async 전체 레이어: runtime_compose_bridge + async_runtime_host + 각 어댑터의 block_on 패턴이 전체 provider/tool 레이어에 확산. CLI 명령마다 전역 tokio 런타임
  초기화
- MemoryAdapter 트레잇 &mut self: store/delete의 &mut self 요구로 이중 Mutex 강제 (Arc<Mutex<Connection>> 위에 Mutex<Box<dyn MemoryAdapter>>)
- lib.rs vs main.rs 모듈 중복 컴파일: agent_loop 등 동일 소스가 두 컴파일 단위에서 컴파일됨

[채널 레이어]

- ChannelMessage 두 개 공존: channel::ChannelMessage {kind, route, payload} vs contracts::ChannelMessage {topic, body} — 동일 도메인 개념의 이중 표현
- topic 필드 의미론 붕괴: Discord=채널명, Telegram=chat_id, Matrix=room_id, IRC=IRC채널 — 동일 필드가 채널마다 다른 의미
- live() 생성자 silent offline 폴백: Discord/Slack/IRC/Matrix 모두 설정 누락 시 Offline으로 조용히 폴백 — 설정 오류 무음 통과
- Telegram bot_token Debug 노출: 다른 5개 어댑터는 마스킹하는데 Telegram만 노출
- Telegram health=Degraded but 실제는 Unavailable: allowed_users 비어있으면 health=Degraded인데 실제로는 모든 메시지 드롭

[core 레이어]

- mirror type explosion: Intent/IntentPayload, Effect/EffectPayload, DomainEvent/DomainEventPayload 등 구조적으로 동일한 타입 쌍 5개 (~200줄 보일러플레이트). serde
  feature로 제거 가능
- CoreError/CoreResult dead code: 어떤 public 함수도 이 타입을 반환하지 않음
- AgentState.last_policy_code 의미론: 초기 상태에서 PolicyCode::Allowed — Option<PolicyCode>가 맞음

[apps 레이어]

- integrations.rs Remove 노옵: 이름 검사 후 실제로 아무것도 제거하지 않고 성공 반환
- daemon.rs 슈퍼바이저 위치: 루프 완료 후 실행 — 루프 실행 중 컴포넌트 장애 감지 불가
- cron.rs 스케줄 파서: 분 필드 변형만 지원. 표준 cron 표현식 대부분 미지원
- gateway.rs handle(): ~130줄, 서명 검증 + 경계 검증 + 정책 평가 + 투영 + 메트릭 혼합
- migrate_io.rs 수기 TOML 이스케이프: Windows 경로 등 엣지케이스에서 파손 가능
- agent_registry.rs MockAgentAdapter 노출: 프로덕션 레지스트리에 격리 없이 노출 (AXONRUNNER_AGENT_ID=mock)
- memory_hybrid.rs::semantic_score: 이름은 "semantic"이지만 실제로 substring/prefix 매칭
- h2_verify.rs 가독성 훼손: #[rustfmt::skip] + 극단적 코드 압축

---

P2 — Minor (기술 부채 백로그)

중복 코드 패턴 (공통 추출 필요):

- block_on 헬퍼 8개 테스트 파일 중복
- HTTP_CONNECT_TIMEOUT_SECS, MAX_ERROR_BODY_PREVIEW 상수 5개 채널 파일 중복
- hex_nibble/hex_encode/hex_decode 로직 3곳 (hex_util.rs, gateway_signature.rs, migrate_h1.rs)
- write_json_string 함수 2곳 (migrate_report.rs, cli_perf_suite_report.rs)
- RAG 컨텍스트 초기화 패턴 daemon.rs와 channel.rs에 중복
- CLI 인수 파싱 패턴(--flag value / --flag=value) 4곳 중복

네이밍/의미 불명확:

- channel_serve.rs: 3개 tokio 런타임 계층 중첩 (outer sync → BlockingBatchExecutor → spawn_blocking)
- DevModePolicy.enabled vs env_flags.dev_mode 중복 필드
- GuardMode enum과 relaxes_guard(): bool 의미 중복
- SchemaVersion::Display ("1.2.3") vs normalized() ("v1.2.3") canonical 불분명
- channel.rs 한국어 주석 (영어 코드베이스 불일치)

---

종합 평가

잘 된 부분:

- 순수 함수 규율 (core/ 전체, metrics.rs, gateway_boundary.rs) — 테스트 가능성 최고
- 보안 설계 (HMAC-SHA256 constant-time, API key 마스킹, allowlist 패턴, #![forbid(unsafe_code)])
- 테스트 커버리지 (563 passed, 단위/통합/E2E/속성 기반 테스트)
- RetryClass 기반 에러 분류, ToolPolicy::deny_all() opt-in 설계

근본적 구조 문제:

1. async-sync 경계 미결 — 동기 CLI가 async 트레잇을 사용해야 하는 긴장이 runtime_compose_bridge + async_http_bridge + agent_coclai의 block_on 패턴으로 표면화. 프로젝트
   전체에 걸쳐 async 표면 뒤에 blocking 호출이 숨어있음
2. schema 크레이트 응집력 부재 — config merge 정책 + schema version compat + dev mode 보안이 이유 없이 한 크레이트에 묶임. SchemaVersion 타입 위치 오류가 가장 심각
3. mirror type 보일러플레이트 — serde feature 없이 직렬화 레이어를 수작업으로 구현한 결과. core 도메인 타입 수가 2배
