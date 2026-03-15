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

실행 예:

```bash
cargo run -q -p axiomrunner_apps -- \
  --workspace="$PWD/examples/rust_service" \
  run examples/rust_service/goal.json
```

같은 방식으로 `node_api`, `nextjs_app`, `python_fastapi` 도 실행할 수 있다.

이 예제는 framework starter repo가 아니라 verifier flow와 goal/pack 연결 예시다.

## v0.2 Developer Automation Milestone

아래 4개 예제가 모두 유지 대상이다.

- `rust_service` — Rust service verifier path
- `node_api` — Node API verifier path
- `nextjs_app` — Next.js app verifier path
- `python_fastapi` — Python FastAPI verifier path

목표는 각 예제가 "goal + pack + verifier flow"를 바로 재현하게 두는 것이다.
