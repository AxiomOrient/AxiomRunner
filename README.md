# AxonRunner Product Blueprint

이 문서 묶음은 **현재 AxonRunner Option C 정합화 작업 이후**, 실제 제품으로 수렴시키기 위한 설계/실행 계획 세트다.

기준 원칙은 하나다.

> **단순해 보이는 제품**이 아니라, **핵심 경로가 끝까지 완전하게 동작하는 제품**을 만든다.

이 기준은 AxonRunner의 기존 charter와 일치한다. AxonRunner는 원래부터 기능 수를 목표로 하지 않고, **정확한 자동화 결과**, **제어 가능한 실행**, **빠른 회복성**을 목표로 둔다. 현재 저장소는 이미 `core / apps / adapters / infra / schema` 구조, event-sourcing 중심 core, release gate, benchmark, rollback rehearsal 문화를 갖추고 있다. 이 강점은 유지하고, 제품 표면만 좁혀야 한다.

## 문서 구성

1. `docs/01_PRODUCT_SPEC.md`
   - 제품 정의, 사용자 문제, 핵심 기능 완전성 기준
2. `docs/02_TARGET_ARCHITECTURE.md`
   - 목표 아키텍처, crate/모듈 경계, 현재 저장소에서의 이동 계획
3. `docs/03_CODEK_INTEGRATION_RFC.md`
   - `coclai -> codek` 전환 설계, 위험, 인터페이스, 단계별 교체 전략
4. `docs/04_CAPABILITY_MATRIX.md`
   - 포함/제외/실험 기능 매트릭스
5. `docs/05_IMPLEMENTATION_MASTER_PLAN.md`
   - A to Z 제품화 계획
6. `docs/06_TASKS_BACKLOG.md`
   - 파일 단위에 가까운 구체 태스크 보드
7. `docs/07_LIBRARY_AND_SOURCE_RECOMMENDATIONS.md`
   - 추가 라이브러리, 외부 소스 차용 추천, 라이선스/주의점
8. `docs/08_VALIDATION_RELEASE_RUNBOOK.md`
   - 검증, 품질 게이트, release/rehearsal/runbook
9. `docs/09_SELF_REVIEW.md`
   - 설계 자체에 대한 비판과 후속 검증 포인트

## 이번 문서 세트의 최종 결론

### 1. `coclai`는 `codek`로 대체한다

다만 저장소 이름은 `codek`이지만 실제 공개 크레이트 이름은 `codex-runtime`이다. 따라서 Cargo 의존성은 아래 두 방식 중 하나로 잡는다.

```toml
# crates.io 사용
codex_runtime = { package = "codex-runtime", version = "0.4.0" }

# 또는 Git 고정 사용
codex_runtime = { package = "codex-runtime", git = "https://github.com/AxiomOrient/codek", tag = "v0.4.0" }
```

실제 코드에서는 `codex_runtime` crate 이름을 그대로 사용하는 편이 가장 혼동이 적다.

### 2. v1 제품은 “작은 제품”이 아니라 “핵심 경로가 완전한 제품”이다

핵심 경로는 아래 다섯 단계다.

1. 워크스페이스를 정확하게 읽는다.
2. 명시적 계획을 세운다.
3. 안전하게 파일을 수정한다.
4. 제한된 명령을 실행해 검증한다.
5. 근거와 trace를 남기고 종료한다.

### 3. AxonRunner는 `event-sourcing core + codek execution substrate` 조합으로 간다

즉,

- **AxonRunner가 소유할 것**: intent/policy/effect/event/projection/trace/doctor/replay/product contract
- **codek가 맡을 것**: local Codex app-server와의 연결, 세션 수명주기, 이벤트 스트리밍, 훅 계층

이렇게 해야 AxonRunner의 철학과 codek의 실전 활용성을 동시에 얻는다.

### 4. 멀티채널/게이트웨이/데몬은 기본 제품면에서 제거한다

현재 저장소의 Option C 작업은 문서와 capability 정합화를 많이 진전시켰다. 하지만 실제 제품화에서는 여전히 범위가 넓다. 제품 build는 `run / doctor / replay`만을 보장해야 한다.

## 지금 당장 가장 먼저 해야 할 일

1. `crates/adapters/Cargo.toml`에서 `coclai` path 의존성 제거
2. `crates/apps`와 `crates/adapters`에서 legacy `agent` 표면 제거
3. provider/memory/tool만 남는 runtime contract로 단순화
4. `channel_* / gateway / daemon / service / cron / skills / metrics_http / browser/composio/delegate`를 `experimental`로 격리
5. `run` 경로를 golden task corpus로 고정 검증

## 산출물 사용 순서

1. `01_PRODUCT_SPEC.md`부터 읽고 품질 bar를 확정한다.
2. `03_CODEK_INTEGRATION_RFC.md`로 `coclai -> codek` 전환을 먼저 끝낸다.
3. `05_IMPLEMENTATION_MASTER_PLAN.md`와 `06_TASKS_BACKLOG.md` 순서대로 실행한다.
4. 각 머지 전 `08_VALIDATION_RELEASE_RUNBOOK.md` 기준으로 게이트를 통과시킨다.
