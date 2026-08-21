# Identity and digest grammars

All byte sequences below are exact version-1 semantic transcripts. Unless a
field explicitly says otherwise, integer fields in these hash transcripts are
fixed little-endian. This does not change the maintained private AWBC wire
grammar, whose ordinary `u32` values remain canonical shortest base-128
varints.

All hashes use BLAKE3 with the displayed NUL-terminated domain bytes written
first. No transcript is serialized through generic Serde.

## 1. Common primitives

```text
u8              exactly one byte
u32-le          exactly four little-endian bytes
u64-le          exactly eight little-endian bytes
digest32        exactly 32 bytes
string          byte_len:u32-le || validated UTF-8 bytes
option<T>       present:u8 (0 or 1) || T only when present=1
list<T>         count:u32-le || source/canonical-order elements
```

String inputs are accepted semantic identities, not source/debug spelling.
Counts are checked before allocation and must fit `u32`.

## 2. Digest-only producer family tags

These tags exist only inside the producer-instance transcript. They are not
AWBC opcodes, function kinds, function flags, or wire discriminants.

| Tag | `NeedProducerFamily` | Producer site meaning |
|---:|---|---|
| 0 | `StructuredTaskPlan` | accepted structured runtime task site |
| 1 | `AwbcTaskPlan` | accepted AWBC task producer site |
| 2 | `ViewMatchSubscription` | checked View Match subscription producer site |
| 3 | `AwaitManyBase` | one aggregate AwaitMany Need |
| 4 | `AwaitManyChild` | one source-indexed child producer |
| 5 | `Timeout` | derived timeout producer |
| 6 | `LineTask` | accepted line-plan child task |
| 7 | `HostAdapterTask` | accepted direct host/adaptor producer |
| 8 | `MakeNeedHandle` | verified reusable handle producer |

Unknown tags are rejected. The enum and its inherent methods are the only
numeric authority for this table.

## 3. `NeedProducerContractDigest`

```text
domain = "arcweft.need.producer-contract.v1\0"
owner_tag:u8
owner_payload:
  owner_tag = 0, CheckedCallable:
    callable:string
    CallableContractHash:digest32

  owner_tag = 1, HostOperation:
    HostCapabilityId:string
    RuntimeHostOperationId:string
    CallableContractHash:digest32

  owner_tag = 2, BuiltinTimeout:
    NeedTimeoutContractDigest:digest32
```

The callable/host identifiers are accepted catalog values. The digest excludes:

- producer family;
- executable or task plan;
- producer site;
- payload type;
- actual argument values/digest;
- generation;
- task policy;
- launch ordinal;
- priority/cancellation scope; and
- debug/source strings.

Thus this owner answers only “which accepted producer contract?” and does not
duplicate instance, plan, value, or correlation authority.

## 4. `TaskPlanSemanticDigest`

The digest is computed by the owning task-plan type. It is never a caller
field and is never stored inside the same plan.

```text
domain = "arcweft.task.plan-semantic.v1\0"
plan_owner_tag:u8
executable_semantic_digest:digest32
producer_function_semantic_digest:digest32
family_tag:u8
task_class_tag:u8
request_template_digest:digest32
control_effect_contract_digest:digest32
semantic_binding
```

Plan owner tags:

| Tag | Owner | `executable_semantic_digest` owner |
|---:|---|---|
| 0 | structured `RuntimeTaskPlan` | the owning `RuntimePlan` semantic encoder |
| 1 | `AwbcTaskPlan` | the owning `AwbcProgram` semantic encoder |
| 2 | line task plan | the accepted line-plan semantic encoder |

`TaskClass::semantic_tag()` is inherent on the existing enum and uses this
digest-only order:

```text
LocalView=0, Io=1, Cpu=2, GpuPrepare=3, ShaderCompile=4,
WasmCall=5, AssetDecode=6, AudioDecode=7, AudioRender=8,
TtsSynthesis=9, BgmPrecompose=10, Lsp=11, Background=12
```

`request_template_digest` is:

```text
domain = "arcweft.task.request-template.v1\0"
accepted endpoint coordinate
static source-independent expression child-role transcript
static request field/type roles in declaration order
```

It excludes evaluated runtime values. `control_effect_contract_digest` is the
accepted checked control/effect row of the producer and excludes scheduling
metadata.

`semantic_binding` is exactly one row:

```text
0 Ordinary:
  no payload

1 View:
  ViewProgramId:string
  ViewMatchSiteId:digest32
  CheckedViewMatchAdmissionDigest:digest32

2 AwaitManyBase:
  no payload

3 AwaitManyChild:
  no payload

4 Timeout:
  NeedTimeoutContractDigest:digest32

5 Line:
  LinePlanSemanticDigest:digest32
```

The View row includes current `ViewProgramId`, stable site, and exact admission;
it excludes `AcceptedViewProgramRevision`.

The plan digest excludes producer contract, producer site, payload type,
actual arguments, generation, policy, launch ordinal, priority, cancellation
scope, debug label, expected stored digest, and accepted View revision. Those
fields are owned elsewhere.

Identical static plans may share the same plan digest. Their stable
`producer_site` field in the producer-instance transcript distinguishes sites.

## 5. Runtime type semantic digest

`RuntimeTypeSemanticDigest` is computed by the existing closed
`RuntimeCheckedType`/runtime-plan type projection owner:

```text
domain = "arcweft.runtime-type.semantic.v1\0"
closed runtime type tag:u8
type-specific payload:
  scalar: none or exact width tag
  sequence/tuple/result/option/shared/need: child digest(s), source order
  nominal: accepted nominal semantic identity digest
  opaque:
    producer identity
    RuntimeOpaqueValueClass inherent semantic transcript
    RuntimeOpaquePersistence inherent semantic tag
    child type arguments in source order
```

It contains no source type spelling, HIR `TypeId`, plan-local type index, debug
label, generation, or runtime value. Compiler projection is total and checked;
it does not publish a second type map.

## 6. `NeedProducerInstanceKey`

The exact request-mandated transcript is:

```text
domain = "arcweft.need.producer-instance.v1\0"
family_tag:u8
NeedProducerContractDigest:digest32
TaskPlanSemanticDigest:digest32
producer_site:u32-le
payload_type_digest:digest32
arguments_digest:arcweft_core::entry::RuntimeValueDigest
```

```text
NeedProducerInstanceKey =
  BLAKE3(domain || ordered fields above)
```

The result must not be all zero. A zero result returns
`NeedProducerIdentityError::ZeroHash(NeedProducerInstance)`; there is no
rehash, salt, retry, or fallback identity.

## 7. Need/task correlations

Exact transcripts:

```text
NeedId =
  BLAKE3(
    "arcweft.need.id.v1\0"
    || NeedProducerInstanceKey:digest32
    || policy:u8
    || launch_ordinal:u64-le
  )

TaskKey =
  BLAKE3(
    "arcweft.task.key.v1\0"
    || GenerationId:u64-le
    || NeedProducerInstanceKey:digest32
    || policy:u8
  )

TaskId =
  BLAKE3(
    "arcweft.task.id.v1\0"
    || TaskKey:digest32
    || launch_ordinal:u64-le
  )
```

Policy tags are `JoinSameKey=0` and `AlwaysStart=1`.

The fixed results reject all-zero. `GenerationId(0)` and Join ordinal `0` are
valid. `TaskKey` never contains the launch ordinal. `TaskId` contains it once;
the ordinal already inside `NeedId` is not copied into TaskId.

## 8. Policy truth table

| Policy | Ordinal source | First ordinal | Reusable pre-launch handle | Same generation + instance | Need cell | Task group | Launch |
|---|---|---:|---:|---|---|---|---|
| `JoinSameKey` | constant | 0 | yes | same NeedId/TaskKey/TaskId | one | one | one |
| `AlwaysStart` | journal counter scoped to `(GenerationId, NeedProducerInstanceKey)` | 1 | no | same TaskKey; distinct NeedId/TaskId per ordinal | one per launch | one | one per call |

Across generations, the same producer instance and ordinal retain the same
NeedId but receive a different TaskKey and TaskId. Every runtime lookup carries
`GenerationId` in `TaskCorrelation`, and terminal conflict is generation
scoped.

## 9. Canonical RuntimeValue arguments

The current canonical runtime-value owner begins directly with its version-1
variant tag. The new canonical tag is allocated in that existing owner:

```text
RuntimeValue::NeedHandle tag = 20
payload = NeedId:digest32
```

Existing tags remain unchanged. This is a RuntimeValue grammar row, not an AWBC
numeric allocation.

Arguments:

```text
zero arguments  = RuntimeValue::Tuple([])
N arguments     = RuntimeValue::Tuple([arg0, ..., argN-1])
AwaitMany items = RuntimeValue::Tuple([item0, ..., itemN-1])
```

`RuntimeValue::try_digest` hashes the exact bytes emitted by the same
sink-parametric visitor. Empty arguments therefore equal:

```text
digest(canonical_bytes(RuntimeValue::Tuple([])))
```

and must not equal or be substituted by `RuntimeValueDigest::ZERO`.

AwaitMany child arguments are:

```text
RuntimeValue::Tuple([
  RuntimeValue::Tuple(captured_arguments),
  RuntimeValue::UInt(U32(source_index)),
  item
])
```

The base arguments are:

```text
RuntimeValue::Tuple([
  RuntimeValue::Tuple(captured_arguments),
  RuntimeValue::Tuple(source_items)
])
```

Timeout arguments are:

```text
RuntimeValue::Tuple([
  RuntimeValue::NeedHandle(source),
  limit
])
```

## 10. `CheckedMatchSemanticDigest`

```text
domain = "arcweft.lang.checked-match-semantic.v1\0"
checked_scrutinee_expression_digest:digest32
checked_scrutinee_type_digest:digest32
arm_count:u32-le
for each arm in source order:
  arm_ordinal:u32-le
  checked_pattern_digest:digest32
  binding_count:u32-le
  for each binding in stable pattern-preorder:
    stable_pattern_coordinate
    binding_type_digest:digest32
  guard_presence:u8
  if present:
    checked_guard_expression_digest:digest32
    guard_class:u8
  checked_arm_body_expression_digest:digest32
coverage_exhaustive:u8
unreachable_count:u32-le
for each unreachable row sorted by arm ordinal:
  arm_ordinal:u32-le
  unreachable_reason:u8
```

Guard class tags:

```text
ConstantTrue=0
ConstantFalse=1
Dynamic=2
```

The `None` guard is represented by `guard_presence=0`, not a fourth guard
class.

A stable pattern coordinate is:

```text
step_count:u32-le
steps in pattern preorder:
  pattern_step_tag:u8
  optional source-order child ordinal:u32-le
  optional accepted field/case semantic identity:digest32
```

Unreachable reason tags:

```text
CoveredByPriorRows=0
FalseGuard=1
RedundantOrAlternative=2
UninhabitedDomain=3
```

The public retained list remains arm-based; `RedundantOrAlternative` may be
retained only as the reason for an arm that is wholly redundant.

Included fields are exactly:

- checked scrutinee expression and type;
- source-ordered arms;
- checked pattern per arm;
- binding stable pattern coordinate and type;
- checked guard expression and exact Boolean-literal guard class;
- checked arm body; and
- exhaustive value plus sorted unreachable arm ordinal/reason.

Excluded fields are exactly:

- `ViewProgramId`;
- `AcceptedViewProgramRevision`;
- View site/arm/output coordinates;
- View ownership/persistence/resource evidence;
- coverage counters and work accounting;
- HIR/session/arena IDs;
- `SourceSpan`; and
- source/debug spelling.

## 11. `OwnershipEvidenceDigest`

Only consulted evidence is committed:

```text
domain = "arcweft.lang.ownership-evidence.v1\0"
row_count:u32-le
rows sorted by (row_tag, semantic key bytes):
  row_tag:u8
  row payload
```

Rows:

```text
0 ProjectNominal:
  accepted project nominal semantic identity:digest32
  checked type digest:digest32
  declaration-shape digest:digest32

1 AcceptedOpaque:
  accepted nominal semantic identity:digest32
  runtime producer:string
  RuntimeOpaqueValueClass semantic transcript
  RuntimeOpaquePersistence semantic tag:u8

2 AgentDto:
  Agent DTO kind tag:u8
  core Agent DTO snapshot contract digest:digest32

3 StableCallableValue:
  RuntimeCallableId:string
  CallableContractHash:digest32
```

Duplicate consulted rows are removed before sorting. An unrelated catalog row
does not affect the digest. The whole `AcceptedNominalCatalogDigest` is not a
substitute.

## 12. `CheckedNeedProducerAdmissionDigest`

```text
domain = "arcweft.lang.need-producer-admission.v1\0"
argument_count:u32-le
for each argument/capture in source order:
  stable_checked_value_coordinate
  semantic_type_digest:digest32
  disposition:u8  // Copy=0, SnapshotClone=1
OwnershipEvidenceDigest:digest32
```

It excludes producer contract, family, task plan, producer site, runtime
argument values/digest, policy, generation, and IDs. This certificate answers
only whether the producer may safely retain the selected values.

## 13. `CheckedViewMatchAdmissionDigest`

```text
domain = "arcweft.view.checked-match-admission.v1\0"
CheckedMatchSemanticDigest:digest32
retained_output_count:u32-le
for each retained output in source order:
  stable_checked_value_coordinate
  semantic_type_digest:digest32
  disposition:u8
retained_capture_count:u32-le
for each retained capture in source order:
  stable_checked_value_coordinate
  semantic_type_digest:digest32
  disposition:u8
OwnershipEvidenceDigest:digest32
CheckedNeedProducerAdmissionDigest:digest32
```

It excludes `ViewProgramId`, revision, site, source/HIR coordinates, whole
resource/nominal catalog digests, and work counters.

## 14. `ViewMatchSiteId`

```text
domain = "arcweft.view.match-site.v1\0"
ViewProgramId:string
AcceptedDeclarationSemanticId:digest32
child_role_count:u32-le
for each checked child role:
  child_role_tag:u8
  role-specific source-order ordinal or accepted field identity
```

The closed child-role tags are owned by
`CheckedExpressionChildRole`. HIR IDs, `SourceSpan`, revision, and spelling are
not inputs. As a semantic coordinate digest, zero is not reserved; absence is
`Option<ViewMatchSiteId>`.

## 15. View task identity and revision proof

For a View producer:

```text
TaskPlanSemanticDigest
  includes ViewProgramId + ViewMatchSiteId + CheckedViewMatchAdmissionDigest
NeedProducerInstanceKey
  includes that plan digest + producer contract/site/payload/arguments
```

`AcceptedViewProgramRevision` is absent from both transcripts. A
revision-only replacement therefore preserves the producer instance and
NeedId. The replacement transaction changes only the generation-bound TaskKey
and TaskId.

## 16. Event digest and replay

Replay envelopes use:

```text
domain = "arcweft.task.event.v1\0"
TaskCorrelation fields in declared order
cursor.logical_epoch:u64-le
cursor.sequence:u64-le
TaskEventKind canonical transcript
```

Event kind transcript:

```text
Progress=0 || canonical Progress RuntimeValue bytes
Ready=1 || canonical RuntimePayload bytes
InfrastructureFailure=2 || failure_kind:u8 || bounded diagnostic bytes
Cancelled=3
```

The event digest is not identity and accepts the full BLAKE3 output. Replay
first validates the stored digest, then correlation, then cursor semantics.

## 17. Nonduplication matrix

| Field | Sole transcript owner |
|---|---|
| accepted producer contract | `NeedProducerContractDigest` |
| executable/static task meaning | `TaskPlanSemanticDigest` |
| selected producer row | `producer_site` in instance key |
| payload runtime type | `RuntimeTypeSemanticDigest` in instance key |
| evaluated source-order arguments | existing `RuntimeValueDigest` in instance key |
| policy | `NeedId` and `TaskKey` |
| generation | `TaskKey` |
| launch ordinal | `NeedId` and `TaskId` |
| generic Match meaning | `CheckedMatchSemanticDigest` |
| retained View persistence | `CheckedViewMatchAdmissionDigest` |
| accepted View catalog revision | `AcceptedViewProgramRevision`, outside identities |

No row is reconstructed from source, debug labels, map iteration, generic
Serde, or an old identity translation table.
