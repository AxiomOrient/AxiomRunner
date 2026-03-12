# 09. Self Review

## 1. 이번 설계의 강점

### 1.1 제품 정의가 명확하다

이번 설계는 AxonRunner를 “범용 플랫폼”이 아니라 “workspace task completion engine”으로 다시 고정했다. 이건 현재 charter와도 맞고, 실제로 완성 가능한 범위다.

### 1.2 기존 자산을 버리지 않았다

- `core`
- `schema`
- contract 사고방식
- gate/rehearsal 문화

이 네 가지는 유지하고, 복잡한 표면만 잘라냈다. 완전한 재작성보다 현실적이다.

### 1.3 `codek`를 substrate로 좁게 사용한다

`codek` 전체를 제품 정체성으로 삼지 않고, 세션/transport/hook substrate로만 사용하게 한 점이 중요하다. 이 경계가 있어야 AxonRunner가 독립 제품으로 남는다.

### 1.4 release bar가 “실행 가능성” 중심이다

문서/CLI/doctor/replay/golden corpus를 함께 묶어서 제품 품질을 정의했다. 이건 실제 유지보수에 강하다.

## 2. 남아 있는 리스크

### 2.1 `codek`와 `codex app-server`의 변화 가능성

이건 가장 큰 외부 리스크다. app-server 계층은 바뀔 수 있다. 따라서 다음이 없으면 설계가 약해진다.

- version pinning
- compatibility guard
- mock backend
- smoke test

### 2.2 현재 저장소 실제 코드와의 세부 충돌 가능성

이번 설계는 저장소 구조와 문서를 근거로 했고, 이전 정밀 분석 보고서를 함께 사용했다. 하지만 모든 파일을 함수 단위로 line-by-line 재검토한 것은 아니다. 따라서 구현 착수 전에 **P1 backend migration spike**는 반드시 필요하다.

### 2.3 experimental 격리 작업의 체감 난이도

현재 apps/adapters 안에 기능이 넓게 퍼져 있다. 모듈 격리는 생각보다 연결 부위를 많이 건드릴 수 있다. 그래서 `coclai -> codek`와 `run canonical path`를 먼저 잠그는 순서가 중요하다.

## 3. 일부러 하지 않은 것

### 3.1 direct provider multi-backend 강화

지금은 유혹적이지만, v1 성공과 직접 관계가 약하다.

### 3.2 browser / MCP / skills 우선 도입

좋아 보일 수 있지만 제품 핵심 경로를 분산시킨다.

### 3.3 대규모 async infra 재설계

문제의 본질은 runtime 모델의 복잡성이 아니라 제품면의 과확장이다. 먼저 제품 경계를 고정하는 것이 맞다.

## 4. 가장 중요한 후속 검증

아래 네 가지는 설계 승인 전 실제 spike로 확인하는 것이 좋다.

1. `codek`로 세션 생성/종료가 안정적인가
2. hook bridge로 AxonRunner trace에 충분한 이벤트를 얻을 수 있는가
3. `run` path 하나로 기존 apps/adapters 조합을 실제 단순화할 수 있는가
4. golden task corpus를 mock/codek 양쪽으로 동일 contract에 맞출 수 있는가

## 5. 최종 판단

이번 설계는 “완전무결”을 보장한다고 말할 수는 없다. 그런 표현은 정직하지 않다.

하지만 현재 정보와 구조를 기준으로 보면, **가장 높은 확률로 AxonRunner를 실제 제품으로 만들 수 있는 설계**는 맞다.

핵심 이유는 단순하다.

- 기존 강점을 살리고
- 외부 substrate는 좁게 차용하고
- 제품면을 줄이고
- 품질 bar를 아주 높게 잡았기 때문이다.

## 6. 권장 시작점

지금 바로 실행한다면 순서는 이게 맞다.

1. `TASK-P1-001` ~ `TASK-P1-008`
2. `TASK-P6-001` ~ `TASK-P6-005`
3. `TASK-P2-001` ~ `TASK-P2-005`
4. `TASK-P3-*` / `TASK-P4-*`
5. `TASK-P5-*`
6. `TASK-P7-*`
7. `TASK-P8-*` / `TASK-P9-*`

즉, backend 전환 → canonical run path → workspace/command hardening → trace/replay → product surface 축소 → release gate 순으로 가야 한다.
