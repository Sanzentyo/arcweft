# Real source/deletion inventory and compile-clean sequence

## Repository basis

- observed latest `origin/main`: `f42157fc4c8ca890eeaacec6dde3bb3e3af5d009`
- production source cut reconciled by the request/intake: `3670625a02b9e7e8578b57fc7b148a1758a17dba`
- source reads were performed through the private GitHub connector; no repository file was modified.

The table names concrete existing files and the final owner/migration action. Paths are implementation targets, not a patch bundled here.

## Source and deletion inventory

| Path | Current authority/route | Required implementation | Same-cut deletion/absence |
|---|---|---|---|
| `crates/arcweft-core/src/task.rs` | Current TaskSpec/HostTaskRequest/TaskEvent families | add final task identity/spec/execution/correlation/NeedHandle states, protocol envelopes and TaskLaunchAdapter in the original core owner; replace event key construction | delete old field shapes/constructors once all consumers use final owners |
| `crates/arcweft-core/src/value.rs` | RuntimeValue and RuntimeFunctionBody | add RuntimeValue::NeedHandle and inherent canonical/snapshot integration in the existing exhaustive matches | no extension trait, second RuntimeValue enum, or second digest visitor |
| `crates/arcweft-core/src/value/awbc_save.rs` | Existing AwbcRuntimeValueSnapshot owner | evolve in place with NeedHandle and exact executable-authority row; retain current exact nested variants | delete any parallel RuntimeValueSnapshotV1/generic summary reader |
| `crates/arcweft-core/src/pattern.rs` | RuntimeCheckedType | add inherent projection to complete RuntimeCheckedTypeProjectionV1 or place the projection enum beside this owner | delete undefined prose-only projection references |
| `crates/arcweft-core/src/plan/type_kind.rs` | RuntimeAgentOperationalType/RuntimeAgentTypeProjection | add inherent projection/catalog validation used by ownership admission | no helper trait that mirrors the Arcweft-owned enum |
| `crates/arcweft-core/src/awbc/schema.rs` | Existing AwbcOpcode/MakeNeedHandle/NeedTimeout allocation | wire final handle construction semantics without renumbering | delete no opcode and add no compatibility decoding table |
| `crates/arcweft-runtime-scheduler/src/lib.rs` | Current deterministic scheduler | replace single-row-only mutation path with shared internal plan/apply engine; own ensure batch, observer allocator, cancellation and restart restore transaction | delete per-child aggregate ensure commits, ephemeral observer IDs, old event order keys |
| `crates/arcweft-runtime-scheduler/Cargo.toml` | Current dependency only on arcweft-core | retain this lower dependency set | forbid arcweft-host-adapter/runtime-host/native/web/headless dependency |
| `crates/arcweft-host-adapter/src/lib.rs` | HostAdapter::submit and cancel(&TaskId)->bool | implement core TaskLaunchAdapter with unpublished reservation tokens and worker-visible commit queue | delete immediate submit/cancel-bool trait, registry forwarding and direct prepare-time worker start |
| `crates/arcweft-runtime-host/src/native_task.rs` | Native task dispatch/cancellation | split reservation from worker-visible enqueue and emit typed post-commit InfrastructureFailure | delete direct submit/cancel timing reachable from prepare |
| `crates/arcweft-adapter-desktop/src/adapter.rs` | Desktop adapter task route | implement/forward the core protocol upward without moving trait ownership | delete direct scheduler-to-desktop coupling |
| `crates/arcweft-lang-hir/src/expr.rs` | HirExprKind and direct_expression_children | add inherent stable semantic tag and ordered CheckedExpressionChildRole projection beside the original enum | delete duplicated external role-order switch |
| `crates/arcweft-lang-hir/src/pattern.rs` | HirPatternKind | add inherent stable pattern tag/payload transcript integration | delete wildcard/default family handling |
| `crates/arcweft-lang-sema/src/final_analysis/model.rs` | CheckedExpressionResolution and selected/value facts | publish exact constructor tags and catalog-backed callable join APIs | delete HirName/source spelling as semantic identity |
| `crates/arcweft-lang-sema/src/callable/identity.rs` | CheckedCallableId/CheckedCallableDigest/catalog | expose exact same-cut join constructor consumed by Match transcript | delete ad hoc RuntimeCallableId derivation without catalog digest equality |
| `crates/arcweft-lang-sema/src/types.rs` | Current 85 TypeKind variants | implement one exhaustive ownership classifier with exact same-cut projection owners | delete IntOrUInt and ambiguous Tuple-or-Variant success prose |
| `crates/arcweft-compiler/src/persistent.rs` | Persistent compiler/View rows | retain persistent semantic facts without HirSnapshotId/ExprId | delete any compiler-local ID persistence |
| `crates/arcweft-runtime-plan/src/awbc_lower/inventory.rs` | AWBC lowering inventory | map MakeNeedHandle and runtime task rows to final owners and exact truth table | delete old origin/finished-child-only task shape |
| `docs/02-runtime/async-scheduler.md` | Maintained scheduler contract | update TaskSpec/protocol/batch/observer/event-order/restartable snapshot model in Cut 5 | delete obsolete immediate Host submission and old TaskSpec shape |
| `docs/02-runtime/need-timeout.md` | Maintained timeout contract | retain timeout as runtime-owned Need producer under final task substrate | delete no timeout semantics |
| `docs/02-runtime/executable-runtime-core.md` | Maintained AWBC v1 allocation | update MakeNeedHandle semantics and persistence owner references | do not renumber or retain compatibility allocation |

## Cut 5 hard deletion set

- `NeedHandleOrigin`
- `AwaitMany source_items+finished-child-spec-only request`
- `per-child committed aggregate ensure path`
- `ephemeral observer ID allocation`
- `HostAdapter::submit immediate-launch path`
- `HostAdapter::cancel(&TaskId)->bool path`
- `lossy/parallel runtime snapshot owners`
- `IntOrUInt ownership row`
- `sequence-before-TaskId order keys`
- `blanket active-Host snapshot rejection`

A wrapper around the old Host adapter methods is not a deletion: the old method timing starts work during what would be `prepare`, so both symbol and timing path must disappear before Cut 5 publishes.

## Compile-clean five-cut order

### Cut 1 — semantic Match identity and role authority

Dependencies: `[]`. Task types: `false`.

Publishes:

- CheckedExpressionChildRole and stable tags
- all 38 current HirExprKind semantic tags and ordered direct-child role grammars
- expression/value/select/pattern/literal/guard/coverage transcript tags
- CheckedCallableCatalogV1 joins and RuntimeCallableProjectionV1 constructor

Private until complete:
- any digest whose callable catalog join or role inventory is incomplete

### Cut 2 — ownership classification and same-cut projections

Dependencies: `[1]`. Task types: `false`.

Publishes:

- 85-row exhaustive ownership classifier
- RuntimeCheckedTypeProjectionV1
- RuntimeAgentValueProjectionV1 for the current concrete Agent value algebra
- RuntimeTextProjectionV1
- exact signed Int and unsigned UInt carriers

Private until complete:
- Need ownership certificate until Cut 5
- all rows lacking current/same-cut nominal/case/field owners

### Cut 3 — compiler-local Match lookup and role paths

Dependencies: `[1, 2]`. Task types: `false`.

Publishes:

- HirSnapshotId+ExprId lookup
- compiler-local role-path construction and differential checks

### Cut 4 — standalone identity, digest and sink infrastructure

Dependencies: `[1, 2, 3]`. Task types: `false`.

Publishes:

- retained Need/task identity newtypes and inherent constructors
- canonical RuntimeValue sink infrastructure
- HostOperationCatalogDigest/HostOperationId standalone typed catalog owner
- HostCancelCommandId canonical constructor

### Cut 5 — atomic runtime task, adapter and persistence publication

Dependencies: `[1, 2, 3, 4]`. Task types: `true`. Atomic: `true`.

Publishes:

- RuntimeNeedHandle ReusableJoin/AcceptedLaunch states and both constructors
- rederivable AwaitMany captured/template request
- whole-child batch ensure transaction
- persistent observer allocator
- core-owned launch/restore/rebind/cancel adapter protocol
- restartable Host snapshot policy
- in-place isomorphic AwbcRuntimeValueSnapshot codec including NeedHandle
- final ownership carriers held private before this cut
- maintained event ordering

Deletes in the same cut:
- NeedHandleOrigin
- AwaitMany source_items+finished-child-spec-only request
- per-child committed aggregate ensure path
- ephemeral observer ID allocation
- HostAdapter::submit immediate-launch path
- HostAdapter::cancel(&TaskId)->bool path
- lossy/parallel runtime snapshot owners
- IntOrUInt ownership row
- sequence-before-TaskId order keys
- blanket active-Host snapshot rejection

## Forward-reference admission rule

For each public row, construct the set of referenced named owners and resolve every owner to its publication cut. The maximum referenced cut must be less than or equal to the row's publication cut. A private certificate may cite a later owner only when its public publication is delayed to that same later atomic cut. This is why the Need ownership certificate is computed privately in Cut 2 but cannot be public until Cut 5.

## Structural source gates

- `arcweft-runtime-scheduler` dependency closure contains `arcweft-core` and  excludes host implementation crates.
- every Arcweft-owned enum behavior is implemented by an inherent match on its  original owner; extension-trait mirrors and wildcard success are rejected.
- `AwbcRuntimeValueSnapshot` is the sole runtime-value snapshot owner after  Cut 5; no second reader or DTO remains.
- all deleted adapter and task symbols have zero production references.
- every public cut compiles using only same/earlier public owners before the  next cut begins.
