# Total ownership and persistence admission

## Legitimate owner and inputs

Ownership is not an inherent query on `RegisteredSemanticWorld` alone. The sole
owner is a crate-private `CheckedOwnershipContext` constructed by final analysis
from the exact objects already available at the semantic publication boundary:

```rust
pub(crate) struct CheckedOwnershipContext<'a> {
    symbols: &'a ProjectSymbolTable,
    world: &'a RegisteredSemanticWorld,
    resources: &'a ResourceTypeRegistry,
    limits: CheckedOwnershipLimits,
}
```

It performs no source/string lookup and builds no copied nominal, resource, or
opaque side table. `FinalSemanticCatalogs::production` verifies
`ResourceTypeRegistry::verify_integrity()` and retains the existing
`ResourceTypeRegistryDigest`.

```rust
pub enum CheckedOwnershipDisposition {
    Copy,
    SnapshotClone,
}

pub enum CheckedOwnershipRejection {
    Borrowed,
    MutableBorrow,
    AffineHandle,
    RuntimeHandle,
    ThreadHandle,
    StreamHandle,
    FrameLocalCapability,
    FrameLocalIterator,
    FrameLocalOperation,
    CallableNeedsValueEvidence,
    NonSnapshotOpaque,
    MissingOpaqueEvidence,
    MissingNominalEvidence,
    MissingResourceEvidence,
    MissingRegisteredEvidence,
    OpenNominal,
    PoisonedType,
    UnresolvedGeneric,
    UnresolvedProjection,
    UnresolvedArrayLength,
    RecursiveValueCycle,
    DepthLimit,
    NodeLimit,
    NominalExpansionLimit,
    ProducerArgumentRejected,
}
```

A rejection is returned as an error; it is never a successful disposition
stored as `Rejected(...)` in a generic Match fact.

## Exhaustive TypeKind mapping

| Current TypeKind | Disposition rule | Existing/extended owner | Closed rejection |
|---|---|---|---|
| `Bool` | `Copy` | primitive value | none |
| `I8` | `Copy` | primitive value | none |
| `I16` | `Copy` | primitive value | none |
| `I32` | `Copy` | primitive value | none |
| `I64` | `Copy` | primitive value | none |
| `I128` | `Copy` | primitive value | none |
| `ISize` | `Copy` | semantic scalar; never encoded as platform usize | none |
| `U8` | `Copy` | primitive value | none |
| `U16` | `Copy` | primitive value | none |
| `U32` | `Copy` | primitive value | none |
| `U64` | `Copy` | primitive value | none |
| `U128` | `Copy` | primitive value | none |
| `USize` | `Copy` | semantic scalar; never encoded as platform usize | none |
| `F32` | `Copy` | raw IEEE-754 bits | none |
| `F64` | `Copy` | raw IEEE-754 bits | none |
| `String` | `SnapshotClone` | RuntimeValue::String | none |
| `Char` | `Copy` | Unicode scalar | none |
| `Bytes` | `SnapshotClone` | RuntimeValue::Bytes | none |
| `TextCluster` | `SnapshotClone` | text-model immutable value | none |
| `Duration` | `Copy` | LogicalDuration | none |
| `Progress` | `SnapshotClone` | standard record ratio + optional label | none |
| `StageApi` | `Rejected` | compile-time capability | FrameLocalCapability |
| `LineContext` | `Rejected` | line activation capability | FrameLocalCapability |
| `StageActorHandle` | `Rejected` | RuntimeOpaqueValueClass::AffineHandle(StageActor) | AffineHandle |
| `CueHandle` | `Rejected` | RuntimeOpaqueValueClass::AffineHandle(Cue) | AffineHandle |
| `VoiceHandle` | `Rejected` | RuntimeOpaqueValueClass::AffineHandle(Voice) | AffineHandle |
| `Range` | `RecursiveJoin` | range bounds through element TypeKind | first child rejection |
| `IteratorState` | `Rejected` | frame-local iterator cursor | FrameLocalIterator |
| `DisplayText` | `SnapshotClone` | immutable display text | none |
| `DebugStatePath` | `SnapshotClone` | Agent value owner | none |
| `ObservationFieldPath` | `SnapshotClone` | Agent value owner | none |
| `Ref` | `Copy` | semantic entity identity; payload parameter is metadata, not contained value | none |
| `Probe` | `RecursiveJoin` | Agent probe wrapper | first child rejection |
| `Predicate` | `SnapshotClone` | Agent immutable DTO | none |
| `Observation` | `SnapshotClone` | Agent immutable DTO | none |
| `ObservedObject` | `SnapshotClone` | Agent immutable DTO | none |
| `AgentBBox` | `SnapshotClone` | Agent immutable DTO | none |
| `ActionName` | `SnapshotClone` | Agent immutable DTO | none |
| `ActionTarget` | `SnapshotClone` | Agent immutable DTO | none |
| `ActionResult` | `SnapshotClone` | Agent immutable DTO | none |
| `AgentValue` | `SnapshotClone` | Agent immutable DTO | none |
| `DataFormat` | `SnapshotClone` | Agent immutable DTO | none |
| `DataShape` | `SnapshotClone` | Agent immutable DTO | none |
| `AgentEntityMetadata` | `SnapshotClone` | Agent immutable DTO | none |
| `AgentSourceAnchor` | `SnapshotClone` | Agent immutable DTO | none |
| `AgentProjectGraphNeighborhood` | `SnapshotClone` | Agent immutable DTO | none |
| `AgentProjectGraphSymbol` | `SnapshotClone` | Agent immutable DTO | none |
| `AgentProjectGraphEdge` | `SnapshotClone` | Agent immutable DTO | none |
| `CaptureTarget` | `SnapshotClone` | Agent immutable DTO | none |
| `CaptureRef` | `SnapshotClone` | Agent immutable reference value, not language borrow | none |
| `AgentResource` | `RegistryRecursive` | ResourceTypeRegistry descriptor/schema | MissingResourceEvidence |
| `AgentResourceBody` | `RegistryRecursive` | ResourceTypeRegistry descriptor/schema | MissingResourceEvidence |
| `RagContextPack` | `SnapshotClone` | Agent immutable DTO | none |
| `AgentBuiltin` | `Rejected` | runtime capability/procedure owner | FrameLocalCapability |
| `Vec` | `RecursiveJoin` | sequence element | first child rejection |
| `Array` | `RecursiveJoin` | constant length + element; generic/error/inferred length rejects | UnresolvedArrayLength |
| `Slice` | `RecursiveJoin` | owning runtime sequence projection | first child rejection |
| `Seq` | `RecursiveJoin` | owning runtime sequence projection | first child rejection |
| `Map` | `RecursiveJoin` | key then value in canonical order | first child rejection |
| `BorrowRef` | `Rejected` | BorrowKind + lifetime | BorrowedOrMutableBorrow |
| `Need` | `Copy` | immutable typed RuntimeNeedHandle identity | producer arguments certified separately |
| `Stream` | `Rejected` | affine live producer/consumer state | StreamHandle |
| `Result` | `RecursiveJoin` | ok then error | first child rejection |
| `Option` | `RecursiveJoin` | item | first child rejection |
| `Handle` | `Rejected` | lifetime/state/must_drop runtime handle | RuntimeHandle |
| `ThreadHandle` | `Rejected` | live task/thread owner | ThreadHandle |
| `Shared` | `RecursiveSnapshot` | shared snapshot owner; breaks nominal recursion | first child rejection |
| `Function` | `Rejected` | type alone lacks exact capture/value evidence | CallableNeedsValueEvidence |
| `GenericParam` | `Rejected` | unsubstituted generic | UnresolvedGeneric |
| `ProjectNominal` | `ProjectRecursive` | ProjectSymbolTable declaration fields/cases with substitutions | MissingNominalEvidence |
| `AcceptedNominal` | `CatalogRecursive` | AcceptedNominalRecord semantics + opaque value class/persistence | MissingOpaqueEvidence |
| `OpenNominal` | `Rejected` | open rule has no closed layout/persistence fact | OpenNominal |
| `Error` | `Rejected` | poisoned semantic type | PoisonedType |
| `Projection` | `Rejected` | unresolved associated projection | UnresolvedProjection |
| `CharacterPatch` | `SnapshotClone` | character patch value owner | none |
| `FocusPatch` | `SnapshotClone` | focus patch value owner | none |
| `CharacterDialogue` | `SnapshotClone` | dialogue immutable presentation value | none |
| `DialogueLine` | `Rejected` | non-escaping suspension operation | FrameLocalOperation |
| `ViewValue` | `SnapshotClone` | checked View persisted value owner | none |
| `CharacterNominal` | `SnapshotClone` | manifest-backed closed character value | none |
| `Named` | `Rejected` | internal/host value without exact accepted owner | MissingRegisteredEvidence |
| `Tuple` | `RecursiveJoin` | source-order elements | first child rejection |
| `Choice` | `RecursiveJoin` | source-order alternatives | first child rejection |
| `Unit` | `Copy` | unit | none |
| `Never` | `Copy` | empty value domain | none |

`RecursiveJoin` means canonical depth-first classification: all children Copy
produces Copy; otherwise all admissible with any SnapshotClone produces
SnapshotClone; the first rejection wins. `RecursiveSnapshot` always produces
SnapshotClone after child admission. Project and catalog recursion use exact
accepted declarations and substitutions, not names.

## Accepted nominal and opaque evidence

The original Arcweft-owned `AcceptedNominalSemantics::Opaque` variant is
extended in place rather than wrapped by a helper or extension trait:

```rust
Opaque {
    producer: RuntimeOpaqueTypeProducerId,
    value_class: RuntimeOpaqueValueClass,
    persistence: RuntimeOpaquePersistence,
}
```

`AffineHandle` rejects. `Plain + SnapshotOnly` is SnapshotClone.
`Plain + ConstantAndSnapshot` recursively admits arguments and is
SnapshotClone; primitive Copy is not guessed for opaque values. Missing or
stale evidence rejects. Exact/Character semantics use their existing owners.
Open nominal rules have no persistence layout and reject.

## Project nominal, resource, and cycles

Project structs and enum payloads are expanded through `ProjectSymbolTable` in
accepted declaration order after exact generic substitution. A direct by-value
cycle rejects `RecursiveValueCycle`; the error path names the first repeated
accepted nominal identity. `Shared`, Need handle, and opaque identity stop
structural expansion according to their owner rules. Missing fields/cases or
stale layout facts reject before a disposition exists.

AgentResource and AgentResourceBody resolve the exact descriptor/schema through
`ResourceTypeRegistry`; nested value schemas are classified in canonical field
or variant order. Registry integrity or missing schema/type evidence rejects.
The existing registry digest is retained in final semantic analysis and the
checked Match digest.

## Need and callable split

`Need<T>` classifies as Copy because the runtime handle contains immutable fixed
identities, producer contract, payload type digest, and immutable verified
argument ownership; duplicating observers does not duplicate affine resources.
This type-level result never certifies construction.

Every `MakeNeedHandle`/View/line/host producer requires a separate
`CheckedNeedProducerAdmission` containing the exact contract, payload type,
argument/capture dispositions, and runtime-value digest schema. Missing or
rejected arguments block producer publication. Runtime never guesses.

A Function TypeKind rejects because type alone has no capture facts. A
value-level classifier may issue a Copy certificate for a capture-free
registered callable or SnapshotClone for a closure whose exact captures are all
admitted; that certificate is bound to the checked value, not inferred from the
Function type row.

## Limits and precedence

Limits are depth 64, nodes 65,536, and nominal expansions 4,096. Counters are
charged before descent. Canonical first failure order is current node owner,
then children in declared/source order, then cycle, then depth/node/expansion
limit at the operation that would exceed it. No partial local, Match, producer,
or View catalog row is published.
