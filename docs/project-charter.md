# AxiomRunner Charter

## 제품 정의

**AxiomRunner는 goal file과 workflow pack을 받아, 로컬 workspace에서 verifier-first goal run을 실행하고 `status / replay / resume / abort / doctor`로 그 상태와 증거를 운영하는 single-agent CLI runtime이다.**

공식 identity:

- 제품 이름: `AxiomRunner`
- 바이너리: `axiomrunner_apps`
- 환경 변수 prefix: `AXIOMRUNNER_`

## 아키텍처

```
Kernel (이 저장소)
  goal schema / run lifecycle / verifier-first execution / status / replay / resume / abort / doctor
  trace / report / evidence / budget / approval / terminal outcome
  workspace safety / failure propagation

Workflow Pack  (pack manifest)
  planner hints / verifier rules / approval mode

Adapter  (crates/adapters)
  provider substrate / tool execution backend / memory backend / health probe
```

Kernel이 `goal`, `run`, `resume`, `abort`, `trace`, `report`, `done`의 의미를 소유한다.
Pack과 adapter는 도메인별 실행 수단만 제공하며, 이 의미를 재정의할 수 없다.

현재 기본 경로는 general autonomous planning runtime이 아니라 verifier-first runtime이다.
pack은 planner hint를 담을 수 있지만, 현재 제품 의미를 잠그는 핵심은 verifier/evidence loop다.

## Retained CLI Surface

```
run <goal-file>
status [run-id|latest]
replay [run-id|latest]
resume [run-id|latest]
abort  [run-id|latest]
doctor [--json]
health
help
```

`resume`은 `waiting_approval` 상태 pending run을 승인 후 재개할 때만 허용된다.
`abort`는 pending run을 terminal outcome으로 닫는 전용 명령이다. rerun이 아니다.

## Crate 구조

- `crates/core` — goal/run 계약과 상태 primitive
- `crates/adapters` — provider/tool/memory substrate
- `crates/apps` — CLI runtime과 run orchestration

## Non-goals (v1)

- multi-agent orchestration
- daemon/service/gateway
- cron 스케줄링
- skills marketplace
- integrations catalog
- long-term generalized memory platform
- multi-channel messaging

## Non-negotiable Principles

- **single-agent first**: 복수 agent 없이 단일 runtime이 workspace 경계 안에서 동작한다.
- **workspace-bound execution**: provider cwd는 runtime tool workspace와 같은 경계에 묶인다.
- **verify-before-done**: `success`는 verification이 `passed`이고 done condition이 모두 증거를 가질 때만 허용된다.
- **hidden fallback 금지**: 실패는 숨기지 않고 operator-visible reason으로 올린다.
- **eval-driven release**: 자율 eval corpus, fault path suite, nightly dogfood가 release gate를 통과해야 한다.

## v1 Release Gate

아래가 모두 충족되어야 release가 가능하다.

- retained CLI surface 전체 동작
- approval / budget / resume / abort semantics가 status/replay/report에서 일관
- weak verification(`verification_weak`, `verification_unresolved`, `pack_required`)이 success처럼 보이지 않음
- plan/apply/verify/report artifact가 모든 run에 남음
- rollback metadata와 nightly dogfood evidence가 release gate에 잠김
- docs truth lock: 이 파일, RUNBOOK, CAPABILITY_MATRIX, WORKFLOW_PACK_CONTRACT, release gate 테스트가 같은 surface를 가리킴
- `release_security_gate` 테스트 통과

## 구조 규칙

- retained CLI surface에 직접 기여하지 않는 기능은 추상화하지 않고 제거한다.
- 현재 설명과 bridge/작업 문서가 다르면 이 파일과 `RUNBOOK`, `CAPABILITY_MATRIX`가 우선이다.
