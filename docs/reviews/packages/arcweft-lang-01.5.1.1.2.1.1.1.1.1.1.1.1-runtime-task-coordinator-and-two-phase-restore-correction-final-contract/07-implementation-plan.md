# File-by-file implementation plan

This is an implementation plan only. No production overlay/patch is included.

## 1. Ordered work packages

| Order | Target owner/file | Concrete change | Dependency | Admission gate |
|---:|---|---|---|---|
| 1 | latest applicable `AGENTS.md`; Cargo workspace | confirm repository commands, feature policy, MSRV, unsafe policy, and exact owner modules at `UNAVAILABLE` | none | record source map; no code change yet |
| 2 | `crates/arcweft-runtime/src/task/persistence.rs` | extend the existing canonical record-kind/version enum and original `impl` with `RestorePrepared` and `RestoreCommitted`; add strict decoder/reducer | 1 | golden bytes, truncation, unknown-version tests pass |
| 3 | `crates/arcweft-runtime/src/task/plan.rs` | expose/reuse canonical plan semantic child/seal verification needed by detached restore; add behavior to original owned enum/`impl` if missing | 1 | normal admission and restore produce identical seals |
| 4 | `crates/arcweft-runtime/src/task/handle.rs` | add private prepared handle slots and infallible prepared→published batch conversion; preserve canonical slot/generation | 1,3 | isomorphism property tests pass |
| 5 | `crates/arcweft-runtime/src/task/match_substrate.rs` | add detached snapshot builder, complete transcript/coverage verification, and private publish conversion | 1,3 | generic-match complete-coverage tests pass |
| 6 | `crates/arcweft-runtime/src/task/coordinator.rs` | add coordinator fields for restore serial state, pending publication, epoch, and single aggregate published root; migrate readers to one-root snapshot if necessary | 1,4,5 | old/new epoch visibility tests pass |
| 7 | coordinator restore sibling/module | implement `prepare_restore`, exact error mapping, budgets, graph/index validation, independent digest recomputation | 2–6 | all prepare-negative tests prove zero live mutation |
| 8 | coordinator restore sibling/module | implement consuming `commit_restore`, journal ordering, idempotency reducer, preconstruction, atomic publication, receipt | 2,6,7 | crash matrix CP-00..CP-11 passes |
| 9 | startup recovery owner | replay COMMITTED records before scheduler opens; treat PREPARED as non-visible; stable receipt synthesis | 2,8 | process restart/fault injection tests pass |
| 10 | cancellation/shutdown owners | integrate non-cancellable post-commit completion, restore gating, task cancellation after publication | 6–9 | race/loom matrix passes |
| 11 | observability owner | add bounded metrics/events at prepare/decision/publish/replay boundaries | 7–10 | no payload leakage; metric timing tests |
| 12 | docs/review package | update contract links and implementation evidence with exact final SHA/commands | all | acceptance checklist complete |

## 2. Required refactoring rule

When a required record/state/carrier variant belongs to an enum already defined inside arcweft, edit that enum and its original `impl`. Do **not**:

- add a restore-only extension trait;
- convert variants to strings and switch ad hoc;
- add a duplicate helper enum solely to avoid touching the owner;
- hide missing semantics in a free function next to the caller.

A private helper is acceptable only for cohesive mechanics inside the owning module, not as a second semantic authority.

## 3. Commit-sized implementation slices

1. canonical journal types + reducer + golden tests;
2. detached plan/handle/match restore builders + negative tests;
3. coordinator aggregate root and read-path epoch consistency;
4. prepare API + digest/graph/limit tests;
5. commit API + idempotency/conflict tests;
6. crash hooks + recovery;
7. cancellation/shutdown concurrency tests;
8. observability, docs, full workspace gates.

Each slice must compile and keep existing behavior green. Do not land a temporary task-by-task publication path.

## 4. Source reconciliation checklist before coding

- locate the actual task coordinator or closest task-table owner and make it the sole owner;
- locate all readers of task map, handle batch, and match substrate; convert them to one epoch/root snapshot;
- locate the current persistence version/record enum and canonical encoder; extend it in place;
- locate task identity/generation definitions and prohibit a restore-local duplicate;
- locate snapshot digest/seal and semantic child encoder APIs; confirm normal admission and restore share them;
- locate runtime shutdown/cancellation gates and scheduler-open boundary;
- locate test support for deterministic crash/fault injection and concurrency modeling;
- read nested `AGENTS.md` for every target path before edits.

## 5. Completion definition

Implementation is complete only when:

- every `01-request-coverage.md` row links to source and a passing test;
- CP-00..CP-11 are deterministic fault-injection tests;
- the old/new root concurrency test proves no mixed epoch;
- persisted golden bytes and strict decoder negatives pass;
- same-token replay returns byte/field-equal receipt;
- current main workspace commands required by `AGENTS.md` pass;
- no production code uses a second restore authority or ad-hoc owned-enum workaround;
- source evidence document records the implementation commit, not just this design SHA.
