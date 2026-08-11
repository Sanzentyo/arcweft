# Evaluation, effects, affine ownership, snapshot, and restore

## 1. Evaluation schedule

For group `g`, the compiler emits two orders:

- `authored_evaluation`: authored expressions in source order; and
- `canonical_slots`: every declared parameter in coordinate order.

Execution follows this schedule:

1. validate all callee/definition/declaration/signature/generation/group facts and
   all statically knowable slot shape;
2. evaluate authored expressions once in increasing `source_ordinal`;
3. evaluate selected missing defaults once in increasing parameter coordinate;
4. build positional-rest and named-rest aggregates from the already evaluated
   authored values;
5. materialize optional omission cells;
6. assemble the group product in coordinate order;
7. validate the candidate prefix/full product and affine transfer batch; and
8. commit a partial or open transaction.

Authored named arguments are not reordered for evaluation. Canonical product order
is not evaluation order. Rest aggregation moves/references already evaluated
values and never evaluates an expression a second time.

A default expression belongs to the application of its own group. Its effects do
not occur when the declaration is loaded, when an earlier group is applied, or
when a later/final group is applied.

## 2. Effect accounting

Let `E(g)` be the ordered effects from authored and selected default expressions
in group `g`. Let `OPEN` be the external capability open effect.

```text
non-final apply(g) = E(g)
final apply(g)     = E(g) followed by OPEN
```

There is no open effect for:

- initial callable construction;
- non-final application;
- capture storage;
- unrestricted duplication;
- affine movement;
- suspension;
- snapshot encoding;
- restore validation; or
- partial drop.

The open effect is recorded only when the full product and instance allocation
have passed validation and the atomic commit appends the typed request.

If an authored/default expression has already produced ordinary language effects
and a later expression fails, those language effects follow existing expression
semantics. The Stream instance table and Stream request batch remain unchanged.
Every failure detectable from metadata or register types is rejected before the
first expression evaluation.

## 3. Suspension during and between groups

The execution owner retains a group-application frame while evaluation is in
progress:

```rust
pub struct RuntimeExternalStreamGroupApplicationFrame {
    pub callee: RuntimeFunctionValue,
    pub plan: RuntimeExternalStreamGroupApplicationPlanId,
    pub phase: RuntimeExternalStreamApplicationPhase,
    pub next_authored_ordinal: u16,
    pub evaluated_authored: Vec<RuntimeEvaluatedAuthoredArgument>,
    pub next_default_coordinate: Option<RuntimeCallableParameterCoordinate>,
    pub evaluated_defaults: Vec<RuntimeEvaluatedDefaultArgument>,
}

pub enum RuntimeExternalStreamApplicationPhase {
    Authored,
    Defaults,
    Assemble,
    Commit,
}
```

For AWBC, the same information is represented by the existing exact instruction
cursor and registers; no duplicate sidecar frame is added. Structured RuntimePlan
execution owns the explicit frame above.

A suspension stores the next expression/instruction cursor after every completed
evaluation. Resume continues at that cursor. It cannot reconstruct earlier values
from source or restart the group.

After a successful non-final commit, the execution frame is gone and the new
partial is a normal `RuntimeValue::Function`. Suspension between groups therefore
needs no special pending-open state.

## 4. Affine ownership

The existing ABI-2 affine-value owner classifies every captured value. The partial
ownership is the maximum of all cells:

```text
all cells unrestricted -> partial unrestricted
any cell affine         -> partial affine
```

`OmittedOptional` contributes no owner. A rest aggregate is affine when any item
or entry is affine. The aggregate is the sole owner of its affine members.

Before committing a group, the runtime prepares one ownership-transfer batch that
contains every affine source and destination. It rejects duplicate tokens,
already-moved values, a token appearing in both the old prefix and new group, and
snapshot-only/nontransferable values. The batch commits together with the new
partial or open request.

An affine partial cannot be cloned. An unrestricted partial can be duplicated
through the existing checked duplication API. The public runtime value model does
not expose an unconditional clone operation that silently duplicates an affine
partial or Stream handle.

## 5. Save schema 2 shape

The outer version is:

```rust
pub const BUNDLE_SESSION_SAVE_SCHEMA_VERSION: u32 = 2;
```

The existing save traversal of `FiberState`/runtime values gains this variant:

```rust
pub enum RuntimeFunctionValueSnapshotV2 {
    Closure(RuntimeClosureValueSnapshotV2),
    ExternalStreamPartial(RuntimeExternalStreamPartialSnapshotV2),
}

pub struct RuntimeExternalStreamPartialSnapshotV2 {
    pub definition: StreamDefinitionId,
    pub declaration: RuntimeCallableDeclarationDigest,
    pub generation: GenerationId,
    pub signature: RuntimeExternalStreamSignatureFingerprint,
    pub next_group: RuntimeCallableGroupIndex,
    pub ownership: RuntimeValueOwnership,
    pub arguments: RuntimeExternalStreamArgumentProductSnapshotV2,
}

pub struct RuntimeExternalStreamArgumentProductSnapshotV2 {
    pub completed_groups: RuntimeCallableGroupCount,
    pub coordinates: Vec<RuntimeCallableParameterCoordinate>,
    pub values: Vec<RuntimeExternalStreamArgumentValueSnapshotV2>,
}
```

Checked value snapshots retain exact runtime type layout, value snapshot, value
digest, and affine token/owner evidence selected by the parent affine contract.
No expression, source range, parameter name, or provider resume token is stored.

Every generation referenced by a partial is added to the parent schema-2 required
generation set. Save succeeds only when the corresponding verified AWBC/bundle
generation artifact remains available under its exact content identity. A partial
never silently rebinds to the currently active generation.

## 6. Save blockers

A committed partial is saveable when all captures are snapshot-safe and its exact
generation artifact is retained. The correction adds these typed blockers to the
parent blocker enum:

```rust
ExternalStreamGroupApplicationActive { count: usize },
UnsnapshotableExternalStreamPartialCaptures { count: usize },
MissingExternalStreamPartialGeneration { generations: Vec<GenerationId> },
```

The first blocker is present only when the save policy reaches a non-safe-point
structured application frame. AWBC instruction boundaries use the existing fiber
safe-point rule.

A partial is not an externally live Stream, so it does not trigger the parent
`ExternalLiveStream` blocker. After final application, the resulting Opening/Open
instance follows the parent external-live save rule.

## 7. Restore validation order

Restore decodes into temporary state and performs these checks before installing
any session state or invoking a host:

1. outer schema ID/version and canonical decoding;
2. artifact identity, AWBC ABI 2, codec 8, and host ABI digest;
3. required generation artifacts and their content roots;
4. function-value variant tag and field bounds;
5. Stream definition existence and external origin;
6. callable declaration digest equality;
7. signature fingerprint equality;
8. generation liveness/retained-generation membership;
9. `next_group == completed_groups < group_count`;
10. coordinate/value length, completeness, uniqueness, and order;
11. disposition/default/type/rest validation for every cell;
12. runtime value digest validation;
13. affine token uniqueness and owner graph validation; and
14. whole-fiber/register/frame validation.

Only after all saved runtime, presentation, root, view, and Stream state passes its
parent validators is the session swapped into the driver. Restore failure is
atomic.

Restore of an external partial emits no argument effects and no open request. It
preserves the next group exactly. A stale/foreign snapshot cannot be repaired by
looking up a similarly named active callable.

## 8. Snapshot/replay invariants

The following equalities are normative:

```text
restore(snapshot(partial)).captured == partial.captured
restore(snapshot(partial)).next_group == partial.next_group
open_requests_during_snapshot == 0
open_requests_during_restore == 0
argument_evaluations_during_restore == 0
```

A staged program that snapshots after group 1 and resumes with group 2 must produce
the same final canonical request as the same program without snapshot, excluding
only the instance allocator position when unrelated instances were opened in
between.
