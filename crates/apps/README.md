# axonrunner_apps

AxonRunner 프로젝트의 핵심 **컴포지션 및 실행 애플리케이션(Composition & Execution Application)** 크레이트입니다.  
이 크레이트는 다른 핵심 크레이트들(`core`, `adapters`, `schema`)의 요소들을 결합하여, 사용자가 실제로 구동하고 상호작용할 수 있는 엔드포인트(CLI, Daemon, Gateway)를 제공하는 역할을 담당합니다.

## 시스템 내 역할 (Architectural Role)

이 곳은 비즈니스 로직(`core`)이나 인프라 연동망(`adapters`)이 직접 선언되는 곳이 아니라, **기존의 부품들을 모아 완전한 애플리케이션으로 묶어주는 주도 계층(Driving Adapter)**입니다.

- `core` 크레이트: 인텐트(Intent), 폴리시(Policy), 에이전트 상태 등 근본 데이터 구조 및 의사결정 로직 제공
- `adapters` 크레이트: 프로바이더(언어 모델), 디스코드/슬랙, 메모리 스토어 등을 위한 추상화와 구현체 제공
- **`apps` 크레이트**: 위 두 세계를 연결(Wiring)하여 `runtime_compose`를 거쳐 CLI 커맨드나 데몬 기반의 트리거로 실제 에이전트를 가동합니다.

## 주요 기능 그룹 (Key Features)

현재 이 크레이트는 편의성 유지를 위해 여러 애플리케이션 모드를 단일 바이너리로 통합 처리합니다.

1. **CLI 엔진 (`cli_*.rs`)**
   - 사용자 명령어를 해석하고 로컬 환경에서 에이전트 런타임을 구동하거나 시스템 상태를 확인(`doctor`, `status`)합니다.
2. **Daemon 루프 (`daemon.rs`, `cron.rs`)**
   - 백그라운드에서 동작하며, 작업 큐를 폴링(Polling)하고 스케줄된 크론(Cron) 작업이나 자동 복구를 처리합니다.
3. **HTTP Gateway (`gateway.rs`)**
   - 자체적인 간단한 HTTP 레이어를 통해 외부의 Webhook 스펙을 수신하고 시그니처를 검증하여 `core` 모듈의 인텐트(Intent)로 변환해 내부 시스템에 전달합니다.
4. **Composition 레이어 (`runtime_compose.rs`, `agent_loop.rs`)**
   - 프로바이더, 메모리 시스템, 툴 호출, 채널 라우터 등을 연결하여 실제적인 인텐트 실행 컨텍스트(Execution Context)를 조립합니다.

## 구조 개편 방향성 (Future Organization)

현재 모든 도메인의 로직이 `apps/src/` 아래에 평면적(Flat)으로 위치해 있습니다. 향후 앱 복잡도가 커질 것을 대비해 다음과 같은 구조 개편이 계획되어 있습니다:

1. **내부 모듈 공간 구축 (모듈화)**: `src/cli/`, `src/daemon/`, `src/gateway/`, `src/migrate/` 등 하위 디렉터리를 두어 책임을 그룹화할 예정입니다.
2. **부분 크레이트 독립**: 이 크레이트에 종속되어 있는 구버전 지원용 마이그레이션 모듈(`migrate_runner.rs` 등)이나 독립적인 HTTP 서빙을 위한 `gateway` 모듈은 향후 의존성 경량화를 위해 별도의 크레이트로(e.g., `crates/gateway`) 추출하는 것을 권장합니다.
