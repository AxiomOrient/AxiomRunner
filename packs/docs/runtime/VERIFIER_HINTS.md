# Workflow Pack Verifier Hints

workflow pack은 `recommended_verifier_flow[]`로 추천 검증 순서를 남긴다.

이 힌트는 실제 verifier rule을 바꾸지 않는다.
목적은 planner와 operator가 "보통 어떤 순서로 확인하는지"를 바로 읽게 하는 것이다.

예시:

- `rust-service-basic` -> `build > test > lint`
- `node-api-basic` -> `build > test > lint`
- `nextjs-app-basic` -> `build > test > lint`
- `python-fastapi-basic` -> `test > generic`

기본 goal pack은 goal의 `verification_checks[]`를 읽어 이 흐름을 추론한다.
