# Examples

운영자가 바로 돌려볼 수 있는 representative verifier 예제 묶음이다.
이 디렉터리는 fixture 모음이 아니라 공식 operator-facing starter asset으로 취급한다.

- `examples/rust_service`
- `examples/node_api`
- `examples/nextjs_app`
- `examples/python_fastapi`

각 디렉터리는 아래 파일을 가진다.

- `goal.json`
- `pack.json`
- 최소 verifier workspace 파일

이 예제들은 self-contained minimal workspace다.
verifier command가 바로 실행될 수 있게 필요한 최소 파일을 함께 둔다.

goal을 직접 쪼개서 만들 때는 `examples/goal_stacks/` 와
`docs/GOAL_STACK_PLAYBOOK.md` 를 같이 본다.

실행 예:

```bash
cargo run -q -p axiomrunner_apps -- \
  --provider=codek \
  --workspace="$PWD/examples/rust_service" \
  run examples/rust_service/goal.json
```

같은 방식으로 `node_api`, `nextjs_app`, `python_fastapi` 도 실행할 수 있다.

이 예제는 framework starter repo가 아니라 verifier flow와 goal/pack 연결 예시다.

## generic run vs pack-backed run

- generic run은 goal file의 `verification_checks` detail만으로 verifier를 추론한다.
- detail이 약하거나 도메인 pack이 필요하면 `verification_weak`, `verification_unresolved`, `pack_required` 로 정직하게 드러난다.
- pack-backed run은 `pack.json`이 verifier 순서, command example, allowed tool 경계를 명시한다.
- app/server 자동화처럼 build/test/lint/typecheck 의미가 중요하면 pack-backed run이 기준이다.

## 1.0.0 Example Set

아래 4개 예제가 모두 유지 대상이다.

- `rust_service` — Rust service verifier path
- `node_api` — Node API verifier path
- `nextjs_app` — Next.js app verifier path
- `python_fastapi` — Python FastAPI verifier path

목표는 각 예제가 "goal + pack + verifier flow"를 바로 재현하게 두는 것이다.
