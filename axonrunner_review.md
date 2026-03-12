# AxonRunner 정밀 분석 및 재설계 제안

## 정정
GitHub 루트 렌더링에서 한때 `This repository is empty`가 표시되었지만, 실제 `tree/main`, 브랜치, 커밋 히스토리 기준으로 저장소는 비어 있지 않습니다. 따라서 분석 기준은 **실제 트리와 커밋 히스토리**로 잡았습니다.

## 1. 저장소 요약
- 워크스페이스 크레이트: `apps`, `adapters`, `core`, `infra`, `schema`
- 지향: 이벤트 소싱 기반, 단일 바이너리 멀티채널 AI 에이전트 프레임워크
- 현재 표면: CLI, 채널, 게이트웨이, 데몬, 메트릭스, 메모리, 툴, 프로바이더, 컨텍스트, 배포/릴리즈/벤치 운영 문서
- 실제 상태: 철학과 운영 문서는 강함. 반면 제품 범위가 빠르게 확장되며 “한 가지를 확실히 잘하는 에이전트” 목표와 긴장이 생김.

## 2. 최상위 파일/디렉터리별 분석

### 루트
- `.github/workflows/`
  - `audit.yml`: 품질/보안 감사 파이프라인
  - `benchmarks.yml`: 성능 벤치 실행
  - `ci.yml`: 일반 CI
  - `ignored-live-tests.yml`: 선택적 라이브 테스트
  - `release-security-gate.yml`: 릴리즈 보안 게이트
  - `renewal-readiness.yml`: 갱신/운영 준비도 게이트
  - `transition-gates.yml`: 전환 리허설/게이트
  - 판단: **좋음**. 운영 성숙도는 높음. 다만 제품 핵심보다 운영 표면이 앞서 있다.

- `benchmarks/`
  - `raw/`: 원시 벤치 결과 저장
  - `summary.tsv`: 집계 결과
  - 판단: **좋음**. 성능 회귀 관리에 유리.

- `crates/`: 핵심 코드. 아래 별도 분석.

- `docs/`
  - `ADR-001-public-async-adapter-contract.md`: 공개 adapter contract를 async로 정하는 ADR
  - `DEPLOYMENT.md`: 배포/환경 변수/런타임 구성 문서
  - `IMPLEMENTATION-PLAN.md`: 현재 구현 계획과 옵션 비교, 남은 정합성 문제 정리
  - `TASKS.md`: 작업 상태 추적
  - `feature-adoption-checklist.md`: 기능 채택 기준
  - `performance-benchmark-suite.md`: 성능 검증 절차
  - `project-charter.md`: 미션/비목표/원칙
  - `release-readiness-gate.md`: 릴리즈 게이트
  - 판단: **매우 좋음**. 문서 규율은 이 저장소의 강점.

- `scripts/`
  - `rollback_recovery.sh`: 롤백/복구 리허설
  - `run_benchmarks.sh`: 벤치 실행
  - `run_execution_contract_gate.sh`: 실행 계약 게이트
  - `run_h2_parallel.sh`: 병렬/하니스 실행
  - `run_h4_transition_rehearsal.sh`: 전환 리허설
  - `run_ignored_live_tests.sh`: 라이브 테스트 실행
  - `run_release_approval_gate.sh`: 릴리즈 승인 게이트
  - `run_release_rehearsal_gate.sh`: 릴리즈 리허설
  - `run_renewal_readiness.sh`: 운영 준비도 실행
  - `run_transition_gates.sh`: 전환 게이트 실행
  - 판단: **좋음**. 단, 지금 수준에선 운영 자동화가 코어 제품보다 넓다.

- `.gitignore`: 일반 ignore
- `CHANGELOG.md`: 릴리즈/변경 기록
- `Cargo.lock`: 의존성 잠금
- `Cargo.toml`: workspace 멤버 정의
- `LICENSE`: 라이선스
- `README.md`: 제품 포지셔닝과 사용 표면
- `fix.md`: 저장소 자체 문제점 분석 메모
- 판단:
  - `README.md`: 제품 설명은 강하지만 실제 실행 가능성은 일부 의존성 문제로 흔들린다.
  - `fix.md`: 매우 유용. 스스로의 구조 문제를 정확히 드러낸다.

## 3. crates/core 파일별 분석

### `crates/core/`
- `Cargo.toml`: 최소 의존성, 순수 도메인 지향
- `Cargo.lock`: 잠금 파일
- `ANALYSIS_REPORT.md`: 코어 분석 문서
- `analysis_payload.md`: 분석 입력/메모
- `src/lib.rs`: 도메인 모듈 재노출. **좋음**
- `src/audit.rs`: 정책/결정 감사 모델. **유지**
- `src/decision.rs`: 결정 타입. **유지**
- `src/effect.rs`: 상태 변화 효과 모델. **유지**
- `src/event.rs`: 이벤트 모델. **유지**
- `src/intent.rs`: 의도 모델. **유지**
- `src/policy.rs`: 정책 판단과 dev mode 완화 규칙. **핵심**
- `src/policy_codes.rs`: 정책 코드 분류. **좋음**
- `src/projection.rs`: 리플레이/투영. **핵심**
- `src/reducer.rs`: 상태 전이 엔진. **핵심**
- `src/state.rs`: 상태 모델. **핵심**
- `src/validation.rs`: 경계 검증. **유지**
- `tests/domain_flow.rs`: 도메인 흐름 테스트
- `tests/policy_codes.rs`: 정책 코드 테스트
- `tests/policy_decision.rs`: 정책 결정 테스트
- `tests/projection_replay.rs`: 리플레이 테스트
- `tests/reducer_cases.rs`: reducer 케이스
- `tests/schema_boundaries.rs`: 스키마/경계 테스트
- `tests/state_invariants.rs`: 상태 불변성 테스트

### 코어 판단
`core`는 저장소에서 가장 설계가 안정적인 부분이다. 도메인/정책/리듀서/프로젝션 경계가 선명하고, **AxonRunner의 철학적 중심으로 삼기에 적합**하다.

## 4. crates/schema 파일별 분석

### `crates/schema/`
- `Cargo.toml`: 구성 전용 크레이트
- `src/lib.rs`: config/dev_mode 재노출
- `src/config.rs`: 다중 소스 설정 병합 규칙
- `src/dev_mode.rs`: 개발 모드 가드
- `tests/config_source_merge.rs`: 병합 테스트
- `tests/dev_mode_relax_guard.rs`: dev 완화 규칙 테스트

### 판단
`schema`는 작고 목적이 명확하다. **유지**. 다만 장기적으로는 `core`와 통합 가능성도 있다. 별도 크레이트로 둘 이유가 “외부 표면 계약 분리”라면 유지, 아니면 단순화를 위해 합칠 수 있다.

## 5. crates/infra 파일별 분석

### `crates/infra/`
- `Cargo.toml`: 인프라 크레이트
- `ANALYSIS_REPORT.md`: 분석 문서
- `analysis_payload.md`: 분석 메모
- `src/lib.rs`: infra 공용 export
- `src/error.rs`: 공용 에러 타입
- `tests/error.rs`: 에러 테스트

### 판단
현재 범위 기준으로 `infra`는 매우 얇다. **독립 크레이트 가치가 약하다.** `core` 또는 `apps`로 흡수해도 된다. 지금 형태는 계층을 위한 계층에 가깝다.

## 6. crates/adapters 파일별 분석

### 루트
- `Cargo.toml`: adapter 종속성 정의
  - 문제: `../../../coclai/crates/coclai` 로컬 path 의존성 존재
  - 문제: `axiomme-core` git 의존성 존재
  - 판단: **즉시 수정 필요**. 저장소 단독 재현성을 깨뜨린다.
- `ANALYSIS_REPORT.md`: 분석 문서
- `src/lib.rs`: adapter 모듈 export
- `src/error.rs`: adapter error

### 계약/레지스트리/런타임 계층
- `src/contracts.rs`: `ProviderAdapter`, `MemoryAdapter`, `ChannelAdapter`, `ToolAdapter`, `AgentAdapter`, `RuntimeAdapter`, `ContextAdapter` 정의
  - 판단: **가장 가치 높은 파일 중 하나**. ZeroClaw식 좋은 경계와 잘 맞는다.
- `src/runtime.rs`: 런타임 어댑터 구현/조립
  - 판단: 유지하되 축소 필요
- `src/provider_registry.rs`: provider registry
  - 판단: 좋음. 다만 provider 수는 줄여야 함
- `src/channel_registry.rs`: channel registry
  - 판단: v1 단일 목적 에이전트에는 과함
- `src/agent_registry.rs`: agent registry
  - 판단: agent 종류가 1개면 불필요
- `src/channel.rs`, `src/memory.rs`, `src/tool.rs`: 표면 re-export/정리 계층
  - 판단: 유지 가능

### 프로바이더
- `src/provider_openai.rs`: OpenAI 호환 provider
  - 판단: **핵심 유지 후보**
- `src/provider_anthropic.rs`: Anthropic provider
  - 판단: 유지 가능하지만 v1은 하나만 우선
- `src/agent_coclai.rs`: coclai 기반 에이전트 연결
  - 판단: **현재 설계의 가장 위험한 결합점**. 로컬 path 의존성과 함께 제거/내재화/대체 필요

### 메모리
- `src/memory_sqlite.rs`: sqlite memory backend
  - 판단: **v1 기본값으로 적합**
- `src/memory_markdown.rs`: markdown memory backend
  - 판단: 개념은 단순하나 정합성 드리프트 위험
- `src/memory_hybrid.rs`: 하이브리드 메모리
  - 판단: v1에서 제거 권장
- `src/memory_axiomme.rs`: axiomme 연동
  - 판단: 외부 의존성 높은 실험 영역. v1 제외 권장
- `src/context_axiomme.rs`: context 검색/연동
  - 판단: 마찬가지로 v1 제외 권장

### 툴
- `src/tool_memory.rs`: memory tool
  - 판단: 유지 가능
- `src/tool_browser.rs`: browser tool
  - 판단: 문서상/계획상 stub 문제 있었음. v1 제외 또는 명확히 experimental 표시
- `src/tool_composio.rs`: composio 연동 툴
  - 판단: v1 제외
- `src/tool_delegate.rs`: delegate tool
  - 판단: 단일 목적 에이전트 철학과 상충. v1 제외

### 채널
- `src/channel_discord.rs`
- `src/channel_irc.rs`
- `src/channel_matrix.rs`
- `src/channel_slack.rs`
- `src/channel_telegram.rs`
- `src/channel_whatsapp.rs`
- `src/channel_validate.rs`
- 판단: 멀티채널 표면 자체가 목표를 흐린다. `channel_irc`는 저장소 자체 메모에도 blocking TCP 문제가 지적되어 있음. **v1은 전부 분리 또는 feature-gated experimental로 격리**.

### 브리지/보조
- `src/async_http_bridge.rs`: 브리지 레이어
  - 판단: async/sync 경계 혼선의 신호. 정리 필요

### 바이너리
- `src/bin/http_bridge_perf.rs`: 성능 실험/측정 바이너리
  - 판단: 유지 가능하나 제품 핵심 아님

### 테스트
- `tests/channel_external.rs`: 외부 채널 테스트
- `tests/channel_tool.rs`: 채널+툴 표면 테스트
- `tests/contracts.rs`: 계약 테스트
- `tests/error.rs`: 에러 테스트
- `tests/memory.rs`: 메모리 테스트
- `tests/memory_hybrid.rs`: 하이브리드 메모리 테스트
- `tests/tool_surface.rs`: 툴 표면 테스트
- 판단: 계약/메모리 테스트는 가치 높음. 채널/하이브리드 관련은 범위 축소시 재배치 가능.

## 7. crates/apps 파일별 분석

### 루트
- `Cargo.toml`: 앱 계층 조립. core/adapters/infra/schema 전부 의존
- `IMPROVEMENTS.md`: apps 구조 개선 메모
- `README.md`: apps 역할 설명
- 판단: `apps`는 composition layer로 의도는 맞지만 현재는 너무 많은 책임을 가짐.

### `src/`
- `lib.rs`: CLI 진입용 조립 허브. **핵심이지만 과밀**
- `main.rs`: `run_cli_entrypoint()` 호출. **좋음**

#### CLI/명령
- `cli_args.rs`, `cli_command.rs`: CLI 구조
- `cli/args.rs`, `cli/command.rs`, `cli/mod.rs`: 중복된 CLI 구조 정리 시도
- `cli_runtime.rs`, `cli_runtime/actions.rs`: CLI 실행 로직
- `cli_perf_suite_args.rs`, `cli_perf_suite_report.rs`, `cli_perf_suite_targets.rs`: 성능 스위트 관련 CLI
- 판단: 구조가 **이행 중**이다. flat 파일과 하위 모듈이 공존한다. 하나로 정리해야 한다.

#### 런타임/조립
- `runtime_compose.rs`: provider/memory/tool/channel/context 조립
- `runtime_compose/plan.rs`: 조립 계획
- `async_runtime_host.rs`: 비동기 런타임 호스트
- `channel_runtime.rs`: 채널 기반 런타임
- `channel_serve.rs`: 채널 서빙
- `agent_loop.rs`: 에이전트 루프
- 판단: 제품의 핵심 흐름이 여기 있지만, CLI/채널/서비스/데몬 논리가 서로 섞여 있다.

#### 채널/게이트웨이/데몬
- `channel.rs`: 채널 앱 계층
- `channel/store.rs`: 채널 스토어
- `gateway_signature.rs`: 게이트웨이 서명
- `gateway/boundary.rs`, `gateway/mod.rs`, `gateway/signature.rs`: 게이트웨이 모듈
- `daemon/mod.rs`, `daemon/supervisor.rs`, `daemon/types.rs`: 데몬 수퍼바이저
- `service.rs`: 서비스 진입
- 판단: 모두 넓은 운영 플랫폼 기능. 단일 목적 에이전트 v1에서는 제거 또는 별도 앱으로 분리.

#### 운영/보조
- `cron.rs`: 스케줄링
- `dev_guard.rs`: 개발 모드 가드
- `display.rs`: 출력 표시
- `doctor.rs`: 진단
- `env_util.rs`: 환경 유틸
- `estop.rs`: emergency stop
- `heartbeat.rs`: 상태 heartbeat
- `hex_util.rs`: hex 유틸
- `identity_bootstrap.rs`: identity bootstrap
- `integrations.rs`: 통합 표면
- `metrics.rs`, `metrics_http.rs`: 메트릭 수집/노출
- `onboard.rs`: onboarding
- `otp_gate.rs`: OTP gate
- `parse_util.rs`: 파싱 유틸
- `skills.rs`, `skills_registry.rs`: skills 표면
- `status.rs`: 상태 표면
- `time_util.rs`: 시간 유틸
- 판단:
  - `doctor`, `display`, `dev_guard`, `parse_util` 정도는 유용
  - `cron`, `metrics_http`, `otp_gate`, `heartbeat`, `skills`, `integrations`, `identity_bootstrap` 등은 v1 핵심 범위와 거리 있음

### `src/bin/`
- `axiom_apps.rs`: 별도 실행 엔트리
- `h2_verify.rs`: 검증/하니스 바이너리
- `perf_suite.rs`: 성능 스위트 실행기
- 판단: 운영/검증에는 좋음. 제품 단순화 시 일부는 `xtask`나 스크립트로 이동 가능.

### `tests/`
- `common/mod.rs`: 테스트 공용 유틸
- `config_priority.rs`: 설정 우선순위
- `daemon_loop.rs`: 데몬 루프
- `daemon_supervisor.rs`: 데몬 수퍼바이저
- `e2e_cli.rs`: CLI 종단간
- `gateway_e2e.rs`: 게이트웨이 종단간
- `h2_parallel.rs`: 병렬 하니스
- `heartbeat_loop.rs`: heartbeat 루프
- `metrics_dashboard.rs`: 메트릭 대시보드
- `perf_suite_smoke.rs`: 성능 스모크
- `policy_audit.rs`: 정책 감사
- `release_security_gate.rs`: 릴리즈 보안 게이트
- `rollback_recovery_h3.rs`: 롤백 복구
- `transition_rehearsal_h4.rs`: 전환 리허설
- 판단: 좋은 테스트 문화. 하지만 범위가 넓어 현재 제품의 우선순위를 흐린다.

## 8. 지금 설계의 강점
1. **문서 규율이 강하다**: charter, ADR, gate, task, benchmark 체계가 좋다.
2. **core가 명확하다**: event / intent / policy / reducer / projection 구조는 재사용 가치가 높다.
3. **adapter contract가 좋다**: 계약 지향 경계는 ZeroClaw의 장점을 건강하게 차용할 수 있는 지점이다.
4. **운영 자동화 의식이 높다**: 릴리즈 게이트, 복구, 리허설, 성능 관리가 있다.

## 9. 지금 설계의 본질적 문제
1. **목표와 표면이 충돌한다**
   - 저장소 철학은 신중하고 정확한 자동화인데, 실제 구현 표면은 멀티채널/게이트웨이/데몬/메모리/스킬/브라우저/통합으로 확장되었다.
2. **단독 재현성이 약하다**
   - `coclai` 로컬 path 의존성은 심각하다.
3. **진실한 capability surface가 아니다**
   - 문서와 표면은 넓은데, 계획 문서와 자체 분석 메모에는 미완/스텁/정합성 문제가 드러난다.
4. **계층이 많은데 일부는 얇다**
   - `infra`는 현재 분리 효용이 약하다.
5. **async/sync 경계와 런타임 경계가 완전히 정리되지 않았다**
6. **실험 기능과 핵심 기능이 같은 중심선에 놓여 있다**

## 10. ZeroClaw에서 차용할 것 / 차용하지 말 것

### 차용할 것
- trait/factory 중심 계약 구조
- fail-fast / secure-by-default / docs-first 운영 규율
- 프롬프트/런타임/툴 경계의 명시성
- 관측 가능성(Observer/trace) 개념
- 기능 노출 전 gate와 runbook 문화

### 차용하지 말 것
- 넓은 운영체제형 표면 전체
- 멀티채널/브라우저/하드웨어/메모리 확장 중심 사고
- “모든 걸 할 수 있는 에이전트” 지향

## 11. 최고의 설계 제안: AxonRunner Solo

### 제품 정의
AxonRunner v1은 **로컬 워크스페이스 작업을 정확하고 재현 가능하게 끝내는 단일 목적 에이전트**다.

### 절대 목표
- 저장소/워크스페이스를 읽는다.
- 필요한 수정 계획을 세운다.
- 파일을 안전하게 수정한다.
- 명령을 제한적으로 실행한다.
- 실행 trace와 결과를 남긴다.

### v1에서 제외
- 모든 채널(Discord/Slack/Telegram/IRC/Matrix/WhatsApp)
- 게이트웨이 서빙
- 데몬/서비스 모드
- 브라우저/Composio/Delegate 툴
- 다중 장기 메모리 백엔드
- OTP/HMAC 운영 표면
- heartbeat/metrics HTTP/cron

## 12. 추천 아키텍처

### 옵션 A — 가장 권장
3 크레이트만 유지:
1. `axonrunner-core`
   - `intent`, `policy`, `effect`, `event`, `state`, `reducer`, `projection`
2. `axonrunner-runtime`
   - provider, tool registry, workspace sandbox, sqlite run log, orchestration loop
3. `axonrunner-cli`
   - `run`, `doctor`, `replay`

### 옵션 B — 기존 트리 최대 활용
4 크레이트 유지:
1. `core`
2. `adapters`
3. `apps`
4. `schema`

단,
- `infra`는 흡수
- `adapters`에서 channel/context experimental 분리
- `apps`에서 daemon/gateway/service 제거

### 제가 권장하는 최종안
**옵션 B로 한 번 수축한 뒤, 안정화되면 옵션 A로 재편**.
이유: 현재 저장소와의 diff를 관리하기 쉽고, 동시에 목표 범위를 강제할 수 있다.

## 13. v1 핵심 인터페이스

### 단일 provider contract
- OpenAI-compatible chat/completions 또는 responses 한 가지로 고정
- Anthropic 직접 adapter는 v1.1 이후

### 단일 memory
- 장기 메모리 없음
- 각 run의 sqlite trace만 유지
- 필요 시 `summary` 테이블 정도만 추가

### 단일 tool surface
- `list_files`
- `read_file`
- `write_file`
- `replace_text`
- `search_text`
- `run_command` (allowlist 필수)

### 단일 run model
- 입력: `task`, `workspace`, `constraints`
- 출력: `plan`, `actions`, `patches`, `commands`, `result`, `trace`

## 14. 설계 불변조건
1. 저장소 단독으로 `cargo test`가 돌아야 한다.
2. 로컬 path 의존성 금지.
3. 숨은 fallback 금지.
4. 공개 capability와 실제 구현이 일치해야 한다.
5. canonical message/tool/result/event 타입은 하나만 둔다.
6. async public contract를 쓸 거면 끝까지 async로 밀고, 아니면 접는다.
7. 실험 기능은 `experimental` feature 뒤로 보낸다.

## 15. 즉시 실행해야 할 리팩터링 순서

### P0 — 반드시 먼저
1. `coclai` 로컬 path 의존성 제거
2. `agent_coclai`를 내부 최소 에이전트 런타임으로 대체 또는 별도 optional feature로 격리
3. `channel_*`, `gateway`, `daemon`, `service`, `skills`, `metrics_http`, `cron`를 기본 빌드에서 제외
4. `README`와 `DEPLOYMENT`를 실제 capability 기준으로 수정
5. `cargo test --workspace` 기준 green 확보

### P1 — 구조 단순화
1. `apps`의 flat/duplicated CLI 구조 정리
2. `infra` 흡수
3. `schema` 유지 여부 결정
4. `runtime_compose`를 `provider + tools + workspace_policy` 중심으로 축소
5. `tool_browser`, `tool_composio`, `tool_delegate` 제거 또는 labs 이동

### P2 — 완성도 강화
1. replayable trace
2. deterministic patch application
3. golden tests for task execution
4. benchmark를 “에이전트 작업 완료율/수정 정확도” 중심으로 재정의

## 16. 실제 디렉터리 목표 예시

```text
crates/
  core/
    src/
      intent.rs
      policy.rs
      event.rs
      effect.rs
      state.rs
      reducer.rs
      projection.rs
      lib.rs
  runtime/
    src/
      config.rs
      provider.rs
      provider_openai.rs
      tools.rs
      tool_fs.rs
      tool_exec.rs
      sandbox.rs
      trace.rs
      runner.rs
      lib.rs
  cli/
    src/
      main.rs
      command.rs
      doctor.rs
      replay.rs
```

## 17. 이 설계가 AxonRunner 철학과 맞는 이유
- **정확성**: 범위를 줄이면 실패 모드가 줄어든다.
- **제어 가능성**: 워크스페이스/툴/명령 경계를 엄격히 관리할 수 있다.
- **회복 가능성**: trace/replay/patch log로 복구가 쉬워진다.
- **본질 집중**: “작업을 끝낸다”라는 하나의 책임에 수렴한다.

## 18. 자체 피드백

### 잘한 점
- 현재 저장소의 강점(core, docs, contract)을 버리지 않고 살렸다.
- ZeroClaw의 좋은 추상화만 좁게 차용하는 방향으로 정리했다.
- “멀티채널 플랫폼”이 아니라 “단일 목적 실행기”로 목적을 고정했다.

### 약한 점
- 실제 각 파일의 세부 코드까지 전부 line-by-line 독해한 것은 아니다. 핵심 실행 파일과 설계 문서는 내용 기준으로 읽었고, 나머지 다수 파일은 파일명/역할/문맥/연결 문서 기준으로 분류했다.
- 따라서 다음 단계에서는 반드시 **코드 레벨 audit pass**를 추가해야 한다.

### 다음 가장 좋은 작업
1. 현재 repo를 그대로 clone
2. `cargo metadata` / `cargo test --workspace` / `cargo tree`로 실제 빌드 실패 지점 확인
3. `coclai` 제거 브랜치 생성
4. v1 범위 밖 기능을 `experimental`로 격리
5. CLI 단일 run path를 먼저 green으로 만들기
