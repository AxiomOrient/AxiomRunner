# Goal Stack: AxiomRunner dogfood hardening

- workspace_root: `.`
- preset: `rust-service`
- generated_goals: `3`

## Independence Rules
- each slice must have a bounded path scope
- each slice must be proven by one verifier bundle
- each slice must be reviewable without reading the whole repo

## Slices

### 01. Make representative examples self-contained workspaces
- goal_file: `01_examples-self-contained.goal.json`
- why: dogfood should fail on product logic, not because example fixtures are missing
- paths: `examples,README.md,docs`
- workflow_pack: `examples/rust_service/pack.json`
- verifier_labels: `examples dogfood`
- acceptance:
  - representative examples contain the minimum files needed by their verifier commands
  - example documentation matches the real workspace contents

### 02. Accept spaced global config options
- goal_file: `02_spaced-global-options.goal.json`
- why: the docs already show spaced examples and first-run parsing friction is avoidable
- paths: `crates/apps/src,crates/apps/tests,README.md,docs`
- workflow_pack: `examples/rust_service/pack.json`
- verifier_labels: `config parser,cli e2e`
- acceptance:
  - spaced and equals forms both work for global config options
  - CLI coverage catches the supported spellings

### 03. Add a brief-to-goal workflow for atomic goal authoring
- goal_file: `03_goal-stack-playbook.goal.json`
- why: operators should edit a smaller brief instead of hand-writing raw goal json
- paths: `docs,examples,scripts`
- workflow_pack: `examples/rust_service/pack.json`
- verifier_labels: `python syntax`
- acceptance:
  - the repo contains a documented workflow for turning slices into goal files
  - the workflow includes a runnable example and generated outputs
