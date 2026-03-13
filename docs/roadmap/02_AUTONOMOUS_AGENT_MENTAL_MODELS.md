# 02. Autonomous Agent Mental Models

이 문서는 AxonRunner를 자율 에이전트로 발전시킬 때 따라야 할 **핵심 멘탈 모델**을 정리한다.

## MM-1. 복잡한 프레임워크보다 단순한 조합이 더 잘 된다

Anthropic은 실제 현장에서 성공한 에이전트는 “복잡한 프레임워크”보다 **simple, composable patterns**를 사용했다고 말한다.

### AxonRunner에 적용
- single agent 먼저
- 최소 tool surface 먼저
- 작은 루프를 끝까지 완성
- orchestration은 나중

즉, AxonRunner의 경쟁력은 breadth가 아니라 **짧은 인과관계**다.

---

## MM-2. 멀티 에이전트보다 single-agent를 먼저 완성하라

OpenAI의 practical guide는 orchestration을 설명하면서도 **incremental approach**, 특히 **single-agent systems**로 먼저 성공하는 것이 일반적으로 낫다고 설명한다.

### AxonRunner에 적용
v1/v2 목표는 swarm이 아니다.  
다음 한 가지만 잘해야 한다.

> 하나의 에이전트가 하나의 workspace에서 하나의 goal을 계획하고, 실행하고, 검증하고, 실패를 고치고, 보고한다.

멀티 에이전트는 아래가 충족된 뒤에만 고려한다.
- 단일 에이전트 성공률 충분
- false-done 거의 없음
- replay와 eval이 안정화됨

---

## MM-3. 프롬프트보다 harness가 더 중요하다

OpenAI Codex의 long-horizon 문서는 긴 작업의 핵심이 “거대한 프롬프트”가 아니라 **agent loop**라고 설명한다.

대표 루프:
1. Plan
2. Edit
3. Run tools
4. Observe results
5. Repair failures
6. Update docs/status
7. Repeat

### AxonRunner에 적용
제품의 핵심은 모델이 아니라 **run harness**다.
- plan
- act
- verify
- repair
- report

이 루프가 AxonRunner의 실질적 제품이 되어야 한다.

---

## MM-4. Context는 유한하다. 상태를 바깥으로 빼라

Anthropic의 context engineering 문서는 context를 “critical but finite resource”라고 설명한다.

### AxonRunner에 적용
상태를 프롬프트에 쌓지 말고 외부화한다.
- state snapshot
- trace events
- artifacts
- selected file set
- concise run summary
- durable instructions (`AGENTS.md`)

즉, AxonRunner는 “큰 prompt”가 아니라 **작은 prompt + 큰 external state** 여야 한다.

---

## MM-5. Guardrails와 tracing은 처음부터 코어 인프라다

OpenAI의 governed agents 자료와 building agents track은 guardrails, tracing, observability를 core scaffolding으로 두는 것이 맞다고 본다.

### AxonRunner에 적용
아래는 option이 아니라 product contract다.
- policy as code
- approval policy
- command allowlist
- workspace boundary
- structured trace
- operator-visible failure reason
- doctor/replay/report

---

## MM-6. 에이전트는 eval이 없으면 결국 드리프트한다

Anthropic의 agent evals 글은 multi-turn agent는 실수가 누적되고 복합화되므로, **정적 테스트보다 더 엄격한 eval discipline**이 필요하다고 설명한다.

### AxonRunner에 적용
CI가 통과해도 충분하지 않다.
필요한 것은:
- golden autonomy tasks
- blocked provider / missing binary / bad diff / unsafe command scenario
- false-success / false-done 측정
- long-horizon task success rate
- replay completeness
- run artifact completeness

## MM-7. Codex/Agents SDK는 substrate일 뿐, 제품 계약은 우리가 소유한다

OpenAI는 Agents SDK가 loop handling, guardrails, tracing을 쉽게 해 주지만, 더 세밀한 제어가 필요하면 lower-level control이 유리하다고 설명한다.

### AxonRunner에 적용
`codex-runtime`는 AxonRunner의 **actuation substrate**다.
하지만 다음은 AxonRunner가 소유해야 한다.
- run lifecycle
- budgets
- approvals
- state machine
- trace schema
- report schema
- release gate
- operator UX

## 요약

AxonRunner가 따라야 할 철학은 아래 한 문장으로 압축된다.

> **하나의 agent, 하나의 workspace, 하나의 goal, 하나의 explicit loop, 하나의 traceable truth.**
