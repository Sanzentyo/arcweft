# Carrier-backed ownership and producer-admission matrix

## 1. Classifier result

```rust
pub enum OwnershipDisposition {
    Copy,
    SnapshotClone(OwnershipSnapshotEvidence),
    Delegate(OwnershipDelegate),
}

pub struct OwnershipSnapshotEvidence {
    pub runtime_projection: RuntimeCarrierProjection,
    pub live_carrier: RuntimeValueCarrierKind,
    pub canonical_identity: RuntimeValueCanonicalCarrier,
    pub snapshot_codec: RuntimeValueSnapshotCarrierV1,
    pub consulted_catalog_rows: Box<[OwnershipCatalogEvidence]>,
}

pub enum OwnershipRejection {
    AffineValue,
    BorrowedValue,
    StreamValue,
    FrameLocalValue,
    MissingCanonicalIdentity,
    MissingRuntimeSnapshotOwner,
    MissingViewPersistenceEvidence,
    FunctionValueRequiresCertificate,
    UnresolvedType,
}
```

A successful `SnapshotClone` is impossible until all four concrete owner fields
are present. Family-name prose and planned carriers do not satisfy the
constructor.

The classifier is one exhaustive match on the current 85-variant `TypeKind`.
It may delegate only to a closed sub-enum whose every variant has an exact
mapping. There is no wildcard success arm.

## 2. Recursion rules

- ordinary aggregate/nominal children are visited in source/semantic order;
- cycles are detected through accepted semantic identities, not raw TypeId;
- `Ref<T>` and `Need<T>` do not recurse into a live owned child because their
  payload type is a contract, not an embedded RuntimeValue;
- `Probe<T>` validates the result-type certificate but its runtime probe value
  has no child RuntimeValue;
- `Predicate` is a leaf in the TypeKind graph;
- `Shared<T>` is rejected before child recursion;
- ProjectNominal walks accepted fields/cases/layout;
- AcceptedNominal delegates to exact/opaque/character semantic owners.

`RuntimeAgentPredicate` may itself contain canonical RuntimeValue operands.
That is value encoding recursion and is not a `TypeKind::Predicate` child edge.

## 3. Same-cut exact projection owners

### Agent protocol records

For current Agent DTO types whose runtime-plan projection reports
`accepts_protocol_record`, Cut 2 defines:

```rust
pub struct RuntimeAgentProtocolRecordProjectionV1 {
    operational_type: RuntimeAgentOperationalType,
    nominal: RuntimeNominalTypeId,
    layout: TypeLayoutHash,
    fields: Box<[RuntimeAgentProtocolFieldProjectionV1]>,
}
```

`nominal`, `layout` and field identities come from the accepted Agent Prelude
catalog for the exact `RuntimeAgentOperationalType`. The live value is
`RuntimeValue::NominalRecord`. Restore verifies the same nominal/layout/field
order. It is not an opaque name or family side table.

### Closed Agent scalar/variant values

`RuntimeAgentClosedVariantProjectionV1` maps the exact accepted Agent Prelude
owner and case ordinal to `RuntimeValue::Variant`. The display name, when
retained for diagnostics, must agree with owner+ordinal but is not the primary
case identity.

### Dialogue/character nominals

`RuntimeDialogueNominalProjectionV1` maps CharacterPatch, FocusPatch,
CharacterDialogue and CharacterNominal to accepted `RuntimeNominalTypeId`,
`TypeLayoutHash` and source-order fields, carried by
`RuntimeValue::NominalRecord`.

These projection owners are published with the ownership cut and are consumed
later by final runtime lowering/snapshot code. They introduce no View or task
carrier.

## 4. Exhaustive current matrix

| `TypeKind` | Disposition | Recursion | Runtime projection | Live carrier | Canonical identity | Snapshot codec | Rejection/notes |
|---|---|---|---|---|---|---|---|
| `Bool` | `Copy` | none | — | RuntimeValue::Bool | canonical RuntimeValue visitor for RuntimeValue::Bool | RuntimeValueSnapshotV1::Bool | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `I8` | `Copy` | none | — | RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | canonical RuntimeValue visitor for RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | RuntimeValueSnapshotV1::IntOrUInt | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `I16` | `Copy` | none | — | RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | canonical RuntimeValue visitor for RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | RuntimeValueSnapshotV1::IntOrUInt | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `I32` | `Copy` | none | — | RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | canonical RuntimeValue visitor for RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | RuntimeValueSnapshotV1::IntOrUInt | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `I64` | `Copy` | none | — | RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | canonical RuntimeValue visitor for RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | RuntimeValueSnapshotV1::IntOrUInt | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `I128` | `Copy` | none | — | RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | canonical RuntimeValue visitor for RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | RuntimeValueSnapshotV1::IntOrUInt | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `ISize` | `Copy` | none | — | RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | canonical RuntimeValue visitor for RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | RuntimeValueSnapshotV1::IntOrUInt | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `U8` | `Copy` | none | — | RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | canonical RuntimeValue visitor for RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | RuntimeValueSnapshotV1::IntOrUInt | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `U16` | `Copy` | none | — | RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | canonical RuntimeValue visitor for RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | RuntimeValueSnapshotV1::IntOrUInt | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `U32` | `Copy` | none | — | RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | canonical RuntimeValue visitor for RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | RuntimeValueSnapshotV1::IntOrUInt | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `U64` | `Copy` | none | — | RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | canonical RuntimeValue visitor for RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | RuntimeValueSnapshotV1::IntOrUInt | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `U128` | `Copy` | none | — | RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | canonical RuntimeValue visitor for RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | RuntimeValueSnapshotV1::IntOrUInt | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `USize` | `Copy` | none | — | RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | canonical RuntimeValue visitor for RuntimeValue::Int/UInt with exact RuntimeSignedIntWidth/RuntimeUnsignedIntWidth | RuntimeValueSnapshotV1::IntOrUInt | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `F32` | `Copy` | none | — | RuntimeValue::F32 | canonical RuntimeValue visitor for RuntimeValue::F32 | RuntimeValueSnapshotV1::F32 | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `F64` | `Copy` | none | — | RuntimeValue::F64 | canonical RuntimeValue visitor for RuntimeValue::F64 | RuntimeValueSnapshotV1::F64 | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `String` | `SnapshotClone` | none | RuntimePlanTypeProjection::String | RuntimeValue::String(String) | canonical RuntimeValue String tag + validated UTF-8 bytes | RuntimeValueSnapshotV1::String | TextCluster and DisplayText projection validation occurs before constructing the shared String carrier; no source spelling is retained. |
| `Char` | `Copy` | none | — | RuntimeValue::Char | canonical RuntimeValue visitor for RuntimeValue::Char | RuntimeValueSnapshotV1::Char | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `Bytes` | `SnapshotClone` | none; every element is validated U8 | RuntimePlanTypeProjection::Bytes | RuntimeValue::Seq(RuntimeSeq::Values([RuntimeValue::UInt(RuntimeUInt::U8(_)), ...])) | canonical RuntimeValue Seq transcript with exact U8 element tags | RuntimeValueSnapshotV1::Seq(RuntimeSeqSnapshotV1::Values) | The same-cut projection forbids dense non-U8 storage for this semantic type. |
| `TextCluster` | `SnapshotClone` | none | RuntimeTextClusterProjectionV1::ValidatedUtf8 | RuntimeValue::String(String) | canonical RuntimeValue String tag + validated UTF-8 bytes | RuntimeValueSnapshotV1::String | TextCluster and DisplayText projection validation occurs before constructing the shared String carrier; no source spelling is retained. |
| `Duration` | `Copy` | none | — | RuntimeValue::Duration | canonical RuntimeValue visitor for RuntimeValue::Duration | RuntimeValueSnapshotV1::Duration | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `Progress` | `SnapshotClone` | none | RuntimePlanTypeProjection::Progress | RuntimeValue::Progress(arcweft_need::Progress) | canonical RuntimeValue Progress transcript with exact f32 bits and optional label | RuntimeValueSnapshotV1::Progress | — |
| `StageApi` | `Reject` | none | — | — | — | — | AffineValue; Runtime-owned capability/handle; no snapshot-clone producer value identity is admitted. |
| `LineContext` | `Reject` | none | — | — | — | — | AffineValue; Runtime-owned capability/handle; no snapshot-clone producer value identity is admitted. |
| `StageActorHandle` | `Reject` | none | — | — | — | — | AffineValue; Runtime-owned capability/handle; no snapshot-clone producer value identity is admitted. |
| `CueHandle` | `Reject` | none | — | — | — | — | AffineValue; Runtime-owned capability/handle; no snapshot-clone producer value identity is admitted. |
| `VoiceHandle` | `Reject` | none | — | — | — | — | AffineValue; Runtime-owned capability/handle; no snapshot-clone producer value identity is admitted. |
| `Range` | `Reject` | none | RuntimePlanTypeProjection::Range | RuntimeValue::Range | — | RuntimeValueSnapshotV1::Range | MissingCanonicalIdentity; A save carrier exists, but the current sole canonical identity grammar has no admitted stable Range identity. |
| `IteratorState` | `Reject` | none | RuntimePlanTypeProjection::Iterator | RuntimeValue::Iterator | — | RuntimeValueSnapshotV1::Iterator | FrameLocalValue; Iterator cursor state is frame/session local and is not producer-retainable. |
| `DisplayText` | `SnapshotClone` | none | RuntimeDisplayTextProjectionV1::ValidatedUtf8 | RuntimeValue::String(String) | canonical RuntimeValue String tag + validated UTF-8 bytes | RuntimeValueSnapshotV1::String | TextCluster and DisplayText projection validation occurs before constructing the shared String carrier; no source spelling is retained. |
| `DebugStatePath` | `SnapshotClone` | none | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::DebugStatePath) | RuntimeValue::Agent(RuntimeAgentValue::DebugStatePath(RuntimeAgentPath)) | canonical RuntimeValue Agent/DebugStatePath transcript | RuntimeValueSnapshotV1::Agent(RuntimeAgentSnapshotV1::DebugStatePath) | — |
| `ObservationFieldPath` | `SnapshotClone` | none | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::ObservationFieldPath) | RuntimeValue::Agent(RuntimeAgentValue::ObservationFieldPath(RuntimeAgentPath)) | canonical RuntimeValue Agent/ObservationFieldPath transcript | RuntimeValueSnapshotV1::Agent(RuntimeAgentSnapshotV1::ObservationFieldPath) | — |
| `Ref` | `SnapshotClone` | do not recurse associated payload type | RuntimePlanTypeProjection::Reference(payload_type) | RuntimeValue::EntityRef(String) | canonical RuntimeValue EntityRef transcript | RuntimeValueSnapshotV1::EntityRef | The payload type is a lookup contract, not an owned child value. |
| `Probe` | `SnapshotClone` | validate the checked probe-result type certificate; no child RuntimeValue is owned | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::Probe(result_type)) | RuntimeValue::Agent(RuntimeAgentValue::Probe(RuntimeAgentProbe)) | canonical RuntimeValue Agent/Probe transcript | RuntimeValueSnapshotV1::Agent(RuntimeAgentSnapshotV1::Probe) | — |
| `Predicate` | `SnapshotClone` | none | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::Predicate) | RuntimeValue::Agent(RuntimeAgentValue::Predicate(RuntimeAgentPredicate)) | canonical RuntimeValue Agent/Predicate transcript, recursively visiting predicate-owned RuntimeValue operands only | RuntimeValueSnapshotV1::Agent(RuntimeAgentSnapshotV1::Predicate) | Predicate is a TypeKind leaf. The predicate value encoder may visit value operands, but the TypeKind classifier has no child edge. |
| `Observation` | `SnapshotClone` | closed protocol-record fields in accepted declaration order | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::Observation); RuntimeAgentProtocolRecordProjectionV1 | RuntimeValue::NominalRecord(RuntimeNominalRecordValue { type_id: core-owned Agent protocol nominal id, layout: accepted Agent protocol TypeLayoutHash, fields }) | canonical RuntimeValue NominalRecord transcript over exact accepted Agent protocol type/layout and source-order fields | RuntimeValueSnapshotV1::NominalRecord | RuntimeAgentProtocolRecordProjectionV1 is fully specified in this cut and derives type_id/layout from RuntimeAgentOperationalType; it is not a family-name side table. |
| `ObservedObject` | `SnapshotClone` | closed protocol-record fields in accepted declaration order | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::ObservedObject); RuntimeAgentProtocolRecordProjectionV1 | RuntimeValue::NominalRecord(RuntimeNominalRecordValue { type_id: core-owned Agent protocol nominal id, layout: accepted Agent protocol TypeLayoutHash, fields }) | canonical RuntimeValue NominalRecord transcript over exact accepted Agent protocol type/layout and source-order fields | RuntimeValueSnapshotV1::NominalRecord | RuntimeAgentProtocolRecordProjectionV1 is fully specified in this cut and derives type_id/layout from RuntimeAgentOperationalType; it is not a family-name side table. |
| `AgentBBox` | `SnapshotClone` | closed protocol-record fields in accepted declaration order | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::BoundingBox); RuntimeAgentProtocolRecordProjectionV1 | RuntimeValue::NominalRecord(RuntimeNominalRecordValue { type_id: core-owned Agent protocol nominal id, layout: accepted Agent protocol TypeLayoutHash, fields }) | canonical RuntimeValue NominalRecord transcript over exact accepted Agent protocol type/layout and source-order fields | RuntimeValueSnapshotV1::NominalRecord | RuntimeAgentProtocolRecordProjectionV1 is fully specified in this cut and derives type_id/layout from RuntimeAgentOperationalType; it is not a family-name side table. |
| `ActionName` | `SnapshotClone` | closed variant payloads in declaration order | RuntimeAgentClosedVariantProjectionV1::ActionName | RuntimeValue::Variant { owner: RuntimeVariantIdentity, ordinal, name, payload } with owner and ordinal derived from the accepted Agent prelude catalog | canonical RuntimeValue Variant transcript; display name is validated but identity is owner+ordinal+payload | RuntimeValueSnapshotV1::Variant | The projection owner is fully specified in this cut and rejects a mismatched display name or payload layout. |
| `ActionTarget` | `SnapshotClone` | none | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::ActionTarget) | RuntimeValue::Agent(RuntimeAgentValue::ActionTarget(RuntimeAgentActionTarget)) | canonical RuntimeValue Agent/ActionTarget transcript | RuntimeValueSnapshotV1::Agent(RuntimeAgentSnapshotV1::ActionTarget) | — |
| `ActionResult` | `SnapshotClone` | closed protocol-record fields in accepted declaration order | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::ActionResult); RuntimeAgentProtocolRecordProjectionV1 | RuntimeValue::NominalRecord(RuntimeNominalRecordValue { type_id: core-owned Agent protocol nominal id, layout: accepted Agent protocol TypeLayoutHash, fields }) | canonical RuntimeValue NominalRecord transcript over exact accepted Agent protocol type/layout and source-order fields | RuntimeValueSnapshotV1::NominalRecord | RuntimeAgentProtocolRecordProjectionV1 is fully specified in this cut and derives type_id/layout from RuntimeAgentOperationalType; it is not a family-name side table. |
| `AgentValue` | `SnapshotClone` | closed variant payloads in declaration order | RuntimeAgentClosedVariantProjectionV1::AgentValue | RuntimeValue::Variant { owner: RuntimeVariantIdentity, ordinal, name, payload } with owner and ordinal derived from the accepted Agent prelude catalog | canonical RuntimeValue Variant transcript; display name is validated but identity is owner+ordinal+payload | RuntimeValueSnapshotV1::Variant | The projection owner is fully specified in this cut and rejects a mismatched display name or payload layout. |
| `DataFormat` | `SnapshotClone` | closed variant payloads in declaration order | RuntimeAgentClosedVariantProjectionV1::DataFormat | RuntimeValue::Variant { owner: RuntimeVariantIdentity, ordinal, name, payload } with owner and ordinal derived from the accepted Agent prelude catalog | canonical RuntimeValue Variant transcript; display name is validated but identity is owner+ordinal+payload | RuntimeValueSnapshotV1::Variant | The projection owner is fully specified in this cut and rejects a mismatched display name or payload layout. |
| `DataShape` | `SnapshotClone` | closed variant payloads in declaration order | RuntimeAgentClosedVariantProjectionV1::DataShape | RuntimeValue::Variant { owner: RuntimeVariantIdentity, ordinal, name, payload } with owner and ordinal derived from the accepted Agent prelude catalog | canonical RuntimeValue Variant transcript; display name is validated but identity is owner+ordinal+payload | RuntimeValueSnapshotV1::Variant | The projection owner is fully specified in this cut and rejects a mismatched display name or payload layout. |
| `AgentEntityMetadata` | `SnapshotClone` | closed protocol-record fields in accepted declaration order | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::EntityMetadata); RuntimeAgentProtocolRecordProjectionV1 | RuntimeValue::NominalRecord(RuntimeNominalRecordValue { type_id: core-owned Agent protocol nominal id, layout: accepted Agent protocol TypeLayoutHash, fields }) | canonical RuntimeValue NominalRecord transcript over exact accepted Agent protocol type/layout and source-order fields | RuntimeValueSnapshotV1::NominalRecord | RuntimeAgentProtocolRecordProjectionV1 is fully specified in this cut and derives type_id/layout from RuntimeAgentOperationalType; it is not a family-name side table. |
| `AgentSourceAnchor` | `SnapshotClone` | closed protocol-record fields in accepted declaration order | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::SourceAnchor); RuntimeAgentProtocolRecordProjectionV1 | RuntimeValue::NominalRecord(RuntimeNominalRecordValue { type_id: core-owned Agent protocol nominal id, layout: accepted Agent protocol TypeLayoutHash, fields }) | canonical RuntimeValue NominalRecord transcript over exact accepted Agent protocol type/layout and source-order fields | RuntimeValueSnapshotV1::NominalRecord | RuntimeAgentProtocolRecordProjectionV1 is fully specified in this cut and derives type_id/layout from RuntimeAgentOperationalType; it is not a family-name side table. |
| `AgentProjectGraphNeighborhood` | `SnapshotClone` | closed protocol-record fields in accepted declaration order | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::ProjectGraphNeighborhood); RuntimeAgentProtocolRecordProjectionV1 | RuntimeValue::NominalRecord(RuntimeNominalRecordValue { type_id: core-owned Agent protocol nominal id, layout: accepted Agent protocol TypeLayoutHash, fields }) | canonical RuntimeValue NominalRecord transcript over exact accepted Agent protocol type/layout and source-order fields | RuntimeValueSnapshotV1::NominalRecord | RuntimeAgentProtocolRecordProjectionV1 is fully specified in this cut and derives type_id/layout from RuntimeAgentOperationalType; it is not a family-name side table. |
| `AgentProjectGraphSymbol` | `SnapshotClone` | closed protocol-record fields in accepted declaration order | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::ProjectGraphSymbol); RuntimeAgentProtocolRecordProjectionV1 | RuntimeValue::NominalRecord(RuntimeNominalRecordValue { type_id: core-owned Agent protocol nominal id, layout: accepted Agent protocol TypeLayoutHash, fields }) | canonical RuntimeValue NominalRecord transcript over exact accepted Agent protocol type/layout and source-order fields | RuntimeValueSnapshotV1::NominalRecord | RuntimeAgentProtocolRecordProjectionV1 is fully specified in this cut and derives type_id/layout from RuntimeAgentOperationalType; it is not a family-name side table. |
| `AgentProjectGraphEdge` | `SnapshotClone` | closed protocol-record fields in accepted declaration order | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::ProjectGraphEdge); RuntimeAgentProtocolRecordProjectionV1 | RuntimeValue::NominalRecord(RuntimeNominalRecordValue { type_id: core-owned Agent protocol nominal id, layout: accepted Agent protocol TypeLayoutHash, fields }) | canonical RuntimeValue NominalRecord transcript over exact accepted Agent protocol type/layout and source-order fields | RuntimeValueSnapshotV1::NominalRecord | RuntimeAgentProtocolRecordProjectionV1 is fully specified in this cut and derives type_id/layout from RuntimeAgentOperationalType; it is not a family-name side table. |
| `CaptureTarget` | `SnapshotClone` | none | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::CaptureTarget) | RuntimeValue::Agent(RuntimeAgentValue::CaptureTarget(RuntimeAgentCaptureTarget)) | canonical RuntimeValue Agent/CaptureTarget transcript | RuntimeValueSnapshotV1::Agent(RuntimeAgentSnapshotV1::CaptureTarget) | — |
| `CaptureRef` | `SnapshotClone` | closed protocol-record fields in accepted declaration order | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::CaptureReference); RuntimeAgentProtocolRecordProjectionV1 | RuntimeValue::NominalRecord(RuntimeNominalRecordValue { type_id: core-owned Agent protocol nominal id, layout: accepted Agent protocol TypeLayoutHash, fields }) | canonical RuntimeValue NominalRecord transcript over exact accepted Agent protocol type/layout and source-order fields | RuntimeValueSnapshotV1::NominalRecord | RuntimeAgentProtocolRecordProjectionV1 is fully specified in this cut and derives type_id/layout from RuntimeAgentOperationalType; it is not a family-name side table. |
| `AgentResource` | `SnapshotClone` | closed protocol-record fields in accepted declaration order | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::Resource); RuntimeAgentProtocolRecordProjectionV1 | RuntimeValue::NominalRecord(RuntimeNominalRecordValue { type_id: core-owned Agent protocol nominal id, layout: accepted Agent protocol TypeLayoutHash, fields }) | canonical RuntimeValue NominalRecord transcript over exact accepted Agent protocol type/layout and source-order fields | RuntimeValueSnapshotV1::NominalRecord | RuntimeAgentProtocolRecordProjectionV1 is fully specified in this cut and derives type_id/layout from RuntimeAgentOperationalType; it is not a family-name side table. |
| `AgentResourceBody` | `SnapshotClone` | closed protocol-record fields in accepted declaration order | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::ResourceBody); RuntimeAgentProtocolRecordProjectionV1 | RuntimeValue::NominalRecord(RuntimeNominalRecordValue { type_id: core-owned Agent protocol nominal id, layout: accepted Agent protocol TypeLayoutHash, fields }) | canonical RuntimeValue NominalRecord transcript over exact accepted Agent protocol type/layout and source-order fields | RuntimeValueSnapshotV1::NominalRecord | RuntimeAgentProtocolRecordProjectionV1 is fully specified in this cut and derives type_id/layout from RuntimeAgentOperationalType; it is not a family-name side table. |
| `RagContextPack` | `SnapshotClone` | closed protocol-record fields in accepted declaration order | RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::RagContextPack); RuntimeAgentProtocolRecordProjectionV1 | RuntimeValue::NominalRecord(RuntimeNominalRecordValue { type_id: core-owned Agent protocol nominal id, layout: accepted Agent protocol TypeLayoutHash, fields }) | canonical RuntimeValue NominalRecord transcript over exact accepted Agent protocol type/layout and source-order fields | RuntimeValueSnapshotV1::NominalRecord | RuntimeAgentProtocolRecordProjectionV1 is fully specified in this cut and derives type_id/layout from RuntimeAgentOperationalType; it is not a family-name side table. |
| `AgentBuiltin` | `Delegate` | delegate to the exact closed AgentBuiltinType case | RuntimeAgentBuiltinProjectionV1::from_checked_case | the exact carrier named by the delegated current TypeKind row | the exact canonical transcript named by the delegated row | the exact RuntimeValueSnapshotV1 row named by the delegated row | No generic opaque or string fallback exists; an unlisted AgentBuiltinType case rejects. |
| `Vec` | `SnapshotClone` | classify item; Array also validates exact accepted length; Slice snapshots an owned copy | RuntimePlanTypeProjection::Sequence | RuntimeValue::Seq(RuntimeSeq), with Slice projected to an owned RuntimeSeq | canonical RuntimeValue Seq transcript | RuntimeValueSnapshotV1::Seq | — |
| `Array` | `SnapshotClone` | classify item; Array also validates exact accepted length; Slice snapshots an owned copy | RuntimePlanTypeProjection::Array | RuntimeValue::Seq(RuntimeSeq), with Slice projected to an owned RuntimeSeq | canonical RuntimeValue Seq transcript | RuntimeValueSnapshotV1::Seq | — |
| `Slice` | `SnapshotClone` | classify item; Array also validates exact accepted length; Slice snapshots an owned copy | RuntimePlanTypeProjection::Sequence | RuntimeValue::Seq(RuntimeSeq), with Slice projected to an owned RuntimeSeq | canonical RuntimeValue Seq transcript | RuntimeValueSnapshotV1::Seq | — |
| `Seq` | `SnapshotClone` | classify item; Array also validates exact accepted length; Slice snapshots an owned copy | RuntimePlanTypeProjection::Sequence | RuntimeValue::Seq(RuntimeSeq), with Slice projected to an owned RuntimeSeq | canonical RuntimeValue Seq transcript | RuntimeValueSnapshotV1::Seq | — |
| `Map` | `Reject` | validate key/value closure for diagnostics only | RuntimePlanTypeProjection::Map | — | — | — | MissingRuntimeSnapshotOwner; No unconditional canonical Map carrier/snapshot owner exists. |
| `BorrowRef` | `Reject` | none | — | — | — | — | BorrowedValue; Borrow kind and lifetime remain exact; no owned snapshot projection is fabricated. |
| `Need` | `SnapshotClone` | do not classify payload ownership; require exact RuntimeTypeSemanticDigest and producer argument admission | RuntimePlanTypeProjection::Need(payload_type) | RuntimeValue::NeedHandle(RuntimeNeedHandle) | canonical RuntimeValue tag 20 + NeedId only | RuntimeValueSnapshotV1::NeedHandle(RuntimeNeedHandleSnapshotV1) | — |
| `Stream` | `Reject` | none | RuntimePlanTypeProjection::Stream | — | — | — | StreamValue; Protected affine stream owner. |
| `Result` | `SnapshotClone` | classify child types in semantic/source order | RuntimePlanTypeProjection::Result | RuntimeValue::Variant with accepted Result owner/case | canonical RuntimeValue transcript for RuntimeValue::Variant with accepted Result owner/case | RuntimeValueSnapshotV1::Tuple or ::Variant as selected by the exact projection | — |
| `Option` | `SnapshotClone` | classify child types in semantic/source order | RuntimePlanTypeProjection::Option | RuntimeValue::Variant with accepted Option owner/case | canonical RuntimeValue transcript for RuntimeValue::Variant with accepted Option owner/case | RuntimeValueSnapshotV1::Tuple or ::Variant as selected by the exact projection | — |
| `Handle` | `Reject` | none | — | — | — | — | AffineValue; Runtime-owned capability/handle; no snapshot-clone producer value identity is admitted. |
| `ThreadHandle` | `Reject` | none | — | — | — | — | AffineValue; Runtime-owned capability/handle; no snapshot-clone producer value identity is admitted. |
| `Shared` | `Reject` | none | RuntimePlanTypeProjection::Shared | — | — | — | MissingRuntimeSnapshotOwner; This correction does not introduce a core Shared carrier, canonical identity, or snapshot codec. |
| `Function` | `RejectAtTypeLevel` | none | — | — | — | — | FunctionValueRequiresCertificate; Only a concrete capture-free stable callable may receive a value-level certificate naming RuntimeCallableId and CallableContractHash; closure/function types are not admitted generically. |
| `GenericParam` | `Reject` | none | — | — | — | — | UnresolvedType; Must be resolved to a closed accepted owner before admission. |
| `ProjectNominal` | `SnapshotClone` | walk accepted fields/cases in declaration order with semantic-id cycle guard | RuntimePlanTypeProjection::ProjectNominal { nominal: RuntimeNominalTypeId, layout: TypeLayoutHash, arguments } | RuntimeValue::NominalRecord(RuntimeNominalRecordValue) or RuntimeValue::Variant with the same accepted nominal owner/layout | canonical NominalRecord/Variant transcript including exact nominal owner and TypeLayoutHash | RuntimeValueSnapshotV1::NominalRecord or ::Variant selected by the accepted declaration shape | — |
| `AcceptedNominal` | `Delegate` | Exact -> accepted child layout; Opaque -> class/persistence; Character -> accepted character fields | RegisteredSemanticWorld accepted nominal record with mandatory exact semantics | Exact: NominalRecord/Variant; Opaque Plain: RuntimeValue::Opaque; Character: NominalRecord | Exact/Character canonical aggregate transcript; Opaque uses the sole opaque transcript and accepts Plain+ConstantAndSnapshot or Plain+SnapshotOnly | RuntimeValueSnapshotV1::NominalRecord/Variant/Opaque | AffineHandle is rejected even though its snapshot projection may exist for non-identity save state. |
| `OpenNominal` | `Reject` | none | — | — | — | — | UnresolvedType; Must be resolved to a closed accepted owner before admission. |
| `Error` | `Reject` | none | — | — | — | — | UnresolvedType; Must be resolved to a closed accepted owner before admission. |
| `Projection` | `Reject` | none | — | — | — | — | UnresolvedType; Must be resolved to a closed accepted owner before admission. |
| `CharacterPatch` | `SnapshotClone` | walk accepted dialogue/character fields in declaration order | RuntimeDialogueNominalProjectionV1::CharacterPatch | RuntimeValue::NominalRecord(RuntimeNominalRecordValue) with accepted dialogue/character RuntimeNominalTypeId and TypeLayoutHash | canonical RuntimeValue NominalRecord transcript | RuntimeValueSnapshotV1::NominalRecord | — |
| `FocusPatch` | `SnapshotClone` | walk accepted dialogue/character fields in declaration order | RuntimeDialogueNominalProjectionV1::FocusPatch | RuntimeValue::NominalRecord(RuntimeNominalRecordValue) with accepted dialogue/character RuntimeNominalTypeId and TypeLayoutHash | canonical RuntimeValue NominalRecord transcript | RuntimeValueSnapshotV1::NominalRecord | — |
| `CharacterDialogue` | `SnapshotClone` | walk accepted dialogue/character fields in declaration order | RuntimeDialogueNominalProjectionV1::CharacterDialogue | RuntimeValue::NominalRecord(RuntimeNominalRecordValue) with accepted dialogue/character RuntimeNominalTypeId and TypeLayoutHash | canonical RuntimeValue NominalRecord transcript | RuntimeValueSnapshotV1::NominalRecord | — |
| `DialogueLine` | `Reject` | none | — | — | — | — | FrameLocalValue; Non-escaping suspension/line-plan result. |
| `ViewValue` | `Reject` | none | — | — | — | — | MissingViewPersistenceEvidence; No public persistent View value carrier is introduced by this correction. |
| `CharacterNominal` | `SnapshotClone` | walk accepted dialogue/character fields in declaration order | RuntimeDialogueNominalProjectionV1::CharacterNominal | RuntimeValue::NominalRecord(RuntimeNominalRecordValue) with accepted dialogue/character RuntimeNominalTypeId and TypeLayoutHash | canonical RuntimeValue NominalRecord transcript | RuntimeValueSnapshotV1::NominalRecord | — |
| `Named` | `Reject` | none | — | — | — | — | UnresolvedType; Must be resolved to a closed accepted owner before admission. |
| `Tuple` | `SnapshotClone` | classify child types in semantic/source order | RuntimePlanTypeProjection::Tuple | RuntimeValue::Tuple | canonical RuntimeValue transcript for RuntimeValue::Tuple | RuntimeValueSnapshotV1::Tuple or ::Variant as selected by the exact projection | — |
| `Choice` | `SnapshotClone` | classify child types in semantic/source order | RuntimePlanTypeProjection::Choice | RuntimeValue::Variant with accepted Choice owner/case | canonical RuntimeValue transcript for RuntimeValue::Variant with accepted Choice owner/case | RuntimeValueSnapshotV1::Tuple or ::Variant as selected by the exact projection | — |
| `Unit` | `Copy` | none | — | RuntimeValue::Unit | canonical RuntimeValue visitor for RuntimeValue::Unit | RuntimeValueSnapshotV1::Unit | Copy values are also representable by the named snapshot row; producer retention does not require cloning. |
| `Never` | `Copy` | none | — | — | — | — | Uninhabited. No live value or snapshot row can be constructed; classification is vacuously unrestricted and any attempted value construction is rejected earlier. |

## 5. Opaque branch

An accepted opaque nominal must provide a catalog row containing:

- accepted nominal semantic identity;
- exact runtime opaque producer identity;
- `RuntimeOpaqueValueClass`;
- `RuntimeOpaquePersistence`;
- payload validator/codec identity.

Branches:

| Class/persistence | Ownership | Canonical identity | Constant publication |
|---|---|---|---|
| Plain + ConstantAndSnapshot | SnapshotClone | success | success |
| Plain + SnapshotOnly | SnapshotClone | success | reject at constant fence |
| AffineHandle + either persistence | reject | reject | reject |

No source spelling or inferred producer is accepted.

## 6. Shared closure

`Shared<T>` returns:

```rust
Err(OwnershipRejection::MissingRuntimeSnapshotOwner)
```

This correction deliberately defines none of the four required Shared owners.
There is no core Shared RuntimeValue carrier, canonical transcript, snapshot
row or construction/restore invariant. It also does not encode Shared as an
opaque nominal, extension trait or side table.

## 7. Machine/prose/classifier parity

`machine/ownership_matrix.json` has exactly 85 unique rows in the same order as
the current `TypeKind` enum. The validator enforces:

- set and count equality with the frozen current inventory;
- `Predicate.recursion == "none"`;
- Shared rejection and reason;
- every SnapshotClone has nonempty projection/live/canonical/snapshot fields;
- no planned/future carrier wording counts as evidence;
- no duplicate variant;
- no wildcard machine row.

The implementation test should instantiate an exhaustive match whose arms
export the same symbolic row ID, then compare the generated row set to the JSON
fixture.
