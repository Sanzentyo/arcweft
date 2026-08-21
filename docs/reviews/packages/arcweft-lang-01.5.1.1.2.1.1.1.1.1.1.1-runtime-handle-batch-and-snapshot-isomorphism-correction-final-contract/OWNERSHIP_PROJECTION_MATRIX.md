# Ownership carrier and projection matrix

## Rules

- The classifier is one exhaustive match over all 85 current `TypeKind` variants.
- Signed integers use `RuntimeValue::Int` and snapshot `Int`; unsigned integers   use `UInt` and snapshot `UInt`. `IntOrUInt` is forbidden.
- Every success names one outer live carrier, one snapshot carrier, one exact   projection constructor and one canonical identity owner.
- `Predicate` is a TypeKind leaf. Recursive predicate operands belong to the   value codec, not TypeKind recursion.
- `Shared` rejects `MissingRuntimeSnapshotOwner` before child recursion.
- A Cut-2 row that needs the Cut-5 Need snapshot owner remains private until Cut 5.
- Exact nominal/case/field maps are catalog joins, never source strings. A missing   current map rejects.

## Exhaustive matrix

| TypeKind | Disposition | Runtime projection | Live carrier | Snapshot carrier | Recursion | Rejection/notes | Publication |
|---|---|---|---|---|---|---|---|
| `Bool` | `Copy` | `RuntimeCheckedTypeProjectionV1::Bool` | `RuntimeValue::Bool` | `AwbcRuntimeValueSnapshot::Bool` | none | — | Cut 2 / `public_at_cut` |
| `I8` | `Copy` | `RuntimeCheckedTypeProjectionV1::Signed(RuntimeSignedIntWidth::I8)` | `RuntimeValue::Int(RuntimeInt::I8)` | `AwbcRuntimeValueSnapshot::Int` | none | — | Cut 2 / `public_at_cut` |
| `I16` | `Copy` | `RuntimeCheckedTypeProjectionV1::Signed(RuntimeSignedIntWidth::I16)` | `RuntimeValue::Int(RuntimeInt::I16)` | `AwbcRuntimeValueSnapshot::Int` | none | — | Cut 2 / `public_at_cut` |
| `I32` | `Copy` | `RuntimeCheckedTypeProjectionV1::Signed(RuntimeSignedIntWidth::I32)` | `RuntimeValue::Int(RuntimeInt::I32)` | `AwbcRuntimeValueSnapshot::Int` | none | — | Cut 2 / `public_at_cut` |
| `I64` | `Copy` | `RuntimeCheckedTypeProjectionV1::Signed(RuntimeSignedIntWidth::I64)` | `RuntimeValue::Int(RuntimeInt::I64)` | `AwbcRuntimeValueSnapshot::Int` | none | — | Cut 2 / `public_at_cut` |
| `I128` | `Copy` | `RuntimeCheckedTypeProjectionV1::Signed(RuntimeSignedIntWidth::I128)` | `RuntimeValue::Int(RuntimeInt::I128)` | `AwbcRuntimeValueSnapshot::Int` | none | — | Cut 2 / `public_at_cut` |
| `ISize` | `Copy` | `RuntimeCheckedTypeProjectionV1::Signed(RuntimeSignedIntWidth::ISize)` | `RuntimeValue::Int(RuntimeInt::ISize)` | `AwbcRuntimeValueSnapshot::Int` | none | — | Cut 2 / `public_at_cut` |
| `U8` | `Copy` | `RuntimeCheckedTypeProjectionV1::Unsigned(RuntimeUnsignedIntWidth::U8)` | `RuntimeValue::UInt(RuntimeUInt::U8)` | `AwbcRuntimeValueSnapshot::UInt` | none | — | Cut 2 / `public_at_cut` |
| `U16` | `Copy` | `RuntimeCheckedTypeProjectionV1::Unsigned(RuntimeUnsignedIntWidth::U16)` | `RuntimeValue::UInt(RuntimeUInt::U16)` | `AwbcRuntimeValueSnapshot::UInt` | none | — | Cut 2 / `public_at_cut` |
| `U32` | `Copy` | `RuntimeCheckedTypeProjectionV1::Unsigned(RuntimeUnsignedIntWidth::U32)` | `RuntimeValue::UInt(RuntimeUInt::U32)` | `AwbcRuntimeValueSnapshot::UInt` | none | — | Cut 2 / `public_at_cut` |
| `U64` | `Copy` | `RuntimeCheckedTypeProjectionV1::Unsigned(RuntimeUnsignedIntWidth::U64)` | `RuntimeValue::UInt(RuntimeUInt::U64)` | `AwbcRuntimeValueSnapshot::UInt` | none | — | Cut 2 / `public_at_cut` |
| `U128` | `Copy` | `RuntimeCheckedTypeProjectionV1::Unsigned(RuntimeUnsignedIntWidth::U128)` | `RuntimeValue::UInt(RuntimeUInt::U128)` | `AwbcRuntimeValueSnapshot::UInt` | none | — | Cut 2 / `public_at_cut` |
| `USize` | `Copy` | `RuntimeCheckedTypeProjectionV1::Unsigned(RuntimeUnsignedIntWidth::USize)` | `RuntimeValue::UInt(RuntimeUInt::USize)` | `AwbcRuntimeValueSnapshot::UInt` | none | — | Cut 2 / `public_at_cut` |
| `F32` | `Copy` | `RuntimeCheckedTypeProjectionV1::F32` | `RuntimeValue::F32` | `AwbcRuntimeValueSnapshot::F32` | none | — | Cut 2 / `public_at_cut` |
| `F64` | `Copy` | `RuntimeCheckedTypeProjectionV1::F64` | `RuntimeValue::F64` | `AwbcRuntimeValueSnapshot::F64` | none | — | Cut 2 / `public_at_cut` |
| `String` | `SnapshotClone` | `RuntimeCheckedTypeProjectionV1::String` | `RuntimeValue::String` | `AwbcRuntimeValueSnapshot::String` | none | — | Cut 2 / `public_at_cut` |
| `Char` | `Copy` | `RuntimeCheckedTypeProjectionV1::Char` | `RuntimeValue::Char` | `AwbcRuntimeValueSnapshot::Char` | none | — | Cut 2 / `public_at_cut` |
| `Bytes` | `SnapshotClone` | `RuntimeCheckedTypeProjectionV1::Bytes` | `RuntimeValue::Seq(RuntimeSeq::Values of exact UInt::U8)` | `AwbcRuntimeValueSnapshot::Seq(Values)` | source-order U8 elements | — | Cut 2 / `public_at_cut` |
| `TextCluster` | `SnapshotClone` | `RuntimeTextProjectionV1::TextClusterUtf8` | `RuntimeValue::String` | `AwbcRuntimeValueSnapshot::String` | none | constructor validates the accepted TextCluster nominal before discarding display spelling | Cut 2 / `public_at_cut` |
| `Duration` | `Copy` | `RuntimeCheckedTypeProjectionV1::Duration` | `RuntimeValue::Duration` | `AwbcRuntimeValueSnapshot::Duration` | none | — | Cut 2 / `public_at_cut` |
| `Progress` | `SnapshotClone` | `RuntimeCheckedTypeProjectionV1::Progress` | `RuntimeValue::Progress` | `AwbcRuntimeValueSnapshot::Progress` | none | — | Cut 2 / `public_at_cut` |
| `StageApi` | `Reject` | — | — | — | none | `AffineValue` | Cut 2 / `public_at_cut` |
| `LineContext` | `Reject` | — | — | — | none | `AffineValue` | Cut 2 / `public_at_cut` |
| `StageActorHandle` | `Reject` | — | — | — | none | `AffineValue` | Cut 2 / `public_at_cut` |
| `CueHandle` | `Reject` | — | — | — | none | `AffineValue` | Cut 2 / `public_at_cut` |
| `VoiceHandle` | `Reject` | — | — | — | none | `AffineValue` | Cut 2 / `public_at_cut` |
| `Range` | `Reject` | `RuntimeOwnershipTypeProjectionV1::Range` | — | — | none | `MissingCanonicalIdentity`; save carrier exists, but producer-retained canonical identity is not admitted | Cut 2 / `public_at_cut` |
| `IteratorState` | `Reject` | `RuntimeOwnershipTypeProjectionV1::IteratorState` | — | — | none | `FrameLocalValue` | Cut 2 / `public_at_cut` |
| `DisplayText` | `SnapshotClone` | `RuntimeTextProjectionV1::DisplayTextUtf8` | `RuntimeValue::String` | `AwbcRuntimeValueSnapshot::String` | none | constructor validates accepted DisplayText identity | Cut 2 / `public_at_cut` |
| `DebugStatePath` | `SnapshotClone` | `RuntimeAgentValueProjectionV1::DebugStatePath` | `RuntimeValue::Agent(RuntimeAgentValue::DebugStatePath)` | `AwbcRuntimeValueSnapshot::Agent(DebugStatePath)` | none | — | Cut 2 / `public_at_cut` |
| `ObservationFieldPath` | `SnapshotClone` | `RuntimeAgentValueProjectionV1::ObservationFieldPath` | `RuntimeValue::Agent(RuntimeAgentValue::ObservationFieldPath)` | `AwbcRuntimeValueSnapshot::Agent(ObservationFieldPath)` | none | — | Cut 2 / `public_at_cut` |
| `Ref` | `SnapshotClone` | `RuntimeCheckedTypeProjectionV1::EntityReference` | `RuntimeValue::EntityRef` | `AwbcRuntimeValueSnapshot::EntityRef` | no owned child; payload type is lookup contract | — | Cut 2 / `public_at_cut` |
| `Probe` | `SnapshotClone` | `RuntimeAgentValueProjectionV1::Probe` | `RuntimeValue::Agent(RuntimeAgentValue::Probe)` | `AwbcRuntimeValueSnapshot::Agent(Probe)` | validate result type certificate; no owned RuntimeValue child | — | Cut 2 / `public_at_cut` |
| `Predicate` | `SnapshotClone` | `RuntimeAgentValueProjectionV1::Predicate` | `RuntimeValue::Agent(RuntimeAgentValue::Predicate)` | `AwbcRuntimeValueSnapshot::Agent(Predicate)` | TypeKind leaf; predicate-owned RuntimeValue operands recurse only in value codec | — | Cut 2 / `public_at_cut` |
| `Observation` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `ObservedObject` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `AgentBBox` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `ActionName` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `ActionTarget` | `SnapshotClone` | `RuntimeAgentValueProjectionV1::ActionTarget` | `RuntimeValue::Agent(RuntimeAgentValue::ActionTarget)` | `AwbcRuntimeValueSnapshot::Agent(ActionTarget)` | none | — | Cut 2 / `public_at_cut` |
| `ActionResult` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `AgentValue` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; the umbrella type has no single closed carrier projection at this cut | Cut 2 / `public_at_cut` |
| `DataFormat` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `DataShape` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `AgentEntityMetadata` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `AgentSourceAnchor` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `AgentProjectGraphNeighborhood` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `AgentProjectGraphSymbol` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `AgentProjectGraphEdge` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `CaptureTarget` | `SnapshotClone` | `RuntimeAgentValueProjectionV1::CaptureTarget` | `RuntimeValue::Agent(RuntimeAgentValue::CaptureTarget)` | `AwbcRuntimeValueSnapshot::Agent(CaptureTarget)` | none | — | Cut 2 / `public_at_cut` |
| `CaptureRef` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `AgentResource` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `AgentResourceBody` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `RagContextPack` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete accepted Agent Prelude nominal/case/field map is current at the production cut; this package does not fabricate one | Cut 2 / `public_at_cut` |
| `AgentBuiltin` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner` | Cut 2 / `public_at_cut` |
| `Vec` | `SnapshotClone` | `RuntimeSequenceProjectionV1::Vec` | `RuntimeValue::Seq` | `AwbcRuntimeValueSnapshot::Seq` | item in semantic/source order | — | Cut 2 / `public_at_cut` |
| `Array` | `SnapshotClone` | `RuntimeSequenceProjectionV1::Array` | `RuntimeValue::Seq` | `AwbcRuntimeValueSnapshot::Seq` | item in semantic/source order | — | Cut 2 / `public_at_cut` |
| `Slice` | `SnapshotClone` | `RuntimeSequenceProjectionV1::Slice` | `RuntimeValue::Seq` | `AwbcRuntimeValueSnapshot::Seq` | item in semantic/source order | — | Cut 2 / `public_at_cut` |
| `Seq` | `SnapshotClone` | `RuntimeSequenceProjectionV1::Seq` | `RuntimeValue::Seq` | `AwbcRuntimeValueSnapshot::Seq` | item in semantic/source order | — | Cut 2 / `public_at_cut` |
| `Map` | `Reject` | `RuntimeOwnershipTypeProjectionV1::Map` | — | — | none | `MissingRuntimeSnapshotOwner` | Cut 2 / `public_at_cut` |
| `BorrowRef` | `Reject` | — | — | — | none | `BorrowedValue` | Cut 2 / `public_at_cut` |
| `Need` | `SnapshotClone` | `RuntimeOwnershipTypeProjectionV1::Need` | `RuntimeValue::NeedHandle` | `AwbcRuntimeValueSnapshot::NeedHandle` | payload type is a contract; no embedded value | Cut-2 evidence remains private and is published atomically at Cut 5 | Cut 5 / `private_until_cut_5` |
| `Stream` | `Reject` | `RuntimeOwnershipTypeProjectionV1::Stream` | — | — | none | `StreamValue` | Cut 2 / `public_at_cut` |
| `Result` | `SnapshotClone` | `RuntimeCheckedTypeProjectionV1::Result` | `RuntimeValue::Variant(owner=Result, ordinal 0|1)` | `AwbcRuntimeValueSnapshot::Variant(owner=Result)` | selected payload only | — | Cut 2 / `public_at_cut` |
| `Option` | `SnapshotClone` | `RuntimeCheckedTypeProjectionV1::Option` | `RuntimeValue::Variant(owner=Option, ordinal 0|1)` | `AwbcRuntimeValueSnapshot::Variant(owner=Option)` | Some payload only | — | Cut 2 / `public_at_cut` |
| `Handle` | `Reject` | — | — | — | none | `AffineValue` | Cut 2 / `public_at_cut` |
| `ThreadHandle` | `Reject` | `RuntimeOwnershipTypeProjectionV1::ThreadHandle` | — | — | none | `AffineValue` | Cut 2 / `public_at_cut` |
| `Shared` | `Reject` | `RuntimeOwnershipTypeProjectionV1::Shared` | — | — | reject before child recursion | `MissingRuntimeSnapshotOwner` | Cut 2 / `public_at_cut` |
| `Function` | `Reject` | `RuntimeOwnershipTypeProjectionV1::Function` | — | — | none | `FunctionValueRequiresCertificate`; AWBC function snapshots exist, but ownership classification requires a separate accepted capture/rebind certificate | Cut 2 / `public_at_cut` |
| `GenericParam` | `Reject` | — | — | — | none | `UnresolvedType` | Cut 2 / `public_at_cut` |
| `ProjectNominal` | `SnapshotClone` | `RuntimeNominalRecordProjectionV1` | `RuntimeValue::NominalRecord` | `AwbcRuntimeValueSnapshot::NominalRecord` | accepted field order | success only after exact accepted nominal/layout/field catalog join | Cut 2 / `public_at_cut` |
| `AcceptedNominal` | `Delegate` | `RuntimeAcceptedNominalProjectionV1` | `exact|opaque|character delegate` | `delegate-selected exact snapshot` | delegate-owned | constructor is exhaustive; missing exact owner rejects | Cut 2 / `public_at_cut` |
| `OpenNominal` | `Reject` | — | — | — | none | `UnresolvedType` | Cut 2 / `public_at_cut` |
| `Error` | `Reject` | — | — | — | none | `UnresolvedType` | Cut 2 / `public_at_cut` |
| `Projection` | `Reject` | — | — | — | none | `UnresolvedType` | Cut 2 / `public_at_cut` |
| `CharacterPatch` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete current accepted nominal/layout/field map; source spelling is not used as identity | Cut 2 / `public_at_cut` |
| `FocusPatch` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete current accepted nominal/layout/field map; source spelling is not used as identity | Cut 2 / `public_at_cut` |
| `CharacterDialogue` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete current accepted nominal/layout/field map; source spelling is not used as identity | Cut 2 / `public_at_cut` |
| `DialogueLine` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete current accepted nominal/layout/field map; source spelling is not used as identity | Cut 2 / `public_at_cut` |
| `ViewValue` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete current accepted nominal/layout/field map; source spelling is not used as identity | Cut 2 / `public_at_cut` |
| `CharacterNominal` | `Reject` | — | — | — | none | `MissingRuntimeSnapshotOwner`; no complete current accepted nominal/layout/field map; source spelling is not used as identity | Cut 2 / `public_at_cut` |
| `Named` | `Reject` | — | — | — | none | `UnresolvedType`; aliases must normalize before ownership classification | Cut 2 / `public_at_cut` |
| `Tuple` | `SnapshotClone` | `RuntimeCheckedTypeProjectionV1::Tuple` | `RuntimeValue::Tuple` | `AwbcRuntimeValueSnapshot::Tuple` | elements in source order | — | Cut 2 / `public_at_cut` |
| `Choice` | `Reject` | `RuntimeCheckedTypeProjectionV1::Choice` | — | — | none | `MissingRuntimeSnapshotOwner`; direct-alternative and tagged-variant carriers are not interchangeable; no ambiguous success | Cut 2 / `public_at_cut` |
| `Unit` | `Copy` | `RuntimeCheckedTypeProjectionV1::Unit` | `RuntimeValue::Unit` | `AwbcRuntimeValueSnapshot::Unit` | none | — | Cut 2 / `public_at_cut` |
| `Never` | `Copy` | `RuntimeCheckedTypeProjectionV1::Never` | `uninhabited` | `uninhabited` | none | — | Cut 2 / `public_at_cut` |

## Closed nominal decisions

`TextCluster` and `DisplayText` succeed only through `RuntimeTextProjectionV1`, which validates the accepted nominal/semantic identity and then constructs the single String carrier. No source spelling survives as identity.

Current production has concrete snapshot owners for selected `RuntimeAgentValue` variants, so DebugStatePath, ObservationFieldPath, Probe, Predicate, ActionTarget and CaptureTarget succeed. Agent protocol records/variants without a complete accepted Prelude nominal/case/field map reject `MissingRuntimeSnapshotOwner`.

CharacterPatch, FocusPatch, CharacterDialogue, DialogueLine, ViewValue and CharacterNominal also reject at this cut. This is deliberate: adding a name-based or generic record carrier would violate the request. A later contract may introduce a complete catalog and change those rows; this package does not speculate.

## Result, Option, Tuple, Choice

- Result is exactly Variant owner `Result`, ordinal 0 for Ok and 1 for Err.
- Option is exactly Variant owner `Option`, ordinal 0 for None and 1 for Some   (the implementation must retain the existing accepted ordinal convention).
- Tuple is exactly `RuntimeValue::Tuple`.
- Choice rejects because the current runtime type permits direct alternative   values while a tagged Variant would be a different representation. The package   does not call these carriers interchangeable.

## Cross-artifact invariant

`machine/ownership_matrix.json`, `schemas/final_contract.rs`, `SNAPSHOT_ISOMORPHISM.md`, lowering constructors and the test matrix use the same carrier names. The validator checks all 85 rows, integer separation, projection definitions, Predicate/Shared rules and absence of ambiguous carriers.
