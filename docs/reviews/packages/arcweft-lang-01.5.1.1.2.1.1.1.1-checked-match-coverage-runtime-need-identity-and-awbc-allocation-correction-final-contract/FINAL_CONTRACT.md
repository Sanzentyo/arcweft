# Complete final contract

## 1. Scope and precedence

This contract redelivers the generic Match and typed Need producer design over
repository main `c49099fb154d9e3dbb587e1bcd7ee243214da0c4`. Current production and maintained contracts take
precedence over stale observations in predecessor packages. The correction is
strict version 1: old wire meanings, String identities, payloadless handles,
and caller-provided coverage have no reader or alias.

The selector Variant/Tuple ABI, explicit guard Branch lowering, View/core
independence, typed Need carrier, resource registry authority, unary Need
lifecycle, journal/observer/start/cancellation semantics, persistence, and the
strict atomic switch are retained.

## 2. Global AWBC authority

`#[repr(u8)] AwbcOpcode` is the only opcode numeric authority. Each allocated
byte appears exactly once, as an enum discriminant. `encoded()` returns
`self as u8`; `from_encoded()` indexes an allocation-free 256-entry table built
at compile time from `AwbcOpcode::ALL`. Instruction and terminator methods map
semantic variants to enum variants and contain no byte literals.

Manual Serde is numeric u8. Private `Wire` calls the same inherent encode/decode
methods. Unknown bytes and wrong instruction/terminator classes reject. The
exact table is in `AWBC_ALLOCATION_AND_WIRE.md` and
`machine/awbc-allocation.json`.

`AwbcFunctionKind` uses exact tags 0,1,2,3,6,7,8,9,10; tags 4/5 are permanent
tombstones. `AwbcFunctionFlag` owns bit positions 0..5. The private
`AwbcFunctionFlags(u32)` accepts only `0x3f`, derives masks by shifting the enum
position, and enforces the producer-kind matrix before program publication.

All ordinary u32 values use shortest ULEB128, at most five bytes. Collection
lengths are checked u32 varints; `usize` has no Wire implementation. Fixed-width
exceptions are closed in the wire contract. Tensor shape write/read both use
`u32::Wire`. Canonical encoding uses one final Vec, writes the envelope and
payload in place, patches the u64 payload length, and rolls back by truncation
on failure.

## 3. Runtime Need/task identity

The String newtypes are replaced in place by nonzero fixed-byte `NeedId`,
`TaskKey`, and `TaskId`. `NeedId` is the logical result identity; active runtime
correlation is `(GenerationId, NeedId)`. `TaskKey` additionally commits to the
start policy and deterministic launch ordinal, and `TaskId` commits to the
TaskKey plus that ordinal.

`AwbcTaskPlan.need_id` is replaced by mandatory `AwbcTaskProducer { family,
contract, site, plan_digest }`. The plan digest is verifier-recomputed from the
canonical typed plan fields. No optional legacy row, string fallback, or
parallel identifier survives.

Host task, View producer, line task, AwaitMany base/child, and timeout output use
distinct BLAKE3 domains. Direct Await reads the `NeedId` embedded in the typed
`RuntimeNeedHandle` and derives nothing. AwaitMany indexes are source-order u32
encoded as u32-le in the digest transcript. Display is lowercase hex only and
is never parsed. Snapshots store typed identities and restore recomputes them
before mutating runtime state.

## 4. Checked Match authority

Every live HIR Match receives one `CheckedExpressionResolution::Match` fact.
`CheckedMatch::try_from_hir` accepts HIR/checked maps and the exact semantic
catalog context, but accepts no coverage object. It invokes the private
`MatchCoverageAnalyzer`, then the private `CheckedOwnershipContext`, and only
constructs the final fact after both succeed.

Coverage is a bounded typed usefulness matrix. Absent and constant-true guards
contribute. Constant-false and dynamic guards do not contribute to
exhaustiveness or make later arms unreachable; a guarded arm can itself be
unreachable when earlier contributing rows already cover its entire pattern.

Non-exhaustiveness, poisoned/unsupported domains, missing semantic evidence,
and limit overflow are hard errors and publish no fact. Unreachable arms are
sorted/unique retained evidence plus diagnostics, but diagnostics are emitted
only after the whole Match succeeds. No caller can fabricate the bit or rows.

## 5. Ownership and persistence admission

`FinalSemanticCatalogs::production` is extended with the exact immutable
`ResourceTypeRegistry`, verifies its integrity, and retains its existing digest.
`CheckedOwnershipContext` receives `ProjectSymbolTable`,
`RegisteredSemanticWorld`, `ResourceTypeRegistry`, and fixed limits. It resolves
project nominal fields/cases, accepted opaque value class/persistence, resource
schemas, and all composite children directly from those owners.

Successful classification is `Copy` or `SnapshotClone`; rejection is a closed
error, not a third successful disposition. Canonical depth-first order selects
the first error. Direct by-value nominal cycles reject. `Shared`, typed Need
handles, and registered opaque identity boundaries stop structural cycle
expansion under their own rules.

`Need<T>` is `Copy` as an immutable handle type. This never certifies a producer.
Every producer separately carries a checked argument/capture certificate; any
borrowed, affine, frame-local, non-snapshot opaque, unresolved nominal, resource
failure, or missing certificate rejects before AWBC publication.

## 6. Product identity

`ExprId`, `ScopeId`, `PatternId`, `LocalId`, snapshot IDs, and arena positions
are session lookup facts only. The compiler performs one projection to
`ViewProgramSemanticDigest`, `ViewProgramRevision`, `ViewMatchSiteId`, source
arm ordinal, and source binding-output ordinal. Only the stable coordinates and
semantic digests cross the product boundary.

The checked Match semantic digest is BLAKE3 with domain
`arcweft.checked-match.semantic.v1\0`. It commits to program/revision/site,
scrutinee/result/effect digests, resource digest, canonical coverage, every arm
pattern/guard/value semantic digest, every binding type/ownership row, and the
selector result schema. It never commits to source spelling, debug text, HIR
IDs, platform-sized integers, or an unspecified helper.

Deterministic recompilation with equal semantic owners yields the same digest.
Arm order, stable constructor identity, guard/body semantics, ownership,
coverage, or resource registry changes yield a different digest. Bundle roots,
save/replay facts, and hot replacement compare these exact digests.

## 7. Persistence, replay, and replacement

Journal and snapshot schemas remain version 1 but are replaced atomically.
Task events carry `TaskId`, `TaskKey`, `NeedId`, producer contract, and active
generation. AwaitMany rows additionally carry source index and base identity.
Restore validates nonzero fixed IDs, recomputes transcript results, verifies
contract/type/resource/bundle digests, and only then publishes restored state.

Duplicate JoinSameKey observers share one task and terminal publication.
AlwaysStart launches have distinct TaskKey/TaskId but may share one logical
NeedId; equal terminal publication is idempotent and contradictory publication
is a deterministic error. Replay restores the journaled launch ordinal.

Hot replacement preserves live state only through an explicit revision mapping
whose old/new program identity, checked Match digest, producer contract,
payload type, resource digest, and task-plan digest all agree. Otherwise the
old generation is cancelled and the new producer is created transactionally.
Failed installation leaves the old generation authoritative.

## 8. Implementation boundary

Pending enum variants never land as dummy execution. The compile-clean sequence
first migrates existing numeric/wire owners, then publishes CopyValue, typed
Need producer, timeout, line-plan, and Stream families only with their complete
verifier/VM/AOT cuts. The last switch updates bundle/runtime journal/save/replay
consumers and deletes every legacy route in the same atomic publication.

## 9. Final disposition

All Required exact decisions 1–23 are closed. `OPEN_QUESTIONS.md` contains
exactly `none`. This archive is independently usable as the implementation
contract and validation source.
