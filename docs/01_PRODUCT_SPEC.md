# 01. Product Specification

## 1. 제품 한 줄 정의

AxonRunner는 **하나의 로컬 워크스페이스 작업을 정확하게 끝내는 에이전트 런타임**이다.

핵심은 “많이 하는 것”이 아니라 “끝까지 맞게 하는 것”이다.

## 2. 왜 지금 재정의가 필요한가

현재 AxonRunner 저장소는 다음 강점을 이미 가진다.

- event-sourcing 기반 `core`
- 문서/게이트/리허설/벤치 규율
- `contracts.rs` 중심의 adapter 경계
- capability 정합화 작업을 진행한 `IMPLEMENTATION-PLAN.md`와 `TASKS.md`

반면 제품 표면은 여전히 넓다.

- multi-channel
- gateway signing
- daemon/service mode
- cron
- browser/composio/delegate
- multi memory backend
- integrations catalog
- metrics HTTP

이 넓은 표면은 “실제로 잘 동작하는 제품”을 만드는 데 방해가 된다. v1 제품은 범위를 강하게 줄이고, **핵심 경로의 정확성/재현성/회복성**을 release 기준으로 삼아야 한다.

## 3. 사용자 문제

대상 사용자는 다음 문제를 갖는다.

1. 코드 저장소나 워크스페이스에서 반복적인 유지보수/수정/검증 작업을 해야 한다.
2. 단순 채팅형 요약이 아니라, 실제 파일 변경과 검증 실행이 필요하다.
3. 결과가 왜 그렇게 나왔는지 trace와 evidence가 필요하다.
4. 실패했을 때 어디서 실패했는지 즉시 진단 가능해야 한다.
5. 범용 플랫폼보다, **한 작업을 안전하게 끝내는 도구**가 필요하다.

## 4. v1 제품 약속

AxonRunner v1은 아래를 반드시 만족한다.

### 4.1 Inspect

- workspace root를 기준으로만 동작한다.
- `.gitignore`와 명시적 include/exclude 규칙을 존중한다.
- 파일 인벤토리, 검색, 읽기가 deterministic 하다.

### 4.2 Plan

- 실행 전 최소한의 계획을 명시적으로 만든다.
- 계획은 “무엇을 읽고 / 무엇을 고치고 / 무엇으로 검증할지”를 포함한다.
- 계획 없이 바로 광범위 편집에 들어가지 않는다.

### 4.3 Apply

- 파일 수정은 patch 기반으로 수행한다.
- root escape가 불가능해야 한다.
- atomic write를 사용한다.
- 적용 전/후 diff를 기록한다.

### 4.4 Verify

- 실행 가능한 allowlist 명령만 수행한다.
- timeout / output truncation / exit code / stderr 요약을 남긴다.
- 검증 실패 시 실패 사실을 숨기지 않는다.

### 4.5 Explain

- 최종 응답은 “무엇을 바꿨는지 / 무엇을 실행했는지 / 무엇이 통과/실패했는지”를 포함한다.
- 내부 실행 trace가 SQLite 또는 JSONL 형태로 남는다.
- replay 가능해야 한다.

## 5. v1 핵심 기능 완전성 정의

“핵심 기능이 완벽하게 구현된 제품”은 다음 의미다.

### 필수 기능

1. **Task session**
   - 한 번의 `run`이 하나의 추적 가능한 세션을 만든다.
2. **Workspace reading**
   - 빠르고 정확한 파일 탐색/검색/읽기
3. **Safe editing**
   - 경계 강제, atomic write, patch evidence
4. **Command verification**
   - 제한된 실행, timeout, 결과 기록
5. **Trace and replay**
   - 무엇을 했는지 다시 재생 가능
6. **Doctor**
   - 환경, backend, 권한, workspace 제약, binary presence 진단
7. **Deterministic tests**
   - mock backend + golden task corpus

### 불완전하면 안 되는 이유

이 중 하나라도 부실하면 AxonRunner는 단순한 데모가 된다. v1 release는 데모가 아니라 **작업 완료 도구**여야 한다.

## 6. 비목표

다음은 v1의 비목표다.

- 멀티채널 운영 플랫폼
- 웹훅/Gateway 서버
- 다중 장기 메모리/RAG
- 브라우저 자동화
- Composio/Delegate 계열 외부 액션 오케스트레이션
- multi-agent orchestration
- skill marketplace
- dashboard/metrics web UI

## 7. 사용자 명령 표면

v1의 사용자 명령은 세 개면 충분하다.

```bash
axonrunner run --task "..." --workspace .
axonrunner doctor --workspace .
axonrunner replay --run-id <id>
```

### `run`
- 실제 작업 수행

### `doctor`
- 환경 및 제약 검증

### `replay`
- 실행 세션 재생/분석

## 8. 성공 기준

아래가 모두 만족되면 v1은 성공이다.

1. golden task corpus 통과율이 release gate 기준을 충족한다.
2. path escape/명령 우회/hidden fallback이 없다.
3. `run`의 실패 원인이 trace에서 즉시 드러난다.
4. mock backend와 production backend가 같은 contract를 따른다.
5. README, CLI, doctor, capability 문서가 모두 일치한다.

## 9. 제품 철학

AxonRunner v1의 철학은 다음 문장으로 요약된다.

> 하나의 작업을 끝내는 데 필요한 최소면만 남기고, 그 면은 끝까지 정교하게 다듬는다.

즉,

- 범위는 좁힌다.
- 품질 bar는 높인다.
- 실패는 숨기지 않는다.
- 문서/실행/검증의 불일치를 허용하지 않는다.
