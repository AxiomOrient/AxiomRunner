# Axiom AI - Adapters Crate 분석 리포트

## 1. 개요

`crates/adapters`는 Axiom AI의 핵심 비즈니스 로직(Core, Apps)을 외부 시스템(LLM, 채널, 메모리 DB, 툴 등)과 연결하는 어댑터 패턴의 구현체입니다. `contracts.rs`에 정의된 핵심 트레이트를 기반으로 다양한 프로바이더의 구체적 구현을 제공하며, 각 기능별로 세분화된 모듈 구조를 가집니다.

## 2. 파일 및 모듈 분석

### 2.1 코어 컨트랙트 및 에러 규격

- **`contracts.rs`**: 모든 어댑터의 인터페이스를 정의합니다 (`ProviderAdapter`, `MemoryAdapter`, `ChannelAdapter`, `ToolAdapter`, `AgentAdapter`, `RuntimeAdapter`, `ContextAdapter`). 각 어댑터는 동일하게 Identity (`id()`)와 Health Probe (`health()`)를 제공하여 일관성 있는 D-check(의존성 검증)를 지원합니다. 아주 정갈하고 목적이 뚜렷한 설계입니다.
- **`error.rs`**: `AdapterError`와 `RetryClass`를 정의하여 어댑터에서 발생하는 에러를 중앙 집중적으로 관리합니다. Policy Violation, Retry 여부를 명확히 구분하는 점이 훌륭합니다.

### 2.2 채널 어댑터 (Channels)

- **구현체**: `channel_discord.rs`, `channel_slack.rs`, `channel_telegram.rs`, `channel_irc.rs`, `channel_matrix.rs`, `channel_whatsapp.rs`
- **분석**:
  - 대부분의 채널이 HTTP 기반 폴링 혹은 Webhook 방식을 지원하기 위해 구현되어 있습니다.
  - 각 모듈은 `channel-discord`, `channel-slack` 등 개별 Feature 플래그로 분리되어 있어 컴파일 타임에 의존성을 최적화할 수 있도록 설계된 점이 우수합니다.
- **`channel_registry.rs`**: 팩토리 패턴을 사용하여 이름(alias)으로 특정 채널 어댑터를 생성합니다. 환경변수(`read_env_required` 등)를 통해 초기화 값을 직접 주입받고 있습니다.
- **개선점**: 모든 설정값을 `std::env::var`를 통해 직접 읽고 있습니다. `crates/infra` 또는 별도의 Config 매니저를 통해 주입받는 형태로 리팩토링하면 테스트 용이성과 일관성을 더 확보할 수 있습니다.

### 2.3 메모리 어댑터 (Memory & Context)

- **구현체**: `memory_sqlite.rs`, `memory_markdown.rs`, `memory_axiomme.rs`, `context_axiomme.rs`
- **분석**:
  - **SQLite**: 빠른 로컬 RDBMS 기반의 키-밸류 저장소입니다.
  - **Markdown**: 마크다운 파일 기반의 기억 저장소로 사람이 직접 읽고 수정할 수 있는 형태를 지원합니다 (투명성 확보).
  - **AxiomMe**: Context & Memory의 시맨틱 검색(Semantic Search)과 세션 관리를 수행하는 고도화된 메모리 백엔드입니다.
- **`memory_hybrid.rs`**: Sqlite나 다른 메모리를 조합하거나, 백그라운드 Retention(보존/만료) 정책을 처리하는 로직을 포함하고 있습니다 (`run_sqlite_retention_job`).

### 2.4 툴 및 프로바이더 어댑터 (Tools & Providers)

- **구현체**: `tool_browser.rs`, `tool_composio.rs`, `tool_delegate.rs`, `tool_memory.rs`
  - 다양한 외부 기능을 에이전트가 활용할 수 있도록 규격화된 `ToolCall` 및 `ToolOutput` 형태로 변환합니다. `Composio` 등 외부 API 연동 기능이 포함되어 있습니다.
- **구현체**: `provider_openai.rs`, `provider_registry.rs`
  - OpenAI 호환 API (OpenAI, OpenRouter 등)를 사용하는 Provider. 현재 Anthropic 등에 대해서도 OpenRouter를 통한 우회 사용을 권장하고 있습니다.

### 2.5 비동기 및 에이전트 연동 (Bridge & Agent)

- **`async_http_bridge.rs`**:
  - 어댑터 계층에서 `reqwest` (비동기 및 동기)를 매끄럽게 사용하기 위한 브릿지입니다. `OnceLock`과 `tokio::runtime::Builder::new_multi_thread()`를 사용하여 전역 공유 런타임을 구성합니다.
- **`agent_coclai.rs`**:
  - 실제 LLM/에이전트 실행을 담당하는 `coclai` 라이브러리와 연동하기 위한 어댑터입니다.

---

## 3. 주요 개선 요구사항 및 권고안 (Options & Decision)

### 🚨 Problem 1: 설정 관리(Config)의 파편화 및 강결합

- **Evidence**: `channel_registry.rs`와 `provider_registry.rs`에서 `std::env::var`를 직접 호출하여 토큰이나 설정값을 가져옵니다.
- **Options**:
  1. 현재 유지: 코드는 단순하지만 환경 변수가 필수적으로 강제되며 유닛 테스트가 번거로움.
  2. Config 파라미터 구조체 주입: `build_contract_channel(config)` 형태로 변경.
- **Decision**: **Option 2 권고**. `apps` 계층이나 별도 Config 모듈에서 일괄적으로 환경변수 및 파일 설정을 파싱한 뒤, 완성된 설정 구조체를 Registry 팩토리 메서드에 주입하는 의존성 주입(DI) 형태로 변경할 것을 권장합니다.

### 🚨 Problem 2: 런타임(Tokio) 관리 파편화

- **Evidence**: `async_http_bridge.rs`는 `shared_runtime()`을 통해 Worker Thread 2개짜리 전역 비동기 런타임을 생성합니다. 반면 `agent_coclai.rs`는 `Builder::new_current_thread()`로 초기화마다 새로운 런타임을 만듭니다.
- **Options**:
  1. 각 모듈별 독립 런타임 유지 (현황).
  2. `infra` 크레이트 수준에서 단일 통합 워커 풀/런타임을 제공하고 어댑터들이 이를 참조.
- **Decision**: **Option 2 권고**. 시스템 전반에 걸쳐 백그라운드 작업, HTTP 요청, 에이전트 실행이 혼재되어 있으므로, 글로벌 리소스 매니저를 통해 Tokio Runtime의 스레드 풀을 통합 관리하는 것이 스레드 과다 생성 및 데드락 방지에 유리합니다.

### 🚨 Problem 3: 기능 특화 프로바이더의 부족

- **Evidence**: `provider_registry.rs`에 "anthropic provider uses a different API format; use openrouter" 라고 명시된 에러가 존재합니다.
- **Decision**: 장기적으로 Anthropic 네이티브 API (Messages API)를 직접 지원하는 `provider_anthropic.rs` 추가가 필요할 수 있습니다. OpenRouter 의존도를 낮출 때 유용합니다.

## 4. 총평

`crates/adapters`는 트레이트를 활용한 전형적이고 깔끔한 어댑터 패턴의 정석을 보여주고 있습니다. 특히 Error 규격을 통일하고 Retry 단계를 열거형(`RetryClass`)으로 명시적으로 설계한 점은 네트워크 및 외부 시스템 연동이 잦은 모듈에서 매우 훌륭한 설계입니다. 향후 환경 변수 파싱과 Tokio 런타임 관리만 상위 계층으로 역전(IoC)시키면 더욱 완벽한 모듈이 될 것입니다.
