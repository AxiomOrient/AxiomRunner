# Workflow Pack Contract

## Purpose

이 문서는 workflow pack과 adapter가 AxonRunner 안에 들어올 때 지켜야 할
최소 계약을 고정한다.

핵심 원칙:

- AxonRunner가 `goal`, `run`, `resume`, `abort`, `trace`, `report`, `done`의 뜻을 소유한다.
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
- `required`

`recommended_verifier_flow[]`는 pack이 추천하는 검증 순서를 고정한다.

- `build`
- `test`
- `lint`
- `generic`

이 배열은 실제 verifier rule을 대체하지 않는다.
의미는 "보통 이 순서로 보는 것이 좋다"는 힌트다.

## Risk Policy

workflow pack은 위험도 힌트를 줄 수 있지만, 최종 승인은 AxonRunner가 판단한다.

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
