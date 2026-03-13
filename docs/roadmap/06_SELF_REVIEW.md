# 06. Self Review

## 잘한 점

1. 현재 AxonRunner의 장점을 **작은 런타임**으로 규정하고, 그 장점을 잃지 않는 방향으로 roadmap을 그렸다.
2. ZeroClaw/OpenClaw처럼 표면을 넓히는 대신, **single-agent autonomous harness**에 집중했다.
3. 현재 저장소의 실제 강점(`core`, `doctor`, `trace`, `codek` contract, tests)을 출발점으로 삼았다.
4. 단순한 “기능 목록”이 아니라, product contract → domain → harness → tool → context → safety → eval 순서로 설계했다.

## 남은 리스크

1. 이 분석은 **정적 저장소 검토 + 문서/코드 읽기**를 기반으로 한다. 이 환경에서는 실제 `cargo test`, `codex` binary probing, long-horizon dogfood run을 실행하지 못했다.
2. 현재 코드 일부는 raw rendering 한계 때문에 세부 구현까지 line-by-line 확인하지 못했다. 따라서 roadmap은 **현실적인 구조 판단**에는 강하지만, 모든 함수 단위의 미세 버그를 보장하진 않는다.
3. `codex-runtime`의 upstream 변화는 AxonRunner 제품 리듬에 영향을 줄 수 있다. substrate pin과 compatibility policy는 반드시 유지해야 한다.

## 그래서 다음 구현 루프의 첫 검증 포인트

가장 먼저 닫아야 하는 태스크:
- `AR-001` ~ `AR-005`
- `AR-011` ~ `AR-018`
- `AR-030` ~ `AR-036`

이 세 묶음을 먼저 끝내야 product contract, autonomous loop, operator control, eval discipline이 같이 잠긴다.

## 최종 자기 점검 질문

- 지금 public surface는 goal-oriented 인가?
- run이 “완료”라고 말할 때 verifier evidence가 있는가?
- 실패는 실제로 실패처럼 보이는가?
- replay만으로 사람이 무슨 일이 있었는지 이해할 수 있는가?
- context가 길어질수록 품질이 나빠지는가, 아니면 compaction으로 버티는가?
- multi-agent를 도입하지 않아도 대부분의 가치가 나오는가?

이 질문들에 모두 “예”라고 답할 수 있을 때 AxonRunner는 비로소 제대로 된 자율 에이전트에 가까워진다.
