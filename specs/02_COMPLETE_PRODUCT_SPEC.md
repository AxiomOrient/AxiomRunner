# 완성본 제품 스펙

## 1. 제품 범위

이 스펙은 **v0.1/v0.2 마일스톤 설명이 아니라**, 현재 저장소가 지향하는 **완성본 제품**을 한 벌의 계약으로 정리한 문서다.

완성본 제품의 기준은 다음이다.

- 현재 retained CLI surface를 유지한다.
- `AUTONOMOUS_AGENT_TARGET`이 요구하는 single-agent / workspace-bound / verify-before-done / eval-driven release를 실제 동작으로 닫는다.
- workflow pack은 확장 단위이지만 runtime semantics는 본체가 소유한다.

---

## 2. public interface

## 2.1 CLI surface
```text
axiomrunner_apps run <goal-file>
axiomrunner_apps status [run-id|latest]
axiomrunner_apps replay [run-id|latest]
axiomrunner_apps resume [run-id|latest]
axiomrunner_apps abort [run-id|latest]
axiomrunner_apps doctor [--json]
axiomrunner_apps health
axiomrunner_apps help
```

## 2.2 config surface

### 정식 config keys
- `profile`
- `provider`
- `provider_model`
- `workspace`
- `state_path`
- `config_file`

### env surface
- `AXIOMRUNNER_PROFILE`
- `AXIOMRUNNER_RUNTIME_PROVIDER`
- `AXIOMRUNNER_RUNTIME_PROVIDER_MODEL`
- `AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE`
- `AXIOMRUNNER_RUNTIME_STATE_PATH`

### env-only runtime knobs
- `AXIOMRUNNER_RUNTIME_MEMORY_PATH`
- `AXIOMRUNNER_RUNTIME_TOOL_LOG_PATH`
- `AXIOMRUNNER_RUNTIME_MAX_TOKENS`
- `AXIOMRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION`
- `AXIOMRUNNER_CODEX_BIN`
- `AXIOMRUNNER_EXPERIMENTAL_OPENAI`
- `OPENAI_API_KEY`

### config 규칙
- 현재 제품 경로는 `provider=codek` 또는 deterministic test path를 우선한다.
- `openai`는 opt-in experimental compatibility path이며 기본 제품 약속에 포함하지 않는다.

---

## 3. goal file contract

## 3.1 goal schema
```json
{
  "summary": "string",
  "workspace_root": "string",
  "constraints": [
    { "label": "string", "detail": "string" }
  ],
  "done_conditions": [
    { "label": "string", "evidence": "string" }
  ],
  "verification_checks": [
    { "label": "string", "detail": "string" }
  ],
  "budget": {
    "max_steps": 5,
    "max_minutes": 10,
    "max_tokens": 8000
  },
  "approval_mode": "never | on-risk | always",
  "workflow_pack": "optional relative path"
}
```

## 3.2 required validation
- `summary` non-empty
- `workspace_root` non-empty
- `constraints[].label` non-empty
- `constraints[].detail` non-empty
- at least 1 `done_condition`
- each done condition label/evidence non-empty
- at least 1 `verification_check`
- each verification check label/detail non-empty
- `budget.max_steps > 0`
- `budget.max_minutes > 0`
- `budget.max_tokens > 0`

## 3.3 constraint vocabulary
### enforced subset
- `path_scope`
- `destructive_commands`
- `external_commands`
- `approval_escalation`

### advisory-only
- 위 4개를 제외한 나머지 label

## 3.4 constraint semantics
### `path_scope`
- detail: comma-separated relative paths
- `workspace` 또는 `.`은 whole workspace
- verifier command의 실행 위치 또는 경로 사용이 해당 scope 밖으로 나가면 blocked

### `destructive_commands`
- detail: `deny`
- verifier로 분류된 command class가 destructive면 blocked

### `external_commands`
- detail: `deny`
- verifier로 분류된 command class가 external이면 blocked

### `approval_escalation`
- detail: `required`
- planned verifier command가 high-risk이면 pre-execution approval required

### operator 요구사항
- advisory constraint는 advisory라고 보이게 해야 한다.
- enforced subset은 policy code와 reason으로 보이게 해야 한다.

---

## 4. workflow pack contract

## 4.1 ownership boundary
workflow pack은 runtime semantics를 바꾸지 않는다.  
workflow pack은 아래만 제공한다.

- planner hints
- allowed tools
- verifier rules
- recommended verifier flow
- risk hints

## 4.2 pack schema
```json
{
  "pack_id": "string",
  "version": "string",
  "description": "string",
  "entry_goal": "string",
  "planner_hints": ["string"],
  "recommended_verifier_flow": ["build | test | lint | generic"],
  "allowed_tools": [
    { "operation": "string", "scope": "string" }
  ],
  "verifier_rules": [
    {
      "label": "string",
      "profile": "build | test | lint | generic",
      "command_example": "string",
      "artifact_expectation": "string",
      "strength": "strong | weak | unresolved | pack_required",
      "required": true
    }
  ],
  "risk_policy": {
    "approval_mode": "never | on-risk | always",
    "max_mutating_steps": 8
  }
}
```

## 4.3 fail-closed rule
- goal file에 `workflow_pack` 경로가 있으면 먼저 manifest를 읽고 검증한다.
- pack manifest가 깨졌으면 run은 fail-closed 해야 한다.
- pack은 새 run phase / terminal outcome / replay schema를 정의할 수 없다.

---

## 5. tool contract

## 5.1 guaranteed operations
- `list_files`
- `read_file`
- `search_files`
- `file_write`
- `replace_in_file`
- `remove_path`
- `run_command`

## 5.2 request schema 요약
| operation | required input | required rule |
|---|---|---|
| `list_files` | `path` | workspace 안 경로만 허용 |
| `read_file` | `path` | 파일만 허용, 크기 제한 |
| `search_files` | `path`, `needle`, `mode` | substring/regex만 허용 |
| `file_write` | `path`, `contents`, `append` | bounded write/append |
| `replace_in_file` | `path`, `needle`, `replacement`, `expected_replacements?` | multi-match는 expected count 일치 필요 |
| `remove_path` | `path` | workspace boundary 안만 허용 |
| `run_command` | `program`, `args` | allowlist, timeout, truncation 적용 |

## 5.3 mutation evidence schema
모든 mutation evidence는 최소 아래를 제공해야 한다.

- `operation`
- `artifact_path`
- `before_digest`
- `after_digest`
- `before_excerpt`
- `after_excerpt`
- `unified_diff`

## 5.4 tool risk model
- `low`: list/read/search
- `medium`: small write / bounded replace / ordinary command
- `high`: remove_path / large write / large replace / destructive or impactful command

---

## 6. runtime lifecycle contract

## 6.1 phases
- `planning`
- `executing_step`
- `verifying`
- `repairing`
- `waiting_approval`
- `blocked`
- `completed`
- `failed`
- `aborted`

## 6.2 terminal outcomes
- `success`
- `blocked`
- `budget_exhausted`
- `approval_required`
- `failed`
- `aborted`

## 6.3 finalize rules
### success
- verifier status == `passed`
- every declared done condition has evidence

### blocked
- `verification_weak`
- `verification_unresolved`
- `pack_required`
- policy rejection
- provider health blocked
- explicit blocked reason

### budget_exhausted
- pre-execution step budget 부족
- requested tokens > goal token budget
- elapsed minute budget 초과
- repair budget exhausted

### approval_required
- approval mode `always`
- current implementation의 `on-risk`
- `approval_escalation=required` + high-risk verifier

### failed
- provider/tool/memory failure
- unreadable verifier evidence
- verification failed
- unsupported done condition evidence
- report write failure

### aborted
- operator가 pending run을 terminal control로 닫음

---

## 7. verification contract

## 7.1 verifier derivation
- verification detail이 command로 파싱되면 direct verifier command 사용
- command로 파싱되지 않으면 fallback probe 사용
- fallback probe는 verifier strength를 `weak` / `unresolved` / `pack_required`로 남긴다
- fallback probe는 success를 주장하면 안 된다

## 7.2 verifier status vocabulary
- `passed`
- `failed`
- `verification_weak`
- `verification_unresolved`
- `pack_required`
- `skipped`

## 7.3 verifier strength rules
- `strong`: 직접적인 verifier path
- `weak`: 약한 fallback probe
- `unresolved`: 안전한 strong verifier 미구성
- `pack_required`: explicit domain pack 필요

## 7.4 verify-before-done rule
- success 전에 verification이 passed여야 한다
- done condition evidence가 모두 충족돼야 한다
- done condition 미충족이면 `false_done`으로 남아야 한다

---

## 8. persistence and evidence contract

## 8.1 state snapshot
### format
- versioned snapshot (`axiomrunner-state-v2`)
- atomic temp write + rename
- legacy migration path 없음

### state fields
- `revision`
- `mode`
- `last_intent_id`
- `last_actor_id`
- `last_decision`
- `last_policy_code`

### pending run fields
- `run_id`
- `intent_id`
- `goal_file_path`
- `phase`
- `reason`
- `approval_state`
- `verifier_state`

## 8.2 single-writer lock
- `run`, `resume`, `abort`는 `.axiomrunner/runtime.lock` 필요
- stale lock이면 1회 자동 복구 후 재시도
- `status/replay/doctor/health/help`는 lock 없이 읽기 가능

## 8.3 trace
- JSONL append-only
- trailing partial line 1개까지만 복구
- newline까지 끝난 malformed line은 corruption으로 실패
- replay summary는 false-success / false-done metrics를 제공해야 함

## 8.4 report artifacts
각 run은 최소 아래 논리 artifact를 남긴다.

- `*.plan.md`
- `*.apply.md`
- `*.verify.md`
- `*.report.md`

## 8.5 isolation artifacts
opt-in isolated worktree path에서는 추가로 남긴다.

- `*.checkpoint.json`
- `*.rollback.json`

---

## 9. operator output contract

## 9.1 doctor
반드시 아래를 보여야 한다.

- provider_state
- memory_state
- tool_state
- lock_state
- lock_path
- workspace / artifact / state / memory paths
- pending run detail
- provider health detail (`cli_bin`, `version`, `compatibility` 등)

## 9.2 status
반드시 아래를 재구성해야 한다.

- latest run id
- phase
- outcome
- reason
- approval_state
- execution_workspace
- verifier_state
- verifier_strength
- verifier_summary
- planned_steps / step_count
- artifact summary
- pending run detail

## 9.3 replay
반드시 아래를 재구성해야 한다.

- health summary
- false_success_intents
- false_done_intents
- latest_failure
- run phase/outcome/reason
- reason_code / reason_detail
- verifier_strength
- step journal
- verifier evidence
- changed_paths
- patch artifact paths
- rollback metadata

---

## 10. release contract

## 10.1 minimum evidence bundle
- `autonomous_eval_corpus`
- `fault_path_suite`
- `nightly_dogfood_contract`
- `release_security_gate`
- adapters test suite

## 10.2 release blockers
다음 중 하나라도 깨지면 release 금지다.

- retained CLI surface drift
- identity drift (`AxiomRunner` / `axiomrunner_apps` / `AXIOMRUNNER_*`)
- weak/unresolved/pack_required verifier가 success로 보임
- workspace boundary 우회 가능
- allowlist 우회 가능
- report artifact 미생성
- replay quality 저하
- rollback evidence 누락
- nightly false success 지표 양수
- current truth docs 불일치

---

## 11. non-goal

- multi-agent
- daemon / service / HTTP gateway
- cron
- channels
- skills marketplace
- broad integrations
- generalized long-term memory platform
- hidden fallback runtime

---

## 12. 완성 정의

완성본 제품은 아래가 모두 참일 때만 완성이다.

1. CLI surface는 그대로다.
2. goal/workflow pack/operator evidence 계약이 drift 없이 잠겨 있다.
3. verifier truth와 done truth가 success를 실제로 잠근다.
4. constraint enforced subset이 실제 behavior로 동작한다.
5. workspace safety와 rollback evidence가 운영 루프에서 읽힌다.
6. representative examples, nightly, release gate가 제품 약속을 반복 검증한다.
