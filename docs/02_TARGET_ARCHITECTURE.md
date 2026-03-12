# 02. Target Architecture

## 1. 핵심 결정

목표 아키텍처는 다음이다.

> **AxonRunner = event-sourcing domain core + codek-backed execution runtime + minimal CLI**

즉,

- `core`는 계속 순수 도메인 경계로 유지
- 실행 substrate는 `codek`로 수렴
- 제품 표면은 CLI 세 개(`run/doctor/replay`)로 축소
- 나머지 채널/서비스/게이트웨이 계열은 기본 제품면에서 제거

## 2. 현재 저장소에서 유지할 자산

### 유지

- `crates/core/src/*`
- `crates/schema/src/*`
- `crates/adapters/src/contracts.rs`
- 문서/게이트/리허설 문화
- `doctor` 개념
- rollback/rehearsal 사고방식

### 축소 후 유지

- `crates/apps/src/main.rs`
- `crates/apps/src/cli_runtime*`
- `crates/apps/src/display.rs`
- `crates/adapters/src/memory_sqlite.rs` → trace store 쪽으로 재배치

### 실험 격리 또는 제거

- `channel_*`
- `gateway/*`
- `daemon/*`
- `service.rs`
- `cron.rs`
- `metrics_http.rs`
- `skills*`
- `tool_browser.rs`
- `tool_composio.rs`
- `tool_delegate.rs`
- `memory_axiomme.rs`
- `context_axiomme.rs`
- `memory_hybrid.rs`

## 3. 권장 crate 구조

최종 구조는 아래가 가장 좋다.

```text
crates/
  axonrunner-core/
  axonrunner-runtime/
  axonrunner-cli/
  axonrunner-schema/
  axonrunner-experimental/   # 선택적, 기본 build 제외
```

### 3.1 `axonrunner-core`

책임:

- intent
- decision
- effect
- event
- projection
- reducer
- state
- validation
- policy

절대 하지 않을 것:

- 환경 변수 읽기
- 파일 IO
- 프로세스 실행
- 네트워크 호출
- backend 연결

### 3.2 `axonrunner-runtime`

책임:

- backend 추상화 (`CodekBackend`, `MockBackend`)
- workspace scan / file search / edit / command verify
- trace 저장
- 세션 orchestration
- doctor probes
- replay loader

### 3.3 `axonrunner-cli`

책임:

- `run`
- `doctor`
- `replay`
- stdout/stderr UX
- exit code contract

### 3.4 `axonrunner-schema`

책임:

- config merge rules
- config file schema
- CLI/env/workspace config precedence
- compatibility/migration hints

### 3.5 `axonrunner-experimental`

책임:

- channels
- browser/composio/delegate
- gateway/daemon/service
- 그 외 future playground

기본 release artifact에는 포함하지 않는다.

## 4. 현재 워크스페이스에서의 점진적 수렴 전략

한 번에 crate를 전부 갈아엎지 않는다.

### 단계 1

현재 구조 유지:

- `core`
- `schema`
- `adapters`
- `apps`
- `infra`

다만,

- `coclai` 제거
- legacy `agent_*` layer 제거
- `run` 경로를 canonical path로 고정
- 비핵심 기능 feature-gate

### 단계 2

- `apps + adapters`의 핵심 실행 경로를 `runtime` crate로 추출
- `infra` 흡수
- `experimental` 분리

### 단계 3

- 제품 릴리즈는 `core/runtime/cli/schema`만 유지

## 5. 실행 상태 기계

`run`은 아래 상태 전이를 가진다.

```text
TaskAccepted
  -> WorkspaceScanned
  -> PlanDrafted
  -> PlanConfirmedInternally
  -> ChangesApplied
  -> VerificationStarted
  -> VerificationPassed | VerificationFailed
  -> FinalResponsePrepared
  -> RunCompleted | RunFailed
```

각 전이는 event로 남아야 하며, projection으로 세션 요약을 만들 수 있어야 한다.

## 6. 핵심 runtime 내부 모듈

```text
runtime/
  src/
    backend/
      mod.rs
      codek.rs
      mock.rs
    workspace/
      scan.rs
      read.rs
      search.rs
      write.rs
      patch.rs
      policy.rs
    command/
      allowlist.rs
      exec.rs
      capture.rs
    trace/
      event_log.rs
      sqlite.rs
      jsonl.rs
      replay.rs
    session/
      run.rs
      doctor.rs
      replay.rs
      summary.rs
    prompt/
      system.rs
      task.rs
      context.rs
    error.rs
    lib.rs
```

## 7. 런타임과 codek의 경계

### AxonRunner가 소유하는 것

- task contract
- event model
- policy model
- workspace boundary rules
- command allowlist
- patch validation
- trace format
- doctor / replay
- release gates

### codek가 소유하는 것

- Codex app-server transport/session lifecycle
- streamed event ingestion
- hook lifecycle integration
- backend-specific protocol adaptation

## 8. 데이터 저장 모델

v1은 단순하게 간다.

### SQLite tables

- `runs`
- `events`
- `artifacts`
- `commands`
- `file_patches`
- `final_reports`

### JSON payload 원칙

- 각 row는 최소한의 indexed column + `payload_json` 형태
- schema evolution은 additive first
- replay는 DB만으로 가능해야 한다

## 9. 파일 수정 모델

파일 수정은 아래 순서로만 허용한다.

1. read current content
2. derive patch
3. validate path + size + line ending policy
4. write temp file
5. fsync/rename atomic replace
6. diff 기록

이 흐름을 벗어나는 직접 overwrite는 금지한다.

## 10. 명령 실행 모델

명령은 아래 제약을 따른다.

- executable basename allowlist
- `shell = false`
- cwd는 workspace root 또는 하위만 허용
- timeout 강제
- stdout/stderr size cap
- env pass-through 최소화

## 11. observability 모델

v1에서 필요한 관측성은 많지 않다. 대신 핵심을 정확히 남긴다.

### 반드시 남길 것

- run id
- backend type/version
- workspace root
- prompt digest
- 주요 단계 event
- file patch summary
- command summary
- final outcome

### 남기지 않아도 되는 것

- 화려한 dashboard
- 실시간 metrics 서버
- 분산 tracing stack

## 12. target file mapping

| 현재 위치 | target fate |
|---|---|
| `crates/core/src/*` | 유지 |
| `crates/schema/src/*` | 유지 |
| `crates/adapters/src/contracts.rs` | 유지, runtime contract basis |
| `crates/adapters/src/agent_*` | 제거, runtime/provider path로 흡수 |
| `crates/adapters/src/provider_*` | 제품 기본면에서는 축소 또는 experimental |
| `crates/adapters/src/memory_sqlite.rs` | trace/sqlite로 재배치 |
| `crates/adapters/src/tool_*` | fs/exec 핵심만 runtime으로 이동 |
| `crates/apps/src/main.rs` | 유지 |
| `crates/apps/src/runtime_compose.rs` | runtime/backend + workspace policy로 분해 |
| `crates/apps/src/doctor.rs` | 유지, runtime probe 기반으로 재작성 |
| `crates/apps/src/channel*` | experimental |
| `crates/apps/src/daemon*` | experimental |
| `crates/apps/src/gateway*` | experimental |
| `crates/apps/src/service.rs` | experimental |
| `crates/apps/src/metrics_http.rs` | 제거 또는 experimental |
| `crates/infra/*` | 단계적으로 runtime 또는 core로 흡수 |

## 13. 최종 설계 평가 기준

이 아키텍처는 아래를 만족해야 한다.

1. 기본 build가 실제 제품 capability와 일치한다.
2. `run` 경로는 하나이며, 숨은 fallback이 없다.
3. backend 교체는 contract로 제한된다.
4. event/replay/doctor가 first-class 기능이다.
5. 비핵심 기능은 제품면을 오염시키지 않는다.
