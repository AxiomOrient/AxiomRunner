# 03. RFC: Replace `coclai` with `codek`

## 1. 결정

`coclai`는 제거하고 `codek`를 사용한다.

단, 실제 crate는 `codex-runtime`이므로 Rust 코드에서는 `codex_runtime` 이름으로 사용된다.

```toml
[dependencies]
codex_runtime = { package = "codex-runtime", version = "0.4.0" }
```

또는 초기에는 Git tag pinning으로 시작해도 된다.

```toml
[dependencies]
codex_runtime = { package = "codex-runtime", git = "https://github.com/AxiomOrient/codek", tag = "v0.4.0" }
```

## 2. 왜 바꾸는가

현재 `coclai` 경로는 다음 문제가 있다.

1. 저장소 단독 재현성 파괴
2. 로컬 path 의존성으로 clone/repro experience 손상
3. legacy agent-specific backend layer가 제품 핵심 경로와 분리되어 유지비만 만든다
4. 제품 backend로서의 contract가 저장소 내부가 아니라 외부 path에 묶여 있음

반면 `codek`는 아래 특성을 갖는다.

- 저장소/배포 크레이트가 명확함
- `Workflow`, `Client/Session`, `AppServer`의 3계층 이상 API 제공
- hook system 제공
- local Codex app-server와의 typed integration 제공
- MIT license

## 3. codek를 어떻게 쓸 것인가

### 권장 결론

**v1 canonical integration surface는 `runtime::{Client, Session}`로 간다.**

### 이유

#### `Workflow`
장점:
- 가장 빠른 교체
- smoke test용으로 좋음

단점:
- 제품 런타임에는 lifecycle 통제가 상대적으로 약함
- resume/interrupt/explicit state handling 측면에서 덜 직접적

#### `Client/Session`
장점:
- 세션 lifecycle 명시적
- 제품 수준 trace/replay/doctor에 적합
- AxonRunner가 세션 경계를 명확히 소유 가능

단점:
- `Workflow`보다 integration code가 조금 늘어남

#### `AppServer`
장점:
- approvals/events/raw JSON-RPC에 가장 가깝다
- deep integration에 유리

단점:
- protocol volatility 영향을 더 많이 받음
- 코드량 증가
- 잘못 쓰면 AxonRunner가 app-server protocol wrapper로 퇴화함

### 선택

- migration spike: `Workflow`
- production canonical path: `Client/Session`
- advanced compatibility fallback: 제한적으로 `AppServer`

## 4. 중요한 리스크

공식 문서상 `codex app-server`는 **주로 개발/디버깅용이며 예고 없이 바뀔 수 있음**으로 설명된다. 따라서 AxonRunner는 raw protocol에 직접 종속되면 안 된다.

### 대응

1. `codek` 버전 pinning
2. `CompatibilityGuard`/contract test 추가
3. raw JSON-RPC 직접 사용 최소화
4. typed API 우선
5. `MockBackend` 필수 유지
6. backend smoke test를 release gate에 포함

## 5. 목표 인터페이스

```rust
pub trait AgentBackend {
    type Error;

    fn name(&self) -> &'static str;
    fn version(&self) -> &str;

    async fn start_run(&self, req: StartRun) -> Result<RunHandle, Self::Error>;
    async fn continue_run(&self, handle: &RunHandle, input: TurnInput) -> Result<TurnOutput, Self::Error>;
    async fn close_run(&self, handle: RunHandle) -> Result<(), Self::Error>;
}
```

### `CodekBackend`

책임:

- `Client` 연결
- `Session` 생성/종료
- prompt injection
- streamed assistant output 수집
- hook bridge 설치

### `MockBackend`

책임:

- deterministic canned response
- golden test corpus
- failure simulation

## 6. 이벤트 브리지 설계

codek hook과 AxonRunner event log를 연결한다.

### hook phases to capture

- `PreRun`
- `PostRun`
- `PreSessionStart`
- `PostSessionStart`
- `PreTurn`
- `PostTurn`
- `PreToolUse`
- `PostToolUse`

### AxonRunner event mapping

| codek hook/event | AxonRunner event |
|---|---|
| session start | `BackendSessionStarted` |
| turn start | `TurnStarted` |
| assistant text partial/final | `AssistantOutputObserved` |
| tool request | `ToolRequestObserved` |
| tool result | `ToolResultObserved` |
| turn end | `TurnCompleted` |
| session close | `BackendSessionClosed` |

## 7. backend 구성 정책

### 기본값

- backend: `codek`
- mock backend: test only or explicit flag

### config example

```toml
[backend]
kind = "codek"
model = "gpt-5-codex"
workspace_write = false
approval_mode = "internal_policy"
timeout_seconds = 120
```

## 8. AxonRunner의 policy가 codek보다 먼저여야 하는 이유

codek는 강력한 substrate지만, AxonRunner 제품 정체성은 **explicit side effects**에 있다.

따라서,

- task acceptance
- write permission
- command permission
- final verification outcome

이 네 가지는 AxonRunner policy/event layer가 먼저 결정하고, codek는 그것을 실행하는 역할에 가까워야 한다.

## 9. 파일/코드 변경 계획

### 9.1 필수 변경 파일

- `crates/adapters/Cargo.toml`
- `crates/adapters/src/lib.rs`
- `crates/adapters/src/provider_registry.rs`
- `crates/adapters/src/contracts.rs`
- `crates/apps/src/runtime_compose.rs`
- `crates/apps/src/doctor.rs`
- `crates/schema/src/config.rs`
- `README.md`
- `docs/DEPLOYMENT.md`

### 9.2 의존성 예시

```toml
# crates/adapters/Cargo.toml
[dependencies]
codex_runtime = { package = "codex-runtime", version = "0.4.0" }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "process", "sync", "time"] }
tracing = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
```

## 10. 3단계 전환 순서

### 단계 A — Compile-safe replacement

목표:
- `coclai` path dependency 제거
- `codek` 의존성 도입
- `MockBackend`와 함께 build green

### 단계 B — Session-based canonical path

목표:
- `Client/Session` 기반 backend 구현
- `run` 경로를 `codek` session으로 전환
- doctor에 `codex` binary / app-server compatibility probe 추가

### 단계 C — Hook + trace integration

목표:
- pre/post phase hook capture
- file patch / command / turn output event log 기록
- replay까지 연결

## 11. 절대 하지 말아야 할 것

1. raw app-server JSON-RPC 문자열 조작을 AxonRunner 전역에 퍼뜨리지 말 것
2. `Workflow`를 영구 canonical path로 굳히지 말 것
3. `codek`를 AxonRunner 내부 everywhere dependency로 퍼뜨리지 말 것
4. `MockBackend`를 제거하지 말 것
5. backend fallback을 조용히 넣지 말 것

## 12. Done 기준

- `cargo test --workspace`에서 `coclai` 없이 green
- `doctor`가 `codex`/`codek` readiness를 진단
- `run`이 `codek` backend로 실제 세션을 열고 종료
- trace에 backend session lifecycle이 기록
- golden tasks가 mock/codek 둘 다 같은 contract를 만족
