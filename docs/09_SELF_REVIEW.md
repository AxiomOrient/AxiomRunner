# 09. Self Review

## 1. 현재 설계의 강점

### 1.1 제품 표면이 실제 코드와 가깝다

지금 AxonRunner가 실제로 약속하는 것은 `run/batch/status/health/help`와 thin alias다. 예전처럼 `agent`, `doctor`, `replay`를 문서 전면에 두지 않고 현재 구현된 표면을 기준으로 설명하게 된 점은 큰 진전이다.

### 1.2 failure semantics가 제품 수준으로 올라왔다

- provider/tool/memory 실패가 success처럼 보이지 않음
- exit code taxonomy가 분리됨
- stderr prefix가 category를 말함
- failed run도 report artifact를 남김

이 네 가지는 운영 품질에 직접 연결된다.

### 1.3 persisted state와 runtime substrate의 경계가 선명해졌다

`freeze`/`halt`/revision/facts는 state snapshot으로 남고, provider/memory/tool은 runtime substrate로 다뤄진다. 이 분리는 이후 trace/replay를 붙일 때도 유리하다.

### 1.4 tool surface가 이제 inspect/apply/verify 루프를 닫을 수 있다

list/read/search/write/replace/remove/run-command이 모두 workspace boundary 안에 있고, artifact도 남긴다. 이것만으로도 현재 제품 범위에서는 충분히 강한 최소 표면이다.

## 2. 남아 있는 리스크

### 2.1 docs 하위 일부는 아직 과거 product blueprint의 언어를 유지한다

이번에 truth surface를 많이 고쳤지만, `docs/02_*`, `docs/05_*`, `docs/06_*` 같은 과거 계획 문서는 여전히 현재 제품보다 넓은 세계를 말한다. 이 문서들은 reference/archive 성격을 더 분명히 해야 한다.

### 2.2 tool 구현은 단순하지만 아직 거친 부분이 있다

- `.gitignore`를 존중하지 않는 file inventory
- line ending / encoding preservation 정책 부재
- command stdout/stderr truncation 부재
- timeout/cancel hardening 부재

지금은 `.gitignore`, line ending, timeout/truncation은 들어갔다. 다만 non-UTF8 파일은 보존 변환이 아니라 명시적 거부 정책이므로, 대형 workspace와 mixed-encoding 환경에 대한 추가 설계는 남아 있다.

### 2.3 trace/replay/doctor는 아직 비대칭이다

현재 제품은 `replay` 최소 요약 명령까지는 들어갔다. 하지만 `doctor`는 아직 없고, `replay`도 JSONL trace 기반의 최소 summary 수준이다. 따라서 문서는 현재 표면만 말하고, richer replay/doctor를 이미 있는 것처럼 쓰면 안 된다.

### 2.4 codek substrate 변화 리스크는 계속 남아 있다

`codek`/`codex app-server`는 외부 변화 지점이다. 지금은 health/compatibility 쪽을 어느 정도 올렸지만, 장기적으로는 더 강한 compatibility probe와 opt-in smoke가 필요하다.

## 3. 일부러 하지 않은 것

### 3.1 OpenAI 경로를 first-class provider로 키우지 않았다

현재 제품 정체성은 local workspace automation이다. 그래서 `openai`는 experimental fallback으로 낮추는 편이 더 정직했다.

### 3.2 schema-locked trace DB와 full replay를 한 번에 도입하지 않았다

그 작업은 blast radius가 크고, 지금 단계에서는 report artifact + JSONL trace + minimal replay만으로도 제품 핵심 경로를 설명할 수 있다.

### 3.3 tool abstraction을 플랫폼화하지 않았다

tool contract는 넣었지만, browser/MCP/delegate 같은 넓은 추상화는 의도적으로 넣지 않았다. 현재는 좁고 강한 local tool surface가 맞다.

## 4. 다음에 가장 중요한 검증/구현

우선순위는 이 순서가 맞다.

1. `.gitignore` 존중 inventory와 search hardening
2. command timeout / truncation / cancel hardening
3. report artifact를 더 machine-readable 하게 고정할지 결정
4. docs archive/reference 층을 현재 product truth와 더 분리

## 5. 최종 판단

지금 AxonRunner는 “큰 플랫폼”은 아니다. 대신 현재 retained surface 안에서는 꽤 높은 truth density를 가진다.

강점은 분명하다.

- 표면이 좁다
- 실패가 숨지 않는다
- 상태가 남는다
- artifact가 남는다
- provider 위상이 분명하다

남은 과제도 분명하다.

- workspace hardening 추가
- command hardening 추가
- 오래된 문서층 정리

즉, 지금의 위험은 “무엇을 해야 할지 모른다”가 아니라, “남은 거친 부분을 어떤 순서로 다듬을지”의 문제다. 그 점에서는 이미 제품 수렴 단계에 들어왔다고 볼 수 있다.
