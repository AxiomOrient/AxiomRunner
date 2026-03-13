# ADR: codex-runtime 0.5.0

## Status

Accepted

## Decision

AxonRunner는 `codex-runtime 0.5.0`을 현재 bundled substrate crate pin으로 사용한다.

## Why

- crates.io published latest와 현재 codebase reality를 맞춘다.
- session lifecycle과 CLI compatibility probing을 현재 codebase에서 이미 사용 중인 contract와 정렬한다.
- `0.5.x`를 기준으로 문서, doctor, provider tests를 잠그는 편이 drift를 줄인다.

## Non-goals

- 모든 Codex CLI minor를 무제한 supported로 선언하지 않는다.
- OpenAI compat provider를 primary path로 올리지 않는다.
- substrate upgrade를 기능 확장 프로젝트로 바꾸지 않는다.

## Rollback Trigger

- `0.5.x`가 current provider contract를 깨고 빠르게 patch 할 수 없을 때
- published crate와 actual Codex CLI compatibility가 release path에서 다시 불안정해질 때

## Consequence

- `doctor`는 crate pin과 별개로 actual Codex CLI minimum compatibility를 계속 표시해야 한다.
- future bump는 이 문서와 같은 수준의 decision record를 남긴다.
