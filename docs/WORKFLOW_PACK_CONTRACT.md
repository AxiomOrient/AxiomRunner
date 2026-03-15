# Workflow Pack Contract

## Purpose

이 문서는 workflow pack과 adapter가 AxiomRunner 안에 들어올 때 지켜야 할
현재 계약을 고정한다.

핵심 원칙:

- AxiomRunner가 `goal`, `run`, `resume`, `abort`, `trace`, `report`, `done`의 뜻을 소유한다.
- workflow pack은 planner 힌트, 허용 도구, verifier 규칙만 제공한다.
- workflow pack은 새 run phase, terminal outcome, replay schema를 마음대로 만들 수 없다.

## Pack Shape

workflow pack manifest는 아래 필드를 가져야 한다.

- `pack_id`
- `version`
- `description`
- `entry_goal`
- `planner_hints[]`
- `recommended_verifier_flow[]`
- `allowed_tools[]`
- `verifier_rules[]`
- `risk_policy`

goal file은 선택적으로 `workflow_pack` 경로를 가질 수 있다.
경로가 주어지면 AxiomRunner는 그 manifest를 먼저 읽고 검증한다.
manifest가 깨졌으면 fail-closed 로 멈춘다.

## Allowed Tools

`allowed_tools[]`는 기존 tool contract 안의 operation만 고를 수 있다.

예:

- `list_files`
- `read_file`
- `search_files`
- `file_write`
- `replace_in_file`
- `remove_path`
- `run_command`

각 항목은 operation 이름과 허용 scope를 함께 가져야 한다.

## Verifier Rules

`verifier_rules[]`는 기존 verifier profile만 쓸 수 있다.

- `build`
- `test`
- `lint`
- `generic`

각 verifier rule은 아래를 가져야 한다.

- `label`
- `profile`
- `command_example`
- `artifact_expectation`
- `strength`
- `required`

`strength` 의미:

- `strong` — verifier command가 직접적인 검증 경로다.
- `weak` — 약한 fallback probe다. `success`로 숨기면 안 된다.
- `unresolved` — detail에서 안전한 strong verifier를 만들지 못했다.
- `pack_required` — 도메인용 explicit pack이 필요하다.

`recommended_verifier_flow[]`는 pack이 추천하는 검증 순서를 고정한다.

- `build`
- `test`
- `lint`
- `generic`

이 배열은 실제 verifier rule을 대체하지 않는다.
의미는 "보통 이 순서로 보는 것이 좋다"는 힌트다.

## Risk Policy

workflow pack은 위험도 힌트를 줄 수 있지만, 최종 승인은 AxiomRunner가 판단한다.

`risk_policy`는 아래를 가진다.

- `approval_mode`
- `max_mutating_steps`

## Ownership Boundary

workflow pack이 할 수 있는 것:

- planner 힌트 제공
- 허용 도구 범위 축소
- verifier rule 제공
- 앱/서버 같은 도메인별 기본 흐름 제안

workflow pack이 하면 안 되는 것:

- 새 terminal outcome 정의
- `status`/`replay` 출력 형식 변경
- `done` 판단 규칙 우회
- adapter마다 다른 resume/abort 의미 정의

## Adapter Authoring Boundary

user-provided adapter는 backend만 제공해야 한다.

adapter가 소유하는 것:

- provider substrate 연결
- tool execution backend
- memory backend
- health probe detail

adapter가 소유하지 않는 것:

- `run`, `resume`, `abort` phase 의미
- `success`, `blocked`, `budget_exhausted`, `approval_required`, `failed`, `aborted` outcome 의미
- `status`, `replay`, `report` schema
- verify-before-done rule
- workflow pack manifest schema

즉 adapter는 AxiomRunner 본체 semantics를 구현하는 것이 아니라,
이미 잠긴 semantics를 실행 가능한 backend로 연결하는 역할만 한다.

## Example

```text
pack_id: rust-service-basic
version: 1
entry_goal: implement one bounded Rust service task
recommended_verifier_flow:
  - build
  - test
  - lint
allowed_tools:
  - run_command within workspace
  - read_file within workspace
verifier_rules:
  - test via cargo test, required=true
  - lint via cargo clippy, required=false
risk_policy:
  approval_mode: on-risk
  max_mutating_steps: 8
```
