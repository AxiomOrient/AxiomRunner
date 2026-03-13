# 04. Direction and Blueprint

## 목표

AxonRunner는 ZeroClaw처럼 넓은 플랫폼이 아니라,
**로컬 워크스페이스 자동화를 작고 정확하게 끝내는 runtime**여야 한다.

핵심은 다음 한 문장으로 요약된다.

> 입력된 intent를 workspace 경계 안에서 실행하고,
> 결과를 state / trace / evidence / replay로 남겨
> 다시 설명 가능한 자동화를 만든다.

## 제품 원칙

### 1. One product surface
유지하는 공식 표면은 아래만 둔다.

- `run`
- `batch`
- `replay`
- `status`
- `health`
- `doctor`
- `help`
- legacy alias: `read`, `write`, `remove`, `freeze`, `halt`

### 2. One canonical execution loop
모든 실행은 아래 루프를 따른다.

1. parse
2. build intent
3. evaluate policy
4. apply domain events
5. compose side effects
6. write artifacts
7. append trace
8. persist snapshot
9. replayable output

### 3. One workspace truth
- 하나의 workspace root만 진실이다.
- provider/tool/trace/artifact/state는 모두 이 workspace와 연결된다.
- workspace를 못 정하면 실행을 거부한다.

### 4. Fail closed
- provider/tool/memory/report 실패는 반드시 실패로 종료한다.
- fallback은 operator-visible 해야 한다.
- 숨은 success는 허용하지 않는다.

### 5. Explainable automation
자동화는 성공만 하면 끝이 아니다.
적어도 아래를 남겨야 한다.

- intent
- decision
- policy code
- applied steps
- changed paths
- patch evidence
- report
- replayable summary

## 구조 청사진

### `crates/core`
역할:
- intent
- policy
- decision
- effect
- event
- reducer
- projection
- validation
- audit

규칙:
- 외부 의존 최소
- deterministic
- pure domain first

### `crates/apps`
역할:
- CLI surface
- config loading
- runtime orchestration
- state persistence
- trace/replay/doctor/status

규칙:
- 제품 진실 표면의 유일한 입구
- operator UX는 여기서 완성

### `crates/adapters`
역할:
- provider
- memory
- workspace tool
- compatibility probing

규칙:
- contract-first
- fail-closed
- product needs only

## 의도적으로 하지 않을 것

- multi-channel
- daemon / service / gateway
- browser stack
- integrations catalog
- skills marketplace
- broad memory stack
- benchmark theater

## 다음 구현 루프의 정의

다음 구현 루프는 “기능을 더 붙이는 일”이 아니다.
아래 네 가지를 완전히 잠그는 일이다.

1. truth surface
2. schema consistency
3. evidence quality
4. substrate compatibility
