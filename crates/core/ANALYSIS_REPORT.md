# `crates/core` 아키텍처 분석 리포트

현재 `crates/core` 크레이트에 대한 아키텍처 결함과 도메인 모델링에 관한 분석 결과입니다.

## 1. Data Model

### Problem (문제 정의)

`crates/core` 모듈의 아키텍처 완전성, 상태 변경 추적의 안정성, 그리고 헥사고날 아키텍처의 도메인 경계 유지 여부에 대한 검증이 필요합니다.

### Evidence (증거)

1. **인프라스트럭처 종속성 노출**: `crates/core/src/policy.rs` (Lines 5-11) 부분에 `LOCALHOST_BIND`, `ENV_DEV_MODE`, `ENV_ALLOW_REMOTE` 등 네트워크 관련 및 환경 변수 상수가 정의되어 있어 순수 비즈니스 로직에 인프라스트럭처 정보가 침투해 있습니다.
2. **상태 이력(Revision) 정보 누락**: `crates/core/src/event.rs` (Lines 6-12)의 `DomainEvent` 구조체 내부 변형들(`IntentAccepted`, `EffectsApplied` 등)에 `state_revision`이나 고유의 `sequence_id`가 지정되어 있지 않습니다.
3. **명시적 가드 부족**: `crates/core/src/reducer.rs` (Line 7)의 `reduce` 함수 내에서 `next.revision.saturating_add(1)`를 통해 맹목적으로 리비전을 올리고 있으며, 순서가 꼬인 이벤트나 재시도된 이벤트에 대한 방어 로직이 없습니다.
4. **하드코딩된 인증 체계**: `crates/core/src/policy.rs` (Line 100)에 `intent.is_control_action() && actor != "system"` 로직이 존재하여 "system"이라는 특정 문자열 액터에 대한 권한 검사가 코어 정책 파일 내에 하드코딩되어 있습니다.

### Hypotheses (가설 및 검증)

- **가설 1 (계약 위반)**: `policy.rs`에 네트워크 BIND 나 `DEV_MODE` 환경 변수 문자열이 존재하는 것은 Gateway/App 계층에서 담당해야 할 어댑터(Adapter) 책임이 코어 도메인으로 유출된(Leaked) 아키텍처적 결함이다.
  - _검증 절차_: 해당 검사 로직(`DevModeMitigationInput` 파싱)을 `apps/src/gateway`로 분리하고, 코어 정책에는 단순한 `mitigation_enabled: bool` 값만 주입해 본다. 만약 환경 변수가 없어도 코어 도메인 로직이 온전히 실행된다면 이 가설은 참이다.
- **가설 2 (증거 누락)**: `DomainEvent` 자체에 순차 번호(Sequence/Revision IDs)가 없는 것은, 이벤트 저장소가 엄격하게 이벤트 순서를 보장하여 하나의 단일 작성자(Single-writer)에 의해서만 순서대로 나열된다고 가정한 "증거 누락" 설계 결함이다.
  - _검증 절차_: 시뮬레이션 환경에서 `DomainEvent::EffectsApplied` 이벤트를 동시에(Concurrent) 두 개 생성하여 `project()` 함수에 무작위 순서로 주입해 본다. 만약 함수가 순서 엇갈림 오류를 내지 않고 둘 다 그대로 상태에 병합(reduce)해버린다면 이 가설은 참이다.
- **가설 3 (잘못된 가정)**: 인가 체계를 특정 문자열 `"system"`으로 하드코딩한 것은, 추후 외부 IDP나 다양한 권한 레벨 확장을 불가능하게 만드는 "가정 오류(Assumption Error)"이자 개방-폐쇄 원칙(OCP) 위반이다.
  - _검증 절차_: 명백하게 어드민 권한을 갖는 사용자의 식별자(예: `admin-uuid-1234`)를 외부에서 주입하여 `FreezeWrites` 인텐트를 실행해 본다. `"system"` 문자열이 아니기 때문에 정상적인 관리자 명령임에도 실패한다면 이 가설은 참이다.

### Options (대안)

- **대안 A (전면 개편 / Aggressive Purge)**
  - 코어에서 즉시 모든 인프라 환경변수, 네트워크 참조를 지우고 `DomainEvent`마다 `sequence_id`를 강제하며, 액터를 `String`에서 `ActorRole` Enum으로 전면 교체합니다.
  - _트레이드오프_: 순수성과 안전성이 극대화되나, `apps` 및 `adapters` 전반의 저장소/스키마 구조를 대대적으로 변경해야 합니다.
- **대안 B (사용자 계층 위임 / Status Quo)**
  - `core`를 이대로 놔두고, 앞단의 `apps/src/engine/agent_loop.rs` 내에서 환경변수 매핑, 이벤트 순서 보장, 액터 식별을 전적으로 책임지게 만듭니다.
  - _트레이드오프_: 당장에 수정할 비용이 적게 들지만, 코어 도메인은 여전히 환경변수에 의존하는 등 잘못된 설계를 유지하게 됩니다.
- **대안 C (실용적 경계 수정 / Pragmatic Boundary Correction)**
  - `policy.rs`의 `ENV_` 식별자들과 네트워크 관련 코드를 코어에서 도려내어 `apps` 게이트웨이 쪽으로 이관합니다.
  - `reduce()` 내에서 현재 상태 리비전보다 과거의 것이 들어오면 무시하도록 리비전 검사를 작게 추가합니다.
  - "system" 하드코딩 역학은 일체형 상수로 분리하고 점진적으로 권한(Roles) 체계를 도입합니다.
  - _트레이드오프_: 이전 DB 이벤트를 파괴하는 거대한 마이그레이션 변경 없이, 아키텍처 헥사고날 경계 침범 이슈를 가장 빠르고 안전하게 보완할 수 있습니다.

### Decision (결정 및 권고사항)

- **권고안**: **대안 C (실용적 경계 수정)**
- **채택 이유**: 단일 책임 원칙과 육각형(Hexagonal) 아키텍처 위반(환경변수/네트워크 지식의 코어 유입)을 직접적으로 제거할 수 있는 가장 빠르고 안전한 방법입니다. 현 시점에서 이벤트 스토리지 스키마를 엎는 과도한 비용(`대안 A`) 없이 시스템의 오작동 및 데이터 무결성 훼손(`가설 2` 방어)을 합리적으로 막아냅니다.

## 2. Immediate Next Action

- 도출된 분석을 바탕으로, `crates/core/src/policy.rs` 내에서 인프라/환경 의존성을 걷어내어 상위 크레이트로 이동시키는 작업을 다음 스프린트에서 계획합니다.
- 본 분석 산출물을 검토 후, 즉시 코드 수정을 원하시면 이어서 진행하도록 지시해 주십시오.

---

_(내부 감사 페이로드 절차 이행 완료 (`crates/core/analysis_payload.md`에 `audit_resolution` 필드로 영구 보존됨). 1. 아키텍처 계약 위반, 2. 증거 누락, 3. 가정 오류 3가지의 결함을 식별함)_
