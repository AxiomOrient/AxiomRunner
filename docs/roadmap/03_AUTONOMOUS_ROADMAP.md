# 03. Autonomous Roadmap

## 목표 정의

AxonRunner의 다음 제품 목표는 아래다.

> **로컬 워크스페이스에서 주어진 goal을 자율적으로 끝까지 수행하고, 그 과정과 결과를 설명 가능하게 남기는 단일 에이전트.**

이 목표는 “심플한 제품”이 아니라, **핵심 기능이 끝까지 정확히 동작하는 제품**을 뜻한다.

---

## Phase R0 — Product Contract Lock

### 목적
현재 minimal runtime을 **autonomous agent product contract**로 전환할 준비를 한다.

### 산출물
- `AUTONOMOUS_AGENT_SPEC.md`
- run contract
- goal schema
- done condition schema
- budget / approval schema
- CLI truth surface 초안

### 종료 조건
- 제품이 더 이상 `read/write/remove` 중심이 아니라 **goal run 중심**으로 정의된다.
- 문서, help, release gate가 같은 제품을 말한다.

### 비목표
- multi-agent
- browser
- external integrations 확대

---

## Phase R1 — Goal-Oriented Domain Remodel

### 목적
현재 fact-oriented intent surface 아래에 **goal/run domain**을 만든다.

### 산출물
- `RunGoal`
- `RunStatus`
- `RunPhase`
- `RunBudget`
- `RunApprovalMode`
- `RunEvent` / `RunProjection`
- backward compatibility shim

### 종료 조건
- 런타임 내부 truth가 fact mutation이 아니라 **goal execution state machine**으로 바뀐다.

---

## Phase R2 — Single-Agent Autonomous Loop

### 목적
AxonRunner의 핵심 루프를 제품의 중심으로 올린다.

### 루프
1. Understand goal
2. Select context
3. Create bounded plan
4. Execute one step
5. Verify result
6. Repair if failed
7. Update trace/report
8. Continue or stop

### 산출물
- planner
- executor
- verifier
- repair loop
- done evaluator

### 종료 조건
- `run "<goal>"` 가 단일 autonomous loop를 실제로 수행한다.
- 멈춤 이유가 명확하다: success / blocked / budget_exhausted / approval_required / failed

---

## Phase R3 — Tool & Verification Hardening

### 목적
agent가 실제로 일을 끝낼 수 있는 최소 도구와 검증기를 완성한다.

### 필수 도구
- list_files
- read_file
- search_files
- file_write
- replace_in_file
- remove_path
- run_command

### 필수 verifier
- file existence / content check
- build/test/lint runner
- patch evidence
- changed path summary

### 종료 조건
- agent가 “수정했다”가 아니라 “검증했다”까지 끝낸다.

---

## Phase R4 — Context & Memory Engineering

### 목적
긴 작업에서도 drift 없이 유지되는 context discipline을 만든다.

### 산출물
- run summary compaction
- selected files manifest
- artifact index
- working memory vs recall memory 분리
- `AGENTS.md` durable instruction ingestion

### 종료 조건
- context가 비대해져도 run quality가 급락하지 않는다.
- replay만으로 핵심 판단과 변화를 다시 이해할 수 있다.

---

## Phase R5 — Safety, Approval, Resume

### 목적
자율성을 늘리되 operator control을 잃지 않는다.

### 산출물
- approval policy (`never`, `on-risk`, `always`)
- budget policy (steps / minutes / tokens)
- checkpoint / resume
- abort
- rollback / restore semantics
- git checkpoint or worktree isolation

### 종료 조건
- long-running run이 중간 실패/중단 후에도 안전하게 이어질 수 있다.
- 고위험 작업은 승인 없이는 실행되지 않는다.

---

## Phase R6 — Autonomy Eval & Release Gate

### 목적
“되겠지”가 아니라 “검증됐다” 상태로 만든다.

### 산출물
- golden autonomy corpus
- long-horizon benchmark
- false-success / false-done counters
- release gate checklist
- nightly dogfood runs

### 종료 조건
- release decision이 감으로 이뤄지지 않는다.
- 주요 실패 유형이 전부 재현 가능하다.

---

## Phase R7 — Product Polish & Dogfooding

### 목적
실제 사용 가능한 제품으로 마감한다.

### 산출물
- concise CLI UX
- report bundle
- runbook for daily use
- migration notes
- release packaging
- example projects

### 종료 조건
- 개발자가 AxonRunner를 자기 프로젝트에 바로 써서 가치가 나온다.
- operator가 문서 없이도 `run → status → replay → doctor` 흐름을 이해한다.

---

## 명시적 비목표

아래는 roadmap 후반부까지도 기본 비목표로 둔다.
- multi-channel chat agent
- gateway/server mode
- cron/daemon first architecture
- skills marketplace first
- broad MCP integration sprawl
- multi-agent swarm orchestration

## 성공 기준

AxonRunner가 자율 에이전트가 되었다고 말할 수 있으려면 아래를 만족해야 한다.
- 한 개 workspace goal을 끝까지 수행
- 실패 시 멈추고 설명
- 결과가 replay 가능
- 고위험 작업은 통제 가능
- golden task success rate가 안정적
- 문서와 실제 동작이 같다
