# 01. Current Status

## 1) 확인된 기준선

- 커밋 `c47e77d`는 실제 커밋으로 존재하며 제목은 **`Remove legacy planning and deployment docs`** 이다.
- 현재 저장소의 `dev` 브랜치 루트는 `crates/`, `docs/`, `plans/`, `README.md`, `CHANGELOG.md`, `Cargo.toml` 로 구성된다.
- 현재 워크스페이스는 `crates/apps`, `crates/core`, `crates/adapters` 3크레이트다.
- `docs/`에는 현재 제품면 문서로 `CAPABILITY_MATRIX.md`, `CODEK_RUNTIME_CONTRACT.md`, `RUNBOOK.md`, `VERSIONING.md`, `project-charter.md` 가 있다.
- `plans/`에는 `IMPLEMENTATION-PLAN.md`, `TASKS.md`가 있고, post-board 태스크는 모두 `DONE`으로 표시되어 있다.

## 2) 현재 구현이 이미 잘하고 있는 것

### 제품면 축소
README와 charter는 AxonRunner를 **minimal event-sourced CLI runtime**으로 규정하고, 제품면을 `run`, `batch`, `doctor`, `replay`, `status`, `health`, `help`와 legacy alias(`read`, `write`, `remove`, `freeze`, `halt`)로 제한한다.

### substrate 계약 명시
`provider=codek`는 `codex-runtime 0.5.0`에 pinned 되어 있고, 최소 지원 Codex CLI를 `0.104.0`으로 문서화한다. session reuse도 `cwd`와 `model`이 같을 때만 허용한다.

### provider/memory/tool 조합
`runtime_compose`는 provider, memory, tool을 한 런에 결합하고, `plan/apply/verify/report` artifact를 남기도록 설계되어 있다.

### operator 진단성
`doctor`는 provider/memory/tool 상태, path, trace log, async host 정보를 모아 text/json으로 보여준다.

### 테스트 기반
현재 테스트 트리는 다음처럼 유지된다.
- `crates/core/tests`: domain/policy/projection/state invariant 검증
- `crates/adapters/tests`: error/memory/tool 검증
- `crates/apps/tests`: config priority, e2e CLI, release/security gate 검증

## 3) 왜 아직 “자율 에이전트 제품”은 아닌가

현재 public product surface는 여전히 **intent 중심**이다.
- `run <intent-spec>`
- `read:<key>`
- `write:<key>=<value>`
- `remove:<key>`
- `freeze`
- `halt`

이 surface는 runtime contract를 고정하는 데는 좋지만, **사용자의 목표를 끝까지 수행하는 autonomous run contract**를 직접 표현하지 못한다.

아직 공개 제품 계약으로 잠겨 있지 않은 것:
- goal spec (`무엇을 끝내야 하는가`)
- done condition (`언제 끝난 것으로 볼 것인가`)
- budget (`토큰/시간/스텝`)
- approval policy (`언제 사용자 승인이 필요한가`)
- resume/abort semantics
- run-scoped worktree/checkpoint/rollback contract
- long-horizon autonomy eval corpus

즉, 지금 AxonRunner는 **좋은 런타임 골격**이지, 아직 **완성된 자율 작업 에이전트**는 아니다.

## 4) 지금 단계의 핵심 판단

현재 `plans/TASKS.md`의 post-board hardening 작업이 모두 끝났다면, 다음 단계는 더 많은 기능을 추가하는 것이 아니다.  
다음 단계는 아래 하나로 수렴해야 한다.

> **“single-workspace autonomous execution loop”를 제품의 중심 계약으로 승격한다.**

이때 지켜야 할 원칙:
- 멀티 에이전트보다 단일 에이전트 우선
- 도구 수보다 실행 완결성 우선
- hidden fallback 금지
- 실패를 성공처럼 보이지 않게 할 것
- 모든 run은 trace / replay / report로 재현 가능해야 할 것

## 5) 유지해야 할 강점

- 좁은 제품면
- event-sourced core
- provider health / doctor / replay
- workspace boundary와 allowlist 사고방식
- evidence 중심 release discipline

## 6) 버려야 할 유혹

- “ZeroClaw/OpenClaw처럼 보이는” 넓은 플랫폼 표면
- 성급한 multi-agent orchestration
- browser / channel / daemon / cron 같은 주변부 재도입
- 프롬프트에 상태를 몰아넣는 설계
- eval 없이 autonomy를 늘리는 방식
