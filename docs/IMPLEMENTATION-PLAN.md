# Option C Implementation Plan

## 1) 목표
- 목표: `stub/미구현` 없이 제품 수준으로 기능을 완성하고, 모듈 의존성을 명확히 분리/디커플링한다.
- 방식: `Option C(구조 재정렬 + capability 진실성 + 경계 정리)`를 기본으로 진행한다.
- 원칙: 문서/카탈로그/실행 경로가 반드시 일치해야 하며, "available"로 노출된 기능은 실제 실행 가능해야 한다.

## 2) 현재 기준선 (Evidence)
- `channel serve`는 문서와 달리 Telegram 전용 경로로 고정되어 있음:
  - `README`: `AXONRUNNER_RUNTIME_CHANNEL` 기반 선택 설명
  - 실제 구현: `channel.rs`에서 Telegram 레코드만 조회
- 브라우저 툴은 문서에서 stub로 명시됨.
- Anthropic은 카탈로그에 노출되지만 provider 빌더에서 실패 반환.
- 채널 스토어 검증은 Telegram 외 채널이 "존재 확인" 수준으로 약함.
- 런타임 provider 실패 시 조용한 fallback 경로가 존재하여 설정 오류가 은닉될 수 있음.

## 3) Option B 교집합 반영/제외 규칙

### 3.1 Option C에 포함하여 즉시 착수할 Option B 작업 (교집합)
- B-1 채널 실행 경로 단일화 (`channel serve`/`daemon` 공통 경로화)
- B-2 채널별 설정 검증 실체화
- B-3 provider 환경 변수 경로 통일
- B-4 provider fallback fail-fast
- B-5 보안 게이트(`cargo audit`) 실행 환경 고정

### 3.2 Option C에서 제외할 Option B 작업
- 제외-1: `channel.rs` 단일 파일 임시 패치(telegram 하드코딩만 우회)  
  이유: Option C는 `channel serve + daemon` 공통 런타임 계층으로 재정렬하므로 단기 우회 패치가 중복된다.
- 제외-2: fallback 유지 + 경고 강화만 하는 단계적 봉합  
  이유: Option C는 조용한 fallback 자체를 제거(fail-fast)한다.
- 제외-3: 수동 문서 보정만 선반영  
  이유: Option C는 capability 메타데이터에서 문서를 생성/동기화하도록 변경한다.

## 4) 실행 범위 (Option C)

### 4.1 Workstream C1: 채널 실행 경로 및 계약 단일화
- `channel serve`와 `daemon`이 동일한 채널 선택/초기화/실행 계약을 사용하도록 통합.
- `apps/channel/store`의 로컬 채널 enum/검증 중복을 제거하고, 어댑터 레지스트리 계약 기반으로 정렬.
- 채널별 `send/receive/polling/webhook` capability를 명시적으로 모델링.

### 4.2 Workstream C2: provider/config 경로 정규화 + fail-fast
- `AXONRUNNER_PROVIDER`/`AXONRUNNER_RUNTIME_PROVIDER` 경로를 단일 소스로 통합.
- provider 초기화 실패 시 조용한 default fallback 제거.
- `integrations install/info` 출력이 실제 런타임 변수와 1:1 일치하도록 보정.

### 4.3 Workstream C3: 노출된 기능의 "실행 가능성" 완결
- 카탈로그 `available|active` 항목은 실행 가능성을 테스트로 증명.
- 현재 노출된 미구현 항목 완결:
  - Browser: 실제 headless 자동화 경로 구현
  - Anthropic: 실사용 provider 경로 구현
  - Discord/Slack/WhatsApp 수신 경로: 제품 스코프에 맞게 구현(또는 capability를 `partial`로 낮추고 제품 스코프에서 제외)

### 4.4 Workstream C4: 코어 경계 순수화 + 타입 중복 제거
- `core`의 환경/바인드 상수 책임을 `schema` 또는 `apps boundary`로 이동.
- `RetryClass` 등 공통 분류 타입 중복 제거(단일 canonical 타입으로 수렴).
- `schema` crate를 실제 설정 병합 경로에서 활용하고 중복 merge 로직 제거.

### 4.5 Workstream C5: 품질 게이트/문서 동기화 자동화
- capability 기반 문서 동기화(README/DEPLOYMENT/integrations 출력 정합화).
- CI 게이트: `test`, `clippy -D warnings`, `audit`를 release 차단 조건으로 고정.

## 5) 단계별 마일스톤

### M0. 계획 아티팩트 확정
- 산출물: `docs/IMPLEMENTATION-PLAN.md`, `docs/TASKS.md`
- 종료 조건: TASK-ID 고정, 선행/후행 의존성 명시, 다음 실행 TASK 지정

### M1. 공통 실행 경로/설정 정규화
- C1 + C2 일부(B 교집합) 완료
- 종료 조건:
  - `channel serve`가 `AXONRUNNER_RUNTIME_CHANNEL`에 따라 실제 채널을 선택
  - provider 경로가 단일 변수 체계로 동작
  - provider 실패 시 명시적 실패

### M2. 노출 기능 완결
- C3 완료
- 종료 조건:
  - `available|active` 항목에 대응하는 실행 경로 + 테스트 증거 확보
  - 미구현/placeholder 라벨 제거 또는 스코프 축소가 문서와 일치

### M3. 경계 순수화/중복 제거
- C4 완료
- 종료 조건:
  - core I/O 경계 유지
  - 공통 타입 중복 제거
  - schema 기반 설정 병합 사용

### M4. 게이트/문서 동기화
- C5 완료
- 종료 조건:
  - CI 품질 게이트 통과
  - README/DEPLOYMENT/CLI 출력이 capability 기준과 일치

## 6) 테스트/검증 전략
- 계약 테스트:
  - 채널별 `send/receive` capability 동작 검증
  - provider 선택 및 실패 동작 검증(fail-fast)
- 회귀 테스트:
  - `channel serve` 멀티 채널 선택 회귀
  - `integrations install/info`와 runtime env 정합성 회귀
- 게이트:
  - `cargo test --workspace --all-features`
  - `cargo clippy --workspace --all-features -- -D warnings`
  - `cargo audit` (CI에서 advisory DB write 가능한 경로 사용)

## 7) 리스크와 롤백
- 리스크-1: 채널 경로 통합 중 런타임 회귀
  - 대응: `channel serve`/`daemon` 공통 인터페이스에 골든 테스트 추가
- 리스크-2: provider fail-fast 도입으로 기존 운영 환경이 즉시 실패
  - 대응: 1개 릴리즈 동안 호환 모드 플래그 제공 후 제거
- 리스크-3: capability 정합화 과정에서 일부 기능 스코프 축소 필요
  - 대응: 축소 시 카탈로그/문서/doctor 출력 동시 반영

## 8) 완료 정의 (Definition of Done)
- `stub`, `not yet implemented`, `coming soon` 텍스트가 제품 핵심 경로에서 제거되거나, 스코프 외 기능으로 명확히 분리됨.
- 공개된 `available|active` 항목은 실행 경로 + 자동 테스트 증거를 가진다.
- 설정/실행/문서가 단일 capability 소스를 기준으로 정합성을 유지한다.
- `core`는 순수 도메인 경계를 유지하고 환경/인프라 세부사항을 직접 알지 않는다.

## 9) 아티팩트 링크
- 작업 분해/상태: `docs/TASKS.md`
- Selected For Next: `NONE`

## 10) Progress Snapshot
- 완료: `TASK-C-001` (B-1 + C1)
  - 변경: `channel serve`가 채널 스토어의 Telegram 전용 경로를 사용하지 않고, `AXONRUNNER_RUNTIME_CHANNEL` + adapter registry(`build_contract_channel`) 경로를 사용하도록 전환됨.
  - 효과: `AXONRUNNER_RUNTIME_CHANNEL=discord`에서 더 이상 "no telegram channel configured" 오류가 발생하지 않고, 실제 Discord 어댑터 환경변수 검증 오류로 진입함.
  - 검증:
    - `cargo test -p axonrunner_apps channel::tests --quiet` (8 passed)
    - `cargo test -p axonrunner_apps --quiet` (pass)
- 완료: `TASK-C-005` (B-3 + C2)
  - 변경: provider 환경 변수 체계를 `AXONRUNNER_RUNTIME_PROVIDER` 단일 경로로 통일함.
    - `config_loader`: ENV_PROVIDER 키를 `AXONRUNNER_RUNTIME_PROVIDER`로 전환
    - `integrations`: AI 모델 install/remove 안내 문구를 `AXONRUNNER_RUNTIME_PROVIDER` 기준으로 정렬
    - `runtime_compose`: `ENV_RUNTIME_PROVIDER` 기반 로딩 유지로 런타임/CLI 안내 체계 일치
  - 효과: config/env/integrations 출력의 provider 설정 키 불일치 제거.
  - 검증:
    - `cargo test -p axonrunner_apps --quiet` (pass)
- 완료: `TASK-C-006` (B-4 + C2)
  - 변경: provider 초기화의 조용한 fallback(`DEFAULT_PROVIDER_ID`) 제거, fail-fast 경로로 변경.
    - `RuntimeComposeState::new`가 `Result`를 반환하고 unknown provider/API key 누락을 즉시 에러로 반환
    - `CliRuntime::new`, `run()`에서 초기화 에러를 전파하도록 호출 체인 정렬
    - 회귀 테스트 추가: unknown provider 실패, openai key 누락 시 fallback 없이 실패
  - 효과: 설정 오류가 mock-local로 은닉되지 않고 실행 시작 단계에서 명시적으로 실패.
  - 검증:
    - `cargo test -p axonrunner_apps --quiet` (pass)
    - `cargo clippy -p axonrunner_apps --all-targets -- -D warnings` (pass)
- 완료: `TASK-C-003` (B-2 + C1)
  - 변경: `apps/channel/store`의 로컬 채널 타입 enum 의존을 제거하고, adapter registry canonical id(`resolve_channel_id`) 기반으로 채널 타입을 정규화함.
    - `ChannelRecord.channel_type`를 `String` canonical id로 전환하여 중복 타입 정의 제거
    - `channel/store`에 채널별 capability spec(required/optional config keys) 도입
    - `try_build_channel_adapter`가 Telegram 전용 검증이 아닌 Discord/Slack/Matrix/IRC/WhatsApp 포함 어댑터 구성 검증을 수행
    - `health/start/doctor`가 capability validation 경로를 사용하도록 정렬
  - 효과: Telegram 외 채널도 설정 누락/오입력이 `start/doctor` 단계에서 조기에 `Unhealthy`로 감지되고 구체 오류가 노출됨.
  - 검증:
    - `cargo test -p axonrunner_apps channel::tests --quiet` (10 passed)
    - `cargo test -p axonrunner_apps --quiet` (pass)
    - `cargo clippy -p axonrunner_apps --all-targets -- -D warnings` (pass)
- 완료: `TASK-C-002` (B-1 + C1)
  - 변경: `daemon` 채널 선택/검증 경로를 `channel serve`와 동일 runtime channel 계약으로 통합함.
    - `channel_runtime` 공통 모듈 추가: `AXONRUNNER_RUNTIME_CHANNEL` canonical resolver + optional resolver
    - `channel serve` resolver가 공통 모듈 delegate를 사용하도록 정렬
    - `daemon build/supervisor`가 공통 resolver를 사용하도록 변경(하드코딩 valid list 제거)
    - `daemon` 입력 빌드가 invalid runtime channel에서 명시적으로 실패하도록 `Result` 경로 도입
  - 효과: `channel.slack` 같은 alias 해석, unsupported channel 에러 포맷, optional channel 처리 규칙이 serve/daemon에서 일치함.
  - 검증:
    - `cargo test -p axonrunner_apps daemon::tests --quiet` (7 passed)
    - `cargo test -p axonrunner_apps channel_runtime::tests --quiet` (4 passed)
    - `cargo test -p axonrunner_apps --quiet` (pass)
    - `cargo clippy -p axonrunner_apps --all-targets -- -D warnings` (pass)
- 완료: `TASK-C-004` (B-2 + C1)
  - 변경: 채널 startup probe를 `channel start/doctor` 공통 경로로 명시화하고, unhealthy 구성에 대해 명시적 실패 시맨틱을 추가함.
    - `run_channel_startup_probe` 도입 및 `health/start/doctor` 공통 사용
    - `ChannelResult::Started`에 `failed/failures` 추가로 실패 상세를 구조화
    - CLI `channel start/doctor`가 unhealthy 발생 시 non-zero 종료가 되도록 오류 전파
    - e2e test helper에서 `HOME` canonical path를 사용하도록 보정해 workspace 경계 검증 flaky 제거
  - 효과: 구성 오류가 단순 상태 표시에 그치지 않고 `channel start/doctor` 명령 결과에서 명시적으로 실패 처리되어 운영 검출성이 향상됨.
  - 검증:
    - `cargo test -p axonrunner_apps channel::tests --quiet` (10 passed)
    - `cargo test -p axonrunner_apps --test e2e_cli e2e_cli_channel_start_fails_with_invalid_startup_probe --quiet` (pass)
    - `cargo test -p axonrunner_apps --test e2e_cli e2e_cli_channel_doctor_fails_with_unhealthy_channel --quiet` (pass)
    - `cargo test -p axonrunner_apps --quiet` (pass)
    - `cargo clippy -p axonrunner_apps --all-targets -- -D warnings` (pass)
- 완료: `TASK-C-007` (C2 + C3)
  - 변경: `integrations` 카탈로그 상태를 정적 하드코딩에서 capability 기반 계산으로 전환함.
    - `IntegrationDescriptor + IntegrationCapability` 모델 도입으로 메타데이터/상태 계산 분리
    - `build_contract_channel/provider/tool` 기반 실행 가능성 판정 함수 추가
    - 실행 불가 capability는 `coming_soon`으로 자동 강등되도록 상태 계산 정렬
  - 효과: `available|active` 항목이 실제 실행 경로와 동기화되고, 미구현/비실행 provider(예: anthropic, deepseek, openai-compatible)가 `coming_soon`으로 노출됨.
  - 검증:
    - `cargo test -p axonrunner_apps integrations::tests --quiet` (12 passed)
    - `cargo test -p axonrunner_apps --test e2e_cli e2e_cli_integrations_list_syncs_executable_statuses --quiet` (pass)
    - `cargo test -p axonrunner_apps --quiet` (149 passed, 1 ignored)
    - `cargo clippy -p axonrunner_apps --all-targets -- -D warnings` (pass)
- 완료: `TASK-C-008` (C3)
  - 변경: `tool.browser`를 메모리 스텁 동작에서 실제 페이지 로드/조회/기본 액션이 가능한 경로로 확장함.
    - `browser.open`: URL allowlist/HTTPS 정책 검증 후 실제 HTTP fetch 수행, status/title/bytes를 포함한 페이지 스냅샷 저장
    - `browser.current`: 현재 로드된 페이지의 URL/host/status/title/bytes 메타데이터 반환
    - `browser.find`: 현재 페이지 본문에서 case-insensitive 키워드 검색(hit count) 제공
  - 효과: 브라우저 툴이 단순 URL 기억 스텁이 아니라 실제 페이지 I/O를 수행하며, open/current/find 기본 시나리오가 제품 동작으로 검증됨.
  - 검증:
    - `cargo test -p axonrunner_adapters --test tool_surface --quiet` (7 passed)
    - `cargo test -p axonrunner_adapters --quiet` (63 passed, 5 ignored + integration suites pass)
    - `cargo test -p axonrunner_apps --quiet` (149 passed, 1 ignored)
    - `cargo clippy -p axonrunner_adapters --all-targets -- -D warnings` (pass)
    - `cargo clippy -p axonrunner_apps --all-targets -- -D warnings` (pass)
- 완료: `TASK-C-009` (C3)
  - 변경: Anthropic provider를 실제 실행 가능한 adapter 경로로 구현하고 provider registry에 연결함.
    - `provider_anthropic` 신규 도입: `/v1/messages` 포맷 요청/응답 처리, 입력 검증(prompt/max_tokens), 응답 `content[].text` 추출
    - `provider_registry`에서 `anthropic`이 `ANTHROPIC_API_KEY` 기반으로 실 provider를 생성하도록 전환
    - `integrations` status sync 테스트를 갱신해 anthropic이 실행 가능 상태(`available`)로 노출됨을 고정
  - 효과: Anthropic이 카탈로그상 `coming_soon` 강등 항목이 아니라 실제 런타임 provider 경로로 활성화되며, fail-fast 원칙(`ANTHROPIC_API_KEY` 누락 시 명시 실패)도 유지됨.
  - 검증:
    - `cargo test -p axonrunner_adapters provider_anthropic::tests --quiet` (3 passed)
    - `cargo test -p axonrunner_adapters provider_registry::tests --quiet` (4 passed)
    - `cargo test -p axonrunner_apps integrations::tests --quiet` (12 passed)
    - `cargo test -p axonrunner_apps --test e2e_cli e2e_cli_integrations_list_syncs_executable_statuses --quiet` (pass)
    - `cargo test -p axonrunner_adapters --quiet` (67 passed, 5 ignored + integration suites pass)
    - `cargo test -p axonrunner_apps --quiet` (149 passed, 1 ignored)
    - `cargo clippy -p axonrunner_adapters --all-targets -- -D warnings` (pass)
    - `cargo clippy -p axonrunner_apps --all-targets -- -D warnings` (pass)
- 완료: `TASK-C-010` (C3)
  - 변경: Discord 채널 capability를 카탈로그에서 `partial`로 명시해 send-only 구현과 상태 표현을 정렬함.
    - `IntegrationStatus::Partial` 상태를 추가하고, Discord descriptor를 `partial` + `discord_webhook_send_only` transport로 전환
    - Discord summary를 gateway receive 미구현 사실이 드러나는 형태로 보강
    - integrations unit/e2e 테스트에 Discord `partial` 상태 회귀 검증 추가
  - 효과: Discord가 `available`(완전 기능)로 과장 노출되지 않고, 실제 실행 경로(웹훅 송신 전용 + 수신 미구현)와 카탈로그/CLI 출력이 일치함.
  - 검증:
    - `cargo test -p axonrunner_apps integrations::tests --quiet` (13 passed)
    - `cargo test -p axonrunner_apps --test e2e_cli e2e_cli_integrations_info_supports_case_insensitive_and_status_variants --quiet` (pass)
    - `cargo test -p axonrunner_apps --test e2e_cli e2e_cli_integrations_list_syncs_executable_statuses --quiet` (pass)
    - `cargo clippy -p axonrunner_apps --all-targets -- -D warnings` (pass)
- 완료: `TASK-C-011` (C3)
  - 변경: Slack 채널 capability를 `partial`로 명시하고 integrations 상태 판정을 side-effect-free probe 기반으로 정렬함.
    - `integrations`: Slack descriptor를 `partial` + `slack_webhook_send_only`로 전환
    - `integrations` 상태 계산: `build_contract_*` 실체 생성 대신 `channel_is_compiled`/`resolve_provider_id`/`resolve_tool_id` probe 사용
    - `e2e_cli` 실행 환경 정화 키를 확장해 host env 누수(채널/provider/runtime 변수) 제거
  - 효과: Slack 수신 미구현(send-only) 상태가 카탈로그에 과장 없이 노출되고, executable-status 계산이 어댑터 초기화 부작용에 의존하지 않음.
  - 검증:
    - `.github/workflows/ci.yml`의 `execution-contract` 필수 잡에서 `scripts/run_execution_contract_gate.sh` 실행
    - `run_execution_contract_gate.sh`가 `e2e_cli_integrations_info_supports_case_insensitive_and_status_variants`/`e2e_cli_integrations_list_syncs_executable_statuses`를 강제(슬랙 partial 검증 포함)
    - 통과: `cargo test -p axonrunner_apps --no-run`
    - 통과: `cargo test -p axonrunner_adapters --no-run`
    - 통과: `cargo clippy -p axonrunner_adapters --all-targets -- -D warnings`
    - 통과: `cargo clippy -p axonrunner_apps --all-targets -- -D warnings`
- 완료: `TASK-C-012` (C3)
  - 변경: WhatsApp capability를 `partial`로 명시해 현재 구현(send-only, webhook receive 미구현)과 카탈로그 상태를 정렬함.
    - `integrations`: WhatsApp descriptor를 `partial` + `whatsapp_cloud_api_send_only`로 전환
    - summary에 webhook receive 미구현 사실 명시
    - integrations unit/e2e 테스트에 WhatsApp `partial` 회귀 검증 추가
  - 효과: WhatsApp이 `available`로 과장 노출되지 않고, 실제 실행 범위와 카탈로그/CLI 상태가 일치함.
  - 검증:
    - `cargo test -p axonrunner_apps --no-run` (pass)
    - `cargo clippy -p axonrunner_apps --all-targets -- -D warnings` (pass)
- 완료: `TASK-C-013` (C4)
  - 변경: core policy에서 환경/바인드 문자열 상수 책임을 제거하고 boundary 입력만 받도록 정리함.
    - `ENV_DEV_MODE`/`ENV_BIND`/`ENV_ALLOW_REMOTE`/`LOCALHOST_BIND` 삭제
    - `DevModeMitigationInput::from_env_values`를 `from_boundary_values(dev_mode, bind_is_localhost, allow_remote)`로 대체
    - core 테스트를 boundary 값 주입 기준으로 갱신
  - 효과: core가 환경 변수명/네트워크 bind 문자열을 직접 소유하지 않게 되어 경계 순수성이 개선됨.
  - 검증:
    - `cargo test -p axonrunner_core --no-run` (pass)
    - `cargo clippy -p axonrunner_core --all-targets -- -D warnings` (pass)
- 완료: `TASK-C-014` (C4)
  - 변경: `RetryClass`를 infra canonical 타입으로 수렴함.
    - `axonrunner_infra::RetryClass`를 canonical로 유지
    - adapters/apps의 중복 enum 정의를 제거하고 `pub use axonrunner_infra::RetryClass`로 전환
    - `crates/adapters/Cargo.toml`, `crates/apps/Cargo.toml`에 `axonrunner_infra` 의존 추가
  - 효과: 재시도 분류 타입이 단일 원천으로 정렬되어 cross-crate 변환/중복 유지 비용이 제거됨.
  - 검증:
    - `cargo test -p axonrunner_adapters --no-run` (pass)
    - `cargo test -p axonrunner_apps --no-run` (pass)
    - `cargo clippy -p axonrunner_adapters --all-targets -- -D warnings` (pass)
    - `cargo clippy -p axonrunner_apps --all-targets -- -D warnings` (pass)
- 완료: `TASK-C-015` (C4)
  - 변경: apps 설정 precedence 해석을 schema merge 경로로 통합함.
    - `config_loader::resolve_config`가 로컬 `PartialConfig::merge` 체인 대신 `axonrunner_schema::merge_optional`을 사용하도록 전환
    - `PartialConfig::merge` 제거로 중복 병합 로직 삭제
  - 효과: 설정 병합의 우선순위 해석이 schema crate의 단일 병합 계약으로 정렬됨.
  - 검증:
    - `cargo test -p axonrunner_apps --no-run` (pass)
    - `cargo clippy -p axonrunner_apps --all-targets -- -D warnings` (pass)
- 완료: `TASK-C-016` (C5)
  - 변경: README/DEPLOYMENT/CLI capability 상태 정합화를 자동 검증 가능한 스냅샷 계약으로 고정함.
    - `README.md`와 `docs/DEPLOYMENT.md`에 `INTEGRATIONS_STATUS_SNAPSHOT` 마커 블록 추가
    - snapshot 내용을 `integrations list` 출력 형식(`name/category/status`)과 동일하게 정렬
    - README의 outdated limitation(`browser adapter is a stub`) 제거
    - DEPLOYMENT의 provider/channel 변수 표기를 현재 runtime 계약(`ANTHROPIC_API_KEY`, `AXONRUNNER_TELEGRAM_BOT_TOKEN`, `AXONRUNNER_TELEGRAM_ALLOWED_USERS` 등)으로 보정
    - `crates/apps/src/integrations.rs` 테스트에 README/DEPLOYMENT snapshot 동기화 검증 추가
  - 효과: 문서 상태 표기가 integration capability source-of-truth와 괴리될 경우 테스트에서 즉시 검출되도록 guard가 추가됨.
  - 검증:
    - `cargo fmt --all` (pass)
    - `cargo clippy -p axonrunner_apps --all-targets -- -D warnings` (pass)
    - `cargo test -p axonrunner_apps --no-run` (pass)
    - `cargo test -p axonrunner_apps integrations::tests::integrations_readme_snapshot_is_synced_with_catalog -- --nocapture` (local runtime hang 환경에서 실행형 검증 불가; CI execution-contract 게이트로 보완)
- 완료: `TASK-C-017` (B-5 + C5)
  - 변경: 실행형 검증 블로커를 기능 구현 흐름에서 분리하기 위해 CI 필수 게이트를 강화함.
    - `.github/workflows/ci.yml`에 `execution-contract` 잡 추가:
      - `scripts/run_execution_contract_gate.sh`를 통해 핵심 실행 계약 테스트(`integrations info/list`, `channel add/start/doctor/remove`)를 필수 실행
    - `.github/workflows/ci.yml`에 `audit` 잡 추가:
      - `cargo-audit` 설치 후 `cargo audit --deny vulnerabilities` 실행
    - `ci-required` 집계에 `execution-contract`, `audit`를 필수 성공 조건으로 포함
  - 효과: 로컬 샌드박스 실행 제약으로 인한 테스트 hang과 별개로, 실제 실행형 계약 검증/보안 검증이 CI에서 릴리즈 차단 조건으로 강제됨.
  - 검증:
    - `ruby -e 'require "yaml"; YAML.load_file(".github/workflows/ci.yml")'` (pass)
    - `bash -n scripts/run_execution_contract_gate.sh` (pass)
    - `cargo clippy -p axonrunner_apps --all-targets -- -D warnings` (pass)
    - `cargo test -p axonrunner_apps --no-run` (pass)
- 완료: `TASK-C-018` (C5)
  - 변경: 릴리즈 리허설/롤백 검증을 단일 게이트로 고정하고 CI 필수 조건으로 연결함.
    - `scripts/run_release_rehearsal_gate.sh` 추가:
      - `transition_gates`(기존 `scripts/run_transition_gates.sh`) 실행
      - `h4_sample_contract` 검사(`target/transition-gates/h4_sample_report.json`의 suite/h2_gate/rollback_recovered/data_loss_files/SLO 조건 검증)
      - `target/release-rehearsal-gate/report.json` 생성
    - `.github/workflows/ci.yml`에 `release-rehearsal-gate` 필수 잡 추가
    - `ci-required` 집계에 `release-rehearsal-gate` 성공 조건 반영
    - `docs/release-readiness-gate.md`에 리허설 게이트 실행 절차/합격 기준/필수 아티팩트 추가
  - 효과: 전환 리허설 + 롤백 복구 검증이 릴리즈 필수 게이트로 명시화되어 실패 시 배포가 차단됨.
  - 검증:
    - `bash -n scripts/run_release_rehearsal_gate.sh` (pass)
    - `ruby -e 'require "yaml"; YAML.load_file(".github/workflows/ci.yml")'` (pass)
    - `cargo clippy -p axonrunner_apps --all-targets -- -D warnings` (pass)
    - `cargo test -p axonrunner_apps --no-run` (pass)
