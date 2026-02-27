# Release Approval & Renewal Readiness Runbook

## 1) Production 배포 필수 게이트 (Release Approval Gate)

배포 직전에는 아래 단일 게이트를 반드시 통과해야 합니다.

```bash
bash scripts/run_release_approval_gate.sh
```

이 스크립트는 다음 3개 항목을 한 번에 검증합니다.

- `security_gate_debug` (`cargo test -q -p axiom_apps --test release_security_gate`)
- `security_gate_release` (`cargo test -q --release -p axiom_apps --test release_security_gate`)
- `perf_gate` (`scripts/run_benchmarks.sh`)

### 합격 기준

`target/release-approval-gate/report.json`이 아래 조건을 만족해야 합니다.

```bash
jq -e '
  .suite == "release_approval_gate_v1" and
  .security_gate_debug == "pass" and
  .security_gate_release == "pass" and
  .perf_gate == "pass" and
  .passed == true and
  (.errors | length == 0)
' target/release-approval-gate/report.json
```

### 필수 아티팩트

- `target/release-approval-gate/report.json`
- `target/release-approval-gate/security_gate_debug.log`
- `target/release-approval-gate/security_gate_release.log`
- `target/release-approval-gate/perf_gate.log`
- `benchmarks/summary.md`
- `benchmarks/summary.tsv`

### 장애 대응 절차 (Runbook)

1. 배포 즉시 중지: `report.json`에서 실패 필드 확인 전에는 릴리즈 진행 금지.
2. 실패 필드 판별: `security_gate_debug`, `security_gate_release`, `perf_gate` 중 실패 항목 식별.
3. 로그 우선 확인: `target/release-approval-gate/<field>.log`에서 첫 실패 지점 추출.
4. 원인별 조치 후 재실행: 실패 항목 수정 후 `bash scripts/run_release_approval_gate.sh` 재실행.
5. 재검증 완료 시 배포 재개: `passed=true`와 `errors=[]` 확인 후만 배포 재개.

### 실패 유형별 1차 조치 가이드

| 실패 항목 | 1차 확인 | 즉시 조치 |
| --- | --- | --- |
| `security_gate_debug` | `target/release-approval-gate/security_gate_debug.log` | `release_security_gate` 관련 코드/설정 회귀 수정 후 debug 테스트 재실행 |
| `security_gate_release` | `target/release-approval-gate/security_gate_release.log` | release 빌드 전용 분기/옵션 확인 후 release 테스트 재실행 |
| `perf_gate` | `target/release-approval-gate/perf_gate.log`, `benchmarks/summary.md` | 임계값 초과 target 식별 후 성능 회귀 수정, 필요 시 벤치 설정 재확인 |

## 2) 확장 게이트 (Renewal Readiness)

전환 리허설/계약 회귀까지 포함한 확장 점검이 필요하면 아래를 실행합니다.

```bash
bash scripts/run_renewal_readiness.sh
```

`target/release-readiness/report.json` 합격 기준:

```bash
jq -e '
  .suite == "renewal_readiness" and
  .release_gate_debug == "pass" and
  .release_gate_release == "pass" and
  .adapter_contract == "pass" and
  .cli_contract == "pass" and
  .transition_gates == "pass" and
  .benchmarks == "pass" and
  .passed == true and
  (.errors | length == 0)
' target/release-readiness/report.json
```

## 3) Fast Local Dry Run

빠른 로컬 점검(저부하) 기준:

```bash
RELEASE_GATE_BENCH_ITERATIONS=1 \
RELEASE_GATE_BENCH_RECORDS=200 \
RELEASE_GATE_BENCH_WARMUP=0 \
RELEASE_GATE_BENCH_REQUIRED_CONSECUTIVE_PASSES=1 \
RELEASE_GATE_BENCH_MAX_PASSES=1 \
bash scripts/run_release_approval_gate.sh
```

필수 확인:

```bash
jq -e '.passed == true' target/release-approval-gate/report.json
grep -q 'gate result: `pass`' benchmarks/summary.md
```
