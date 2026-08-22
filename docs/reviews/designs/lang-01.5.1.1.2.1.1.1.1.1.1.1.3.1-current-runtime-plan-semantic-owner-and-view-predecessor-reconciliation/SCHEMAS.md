# Rust-shaped schemas

This file separates the core-independent boundary that is closed now from the
predecessor-owned types that must be substituted after `.1.2` and `.1.4`
intake. Production must not implement the latter from the provisional names in
this document.

## 1. Core construction ownership

The current `RuntimePlanConstructionIssuer` remains the only candidate owner.
No `NonZeroU32` process-global token is introduced.

```rust
// arcweft-core::plan::construction
use std::sync::Arc;

pub struct RuntimeTaskPlanBuildCoordinate {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    ordinal: u32,
}

impl Clone for RuntimeTaskPlanBuildCoordinate { /* clone the Arc */ }

impl RuntimeTaskPlanBuildCoordinate {
    pub const fn ordinal(&self) -> u32;

    // Pointer identity plus ordinal. No token or address is returned.
    pub fn same_candidate_and_row(&self, other: &Self) -> bool;
}
```

The coordinate implements neither `Copy`, `Serialize`, `Deserialize`, nor a
raw constructor. `Debug` prints only the ordinal. It does not implement `Ord`:
pointer order is not canonical. Any upper binding collection is a validated,
ordinal-sorted slice and checks `same_candidate_and_row` after lookup.

The final row is not caller-constructible. Runtime-plan supplies a seed whose
function handle and expression children are already tied to the same issuer.

```rust
pub struct RuntimeTaskPlanSeed {
    producer_function: RuntimeFunctionSiteSeedId,
    family: NeedProducerFamily,
    class: TaskClass,
    request: RuntimeTaskRequestTemplateSeed,
    control_effect: RuntimeControlEffectContractSeed,
    binding: RuntimeTaskSemanticBindingSeed,
}

impl RuntimeTaskPlanSeed {
    pub fn try_new(
        producer_function: RuntimeFunctionSiteSeedId,
        family: NeedProducerFamily,
        class: TaskClass,
        request: RuntimeTaskRequestTemplateSeed,
        control_effect: RuntimeControlEffectContractSeed,
        binding: RuntimeTaskSemanticBindingSeed,
    ) -> Result<Self, RuntimeTaskPlanSeedError>;
}

impl RuntimePlanBuilder {
    pub fn push_runtime_task_plan_seed(
        &mut self,
        seed: RuntimeTaskPlanSeed,
    ) -> Result<RuntimeTaskPlanBuildCoordinate, RuntimePlanBuildError>;
}
```

`push_runtime_task_plan_seed` performs, in order:

1. poisoned-builder check;
2. same-issuer resolution of the function and every expression/local child;
3. family/binding validation;
4. request/control structural validation;
5. checked row-count/ordinal allocation; and
6. one atomic push, returning the coordinate for that accepted row.

Failure poisons the builder under the current aggregate-builder rule and
publishes neither a row nor a coordinate. There is no public
`RuntimeTaskPlan::new`.

`RuntimeTaskRequestTemplateSeed` is a same-cut replacement for the current
`RuntimeHostTaskRequestTemplateSeed`, not a parallel wrapper. Its final closed
variants are:

```rust
pub enum RuntimeTaskRequestTemplateSeed {
    Host(RuntimeHostTaskRequestTemplateSeed),
    Await(RuntimeAwaitTaskRequestTemplateSeed),
    AwaitManyBase(RuntimeAwaitManyBaseTemplateSeed),
    AwaitManyChild(RuntimeAwaitManyChildTemplateSeed),
    Timeout(RuntimeTimeoutTemplateSeed),
    Line(RuntimeLineTaskTemplateSeed),
    View(RuntimeViewTaskTemplateSeed), // admitted only after .1.4
    MakeNeedHandle(RuntimeMakeNeedHandleTemplateSeed),
}
```

The existing host seed is migrated in place: capability/operation strings are
resolved through the Cut 4 `HostOperationCatalog` projection before the final
row exists. The final host template retains one typed
`HostOperationIdentity`, one catalog-issued route-independent request-semantic
certificate, source-ordered positional/named/spread argument templates, and no
duplicate capability/operation string authority.

```rust
pub struct HostOperationPlanAdmission {
    operation: HostOperationIdentity,
    request_semantic: HostOperationRequestSemanticDigest,
}

impl HostOperationCatalog {
    pub fn plan_admission(
        &self,
        operation: &HostOperationIdentity,
    ) -> Result<HostOperationPlanAdmission, HostOperationCatalogError>;
}

pub struct RuntimeHostTaskRequestTemplate {
    operation: HostOperationIdentity,
    request_semantic: HostOperationRequestSemanticDigest,
    arguments: Box<[RuntimeHostArgumentTemplate]>,
}
```

`HostOperationPlanAdmission` has private fields and is issued only after exact
catalog lookup. The builder consumes it into the final template. Its semantic
digest is inherent on the existing catalog row and commits operation family,
capability, and `HostTaskRequestContract`, but excludes route, restart, and
cancellation fields. Runtime lookup still uses `HostOperationIdentity`; the
task request transcript writes `request_semantic`, so a physical route change
does not silently become plan identity.

The exact payloads of the View variant and any `.1.2`-derived expression child
roles remain gated. The enum must not be added to production until those
payload types are accepted.

The binding seed is already fully constructible and contains no upper View
identity:

```rust
pub enum RuntimeTaskSemanticBindingSeed {
    Ordinary,
    View,
    AwaitManyBase,
    AwaitManyChild,
    Timeout { contract: RuntimeNeedTimeoutContractSeed },
    Line { group: RuntimeLineTaskGroupSeedId },
}
```

`NeedProducerFamily::validate_runtime_task_binding` is the sole exhaustive
family/binding match. `AwbcTaskPlan` rejects from this structured owner.
The builder derives `NeedTimeoutContractDigest` and `LinePlanSemanticDigest`
from the accepted typed timeout contract and final line-task group; a caller
cannot seed either binding with digest bytes.

## 2. Final core row and runtime references

The final immutable row is constructed only inside builder materialization:

```rust
pub struct RuntimeTaskPlan {
    producer_function: RuntimeFunctionSiteId,
    family: NeedProducerFamily,
    class: TaskClass,
    request: RuntimeTaskRequestTemplate,
    control_effect: RuntimeControlEffectContract,
    binding: RuntimeTaskSemanticBinding,
}

pub enum RuntimeTaskSemanticBinding {
    Ordinary,
    View,
    AwaitManyBase,
    AwaitManyChild,
    Timeout { contract: NeedTimeoutContractDigest },
    Line { plan: LinePlanSemanticDigest },
}
```

`RuntimeNeedTimeoutContract` and the final `LineTaskGroup` are the same-cut
semantic owners of the two payload digests. Their purpose-built inherent
visitors own the domains
`arcweft.need.timeout-contract.v1\0` and
`arcweft.line.plan-semantic.v1\0`. Cut 5 removes the current raw
`NeedTimeoutContractDigest::from_bytes`; `LinePlanSemanticDigest` is never
introduced with a raw constructor.

There is no invented `RuntimeControlEffectContractId` table. The current
repository has no such owner, and indirection would create a second lookup
authority. The closed, immutable contract is owned inline by the task row; its
digest is an inherent traversal over that value. If accepted predecessor
evidence proves that an already-owned core table is required, `.1.3.1`
finalization must record that evidence before changing this decision.

Candidate executable edges use coordinates; final public rows use a table
index:

```rust
pub struct RuntimeTaskPlanIndex(u32); // private constructor

pub struct RuntimeHostCallTarget {
    task_plan: RuntimeTaskPlanIndex,
}

pub struct RuntimeAwaitTarget {
    task_plan: RuntimeTaskPlanIndex,
    source: RuntimeExpr,
}

pub struct RuntimeAwaitManyTarget {
    base_plan: RuntimeTaskPlanIndex,
    child_plan: RuntimeTaskPlanIndex,
    source: RuntimeExpr,
    item_binding: RuntimeLocalDeclarationId,
    limit: usize,
}
```

Static operation/request/outcome/control data moves to the referenced task
row. Live `NeedId`, `TaskId`, generation, launch ordinal, priority, and
cancellation scope remain on their accepted runtime owners and are not added
to `RuntimeTaskPlan`. The Cut 5 migration must cover every current
`FlowOp::{HostCall, Await, AwaitMany}` and every accepted timeout/line/View
producer edge before the old embedded request/ID fields are deleted.

## 3. Digest and table ownership

```rust
pub struct RuntimeExecutableSemanticDigest([u8; 32]);
pub struct ProducerFunctionSemanticDigest([u8; 32]);
pub struct TaskRequestTemplateDigest([u8; 32]);
pub struct ControlEffectContractDigest([u8; 32]);
pub struct TaskPlanSemanticDigest([u8; 32]);

struct SealedRuntimeTaskPlanRow {
    plan: RuntimeTaskPlan,
    digest: TaskPlanSemanticDigest,
}

pub struct RuntimeTaskPlanTable {
    rows: Box<[SealedRuntimeTaskPlanRow]>,
    by_digest: BTreeMap<TaskPlanSemanticDigest, RuntimeTaskPlanIndex>,
}
```

All digest byte fields and hasher-output constructors are private to the
semantic owner. Public code receives `as_bytes` and value comparison only.
Cut 5 deletes the current `TaskPlanSemanticDigest::from_bytes` and Serde
implementations. Private decode keeps `ExpectedTaskPlanKey([u8; 32])` and
resolves it only by comparing with recomputed table rows.

`RuntimeTaskPlanTable` is built after all rows seal. It exposes lookup by index
and digest; it exposes no iterator before the enclosing `RuntimePlan` exists.

## 4. Core-owned prefix and one-use View completion

Core never hands a hasher, writer, buffer, prefix bytes, or completed child
digest tuple to the authority.

```rust
pub struct ViewTaskPlanDigestRequest<'a> {
    issuer: &'a RuntimePlanSemanticSealIssuer,
    coordinate: &'a RuntimeTaskPlanBuildCoordinate,
    prefix: blake3::Hasher,
    family: NeedProducerFamily,
    binding: RuntimeTaskSemanticBindingKind,
}

impl ViewTaskPlanDigestRequest<'_> {
    pub const fn coordinate(&self) -> &RuntimeTaskPlanBuildCoordinate;
    pub const fn family(&self) -> NeedProducerFamily;
    pub const fn binding(&self) -> RuntimeTaskSemanticBindingKind;

    // Exact argument types are finalized from the accepted .1.4 owner.
    // This is one operation, not a sink: core appends tag, program, site,
    // admission, finalizes, and consumes self.
    pub fn finish_view_binding(
        self,
        program: ViewProgramSemanticRef<'_>,
        site: ViewMatchSiteSemanticRef<'_>,
        admission: CheckedViewMatchAdmissionSemanticRef<'_>,
    ) -> Result<TaskPlanSemanticDigest, ViewTaskPlanValidationError>;
}
```

The three `*SemanticRef` names denote the actual borrowed `.1.4` product types,
not core-owned DTOs. They deliberately remain unresolved until `.1.4` intake;
production must not create stand-ins. The final signature must name either the
actual shared lower types or narrow traits implemented by those actual types.
It must not accept `&str`, `[u8; 32]`, `Vec<u8>`, `Hasher`, `Write`, or a
caller-completed digest.

The existing protocol is extended, not replaced:

```rust
pub trait ViewTaskPlanAuthority {
    fn validate_view_task_plan(
        &self,
        request: ViewTaskPlanValidation<'_>,
    ) -> Result<(), ViewTaskPlanValidationError>;

    fn validate_task_plan_seal_scope(
        &self,
        scope: ViewTaskPlanSealScope<'_>,
    ) -> Result<(), ViewTaskPlanValidationError>;

    fn seal_view_task_plan(
        &self,
        request: ViewTaskPlanDigestRequest<'_>,
    ) -> Result<TaskPlanSemanticDigest, ViewTaskPlanValidationError>;
}
```

The caller explicitly selects this authority as the View semantic trust root.
An arbitrary implementation may reject or choose different valid upper View
semantics, just as it may reject live validation, but it cannot omit, reorder,
or replace the core prefix and cannot mint a digest outside an active seal.
The production implementation is the accepted validated View product.

`ViewTaskPlanSealScope` is also core-minted, borrowed, non-Clone, and
nonserialized. It exposes only an exact-size iterator of ordered borrowed View
coordinates. It lets the upper product reject stale, missing, extra,
duplicate, reordered, or foreign-candidate bindings before semantic hashing;
it is not a second catalog or transcript input.

## 5. Two-stage runtime-plan result

```rust
// arcweft-runtime-plan
pub struct RuntimePlanLowerDraft {
    builder: RuntimePlanBuilder,
    stats: RuntimePlanLowerStats,
    non_plan_products: RuntimePlanLowerProducts,
    view_joins: Box<[RuntimeViewTaskPlanJoin]>,
}

impl RuntimePlanLowerDraft {
    pub fn view_joins(&self) -> &[RuntimeViewTaskPlanJoin];

    pub fn finish_without_view(self)
        -> Result<RuntimePlanLowerReport, Vec<RuntimePlanLowerError>>;

    pub fn finish_with_view_authority(
        self,
        authority: &dyn ViewTaskPlanAuthority,
    ) -> Result<RuntimePlanLowerReport, Vec<RuntimePlanLowerError>>;
}
```

`lower_runtime_plan_with_stats` is replaced, not duplicated, by
`lower_runtime_plan_draft`. The draft is nonserialized and owns the sole
builder. It exposes no `RuntimePlan`, row mutation, or digest. Its non-plan
products are moved into the final report only after builder sealing succeeds.

`RuntimeViewTaskPlanJoin` contains the core coordinate and the exact `.1.4`
compiler-local join key. Its second field cannot be finalized until `.1.4`
returns; no raw `ExprId`, HIR ID, source range, or string may substitute.

## 6. Bundle bridge constraint

Bundle must not name `CompilerLocalViewMatchCatalogRow`. The compiler maps the
catalog plus `RuntimeViewTaskPlanJoin` into a bundle constructor input whose
field types are owned by existing common dependencies (`arcweft-core` and the
accepted `.1.4` View product owner):

```rust
// arcweft-bundle; exact View types finalized after .1.4 intake
pub struct ViewTaskPlanBindingInput {
    coordinate: RuntimeTaskPlanBuildCoordinate,
    program: ViewProgramId,
    accepted_revision: AcceptedViewProgramRevision,
    site: ViewMatchSiteId,
    admission: CheckedViewMatchAdmissionDigest,
}
```

If `.1.4` places `site` or `admission` in compiler-only or sema-only modules,
this bridge is not constructible and `.1.3.1` remains blocked. The accepted
owner must be a legitimate shared dependency; no byte projection wrapper may
be invented during Cut 5.

The validated program stores an ordinal-sorted boxed slice of bindings. Each
binding retains its coordinate, so lookup compares issuer identity as well as
ordinal. It implements both methods of `ViewTaskPlanAuthority` and keeps
`AcceptedViewProgramRevision` only as freshness evidence.
