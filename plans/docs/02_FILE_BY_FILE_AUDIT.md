# 02. File-by-File Audit

기준:
- **Keep**: 현재 설계의 핵심 자산
- **Tighten**: 유지하되 계약/품질을 더 잠가야 함
- **Archive**: 현재 source of truth가 아니므로 정리/분리 필요
- **Fix**: 구체 버그 또는 드리프트가 있음

---

## Root

| File | 판정 | 역할 | 핵심 판단 | 액션 |
|---|---|---|---|---|
| `Cargo.toml` | Keep | workspace root | `apps/core/adapters` 3크레이트만 유지하는 현재 방향이 좋다 | 유지 |
| `Cargo.lock` | Keep | reproducibility | 버전 고정 기반 | 유지 |
| `README.md` | Tighten | 제품 진실 표면 | 제품 설명은 대체로 맞지만 `doctor` 누락, 소스 링크 타깃 문제 존재 | 수정 |
| `CHANGELOG.md` | Fix | 변화 기록 | 여전히 `agent`를 retained surface에 포함 | 현재 제품면으로 갱신 |
| `LICENSE` | Keep | 라이선스 | 문제 없음 | 유지 |
| `.gitignore` | Keep | 개발 위생 | 특별한 문제 없음 | 유지 |

## Docs

| File | 판정 | 역할 | 핵심 판단 | 액션 |
|---|---|---|---|---|
| `docs/project-charter.md` | Keep | 현재 제품 charter | 가장 중요한 source of truth 중 하나 | 유지 |
| `docs/DEPLOYMENT.md` | Tighten | 운영/배포 문서 | 실제 command surface와 doctor/replay/runbook 반영 필요 | 갱신 |
| `docs/01_PRODUCT_SPEC.md` | Archive | 과거 blueprint | 현재 3크레이트 제품 현실과 어긋남 | `docs/archive/` 이동 |
| `docs/02_TARGET_ARCHITECTURE.md` | Archive | 과거 blueprint | 현재 구조보다 넓음 | 아카이브 |
| `docs/03_CODEK_INTEGRATION_RFC.md` | Tighten | 통합 방향 | 여전히 유효한 부분이 있으나 현재 구현으로 재기술 필요 | 개정 |
| `docs/04_CAPABILITY_MATRIX.md` | Archive | 과거 capability 문서 | 현재 CLI truth surface 기준 재작성 필요 | 아카이브 후 신규 작성 |
| `docs/05_IMPLEMENTATION_MASTER_PLAN.md` | Archive | 과거 master plan | 현재 post-board 현실과 분리 | 아카이브 |
| `docs/06_TASKS_BACKLOG.md` | Archive | 과거 backlog | 현재 PB-계획과 혼선 가능 | 아카이브 |
| `docs/07_LIBRARY_AND_SOURCE_RECOMMENDATIONS.md` | Tighten | 라이브러리 판단 근거 | 여전히 참고 가치 있으나 현재 코드와 연결 약함 | 요약본 재작성 |
| `docs/08_VALIDATION_RELEASE_RUNBOOK.md` | Tighten | 검증/릴리즈 | 현재 trace/doctor/replay 계약 중심으로 정렬 필요 | 갱신 |
| `docs/09_SELF_REVIEW.md` | Archive | 과거 self review | 현 상태와 분리 | 아카이브 |
| `docs/10_REFERENCE_BASIS.md` | Archive | 과거 리서치 | 참고는 가능하나 source of truth 아님 | 아카이브 |

## Plans

| File | 판정 | 역할 | 핵심 판단 | 액션 |
|---|---|---|---|---|
| `plans/IMPLEMENTATION-PLAN.md` | Keep | post-board 계획 | Wave A-D 구조는 좋다 | 유지, 다음 wave 추가 |
| `plans/TASKS.md` | Tighten | post-board task ledger | 실제 테스트 파일 구성과 verification 문구 정합성 확인 필요 | 수정 |

## `crates/core`

| File | 판정 | 역할 | 핵심 판단 | 액션 |
|---|---|---|---|---|
| `Cargo.toml` | Keep | core package | 외부 의존이 거의 없는 점이 강점 | 유지 |
| `Cargo.lock` | Keep | local reproducibility | 문제 없음 | 유지 |
| `src/lib.rs` | Keep | 공개 표면 | 작고 명확한 재수출 | 유지 |
| `src/intent.rs` | Keep | intent 모델 | 입력 모델이 간결하고 명확 | 유지 |
| `src/policy.rs` | Keep | 정책 엔진 | actor/mode/payload 정책이 명시적 | 유지 |
| `src/policy_codes.rs` | Keep | 정책 코드 표준화 | 운영/trace 친화적 | 유지 |
| `src/decision.rs` | Keep | decision 생성 | core 흐름이 명확 | 유지 |
| `src/effect.rs` | Keep | effect 모델 | intent ↔ effect 관계가 단순 | 유지 |
| `src/event.rs` | Keep | domain event 모델 | event-sourcing 코어 유지 가치 높음 | 유지 |
| `src/reducer.rs` | Tighten | 상태 전이 | revision이 event-count라는 점을 문서화해야 함 | 문서 보강 |
| `src/projection.rs` | Keep | replay/projection | 명확 | 유지 |
| `src/state.rs` | Keep | 상태 모델 | 필드가 아직 과하지 않다 | 유지 |
| `src/validation.rs` | Keep | payload 경계 | core 안전성 기반 | 유지 |
| `src/audit.rs` | Keep | policy audit formatting | trace/doctor와 잘 맞음 | 유지 |
| `tests/domain_flow.rs` | Keep | 도메인 흐름 검증 | 핵심 | 유지 |
| `tests/policy_codes.rs` | Keep | 정책 코드 검증 | 유지 |
| `tests/policy_decision.rs` | Keep | 정책/결정 일관성 | 유지 |
| `tests/projection_replay.rs` | Keep | projection 검증 | 유지 |
| `tests/reducer_cases.rs` | Keep | reducer 검증 | 유지 |
| `tests/schema_boundaries.rs` | Keep | schema/invariant 경계 | 유지 |
| `tests/state_invariants.rs` | Keep | invariant 검증 | 유지 |

## `crates/apps`

| File | 판정 | 역할 | 핵심 판단 | 액션 |
|---|---|---|---|---|
| `Cargo.toml` | Keep | app package | 의존이 비교적 얕다 | 유지 |
| `src/main.rs` | Keep | binary entry | 단순 | 유지 |
| `src/lib.rs` | Keep | CLI orchestration | exit code 구분이 좋다 | 유지 |
| `src/cli_args.rs` | Keep | argv parsing | 좁고 명시적 | 유지 |
| `src/cli_command.rs` | Tighten | 명령 파싱 | 실제 CLI source of truth | README/charter와 동기화 |
| `src/cli_runtime.rs` | Keep | 제품 핵심 실행기 | false-success 제거와 snapshot restore가 좋다 | 유지, 추가 하드닝 |
| `src/config_loader.rs` | Keep | 공식 config surface | 현재 제품면과 잘 맞음 | 유지 |
| `src/dev_guard.rs` | Keep | release guard | 제품 discipline에 도움 | 유지 |
| `src/display.rs` | Fix | 사용자 표시 포맷 | `read_only` naming drift의 한 축 | naming 통일 |
| `src/env_util.rs` | Keep | env helper | 문제 없음 | 유지 |
| `src/parse_util.rs` | Keep | parse helper | 문제 없음 | 유지 |
| `src/async_runtime_host.rs` | Tighten | async bridge | fallback runtime 정책을 더 명시적으로 만들 필요 | 하드닝 |
| `src/runtime_compose.rs` | Tighten | provider/memory/tool orchestration | 핵심 파일. workspace/cwd fail-closed, evidence 강화 필요 | 하드닝 |
| `src/runtime_compose/plan.rs` | Tighten | execution planning | read/freeze/halt의 side-effect 전략은 좋음 | artifacts 강화 |
| `src/doctor.rs` | Keep | operator diagnostics | 매우 유용 | 유지 |
| `src/replay.rs` | Keep | replay UX | 현재 제품 가치 높음 | 유지 |
| `src/state_store.rs` | Fix | persisted state snapshot | `readonly` vs `read_only` drift | schema 통일/마이그레이션 |
| `src/status.rs` | Keep | status view | 충분히 단순 | 유지 |
| `src/trace_store.rs` | Tighten | append-only trace | storage facade 분리 방향 좋음 | schema/patch evidence 보강 |
| `tests/common/mod.rs` | Keep | 테스트 도우미 | 유지 |
| `tests/config_priority.rs` | Keep | config precedence | 핵심 | 유지 |
| `tests/e2e_cli.rs` | Keep | 제품 계약 검증 | 핵심 corpus | 확대 |
| `tests/release_security_gate.rs` | Keep | 릴리즈 게이트 | 유지 |

## `crates/adapters`

| File | 판정 | 역할 | 핵심 판단 | 액션 |
|---|---|---|---|---|
| `Cargo.toml` | Tighten | adapter deps | `codex-runtime 0.4.0` pin은 명시적이지만 upstream drift 관리 필요 | compatibility 문서/검증 |
| `src/lib.rs` | Keep | 재수출 | 단순 | 유지 |
| `src/contracts.rs` | Keep | adapter 계약 | 현재 제품의 좋은 경계 | 유지 |
| `src/error.rs` | Keep | 오류 분류 | 운영상 유용 | 유지 |
| `src/memory.rs` | Keep | memory factory | 충분히 작다 | 유지 |
| `src/memory_markdown.rs` | Fix | file memory | header에 `ZeroClaw` 잔재 | 브랜딩 수정 |
| `src/memory_sqlite.rs` | Keep | persisted memory | 현재 수준에서 충분 | 유지 |
| `src/provider_registry.rs` | Keep | provider selection | `mock-local`, `codek`, `openai` 구조가 적절 | 유지 |
| `src/provider_codex_runtime.rs` | Tighten | main provider substrate | session 재사용 방향 좋음. version/compatibility/doctor 잠금 필요 | 하드닝 |
| `src/provider_openai.rs` | Tighten | experimental fallback | 유지할 거면 async/blocking 정리 필요 | experimental 격리 강화 |
| `src/tool_workspace.rs` | Keep | workspace boundary | 핵심 안전 자산 | 유지 |
| `src/tool_write.rs` | Tighten | text mutation/patchevidence | atomic write와 digest는 좋음 | diff-grade evidence 강화 |
| `src/tool.rs` | Keep | essential tool surface | 현재 자동화 본체에 가장 가까움 | 테스트 확대 |
| `tests/error.rs` | Keep | 오류 분류 테스트 | 유지 |
| `tests/memory.rs` | Keep | memory 테스트 | 유지 |
| `tests/tool.rs` | Fix | tool contract tests | 계획 문서 기준 존재가 필요하나 visible tree와 불일치 | 실제 파일/문서 정합화 |

---

## 핵심 판단 요약

- `core`는 거의 전부 **유지**가 맞다.
- `apps`는 **제품 중심**이라 대부분 유지하되 몇 군데 naming/contract를 잠가야 한다.
- `adapters`는 지금 구현의 가치가 높지만,
  `codek` 호환성, patch evidence, tool test coverage를 더 닫아야 한다.
- `docs/0*.md`는 현재 제품 소스가 아니므로 source of truth 옆에 두지 말고 **archive**로 분리하는 것이 맞다.
