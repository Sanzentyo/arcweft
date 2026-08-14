# Selected current-source excerpts

All excerpts are from commit `36f83f8509417d1110a34f1b32aee6f4a113dcf3` and are supplementary to the full hashed file rows in `SOURCE_EVIDENCE.csv`.

## crates/arcweft-core/src/value/ownership/path.rs

`crates/arcweft-core/src/value/ownership/path.rs` around `pub struct RuntimeValuePath`:

```text
0007: pub const MAX_RUNTIME_VALUE_PATH_SEGMENTS: u32 = 64;
0008: 
0009: /// Canonical path from a runtime-value graph root to one nested value.
0010: #[derive(Clone, Debug, Eq, Hash, PartialEq)]
0011: pub struct RuntimeValuePath(Box<[RuntimeValuePathSegment]>);
0012: 
0013: /// One canonical edge in a runtime-value graph.
0014: #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
0015: pub enum RuntimeValuePathSegment {
```

`crates/arcweft-core/src/value/ownership/path.rs` around `pub enum RuntimeValuePathSegment`:

```text
0011: pub struct RuntimeValuePath(Box<[RuntimeValuePathSegment]>);
0012: 
0013: /// One canonical edge in a runtime-value graph.
0014: #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
0015: pub enum RuntimeValuePathSegment {
0016:     TupleElement(u32),
0017:     SequenceElement(u64),
0018:     TupleColumn(u32),
0019:     RecordField(RuntimeRecordFieldId),
```

`crates/arcweft-core/src/value/ownership/path.rs` around `pub const MAX_RUNTIME_VALUE_PATH_SEGMENTS`:

```text
0003: use std::cmp::Ordering;
0004: use thiserror::Error;
0005: 
0006: /// Hard maximum for a canonical runtime-value path.
0007: pub const MAX_RUNTIME_VALUE_PATH_SEGMENTS: u32 = 64;
0008: 
0009: /// Canonical path from a runtime-value graph root to one nested value.
0010: #[derive(Clone, Debug, Eq, Hash, PartialEq)]
0011: pub struct RuntimeValuePath(Box<[RuntimeValuePathSegment]>);
```

`crates/arcweft-core/src/value/ownership/path.rs` around `impl Serialize for RuntimeValuePathSegment`:

```text
0209:     }
0210:     value.parse().map_err(E::custom)
0211: }
0212: 
0213: impl Serialize for RuntimeValuePathSegment {
0214:     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
0215:     where
0216:         S: Serializer,
0217:     {
```

`crates/arcweft-core/src/value/ownership/path.rs` around `Vec::<RuntimeValuePathSegment>::deserialize`:

```text
0309:     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
0310:     where
0311:         D: Deserializer<'de>,
0312:     {
0313:         let segments = Vec::<RuntimeValuePathSegment>::deserialize(deserializer)?;
0314:         Self::try_from_segments(segments).map_err(serde::de::Error::custom)
0315:     }
0316: }
0317: 
```

## crates/arcweft-core/src/value.rs

`crates/arcweft-core/src/value.rs` around `pub enum RuntimeValue`:

```text
0141: ///
0142: /// Typed floats use Rust's native `f32`/`f64` values. Exact bit identity is an
0143: /// explicit operation rather than language equality.
0144: #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
0145: pub enum RuntimeValue {
0146:     Unit,
0147:     Bool(bool),
0148:     Int(RuntimeInt),
0149:     UInt(RuntimeUInt),
```

`crates/arcweft-core/src/value.rs` around `MatrixF32`:

```text
0001: use crate::awbc::schema::AwbcFunctionId;
0002: use crate::entry::{
0003:     RuntimeCallableId, RuntimeIdentityError, RuntimeSchemaError, RuntimeValueDigest,
0004: };
0005: use crate::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
0006: use crate::pattern::{RuntimeCheckedType, RuntimePattern, RuntimeVariantIdentity};
0007: use crate::plan::{
0008:     RuntimeIteratorEvidence, RuntimePureHelperId, RuntimePureInputType, RuntimePureOutputType,
0009:     RuntimeReceiverMode, RuntimeTraitMethodId,
```

`crates/arcweft-core/src/value.rs` around `NominalRecord(RuntimeNominalRecordValue)`:

```text
0161:     EntityRef(String),
0162:     Tuple(Vec<RuntimeValue>),
0163:     Seq(RuntimeSeq),
0164:     Record(Vec<RuntimeFieldValue>),
0165:     NominalRecord(RuntimeNominalRecordValue),
0166:     Opaque(RuntimeOpaqueValue),
0167:     Function(RuntimeFunctionValue),
0168:     Variant {
0169:         owner: RuntimeVariantIdentity,
```

## crates/arcweft-core/src/value/nominal_record.rs

`crates/arcweft-core/src/value/nominal_record.rs` around `pub struct RuntimeNominalRecordValue`:

```text
0179: ///
0180: /// Unlike [`RuntimeValue::Record`], the fields have no source names and retain
0181: /// their defining nominal type and exact layout identity.
0182: #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
0183: pub struct RuntimeNominalRecordValue {
0184:     type_id: RuntimeNominalTypeId,
0185:     layout: TypeLayoutHash,
0186:     fields: Vec<RuntimeValue>,
0187: }
```

Pattern not found: `pub fn nominal_id`

Pattern not found: `pub fn layout`

`crates/arcweft-core/src/value/nominal_record.rs` around `try_from_accepted_layout`:

```text
0215: }
0216: 
0217: impl RuntimeNominalRecordValue {
0218:     /// Constructs a value from fields already arranged in defining-layout order.
0219:     pub(crate) fn try_from_accepted_layout(
0220:         layout: &RuntimeNominalRecordLayout,
0221:         fields_in_layout_order: Vec<RuntimeValue>,
0222:     ) -> Result<Self, RuntimeNominalRecordError> {
0223:         validate_layout_fields(layout, &fields_in_layout_order)?;
```

## crates/arcweft-core/src/pattern.rs

`crates/arcweft-core/src/pattern.rs` around `pub enum RuntimeCheckedType`:

```text
0278: /// The compiler projects this value once from checked semantic type facts.
0279: /// Native execution and AWBC lowering consume the same typed vocabulary; no
0280: /// source/display label is reparsed at either boundary.
0281: #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
0282: pub enum RuntimeCheckedType {
0283:     Never,
0284:     Unit,
0285:     Bool,
0286:     Signed(RuntimeSignedIntWidth),
```

`crates/arcweft-core/src/pattern.rs` around `pub fn accepts_value`:

```text
0367:     }
0368: 
0369:     /// Returns whether a runtime value satisfies this exact closed predicate.
0370:     #[must_use]
0371:     pub fn accepts_value(&self, value: &RuntimeValue) -> bool {
0372:         self.accepts_value_at_depth(value, 0)
0373:     }
0374: 
0375:     fn accepts_value_at_depth(&self, value: &RuntimeValue, depth: usize) -> bool {
```

`crates/arcweft-core/src/pattern.rs` around `Self::Choice`:

```text
0403:                         .iter()
0404:                         .zip(items)
0405:                         .all(|(value, item)| item.accepts_value_at_depth(value, depth + 1))
0406:             }
0407:             (value, Self::Choice(alternatives)) => alternatives
0408:                 .iter()
0409:                 .any(|alternative| alternative.accepts_value_at_depth(value, depth + 1)),
0410:             (RuntimeValue::Opaque(value), Self::Opaque { owner }) => {
0411:                 owner.accepts_opaque_value(value)
```

`crates/arcweft-core/src/pattern.rs` around `Self::Result`:

```text
0326:             } => Some(RuntimeVariantIdentity::Nominal {
0327:                 nominal: nominal.clone(),
0328:                 semantic_identity: *semantic_identity,
0329:             }),
0330:             Self::Result { .. } => Some(RuntimeVariantIdentity::Result),
0331:             Self::Option(_) => Some(RuntimeVariantIdentity::Option),
0332:             _ => None,
0333:         }
0334:     }
```

## crates/arcweft-core/src/plan.rs

`crates/arcweft-core/src/plan.rs` around `pub struct RuntimePlan`:

```text
0034: use std::sync::Arc;
0035: use thiserror::Error;
0036: 
0037: #[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
0038: pub struct RuntimePlan {
0039:     pub entries: Vec<RuntimeEntrySpec>,
0040:     pub callable_executables: Vec<RuntimeCallableExecutable>,
0041:     pub flow_executables: Vec<RuntimeFlowExecutable>,
0042:     pub flows: Vec<RuntimeFlow>,
```

`crates/arcweft-core/src/plan.rs` around `pub struct RuntimePureHelper`:

```text
0241: /// Runtime identifier for a lowered deterministic pure helper.
0242: #[derive(
0243:     Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
0244: )]
0245: pub struct RuntimePureHelperId(pub usize);
0246: 
0247: /// Lowered deterministic pure helper callable from runtime expressions.
0248: #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
0249: pub struct RuntimePureHelper {
```

`crates/arcweft-core/src/plan.rs` around `pub enum FlowOp`:

```text
0461: 
0462: /// Runtime identifier for a lowered stream transform.
0463: 
0464: #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
0465: pub enum FlowOp {
0466:     Bind(Vec<RuntimeBinding>),
0467:     Let {
0468:         pattern: RuntimePattern,
0469:         expr: RuntimeExpr,
```

`crates/arcweft-core/src/plan.rs` around `Bind(Vec<RuntimeBinding>)`:

```text
0462: /// Runtime identifier for a lowered stream transform.
0463: 
0464: #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
0465: pub enum FlowOp {
0466:     Bind(Vec<RuntimeBinding>),
0467:     Let {
0468:         pattern: RuntimePattern,
0469:         expr: RuntimeExpr,
0470:     },
```

## crates/arcweft-core/src/awbc/schema.rs

`crates/arcweft-core/src/awbc/schema.rs` around `pub struct AwbcProgram`:

```text
0114: pub struct AwbcDigest(pub [u8; 32]);
0115: 
0116: /// Canonical executable payload. All identifiers are indices into these tables.
0117: #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
0118: pub struct AwbcProgram {
0119:     pub header: AwbcHeader,
0120:     pub strings: Vec<String>,
0121:     pub runtime_types: Vec<AwbcRuntimeType>,
0122:     pub constants: Vec<AwbcConstant>,
```

`crates/arcweft-core/src/awbc/schema.rs` around `pub struct AwbcTaskPlan`:

```text
1529:     Suspend,
1530: }
1531: 
1532: #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
1533: pub struct AwbcTaskPlan {
1534:     pub public_id: AwbcStringId,
1535:     /// Stable need identifier reported at the shared runtime boundary.
1536:     pub need_id: AwbcStringId,
1537:     pub capability: AwbcStringId,
```

`crates/arcweft-core/src/awbc/schema.rs` around `pub enum AwbcAudioCommand`:

```text
1611: 
1612: /// Canonical typed AWBC representation of `RuntimeAudioCommand`.
1613: #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
1614: #[serde(tag = "kind", rename_all = "snake_case")]
1615: pub enum AwbcAudioCommand {
1616:     Play {
1617:         voice: AwbcAudioValueRef,
1618:         resource: AwbcAudioValueRef,
1619:         bus: AwbcAudioValueRef,
```

`crates/arcweft-core/src/awbc/schema.rs` around `pub struct AwbcEffectPlan`:

```text
1754:     }
1755: }
1756: 
1757: #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
1758: pub struct AwbcEffectPlan {
1759:     pub kind: AwbcEffectKind,
1760:     pub signature: AwbcSignatureId,
1761:     pub capability: Option<AwbcStringId>,
1762:     pub audio: Option<AwbcAudioCommandId>,
```

`crates/arcweft-core/src/awbc/schema.rs` around `pub struct AwbcChoice`:

```text
1823:     Or,
1824: }
1825: 
1826: #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
1827: pub struct AwbcChoice {
1828:     pub public_id: Option<AwbcStringId>,
1829:     pub options: AwbcTableRange,
1830: }
1831: 
```

`crates/arcweft-core/src/awbc/schema.rs` around `pub struct AwbcContentUnit`:

```text
1839:     pub effects: Vec<AwbcEffectPlanId>,
1840: }
1841: 
1842: #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
1843: pub struct AwbcContentUnit {
1844:     pub public_id: AwbcStringId,
1845:     pub line_task_group: Option<AwbcLineTaskGroupId>,
1846:     pub display: Option<AwbcDisplayMapId>,
1847:     pub source: Option<AwbcSourceMapId>,
```

## crates/arcweft-lang-sema/src/registration/model.rs

`crates/arcweft-lang-sema/src/registration/model.rs` around `pub struct RegisteredTypeCheckEnv`:

```text
0205:     pub(crate) externals: Vec<(ExternalDeclarationId, SymbolPath, CharacterId)>,
0206: }
0207: 
0208: #[derive(Clone, Debug)]
0209: pub struct RegisteredTypeCheckEnv {
0210:     pub(crate) nominal_world: Arc<AcceptedNominalWorld>,
0211:     pub(crate) character_dialogue_fields: Arc<CharacterDialogueCustomFieldRegistry>,
0212:     pub(crate) rust_metadata: Arc<AcceptedRustTypeMetadataCatalog>,
0213:     pub(crate) callables: Arc<RegisteredCallableCatalog>,
```

`crates/arcweft-lang-sema/src/registration/model.rs` around `pub struct AcceptedNominalWorld`:

```text
0224: /// This carrier owns the exact environment facts and external-owner mapping
0225: /// needed by authored type resolution without depending on the callable
0226: /// catalog whose signatures are being built.
0227: #[derive(Clone, Debug)]
0228: pub struct AcceptedNominalWorld {
0229:     base: Arc<TypeCheckEnv>,
0230:     external_owners: ExternalOwnerRegistry,
0231:     visibility: Arc<AcceptedNominalVisibilityIndex>,
0232: }
```

`crates/arcweft-lang-sema/src/registration/model.rs` around `pub fn nominal_world`:

```text
0780:     pub const fn symbol_lease(&self) -> &Arc<ProjectSymbolTable> {
0781:         &self.symbols
0782:     }
0783: 
0784:     pub fn nominal_world(&self) -> &AcceptedNominalWorld {
0785:         &self.nominal_world
0786:     }
0787: }
0788: 
```

`crates/arcweft-lang-sema/src/registration/model.rs` around `pub fn typecheck_env`:

```text
0912:         self.nominal_world.environment_binding(id)
0913:     }
0914: 
0915:     /// Exact base type-check environment accepted with this registered world.
0916:     pub fn typecheck_env(&self) -> &TypeCheckEnv {
0917:         self.nominal_world.typecheck_env()
0918:     }
0919: 
0920:     /// Immutable exact/open nominal catalog accepted with this semantic world.
```

## crates/arcweft-lang-sema/src/env/base.rs

`crates/arcweft-lang-sema/src/env/base.rs` around `pub struct TypeCheckEnv`:

```text
0114: }
0115: 
0116: /// Small, explicit environment used to validate that HIR can feed type checking.
0117: #[derive(Clone, Debug, Default, Eq, PartialEq)]
0118: pub struct TypeCheckEnv {
0119:     pub(crate) nominal_catalog: AcceptedNominalCatalog,
0120:     pub(crate) symbols: HashMap<String, TypeKind>,
0121:     closed_enums: HashMap<TypeKind, EnvironmentEnumSchema>,
0122:     pub(crate) standard_functions: Vec<StandardEnvironmentFunction>,
```

## crates/arcweft-dialogue/Cargo.toml

`crates/arcweft-dialogue/Cargo.toml` around `arcweft-character`:

```text
0006: license.workspace = true
0007: repository.workspace = true
0008: 
0009: [dependencies]
0010: arcweft-character = { workspace = true }
0011: arcweft-core = { workspace = true }
0012: arcweft-id = { workspace = true }
0013: arcweft-interaction-model.workspace = true
0014: arcweft-ref = { workspace = true }
```

`crates/arcweft-dialogue/Cargo.toml` around `arcweft-core`:

```text
0007: repository.workspace = true
0008: 
0009: [dependencies]
0010: arcweft-character = { workspace = true }
0011: arcweft-core = { workspace = true }
0012: arcweft-id = { workspace = true }
0013: arcweft-interaction-model.workspace = true
0014: arcweft-ref = { workspace = true }
0015: arcweft-resource-model.workspace = true
```

`crates/arcweft-dialogue/Cargo.toml` around `arcweft-view`:

```text
0014: arcweft-ref = { workspace = true }
0015: arcweft-resource-model.workspace = true
0016: arcweft-rich-text-schema = { workspace = true }
0017: arcweft-source = { workspace = true }
0018: arcweft-view = { workspace = true }
0019: serde = { workspace = true, features = ["derive"] }
0020: thiserror.workspace = true
0021: 
0022: [dev-dependencies]
```
