# Project Structure

AxonRunner 저장소는 크게 3개 실행 크레이트와 몇 개 보조 폴더로 나뉜다.

## 한눈에 보기

- `crates/core` — 제품의 규칙과 상태를 가진다.
- `crates/adapters` — provider, tool, memory처럼 바깥 세계와 붙는 부분을 가진다.
- `crates/apps` — CLI, run loop, replay, doctor 같은 실제 실행 경로를 가진다.
- `docs` — 제품 설명과 운영 문서를 둔다.
- `examples` — goal/pack 연결을 보여주는 representative verifier 예제를 둔다.
- `packs` — workflow pack 관련 참고 문서를 둔다.
- `scripts` — 반복 실행용 스크립트를 둔다.
- `plans` — 작업 계획과 리뷰 메모를 둔다. shipped product docs는 아니다.
- `target` — 빌드 결과물이다. 생성 파일이므로 읽기 대상이 아니다.

## 폴더별 설명

- `crates/core` — owns: 상태, 이벤트, 정책, 검증 규칙
  interacts with: `crates/apps`, `crates/adapters`
  주요 파일:
  - `intent.rs` — goal와 budget 같은 입력 규칙
  - `state.rs` — run 상태
  - `reducer.rs` — 상태 전이
  - `policy*.rs` — 허용/차단 이유 코드

- `crates/adapters` — owns: provider/tool/memory 계약과 실제 연결
  interacts with: `crates/apps`, `crates/core`
  주요 파일:
  - `contracts.rs` — workflow pack, tool, provider 계약
  - `provider_codex_runtime.rs` — codek provider
  - `provider_openai.rs` — opt-in openai provider
  - `tool.rs` — 파일/명령 실행
  - `memory_sqlite.rs`, `memory_markdown.rs` — memory backend

- `crates/apps` — owns: CLI 진입점과 실제 run orchestration
  interacts with: `crates/core`, `crates/adapters`
  주요 파일:
  - `main.rs` — 실행 시작점
  - `cli_command.rs` — 명령 파싱
  - `cli_runtime.rs` — run / resume / abort 흐름
  - `runtime_compose.rs` — plan/apply/verify/report 조립
  - `replay.rs`, `status.rs`, `doctor.rs` — 운영자 확인 경로
  - `trace_store.rs`, `state_store.rs`, `workspace_lock.rs` — 증거, 상태, lock

## `crates/apps`를 더 자세히 보면

`crates/apps`는 아래 5개 층으로 나뉜다.

- 시작층 — 프로그램을 시작하고 종료 코드를 정한다.
  파일:
  - `main.rs`
  - `lib.rs`

- 입력층 — 명령과 설정을 읽고 goal file인지 legacy intent인지 나눈다.
  파일:
  - `cli_args.rs`
  - `cli_command.rs`
  - `config_loader.rs`
  - `goal_file.rs`
  - `parse_util.rs`

- 실행층 — 실제 `run / resume / abort / health` 흐름을 움직인다.
  파일:
  - `cli_runtime.rs`
  - `runtime_compose.rs`
  - `runtime_compose/plan.rs`

- 저장층 — 상태, trace, lock을 파일로 남긴다.
  파일:
  - `state_store.rs`
  - `trace_store.rs`
  - `workspace_lock.rs`

- 조회층 — 운영자가 현재 상태를 읽는 출력 경로다.
  파일:
  - `status.rs`
  - `replay.rs`
  - `doctor.rs`
  - `display.rs`

### `crates/apps` 내부 흐름

1. `main.rs`가 `lib.rs`의 CLI entrypoint를 부른다.
2. `cli_args.rs`와 `cli_command.rs`가 입력을 해석한다.
3. `config_loader.rs`가 설정을 합친다.
4. `cli_runtime.rs`가 명령 종류에 따라 실행을 나눈다.
5. 실제 plan/apply/verify/report는 `runtime_compose.rs`가 조립한다.
6. 실행 결과는 `state_store.rs`와 `trace_store.rs`에 저장된다.
7. 운영자는 `status.rs`, `replay.rs`, `doctor.rs`로 다시 읽는다.

### 왜 이렇게 나뉘는가

- 입력 해석과 실제 실행을 분리해야 명령 규칙을 바꾸기 쉽다.
- 실행과 저장을 분리해야 replay와 doctor가 같은 근거를 다시 읽을 수 있다.
- 조회 경로를 따로 두어야 operator output을 짧게 유지할 수 있다.

- `docs` — owns: 제품 문서
  interacts with: README와 operator flow
  주요 파일:
  - `README.md` — 문서 입구
  - `project-charter.md` — 제품이 무엇인지
  - `CAPABILITY_MATRIX.md` — 무엇을 공식 지원하는지
  - `CODEK_RUNTIME_CONTRACT.md` — codek runtime 규칙
  - `RUNBOOK.md` — 실제 실행 방법
  - `VERSIONING.md` — 버전 정책

- `examples` — owns: representative verifier 예제
  interacts with: `README.md`, `docs/RUNBOOK.md`
  구성:
  - `rust_service`
  - `node_api`
  - `nextjs_app`
  - `python_fastapi`
  - `README.md`

- `packs/docs` — owns: workflow pack 참고 설명
  interacts with: planning/verifier flow 이해

- `scripts` — owns: 반복 실행 보조
  interacts with: local operator flow
  주요 파일:
  - `nightly_dogfood.sh`

- `plans` — owns: 리뷰, 청사진, 계획, 태스크, 셀프 리뷰
  interacts with: 작업 중 판단
  주의:
  - 제품 사용 문서가 아니라 작업 문서다.

## 어떻게 연결되는가

- 사용자는 `crates/apps`의 CLI로 시작한다.
- `crates/apps`는 `crates/core`의 규칙을 읽어 run 상태를 만든다.
- 실제 파일/명령/provider 호출은 `crates/adapters`가 맡는다.
- 결과는 다시 `crates/apps`의 report, replay, doctor, status로 보인다.

즉, 흐름은 보통 아래 순서다.

1. `apps`가 입력을 받는다.
2. `core`가 규칙을 확인한다.
3. `adapters`가 바깥 작업을 수행한다.
4. `apps`가 증거와 결과를 보여준다.

## docs 폴더 점검

지금 `docs/`에는 핵심 문서만 남아 있는 편이다.

매일 읽는 핵심 문서:

- `docs/README.md`
- `docs/project-charter.md`
- `docs/CAPABILITY_MATRIX.md`
- `docs/CODEK_RUNTIME_CONTRACT.md`
- `docs/WORKFLOW_PACK_CONTRACT.md`
- `docs/RUNBOOK.md`
- `docs/VERSIONING.md`
- `docs/PROJECT_STRUCTURE.md`

필요할 때만 보는 보조 문서:

- `docs/DOCS_ALIGNMENT.md`
- `docs/AUTONOMOUS_AGENT_TARGET.md`
- `docs/AUTONOMOUS_AGENT_SPEC.md`

판단:

- 불필요한 옛 분석 문서가 `docs/`에 많이 남아 있지는 않다.
- 작업용 문서는 `plans/`로 빠져 있어서 분리가 잘 되어 있다.
- 지금 필요한 것은 삭제보다 문서 입구를 더 쉽게 만드는 일이다.
