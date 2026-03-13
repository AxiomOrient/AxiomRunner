# 03. Product Quality Findings

## Good enough today

These are no longer major blockers:

1. **Truth surface is narrow**
2. **Provider failure is fatal and visible**
3. **Legacy `readonly` snapshot compatibility is fixed**
4. **Tool contract covers the essential local automation primitives**
5. **Trace/report/replay/doctor form a coherent evidence chain**

## Still below product grade

### 1. Autonomous loop is not yet canonical
The domain already models goal-run concepts, but the app still runs legacy fact-intents as the public truth.
This is the largest architecture gap.

### 2. Runtime tool phase is underpowered
The runtime currently uses the tool adapter mostly for logging/report files, not as a real step executor.
That means the substrate exists, but the harness is still missing.

### 3. Verification is too weak
Current verification proves that stages executed and artifacts were emitted.
It does **not** yet prove that the requested engineering outcome is complete.
For app/server automation, completion must mean:
- required files exist
- diff scope is acceptable
- tests/build/lint/typecheck pass
- declared acceptance checks pass

### 4. Approval semantics are incomplete
Risk tiers exist, but there is not yet a canonical blocking gate for risky operations.
Without that, `approval_required` is not yet trustworthy as a full product behavior.

### 5. Concurrent-process behavior is underdefined
This is the most important low-level quality risk.

#### 5.1 State snapshot
The snapshot file is atomically replaced, which is good.
But there is no cross-process lock, so two writers can still race and last-write-wins.

#### 5.2 Trace log
The trace log appends JSONL with plain append semantics.
That is simple and fast, but there is no explicit file lock or single-writer guard.

#### 5.3 Markdown memory
Markdown memory is rewritten atomically through temp+rename, but that still does not make it multi-writer safe.

#### 5.4 Tool append writes
Append mode uses plain append writes.
For report/log files this is fine in a single run, but not safe as a general shared multi-process log primitive.

#### 5.5 Cached provider session reuse
`provider_codex_runtime` caches a session keyed by `(cwd, model)` and returns a cloned session handle. That is efficient, but if concurrent callers share one provider instance and the underlying session is not meant to handle overlapping asks, this can become a race or protocol corruption risk.

### 6. Hidden fallback risk in async host
The async host can fall back to a default runtime when configured init fails.
This is recoverable behavior, but it weakens the project’s explicit “no hidden fallback” philosophy.

### 7. Some tool APIs are still too permissive
- `replace_in_file` should support exact expected replacements / max replacements
- `remove_path` should sit behind explicit approval plumbing in autonomous mode
- `search_files` should expose unreadable/skipped-file counts for operator visibility

## Duplication / complexity review

### Good: most earlier structural duplication is gone
The workspace is much simpler than before.
There is no sign of platform-width duplication like channels/gateways/daemon/service trees.

### Remaining duplication risk
The real duplication risk is not folders.
It is **split product contracts**:
- legacy intent runtime
- aspirational goal-run runtime
- current tool substrate
- future workflow harness

If these all evolve independently, AxonRunner will drift again.

## Product quality verdict

### Release-grade as:
- a minimal local runtime substrate
- a bounded CLI contract for deterministic local operations

### Not yet release-grade as:
- an autonomous developer agent that can take a spec, execute a multi-step engineering workflow, verify milestones, repair failures, and continue automatically

That is the gap the next product definition must close.
