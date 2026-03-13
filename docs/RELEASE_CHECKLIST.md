# Release Checklist

- `cargo fmt --all --check`
- `cargo test --workspace`
- `doctor --json`로 provider/memory/tool 상태 확인
- README / DEPLOYMENT / charter / capability matrix truth surface 교차 확인
- `run`, `batch`, `replay` 최소 경로 수동 리허설
- blocked provider 경로와 workspace fail-closed 경로 확인
- CHANGELOG entry와 versioning policy 확인
