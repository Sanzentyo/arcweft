# Bounded work and resource accounting

## 1. Hard limits

These are final contract constants owned by the existing runtime-plan/core
limit modules.  All producers, structured execution, AWBC verification/VM,
bundle admission, save, replay, and restore use the same values.

```rust
pub const MAX_LINE_PLAN_ITEMS: usize = 256;
pub const MAX_LINE_ACTIVATION_OPS: usize = 512;
pub const MAX_LINE_LOCALS: usize = 256;
pub const MAX_LINE_GROUP_CAPTURES: usize = 64;
pub const MAX_LINE_CALLBACK_CAPTURES: usize = 32;
pub const MAX_LINE_TOTAL_CAPTURE_VALUES: usize = 256;
pub const MAX_LINE_HANDLE_SITES: usize = 128;
pub const MAX_LINE_LIVE_HANDLES: usize = 256;
pub const MAX_LINE_SCHEDULED_CALLBACKS: usize = 128;
pub const MAX_LINE_TASK_NODES: usize = 512;
pub const MAX_LINE_CANCEL_RULES: usize = 64;
pub const MAX_LINE_CLEANUP_ACTIONS: usize = 128;
pub const MAX_LINE_RESULT_DEPTH: usize = 64;
pub const MAX_LINE_RESULT_NODES: usize = 4_096;
pub const MAX_LINE_RESULT_BYTES: usize = 256 * 1024;
pub const MAX_LINE_CAPTURE_DEPTH: usize = 64;
pub const MAX_LINE_CAPTURE_NODES: usize = 8_192;
pub const MAX_LINE_CAPTURE_BYTES: usize = 1024 * 1024;
pub const MAX_LINE_QUEUED_HOST_COMMANDS: usize = 256;
pub const MAX_LINE_HOST_COMMANDS_PER_STEP: usize = 128;
pub const MAX_LINE_REDUCER_TRANSITIONS_PER_STEP: usize = 4_096;
pub const MAX_ACTIVE_DIALOGUES_PER_SAVE: usize = 64;
pub const MAX_LINE_HANDLES_PER_SAVE: usize = 4_096;
pub const MAX_LINE_RESTORE_VALIDATION_UNITS: usize = 1_000_000;
```

A per-site issuance ordinal is `u32`; checked overflow is always a runtime
error even when the live-handle count is below its limit.

## 2. Construction accounting

| Item | Admission charge |
|---|---:|
| source line-plan item | 1 plan item |
| emitted activation `FlowOp` | 1 activation op |
| declared local | 1 local |
| group capture | 1 group capture and full value-type traversal |
| handle site | 1 site |
| scheduled callback | 1 scheduled callback + child nodes + capture declarations |
| line task node | 1 node |
| cancellation rule | 1 rule + its action ops |
| cleanup action | 1 action |
| result pattern/value type node | 1 result node |

One HIR item may expand to several operations; both item and operation limits
apply.  Reaching a limit is accepted; exceeding it produces
`RuntimePlanLowerError::LineLimitExceeded`.

## 3. Runtime value accounting

Canonical value traversal uses a non-recursive bounded stack and charges:

```text
1 unit per RuntimeValue node
1 unit per nominal/record/variant field
1 unit per sequence element
1 unit per opaque wrapper plus its payload
ceil(byte_payload_len / 64) units for strings/bytes/dense buffers
```

The same traversal computes depth, node count, canonical byte estimate,
ownership, and affine paths.  Result and capture limits are applied separately.
No code may perform an unbounded second traversal after admission; reusable
validated summaries are stored with the transaction.

## 4. Schedule and child accounting

- each armed cue counts against both live handles and scheduled callbacks;
- a fired/running joined cue remains counted until terminal cleanup;
- completed cue leases count as live handles until dropped/released;
- cancelled future cues are removed only after their cancellation transition;
- scheduled capture bytes count at issuance and again in save global totals;
- zero-delay cues are still real scheduled callbacks and consume the same
  budget.

Deadline addition is checked.  Negative duration, invalid conversion, or
`elapsed + delay` overflow fails without issuing a handle.

## 5. Host queue accounting

The per-activation typed command queue is bounded by
`MAX_LINE_QUEUED_HOST_COMMANDS`.  Before enqueue, the runtime charges one slot;
when full, the operation returns a structured backpressure failure and does not
issue the corresponding resource handle unless the operation's transaction
can be rolled back completely.

Within one engine step, at most `MAX_LINE_HOST_COMMANDS_PER_STEP` requests are
materialized.  Remaining deterministic work yields at the same cursor in both
structured and AWBC execution.

## 6. Reducer budget

Every reducer transition charges one unit for state transition plus:

- one unit per node entered;
- one unit per command emitted;
- one unit per child terminal notification consumed;
- one unit per cleanup action selected.

At `MAX_LINE_REDUCER_TRANSITIONS_PER_STEP`, execution emits a deterministic
budget-yield safe point with the complete reducer cursor.  This is not failure;
it resumes next engine step.  Structured and AWBC executions use the same
counter and cursor.

## 7. Result/pattern publication budget

Publication must fit the already validated result limits.  It additionally
charges one unit per pattern node and affine transfer/drop.  If a tampered
snapshot or runtime invariant would exceed the limit, publication fails before
mutation with `ResultLimitExceeded`; it does not partially bind.

## 8. Save and restore accounting

Global save checks:

```text
active_dialogues <= 64
sum(handles) <= 4096
all per-dialogue limits also hold
```

Restore validation units are:

| Candidate component | Units |
|---|---:|
| scalar field/id | 1 |
| type/value/pattern node | 1 |
| handle lease | 8 + payload traversal |
| schedule | 8 + capture traversal |
| reducer node/state | 2 |
| pending command | 8 + argument traversal |
| activation/AWBC frame register | 2 + value traversal when initialized |

Exceeding one million units rejects before host preflight or engine mutation.

## 9. Diagnostic bounds

At most 64 structured diagnostics are retained per admission/restore
transaction.  The first error category still determines the primary failure;
additional errors in the same category are ordered by canonical plan/value
path.  Truncation is explicitly marked and does not trigger a fallback.
