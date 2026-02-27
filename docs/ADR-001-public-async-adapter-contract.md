# ADR-001: Public Async Adapter Contract Migration

- Date: 2026-02-27
- Status: Accepted
- Owner: Runtime/Adapters
- Related Task: `AS-008`, `NX-010`, `NX-011`

## 1. Context

현재 `adapters/src/contracts.rs`의 핵심 공개 계약은 동기 트레이트입니다.

- `ProviderAdapter::complete(...) -> AdapterResult<ProviderResponse>`
- `ChannelAdapter::send(...) -> AdapterResult<ChannelSendReceipt>`
- `ToolAdapter::execute(...) -> AdapterResult<ToolOutput>`

앱 계층은 고부하 리스크 완화를 위해 이미 `AsyncRuntimeHost + run_blocking` 경계를 도입했고,
어댑터 내부 HTTP 경로도 `AsyncHttpBridge`로 점진 전환했습니다.  
즉, 내부 구현은 비동기 친화적으로 이동했지만 공개 계약은 동기 인터페이스를 유지합니다.

이 상태는 호환성에는 유리하지만, 다음 제약을 남깁니다.

1. 호출 경계마다 blocking bridge 오버헤드가 발생한다.
2. cancellation/timeout/backpressure 의미가 계약 타입에 직접 드러나지 않는다.
3. 장기적으로 sync/async 이중 의미를 유지해야 해 구조적 복잡도가 증가한다.

## 2. Decision Drivers

1. 고부하에서 지연/처리량 안정성
2. timeout/cancel/backpressure 의미를 계약에 직접 반영
3. sync/async 이중 경로 제거로 운영 복잡도 축소
4. `dyn` 기반 어댑터 확장성 유지
5. 대규모 리팩토링의 명확한 소유권과 종료 조건

## 3. Considered Options

### Option A: 현행 동기 계약 유지

- 장점: breaking change 없음, 즉시 안정적
- 단점: blocking bridge 의존 지속, async 의미가 계약에 반영되지 않음

### Option B: 동기/비동기 이중 계약 장기 병행

- 장점: 마이그레이션 충격 완화
- 단점: 중복 코드/중복 테스트/중복 문서 비용이 장기 누적

### Option C: 공개 async 계약(v2) 하드 컷오버 (무호환/무deprecation)

- 장점: 구조 단순화가 즉시 발생, 계약 의미가 명확함
- 단점: 단기 breaking impact가 큼, 컷오버 품질 게이트가 필수

## 4. Decision

`Option C` 채택.  
**호환성 유지 없음, deprecation 기간 없음**을 기본 정책으로 하며, 공개 sync 계약은 컷오버 시점에 제거한다.

## 5. Breaking Scope

대상: 공개 어댑터 계약(`ProviderAdapter`, `ChannelAdapter`, `ToolAdapter`)의 호출 메서드.

예상 변경 축:

1. sync 호출 메서드를 async 호출 메서드로 대체
2. 공개 sync 계약(트레이트/호출 경로) 제거
3. 호출자(앱 런타임)에서 `run_blocking` 경계를 제거
4. timeout/cancel/backpressure 정책을 async 경계에서 일관 적용

참고:

- Rust의 `async fn` trait + `dyn` 호환성 문제를 고려해 초기에는 `async-trait` 또는 boxed future 기반 인터페이스를 사용한다.
- 성능/할당 비용은 벤치마크 게이트에서 검증한다.

## 6. Migration Strategy (Phased)

### Phase M0: Design Freeze

- async 공개 계약 시그니처 고정(메서드/오류/timeout/cancel 의미).

### Phase M1: Hard Cutover Implementation

- 공개 sync 계약 제거와 async 계약 도입을 같은 변경 집합으로 적용.
- registry/호출자/테스트를 모두 async 계약 기준으로 정렬.

### Phase M2: App Runtime Cutover

- `apps` 핵심 경로를 async 계약으로 완전 전환.
- `runtime_compose_bridge::run_blocking` 의존 제거.

### Phase M3: Release and Stabilization

 - release gate 통과 후 async 계약 기준 릴리즈 고정.
 - 안정화 기간 동안 성능/회귀 모니터링 수행.

## 7. Hard-Cutover Policy

1. v1/v2 동시 운영을 허용하지 않는다.
2. deprecation window를 두지 않는다.
3. 컷오버 시점 이후 기존 sync 공개 계약은 컴파일 대상에서 제거한다.

## 8. Risk and Mitigation

1. Risk: `dyn` async trait object 복잡도 증가  
   Mitigation: v2 초기에는 `async-trait`(boxed future) 채택, 추후 native path 재평가.

2. Risk: 대량 breaking change로 인한 컴파일/런타임 회귀  
   Mitigation: 단일 컷오버 브랜치에서 adapter/app/e2e를 함께 변경하고 통합 게이트를 강제.

3. Risk: 운영 환경 timeout/cancel 의미 차이  
   Mitigation: 게이트 테스트에 timeout/cancel 시나리오 명시 추가.

## 9. Rollback Strategy

1. 코드 레벨 v1 fallback은 두지 않는다.
2. 문제 발생 시 직전 안정 릴리즈 태그/아티팩트로 롤백한다.
3. 롤백 후 필수 재검증:
   - release approval gate
   - adapter contract tests
   - perf gate smoke

## 10. Acceptance Criteria

AS-008 완료 조건:

1. ADR 문서 승인 완료
2. hard cutover 범위 확정(sync 공개 계약 제거 포함)
3. 마이그레이션/롤백 절차 승인
4. no-compatibility / no-deprecation 정책 합의

## 11. Approval Record

1. 2026-02-27: Draft 생성 (`NX-010`)
   - Decision: Proposed
   - Approver: Runtime/Adapters
2. 2026-02-27: ADR 승인 확정 (`NX-011`)
   - Decision: Accepted
   - Approver: Repository Owner (user directive in Codex session)
   - Scope: breaking scope / migration strategy / rollback policy 승인
   - Notes: "호환성필요 없고 deprecation 필요없다" 지시로 hard cutover 정책 확정

## 12. Execution Timeline (No Deprecation)

아래 일정은 2026-02-27 승인 기준 확정본이다. (deprecation window 없음)

| Date | Milestone | Contract Policy |
| --- | --- | --- |
| 2026-03-15 | M0 Design Freeze 완료 | async 공개 계약 시그니처 고정 |
| 2026-04-30 | M1 Hard Cutover 완료 | sync 공개 계약 제거 + async 공개 계약 도입 |
| 2026-06-15 | M2 App Runtime Cutover 완료 | `run_blocking` 공개 경계 제거 |
| 2026-07-31 | M3 Release/Stabilization 완료 | async 공개 계약 기준 릴리즈 고정 |

Timeline gate rules:

1. 각 마일스톤은 release approval gate + contract regression 통과 후에만 다음 단계 진입.
2. 운영 장애 발생 시 직전 안정 릴리즈 태그로 롤백.
3. 일정 조정 필요 시 ADR 업데이트와 승인 기록 갱신이 선행되어야 함.
