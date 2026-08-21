# Ownership and persistence evidence

## 1. Mandatory opaque publication chain

The opaque evidence chain is one atomic semantic authority:

```text
AcceptedNominalInventoryInput
  { id, arity, runtime_producer, value_class, persistence, ... }
    |
    | registrar validation; no omitted-field overload and no default
    v
AcceptedNominalRecord::try_new_opaque
    |
    v
AcceptedNominalSemantics::Opaque
  { producer, value_class, persistence }
    |
    | canonical accepted nominal catalog transcript
    v
AcceptedNominalCatalogDigest
    |
    +--> runtime normalized type projection
    +--> CheckedOwnershipCertificate
    +--> CheckedNeedProducerAdmission
    +--> CheckedViewMatchAdmission
```

The registrar rejects a missing, unknown, or context-incompatible value class
or persistence mode before it publishes the accepted world. Producer names,
Rust type names, nominal names, or suffixes never infer either field.

Every standard opaque spec and every environment/adapter/test constructor is
updated in the same ownership-evidence cut. There is no period in which an old
constructor silently chooses a default.

## 2. Existing enum ownership

`RuntimeOpaqueValueClass` and `RuntimeOpaquePersistence` are Arcweft-owned
closed enums. Any semantic tag/payload behavior required by catalog digest or
ownership classification is added to their original inherent `impl`. An
extension trait, duplicated match, or registration side table is rejected.

Opaque rows:

| Value class | Persistence | Retained disposition |
|---|---|---|
| `Plain` | constant-admissible | `SnapshotClone` |
| `Plain` | snapshot-only | `SnapshotClone` |
| `AffineHandle(_)` | either | reject `AffineValue` |

`Plain` admission still requires the exact producer, semantic type identity,
runtime projection, and snapshot path to agree. “Plain” is not a name-based
blanket permission.

## 3. Current ownership context

```rust
CheckedOwnershipContext {
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
}
```

This context is exact. `ResourceTypeRegistry` is removed until a `TypeKind`
variant carries an exact typed resource identity that can select one registry
row.

Current `AgentResource` and `AgentResourceBody` use their existing core Agent
DTO runtime/snapshot owner and classify `SnapshotClone`. The classifier does
not query a registry by display name or by a generic Agent family.

## 4. Semantic dispositions

The only successful type-directed dispositions are:

- `Copy`: the exact runtime carrier is semantically Copy and needs no heap or
  external identity reconstruction;
- `SnapshotClone`: the exact runtime/snapshot owner can preserve an immutable
  equivalent value.

A semantic disposition is not enough on its own. The classifier validates that
the concrete runtime carrier and snapshot path agree. A value-level certificate
is required where type alone cannot establish the carrier, notably Function
and Need producer arguments.

Disposition combination is:

```text
combine(Copy, Copy) = Copy
combine(Copy, SnapshotClone) = SnapshotClone
combine(SnapshotClone, SnapshotClone) = SnapshotClone
combine(any, Reject) = Reject
```

## 5. Classifier algorithm

`RegisteredSemanticWorld::checked_ownership` performs:

1. initialize checked counters and a project/accepted nominal DFS stack;
2. visit the root `TypeKind`;
3. charge one node before matching the variant;
4. dispatch exhaustively on the original enum;
5. obtain exact accepted owner evidence before descending;
6. descend children in semantic/source/declaration order;
7. return the first failure immediately;
8. collect each consulted evidence row;
9. deduplicate/sort evidence only after successful traversal;
10. compute `OwnershipEvidenceDigest`;
11. publish one complete certificate.

There is no wildcard/default branch.

### Cycle behavior

Structural `Box<TypeKind>` recursion is finite by construction, but accepted
project/nominal schemas may be recursive. The DFS key is the exact semantic
type/nominal identity. Re-entering an active key fails
`RecursiveRetentionCycle`; current runtime snapshot owners do not publish a
general cyclic object graph. A completed key may reuse its memoized disposition
and evidence.

### First-error behavior

Children are visited:

- tuple/choice/result/option in source/semantic order;
- record fields in accepted declaration order;
- enum cases then payload fields in accepted declaration order;
- generic arguments in source order;
- project nominal edges in accepted declaration order.

The first failure is stable under map/hash iteration differences.

## 6. Function value certificate

`TypeKind::Function` always returns
`FunctionValueRequiresCertificate`. A separate exact value-level classifier
may admit only:

```text
CaptureFreeStableCallable {
  RuntimeCallableId,
  CallableContractHash,
  zero captures,
  accepted stable runtime callable reference
}
```

as `SnapshotClone`.

Any closure, nonempty capture environment, dynamically constructed function,
opaque host closure, or missing accepted callable identity rejects. Capture
types are not guessed from the Function type. This contract does not add a
generic closure snapshot format.

## 7. Need value certificate

`TypeKind::Need<T>` itself classifies `SnapshotClone` because the concrete
carrier is `RuntimeNeedHandle`, not `T`. It requires:

- exact runtime payload type identity for `T`;
- a verified `RuntimeNeedHandle`;
- a complete `CheckedNeedProducerAdmission`;
- canonical digestible retained producer arguments; and
- rederivation of the handle's correlation/spec relationship.

The classifier does not recursively require `T` to be snapshot-cloneable just
to retain the handle. When a View Match retains a Ready payload binding, that
binding's own `T` type is classified independently.

## 8. Exact work limits

```text
max_type_nodes                 = 65_536
max_recursion_depth            = 64
max_nominal_edges              = 16_384
max_active_nominal_depth       = 64
max_evidence_rows              = 16_384
max_value_certificate_nodes    = 65_536
max_function_captures          = 4_096
max_producer_arguments         = 4_096
```

All counters are checked `u64`, charged before allocation/descent. Exact-limit
input may succeed; one-over returns `CheckedOwnershipError::WorkLimit` and
publishes no certificate/digest/admission row.

## 9. Total current `TypeKind` matrix

| `TypeKind` variant | Result | Recursion | Exact evidence / rejection |
|---|---|---|---|
| `Bool` | `Copy` | none | existing scalar RuntimeValue carrier |
| `I8` | `Copy` | none | existing scalar RuntimeValue carrier |
| `I16` | `Copy` | none | existing scalar RuntimeValue carrier |
| `I32` | `Copy` | none | existing scalar RuntimeValue carrier |
| `I64` | `Copy` | none | existing scalar RuntimeValue carrier |
| `I128` | `Copy` | none | existing scalar RuntimeValue carrier |
| `ISize` | `Copy` | none | existing scalar RuntimeValue carrier |
| `U8` | `Copy` | none | existing scalar RuntimeValue carrier |
| `U16` | `Copy` | none | existing scalar RuntimeValue carrier |
| `U32` | `Copy` | none | existing scalar RuntimeValue carrier |
| `U64` | `Copy` | none | existing scalar RuntimeValue carrier |
| `U128` | `Copy` | none | existing scalar RuntimeValue carrier |
| `USize` | `Copy` | none | existing scalar RuntimeValue carrier |
| `F32` | `Copy` | none | existing scalar RuntimeValue carrier |
| `F64` | `Copy` | none | existing scalar RuntimeValue carrier |
| `Char` | `Copy` | none | existing scalar RuntimeValue carrier |
| `Duration` | `Copy` | none | existing scalar RuntimeValue carrier |
| `Unit` | `Copy` | none | existing scalar RuntimeValue carrier |
| `Never` | `Copy` | none | existing scalar RuntimeValue carrier |
| `String` | `SnapshotClone` | none | owned current runtime/string/progress snapshot owner |
| `Bytes` | `SnapshotClone` | none | owned current runtime/string/progress snapshot owner |
| `TextCluster` | `SnapshotClone` | none | owned current runtime/string/progress snapshot owner |
| `DisplayText` | `SnapshotClone` | none | owned current runtime/string/progress snapshot owner |
| `DebugStatePath` | `SnapshotClone` | none | owned current runtime/string/progress snapshot owner |
| `ObservationFieldPath` | `SnapshotClone` | none | owned current runtime/string/progress snapshot owner |
| `Progress` | `SnapshotClone` | none | owned current runtime/string/progress snapshot owner |
| `StageApi` | `Reject` | none | compile-time capability only; reject `FrameLocalValue` |
| `LineContext` | `Reject` | none | line activation context; reject `FrameLocalValue` |
| `StageActorHandle` | `Reject` | none | affine handle semantics; reject `AffineValue` |
| `CueHandle` | `Reject` | none | affine handle semantics; reject `AffineValue` |
| `VoiceHandle` | `Reject` | none | affine handle semantics; reject `AffineValue` |
| `Handle` | `Reject` | none | affine handle semantics; reject `AffineValue` |
| `ThreadHandle` | `Reject` | none | affine handle semantics; reject `AffineValue` |
| `Range` | `Reject` | none | current RuntimeValue canonical owner has no range replay/save row; reject `MissingRuntimeSnapshotOwner` |
| `IteratorState` | `Reject` | none | stateful/frame-local iterator; reject `FrameLocalValue` |
| `Ref` | `SnapshotClone` | do not recurse associated payload type | RuntimeValue::EntityRef(String) |
| `Probe` | `SnapshotClone` | recurse checked probe value type where present | current core RuntimeAgentValue canonical owner |
| `Predicate` | `SnapshotClone` | recurse checked probe value type where present | current core RuntimeAgentValue canonical owner |
| `Observation` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `ObservedObject` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `AgentBBox` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `ActionName` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `ActionTarget` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `ActionResult` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `AgentValue` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `DataFormat` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `DataShape` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `AgentEntityMetadata` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `AgentSourceAnchor` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `AgentProjectGraphNeighborhood` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `AgentProjectGraphSymbol` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `AgentProjectGraphEdge` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `CaptureTarget` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `CaptureRef` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `AgentResource` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `AgentResourceBody` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `RagContextPack` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `AgentBuiltin` | `SnapshotClone` | closed DTO children in declaration order | current core Agent DTO snapshot owner; no ResourceTypeRegistry lookup |
| `Vec` | `SnapshotClone` | classify item; array validates concrete accepted length | existing RuntimeValue::Seq snapshot owner |
| `Array` | `SnapshotClone` | classify item; array validates concrete accepted length | existing RuntimeValue::Seq snapshot owner |
| `Seq` | `SnapshotClone` | classify item; array validates concrete accepted length | existing RuntimeValue::Seq snapshot owner |
| `Slice` | `SnapshotClone` | classify item | accepted runtime projection owns a copied RuntimeValue::Seq, not BorrowRef |
| `Map` | `Reject` | validate key/value type closure for diagnostics only | current RuntimeValue has no unconditional Map snapshot owner; reject `MissingRuntimeSnapshotOwner` |
| `BorrowRef` | `Reject` | none | borrow kind/lifetime retained exactly; reject `BorrowedValue` |
| `Need` | `SnapshotClone` | do not require payload ownership; require payload runtime type identity and exact producer-argument value certificate | new RuntimeNeedHandle snapshot owner |
| `Stream` | `Reject` | none | protected affine stream owner; reject `StreamValue` |
| `Result` | `max(Copy, SnapshotClone)` | classify children in semantic/source order | existing closed aggregate RuntimeValue owner |
| `Option` | `max(Copy, SnapshotClone)` | classify children in semantic/source order | existing closed aggregate RuntimeValue owner |
| `Tuple` | `max(Copy, SnapshotClone)` | classify children in semantic/source order | existing closed aggregate RuntimeValue owner |
| `Choice` | `max(Copy, SnapshotClone)` | classify children in semantic/source order | existing closed aggregate RuntimeValue owner |
| `Shared` | `SnapshotClone` | classify child; cycle checked | accepted Shared snapshot owner introduced by the ownership/carrier cuts |
| `Function` | `Reject at type level` | value-level certificate only | capture-free stable callable may receive SnapshotClone certificate; closures/captures reject; reject `FunctionValueRequiresCertificate` |
| `GenericParam` | `Reject` | none | must be substituted before admission; reject `UnresolvedType` |
| `ProjectNominal` | `max(Copy, SnapshotClone)` | walk fields/cases in accepted declaration order with semantic-id cycle guard | ProjectSymbolTable exact declaration/layout evidence |
| `AcceptedNominal` | `delegate` | Exact -> child; Opaque -> class/persistence; Character -> character owner | RegisteredSemanticWorld accepted nominal record |
| `OpenNominal` | `Reject` | none | open rule has no exact persistent runtime owner; reject `UnresolvedType` |
| `Error` | `Reject` | none | poison/recovery fact; reject `UnresolvedType` |
| `Projection` | `Reject` | none | associated projection must be resolved first; reject `UnresolvedType` |
| `CharacterPatch` | `SnapshotClone` | walk owned fields/cases in accepted order | current owned Character/dialogue DTO snapshot owner |
| `FocusPatch` | `SnapshotClone` | walk owned fields/cases in accepted order | current owned Character/dialogue DTO snapshot owner |
| `CharacterDialogue` | `SnapshotClone` | walk owned fields/cases in accepted order | current owned Character/dialogue DTO snapshot owner |
| `CharacterNominal` | `SnapshotClone` | walk owned fields/cases in accepted order | current owned Character/dialogue DTO snapshot owner |
| `DialogueLine` | `Reject` | none | non-escaping suspension operation/line-plan result; reject `FrameLocalValue` |
| `ViewValue` | `Reject` | none | no published exact runtime View projection/snapshot owner; reject `MissingViewPersistenceEvidence` |
| `Named` | `Reject` | none | internal/host type lacks accepted closed owner; reject `UnresolvedType` |

## 10. Application boundaries

The classifier is applied only to:

1. retained View outputs;
2. retained View captures;
3. Need producer arguments/captures; and
4. exact value-level certificates requested by those products.

It is not applied while constructing a generic `CheckedMatch`. It is not a
general ban on ordinary lexical use, move, destructuring, pattern matching, or
function invocation.

## 11. Accepted catalog digest change

For an opaque accepted nominal record, the catalog transcript appends these
fields in this exact order after producer identity:

```text
RuntimeOpaqueValueClass inherent semantic transcript
RuntimeOpaquePersistence inherent semantic tag:u8
```

Changing either changes `AcceptedNominalCatalogDigest`, runtime normalized type
projection, and consulted ownership evidence. Tampering cannot leave a stale
digest valid.

The accepted catalog digest is still the whole accepted catalog root. View
admission commits only the exact consulted evidence rows in
`OwnershipEvidenceDigest`; it does not substitute the whole catalog digest.

## 12. Error precedence

```text
unresolved/poisoned type
< missing accepted nominal/project owner
< missing mandatory opaque evidence
< exact carrier/snapshot mismatch
< affine/Stream/borrow/frame-local/ViewValue rejection
< recursive cycle
< work limit
< evidence digest construction
```

For a composite, child source/declaration order takes precedence over category
order between different children.

## 13. Agent resource proof

Current `TypeKind::AgentResource` and `AgentResourceBody` do not carry an exact
resource-registry key. Therefore:

- `ResourceTypeRegistry` cannot be consulted;
- no whole registry digest enters ownership evidence;
- the current core Agent DTO carrier/snapshot contract is the sole evidence;
- both classify `SnapshotClone`;
- a future exact resource-bearing semantic type may add a typed registry
  dependency without changing these current rows.

## 14. Negative structural rules

Implementation must prove absence of:

- `CheckedOwnershipContext.resource_types`;
- `ResourceTypeRegistry` lookup for AgentResource/Body;
- `AcceptedNominalInventoryInput::new` overload omitting evidence;
- `AcceptedNominalRecord::try_new_opaque` overload omitting evidence;
- name/producer-based default inference;
- an extension trait that adds opaque semantic behavior;
- wildcard/default TypeKind classification;
- generic Match calling ownership classification; and
- unconditional ViewValue snapshot admission.
