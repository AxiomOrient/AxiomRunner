# Remaining Gaps

## Status

AxonRunner is already on the `goal-file` public surface.
현재 delivery cycle 기준으로는 **open implementation gap이 없다**.
이 문서는 active implementation backlog가 아니라, 이번 cycle이 닫혔다는 사실과
앞으로 새 backlog를 열 때의 기준점을 남기는 closure note다.

## Current Closure

- async host hidden fallback 제거 완료
- `replace_in_file` expected replacement count 계약 추가 완료
- `search_files` scanned/skipped visibility 추가 완료
- goal-file default workflow-pack resolution 추가 완료
- goal-file verifier step wiring 완료
- done condition 기반 success 판정 완료
- `always` / `on-risk` / budget exhaustion semantics e2e 반영 완료
- eval corpus / release gate refresh 완료

## If A New Backlog Is Needed

다음 backlog를 새로 열 때만 아래를 다시 후보로 본다.

1. richer workflow-pack selection
- 현재는 default workflow-pack resolution이 기본 경로다.
- pack registry, manifest discovery, pack selection policy를 더 붙일지는 새 계획에서 결정한다.

2. richer verifier schema
- 현재 goal verifier는 bounded default command path를 따른다.
- verifier를 structured schema로 승격할지는 새 계획에서 다룬다.

3. richer on-risk classifier
- 현재 default pack 경로는 `on-risk` 를 보수적으로 approval-required 로 처리한다.
- finer-grained risk classifier는 새 계획에서 다룬다.

## Not Legacy

The following are still live and should not be deleted as legacy:

- compatibility CLI surface: `batch`, `read`, `write`, `remove`, `freeze`, `halt`
- legacy trace compatibility tests
- transition docs that preserve closure context and future extension boundaries
