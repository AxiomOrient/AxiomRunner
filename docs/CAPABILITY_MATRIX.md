# 04. Capability Matrix

## 1. 원칙

제품 capability는 세 종류로만 분류한다.

- **Core**: 현재 제품 계약에 포함되며 release blocker다.
- **Experimental**: 저장소에는 남아도 기본 제품 약속에는 포함되지 않는다.
- **Removed from product surface**: 문서, help, 기본 운영 경로에서 숨긴다.

추가 문서 규칙:

- `docs/AUTONOMOUS_AGENT_TARGET.md`와 `docs/AUTONOMOUS_AGENT_SPEC.md`는 다음 단계의 target contract를 설명한다.
- current truth와 target contract가 다를 때는 current truth가 release 기준이다.
- `docs/DOCS_ALIGNMENT.md`는 현재 contract와 전환 계획을 함께 읽을 때의 해석 규칙을 제공한다.

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
| CLI compat | `batch` | Core | must pass |
| CLI alias | `read/write/remove/freeze/halt` | Compatibility | should pass |
| State | persisted state snapshot (`revision/mode/facts`) | Core | must pass |
| Provider | `codek` | Core | must pass |
| Provider | `mock-local` | Core | must pass |
| Tool | list/read/search/write/replace/remove/run-command | Core | must pass |
| Safety | workspace boundary / allowlist / failure propagation | Core | must pass |
| Reports | plan/apply/verify/report artifact | Core | must pass |
| Validation | core/adapters/apps regression suite | Core | must pass |

## 3. Experimental Capability

| 영역 | capability | 이유 |
|---|---|---|
| Provider | `openai` compat provider | 기본 제품 경로를 흐리므로 opt-in fallback으로만 유지 |
| Tools | browser/composio/delegate | 범위를 과도하게 넓힘 |
| Memory | markdown/sqlite 외 장기 memory 확장 | 제품 의미를 흐릴 위험 |
| Ops | daemon/service/cron/metrics HTTP | 현재 retained CLI surface와 무관 |
| Channels | telegram/discord/slack/irc/matrix/whatsapp | 현재 제품 목표와 다른 면 |
| Gateway | ingress/signing/webhook | 현재 제품 핵심 경로가 아님 |

## 4. Removed From Product Surface

현재 README, help, DEPLOYMENT, charter에서 기본 노출하지 않는 것:

- `agent`
- `daemon`
- `gateway`
- `service`
- `cron`
- `skills`
- `integrations` 카탈로그 전면 노출

## 5. CLI Surface Contract

### 허용

```bash
axonrunner_apps run <goal-file>
axonrunner_apps doctor [--json]
axonrunner_apps replay [run-id|latest]
axonrunner_apps status [run-id|latest]
axonrunner_apps resume [run-id|latest]
axonrunner_apps abort [run-id|latest]
axonrunner_apps health
axonrunner_apps help
```

`resume`은 `waiting_approval` 상태의 goal-file pending run 전용이다.

### compatibility surface

```bash
axonrunner_apps batch [--reset-state] <intent-spec>...
```

### 유지되는 thin alias

```bash
axonrunner_apps read <key>
axonrunner_apps write <key> <value>
axonrunner_apps remove <key>
axonrunner_apps freeze
axonrunner_apps halt
```

## 6. Tool Surface Contract

현재 tool surface는 아래만 보장한다.

| tool operation | 기능 |
|---|---|
| `list_files` | 파일 인벤토리 |
| `read_file` | 파일 읽기 |
| `search_files` | 문자열 검색 |
| `file_write` | bounded file write/append |
| `replace_in_file` | bounded text replacement |
| `remove_path` | 파일/디렉터리 제거 |
| `run_command` | allowlisted command 실행 |

핵심은 도구 수가 아니라, 이 표면이 workspace boundary와 failure semantics를 지키면서 끝까지 동작하는 것이다.

### Tool Request Schema

| operation | required input | contract |
|---|---|---|
| `list_files` | `path` | workspace 안의 경로만 허용 |
| `read_file` | `path` | 파일만 허용, 크기 제한 준수 |
| `search_files` | `path`, `needle`, `mode` | substring / regex 검색만 허용, scanned/skipped count 포함 |
| `file_write` | `path`, `contents`, `append` | bounded write/append만 허용 |
| `replace_in_file` | `path`, `needle`, `replacement`, `expected_replacements?` | 최소 1회 치환, multi-match 는 expected count가 맞을 때만 허용 |
| `remove_path` | `path` | workspace boundary 안 경로만 허용 |
| `run_command` | `program`, `args` | allowlist, timeout, output truncation 적용 |

### Tool Result Schema

| operation | required output |
|---|---|
| `list_files` | `base`, `paths` |
| `read_file` | `path`, `contents` |
| `search_files` | `base`, `matches[path,line_number,line]`, `scanned_files`, `skipped_files` |
| `file_write` | `path`, `bytes_written`, `evidence` |
| `replace_in_file` | `path`, `replacements`, `evidence` |
| `remove_path` | `path`, `removed`, `evidence?` |
| `run_command` | `program`, `args`, `exit_code`, `stdout`, `stderr`, `stdout_truncated`, `stderr_truncated`, `artifact_path` |

### Evidence Schema

모든 mutation evidence는 아래 필드를 기준으로 읽는다.

| evidence field | meaning |
|---|---|
| `operation` | overwrite / append / remove 등 mutation 종류 |
| `artifact_path` | patch 또는 command artifact 경로 |
| `before_digest` | 변경 전 digest |
| `after_digest` | 변경 후 digest |
| `before_excerpt` | 변경 전 일부 내용 |
| `after_excerpt` | 변경 후 일부 내용 |
| `unified_diff` | bounded diff text |

## 7. Backend Matrix

| backend | 역할 | status |
|---|---|---|
| `codek` | primary workspace automation substrate | Core |
| `mock-local` | deterministic contract path | Core |
| `openai` | opt-in compat fallback | Experimental |

## 8. Documentation Truth Rules

1. README/help/DEPLOYMENT/charter는 같은 CLI surface를 말해야 한다.
2. Experimental capability는 기본 제품 경로처럼 쓰면 안 된다.
3. hidden fallback backend를 두지 않는다.
4. `ready/degraded/blocked` health 의미는 문서와 출력이 같아야 한다.
5. artifact 경로와 failure semantics는 e2e evidence로 잠가야 한다.

## 9. Release Blocker 조건

아래 중 하나라도 깨지면 release 금지다.

- `run` 또는 `status` 또는 `replay` 또는 `resume` 또는 `abort` 핵심 경로 실패
- provider blocked/failure가 success처럼 보임
- persisted `freeze`/`halt` 의미가 깨짐
- workspace boundary 우회 가능
- allowlist 우회 가능
- report artifact가 남지 않음
- README/help/DEPLOYMENT/charter/capability matrix 불일치
- autonomous eval corpus가 representative run을 통과하지 못함
- replay quality가 step journal / changed paths / failure visibility를 잃음

release evidence 기본 묶음:

- `cargo test -p axonrunner_apps --test autonomous_eval_corpus`
- `cargo test -p axonrunner_apps --test fault_path_suite`
- `cargo test -p axonrunner_apps --test nightly_dogfood_contract`
- `cargo test -p axonrunner_apps --test release_security_gate`
- `cargo test -p axonrunner_adapters`

autonomy evidence가 만족해야 하는 추가 기준:

- replay summary에 `false_success_intents`와 `false_done_intents`가 보여야 한다.
- nightly dogfood driver가 대표 fixture 로그 번들을 남겨야 한다.
- fault path suite가 provider/tool/workspace substrate 고장 경로를 통과해야 한다.

## 10. Bridge References

현재 제품 truth를 유지한 상태에서 다음 전환 목표를 추적하는 문서:

- `docs/AUTONOMOUS_AGENT_TARGET.md`
- `docs/AUTONOMOUS_AGENT_SPEC.md`
- `docs/DOCS_ALIGNMENT.md`
- `docs/WORKFLOW_PACK_CONTRACT.md`
