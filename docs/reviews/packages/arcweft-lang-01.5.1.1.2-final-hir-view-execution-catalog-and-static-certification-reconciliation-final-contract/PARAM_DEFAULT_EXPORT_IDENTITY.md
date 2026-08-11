# Parameter, default, nested-call, part, and export identity

## Identity strata

`ItemId`, `ExprId`, `LocalId`, `SyntaxNodeId`, HIR snapshots/revisions, and typed
source roles remain live-session semantic/source facts. They never serialize and
never enter a persisted hash. Product and runtime reuse:

- stable public owner `ViewId`;
- stable program owner `ViewProgramId`;
- exact accepted content owner `AcceptedViewProgramRevision`;
- program-local nonzero typed coordinates for nodes, instructions, sites,
  parameters, and locals; and
- canonical `ProductSourceId`/`SourceRangeRef` only for diagnostics.

A local coordinate must always be interpreted under the exact accepted program
revision. Raw integer equality across revisions is not identity.

## Parameters

`ViewParameterRef` is a one-based nonzero program-local coordinate allocated from
the canonical typed parameter inventory. Sema retains a non-Serde
`CheckedViewParameterKey { owner: ItemId, local: LocalId }` for lookup, but that key
is never projected into wire identity.

Product rows include:

```rust
pub struct ViewParameterResource {
    pub parameter: ViewParameterRef,
    pub ordinal: u16,
    pub name: String,
    pub role: ViewParameterRole,
    pub value_type: RuntimeCheckedType,
    pub value_slot: ViewValueSlot,
    pub default: Option<ViewValueProgramId>,
    pub source: Option<SourceRangeRef>,
}
```

Ordinal is canonical order/consistency; name is diagnostics/tooling. Neither is
binding authority. Duplicate parameter coordinate, ordinal, or slot is invalid.
The containing definition supplies `ViewProgramId` and
`AcceptedViewProgramRevision`; a parameter coordinate from any other program or
revision is rejected.

`name` and `source` are excluded from parameter contract identity. The parameter
contract digest contains coordinate, role, exact type, slot, default semantic ID,
and ordering constraints.

## Defaults

Defaults are ordinary generated AWBC programs. Runtime evaluates absent parameters
in declaration order against the callee environment containing preceding
parameters/default results and accepted outer dependencies. It validates exact
result type before binding. A default may return String, RichText, nominal/resource,
sequence, record, or any accepted ordinary value; no Fx scalar restriction applies.
Default failure aborts the nested mount transaction.

## Nested calls

A serialized `ViewCallTargetResource` carries the stable callee `ViewId`,
`ViewProgramId`, and exact `ViewParameterTableContractDigest`; it does not carry or
hash the callee `AcceptedViewProgramRevision`. Each `ViewCallArgument` carries the
exact `ViewParameterRef`, its `ViewParameterContractDigest`, and a
caller-environment value program. The candidate catalog resolves the stable target
to one accepted callee and requires its recomputed table/parameter digests to match.
This permits direct or mutual recursion without a revision-digest cycle while the
whole artifact identity still pins the exact accepted candidate set. Product
validation requires exactly one argument or default for each required parameter,
no duplicate/foreign coordinate, and exact type. Runtime:

1. resolves the stable target in the already validated candidate catalog and checks
   the complete parameter-table contract against that accepted callee revision;
2. evaluates explicit programs in source order in the caller environment;
3. stores candidate values keyed by callee parameter coordinate and verifies each
   parameter contract digest;
4. evaluates missing defaults in callee declaration order;
5. validates complete parameter set and state schema;
6. allocates/publishes nested mount only after success.

A current evaluator's inability to project a value is not permission to drop it.

## Locals and repeat/handler inputs

`ViewLocalRef` is another program-local typed coordinate allocated from canonical
final-HIR local order. Exact typed program input rows replace serialized local
names. Repeat item/index and handler input have dedicated typed input coordinates.
The runtime creates an invocation frame by program/revision/coordinate and slot,
never by source string or HIR arena slot.

## Parts and exports

Sema owns this non-Serde key:

```rust
pub(crate) struct CheckedViewExportKey {
    owner: ItemId,
    target: ExprId,
    source: CheckedViewSourceRole,
}
```

It proves final-HIR owner/target/source correspondence during catalog and compiler
projection. It is not persisted. The product record is:

```rust
pub struct ViewExportResource {
    pub owner: ViewId,
    pub program: ViewProgramId,
    pub public_name: ViewPartName,
    pub contract: ViewExportContractDigest,
    pub target: ViewExportTarget,
    pub source: Option<SourceRangeRef>,
}

pub struct AcceptedViewExportRef<'a> {
    pub accepted: &'a AcceptedViewProgramIdentity,
    pub resource: &'a ViewExportResource,
}

pub struct ViewExportTarget {
    pub node: ViewProgramNodeId,
    pub instruction: ViewInstructionId,
    pub site: ViewEvaluationSiteId,
    pub part: ViewPartId,
}
```

The decoder first validates the enclosing `AcceptedViewProgramIdentity`, then
constructs `AcceptedViewExportRef`. Runtime validates that all coordinates belong
to that accepted program revision, the instruction can own the part, the site
belongs to the same node,
static reachability is truthful, and the public export maps uniquely. Instruction
ordinal, source text, source range, and HIR IDs are never used to infer a target.
Dynamic and certified paths publish identical exported-part evidence.

The export contract digest contains owner/program identity, public name, typed
target coordinates, part identity, state/input schema, and visibility contract. It
excludes diagnostic source data and `AcceptedViewProgramRevision`; the enclosing
program revision incorporates the export contract exactly once, avoiding a
self-referential hash.

## Hot replacement

Replacement first validates the complete candidate and its
`AcceptedViewProgramRevision`. Reconciliation may compare public owner/name plus
parameter/export/state contract digests. Dense node/instruction/site/parameter/local
coordinates are usable only after exact revision equality or an explicit typed
contract-compatible remap owned by the existing replacement transaction; raw
coordinate equality never authorizes retention.

Any changed parameter type/default contract, export target contract, state schema,
resource closure, or certificate dependency is classified by the existing
replacement owner. Stale generation fails before active mount mutation.
