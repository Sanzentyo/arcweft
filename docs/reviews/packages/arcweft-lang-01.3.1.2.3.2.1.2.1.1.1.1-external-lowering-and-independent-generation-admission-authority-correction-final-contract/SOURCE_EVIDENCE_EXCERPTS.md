# Focused current-source excerpts

These excerpts are review evidence from the exact files/hashes in `SOURCE_EVIDENCE.csv`.

## `crates/arcweft-core/src/plan.rs` lines 30–66

current raw plan fields/Default surface.

```text
0030:     RuntimeRouteBinding, RuntimeRouteBindingSource, RuntimeRouteSpec,
0031: };
0032: use serde::{Deserialize, Serialize};
0033: use std::fmt;
0034: use std::sync::Arc;
0035: use thiserror::Error;
0036: 
0037: #[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
0038: pub struct RuntimePlan {
0039:     pub entries: Vec<RuntimeEntrySpec>,
0040:     pub callable_executables: Vec<RuntimeCallableExecutable>,
0041:     pub flow_executables: Vec<RuntimeFlowExecutable>,
0042:     pub flows: Vec<RuntimeFlow>,
0043:     pub pure_helpers: Vec<RuntimePureHelper>,
0044:     pub trait_methods: Vec<RuntimeTraitMethod>,
0045:     pub line_task_groups: Vec<LineTaskGroup>,
0046:     pub stream_plans: Vec<StreamPlan>,
0047:     pub source_plans: Vec<SourcePlan>,
0048: }
0049: 
0050: /// Runtime identifier for a lowered flow.
0051: #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
0052: pub struct FlowRuntimeId {
0053:     path: RuntimeIdPath,
0054:     public_label: RuntimePublicLabel,
0055: }
0056: 
0057: /// Dynamic runtime Flow target lookup failure.
0058: ///
0059: /// Runtime-authored text may select an accepted manual canonical identity
0060: /// exactly, or select one checked/generated declaration through its unique
0061: /// public label. It never reconstructs a checked/generated semantic identity.
0062: #[derive(Clone, Debug, Eq, Error, PartialEq)]
0063: pub enum RuntimeFlowTargetError {
0064:     #[error(transparent)]
0065:     Invalid(#[from] RuntimeIdError),
0066:     #[error("runtime Flow target `{target}` is not present in the accepted plan")]
```

## `crates/arcweft-core/src/pattern.rs` lines 277–327

closed checked type algebra.

```text
0277: ///
0278: /// The compiler projects this value once from checked semantic type facts.
0279: /// Native execution and AWBC lowering consume the same typed vocabulary; no
0280: /// source/display label is reparsed at either boundary.
0281: #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
0282: pub enum RuntimeCheckedType {
0283:     Never,
0284:     Unit,
0285:     Bool,
0286:     Signed(RuntimeSignedIntWidth),
0287:     Unsigned(RuntimeUnsignedIntWidth),
0288:     F32,
0289:     F64,
0290:     String,
0291:     Char,
0292:     Duration,
0293:     EntityReference,
0294:     Bytes,
0295:     Sequence(Box<RuntimeCheckedType>),
0296:     Tuple(Vec<RuntimeCheckedType>),
0297:     Choice(Vec<RuntimeCheckedType>),
0298:     Nominal {
0299:         nominal: RuntimeNominalTypeId,
0300:         semantic_identity: RuntimeSemanticTypeId,
0301:         layout: TypeLayoutHash,
0302:     },
0303:     Opaque {
0304:         owner: RuntimeOpaqueTypeOwner,
0305:     },
0306:     Variant {
0307:         nominal: RuntimeNominalTypeId,
0308:         semantic_identity: RuntimeSemanticTypeId,
0309:         cases: Vec<RuntimeCheckedVariantCase>,
0310:     },
0311:     Result {
0312:         ok: Box<RuntimeCheckedType>,
0313:         error: Box<RuntimeCheckedType>,
0314:     },
0315:     Option(Box<RuntimeCheckedType>),
0316: }
0317: 
0318: impl RuntimeCheckedType {
0319:     #[must_use]
0320:     pub fn variant_identity(&self) -> Option<RuntimeVariantIdentity> {
0321:         match self {
0322:             Self::Variant {
0323:                 nominal,
0324:                 semantic_identity,
0325:                 ..
0326:             } => Some(RuntimeVariantIdentity::Nominal {
0327:                 nominal: nominal.clone(),
```

## `crates/arcweft-core/src/pattern.rs` lines 192–230

current owner-only opaque acceptance.

```text
0192:     pub fn accepts_owner(&self, actual: &Self) -> bool {
0193:         self == actual
0194:             || (self.admission == RuntimeOpaqueTypeAdmission::ProducerWide
0195:                 && actual.admission == RuntimeOpaqueTypeAdmission::ExactIdentity
0196:                 && self.producer == actual.producer)
0197:     }
0198: 
0199:     #[must_use]
0200:     pub fn accepts_opaque_value(&self, actual: &RuntimeOpaqueValue) -> bool {
0201:         &self.producer == actual.producer()
0202:             && (self.admission == RuntimeOpaqueTypeAdmission::ProducerWide
0203:                 || self.semantic_identity == actual.semantic_identity())
0204:     }
0205: 
0206:     pub fn try_wrap(&self, payload: RuntimeValue) -> Result<RuntimeValue, RuntimeOpaqueValueError> {
0207:         if self.admission == RuntimeOpaqueTypeAdmission::ProducerWide {
0208:             return Err(RuntimeOpaqueValueError::NonConcreteOwner {
0209:                 producer: self.producer.clone(),
0210:                 semantic_identity: self.semantic_identity,
0211:             });
0212:         }
0213:         Ok(RuntimeValue::Opaque(RuntimeOpaqueValue::new_exact(
0214:             self, payload,
0215:         )))
0216:     }
0217: }
0218: 
0219: /// Closed identity of a runtime variant value after semantic checking.
0220: ///
0221: /// Generic payload types remain on [`RuntimeCheckedType`]. Values retain only
0222: /// the owner family and source-ordered case ordinal, so Option/Result
0223: /// intrinsics never invent erased generic arguments and nominal values never
0224: /// fall back to source-path strings.
0225: #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
0226: pub enum RuntimeVariantIdentity {
0227:     Nominal {
0228:         nominal: RuntimeNominalTypeId,
0229:         semantic_identity: RuntimeSemanticTypeId,
0230:     },
```

## `crates/arcweft-core/src/value.rs` lines 141–193

complete value families including matrix/tensor/range/function.

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
0150:     F32(f32),
0151:     F64(f64),
0152:     MatrixF32(DenseMatrixF32),
0153:     MatrixF64(DenseMatrixF64),
0154:     TensorF32(DenseTensorF32),
0155:     TensorF64(DenseTensorF64),
0156:     String(String),
0157:     Char(char),
0158:     Duration(LogicalDuration),
0159:     Range(RuntimeRange),
0160:     Iterator(RuntimeIterator),
0161:     EntityRef(String),
0162:     Tuple(Vec<RuntimeValue>),
0163:     Seq(RuntimeSeq),
0164:     Record(Vec<RuntimeFieldValue>),
0165:     NominalRecord(RuntimeNominalRecordValue),
0166:     Opaque(RuntimeOpaqueValue),
0167:     Function(RuntimeFunctionValue),
0168:     Variant {
0169:         owner: RuntimeVariantIdentity,
0170:         ordinal: u32,
0171:         name: String,
0172:         payload: Option<Box<RuntimeValue>>,
0173:     },
0174: }
0175: 
0176: /// Runtime call target after syntax lowering.
0177: #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
0178: pub enum RuntimeCallTarget {
0179:     Intrinsic(RuntimeIntrinsic),
0180:     Callable(RuntimeCallableId),
0181: }
0182: 
0183: impl RuntimeCallTarget {
0184:     pub fn try_from_label(label: impl Into<String>) -> Result<Self, RuntimeIdentityError> {
0185:         let label = label.into();
0186:         RuntimeIntrinsic::from_label(&label)
0187:             .map(Self::Intrinsic)
0188:             .map_or_else(|| RuntimeCallableId::try_new(label).map(Self::Callable), Ok)
0189:     }
0190: 
0191:     pub const fn intrinsic(intrinsic: RuntimeIntrinsic) -> Self {
0192:         Self::Intrinsic(intrinsic)
0193:     }
```

## `crates/arcweft-core/src/value.rs` lines 924–1023

expression variants requiring exhaustive node grammar.

```text
0924: }
0925: 
0926: /// Expression subset executable by the Sans I/O flow runtime.
0927: #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
0928: pub enum RuntimeExpr {
0929:     Value(RuntimeValue),
0930:     Local(String),
0931:     EntityRef(String),
0932:     Let {
0933:         name: String,
0934:         expr: Box<RuntimeExpr>,
0935:         body: Box<RuntimeExpr>,
0936:     },
0937:     Tuple(Vec<RuntimeExpr>),
0938:     BracketSeq(Vec<RuntimeExpr>),
0939:     RepeatSeq {
0940:         value: Box<RuntimeExpr>,
0941:         len: usize,
0942:     },
0943:     Range {
0944:         start: Option<Box<RuntimeExpr>>,
0945:         end: Option<Box<RuntimeExpr>>,
0946:         inclusive: bool,
0947:     },
0948:     Record(Vec<RuntimeFieldExpr>),
0949:     NominalRecord(RuntimeNominalRecordExpr),
0950:     Variant {
0951:         owner: RuntimeCheckedType,
0952:         ordinal: u32,
0953:         name: String,
0954:         payload: Option<Box<RuntimeExpr>>,
0955:     },
0956:     Field {
0957:         target: Box<RuntimeExpr>,
0958:         field: String,
0959:     },
0960:     ProjectTuple {
0961:         target: Box<RuntimeExpr>,
0962:         ordinal: usize,
0963:     },
0964:     ProjectRecord {
0965:         target: Box<RuntimeExpr>,
0966:         ordinal: usize,
0967:     },
0968:     AssignField {
0969:         target: Box<RuntimeExpr>,
0970:         field: String,
0971:         expr: Box<RuntimeExpr>,
0972:         body: Box<RuntimeExpr>,
0973:     },
0974:     Call {
0975:         callee: RuntimeCallTarget,
0976:         args: Vec<RuntimeExpr>,
0977:     },
0978:     Function {
0979:         params: Vec<String>,
0980:         body: Box<RuntimeExpr>,
0981:     },
0982:     Apply {
0983:         callee: Box<RuntimeExpr>,
0984:         args: Vec<RuntimeExpr>,
0985:     },
0986:     TraitCall {
0987:         callable: RuntimeTraitMethodId,
0988:         receiver: Box<RuntimeExpr>,
0989:         receiver_mode: RuntimeReceiverMode,
0990:         args: Vec<RuntimeExpr>,
0991:     },
0992:     PureCall {
0993:         helper: RuntimePureHelperId,
0994:         args: Vec<RuntimeExpr>,
0995:     },
0996:     SpreadArg(Box<RuntimeExpr>),
0997:     MethodCall {
0998:         receiver: Box<RuntimeExpr>,
0999:         method: String,
1000:         args: Vec<RuntimeExpr>,
1001:     },
1002:     Map {
1003:         source: Box<RuntimeExpr>,
1004:         param: String,
1005:         body: Box<RuntimeExpr>,
1006:     },
1007:     Filter {
1008:         source: Box<RuntimeExpr>,
1009:         param: String,
1010:         body: Box<RuntimeExpr>,
1011:     },
1012:     Sum {
1013:         source: Box<RuntimeExpr>,
1014:     },
1015:     Unary {
1016:         op: RuntimeUnaryOp,
1017:         expr: Box<RuntimeExpr>,
1018:     },
1019:     Binary {
1020:         lhs: Box<RuntimeExpr>,
1021:         op: RuntimeBinaryOp,
1022:         rhs: Box<RuntimeExpr>,
1023:     },
```

## `crates/arcweft-core/src/awbc/schema.rs` lines 1600–1653

Arg/Const audio values and named command fields.

```text
1600: }
1601: 
1602: /// One evaluated value consumed by a typed AWBC audio command.
1603: #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
1604: #[serde(tag = "kind", content = "value", rename_all = "snake_case")]
1605: pub enum AwbcAudioValueRef {
1606:     /// Runtime value passed through `AwbcInstruction::EmitEffect.args`.
1607:     Arg(AwbcAudioArg),
1608:     /// Canonical constant used by context-free literal line-task effects.
1609:     Const(AwbcConstantId),
1610: }
1611: 
1612: /// Canonical typed AWBC representation of `RuntimeAudioCommand`.
1613: #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
1614: #[serde(tag = "kind", rename_all = "snake_case")]
1615: pub enum AwbcAudioCommand {
1616:     Play {
1617:         voice: AwbcAudioValueRef,
1618:         resource: AwbcAudioValueRef,
1619:         bus: AwbcAudioValueRef,
1620:         gain_db_milli: AwbcAudioValueRef,
1621:         pan_milli: AwbcAudioValueRef,
1622:         loop_mode: AudioLoopMode,
1623:         start_frame: AwbcAudioValueRef,
1624:         fade_in_millis: AwbcAudioValueRef,
1625:     },
1626:     Stop {
1627:         voice: AwbcAudioValueRef,
1628:         fade_out_millis: AwbcAudioValueRef,
1629:     },
1630:     StopAll {
1631:         fade_out_millis: AwbcAudioValueRef,
1632:     },
1633:     SetVoiceGain {
1634:         voice: AwbcAudioValueRef,
1635:         gain_db_milli: AwbcAudioValueRef,
1636:         transition_millis: AwbcAudioValueRef,
1637:     },
1638:     SetVoicePan {
1639:         voice: AwbcAudioValueRef,
1640:         pan_milli: AwbcAudioValueRef,
1641:         transition_millis: AwbcAudioValueRef,
1642:     },
1643:     SetBusGain {
1644:         bus: AwbcAudioValueRef,
1645:         gain_db_milli: AwbcAudioValueRef,
1646:         transition_millis: AwbcAudioValueRef,
1647:     },
1648:     SetBusMute {
1649:         bus: AwbcAudioValueRef,
1650:         muted: AwbcAudioValueRef,
1651:     },
1652:     SetEffectEnabled {
1653:         bus: AwbcAudioValueRef,
```

## `crates/arcweft-core/src/awbc/schema.rs` lines 1753–1786

effect owns signature and optional audio command ID.

```text
1753:         }
1754:     }
1755: }
1756: 
1757: #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
1758: pub struct AwbcEffectPlan {
1759:     pub kind: AwbcEffectKind,
1760:     pub signature: AwbcSignatureId,
1761:     pub capability: Option<AwbcStringId>,
1762:     pub audio: Option<AwbcAudioCommandId>,
1763:     pub static_args: Vec<AwbcConstantId>,
1764:     pub resources: Vec<AwbcResourceAccess>,
1765: }
1766: 
1767: #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
1768: pub enum AwbcEffectKind {
1769:     RegisterHandle,
1770:     DropHandle,
1771:     Wait,
1772:     Audio,
1773:     Call,
1774:     Log,
1775:     SignalWrite,
1776:     MetricWrite,
1777:     EmitEvent,
1778:     Out,
1779:     Return,
1780:     Goto,
1781:     Panic,
1782:     Fail,
1783:     Bail,
1784:     Ensure,
1785:     Assert,
1786:     Close,
```

## `crates/arcweft-runtime-plan/src/semantic_facts.rs` lines 176–221

accepted semantic operational families.

```text
0176:     }
0177: 
0178:     #[must_use]
0179:     pub const fn steps(&self) -> &[RuntimeTypeProjectionStep] {
0180:         &self.0
0181:     }
0182: }
0183: 
0184: /// Closed diagnostic category for shapes outside the checked value algebra.
0185: #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
0186: pub enum RuntimeUnsupportedTypeShape {
0187:     Range,
0188:     Iterator,
0189:     Map,
0190:     Need,
0191:     Stream,
0192:     Source,
0193:     ThreadHandle,
0194:     Shared,
0195:     Reference,
0196:     Function,
0197: }
0198: 
0199: /// Invalid retained identity on a checked project nominal fact.
0200: #[derive(Clone, Debug, Eq, Error, PartialEq)]
0201: pub enum RuntimeResolvedNominalError {
0202:     #[error(transparent)]
0203:     InvalidIdentity(#[from] RuntimeIdentityError),
0204: }
0205: 
0206: /// Failure to project a normalized semantic type into the closed runtime algebra.
0207: #[derive(Clone, Debug, Eq, Error, PartialEq)]
0208: pub enum RuntimeCheckedTypeProjectionError {
0209:     #[error("runtime type `{type_label}` has no opaque producer evidence")]
0210:     MissingOpaqueProducerEvidence {
0211:         semantic_identity: RuntimeSemanticTypeId,
0212:         path: RuntimeTypeProjectionPath,
0213:         type_label: String,
0214:     },
0215:     #[error("runtime type shape `{shape:?}` is not representable")]
0216:     UnsupportedRuntimeShape {
0217:         semantic_identity: RuntimeSemanticTypeId,
0218:         path: RuntimeTypeProjectionPath,
0219:         shape: RuntimeUnsupportedTypeShape,
0220:     },
0221:     #[error("project nominal runtime identity is invalid")]
```
