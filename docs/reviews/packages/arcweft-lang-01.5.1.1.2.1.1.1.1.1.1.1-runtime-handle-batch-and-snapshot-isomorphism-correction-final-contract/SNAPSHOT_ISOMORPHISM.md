# Live value ↔ snapshot isomorphism

## Sole owner and codec

The existing `AwbcRuntimeValueSnapshot` is evolved in place. The final version-1 codec is purpose-built, rejects unknown/duplicate fields and trailing bytes, and has no compatibility reader. `RuntimeValue::NeedHandle` is added in the same atomic Cut 5.

## Top-level inventory

| Tag | Live variant | Live semantic fields | Snapshot fields | Admission |
|---:|---|---|---|---|
| `0x00` | `Unit` | — | — | lossless |
| `0x01` | `Bool` | value | value: bool | lossless |
| `0x02` | `Int` | value | value: RuntimeInt | lossless |
| `0x03` | `UInt` | value | value: RuntimeUInt | lossless |
| `0x04` | `F32` | value_bits | value: f32 (exact IEEE bits) | lossless |
| `0x05` | `F64` | value_bits | value: f64 (exact IEEE bits) | lossless |
| `0x06` | `MatrixF32` | value | value: DenseMatrixF32 | lossless |
| `0x07` | `MatrixF64` | value | value: DenseMatrixF64 | lossless |
| `0x08` | `TensorF32` | value | value: DenseTensorF32 | lossless |
| `0x09` | `TensorF64` | value | value: DenseTensorF64 | lossless |
| `0x0A` | `String` | utf8 | value: String | lossless |
| `0x0B` | `Char` | scalar | value: char | lossless |
| `0x0C` | `Duration` | logical_duration | value: LogicalDuration | lossless |
| `0x0D` | `Progress` | ratio_bits, label | ratio: f32 (exact bits); label: Option<String> | lossless |
| `0x0E` | `Range` | range | value: RuntimeRange | lossless |
| `0x0F` | `Iterator` | iterator | value: AwbcRuntimeIteratorSnapshot | lossless |
| `0x10` | `EntityRef` | entity_reference | value: String | lossless |
| `0x11` | `Tuple` | items | items: Box<[AwbcRuntimeValueSnapshot]> | lossless |
| `0x12` | `Seq` | sequence | value: AwbcRuntimeSeqSnapshot | lossless |
| `0x13` | `Record` | fields | fields: Box<[AwbcRuntimeFieldSnapshot]> | lossless |
| `0x14` | `NominalRecord` | type_id, layout, fields | value: AwbcRuntimeNominalRecordSnapshot | lossless |
| `0x15` | `Opaque` | producer, semantic_identity, value_class, persistence, payload | value: AwbcRuntimeOpaqueSnapshot | lossless |
| `0x16` | `Reduction` | owner, state, commands | value: AwbcRuntimeReductionSnapshot | lossless |
| `0x17` | `Agent` | agent_value | value: AwbcRuntimeAgentSnapshot | lossless |
| `0x18` | `Function` | function_body | value: AwbcRuntimeFunctionSnapshot (AWBC only; Structured rejects) | AWBC only; Structured rejects |
| `0x19` | `Variant` | owner, ordinal, name, payload | owner: RuntimeVariantIdentity; ordinal: u32; name: String; payload: Option<Box<AwbcRuntimeValueSnapshot>> | lossless |
| `0x1A` | `NeedHandle` | correlation, producer, outcome, state | value: RuntimeNeedHandleSnapshotV1 | lossless |

Field-role equality is machine-checked: each accepted top-level row has the same semantic role sequence on the live and snapshot sides. The Rust storage type may change (`Vec` to boxed slice, `usize` to checked `u64`) only where restore proves the inverse conversion.

## Iterator

| Variant | Exact fields | Restore checks |
|---|---|---|
| `Values` | recursive items, `index: u64` | index fits `usize` and is at most item length |
| `Range` | complete `RuntimeRangeIterator` | exact range iterator invariants |
| `Witness` | recursive state, `RuntimeTraitMethodId next` | method ID resolves in the pinned plan catalog |

## Sequence

| Variant | Exact fields | Restore checks |
|---|---|---|
| `Values` | recursive items | work limits |
| `Dense` | one exact current `DenseSeq` case | storage length/case type |
| `TupleColumns` | `len`, ordered recursive columns | every column length equals `len` |
| `RecordColumns` | `len`, ordered field identity/name/values | IDs equal accepted ordinals, names unique, every column length equals `len` |

Current dense cases are:

`Units`, `Bools`, `I8`, `I16`, `I32`, `I64`, `I128`, `ISize`, `U8`, `U16`, `U32`, `U64`, `U128`, `USize`, `F32`, `F64`, `Strings`, `Chars`, `Durations`, `EntityRefs`, `Bytes`.

## Opaque, reduction and Agent

- Opaque stores producer, semantic identity, value class, persistence and the   recursively boxed payload. Restore reconstructs the exact owner and calls its   inherent validation; no opaque byte summary is accepted.
- Reduction stores owner, recursive state and ordered command rows. Every command   retains constructor, target and recursive payload.
- Agent stores all eight current live variants. Predicate recursively stores   Compare/Exists/ActionEnabled/DiagnosticsHasError/All/Any/Not with every operand.

## Functions

| Live body | Decision |
|---|---|
| `Awbc` | store function ID, remaining parameter list, recursive captures and exact generation/program/function authority; restore validates that authority |
| `Structured` | reject `UnrebindableStructuredFunction` before bytes are exposed; current `Arc<RuntimePlan>` cannot be rebound by an accepted restore authority |

This is an exhaustive live match, not a lossy callable/captures summary.

## Need handle

`RuntimeNeedHandleSnapshotV1` stores complete correlation, producer, outcome and the same closed state enum. Reusable stores a complete `TaskSpecSnapshotV1`; AcceptedLaunch stores no reusable spec and must resolve the exact committed journal row on restore.

## Complete checked-type projection

`RuntimeCheckedTypeProjectionV1` exactly mirrors the current checked type algebra:

`Never`, `Unit`, `Bool`, `Signed(RuntimeSignedIntWidth)`, `Unsigned(RuntimeUnsignedIntWidth)`, `F32`, `F64`, `String`, `Char`, `Duration`, `Progress`, `EntityReference`, `Bytes`, `Sequence(Box<RuntimeCheckedTypeProjectionV1>)`, `Tuple(Box<[RuntimeCheckedTypeProjectionV1]>)`, `Choice(Box<[RuntimeCheckedTypeProjectionV1]>)`, `Nominal { nominal, semantic_identity, layout }`, `Opaque { owner }`, `Variant { nominal, semantic_identity, cases }`, `Result { ok, error }`, `Option(Box<RuntimeCheckedTypeProjectionV1>)`, `Agent(RuntimeAgentOperationalType)`.

`RuntimeAgentValueProjectionV1` exactly mirrors the current concrete Agent value algebra accepted by the snapshot owner:

`ActionTarget`, `CaptureTarget`, `DebugStatePath`, `ObservationFieldPath`, `Probe`, `Diagnostics`, `Predicate`, `ViewportPoint`.

Rows for Agent protocol records/variants that lack a complete accepted Prelude nominal/case/field map do not reuse this enum; their ownership classification rejects `MissingRuntimeSnapshotOwner`.

## Snapshot work and first error

Construction uses a private staging sink. Depth, node, collection length and byte limits are checked before exposing output. Restore validates in wire order and returns the first error; no partial live graph is published.
