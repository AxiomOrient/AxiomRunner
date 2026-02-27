# axiom_apps Crate Improvements (개선점 및 최종 검토)

`crates/apps` 폴더를 워크스페이스(Workspace) 관점에서 다른 크레이트(`core`, `adapters`, `schema` 등)와의 관계를 고려하여 중립적인 시각에서 최종 검토한 결과입니다.

## 1. 아키텍처 역할 검토 (Architectural Role)

현재 `apps` 크레이트는 헥사고날 아키텍처(Ports & Adapters)에서의 **컴포지션 계층(Composition/Integration Layer)** 혹은 주도 계층(Driving Adapter)의 역할을 정확히 수행하고 있습니다.

- `core`에서 제공하는 비즈니스 로직(AgentState, Policy, Intent)을 가져옵니다.
- `adapters`에서 제공하는 인프라스트럭처 연결(DB, API, SDK)을 가져옵니다.
- `apps`는 이를 묶어(Wiring) 사용자가 실행할 수 있는 형태(CLI, Daemon, Gateway)로 제공합니다.

**결론적으로 `apps`가 다른 크레이트들을 조합하여 활용하는 현재의 책임과 의존성 방향은 아키텍처 설계상 '정상적이고 올바른' 접근입니다.** 하지만 이를 구현하는 **내부 구조와 관리 방식에서는 개선이 필요합니다.**

## 2. 과도한 집중(Monolithic) 문제점 점검과 크레이트 분리 여부

현재 `apps` 크레이트 내부는 CLI, Daemon(백그라운드 큐), Gateway(HTTP 서버), Migration, 스킬 및 크론 스케줄링 등이 모두 섞여 있습니다. 이들에 대한 크레이트 분리 타당성을 검토한 결과는 다음과 같습니다.

### **크레이트(Crate)로 분리할 만한 후보**

1. **`gateway`(HTTP 서빙 계층)**
   - 현 상태: 간단한 커스텀 HTTP 게이트웨이를 자체 구현하고 있습니다 (`gateway.rs`, `gateway_boundary.rs`).
   - 분리 타당성 (High): 만약 향후 `axum`이나 `tokio` 생태계를 적극 활용해 대규모 웹 인터페이스로 확장한다면, 무거운 웹 프레임워크 종속성(Dependencies)이 `cli` 나 `daemon` 실행 파일에도 영향을 미치는 것을 막기 위해 `crates/gateway` (또는 `crates/server`) 등으로 독립 크레이트로 분리하는 것이 타당합니다.
2. **`daemon`(백그라운드 폴링 체계)**
   - 현 상태: 큐(Queue) 폴링, 재시도 제어 루프를 가짐 (`daemon.rs`, `daemon_loop.rs`).
   - 분리 타당성 (Medium): `core`의 Agent Loop만 집중 구동하는 특성이 있으므로 분리할 수 있으나, 현재 수준에서는 `apps` 내부의 별도 모듈로 유지해도 크게 무관합니다.
3. **`migration`(버전 및 스키마 데이터 마이그레이션)**
   - 현 상태: 구버전 데이터를 신버전으로 변환 (`migrate_*.rs`).
   - 분리 타당성 (High): 런타임에 필수적이지 않은 로직이 애플리케이션 크레이트에 혼합되어 있습니다. 유지보수와 실행 파일의 경량화를 위해 `crates/migration` 처럼 별도의 도구(Tooling) 크레이트로 완전히 분리하는 것이 좋습니다.

## 3. 파일 트리 구조 개선안 (File Tree & Internal Modularity)

크레이트 분리 이전이라 할지라도, **단일 `apps` 크레이트 안에서 50개가 넘는 파일이 `src/` 최상단에 평면적(Flat)으로 나열된 것은 심각한 유지보수 저하를 유발합니다.**
우선적으로 애플리케이션 내 도메인을 디렉터리 기반으로 계층화(Hierarchical Structure)해야 합니다.

**권장 디렉터리 구조 리팩터링:**

```text
src/
 ├── cli/          # cli_args.rs, cli_command.rs, cli_runtime.rs, doctor.rs 등
 ├── daemon/       # daemon.rs, daemon_loop.rs, daemon_supervisor.rs 등
 ├── gateway/      # gateway.rs, gateway_boundary.rs, gateway_signature.rs 등
 ├── migrate/      # migrate_*.rs 모음 (이후 별도 크레이트로 추출 용이)
 ├── channels/     # channel.rs, channel_serve.rs
 ├── engine/       # runtime_compose.rs, runtime_compose_bridge.rs, agent_loop.rs
 └── utils/        # parse_util.rs, dev_guard.rs, hex_util.rs 등
```

## 4. 최종 개선 로드맵

1. **내부 모듈화 (단기 진행)**
   - `apps` 내의 53개 소스 파일들을 위 권장 구조와 같이 `cli/`, `daemon/`, `gateway/`, `migrate/` 등의 디렉터리로 묶어 네임스페이스를 정리합니다. 이 작업 자체만으로도 복잡도가 대폭 낮아집니다.
2. **크레이트 분할 (중장기 점진 진행)**
   - `migrate` 관련 기능부터 우선적으로 `crates/tools/migrate` 등으로 분리하여 메인 런타임(apps)을 가볍게 만듭니다.
   - 추후 `gateway`가 자체 HTTP 파싱을 넘어 `axum` 등 표준 프레임워크를 연동하게 되는 시점에 `crates/gateway`로 크레이트를 분리합니다.
3. **CLI 명령어 파서 교체**
   - 수십/수백 줄의 직접 작성한 파싱 로직(`cli_command.rs`)을 `clap` 라이브러리로 대체하여 표준적이고 안정적인 파싱 체계로 전환합니다.
