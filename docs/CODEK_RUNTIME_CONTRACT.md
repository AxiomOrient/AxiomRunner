# Codex Runtime Contract

## Scope

이 문서는 `provider=codek` substrate의 운영 의미를 고정한다.

## Session Reuse

- AxiomRunner는 요청마다 새 session을 무조건 만들지 않는다.
- 현재 cached session은 `cwd`와 `model`이 모두 같은 경우에만 재사용한다.
- `cwd` 또는 `model`이 달라지면 기존 session을 닫고 새 session을 만든다.
- closed session은 재사용하지 않는다.
- 같은 provider 인스턴스 안에서는 `ask` 를 동시에 두 번 보내지 않는다.
- 즉 session reuse가 있더라도 현재 정책은 per-session serialization 이다.

## Workspace Binding

- `codek` provider의 `cwd`는 runtime tool workspace와 같은 경계에 묶인다.
- workspace가 결정되지 않으면 runtime init 단계에서 fail-closed로 멈춘다.
- 따라서 provider session이 다른 임의 cwd로 흘러가는 숨은 fallback은 없다.

## Compatibility

- bundled crate pin: `codex-runtime 0.5.0`
- minimum supported Codex CLI: `0.104.0`
- `doctor --json`의 provider detail은 `cli_bin`, `version`, `compatibility`, `min_supported`를 노출한다.

## Health Meaning

- `ready`: binary probe와 handshake가 현재 계약상 통과했다.
- `degraded`: binary는 보였지만 version parse 또는 shutdown path가 깔끔하지 않다.
- `blocked`: binary missing, version minimum 미달, 또는 handshake 실패로 현재 계약을 만족하지 못한다.
