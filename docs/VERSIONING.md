# Versioning And Changelog Policy

## Versioning

- retained CLI surface를 깨면 minor가 아니라 breaking release로 취급한다.
- provider compatibility minimum을 올릴 때는 CHANGELOG와 decision record를 같이 남긴다.
- experimental capability 변화는 primary product contract와 분리해 기록한다.
- public exit code도 retained CLI surface 일부로 본다.

## Exit Codes

- `0`: success
- `2`: parse error
- `3`: config error
- `4`: release gate error
- `5`: runtime init error
- `6`: runtime execution error
- `7`: runtime shutdown error

## v0.1 Release Gate

v0.1 출하 전에는 아래가 모두 잠겨 있어야 한다.

- retained CLI surface: `run`, `status`, `replay`, `resume`, `abort`, `doctor`, `health`, `help`
- operator-visible blocker: approval_required, budget_exhausted, blocked, failed, aborted
- docs truth lock: README, RUNBOOK, CAPABILITY_MATRIX, charter, release gate 테스트가 같은 surface를 말함
- autonomy evidence: `autonomous_eval_corpus`, `fault_path_suite`, `nightly_dogfood_contract`, `release_security_gate`

이 중 하나라도 깨지면 version을 올리지 않고 release를 막는다.

## Changelog

- retained commands 변화는 첫 줄에 드러나야 한다.
- removed product surface와 experimental surface는 분리해서 적는다.
- substrate pin 변경은 version, 이유, rollback condition과 함께 적는다.
- exit code 변경은 breaking change로 적는다.
