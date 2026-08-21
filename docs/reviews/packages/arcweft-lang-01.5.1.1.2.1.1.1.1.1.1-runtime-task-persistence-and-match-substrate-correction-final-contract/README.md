# arcweft-lang-01.5.1.1.2.1.1.1.1.1.1-runtime-task-persistence-and-match-substrate-correction-final-contract

**Status: `READY_FOR_IMPLEMENTATION`**  
**Open result-changing questions: `0`**  
**Production source basis:** `3670625a02b9e7e8578b57fc7b148a1758a17dba` (`main` on 2026-08-22)  
**Request-stated production:** `17b384a36e1412cc7e7d9f13073d8dd33dcb5cbc`  
**Parent ZIP SHA-256 retained from the request:** `2B9B55043E8168D99838C81048E13F752A75B03F48293010BB36B5401043DB0B`

This is a design-only, independently throwable correction package. It closes
the nine current-repository crossings identified by the request without
reopening the retained Need/task identity roles or any AWBC numeric allocation.

## Final owner choices

The final execution owner is
`arcweft_runtime_scheduler::RuntimeTaskScheduler<A: TaskLaunchAdapter>`. It
alone owns generation journals, AlwaysStart counters, task groups/launches,
Need cells, observers, host adapter transactions, runtime-owned AwaitMany and
Timeout tasks, event application, save/restore/replay and replacement rebind.
`arcweft-runtime-driver` becomes a consumer.

`TaskSpec` has exactly one `TaskExecution` field:

```rust
pub enum TaskExecution {
    Host(HostTaskRequest),
    Runtime(RuntimeTaskRequest),
}

pub enum RuntimeTaskRequest {
    AwaitManyAggregate(RuntimeAwaitManyAggregateRequest),
    Timeout(RuntimeTimeoutRequest),
}
```

The canonical RuntimeValue visitor remains the sole value-identity grammar.
`Plain + SnapshotOnly` opaque values now receive canonical bytes and a
`RuntimeValueDigest`; constant publication is protected by a separate explicit
constant-admission fence. Affine handles never receive a canonical value
identity.

`RuntimeNeedHandle` retains canonical identity `tag 20 || NeedId`. Its public
semantic equality, hash and order use NeedId only. Complete correlation/spec
validation remains mandatory at construction, restore, Await, timeout and
replacement boundaries.

Generic Match semantic identity is built from the current
`FinalSemanticAnalysis` authority. `CheckedMatchRef` is compiler-local
`HirSnapshotId + ExprId`; persistent rows contain only accepted semantic
projections and digests.

## Package map

- `FINAL_CONTRACT.md` — closed normative result.
- `DECISION_REGISTER.md` — frozen, corrected and rejected alternatives.
- `RUST_SCHEMAS.md` — Rust-shaped owners and all snapshot/replay rows.
- `CANONICAL_VALUE_AND_CONSTANT_ADMISSION.md` — one grammar/two policy fences.
- `EXECUTION_TRUTH_TABLE.md` — all nine producer families.
- `SCHEDULER_OWNER_AND_API.md` — exact atomic owner and borrow flow.
- `STATE_MACHINES.md` — lifecycle, event, AwaitMany, timeout, cancellation and replacement.
- `FAILURE_PRECEDENCE_AND_ATOMICITY.md` — reachable errors and transaction cuts.
- `PERSISTENCE_AND_REPLAY.md` — complete strict v1 codec and atomic restore.
- `MATCH_SEMANTIC_TRANSCRIPTS.md` — exhaustive expression/pattern/literal transcripts.
- `VIEW_BUNDLE_PROJECTION.md` — compiler-local/persistent split.
- `OWNERSHIP_MATRIX.md` — all 85 current `TypeKind` variants.
- `OWNER_API_MAP.md`, `DEPENDENCY_GRAPH.md` — implementation placement.
- `SOURCE_EVIDENCE.md`, `DELETION_MATRIX.md` — current-tree evidence and removals.
- `COMPILE_CLEAN_SEQUENCE.md` — exact five protected cuts.
- `TEST_MATRIX.md`, `REQUIREMENT_TRACEABILITY.md`, `STRUCTURAL_ABSENCE.md`.
- `machine/` and `tables/` — validated machine-readable equivalents.
- `tools/validate_package.py` — read-only validator plus negative self-tests.
- `VALIDATION_OUTPUT.txt` — validator output produced for this archive.
- `inputs/` — exact user-supplied request, Rust skill and project premise.

## Verification boundary

Actually verified:

1. the attached 415-line request and the complete Rust skill were read;
2. current repository `main` was resolved through the authenticated GitHub
   connector to `3670625a02b9e7e8578b57fc7b148a1758a17dba`;
3. root/docs/reviews/implementation/crates `AGENTS.md`, maintained runtime
   contracts, predecessor frozen mirror/intake and current source owners listed
   in `SOURCE_EVIDENCE.md` were inspected;
4. the machine/prose package validator passed;
5. every required negative self-test passed against a mutated temporary copy;
6. the archive manifest and every packaged file hash were recomputed.

Specified but not executed here:

- no production code was patched;
- no local production checkout was created;
- therefore workspace `cargo fmt`, `cargo check`, `cargo clippy` and
  `cargo test` were not run. Their exact per-cut gates are specified in
  `COMPILE_CLEAN_SEQUENCE.md`;
- the predecessor ZIP's expected SHA is retained from the request. Its frozen
  mirror and intake were inspected, but the predecessor binary itself was not
  independently streamed and rehashed in this environment.

These boundaries do not leave a design choice open; they distinguish source
inspection/package validation from future production implementation testing.
