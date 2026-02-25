# axiomAi 발전 로드맵 & 태스크 목록

> 작성일: 2026-02-21
> 기반: AXIOM_ANALYSIS_REPORT.md 전문가 토론 결과

---

## 비전

> "당신의 AI 에이전트가 무엇을 했고, 왜 했고, 어떻게 되돌릴 수 있는지 항상 알 수 있다."

axiomAi = **감사 가능하고, 정책 통제되며, 다중 프로바이더를 지원하는 자동화 AI 에이전트 런타임**

---

## Phase 1: Functional — "실제로 동작하게" (2주 목표)

### T1 [P0] agent_loop.rs — 실제 LLM 호출 연결

**문제**: `project_agent_response()`가 에코만 반환
**파일**: `apps/src/agent_loop.rs`

```
현재:
fn project_agent_response(...) -> String {
    format!("reply provider={provider} model={model} temp={temperature:.2} text={input}")
}

목표:
fn project_agent_response(provider: &str, model: &str, input: &str, temperature: f64) -> String {
    let provider_impl = build_provider(provider).or_else(|_| build_provider(DEFAULT_PROVIDER_ID))?;
    let request = ProviderRequest::new(model, input);
    provider_impl.complete(&request).map(|r| r.content).unwrap_or_else(|e| format!("error: {e}"))
}
```

**완료 기준**:
- [ ] `axiom agent --message "Hello"` 실제 LLM 응답 반환
- [ ] `OPENAI_API_KEY` 환경변수로 실제 OpenAI 호출
- [ ] `OLLAMA_BASE_URL` 설정으로 로컬 Ollama 호출
- [ ] 에러 시 명확한 메시지 출력

---

### T2 [P0] gateway_signature.rs — HMAC-SHA256 적용

**문제**: FNV1a64는 암호화 보안 없음
**파일**: `apps/src/gateway_signature.rs`

**목표**:
```rust
// 시크릿 키 + HMAC-SHA256
pub fn signature_for(method: &str, path: &str, body: &str, source_ip: &str, secret: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let canonical = format!("{}\n{}\n{}\n{}\n{}",
        method.trim().to_ascii_uppercase(),
        path.trim(), body, source_ip.trim(),
        unix_timestamp_secs()  // 재전송 방지
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC init");
    mac.update(canonical.as_bytes());
    format!("sigv2-{}", hex::encode(mac.finalize().into_bytes()))
}
```

**완료 기준**:
- [ ] `AXIOM_GATEWAY_SECRET` 환경변수로 키 설정
- [ ] 타임스탬프 포함 (±5분 허용)
- [ ] 기존 테스트 마이그레이션
- [ ] 서명 없이 요청 시 401 반환

---

### T3 [P0] provider timeout — 설정 가능하게

**문제**: 하드코딩 `Duration::from_secs(1)` — LLM 응답 불가능
**파일**: `adapters/src/provider_registry.rs:254`

```rust
// 현재
fn provider_policy() -> ProviderPolicy {
    ProviderPolicy::new(Duration::from_secs(1), 0, Duration::from_millis(0))
}

// 목표
fn provider_policy() -> ProviderPolicy {
    let timeout_secs = env_u64("AXIOM_PROVIDER_TIMEOUT_SECS").unwrap_or(30);
    let max_retries = env_u32("AXIOM_PROVIDER_MAX_RETRIES").unwrap_or(2);
    let retry_backoff_ms = env_u64("AXIOM_PROVIDER_RETRY_BACKOFF_MS").unwrap_or(500);
    ProviderPolicy::new(
        Duration::from_secs(timeout_secs),
        max_retries,
        Duration::from_millis(retry_backoff_ms),
    )
}
```

**완료 기준**:
- [ ] `AXIOM_PROVIDER_TIMEOUT_SECS` env 지원 (기본 30)
- [ ] `AXIOM_PROVIDER_MAX_RETRIES` env 지원 (기본 2)
- [ ] `AXIOM_PROVIDER_RETRY_BACKOFF_MS` env 지원 (기본 500)

---

### T4 [P1] 실제 HTTP transport — reqwest 연동

**문제**: `RegistryTransport.send()`가 실제 HTTP 없이 provider_id만 반환
**파일**: `adapters/src/provider_openai.rs`, `adapters/src/provider_registry.rs`

**목표**: `reqwest` (blocking) 또는 OpenAI API 호환 직접 HTTP 구현

**완료 기준**:
- [ ] `openai` 프로바이더로 실제 ChatCompletion 호출
- [ ] `openrouter` 프로바이더로 실제 호출
- [ ] `ollama` 로컬 엔드포인트 호출
- [ ] HTTP 오류 코드 → `ProviderError` 정확한 매핑
- [ ] `Content-Type: application/json` + Authorization 헤더

---

### T5 [P1] `axiom ask` 커맨드 추가

**문제**: 자연어 진입점 없음
**파일**: `apps/src/cli_command.rs`, `apps/src/cli_runtime.rs`

```
# 현재 (기계적)
axiom agent --message "오늘 날씨 어때?" --provider openai

# 목표 (자연어)
axiom ask "오늘 날씨 어때?"
axiom ask --provider anthropic "코드 리뷰해줘"
```

**완료 기준**:
- [ ] `ask <message>` 파싱
- [ ] 기본 provider 사용 (config 또는 env)
- [ ] 응답을 사람이 읽기 좋은 형식으로 출력
- [ ] `--provider`, `--model` 옵션 지원

---

### T6 [P1] Slack 채널 어댑터 실제 구현

**문제**: `channel_slack.rs`가 타입만 정의, 실제 연결 없음
**파일**: `adapters/src/channel_slack.rs`

**목표**:
- Slack Bot Token (`SLACK_BOT_TOKEN`) 설정
- `channel send` → Slack 메시지 전송
- Slack RTM/Events API 수신 → Intent 변환
- Bot 멘션 감지

**완료 기준**:
- [ ] `axiom channel add slack SLACK_BOT_TOKEN=xoxb-...` 동작
- [ ] 슬랙 채널에 메시지 전송
- [ ] 슬랙 @mention → `axiom ask` 처리
- [ ] 헬스체크 (연결 상태 확인)

---

## Phase 2: Usable — "사람이 쓸 수 있게" (3주 목표)

### T7 [P1] CLI 출력 개선

**문제**: `intent id=cli-1 kind=write outcome=accepted policy=allowed effects=1`은 사람에게 불친절

**목표**:
```
# 현재
intent id=cli-1 kind=write outcome=accepted policy=allowed effects=1

# 목표
✓ fact 저장됨: foo = bar

# 또는 --json 플래그
{"id":"cli-1","kind":"write","outcome":"accepted","key":"foo","value":"bar"}
```

**완료 기준**:
- [ ] 성공 시 명확한 완료 메시지
- [ ] 실패 시 컬러 오류 메시지
- [ ] `--json` 플래그 전역 지원
- [ ] `--quiet` 플래그 (출력 억제)
- [ ] 터미널 색상 자동 감지 (NO_COLOR env 존중)

---

### T8 [P1] IntentKind 확장 — LLM 도메인 Intent

**파일**: `core/src/intent.rs`, `core/src/effect.rs`, `core/src/policy.rs`

```rust
pub enum IntentKind {
    // 기존 유지
    ReadFact { key: String },
    WriteFact { key: String, value: String },
    RemoveFact { key: String },
    FreezeWrites,
    Halt,

    // 신규: LLM 특화
    LlmPrompt {
        prompt: String,
        model: Option<String>,
        max_tokens: Option<usize>,
        temperature: Option<f64>,
        context_id: Option<String>,  // 멀티턴 대화
    },
    ToolCall {
        tool: String,
        args: std::collections::BTreeMap<String, String>,
    },
    ConversationReset {
        context_id: String,
    },
}
```

**완료 기준**:
- [ ] `LlmPrompt` Intent → 실제 LLM 호출 Effect
- [ ] `ToolCall` Intent → 도구 실행 Effect
- [ ] Policy에서 LlmPrompt 검증 (max_tokens 제한 등)
- [ ] 기존 테스트 통과

---

### T9 [P2] ReAct 에이전트 루프 구현

**문제**: 도구 사용 루프 없음 (단발성 질문-답변만 가능)
**파일**: `apps/src/agent_loop.rs` 전면 재작성

**ReAct 패턴**:
```
루프:
  1. 시스템 프롬프트 + 도구 목록 + 현재 컨텍스트 → LLM
  2. LLM 응답 파싱: Thought/Action/Observation
  3. Action이면: ToolCallIntent → 도구 실행 → Observation 추가
  4. Final Answer이면: 루프 종료
  5. MAX_TURNS 초과 시 강제 종료
```

**완료 기준**:
- [ ] `axiom ask "파일 목록 보여줘"` → 파일 도구 호출 → 결과 반환
- [ ] 최대 턴 제한 (env `AXIOM_MAX_AGENT_TURNS` 기본 8)
- [ ] 도구 실행 결과가 다음 LLM 컨텍스트에 포함
- [ ] 중간 과정 `--verbose` 모드로 출력

---

### T10 [P2] ensure_not_blank 중복 제거

**파일**: `core/src/intent.rs`, `core/src/decision.rs`, `core/src/event.rs`, `core/src/effect.rs`

**목표**: `core/src/validation.rs`로 추출

**완료 기준**:
- [ ] 4개 파일의 `ensure_not_blank` 제거
- [ ] `validation.rs` 모듈에 통합
- [ ] 기존 테스트 통과

---

### T11 [P2] MemoryAdapter 인터페이스 통일

**문제**:
- `contracts.rs::MemoryAdapter` → `put(key, Vec<u8>)`
- `memory.rs::Memory` → `store(key, &str)`

**목표**: 하나의 인터페이스 (문자열 기반 + 바이너리 기반 옵션)

**완료 기준**:
- [ ] `contracts.rs`와 `memory.rs` 인터페이스 통합
- [ ] 기존 테스트 통과
- [ ] 마이그레이션 가이드 작성

---

### T12 [P2] `axiom init` 온보딩 개선

**문제**: 현재 onboard 커맨드가 복잡하고 대화형 모드 미흡

**목표**:
```bash
$ axiom init
? 사용할 AI 프로바이더를 선택하세요: [OpenAI / Anthropic / OpenRouter / Ollama]
> OpenAI
? OpenAI API 키를 입력하세요: sk-...
✓ 설정 완료! axiom ask "안녕하세요"로 시작하세요.
```

**완료 기준**:
- [ ] 대화형 프로바이더 선택
- [ ] API 키 안전 저장 (`~/.axiom/config` 또는 OS keychain)
- [ ] 즉시 사용 가능한 상태로 완료

---

## Phase 3: Production — "실제 배포 가능하게" (4주 목표)

### T13 [P2] 실제 HTTP 서버 구현

**문제**: `gateway.rs`가 시뮬레이션만 수행 (실제 TCP 없음)

**목표**: tokio + hyper/axum 기반 비동기 HTTP 서버

```bash
axiom serve --mode=gateway --bind=0.0.0.0:8080
```

**완료 기준**:
- [ ] `POST /v1/intents` 실제 HTTP 엔드포인트
- [ ] 동시 요청 처리 (tokio runtime)
- [ ] HTTPS 지원 (rustls)
- [ ] 그레이스풀 셧다운

---

### T14 [P2] OS 데몬화

**문제**: `daemon.rs`는 틱 루프만 있고 실제 OS 데몬화 없음

**목표**:
```bash
axiom service install  # launchd/systemd 설치
axiom service start    # 실제 데몬 시작
```

**완료 기준**:
- [ ] macOS launchd plist 생성/관리
- [ ] Linux systemd unit 생성/관리
- [ ] 프로세스 격리 (PID 파일)
- [ ] 로그 회전 설정

---

### T15 [P3] 컨텍스트 창 관리

**목표**: 멀티턴 대화 시 토큰 예산 관리

```rust
pub struct ConversationContext {
    pub id: String,
    pub turns: Vec<ConversationTurn>,
    pub total_tokens: usize,
    pub max_tokens: usize,
}
```

**완료 기준**:
- [ ] 토큰 카운팅 (tiktoken 또는 추정)
- [ ] 오래된 턴 자동 압축/요약
- [ ] 중요 사실 메모리 백엔드 저장
- [ ] `context_id`로 대화 세션 추적

---

### T16 [P3] OpenTelemetry 통합

**목표**: 관찰 가능성

**완료 기준**:
- [ ] OTEL Metrics (provider_latency, intent_count 등)
- [ ] 구조화 로그 (JSON 형식)
- [ ] 분산 트레이싱 (trace_id 전파)
- [ ] Prometheus 메트릭 엔드포인트 (`/metrics`)

---

### T17 [P3] 스트리밍 응답

**목표**: LLM 응답을 실시간으로 사용자에게 전달

**완료 기준**:
- [ ] OpenAI 스트리밍 API (`stream: true`) 지원
- [ ] CLI에서 실시간 출력 (`axiom ask --stream`)
- [ ] HTTP SSE 엔드포인트 (`GET /v1/stream/{request_id}`)

---

## 우선순위 요약

```
즉시 (이번 주):
  T1 ★★★ agent_loop LLM 실제 연결
  T3 ★★★ provider timeout 조정
  T5 ★★☆ axiom ask 커맨드

단기 (2주):
  T2 ★★★ gateway HMAC 보안
  T4 ★★★ reqwest HTTP transport
  T7 ★★☆ CLI 출력 개선
  T6 ★★☆ Slack 어댑터

중기 (1달):
  T8 ★★☆ IntentKind 확장
  T9 ★★☆ ReAct 루프
  T10 ★☆☆ 코드 정리
  T12 ★★☆ 온보딩 개선
  T11 ★☆☆ 인터페이스 통일

장기 (3달):
  T13 ★★☆ HTTP 서버
  T14 ★★☆ OS 데몬화
  T15 ★★☆ 컨텍스트 관리
  T16 ★☆☆ OTEL
  T17 ★☆☆ 스트리밍
```

---

## 가장 빠른 진전: Quick Win 5가지

1. **T1 + T3** (4시간): `agent_loop.rs` + timeout 수정 → 실제로 동작하는 AI 에이전트
2. **T5** (2시간): `axiom ask "질문"` 커맨드 추가
3. **T7 일부** (1시간): 성공/실패 메시지 개선
4. **T2** (3시간): gateway 서명 보안 패치
5. **T10** (1시간): `ensure_not_blank` 중복 제거

---

*로드맵: axiomAi ToT 전문가 패널 합의 결과*
*Karpathy × Ive × 실용주의 엔지니어*
