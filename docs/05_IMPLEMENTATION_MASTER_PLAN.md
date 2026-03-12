# 05. A to Z Implementation Master Plan

이 계획은 **현재 Option C 정합화 작업 이후**, AxonRunner를 실제 제품으로 수렴시키기 위한 전 범위 계획이다.

시간 추정이 아니라 **순서, 산출물, 종료 기준** 중심으로 작성한다.

---

## A. Alignment Freeze

### 목표
- 제품 정의를 “단일 workspace task completion engine”으로 고정
- 릴리즈에서 보장할 capability를 확정

### 산출물
- `docs/01_PRODUCT_SPEC.md`
- `docs/04_CAPABILITY_MATRIX.md`

### 종료 기준
- README 초안과 capability matrix가 일치
- maintainers가 비목표를 명시적으로 승인

---

## B. Boundary Freeze

### 목표
- `core / runtime / cli / schema / experimental` 목표 구조를 확정
- 현재 저장소에서 core path와 experimental path를 분리

### 산출물
- crate mapping 문서
- feature flag 정책

### 종료 기준
- 기본 build에 experimental 기능이 섞이지 않음

---

## C. Coclai Removal

### 목표
- `coclai` path dependency 제거
- legacy agent-specific backend layer 제거

### 산출물
- provider-backed runtime contract
- Cargo dependency 변경

### 종료 기준
- 저장소 단독 clone 기준 backend compile 가능
- `apps/adapters` 기본 build에 agent 전용 surface가 남지 않음

---

## D. Dependency Policy Lock

### 목표
- runtime 필수 의존성만 남기고 product dependency graph를 축소

### 산출물
- dependency allowlist
- `cargo deny`/license policy

### 종료 기준
- 핵심 build graph가 설명 가능할 정도로 단순해짐

---

## E. Event Taxonomy Completion

### 목표
- run/session/turn/file_patch/command/verification/final_outcome event 정의

### 산출물
- `core` event 확장
- replay projection 규약

### 종료 기준
- `run`의 모든 핵심 단계를 event로 복원 가능

---

## F. Filesystem Core Hardening

### 목표
- list/read/search/patch write를 deterministic 하게 구현

### 산출물
- workspace scan/search/read/write 모듈
- path normalization / root boundary enforcement

### 종료 기준
- path escape regression suite pass

---

## G. Guard Rails

### 목표
- allowlisted command execution, timeout, output caps, env boundary 고정

### 산출물
- `command.run` runtime module
- safe command config

### 종료 기준
- shell injection/timeout regression pass

---

## H. Hook Bridge

### 목표
- codek hook lifecycle을 AxonRunner event log와 연결

### 산출물
- pre/post run/session/turn/tool hooks
- hook-to-event mapper

### 종료 기준
- trace에서 backend lifecycle이 관측됨

---

## I. Interface Unification

### 목표
- `run`만을 canonical execution path로 고정
- CLI와 runtime compose의 중복 제거

### 산출물
- unified runner entrypoint
- CLI thin wrapper

### 종료 기준
- `run` 경로가 하나뿐임

---

## J. JSON / Trace Schema Lock

### 목표
- SQLite/JSONL trace schema 확정
- replay가 schema-stable 하도록 설계

### 산출물
- migrations
- trace schema docs

### 종료 기준
- schema migration + replay compatibility test pass

---

## K. Kill Product Surface Noise

### 목표
- 채널/게이트웨이/데몬/서비스/metrics/cron을 제품 표면에서 제거

### 산출물
- experimental namespace 또는 feature gate
- README/CLI/help 정리

### 종료 기준
- 제품 도움말에서 비핵심 기능이 사라짐

---

## L. Loop Design Finalization

### 목표
- inspect -> plan -> apply -> verify -> explain 루프를 canonical contract로 고정

### 산출물
- runner loop state machine
- stage transition tests

### 종료 기준
- 각 단계 실패가 명시적으로 모델링됨

---

## M. Mock Backend First-Class Support

### 목표
- deterministic golden tests와 failure simulation을 위한 mock backend 완성

### 산출물
- mock backend contract
- canned scenario corpus

### 종료 기준
- 네트워크/실제 backend 없이 대부분의 regression test 가능

---

## N. Non-Core Extraction

### 목표
- experimental 모듈을 별도 feature 혹은 crate로 추출

### 산출물
- feature matrix
- compile profiles

### 종료 기준
- 기본 제품 build가 작고 설명 가능함

---

## O. Observability Minimums

### 목표
- dashboard 없이도 디버깅 가능한 최소 관측성 확보

### 산출물
- structured tracing
- run summary
- doctor output

### 종료 기준
- 실패 run 하나만으로 원인 추적 가능

---

## P. Prompt Contract

### 목표
- system/task/context prompt 조립 규칙 확정

### 산출물
- prompt builder
- prompt digest logging

### 종료 기준
- prompt가 버전 관리되고, trace에 digest 기록

---

## Q. Quality Gates

### 목표
- fmt/clippy/test/deny/contract/replay/golden-task gate 고정

### 산출물
- local gate scripts

### 종료 기준
- release gate가 사람이 아닌 스크립트로 판정됨

---

## R. Replay Engine

### 목표
- `replay --run-id`가 실제 세션을 재현/요약할 수 있어야 함

### 산출물
- replay loader
- projection renderer

### 종료 기준
- failure run을 replay로 재진단 가능

---

## S. Security & Safety Contract

### 목표
- write, command, workspace boundary, backend approval, privileged escalation 정책 고정

### 산출물
- safety spec
- security regression tests

### 종료 기준
- 우회 시나리오 회귀 테스트 pass

---

## T. Test Corpus Construction

### 목표
- 실제 사용자 작업을 닮은 golden corpus 만들기

### 산출물
- inspect-only tasks
- patch tasks
- fix-and-verify tasks
- failure recovery tasks

### 종료 기준
- release는 corpus pass 없이는 불가

---

## U. UX / CLI Finalization

### 목표
- 사용자 CLI 경험을 차갑고 명료하게 고정

### 산출물
- concise help
- machine-readable output option
- exit code table

### 종료 기준
- operator가 help만 보고 run/doctor/replay 사용 가능

---

## V. Verification Phase Excellence

### 목표
- 변경 후 반드시 적절한 검증 명령을 실행하는 습관을 product contract로 강제

### 산출물
- verification policy
- command suggestion heuristics

### 종료 기준
- “수정만 하고 검증 안 함” 케이스가 core path에서 제거

---

## W. Writing Pipeline Excellence

### 목표
- patch derivation, atomic write, diff capture, rollback-friendly artifact 저장

### 산출물
- patch store
- temp file policy
- rollback helpers

### 종료 기준
- write corruption/double-write regression pass

---

## X. eXperimental Quarantine

### 목표
- future ideas가 product release를 오염시키지 못하도록 구조적으로 차단

### 산출물
- experimental crate/module
- explicit README section

### 종료 기준
- experimental 기능이 release blocker가 아님

---

## Y. Yield Criteria / Definition of Done

### 목표
- “완료”의 의미를 인간 해석이 아니라 문서+게이트로 고정

### 산출물
- DoD checklist
- PR template
- release checklist

### 종료 기준
- DONE 상태의 조건이 자동 검증 가능

---

## Z. Zero-Gap Release

### 목표
- 문서, 구현, CLI, tests, trace, doctor 사이의 gap을 없애고 v1.0 release candidate 생성

### 산출물
- RC tag 기준 문서 묶음
- release rehearsal report
- rollback rehearsal report

### 종료 기준
- 문서/실행/검증이 1:1 일치
- 핵심 경로에서 `stub`, `coming soon`, hidden fallback 없음

---

## 전체 순서 요약

가장 중요한 실행 순서는 아래다.

1. A-B-C-D
2. E-F-G-H-I
3. J-L-M-O-P
4. Q-R-S-T-U-V-W
5. K-N-X
6. Y-Z

즉, **먼저 정체성과 경계를 잠그고**, **그 다음 substrate 교체와 핵심 루프를 완성**한 뒤, **마지막에 비핵심 표면을 격리하고 release를 잠근다**.
