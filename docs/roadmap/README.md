# AxonRunner Autonomous Roadmap Bundle

이 번들은 `AxiomOrient/AxonRunner`의 현재 `dev` 상태를 기준으로, **심플하지만 본질적으로 강한 자율 에이전트**로 발전시키기 위한 로드맵과 구현 계획을 정리한 문서 묶음이다.

구성:
- `docs/01_CURRENT_STATUS.md` — 현재 저장소 상태와 핵심 판단
- `docs/02_AUTONOMOUS_AGENT_MENTAL_MODELS.md` — 설계의 기준이 되는 멘탈 모델
- `docs/03_AUTONOMOUS_ROADMAP.md` — 단계별 제품 로드맵
- `docs/04_PHASED_IMPLEMENTATION_PLAN.md` — 실제 구현 계획
- `docs/05_A_TO_Z_TASKS.md` — A to Z 태스크 보드
- `docs/06_SELF_REVIEW.md` — 자체 피드백
- `data/tasks.csv` / `data/tasks.json` — 작업 관리용 데이터

핵심 결론:
1. 현재 AxonRunner는 **잘 다져진 minimal CLI runtime**이다.
2. 하지만 아직 **goal-oriented autonomous agent product**는 아니다.
3. 다음 단계는 기능 폭 확대가 아니라, **단일 에이전트의 목표 실행 루프(plan → act → verify → repair → report)** 를 제품 중심으로 올리는 일이다.
4. `codek`/`codex-runtime`는 substrate로만 두고, **정책·추적·검증·승인·재현성은 AxonRunner가 소유**해야 한다.
