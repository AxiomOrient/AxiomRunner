# Feature Adoption Checklist

Use this checklist before implementing any feature, including ideas inspired by other projects.

## A. Problem Fit
- [ ] The problem is real in AxiomAI usage, not only parity-driven.
- [ ] Expected user/operations benefit is stated in one sentence.
- [ ] Success metric is observable (e.g., fewer failures, faster completion, less operator work).

## B. Data Model First
- [ ] Input/output data structures are explicit and typed.
- [ ] Invariants are defined (required fields, bounds, compatibility rules).
- [ ] Data transformations are deterministic where possible.

## C. Side Effects and Boundaries
- [ ] Pure logic is separated from IO/runtime calls.
- [ ] Side effects are explicit (where, when, and why they happen).
- [ ] Retry, timeout, and failure classes are explicit and testable.

## D. Simplicity and Abstraction Discipline
- [ ] Design uses concrete structures first.
- [ ] New abstraction is introduced only for real duplication/invariant clarity.
- [ ] Hidden indirection and decorative code are removed.

## E. Performance and Mechanical Sympathy
- [ ] Allocation and complexity hotspots are identified.
- [ ] Process spawning, file IO, and network calls are bounded and visible.
- [ ] Ownership/lifetime decisions are clear in code paths.

## F. Operations and Recovery
- [ ] Failure is observable (logs, status, doctor checks).
- [ ] Rollback path exists and is executable.
- [ ] Migration/compatibility impact is documented.

## G. Test Gate
- [ ] Unit tests for pure transforms and invariants.
- [ ] Contract tests for adapter boundaries.
- [ ] End-to-end test for expected operator-facing behavior.
- [ ] Regression case for the primary failure mode.

## H. Ship Decision
- [ ] GO only if A-G are satisfied.
- [ ] CONDITIONAL only with explicit mitigation deadline.
- [ ] NO-GO if migration safety, observability, or rollback criteria fail.
