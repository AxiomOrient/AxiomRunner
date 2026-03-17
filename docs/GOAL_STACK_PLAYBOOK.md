# Goal Stack Playbook

AxiomRunner goal authoring is easiest when you do **not** start from raw `goal.json`.

Use this 3-layer flow instead:

1. `brief`:
   one small slice with `why`, touched paths, acceptance, and verifier intent
2. `goal stack`:
   a list of slices that are independent enough to run one by one
3. generated `goal.json`:
   the machine-facing artifact that AxiomRunner actually executes

This is the practical bridge between PM-style slicing and AxiomRunner's runtime contract.

## Why This Works

Raw goals are too low-level for first authoring.
The user has to know verifier commands, done evidence, path boundaries, and budget shape up front.

The better path is:

- `WWA` from pm-skills:
  use `Why` to justify the slice and keep it valuable
- `Job Story` from pm-skills:
  use the situation/outcome framing to keep the slice user-visible
- `Test Scenario` from pm-skills:
  turn acceptance into concrete verifier commands

Mapped into AxiomRunner:

- `why` stays in the brief and stack markdown
- `summary` becomes `RunGoal.summary`
- `acceptance[]` stays in brief/stack review input
- generated `done_conditions[]`는 기본적으로 `report_artifact_exists`만 사용한다
- `verification_checks[]` becomes executable proof
- `paths[]` becomes `path_scope`

Source references:

- [AxiomRunner pack contract](WORKFLOW_PACK_CONTRACT.md)
- [pm-skills WWA](https://raw.githubusercontent.com/phuryn/pm-skills/main/pm-execution/skills/wwas/SKILL.md)
- [pm-skills job-stories](https://raw.githubusercontent.com/phuryn/pm-skills/main/pm-execution/skills/job-stories/SKILL.md)
- [pm-skills test-scenarios](https://raw.githubusercontent.com/phuryn/pm-skills/main/pm-execution/skills/test-scenarios/SKILL.md)

## Slice Rules

Use one slice only if all rules hold:

1. one main blast radius:
   paths fit in one bounded area or one tightly related set
2. one observable outcome:
   acceptance can be read without hidden intent
3. one verifier bundle:
   build/test/lint or smoke checks can prove the slice
4. low dependency fan-in:
   if the slice needs two unfinished slices first, split it again
5. short repair loop:
   when verification fails, the reason should point back to this slice, not a whole project

## Recommended Decomposition Order

For app/server work, slice in this order:

1. repo entry:
   build, install, or boot path that proves the workspace is runnable
2. contract:
   one endpoint, one UI route, one command, or one schema edge
3. state change:
   one mutation or persistence path
4. failure path:
   one validation or error branch
5. operator evidence:
   logs, report output, or replay signal needed to trust the slice

Bad slice:

- "build auth system"

Good slices:

- "add auth config parsing"
- "reject missing bearer token on one route"
- "record auth failure in report output"

## Generator

[`tools/dev/generate_goal_stack.py`](../tools/dev/generate_goal_stack.py) 는 dev helper다. generated goal은 실행 전 검토 대상이며, runtime contract를 대신 정의하지 않는다.

Supported presets:

- `rust-service`
- `node-api`
- `nextjs-app`
- `python-fastapi`

Each preset fills a default workflow pack and verifier bundle. generated done condition은 기본적으로 `report_artifact_exists` 하나만 넣는다. 더 강한 완료 기준이 필요하면 goal JSON을 직접 수정해 supported typed evidence만 써야 한다.

Example:

```bash
python3 tools/dev/generate_goal_stack.py \
  examples/goal_stacks/axiomrunner_dogfood.brief.json \
  --output-dir examples/goal_stacks/axiomrunner_dogfood
```

## Brief Shape

```json
{
  "epic": "Tighten dogfood onboarding",
  "workspace_root": ".",
  "preset": "rust-service",
  "constraints": [
    { "label": "external_commands", "detail": "deny" }
  ],
  "independence_rules": [
    "each slice must have one verifier bundle",
    "each slice must fit in one reviewable diff"
  ],
  "slices": [
    {
      "id": "spaced-global-options",
      "summary": "Accept spaced global config options",
      "why": "the docs already show this spelling and first-run friction is high",
      "paths": ["crates/apps/src", "crates/apps/tests", "README.md", "docs"],
      "acceptance": [
        "spaced and equals forms both work for global config options",
        "docs examples use supported CLI syntax"
      ],
      "verification_checks": [
        {
          "label": "config parser",
          "detail": "cargo test -p axiomrunner_apps --test config_priority"
        }
      ]
    }
  ]
}
```

## Output Shape

The generator writes:

- one `NN_<slice>.goal.json` per slice
- `GOAL_STACK.md` with the slice order, why, path scope, and verifier labels

That means operators can work from a brief, review the stack, inspect the generated goal, and only then execute it.

## Best Current Method

For this repo, the best current method is:

1. author a stack brief
2. generate atomic goals or copy a static example
3. inspect `done_conditions[]` and keep only supported typed evidence
4. run goals in order with pack-backed presets
5. adjust the brief, not raw goal JSON, when slicing is wrong

This is better than hand-writing raw goals because it keeps the editing surface small and keeps decomposition visible.
