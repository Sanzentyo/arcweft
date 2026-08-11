# Supersession delta against Lang-01.3.1.2.1, .2, and .2.1

This ledger is exhaustive for the three accepted predecessor archives. A predecessor clause not named as superseded/narrowed remains authoritative. This package does not rename or lightly fork their grouped callable, Stream lifecycle, policy, replay, host, wire, bundle, or save authorities.

## 1. Precedence

```text
Lang-01.3.1.2.3
  owns: generic runtime ownership/capture/copy-move-drop,
        executable Clone removal, generic snapshot exclusivity,
        payload/plan constant correction, AWBC CopyValue allocation.

Lang-01.3.1.2.2.1
  remains authoritative for: final grouped coordinates and names,
        canonical external argument product, exact 0x27/0x28/0x29 wire,
        operand tags, parent/child cut interleave except inserted G stages.

Lang-01.3.1.2.2
  remains authoritative for: first-class external partial semantics,
        evaluation/default/effect order, group application timing,
        generation/fingerprint behavior except owned failure/snapshot correction.

Lang-01.3.1.2.1
  remains authoritative for: Stream definition/instance key/table/lifecycle,
        policy/replay/accounting/host/AWBC/bundle/save versions and owners,
        except generic value/token/clone/snapshot corrections below.
```

## 2. Delta against Lang-01.3.1.2.1

| Predecessor clause/shape | Final disposition | Final rule |
|---|---|---|
| `RuntimeValueOwnership`/affine handle direction assumed as existing ABI-2 substrate | **Completed/superseded premise** | One structural `Unrestricted | Affine` classification and one opaque generic affine leaf token are defined in `arcweft-core::value::ownership`. |
| `StreamHandle { key, item_layout, error_layout, lease }` with no `Clone`/`Copy` | **Narrowly extended** | Add private `RuntimeAffineOwnerToken`; make invariant-bearing fields non-constructible/mutable outside owner and expose typed accessors. Key/layout/lease semantics are unchanged. |
| `StreamInstanceKey`/generation/ordinal and `StreamConsumerLease` | **Retained exactly** | They remain typed identity/domain evidence and do not themselves mint authority. |
| Sole `StreamInstanceTable`, entry lifecycle, allocation cursors, producer/consumer reciprocity | **Retained** | Table remains sole Stream lifecycle/lease authority. Generic token is carried in value graph and is not a second table/side map. |
| `#[derive(Clone)]` on `StreamInstanceTable`/runnable execution state shown in parent Rust schema | **Superseded for executable carriers** | Live table/execution/transactions are non-Clone. Strict table snapshot DTOs may be Clone. |
| “Internal transaction/snapshot cloning of an enclosing `RuntimeValue` does not create a second runnable owner because original/candidate are mutually exclusive” | **Replaced** | No runnable `RuntimeValue`/handle is cloned. Snapshot creates only dormant evidence; exclusive activation retires old owners before token creation/install. |
| Language `Move`, construction/destruction, calls, return, child transfer, `Drop` use affine-aware APIs | **Completed** | Exact slot, prepared transfer/drop, failure atomicity, cleanup, AWBC, and compiled parity APIs are fixed. |
| Existing `Move`/`Drop` opcodes retained | **Retained with concrete semantics** | `Move` consumes source; `Drop=0x1f` uses table-aware prepared language drop. |
| Current `RuntimePayload(pub RuntimeValue)` is assumed usable as a shared host/save payload substrate and must not contain a handle | **Corrected in place** | The existing owner/name becomes a closed recursively non-runnable enum and may `Clone`; it no longer wraps `RuntimeValue`, has no `From<RuntimeValue>`/opaque escape, and is host/general canonical data only. Save schema 2 uses separate snapshot DTOs for handle/partial evidence. |
| Save schema 2/bundle schema 6/ABI2/codec8 | **Retained exactly** | No new version is allocated by this contract. Restore validation/order is strengthened without dual readers. |
| Parent generation pins, table snapshot, restore candidate validation | **Retained and completed** | Exact whole-graph traversal, owner occurrence, dormant candidate, tamper order, and failed-restore cleanup are specified. |
| Parent Open atomically creates unique handle/table/request | **Retained and extended** | Generic owner token allocation joins key/lease/handle/table/request in the same non-fallible commit. |
| Current generic sequence storage/operations are assumed to participate in the future affine owner | **Completed on original owner** | The existing `RuntimeSeq`/column carriers and `RuntimeIterator` gain the inherent ownership/materialization/repeat/index/slice/push/take behavior directly; no `RuntimeSequenceValue` or Stream-only sequence wrapper exists. |
| Current plan/pattern literals embed executable `RuntimeValue`; current `FlowOp` also embeds live bindings/iterator continuations | **Corrected in place** | Both live literal variants and `FlowOp::{Bind, LoopNext, WhileNext, WhileLetNext, ForNext}` are removed. Original `RuntimeFlow` becomes an immutable block arena; original control-frame enum owns live continuation/iterator state; `RuntimePlan` is non-Clone/non-Serde and shared only as `Arc<RuntimePlan>`. Constants are closed data and pattern selection borrows. |
| Source deletion, policy, replay, accounting, event behavior, host requests | **Retained** | No redesign. |

## 3. Delta against Lang-01.3.1.2.2

| Predecessor clause/shape | Final disposition | Final rule |
|---|---|---|
| Sole `RuntimeFunctionValue::{Closure, ExternalStreamPartial}` | **Retained exactly as sole enum** | Existing owner is changed in place; no Stream-only value enum/trait dispatch. |
| `RuntimeClosureValue { params, body, captures }` | **Narrowly corrected** | Captures come only from typed `RuntimeCapturePlan`, in first-use ordinal order; no full `bindings_snapshot()` environment capture. Capture plan identity is retained in closure. |
| `RuntimeExternalStreamPartialFunction` identity/generation/signature/next-group/canonical product | **Retained** | Fields become invariant-protected; private ownership cache is recomputed/checked. |
| Public `ownership` field is authority | **Narrowed** | `ownership` is a private checked cache. Recursive value graph is authority. Snapshot/decode/restore reject mismatch. |
| Partial ownership is max/join of captured cells; omitted owns nothing; rest recursively joins | **Retained exactly** | Uses generic `RuntimeValueOwnership::join`. |
| Unrestricted partial duplicates through existing checked API; affine partial cannot clone | **Completed** | Exact `RuntimeValue::try_duplicate_unrestricted` and error/path semantics are supplied; no executable carrier implements `Clone`. |
| `try_apply_external_stream_group(...) -> Result<..., RuntimeFunctionApplicationError>` | **Superseded return boundary** | Returns `RuntimeOwnedFunctionApplicationFailure` containing the still-owned callee and evaluated group, or leaves source slots untouched. |
| Prepare/validate before mutable table/request access; non-final partial/final Open atomic | **Retained and completed** | Exact ownership preflight/staging/source-revision-and-owner-set recheck/non-fallible commit phases and token uniqueness are specified. |
| Authored/default evaluation and effect timing | **Retained exactly** | Earlier ordinary language effects are not rolled back; ownership/table/request publication remains atomic. |
| Structured application frame owns callee/evaluated temporaries across suspension | **Retained** | Its carriers are non-Clone owned slots; no special sidecar clone. |
| AWBC uses instruction cursor/registers rather than a duplicate structured application sidecar | **Retained** | One AWBC register/frame ownership state machine. |
| Function/partial snapshot schema 2 rows | **Retained and corrected** | Snapshot stores dormant owner evidence, exact capture plan/order, recursively checked ownership, exact generation pins; no live token/value clone. |
| Save blockers for active group/unsnapshotable captures/missing generation | **Retained** | Generic transaction/borrow/reciprocity blockers are added to the existing owner. |
| Host-payload eligibility for final arguments | **Retained and completed** | Each transmitted cell uses general payload conversion before Open reservation. A locally affine partial may exist but cannot leak its owner through host payload. |
| Fingerprint/hot reload behavior | **Retained** | Exact generation binding; no translation/rebinding of partials/handles. |

## 4. Delta against Lang-01.3.1.2.2.1

| Predecessor clause/shape | Final disposition | Final rule |
|---|---|---|
| Final grouped identity names/types and coordinates | **Retained exactly** | No alternative group/parameter/slot coordinate authority. |
| Sole `RuntimeExternalStreamArgumentProduct` across RuntimePlan/partial/Open/host/save/fingerprint | **Retained exactly** | It owns values by move; it is not flattened. |
| Sole function-value owner and external partial fields | **Retained with generic-owner narrowing** | Private checked ownership cache and exact no-Clone/owned-failure behavior. |
| Core `RuntimeStreamRequest::Open` with canonical product | **Retained** | Adapters serialize directly; payload eligibility is checked before Open commit. |
| `0x27 OpenStream`, `0x28 FinishStream`, `0x29 ApplyExternalStreamGroup` exact fields/bytes | **Retained exactly** | Ownership effects are consuming as specified in AWBC contract. |
| Operand tags 0 Explicit, 1 Defaulted, 2 OmittedOptional, 3 RestPositional, 4 RestNamed | **Retained exactly** | Every register-bearing operand is consumed; reuse requires prior `CopyValue`. |
| `0x2a..=0x7f` unknown | **Narrowly superseded** | `0x2a = CopyValue { dst, src }`; `0x2b..=0x7f` remain unknown. Exact wire is opcode + dst/src canonical varu32. |
| Removed opcode rejection (`0x1c/0x1d/0x1e/0x20`) | **Retained exactly** | No compatibility reader/alias. |
| P3 -> P4+C1 -> P5+C2 -> C3 -> P6+C4 -> P7+C5 -> P8+C6 | **Extended, relative order retained** | Insert G1/G2/G3 after P3 and before P4+C1. Parent/child relative interleave remains unchanged. `CopyValue` publishes in protected P6+C4. |
| P4+C1 publishes identities/table/grouped boundary/product/sole function enum | **Retained, delayed by owner gate** | It may occur only after G3 removes unconditional executable Clone. This is the first point a Stream handle is constructible. |
| P6+C4 protected ABI2/codec8 atomic cut | **Retained and extended** | Includes generic register ownership facts and `CopyValue=0x2a` together with verifier/VM/lowerer/codegen. |
| P7+C5 host, P8+C6 bundle/save/hot reload | **Retained** | Consume final generic owner; no Stream-only sidecar/DTO/snapshot clone. |
| Exact test matrix and worked bytes | **Retained** | All 105 rows are embedded in `PARENT_TEST_MATRIX_INDEX.json`; new ownership tests are additive. Existing worked bytes stay exact except new independent `CopyValue` golden bytes. |

## 5. Directly retained parent authorities

The following are expressly outside this correction and remain authoritative:

- callable selection/resolution/accounting and accepted sema coordinates;
- group/parameter names, declaration digest, signature/default fingerprints;
- Stream definition key/identity/origin/type/effect/policy/profile;
- instance key, table, lifecycle, queue, replay, accounting, producer behavior;
- host request/event semantics and adapter transport equality;
- ABI2/codec8 parent tables and version numbers;
- bundle schema 6 and save schema 2 outer identities;
- ordinary function role/direct suspension/Proof/Agent decisions;
- parent limits, error precedence, privacy/replay/drop-retention/restart/provider replacement policies.

## 6. New decisions owned only here

1. One structural generic ownership class and opaque leaf token.
2. Exact no-Clone/Copy/Serde/equality trait boundary for executable values.
3. Checked unrestricted duplication and owned error paths.
4. Exact Copy/Move/Drop/assignment/destructure/iterator/repeat/index/slice/equality/cleanup results.
5. Exact typed closure capture membership/order/mode/atomicity.
6. One AWBC ownership state machine and `CopyValue=0x2a`.
7. Dormant snapshot evidence, exclusive restore activation, tamper order, whole-graph pins.
8. Existing `RuntimePayload` wrapper and both live literal variants are replaced in place by the closed payload and checked constant table; direct RuntimePlan Clone/Serde is removed and only `Arc<RuntimePlan>` is shared.
9. Existing `RuntimeFlow`/`FlowOp`/`FlowCursor`/`FlowControlStackEntryKind` are corrected in place: block IDs replace cloned bodies, runtime-only op variants and pending cloned-op queues disappear, and the original control frame owns live iterator/continuation state.
10. Existing `RuntimeSeq`, `RuntimeIterator`, `RuntimePattern`, and `RuntimeEnv` receive the behavior on their inherent owners; no parallel model or copied registry.
11. G1/G2/G3 compile-clean owner migration before P4+C1.

No implementer may choose a different result while claiming conformance to these four contracts.
