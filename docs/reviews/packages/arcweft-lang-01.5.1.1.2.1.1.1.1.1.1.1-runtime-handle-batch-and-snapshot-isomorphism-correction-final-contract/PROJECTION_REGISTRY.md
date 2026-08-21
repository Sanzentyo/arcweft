# Projection and newtype registry

| Name | Complete variants/shape | Constructor rule |
|---|---|---|
| `RuntimeCheckedTypeProjectionV1` | `Never`<br>`Unit`<br>`Bool`<br>`Signed(RuntimeSignedIntWidth)`<br>`Unsigned(RuntimeUnsignedIntWidth)`<br>`F32`<br>`F64`<br>`String`<br>`Char`<br>`Duration`<br>`Progress`<br>`EntityReference`<br>`Bytes`<br>`Sequence(Box<RuntimeCheckedTypeProjectionV1>)`<br>`Tuple(Box<[RuntimeCheckedTypeProjectionV1]>)`<br>`Choice(Box<[RuntimeCheckedTypeProjectionV1]>)`<br>`Nominal { nominal, semantic_identity, layout }`<br>`Opaque { owner }`<br>`Variant { nominal, semantic_identity, cases }`<br>`Result { ok, error }`<br>`Option(Box<RuntimeCheckedTypeProjectionV1>)`<br>`Agent(RuntimeAgentOperationalType)` | exhaustive match on current RuntimeCheckedType; no wildcard |
| `RuntimeAgentValueProjectionV1` | `ActionTarget`<br>`CaptureTarget`<br>`DebugStatePath`<br>`ObservationFieldPath`<br>`Probe`<br>`Diagnostics`<br>`Predicate`<br>`ViewportPoint` | exhaustive match on current RuntimeAgentValue; no protocol-record invention |
| `RuntimeDenseSeqProjectionV1` | `Units`<br>`Bools`<br>`I8`<br>`I16`<br>`I32`<br>`I64`<br>`I128`<br>`ISize`<br>`U8`<br>`U16`<br>`U32`<br>`U64`<br>`U128`<br>`USize`<br>`F32`<br>`F64`<br>`Strings`<br>`Chars`<br>`Durations`<br>`EntityRefs`<br>`Bytes` | exact typed constructor and restore join |
| `RuntimeTextProjectionV1` | `TextClusterUtf8 { nominal, semantic_identity }`<br>`DisplayTextUtf8 { nominal, semantic_identity }` | exact typed constructor and restore join |
| `RuntimeSequenceProjectionV1` | `Vec`<br>`Array { length }`<br>`Slice`<br>`Seq` | exact typed constructor and restore join |
| `RuntimeOwnershipTypeProjectionV1` | `Range`<br>`IteratorState`<br>`Map`<br>`Need`<br>`Stream`<br>`ThreadHandle`<br>`Shared`<br>`Function` | exact typed constructor and restore join |
| `RuntimeNominalRecordProjectionV1` | `Exact { nominal, semantic_identity, layout, fields }` | exact typed constructor and restore join |
| `RuntimeAcceptedNominalProjectionV1` | `ExactRecord`<br>`ExactVariant`<br>`ExactOpaque`<br>`Character` | exact typed constructor and restore join |
| `RuntimeValueCanonicalIdentityV1` | `Unit`<br>`Bool`<br>`Int`<br>`UInt`<br>`F32`<br>`F64`<br>`MatrixF32`<br>`MatrixF64`<br>`TensorF32`<br>`TensorF64`<br>`String`<br>`Char`<br>`Duration`<br>`Progress`<br>`Range`<br>`Iterator`<br>`EntityRef`<br>`Tuple`<br>`Seq`<br>`Record`<br>`NominalRecord`<br>`Opaque`<br>`Reduction`<br>`Agent`<br>`Function`<br>`Variant`<br>`NeedHandle` | exact typed constructor and restore join |
| `AwbcExecutableAuthorityRefV1` | `Program { generation, program_digest, function }` | exact typed constructor and restore join |
| `TaskSpecSnapshotV1` | `CompleteTaskSpec` | exact typed constructor and restore join |
| `NeedProducerTemplateSnapshotV1` | `CompleteNeedProducerTemplate` | exact typed constructor and restore join |
| `HostOperationIdentityV1` | `Builtin(BuiltinHostOperationIdV1)`<br>`Catalog { catalog_digest, operation }` | exact typed constructor and restore join |
| `HostOperationCatalogDigest` | `[u8; 32]` | private representation; validated inherent constructor; no string fallback |
| `HostOperationId` | `NonZeroU32` | private representation; validated inherent constructor; no string fallback |
| `TaskObserverId` | `NonZeroU64` | private representation; validated inherent constructor; no string fallback |
| `TaskObserverKey` | `{ generation, id }` | exact typed constructor and restore join |
| `HostCancelCommandId` | `digest of canonical TaskCorrelation` | private representation; validated inherent constructor; no string fallback |

Every referenced projection in machine data appears in this registry. The validator fails if `RuntimeCheckedTypeProjectionV1`, `RuntimeAgentValueProjectionV1`, `RuntimeDenseSeqProjectionV1`, or any referenced owner is removed.
