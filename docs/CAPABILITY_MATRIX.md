# Capability Matrix

## 1. 원칙

제품 capability는 세 종류로만 분류한다.

- **Core**: 현재 제품 계약에 포함. release blocker다.
- **Experimental**: 저장소에는 남아도 기본 제품 약속에 포함되지 않는다.
- **Removed from product surface**: 문서, help, 기본 운영 경로에서 숨긴다.

current truth와 target contract가 다를 때는 current truth가 release 기준이다.

## 2. v1 Core Capability

| 영역 | capability | 상태 | release 기준 |
|---|---|---|---|
| CLI | `run` | Core | must pass |
| CLI | `resume` | Core | must pass |
| CLI | `abort` | Core | must pass |
| CLI | `doctor` | Core | must pass |
| CLI | `replay` | Core | must pass |
| CLI | `status` | Core | must pass |
| CLI | `health` | Core | must pass |
| CLI | `help` | Core | must pass |
| State | persisted state snapshot (`revision/mode/last_intent/last_decision/last_policy`) | Core | must pass |
| Provider | `codek` | Core | must pass |
| Provider | `mock-local` | Core | must pass |
| Workspace lock | stale recovery on Unix / fail-closed active lock on non-Unix | Core | must pass |
| Tool | list/read/search/write/replace/remove/run-command | Core | must pass |
| Safety | workspace boundary / allowlist / failure propagation | Core | must pass |
| Reports | plan/apply/verify/report artifact | Core | must pass |
| Validation | core/adapters/apps regression suite | Core | must pass |

## 3. Constraint Enforcement

goal file `constraints[]`에서 아래 4개 label만 현재 runtime이 강제 적용한다.

| constraint label | 상태 | 강제 방식 |
|---|---|---|
| `path_scope` | **Enforced** | workspace 경계 밖 경로 접근 시 fail-closed |
| `destructive_commands` | **Enforced** | `rm`, `mv` 등 destructive class command 차단 (`deny`) |
| `external_commands` | **Enforced** | allowlist 밖 external command 차단 (`deny`) |
| `approval_escalation` | **Enforced** | risk 판단 시 pre-execution approval 요구 (`required`) |
| 그 외 constraint label | Advisory-only | 기록만 하며 강제하지 않음 |

advisory constraint는 operator에게 보이지만 실행을 막지 않는다.

## 4. Verifier Strength

현재 runtime에서 `verification.status`는 실행 결과(state)와 verifier 품질(strength)을 같은 어휘로 인코딩한다.
`verifier_strength` 필드는 `verifier_strength_label(verification.status)` 순수 함수로 도출되며,
report artifact와 operator 출력이 같은 값을 표시한다.

| verifier_strength 값 | 의미 |
|---|---|
| `passed` | verification 통과. done condition까지 충족하면 `success` |
| `verification_weak` | 약한 fallback probe. `success`로 숨기면 안 됨 → `blocked` |
| `verification_unresolved` | 안전한 strong verifier를 만들지 못함 → `blocked` |
| `pack_required` | 도메인용 explicit pack이 필요 → `blocked` |
| `failed` | verification 명령 실행 실패 |
| `skipped` | approval-pending 등으로 실행 전 상태 |

`verification_weak`, `verification_unresolved`, `pack_required`는 절대 `success`로 보고되지 않는다.

## 5. Experimental Capability

| 영역 | capability | 이유 |
|---|---|---|
| Provider | `openai` compat provider | 기본 제품 경로를 흐리므로 opt-in fallback으로만 유지 |
| Tools | browser/composio/delegate | 범위를 과도하게 넓힘 |
| Memory | markdown/sqlite 외 장기 memory 확장 | 제품 의미를 흐릴 위험 |
| Ops | daemon/service/cron/metrics HTTP | retained CLI surface와 무관 |
| Channels | telegram/discord/slack 등 | 현재 제품 목표와 다른 면 |
| Gateway | ingress/signing/webhook | 현재 제품 핵심 경로가 아님 |

## 6. Removed From Product Surface

문서, help, 기본 운영 경로에서 노출하지 않는 것:

- `agent`, `daemon`, `gateway`, `service`, `cron`, `skills`, `integrations` 카탈로그 전면 노출

## 7. CLI Surface Contract

```bash
axiomrunner_apps run <goal-file>
axiomrunner_apps doctor [--json]
axiomrunner_apps replay [run-id|latest]
axiomrunner_apps status [run-id|latest]
axiomrunner_apps resume [run-id|latest]
axiomrunner_apps abort  [run-id|latest]
axiomrunner_apps health
axiomrunner_apps help
```

`resume`은 `waiting_approval` 상태의 goal-file pending run 전용이다.

## 8. Tool Surface Contract

| tool operation | 기능 |
|---|---|
| `list_files` | 파일 인벤토리 |
| `read_file` | 파일 읽기 |
| `search_files` | 문자열 검색 |
| `file_write` | bounded file write/append |
| `replace_in_file` | bounded text replacement |
| `remove_path` | 파일/디렉터리 제거 |
| `run_command` | allowlisted command 실행 |

추가 규칙:

- 모든 tool operation은 workspace boundary 안에서만 동작해야 한다.
- `run_command`는 allowlist를 통과한 command만 실행할 수 있다.
- `run_command`에는 timeout과 output truncation이 적용된다.

### Tool Request/Result Schema

| operation | required input | required output |
|---|---|---|
| `list_files` | `path` | `base`, `paths` |
| `read_file` | `path` | `path`, `contents` |
| `search_files` | `path`, `needle`, `mode` | `base`, `matches[path,line_number,line]`, `scanned_files`, `skipped_files` |
| `file_write` | `path`, `contents`, `append` | `path`, `bytes_written`, `evidence` |
| `replace_in_file` | `path`, `needle`, `replacement`, `expected_replacements?` | `path`, `replacements`, `evidence` |
| `remove_path` | `path` | `path`, `removed`, `evidence?` |
| `run_command` | `program`, `args` | `program`, `args`, `exit_code`, `stdout`, `stderr`, `stdout_truncated`, `stderr_truncated`, `artifact_path` |

### Evidence Schema

| evidence field | meaning |
|---|---|
| `operation` | overwrite / append / remove 등 mutation 종류 |
| `artifact_path` | patch 또는 command artifact 경로 |
| `before_digest` | 변경 전 digest |
| `after_digest` | 변경 후 digest |
| `before_excerpt` | 변경 전 일부 내용 |
| `after_excerpt` | 변경 후 일부 내용 |
| `unified_diff` | bounded diff text |

## 9. Backend Matrix

| backend | 역할 | status |
|---|---|---|
| `codek` | primary workspace automation substrate | Core |
| `mock-local` | deterministic contract path | Core |
| `openai` | opt-in compat fallback | Experimental |

## 10. Documentation Truth Rules

1. README/help/charter/RUNBOOK은 같은 CLI surface를 가리켜야 한다.
2. Experimental capability는 기본 제품 경로처럼 쓰면 안 된다.
3. hidden fallback backend를 두지 않는다.
4. `ready/degraded/blocked` health 의미는 문서와 출력이 일치해야 한다.
5. artifact 경로와 failure semantics는 e2e evidence로 잠가야 한다.

## 11. Release Blocker 조건

아래 중 하나라도 깨지면 release 금지:

- `run`/`status`/`replay`/`resume`/`abort` 핵심 경로 실패
- provider blocked/failure가 success처럼 보임
- workspace boundary 우회 가능
- allowlist 우회 가능
- report artifact가 남지 않음
- weak verification 또는 pack required run이 success처럼 보임
- README/help/charter/RUNBOOK/CAPABILITY_MATRIX 불일치
- autonomous eval corpus가 representative run을 통과하지 못함
- replay quality가 step journal / changed paths / failure visibility를 잃음
- rollback metadata 또는 nightly dogfood evidence contract가 release truth에서 빠짐

release evidence 기본 묶음:

```bash
cargo test -p axiomrunner_apps --test autonomous_eval_corpus
cargo test -p axiomrunner_apps --test fault_path_suite
cargo test -p axiomrunner_apps --test nightly_dogfood_contract
cargo test -p axiomrunner_apps --test release_security_gate
cargo test -p axiomrunner_adapters
```
