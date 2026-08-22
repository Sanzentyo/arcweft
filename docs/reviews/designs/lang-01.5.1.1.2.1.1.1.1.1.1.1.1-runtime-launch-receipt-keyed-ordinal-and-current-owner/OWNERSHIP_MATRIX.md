# Producer-argument ownership and carrier matrix

## Authority and result

This matrix is the semantic-type admission used before a `TypeKind` value can
participate in canonical Need producer arguments. It does not replace current
`RuntimeValue::ownership()`: that existing core method remains the sole
transitive live-value graph authority.

`RuntimeProducerArgumentClassifier<'a>` belongs in `arcweft-lang-sema` beside
`TypeKind` and borrows `FinalSemanticAnalysis` plus the accepted nominal
registries. It returns `RuntimeProducerArgumentAdmission::{Copy,
SnapshotClone}(RuntimeOwnershipProjection)` or one typed rejection. Success is
followed by exact carrier/value validation and the existing canonical
`RuntimeValueDigest` visitor. No source name is reparsed.

## Exhaustive outer `TypeKind` matrix

The following table names all 85 current outer variants. “Recursive” means
every owned child is classified in declaration order and joined; the first
child rejection wins. “Catalog exact” means the current accepted semantic
catalog must yield the named record/variant/opaque projection; absence rejects
`MissingRuntimeSnapshotOwner`.

| `TypeKind` | Final disposition and exact carrier |
|---|---|
| `Bool` | Copy — `RuntimeValue::Bool` / snapshot `Bool` |
| `I8`, `I16`, `I32`, `I64`, `I128`, `ISize` | Copy — exact `RuntimeValue::Int` width / snapshot `Int` |
| `U8`, `U16`, `U32`, `U64`, `U128`, `USize` | Copy — exact `RuntimeValue::UInt` width / snapshot `UInt` |
| `F32`, `F64` | Copy — corresponding float carrier, exact IEEE bits in canonical identity |
| `String` | SnapshotClone — `String` |
| `Char` | Copy — `Char` |
| `Bytes` | SnapshotClone — canonical `RuntimeValue::Seq(DenseSeq::Bytes)`; `Values(UInt::U8)` is normalized to Dense before digest/snapshot |
| `TextCluster` | SnapshotClone — accepted TextCluster identity certificate plus `String`; missing certificate rejects |
| `Duration` | Copy — `Duration` |
| `Progress` | SnapshotClone — `Progress` |
| `StageApi` | Reject `AffineValue` for both exact-character values |
| `LineContext` | Reject `AffineValue` |
| `StageActorHandle` | Reject `AffineValue`; nested `Exact` and `Any` are both explicit arms |
| `CueHandle`, `VoiceHandle` | Reject `AffineValue` |
| `Range` | Reject `MissingCanonicalIdentity` before child recursion |
| `IteratorState` | Reject `FrameLocalValue`; all six nested kinds are explicit |
| `DisplayText` | SnapshotClone — accepted DisplayText identity certificate plus `String`; missing certificate rejects |
| `DebugStatePath` | SnapshotClone — `Agent(DebugStatePath)` |
| `ObservationFieldPath` | SnapshotClone — `Agent(ObservationFieldPath)` |
| `Ref` | SnapshotClone — `EntityRef`; `EntityType.kind/value` is lookup contract, not an owned RuntimeValue child |
| `Probe` | SnapshotClone — `Agent(Probe)` after recursive result-type certificate validation |
| `Predicate` | SnapshotClone — `Agent(Predicate)`; TypeKind leaf, recursive value operands belong to the value codec |
| `Observation`, `ObservedObject`, `AgentBBox`, `ActionName` | Reject `MissingRuntimeSnapshotOwner` individually |
| `ActionTarget` | SnapshotClone — `Agent(ActionTarget)` |
| `ActionResult`, `AgentValue`, `DataFormat`, `DataShape` | Reject `MissingRuntimeSnapshotOwner` individually |
| `AgentEntityMetadata`, `AgentSourceAnchor`, `AgentProjectGraphNeighborhood`, `AgentProjectGraphSymbol`, `AgentProjectGraphEdge` | Reject `MissingRuntimeSnapshotOwner` individually |
| `CaptureTarget` | SnapshotClone — `Agent(CaptureTarget)` |
| `CaptureRef`, `AgentResource`, `AgentResourceBody`, `RagContextPack` | Reject `MissingRuntimeSnapshotOwner` individually |
| `AgentBuiltin` | Nested exhaustive table below; no outer blanket arm |
| `Vec` | SnapshotClone — `Seq`, recursive item |
| `Array` | Nested exhaustive length table below; Const uses `Seq` and recursive item |
| `Slice`, `Seq` | SnapshotClone — `Seq`, recursive item |
| `Map` | Reject `MissingRuntimeSnapshotOwner`; all three `MapKind` arms are explicit before child recursion |
| `BorrowRef` | Reject `BorrowedValue`; every `BorrowKind`/lifetime form is rejected before inner recursion |
| `Need` | SnapshotClone — `RuntimeValue::NeedHandle` / snapshot `NeedHandle`; payload type is contract evidence, not embedded value; private until atomic Cut 5 |
| `Stream` | Reject `StreamValue` before item/error recursion |
| `Result` | SnapshotClone — `Variant(Result)`, `Ok=0`/`Err=1`; selected payload recursively classified |
| `Option` | SnapshotClone — `Variant(Option)`, `Some=0`/`None=1`; Some payload recursively classified |
| `Handle` | Nested exhaustive state table below |
| `ThreadHandle` | Reject `AffineValue` before child recursion |
| `Shared` | Reject `MissingRuntimeSnapshotOwner` before child recursion |
| `Function` | Reject `FunctionValueRequiresCertificate` before parameter/result recursion |
| `GenericParam` | Reject `UnresolvedType` |
| `ProjectNominal` | Catalog exact: accepted struct → `NominalRecord`; accepted enum → `Variant`; missing/open declaration rejects |
| `AcceptedNominal` | Catalog exact: accepted record → `NominalRecord`; variant → `Variant`; exact opaque → `Opaque`; no generic fallback |
| `OpenNominal`, `Error`, `Projection` | Reject `UnresolvedType` individually |
| `CharacterPatch`, `FocusPatch`, `CharacterDialogue`, `DialogueLine`, `ViewValue`, `CharacterNominal` | Reject `MissingRuntimeSnapshotOwner` with explicit nested arms where applicable |
| `Named` | Reject `UnresolvedType`; aliases normalize before this boundary |
| `Tuple` | SnapshotClone — `Tuple`, recursive elements in source order |
| `Choice` | Reject `MissingRuntimeSnapshotOwner` before alternative recursion; direct-alternative and tagged-variant carriers are not interchangeable |
| `Unit` | Copy — `Unit` |
| `Never` | Copy — uninhabited certificate; no runtime value can be supplied |

## Required nested classifications

### `AgentBuiltinType`

| Case | Disposition |
|---|---|
| `ObservedObjectId` | Reject `MissingRuntimeSnapshotOwner` |
| `CaptureFormat` | Reject `MissingRuntimeSnapshotOwner` |
| `CaptureKind` | Reject `MissingRuntimeSnapshotOwner` |
| `Diagnostics` | SnapshotClone — `RuntimeValue::Agent(RuntimeAgentValue::Diagnostics)` / `AwbcRuntimeAgentSnapshot::Diagnostics` |
| `WaitError` | Reject `MissingRuntimeSnapshotOwner` |
| `ViewportPoint` | SnapshotClone — `RuntimeValue::Agent(RuntimeAgentValue::ViewportPoint { x, y })` / matching snapshot row |
| `PointerButton` | Reject `MissingRuntimeSnapshotOwner` |
| `RagError` | Reject `MissingRuntimeSnapshotOwner` |

### `ArrayLength`

| Case | Disposition |
|---|---|
| `Const(length)` | SnapshotClone with `RuntimeSequenceProjectionV1::Array { length: checked u64 }`; sequence length must equal it |
| `Generic(_)` | Reject `UnresolvedType` |
| `Error(_)` | Reject `UnresolvedType` |
| `Inferred` | Reject `UnresolvedType` |

### `IteratorStateKind`

`Range`, `Seq`, `Stream`, `Vec`, `Array`, and `Slice` each return
`FrameLocalValue`; there is no wildcard arm.

### `MapKind`

`Ordered`, `Sorted`, and `BTree` each return
`MissingRuntimeSnapshotOwner`; there is no wildcard arm.

### `HandleState`

| State | Disposition |
|---|---|
| `Live`, `Detached` | Reject `AffineValue` |
| `Dropped` | Reject `DeadHandle` |
| `MovedOut` | Reject `MovedValue` |

### `CharacterDialogueCharacterType`

`Exact(_)` and `Any` each reject `MissingRuntimeSnapshotOwner` until a complete
current nominal/layout/field carrier is accepted.

### `CharacterNominalType`

`Look`, `Part`, and `Variant` each reject
`MissingRuntimeSnapshotOwner`; their structural identities remain available
for a later exact catalog, never as source strings.

### `ProjectNominal` and `AcceptedNominal`

These are not blanket successes. The classifier queries the accepted nominal
declaration and produces exactly one of:

- `RuntimeNominalRecordProjectionV1` and `NominalRecord`;
- `RuntimeCheckedTypeProjectionV1::Variant` and `Variant`; or
- `RuntimeAcceptedNominalProjectionV1::ExactOpaque` and `Opaque`.

An absent row, open row, mismatched argument count, unresolved child, missing
layout/case/field identity, or producer mismatch rejects. A display name is
never consulted.

## Variant convention authority

The classifier, Match coverage, lowering, value construction, and snapshot
restore all call the existing `RuntimeCheckedType::variant_case`. They do not
keep a second ordinal map:

```text
Option: Some(0, payload), None(1, no payload)
Result: Ok(0, payload), Err(1, payload)
```

Any other ordinal, missing Some payload, present None payload, or owner mismatch
rejects before canonical bytes are exposed.
