## 결론

**처음 방향으로 완전히 되돌아가는 것은 비추천**입니다.
가장 좋은 방법은 **지금의 좁은 런타임 방향을 유지하되, 그 위에 자율성을 다시 올리는 것**입니다.

정확히는 이렇게 가야 합니다.

> **AxiomRunner를 “작은 CLI 런타임”으로 끝내는 것이 아니라,
> 현재의 좁고 정직한 kernel 위에
> workflow pack + verifier-first loop + operator-visible evidence를 올려
> “single-agent autonomous developer harness”로 완성하는 것.**

이 판단의 근거는 현재 레포가 이미 그렇게 스스로를 정의하고 있기 때문입니다. README는 제품을 “로컬 워크스페이스 자동화를 위한 goal-file 중심 CLI agent runtime”으로 정의하고, 정식 공개 표면을 `run / status / replay / resume / abort / doctor / health / help`로 좁게 고정합니다. charter도 같은 retained surface를 명시하고, multi-channel messaging, daemon/service lifecycle, HTTP gateway, cron, skills marketplace, integrations catalog를 non-goal로 밀어냈습니다. 동시에 autonomous target 문서는 single-agent first, workspace-bound execution, verify-before-done, hidden fallback 금지, eval-driven release를 비타협 원칙으로 둡니다. workflow pack contract도 본체가 `goal / run / resume / abort / trace / report / done`의 뜻을 소유하고, pack은 planner hints, allowed tools, verifier rules만 제공해야 한다고 못 박습니다. 즉 현재 저장소의 “의도된 최종 형태”는 이미 **좁은 커널 + 강한 자율 루프**입니다. ([GitHub][1])

외부 설계 원칙도 같은 결론을 지지합니다. Anthropic은 성공적인 에이전트 구현이 복잡한 프레임워크보다 **simple, composable patterns** 위에서 잘 나온다고 말하고, OpenAI Codex는 장시간 코딩 자동화의 핵심을 `Plan → Edit code → Run tools (tests/build/lint) → Observe → Repair → Update docs/status → Repeat` 루프로 설명합니다. LangChain도 workflow는 predetermined code path, agent는 dynamic tool use라고 구분하고, multi-agent가 항상 필요한 것은 아니라고 설명합니다. 지금 AxiomRunner의 문서가 말하는 방향과 거의 같습니다. ([Anthropic][3])

---

## 왜 “지금 방향 유지 + 자율성 재탑재”가 맞는가

현재 레포를 기준으로 보면, 이미 중요한 선택이 끝나 있습니다.

첫째, **제품 표면을 좁혔습니다.**
README와 capability matrix는 core capability를 `run / resume / abort / doctor / replay / status / health / help`, persisted state snapshot, `codek`, `mock-local`, core tools, workspace boundary, allowlist, failure propagation, plan/apply/verify/report artifact, regression suite로 제한합니다. broad platform 기능은 experimental로 밀렸습니다. 이건 단순 축소가 아니라 **무엇을 반드시 제품급으로 만들 것인지**를 정한 것입니다.

둘째, **자율성을 버린 것이 아닙니다.**
autonomous target과 spec는 goal schema, done condition, verification plan, budget, approval policy, terminal outcomes, replayable evidence를 계속 잠가야 할 public contract로 둡니다. 즉 현재 방향은 “자율성 포기”가 아니라 “자율성의 계약을 넓은 플랫폼이 아니라 좁은 제품 위에 다시 세운다”에 가깝습니다. ([GitHub][4])

셋째, **workflow pack이 이미 재확장 경로로 정의돼 있습니다.**
pack contract는 explicit pack manifest, allowed tools, verifier rules, risk policy를 제공할 수 있게 하되, run phase / terminal outcome / replay schema를 pack이 제멋대로 만들 수 없게 막습니다. 이게 핵심입니다. 다시 말해 “자율 앱/서버 개발”은 돌아갈 필요가 있는 과거 방향이 아니라, **현재 구조 위에 올릴 다음 층**입니다. ([GitHub][5])

---

## 내가 추천하는 최선의 방향

### 추천안: `AxiomRunner = Autonomous Kernel + Workflow Packs`

이 프로젝트는 아래처럼 정의하는 게 가장 좋습니다.

**커널**

- goal schema
- run lifecycle
- status / replay / resume / abort / doctor
- trace / report / evidence
- budget / approval / terminal outcome
- workspace safety / failure propagation

**팩**

- rust service pack
- node api pack
- nextjs app pack
- python fastapi pack
- 이후 iOS / Android / desktop pack

**어댑터**

- provider adapter
- tool adapter
- verifier adapter
- domain workflow adapter

이 구조가 좋은 이유는, 사용자 목표인 “기획문서를 제공하면 그 문서를 기반으로 개발 워크플로우를 받아서 작업하고, 결과를 분석해 다음 작업을 자동 진행”을 만족시키면서도, 제품 의미를 흐리지 않기 때문입니다. AxiomRunner 본체가 lifecycle과 evidence를 소유하고, pack/adapter는 도메인별 실행 방법만 주는 방식이 가장 안전합니다. 이건 현재 workflow pack contract와 정확히 맞습니다. ([GitHub][5])

---

## 지금 레포를 기준으로 남은 핵심 문제

현재 방향이 맞더라도, 그대로 제품급이라고 부르기엔 몇 가지가 남아 있습니다.

가장 큰 문제는 **기본 자율 실행 경로의 깊이**입니다. 현재 goal/runtime 문서는 verification과 done condition을 제품 계약의 중심에 두는데, 기본 goal workflow pack 파생 경로가 placeholder 성격의 verifier를 가질 수 있었습니다. 이건 “verify-before-done” 철학과 긴장이 있습니다. 자율 에이전트가 되려면 explicit pack이 없을 때도 `weak/unresolved/pack_required` 같은 상태를 operator-visible하게 올리고, 강한 verifier 없이는 진짜 success를 주지 않는 쪽이 맞습니다. workflow pack contract는 이미 이 vocabulary를 제공합니다. ([GitHub][4])

또 하나는 **문서 truth lock**입니다. 현재 README, charter, capability matrix, autonomous docs는 전반적으로 같은 방향을 말하지만, backlog/계약 문서가 중복되거나 뒤처지면 operator와 구현자의 mental model이 갈라집니다. 지금처럼 좁은 제품은 문서 한 벌이 절대 기준이어야 합니다. ([GitHub][1])

---

## 내가 제안하는 최종 청사진

### 제품 한 줄 정의

**AxiomRunner는 goal file과 workflow pack을 받아, 로컬 workspace에서 개발 작업을 계획·실행·검증·복구·보고까지 끝내는 single-agent autonomous developer runtime이다.**

### v1에서 반드시 되는 것

- goal file 입력
- workspace-safe execution
- plan/apply/verify/report artifact
- status / replay / doctor
- approval / budget / abort / resume
- explicit verifier strength
- representative app/server workflow packs
- release gate + nightly + eval corpus

### v1에서 하지 않는 것

- multi-agent orchestration
- daemon/service/gateway
- broad integrations catalog
- skills marketplace
- channels
- long-term generalized memory platform

이 분리가 중요합니다. “단순함”은 기능 축소가 아니라 **핵심 경로를 거짓 없이 완전히 닫는 것**이어야 하기 때문입니다.

---
