# Await Pending observer execution — 2026-08-21

## Inspected state

- Base Git revision: `b41cad7c510b78de75808f694136521befb223be`
  (`Make producer payload outcomes authoritative`).
- Branch: `main`, matching `origin/main` before this cut.
- Working tree while this record was written: dirty with the coherent Await
  observer implementation described below.
- Stable language authority:
  [`Await / Need / Result`](../01-language/await-need-result.md) and
  [`Activity`](../02-runtime/activity.md).

## Implemented result

1. RuntimePlan Await now owns source-ordered typed Pending observers. Each
   observer contains one canonical `Progress` pattern and its contextual body;
   the former flat Pending effect list is deleted from single Await.
2. Structured execution tests observers in source order and executes only the
   first match in a fresh lexical scope. A mismatch and normal observer
   fallthrough re-wait the same handle without re-evaluating the operand,
   restarting the task, or emitting a second Await-start event. Return, goto,
   break, continue, cancellation, and line-task close discard the active
   observer continuation.
3. Task publications are consumed by `(logical_epoch, sequence)` cursor.
   Publications received while an observer is executing are latched in order,
   including across a nested suspension, and are replayed into the same Await
   after normal observer fallthrough. One input publication cannot run the same
   observer repeatedly within a drain step.
4. Product AWBC Await carries an optional typed Progress destination and a
   verified observer resume point. Lowering emits ordinary `TestPattern`,
   scoped binding/body blocks, and a verified loop-backedge to a dedicated
   Await block. `StartTask` remains outside that backedge and executes once.
5. Product executor task and direct-Need publication cursors, queued task
   events, and Await observer resume coordinates are part of the version-1
   snapshot authority. Save snapshots project queued runtime values through
   `AwbcRuntimeValueSnapshot`; the schema evolved in place without an old
   reader or version bump.
6. Bundle host-call and static-image traversal now descends into observer
   bodies. RuntimePlan dependency closure and AOT statistics do the same.
7. Stable documentation now states first-match, no-match re-wait, normal
   fallthrough re-wait, and nonlocal termination behavior. The Activity example
   uses the canonical `Progress` observer shape rather than obsolete nominal
   progress states.

## Validation performed

### Passed

- `cargo fmt --all` and `git diff --check`.
- `cargo check --workspace --all-targets --all-features`.
- `cargo clippy --workspace --all-targets --all-features`: exit status 0.
  Existing advisory warnings remain. New observer-owned length and needless
  mutable-reference warnings were decomposed or removed before this record.
- `cargo test -p arcweft-core --all-targets --all-features`: 217 library tests,
  1 API compile boundary, 8 direct-suspension tests, 5 record-admission tests,
  2 runtime-assertion tests, and 11 runtime-ID boundary tests passed.
- `cargo test -p arcweft-runtime-plan --all-targets --all-features`: 48 library
  tests, 1 API compile boundary, 10 assertion-identity tests, 4 Product parity
  tests, and 1 iterator integration test passed.
- Focused native/Product parity proves one Progress publication is consumed
  once and only the first of two matching observers returns `"first"`.
- Focused save-snapshot coverage proves a queued canonical Progress publication
  survives live-to-save-to-live projection.
- `spec_should_pass/check/055_await_pending_progress_fields.arcw` and
  `056_await_infallible_need_payload.arcw` compiled and passed Product AWBC
  verification with zero diagnostics.
- `just test-doc` passed.
- `just structure-audit-gate` passed with 0 blocking violations.

### Failed

- The aggregate
  `spec_should_pass_check_fixtures_pass_after_refactor` suite stopped at
  `022_multiline_trait_method.arcw`: final semantic analysis reported that one
  nominal type resolution was incomplete. This fixture does not exercise
  Await observers; it was not repaired in this cut. The later positive-fixture
  convergence goal remains responsible for the aggregate gate.

## Structural review

- `arcweft-core::plan` owns the sole executable observer shape and rejects the
  engine-only completion marker at construction admission.
- `arcweft-core::engine` owns structured observer scheduling, scope unwind,
  publication latching, and same-handle re-wait.
- `arcweft-core::awbc` owns its schema, version-1 codec, verifier, fiber resume,
  product publication queue, and snapshot projection. Queued event save
  projection was split into
  `awbc/product_step/snapshot/task_publication.rs`, keeping the snapshot owner
  below its structural size trigger.
- `arcweft-runtime-plan::awbc_lower::flow` remains a large triggered owner, but
  the added code is cohesive control-flow lowering: it reuses ordinary pattern,
  scope, resume-point, and backedge machinery rather than adding a parallel
  observer bytecode table or callback model.
- No dependency direction changed, no source string is reconstructed, and no
  compatibility DTO or second reader was introduced.

## Remaining work and non-goals

- Reactive View observation of Need state is not implemented by this cut and
  remains the next independently testable part of the roadmap item.
- Await-many retains its separate bounded-fanout Pending effect behavior; this
  cut changes only source `await ... with { pending ... }` observers.
- Timeout producers, const phase fencing, positive-fixture convergence,
  Dialogue/RichText completion, and the line-plan fixture remain later goals.
