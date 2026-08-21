# Normative final contract

MUST, MUST NOT, REQUIRED, and MAY are normative.

## C1. Baseline and precedence

Target current main `cec30b57fa734efb059d7b846b397ac7d2b0701a` or a later
main after repeating the audit. The Rust tree is identical to inspected parent
`0fa8a3b845b2dc966f181f450a1ca1f36e49d966`; the intervening commit is
documentation-only. Maintained unary-Need/View documentation supersedes stale
direct-Await parent rows. Every other accepted parent decision remains.

## C2. Sole checked owner

`FinalSemanticAnalysis` MUST own exactly one immutable
`Arc<CheckedViewCatalog>`. The catalog owns every View-context unary-Need match
and subscription in the same accepted HIR-generation transaction as expressions,
patterns, bindings, exact types, effects, calls, source roles, and ownership.

`CheckedViewNeedMatch` retains exact owner `ItemId`, match/scrutinee `ExprId`,
arm `ExprId`, `PatternId`, binding `LocalId`, exact `Need<T>` and `T`,
source order, coverage result, effects, source roles, and ownership disposition.
HIR identities are non-Serde and absent from wire/save/digests. A missing row
after semantic success is an internal completeness error.

## C3. Retained subscription identity

Session key: `(owner ItemId, scrutinee ExprId)` under exact
`CheckedViewGeneration`. Product key: dense nonzero
`ViewNeedSubscriptionId`, allocated by canonical View-node order. Stable joins:
`ViewNeedSubscriptionSemanticId` and
`ViewNeedSubscriptionContractDigest`, derived from accepted program/semantic
identity, exact Need/payload type digests, exact producer AWBC binding, and fixed
start/cancellation policies.

Source text, spelling, source role, span, HIR IDs, endpoint copies, display
strings, and RuntimeValue handles are excluded. The ordinary AWBC producer
function terminates in verified `AwbcRuntimeType::NeedHandle(T)`; the subscription
binder obtains existing `NeedId` directly. The handle is never converted to
RuntimeValue or stored in a View local.

## C4. Publication authority

Runtime-driver owns one journal keyed by `(GenerationId, NeedId)`.
`RuntimeNeedState` carries unary Need and `TaskPublicationCursor` ordered
lexicographically by `(LogicalEpoch, TaskSequence)`.

- no prior row + valid current row: accept;
- lower cursor: stale no-op;
- equal cursor + equal canonical state digest: duplicate no-op;
- equal cursor + different digest: hard conflict; reject whole batch;
- greater cursor while nonterminal: accept;
- greater cursor after first Ready/Cancelled: stale-after-terminal no-op;
- retired generation: stale-generation no-op before payload projection;
- unknown future generation: hard error; reject whole batch.

Batches sort/group/apply in scratch. Multiple Pending rows coalesce to the
highest cursor. Same-step Pending-to-Ready selects Ready. Each changed live
observer gets exactly one invalidation. Journal and invalidations commit together.

The first frame uses the latest committed row or synthetic NotStarted. Remount
gets a new `ViewMountId`, current journal state, and no retired mount state.
Multiple mounts/subscriptions share a producer journal but have independent
observer cursors, active arm, retained arm-local state, and invalidation revision.

## C5. Generic Match and ordinary value execution

There is no Need-specific View instruction. The product uses generic
`ViewInstruction::Match`; its selector is an ordinary AWBC synthetic function.
A typed `ViewValueInputSource::NeedState` identifies the subscription.
An inherent `RuntimeNeedState::project_match_value` constructs an ordinary
closed-variant RuntimeValue for NotStarted, Pending(Progress), Ready(T), or
Cancelled, and existing AWBC pattern/match tables execute once in source order.

The selector returns source arm ordinal plus ordinary typed binding registers.
`ViewMatchArmBinding` maps registers to `ViewLocalRef`. View runtime MUST NOT
re-run patterns/guards, interpret patterns, stringify payloads, create a View VM,
or fall back through FxRuntimeValue. Runtime no-match is the ordinary match error
and aborts the frame candidate; no fallback arm is selected.

## C6. Result/Option orthogonality

Need owns only NotStarted, Pending(Progress), Ready(T), and Cancelled.
`Need<Result<T,E>>` exposes domain failure only as `Ready(Result::Err(E))`.
`Need<Option<T>>` exposes absence only as `Ready(Option::None)`.
Synchronous admission denial stays outside Need, e.g.
`Result<Need<T>, AdmissionError>`, and must be handled first. Infrastructure
failure is a runtime fault. Cancelled has no ordinary payload binding in v1.

## C7. Start and cancellation

Observing NotStarted produces one transactional Sans-I/O
`ViewNeedStartIntent`. Existing AWBC task-plan/product-step validation and the
runtime task registry commit `HostTaskDispatch` only with a valid frame.
Dedup is the verified producer `TaskKey` under `TaskPolicy::JoinSameKey`, scoped
by generation, NeedId, and producer contract.

V1 cancellation is `ProducerOwned`. Arm changes, observer removal, and last
View observer removal do not cancel. Existing explicit task/scope APIs and
generation replacement own cancellation. Replacement cancels a retired producer
only after successful swap and only when no exact producer survives. Lower
crates perform no I/O.

## C8. Ownership/persistence admission

Progress, Ready payload, selector bindings, and arm-local state can outlive a
frame and enter save/replay. Sema rejects affine, unique, borrowed, must-drop,
frame-local, non-cloneable, or non-snapshot-admissible payloads/captures before
product construction. Accepted values use the ordinary runtime ownership/type
graph. No presentation copy/debug-string fallback exists.

## C9. Static certification

Any live Need subscription is dynamic. Delete stale `DirectAwait`; add
`LiveNeedSubscription { subscription }` to the parent's dynamic reason.
Source-order DFS selects the exact first Need scrutinee. Authored `#[static]`
fails through `sema.view.static.required_dynamic`, primary at the attribute and
related at the scrutinee. Without the authored requirement, publish ordinary
dynamic evidence. No static certificate exists for a live subscription subject.

## C10. Strict v1 wire cut and deletion

The unreleased ViewProgram transcript remains version 1 and is replaced in
place. Add generic Match records, typed subscription table, Need-state input,
and binding-output contract. Delete every Await type/variant/tag/span/field,
integer discriminant, evaluator, diagnostic, codec/merge/digest/fingerprint
branch, fixture, test, re-export, generated spelling, and stale parent row.

Unknown `"await"` bytes fail the closed enum. No alias, deprecated re-export,
compatibility reader, dual transcript, V2 wrapper, source gate, or
removed-syntax diagnostic remains.

## C11. Save/replay

Every new Arcweft-owned marker is 1. Snapshot canonical tables are: producer,
publication, observer, retained arm, and invalidation. Ready payload uses
ordinary RuntimeValue snapshot encoding under exact type/ownership/depth.
Restore validates marker, counts/order, identities/digests, generation/NeedId,
cursor/terminal invariants, payload, mount allocator, observer references,
arm contracts, queue revisions, and work limits in scratch, then swaps once.

Replay invokes the same generation-bound publication API. It has no second
selection algorithm. Host dispatch is suppressed while replay input remains;
post-replay NotStarted may emit a start intent only in a committed frame.

## C12. Hot replacement

Candidate decode/catalog/reconcile is scratch-only. Join subscriptions by
semantic ID plus contract digest, never dense ID. Exact producer/type/selector/
arm equality preserves publication, cursor, active/retained arms, and queue.
Changed arm contract preserves publication but drops only incompatible arm state
and queues one invalidation. Changed producer/type binds a fresh generation and
never reuses old payload. Catalog, mounts, journal projection, observers, and
queues swap atomically.

## C13. Failure and transaction boundaries

Precedence: generation; ordinary semantic type/pattern/coverage/effect;
ownership/persistence; catalog completeness; authored static requirement;
compiler scratch; strict decode; cross-section/catalog; runtime generation;
cursor; payload/type/ownership/depth/fanout; generic match/frame; restore;
replacement.

Atomic scopes are semantic report, compiler product, decoded catalog,
publication batch, frame/mount/start intents, restore, and replacement.
A previously committed publication is not rolled back by a later frame failure.

## C14. Work limits

Every loop is charged to `WORK_ACCOUNTING.md`, checked before allocation,
traversal, invocation, append, or mutation. Exact-limit succeeds; one-over
returns the owning typed error and leaves authoritative state byte-identical.

## C15. Consumer parity

Native, Web, headless, Agent/MCP, generated artifacts, save/replay, and hot
replacement consume the same validated runtime catalog and shared
`BundleViewFrame`. No backend resolves source or owns publication/match logic.
Headless is the differential oracle.

## C16. Compile-clean order

Land genuinely absent parent checked-catalog/generic-Match/AWBC binding substrate
while construction fails closed; add complete checked Need facts; add core/runtime
projection/journal/start/save/replacement owners; then atomically switch every
consumer and delete old Await in the same v1 cut. Run all focused, tamper,
differential, exact/one-over, workspace, Clippy, docs, generated, backend, and
Tier-2 gates before implementation readiness.

## C17. API absence

After cutover, zero definitions/exports/variants/tags/diagnostics/evaluators/tests/
fixtures/generated spellings remain for `ViewProgramInstruction::Await`,
`ViewInstruction::Await`, `ViewAwait`, `ViewAwaitBranch`,
`ViewAwaitBranchSpan`, `InvalidAwaitState`, direct four-way Await, or `AwaitView`.

## C18. Closure

All result-changing alternatives are selected. `OPEN_QUESTIONS.md` is exactly
`none`. This package is design authority after review, not a claim that
production gates already passed.
