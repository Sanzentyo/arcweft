# Exact version-one semantic transcripts

## 1. Common byte grammar

Every digest in this contract uses BLAKE3. The displayed ASCII domain bytes,
including the final NUL byte, are written first. No generic Serde value is ever
hashed.

```text
u8              one byte
bool             u8, false=0, true=1
u32-le           four little-endian bytes
u64-le           eight little-endian bytes
digest32         exactly 32 bytes
string           byte_len:u32-le || validated UTF-8 bytes
list<T>          count:u32-le || T rows in the stated order
option<T>        present:u8, 0=no payload, 1=one T payload
```

Counts are checked with `usize -> u32` conversion before a count byte is
written. String byte length is checked before the string bytes are written.
Unknown tags, noncanonical booleans/options, count overflow, invalid UTF-8, and
limit exhaustion reject without returning a digest.

## 2. Closed tags retained from the accepted parent

### Plan owner

```text
0 Structured RuntimeTaskPlan
1 AwbcTaskPlan
2 Line task plan
```

This child implements owner `0`. Owners `1` and `2` remain on their accepted
existing encoders; their numeric tags are not reallocated.

### Need producer family

```text
0 StructuredTaskPlan
1 AwbcTaskPlan
2 ViewMatchSubscription
3 AwaitManyBase
4 AwaitManyChild
5 Timeout
6 LineTask
7 HostAdapterTask
8 MakeNeedHandle
```

`NeedProducerFamily::semantic_tag()` is the sole authority.

### Task class

```text
0  LocalView
1  Io
2  Cpu
3  GpuPrepare
4  ShaderCompile
5  WasmCall
6  AssetDecode
7  AudioDecode
8  AudioRender
9  TtsSynthesis
10 BgmPrecompose
11 Lsp
12 Background
```

`TaskClass::semantic_tag()` is the sole authority.

### Semantic binding

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

The core executable-base row writes the same tag, but for View writes only tag
`1`; the three upper fields are added only by the validated View authority when
constructing the final task-plan digest.

## 3. `RuntimeExecutableSemanticDigest`

```text
domain = "arcweft.runtime-plan.executable-semantic.v1\0"
owner_tag:u8 = 0
table_count:u8 = 15
for each table in fixed tag order 0..14:
  table_tag:u8
  row_count:u32-le
  for each row in canonical dense/source order:
    row_ordinal:u32-le
    row_kind_tag:u8
    row_payload
```

For table tags `0..13`, `row_payload` is:

```text
row_semantic_digest:digest32
```

The digest is produced by the corresponding existing row owner's exhaustive
inherent semantic visitor with domain:

```text
"arcweft.runtime-plan.executable-row.v1\0"
table_tag:u8
row_kind_tag:u8
owner-specific ordered semantic fields
```

Table `14` is encoded inline as specified in section 7. The exact included row
families and source-order roles are fixed in `EXECUTABLE_TRANSCRIPT.md`.

The executable digest excludes:

- every final task-plan map key;
- every completed `TaskPlanSemanticDigest`;
- every task-plan self or expected digest;
- decoded expected-key bytes;
- producer contract, producer site, payload type, evaluated argument values;
- generation, policy, launch ordinal, priority, cancellation scope;
- accepted View revision and all upper View identity/admission payloads;
- source spans, raw HIR/arena/session IDs, source spelling, debug labels;
- map iteration order, caches, indexes, wire offsets, compression metadata; and
- whole-catalog or generic serialized digests.

## 4. `ProducerFunctionSemanticDigest`

```text
domain = "arcweft.runtime-plan.producer-function-semantic.v1\0"
function_semantic_id:digest32
function_role_tag:u8
parameter_count:u32-le
for each parameter in declaration/source order:
  parameter_ordinal:u32-le
  parameter_type:RuntimeTypeSemanticDigest
  passing_mode:u8
capture_count:u32-le
for each capture in canonical capture order:
  capture_ordinal:u32-le
  stable_capture_coordinate:digest32
  capture_type:RuntimeTypeSemanticDigest
  capture_mode:u8
return_type:RuntimeTypeSemanticDigest
body_root_semantic_digest:digest32
endpoint_count:u32-le
for each producer endpoint in source order:
  endpoint_ordinal:u32-le
  endpoint_kind:u8
  child_role_path_digest:digest32
```

Function role tags:

```text
0 Ordinary
1 Closure
2 Dialogue
3 Effect
4 Line
5 Stream
```

Passing mode tags:

```text
0 Value
1 Shared
2 Affine
```

Capture mode tags:

```text
0 Copy
1 SnapshotClone
2 Move
```

Producer endpoint kind tags:

```text
0 HostTask
1 ViewMatchSubscription
2 AwaitManyBase
3 AwaitManyChild
4 Timeout
5 LineTask
6 MakeNeedHandle
```

`function_semantic_id` is the accepted runtime semantic identity of the
function site, not its source name or raw plan-local allocation. Parameter and
capture ordinals commit their semantic source-order roles. The body digest is
emitted by the existing typed runtime expression/flow visitor. A task launch in
the body writes a `RuntimeTaskPlanBuildCoordinate` ordinal, never a final plan
digest.

Excluded fields are function source spelling, `SourceSpan`, HIR/session/arena
IDs, debug names, compiled addresses, JIT/AOT choices, cache keys, task-plan map
keys, task-plan completed digests, and evaluated runtime values.

## 5. `TaskRequestTemplateDigest`

```text
domain = "arcweft.task.request-template.v1\0"
endpoint:
  producer_function_semantic_digest:digest32
  endpoint_ordinal:u32-le
  endpoint_kind:u8
argument_role_count:u32-le
for each argument/capture role in declaration/source order:
  role_ordinal:u32-le
  argument_role_tag:u8
  accepted_name_or_field:option<digest32>
  semantic_type:RuntimeTypeSemanticDigest
  value_source_shape:u8
  expression_child_role_path
request_field_count:u32-le
for each static request field in declaration order:
  field_ordinal:u32-le
  accepted_field_identity:digest32
  field_role_tag:u8
  field_type:RuntimeTypeSemanticDigest
  expression_child_role_path
```

Argument role tags:

```text
0 Positional
1 Named
2 Spread
3 Capture
4 AwaitManyItem
5 TimeoutSource
6 TimeoutLimit
7 LineInput
```

Value source shape tags:

```text
0 LiteralRole
1 LocalRole
2 CaptureRole
3 ProjectionRole
4 CallResultRole
5 AggregateItemRole
6 NeedHandleRole
```

Request field role tags:

```text
0 Required
1 Optional
2 Repeated
3 NamedOnly
4 PositionalOnly
```

`expression_child_role_path` is:

```text
step_count:u32-le
for each step in root-to-leaf order:
  step_tag:u8
  step_payload
```

Step tags and payloads:

```text
0 Operand             source_ordinal:u32-le
1 TupleElement        source_ordinal:u32-le
2 RecordField         accepted_field_identity:digest32
3 VariantPayload      accepted_case_identity:digest32
4 CallArgument        source_ordinal:u32-le
5 NamedArgument       accepted_name_identity:digest32
6 SpreadArgument      source_ordinal:u32-le
7 Capture             capture_ordinal:u32-le
8 AwaitManySourceItem no payload
9 TimeoutSource       no payload
10 TimeoutLimit       no payload
11 LineChild          source_ordinal:u32-le
```

The transcript commits static, source-independent endpoint, field/type, and
child-role meaning. It does not commit literal payload bytes, evaluated values,
canonical `RuntimeValueDigest`, producer contract/site, payload result type,
generation, scheduling metadata, source text, or debug spelling. Those values
remain on their legitimate owners.

## 6. `ControlEffectContractDigest`

```text
domain = "arcweft.task.control-effect-contract.v1\0"
control_mode:u8
effect_count:u32-le
for each effect row in checked declaration/source order:
  effect_ordinal:u32-le
  effect_kind:u8
  accepted_effect_identity:option<digest32>
  input_type_count:u32-le
  input types in source order: RuntimeTypeSemanticDigest
  output_type:option<RuntimeTypeSemanticDigest>
  cardinality:u8
  ordering:u8
  cancellation:u8
  terminal_behavior:u8
child_contract_count:u32-le
for each child contract reference in source order:
  child_ordinal:u32-le
  child_contract_semantic_id:digest32
```

Control mode tags:

```text
0 StraightLine
1 MaySuspend
2 MustSuspend
3 RuntimeAggregate
4 TimeoutRace
5 LineTimeline
```

Effect kind tags:

```text
0 HostOperation
1 RuntimeOperation
2 ViewSubscription
3 AwaitObservation
4 TimeoutClock
5 LineEmission
6 CancellationObservation
```

Cardinality tags:

```text
0 ExactlyOnce
1 ZeroOrOne
2 ZeroOrMore
3 OneOrMore
```

Ordering tags:

```text
0 SourceOrder
1 CompletionOrder
2 SingleTerminal
```

Cancellation tags:

```text
0 NotObserved
1 ObservedNoPayload
2 PropagatesToChildren
```

Terminal behavior tags:

```text
0 Value
1 ResultValue
2 OptionValue
3 NonreturningCancellation
4 InfrastructureFailureControl
```

The checked control/effect row is semantic. Priority, cancellation scope ID,
scheduler queue, retry/backoff policy, launch policy/ordinal, generation,
debug label, physical adapter route, and accepted View revision are excluded.

## 7. Structured task-plan base row in the executable transcript

For fixed executable table tag `14`:

```text
row_kind_tag:u8 = 0
coordinate:u32-le
producer_function_semantic_digest:digest32
family_tag:u8
task_class_tag:u8
request_template_digest:digest32
control_effect_contract_digest:digest32
binding_shape
```

`coordinate` must equal the current zero-based row ordinal. `binding_shape` is:

```text
0 Ordinary:       no payload
1 View marker:    no payload
2 AwaitManyBase:  no payload
3 AwaitManyChild: no payload
4 Timeout:        NeedTimeoutContractDigest:digest32
5 Line:           LinePlanSemanticDigest:digest32
```

The View row intentionally has no program/site/admission payload in core.

## 8. Final structured `TaskPlanSemanticDigest`

For every core-owned non-View row:

```text
domain = "arcweft.task.plan-semantic.v1\0"
plan_owner_tag:u8 = 0
RuntimeExecutableSemanticDigest:digest32
ProducerFunctionSemanticDigest:digest32
NeedProducerFamily.semantic_tag:u8
TaskClass.semantic_tag:u8
TaskRequestTemplateDigest:digest32
ControlEffectContractDigest:digest32
semantic_binding  // section 2
```

For a View row, `ValidatedViewProgramResource` writes the identical prefix from
opaque base getters and then writes View binding tag/payload.
`AcceptedViewProgramRevision` is checked but not written.

## 9. Explicit exclusions and legitimate owners

| Excluded task-plan input | Legitimate owner that changes |
|---|---|
| producer contract | `NeedProducerContractDigest` and producer instance |
| producer site | `NeedProducerInstanceKey` |
| payload type | `RuntimeTypeSemanticDigest` and producer instance |
| actual arguments | existing canonical `RuntimeValueDigest` and producer instance |
| generation | TaskKey/TaskId correlation owner |
| policy | Need/Task correlation owner |
| launch ordinal | NeedId/TaskId correlation owner |
| priority | final `TaskSpec` scheduling field |
| cancellation scope | final `TaskSpec` scheduling field |
| debug label/source spelling | diagnostic/source owner only |
| expected stored digest | private codec verification only |
| accepted View revision | validated View resource/replacement authority |

Mutation tests must demonstrate both halves: the plan digest is unchanged and
the listed legitimate owner changes or rejects as appropriate.
