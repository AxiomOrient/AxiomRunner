# IMPLEMENTATION-PLAN

## 결론

- 현재 `cd3573d`는 테스트는 모두 통과하지만, 아직 `1.0.0`을 선언할 상태는 아니다.
- 직접 확인한 결과, 아래 다섯 가지는 실제 문제다.
  - memory summary 저장 실패가 프로세스 실패가 아니라 warning으로만 처리된다.
  - `file_exists` done condition이 runtime workspace가 아니라 goal file의 `workspace_root` 문자열을 기준으로 평가된다.
  - `run_command` 정책이 프로그램 이름만 검사하고 인자 의미를 검사하지 않는다.
  - commit 실패 시 trace/state/artifact는 정리하지만, 실제 workspace 변경은 원자적으로 되돌리지 못한다.
  - `workflow_pack.allowed_tools`는 계약과 문서에는 있지만 실제 집행에는 쓰이지 않는다.
- 반대로 아래 항목은 이번 HEAD에서 이미 닫혔다.
  - `resume` 실패 승격
  - typed done condition / workspace-relative path parse
  - dynamic step journal / trace latest summary 정합성
  - state snapshot tmp fallback corruption 처리
  - Windows stale lock recovery
- 아래 항목은 사실이지만 `1.0.0` 차단선은 아니다.
  - experimental `openai` provider의 reqwest client builder 실패 시 timeout 설정을 잃는 fallback
  - `trace` 전체 JSONL 재로드, markdown memory 전체 재쓰기
  - `runtime_compose.rs`의 과도한 책임 집중

## 범위 계약

- REQUEST: `1.0.0` 출시를 막는 실제 blocker를 코드와 문서에서 닫고, release truth를 `1.0.0` 기준으로 다시 잠근다.
- TARGET_SCOPE:
  - `crates/core/src/intent.rs`
  - `crates/core/src/workflow_pack.rs`
  - `crates/core/src/lib.rs`
  - `crates/adapters/src/contracts.rs`
  - `crates/adapters/src/tool.rs`
  - `crates/adapters/src/tool_write.rs`
  - `crates/adapters/src/memory_markdown.rs`
  - `crates/adapters/src/memory_sqlite.rs`
  - `crates/apps/src/goal_file.rs`
  - `crates/apps/src/command_contract.rs`
  - `crates/apps/src/cli_runtime/lifecycle.rs`
  - `crates/apps/src/cli_runtime/run_session.rs`
  - `crates/apps/src/run_commit.rs`
  - `crates/apps/src/runtime_compose.rs`
  - `crates/apps/src/runtime_compose/plan.rs`
  - `crates/apps/src/runtime_compose/artifacts.rs`
  - `crates/apps/src/operator_render.rs`
  - `crates/apps/src/doctor.rs`
  - `crates/apps/tests/e2e_cli.rs`
  - `crates/apps/tests/release_security_gate.rs`
  - `crates/apps/tests/autonomous_eval_corpus.rs`
  - `crates/apps/tests/fault_path_suite.rs`
  - `README.md`
  - `docs/project-charter.md`
  - `docs/RUNBOOK.md`
  - `docs/CAPABILITY_MATRIX.md`
  - `docs/WORKFLOW_PACK_CONTRACT.md`
  - `docs/VERSIONING.md`
  - `examples/`
  - `tools/dev/generate_goal_stack.py`
  - workspace `Cargo.toml` and crate `Cargo.toml`
- DONE_CONDITION:
  - runtime workspace 의미가 goal file, planner, evaluator, status/replay/report에서 하나로 고정된다.
  - verifier/run-command 계약이 parse, planning, execution에서 같은 규칙과 같은 allowlist/arg rule을 쓴다.
  - memory summary 저장 실패가 success/warning으로 숨지 않고 commit transaction failure로 승격된다.
  - commit 실패 시 file mutation이 남지 않거나, 남을 수 있는 경우가 제품 계약상 허용되지 않는다.
  - workflow pack/documentation surface에는 실제로 집행되는 필드만 남는다.
  - `README`, `project-charter`, `RUNBOOK`, `CAPABILITY_MATRIX`, `WORKFLOW_PACK_CONTRACT`, release gate tests, versioning 문서가 `1.0.0` truth를 같은 말로 가리킨다.
- CONSTRAINTS:
  - GitHub CI는 쓰지 않는다.
  - 웹 검증이 필요한 외부 버전/정책/가격 이슈는 이번 범위에 포함하지 않는다.
  - planner, executor, docs truth를 따로 고치지 말고 공통 contract에서 파생되게 만든다.

## 확인한 사실

### 실제 blocker

1. memory failure honesty mismatch
   - `commit_prepared_run`가 `remember_run_summary(...)` 실패를 `memory_warning`으로만 반환한다.
   - `run_session`은 이 값을 `stderr`에만 출력하고 run을 계속 성공 처리할 수 있다.
   - README의 "provider/tool/memory 단계 실패는 process failure" 문구와 충돌한다.

2. workspace 의미 이중화
   - `DoneConditionEvidence::FileExists`는 typed path를 쓰지만, 실제 평가는 `goal.workspace_root.join(path)`로 수행된다.
   - CLI runtime의 실제 workspace (`--workspace`, config, env)와 goal file 안 문자열이 다르면 잘못된 파일을 검사한다.
   - `workspace_root`는 현재 "실행 경계"와 "문서 필드" 두 역할을 동시에 갖고 있다.

3. run_command contract가 느슨함
   - planner, goal-file validation, executor는 같은 프로그램 수준 validator를 쓰지만, 인자 규칙은 없다.
   - `cargo`, `git`, `npm`, `node`, `python`, `python3`, `pnpm`, `yarn`, `uv`, `make`는 프로그램 이름만 맞으면 통과한다.
   - operator가 `--command-allowlist`로 더 좁힌 경우 planner/pack parse는 그 사실을 모른 채 제품 기본 allowlist 기준으로 허용한다.

4. workspace mutation commit integrity gap
   - `cleanup_commit_artifacts`는 report/checkpoint/rollback/trace/tool artifacts는 지우지만, 이미 바뀐 workspace 파일은 복원하지 않는다.
   - 현재 rollback metadata는 isolated worktree 경로를 기록할 뿐, base workspace 복구 수단은 아니다.
   - 따라서 non-isolated run에서 commit 후반 실패가 나면 workspace가 dirty하게 남을 수 있다.

5. documentary pack surface
   - `WorkflowPackContract.allowed_tools`는 validate와 docs에는 있으나 실제 tool execution gate에는 연결되지 않는다.
   - pack이 tool scope를 줄인다고 문서화하지만 runtime은 그 약속을 집행하지 않는다.

### 사실이지만 release blocker는 아닌 항목

- experimental OpenAI provider의 client builder 실패 fallback은 degraded 상태로 드러나지만 timeout 설정을 잃는다.
- `TraceStore`는 조회마다 JSONL 전체를 읽고, markdown memory는 mutation마다 전체 파일을 다시 쓴다.
- `runtime_compose.rs`는 여전히 책임이 많다.

### 이미 닫힌 항목

- `resume` 실패 전파는 현재 `run`과 같은 수준으로 `Err`를 반환한다.
- done condition은 typed vocabulary로 제한되고 절대경로와 `..`는 parse 단계에서 차단된다.
- trace summary는 최신 이벤트 기준으로 계산되고 `intent_count`는 unique intent 기준이다.
- state snapshot은 primary corrupt를 tmp fallback으로 숨기지 않는다.
- lock stale recovery는 Unix와 Windows에서 구현되어 있다.

## 1.0 설계 원칙

1. workspace는 하나만 가진다.
   - goal file은 실행 경계를 소유하지 않는다.
   - runtime이 고른 canonical workspace만 실행, done-condition, status/replay, artifacts의 기준이다.

2. command contract는 하나만 가진다.
   - 허용 프로그램, 허용 인자, risk/profile 분류, operator override 규칙이 한 모듈에서 정의되어야 한다.
   - parse/planning/execution은 이 모듈을 그대로 재사용해야 한다.

3. commit은 transaction처럼 동작해야 한다.
   - report/trace/state/memory와 workspace mutation이 함께 성공하거나 함께 rollback돼야 한다.
   - rollback이 불가능한 실행 표면은 `1.0.0` core contract에서 제거한다.

4. 문서 필드는 집행되는 것만 남긴다.
   - enforcement 없는 surface는 제거하거나 experimental로 내린다.

5. `1.0.0`은 기능 추가보다 주장 축소와 의미 일치를 우선한다.
   - surface를 좁혀서라도 failure-honest, verifier-first, workspace-bound 의미를 지킨다.

## 권장 설계

### A. workspace identity 정리

- `RunGoal.workspace_root`를 실행 의미에서 제거한다.
- runtime의 canonical workspace를 `RuntimeWorkspaceBinding` 같은 구조로 한 번만 만들고 아래 경로가 모두 이 값을 받게 한다.
  - tool cwd
  - provider cwd
  - done-condition evaluation
  - status / replay / report / trace 표시
  - lock / artifact root 표시
- goal file에서는 workspace-relative evidence만 유지한다.
- 호환성이 필요하면 `workspace_root`는 optional deprecated field로 잠깐 허용하되, 값이 들어오면 runtime workspace와 일치해야만 parse 통과하게 한다.

### B. command contract 재설계

- 프로그램 allowlist를 "이름 목록"에서 "명시적 명령 스펙"으로 바꾼다.
- 예:
  - `git status`
  - `git diff --name-only`
  - `cargo test ...`
  - `cargo build ...`
  - `cargo clippy ...`
  - `rg --files ...`
  - `pytest ...`
- interpreter 성격이 강한 `python`, `python3`, `node`, `npm`, `pnpm`, `yarn`, `uv`, `make`는 그대로 두지 말고 허용할 인자 shape를 명시하거나 core surface에서 제외한다.
- operator allowlist는 명령 스펙 ID를 더 좁히는 방식으로만 적용한다.
- planner, goal-file pack validation, executor는 같은 validator를 써야 하고, 같은 이유 코드로 거부해야 한다.

### C. workspace rollback 가능성 확보

- `run_command`는 read-only verifier 명령으로만 한정한다.
- 실제 mutation은 `file_write`, `replace_in_file`, `remove_path` 같은 explicit file ops로만 남긴다.
- patch artifact schema를 `restore` 정보까지 포함하는 형태로 확장한다.
  - 기존 파일 overwrite/replace: 전체 원본 bytes 또는 restore blob path
  - 새 파일 생성: "created during run" tombstone
  - remove file: 전체 원본 bytes
  - remove dir: directory archive 또는 recursive restore manifest
- commit 실패 시 `cleanup_commit_artifacts`가 artifact 삭제만 하지 말고 restore manifest를 따라 workspace를 복원한다.
- git repo에서는 isolated worktree를 기본값으로 올려 2중 안전장치로 둔다.

### D. memory를 commit transaction 안으로 편입

- memory summary 저장은 warning이 아니라 commit 단계의 필수 write로 취급한다.
- 추천 순서:
  1. report/checkpoint/rollback metadata write
  2. trace append
  3. memory store
  4. state snapshot save
- `memory store` 실패 시 trace rollback + artifact cleanup + workspace restore를 같이 수행한다.
- `state snapshot save` 실패 시 trace rollback + memory delete + artifact cleanup + workspace restore를 같이 수행한다.

### E. pack surface 축소

- `allowed_tools`를 진짜 enforcement에 연결할 자신이 없으면 `1.0.0` contract에서 제거한다.
- pack은 verifier flow / verifier rules / approval mode만 소유하게 단순화하는 쪽이 안전하다.

## 단계별 빌드 경로

### Phase 1. red test 고정

- 현재 다섯 blocker를 red test로 먼저 고정한다.
- 목표는 "문제가 실제로 닫혔는지"를 release gate에서 다시 보이게 만드는 것이다.

### Phase 2. contract 단일화

- workspace binding과 command contract를 단일 source로 만든다.
- 이 단계가 끝나야 docs/examples migration을 안정적으로 할 수 있다.

### Phase 3. transactional commit

- restore payload와 memory rollback을 넣어 commit boundary를 workspace까지 확장한다.
- isolated git worktree는 기본값으로 올린다.

### Phase 4. surface 정리

- pack/doc fields를 실제 집행 기준으로 축소한다.
- status/replay/report/operator wording을 새 contract에 맞춘다.

### Phase 5. 1.0 release lock

- version을 `1.0.0`으로 올리고 docs/tests/examples를 고정한다.
- release evidence bundle과 manual smoke를 남긴다.

## 위험과 대응

- command surface를 너무 급히 줄이면 example/fixture가 대량으로 깨질 수 있다.
  - 대응: red/green matrix test를 먼저 추가하고, 각 example별 허용 명령 목록을 문서화한다.
- restore payload가 크면 patch artifact 저장 비용이 커질 수 있다.
  - 대응: `run_command`를 read-only로 좁혀 복구 대상 수를 줄이고, 파일 restore blob은 artifact root 아래 압축 저장한다.
- `workspace_root` 제거는 fixture와 docs에 큰 파급이 있다.
  - 대응: 한 릴리스 동안 deprecated parse path를 두고, mismatch는 hard fail로 막는다.
- git worktree 기본화는 non-git workspace 경험을 흔들 수 있다.
  - 대응: non-git은 restore-payload rollback 경로로 유지하고, `doctor`에 실제 execution mode를 분명히 노출한다.

## Resolved Decisions

- `RunGoal.workspace_root`는 deprecated compatibility field로 유지하고, 실제 실행 경계는 runtime workspace가 소유한다.
- verifier command contract는 adapters의 spec validator를 apps parse/planning/runtime이 재사용하는 구조로 고정했다.
- `workflow_pack.allowed_tools`는 제거하지 않고, verifier `run_command`와 실제 enforcement를 연결하는 방향으로 고정했다.
- non-git workspace integrity는 restore-payload rollback 경로를 기본으로 지원하는 쪽으로 고정했다.

## Expanded Atomic Path

- `$scout-boundaries`
- `$plan-what-it-does`
- `$plan-how-to-build`
- `$plan-task-breakdown`
