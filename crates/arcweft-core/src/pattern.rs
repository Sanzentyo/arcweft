use crate::entry::{RuntimeIdentityError, RuntimeNominalTypeId, TypeLayoutHash};
use crate::plan::{
    RuntimePlan, RuntimePlanTypeDeclaration, RuntimePlanTypeProjection, RuntimePlanValueTypeError,
};
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};
use crate::value::{
    RuntimeEntityReference, RuntimeLocalBinding, RuntimeNominalRecordValue,
    RuntimeOpaquePersistence, RuntimeOpaqueValue, RuntimeOpaqueValueClass, RuntimeOpaqueValueError,
    RuntimeRecordFieldId, RuntimeSeq, RuntimeSignedIntWidth, RuntimeUnsignedIntWidth, RuntimeValue,
};
pub use arcweft_id::RuntimeSemanticTypeId;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

mod binding;

pub use binding::{
    MAX_RUNTIME_PATTERN_BINDING_DEPTH, RuntimePatternBindingCoordinate,
    RuntimePatternBindingCoordinateError, RuntimePatternBindingPath,
    RuntimePatternBindingPathError, RuntimePatternBindingStep, RuntimePatternBindingWireError,
};

/// Canonical semantic-type identity encoder shared by semantic producers.
///
/// The domain is fixed at version 1. Producers own their typed fragments while
/// this lower-layer encoder owns the common byte grammar and digest algorithm.
pub struct RuntimeSemanticTypeIdentityEncoder(blake3::Hasher);

impl RuntimeSemanticTypeIdentityEncoder {
    #[must_use]
    pub fn new() -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.semantic-type.identity.v1\0");
        Self(hasher)
    }

    pub fn write_tag(&mut self, value: u16) {
        self.write_u16(value);
    }

    pub fn write_u8(&mut self, value: u8) {
        self.0.update(&[value]);
    }

    pub fn write_u16(&mut self, value: u16) {
        self.0.update(&value.to_le_bytes());
    }

    pub fn write_u32(&mut self, value: u32) {
        self.0.update(&value.to_le_bytes());
    }

    pub fn write_u64(&mut self, value: u64) {
        self.0.update(&value.to_le_bytes());
    }

    /// Writes a sequence length in the canonical `u32` representation.
    ///
    /// # Panics
    ///
    /// Panics when `value` exceeds the semantic identity grammar's `u32`
    /// sequence-length bound.
    pub fn write_len(&mut self, value: usize) {
        self.write_u32(
            u32::try_from(value).expect("accepted semantic sequences fit the u32 contract"),
        );
    }

    pub fn write_str(&mut self, value: &str) {
        self.write_len(value.len());
        self.write_bytes(value.as_bytes());
    }

    pub fn write_bytes(&mut self, value: &[u8]) {
        self.0.update(value);
    }

    #[must_use]
    pub fn finish(self) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes(*self.0.finalize().as_bytes())
    }
}

impl Default for RuntimeSemanticTypeIdentityEncoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Stable identity of a producer that validates opaque runtime payloads.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct RuntimeOpaqueTypeProducerId(RuntimeNominalTypeId);

impl RuntimeOpaqueTypeProducerId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, RuntimeIdentityError> {
        RuntimeNominalTypeId::try_new(value).map(Self)
    }

    #[must_use]
    pub const fn from_nominal(value: RuntimeNominalTypeId) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn nominal(&self) -> &RuntimeNominalTypeId {
        &self.0
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<'de> Deserialize<'de> for RuntimeOpaqueTypeProducerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

/// One language-standard opaque runtime carrier shared by semantic checking
/// and host-manifest projection.
///
/// The source-visible path and runtime producer are one closed relation. A
/// consumer must not reconstruct the producer from a display name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeStandardOpaqueTypeSpec {
    path: &'static [&'static str],
    arity: u16,
    producer: &'static str,
}

impl RuntimeStandardOpaqueTypeSpec {
    const fn new(path: &'static [&'static str], arity: u16, producer: &'static str) -> Self {
        Self {
            path,
            arity,
            producer,
        }
    }

    #[must_use]
    pub const fn path(&self) -> &'static [&'static str] {
        self.path
    }

    #[must_use]
    pub const fn arity(&self) -> u16 {
        self.arity
    }

    #[must_use]
    pub const fn producer(&self) -> &'static str {
        self.producer
    }

    #[must_use]
    pub const fn value_class(&self) -> RuntimeOpaqueValueClass {
        RuntimeOpaqueValueClass::Plain
    }

    #[must_use]
    pub const fn persistence(&self) -> RuntimeOpaquePersistence {
        RuntimeOpaquePersistence::ConstantAndSnapshot
    }
}

pub const RUNTIME_STANDARD_REDUCTION: RuntimeStandardOpaqueTypeSpec =
    RuntimeStandardOpaqueTypeSpec::new(&["Reduction"], 1, "std.reduction");
pub const RUNTIME_STANDARD_AGENT_ERROR: RuntimeStandardOpaqueTypeSpec =
    RuntimeStandardOpaqueTypeSpec::new(&["AgentError"], 0, "std.agent_error");

/// Closed standard opaque inventory. Both semantic catalogs and external
/// adapter references consume this inventory, so producer identity cannot
/// diverge between those boundaries.
pub const RUNTIME_STANDARD_OPAQUE_TYPES: [RuntimeStandardOpaqueTypeSpec; 13] = [
    RUNTIME_STANDARD_REDUCTION,
    RuntimeStandardOpaqueTypeSpec::new(&["Watch"], 1, "std.watch"),
    RuntimeStandardOpaqueTypeSpec::new(&["Sample"], 1, "std.sample"),
    RuntimeStandardOpaqueTypeSpec::new(&["VirtualPath"], 0, "std.virtual_path"),
    RuntimeStandardOpaqueTypeSpec::new(&["ArcError"], 0, "std.arc_error"),
    RuntimeStandardOpaqueTypeSpec::new(&["ReducerError"], 0, "std.reducer_error"),
    RUNTIME_STANDARD_AGENT_ERROR,
    RuntimeStandardOpaqueTypeSpec::new(&["AssetError"], 0, "std.asset_error"),
    RuntimeStandardOpaqueTypeSpec::new(&["ContentLoadError"], 0, "std.content_load_error"),
    RuntimeStandardOpaqueTypeSpec::new(&["DialogueText"], 0, "std.dialogue_text"),
    RuntimeStandardOpaqueTypeSpec::new(&["ImageHandle"], 0, "std.image_handle"),
    RuntimeStandardOpaqueTypeSpec::new(&["PresentationLifetime"], 0, "std.presentation_lifetime"),
    RuntimeStandardOpaqueTypeSpec::new(&["VoiceError"], 0, "std.voice_error"),
];

/// Resolves one exact standard opaque source path.
#[must_use]
pub fn runtime_standard_opaque_type(
    path: &[&str],
) -> Option<&'static RuntimeStandardOpaqueTypeSpec> {
    RUNTIME_STANDARD_OPAQUE_TYPES
        .iter()
        .find(|spec| spec.path() == path)
}

/// Whether an opaque checked owner names one exact type or a producer-defined top.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
pub enum RuntimeOpaqueTypeAdmission {
    ExactIdentity = 0,
    ProducerWide = 1,
}

/// Closed checked owner for one producer-validated opaque type.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeOpaqueTypeOwner {
    producer: RuntimeOpaqueTypeProducerId,
    semantic_identity: RuntimeSemanticTypeId,
    admission: RuntimeOpaqueTypeAdmission,
    value_class: RuntimeOpaqueValueClass,
    persistence: RuntimeOpaquePersistence,
}

impl RuntimeOpaqueTypeOwner {
    #[must_use]
    pub const fn with_admission(
        producer: RuntimeOpaqueTypeProducerId,
        semantic_identity: RuntimeSemanticTypeId,
        admission: RuntimeOpaqueTypeAdmission,
        value_class: RuntimeOpaqueValueClass,
        persistence: RuntimeOpaquePersistence,
    ) -> Self {
        Self {
            producer,
            semantic_identity,
            admission,
            value_class,
            persistence,
        }
    }

    #[must_use]
    pub const fn exact(
        producer: RuntimeOpaqueTypeProducerId,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Self {
        Self::exact_with(
            producer,
            semantic_identity,
            RuntimeOpaqueValueClass::Plain,
            RuntimeOpaquePersistence::ConstantAndSnapshot,
        )
    }

    #[must_use]
    pub const fn exact_with(
        producer: RuntimeOpaqueTypeProducerId,
        semantic_identity: RuntimeSemanticTypeId,
        value_class: RuntimeOpaqueValueClass,
        persistence: RuntimeOpaquePersistence,
    ) -> Self {
        Self::with_admission(
            producer,
            semantic_identity,
            RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class,
            persistence,
        )
    }

    #[must_use]
    pub const fn producer_wide(
        producer: RuntimeOpaqueTypeProducerId,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Self {
        Self::producer_wide_with(
            producer,
            semantic_identity,
            RuntimeOpaqueValueClass::Plain,
            RuntimeOpaquePersistence::ConstantAndSnapshot,
        )
    }

    #[must_use]
    pub const fn producer_wide_with(
        producer: RuntimeOpaqueTypeProducerId,
        semantic_identity: RuntimeSemanticTypeId,
        value_class: RuntimeOpaqueValueClass,
        persistence: RuntimeOpaquePersistence,
    ) -> Self {
        Self::with_admission(
            producer,
            semantic_identity,
            RuntimeOpaqueTypeAdmission::ProducerWide,
            value_class,
            persistence,
        )
    }

    #[must_use]
    pub const fn producer(&self) -> &RuntimeOpaqueTypeProducerId {
        &self.producer
    }

    #[must_use]
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        self.semantic_identity
    }

    #[must_use]
    pub const fn admission(&self) -> RuntimeOpaqueTypeAdmission {
        self.admission
    }

    #[must_use]
    pub const fn value_class(&self) -> RuntimeOpaqueValueClass {
        self.value_class
    }

    #[must_use]
    pub const fn persistence(&self) -> RuntimeOpaquePersistence {
        self.persistence
    }

    #[must_use]
    pub fn accepts_owner(&self, actual: &Self) -> bool {
        self == actual
            || (self.admission == RuntimeOpaqueTypeAdmission::ProducerWide
                && actual.admission == RuntimeOpaqueTypeAdmission::ExactIdentity
                && self.producer == actual.producer
                && self.value_class == actual.value_class
                && self.persistence == actual.persistence)
    }

    #[must_use]
    pub fn accepts_opaque_value(&self, actual: &RuntimeOpaqueValue) -> bool {
        &self.producer == actual.producer()
            && self.value_class == actual.value_class()
            && self.persistence == actual.persistence()
            && (self.admission == RuntimeOpaqueTypeAdmission::ProducerWide
                || self.semantic_identity == actual.semantic_identity())
    }

    pub fn try_wrap(&self, payload: RuntimeValue) -> Result<RuntimeValue, RuntimeOpaqueValueError> {
        if self.admission == RuntimeOpaqueTypeAdmission::ProducerWide {
            return Err(RuntimeOpaqueValueError::NonConcreteOwner {
                producer: self.producer.clone(),
                semantic_identity: self.semantic_identity,
            });
        }
        Ok(RuntimeValue::Opaque(RuntimeOpaqueValue::new_exact(
            self, payload,
        )))
    }
}

/// Closed identity of a runtime variant value after semantic checking.
///
/// Generic payload types remain on [`RuntimeCheckedType`]. Values retain only
/// the owner family and source-ordered case ordinal, so Option/Result
/// intrinsics never invent erased generic arguments and nominal values never
/// fall back to source-path strings.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeVariantIdentity {
    Nominal {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
    },
    Builtin(RuntimeBuiltinVariantIdentity),
}

/// Core-owned identity and canonical case schema for standard runtime
/// variants. Every builtin variant boundary uses this same owner; codecs and
/// host adapters must not reconstruct it from case strings.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeBuiltinVariantIdentity {
    Option = 0,
    Result = 1,
    AgentResourceBody = 2,
    AgentBinaryEncoding = 3,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeBuiltinVariantCaseSchema {
    identity: RuntimeBuiltinVariantCaseIdentity,
    name: &'static str,
    has_payload: bool,
}

impl RuntimeBuiltinVariantCaseSchema {
    const fn new(
        identity: RuntimeBuiltinVariantCaseIdentity,
        name: &'static str,
        has_payload: bool,
    ) -> Self {
        Self {
            identity,
            name,
            has_payload,
        }
    }

    pub const fn identity(self) -> RuntimeBuiltinVariantCaseIdentity {
        self.identity
    }

    pub const fn name(self) -> &'static str {
        self.name
    }

    pub const fn has_payload(self) -> bool {
        self.has_payload
    }
}

/// Semantic identity of one case in a core-owned builtin variant.
///
/// Callers use this coordinate instead of copying case ordinals or names.
/// The source-ordered schema slice owned by [`RuntimeBuiltinVariantIdentity`]
/// remains the sole ordinal authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeBuiltinVariantCaseIdentity {
    OptionSome,
    OptionNone,
    ResultOk,
    ResultErr,
    AgentResourceBodyJson,
    AgentResourceBodyText,
    AgentResourceBodyBytesBase64,
    AgentBinaryEncodingBase64,
}

impl RuntimeBuiltinVariantCaseIdentity {
    #[must_use]
    pub const fn owner(self) -> RuntimeBuiltinVariantIdentity {
        match self {
            Self::OptionSome | Self::OptionNone => RuntimeBuiltinVariantIdentity::Option,
            Self::ResultOk | Self::ResultErr => RuntimeBuiltinVariantIdentity::Result,
            Self::AgentResourceBodyJson
            | Self::AgentResourceBodyText
            | Self::AgentResourceBodyBytesBase64 => {
                RuntimeBuiltinVariantIdentity::AgentResourceBody
            }
            Self::AgentBinaryEncodingBase64 => RuntimeBuiltinVariantIdentity::AgentBinaryEncoding,
        }
    }
}

const OPTION_CASES: [RuntimeBuiltinVariantCaseSchema; 2] = [
    RuntimeBuiltinVariantCaseSchema::new(
        RuntimeBuiltinVariantCaseIdentity::OptionSome,
        "Some",
        true,
    ),
    RuntimeBuiltinVariantCaseSchema::new(
        RuntimeBuiltinVariantCaseIdentity::OptionNone,
        "None",
        false,
    ),
];
const RESULT_CASES: [RuntimeBuiltinVariantCaseSchema; 2] = [
    RuntimeBuiltinVariantCaseSchema::new(RuntimeBuiltinVariantCaseIdentity::ResultOk, "Ok", true),
    RuntimeBuiltinVariantCaseSchema::new(RuntimeBuiltinVariantCaseIdentity::ResultErr, "Err", true),
];
const AGENT_RESOURCE_BODY_CASES: [RuntimeBuiltinVariantCaseSchema; 3] = [
    RuntimeBuiltinVariantCaseSchema::new(
        RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyJson,
        "Json",
        true,
    ),
    RuntimeBuiltinVariantCaseSchema::new(
        RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyText,
        "Text",
        true,
    ),
    RuntimeBuiltinVariantCaseSchema::new(
        RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyBytesBase64,
        "BytesBase64",
        true,
    ),
];
const AGENT_BINARY_ENCODING_CASES: [RuntimeBuiltinVariantCaseSchema; 1] =
    [RuntimeBuiltinVariantCaseSchema::new(
        RuntimeBuiltinVariantCaseIdentity::AgentBinaryEncodingBase64,
        "Base64",
        false,
    )];

impl RuntimeBuiltinVariantIdentity {
    const COUNT: usize = Self::AgentBinaryEncoding as usize + 1;
    const DECODE: [Option<Self>; RuntimeBuiltinVariantIdentity::COUNT] = {
        let mut decode = [None; RuntimeBuiltinVariantIdentity::COUNT];
        decode[Self::Option as usize] = Some(Self::Option);
        decode[Self::Result as usize] = Some(Self::Result);
        decode[Self::AgentResourceBody as usize] = Some(Self::AgentResourceBody);
        decode[Self::AgentBinaryEncoding as usize] = Some(Self::AgentBinaryEncoding);
        decode
    };

    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub fn from_wire_tag(tag: u8) -> Option<Self> {
        Self::DECODE.get(tag as usize).copied().flatten()
    }

    #[must_use]
    pub const fn cases(self) -> &'static [RuntimeBuiltinVariantCaseSchema] {
        match self {
            Self::Option => &OPTION_CASES,
            Self::Result => &RESULT_CASES,
            Self::AgentResourceBody => &AGENT_RESOURCE_BODY_CASES,
            Self::AgentBinaryEncoding => &AGENT_BINARY_ENCODING_CASES,
        }
    }

    /// Resolves a semantic case coordinate to its canonical source ordinal
    /// and schema row.
    #[must_use]
    pub fn resolve_case(
        self,
        identity: RuntimeBuiltinVariantCaseIdentity,
    ) -> Option<(u32, RuntimeBuiltinVariantCaseSchema)> {
        if identity.owner() != self {
            return None;
        }
        self.cases()
            .iter()
            .copied()
            .enumerate()
            .find(|(_, schema)| schema.identity() == identity)
            .and_then(|(ordinal, schema)| {
                u32::try_from(ordinal).ok().map(|ordinal| (ordinal, schema))
            })
    }

    /// Resolves one runtime ordinal through the canonical schema table.
    #[must_use]
    pub fn case_at(self, ordinal: u32) -> Option<RuntimeBuiltinVariantCaseSchema> {
        usize::try_from(ordinal)
            .ok()
            .and_then(|ordinal| self.cases().get(ordinal))
            .copied()
    }
}

/// Rejection produced while sealing a checked type for one core-owned
/// builtin variant family.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeBuiltinVariantTypeError {
    #[error(
        "builtin variant {owner:?} requires {expected} cases, but {actual} payload rows were supplied"
    )]
    CaseCount {
        owner: RuntimeBuiltinVariantIdentity,
        expected: usize,
        actual: usize,
    },
    #[error("builtin variant case {case:?} has the wrong payload presence")]
    InvalidPayloadPresence {
        case: RuntimeBuiltinVariantCaseIdentity,
    },
}

/// One source-ordered case in a checked nominal enum.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeCheckedVariantCase {
    pub name: String,
    pub payload: Option<Box<RuntimeCheckedType>>,
}

/// One recursively typed pattern admitted into a runtime plan.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePattern {
    ty: RuntimePlanTypeId,
    kind: RuntimePatternKind,
}

impl RuntimePattern {
    pub(crate) const fn from_admitted_parts(
        ty: RuntimePlanTypeId,
        kind: RuntimePatternKind,
    ) -> Self {
        Self { ty, kind }
    }

    #[must_use]
    pub const fn ty(&self) -> RuntimePlanTypeId {
        self.ty
    }

    #[must_use]
    pub const fn kind(&self) -> &RuntimePatternKind {
        &self.kind
    }
}

/// Closed executable pattern algebra. Local destinations are plan-local IDs
/// paired with coordinates derived by the aggregate plan builder.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimePatternKind {
    Bind {
        mutable: bool,
        binding: RuntimePatternBindingCoordinate,
    },
    Discard,
    Literal(RuntimeValue),
    Entity(RuntimeEntityReference),
    Tuple(Box<[RuntimePattern]>),
    Record {
        fields: Box<[RuntimeRecordPatternField]>,
        rest: RuntimePatternRest,
    },
    Sequence {
        items: Box<[RuntimePattern]>,
        rest: RuntimePatternRest,
    },
    Variant {
        ordinal: u32,
        payload: Option<Box<RuntimePattern>>,
    },
    Whole {
        binding: RuntimePatternBindingCoordinate,
        pattern: Box<RuntimePattern>,
    },
    Typed {
        binding: RuntimePatternBindingCoordinate,
    },
}

/// Exact, open, or binding remainder semantics shared by structural patterns.
///
/// A record rest binding receives the original complete record. A bracket-
/// sequence rest binding receives only the unmatched tail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimePatternRest {
    Exact,
    Ignore,
    Bind(RuntimePatternBindingCoordinate),
}

impl RuntimePatternRest {
    /// Returns whether `actual` elements/fields satisfy this remainder mode.
    #[must_use]
    pub const fn accepts_len(&self, required: usize, actual: usize) -> bool {
        match self {
            Self::Exact => required == actual,
            Self::Ignore | Self::Bind(_) => required <= actual,
        }
    }

    /// Returns the admitted local coordinate written by a binding rest.
    #[must_use]
    pub const fn binding(&self) -> Option<&RuntimePatternBindingCoordinate> {
        match self {
            Self::Bind(binding) => Some(binding),
            Self::Exact | Self::Ignore => None,
        }
    }
}

/// Closed structural type predicate for a runtime typed-binding pattern.
///
/// The compiler projects this value once from checked semantic type facts.
/// Native execution and AWBC lowering consume the same typed vocabulary; no
/// source/display label is reparsed at either boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeCheckedType {
    Never,
    Unit,
    Bool,
    Signed(RuntimeSignedIntWidth),
    Unsigned(RuntimeUnsignedIntWidth),
    F32,
    F64,
    String,
    Char,
    Duration,
    Progress,
    EntityReference,
    AgentValue,
    Bytes,
    Sequence(Box<RuntimeCheckedType>),
    Tuple(Vec<RuntimeCheckedType>),
    Choice(Vec<RuntimeCheckedType>),
    Nominal {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
        arguments: Vec<RuntimeCheckedType>,
    },
    Opaque {
        owner: RuntimeOpaqueTypeOwner,
    },
    Variant {
        owner: RuntimeVariantIdentity,
        arguments: Vec<RuntimeCheckedType>,
        cases: Vec<RuntimeCheckedVariantCase>,
    },
    Result {
        ok: Box<RuntimeCheckedType>,
        error: Box<RuntimeCheckedType>,
    },
    Option(Box<RuntimeCheckedType>),
    Agent(crate::plan::RuntimeAgentOperationalType),
}

impl RuntimeCheckedType {
    /// Seals one core-owned builtin variant type through its canonical case
    /// schema. Callers provide payload types only; names and ordinals remain
    /// owned by [`RuntimeBuiltinVariantIdentity`].
    pub fn try_builtin_variant(
        owner: RuntimeBuiltinVariantIdentity,
        payloads: impl IntoIterator<Item = Option<RuntimeCheckedType>>,
    ) -> Result<Self, RuntimeBuiltinVariantTypeError> {
        let payloads = payloads.into_iter().collect::<Vec<_>>();
        let schemas = owner.cases();
        if payloads.len() != schemas.len() {
            return Err(RuntimeBuiltinVariantTypeError::CaseCount {
                owner,
                expected: schemas.len(),
                actual: payloads.len(),
            });
        }
        let cases = schemas
            .iter()
            .copied()
            .zip(payloads)
            .map(|(schema, payload)| {
                if schema.has_payload() != payload.is_some() {
                    return Err(RuntimeBuiltinVariantTypeError::InvalidPayloadPresence {
                        case: schema.identity(),
                    });
                }
                Ok(RuntimeCheckedVariantCase {
                    name: schema.name().to_owned(),
                    payload: payload.map(Box::new),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::Variant {
            owner: RuntimeVariantIdentity::Builtin(owner),
            arguments: Vec::new(),
            cases,
        })
    }

    /// Appends this checked type's canonical structural transcript to an
    /// enclosing semantic-type identity.
    ///
    /// This is the single transcript authority for checked runtime types.
    /// Higher-layer semantic owners use it when a checked type is either the
    /// complete identity or one structural child; they must not duplicate the
    /// checked-type tag grammar.
    pub fn encode_semantic_identity(&self, encoder: &mut RuntimeSemanticTypeIdentityEncoder) {
        write_checked_type_identity(encoder, self);
    }

    /// Returns the canonical, source-independent digest of this checked type.
    /// The encoder is structural and deliberately ignores debug/display
    /// formatting so producers can share one semantic identity boundary.
    #[must_use]
    pub fn semantic_identity_digest(&self) -> RuntimeSemanticTypeId {
        let mut encoder = RuntimeSemanticTypeIdentityEncoder::new();
        self.encode_semantic_identity(&mut encoder);
        encoder.finish()
    }

    #[must_use]
    pub fn variant_identity(&self) -> Option<RuntimeVariantIdentity> {
        match self {
            Self::Variant { owner, .. } => Some(owner.clone()),
            Self::Result { .. } => Some(RuntimeVariantIdentity::Builtin(
                RuntimeBuiltinVariantIdentity::Result,
            )),
            Self::Option(_) => Some(RuntimeVariantIdentity::Builtin(
                RuntimeBuiltinVariantIdentity::Option,
            )),
            _ => None,
        }
    }

    #[must_use]
    pub fn variant_case(&self, ordinal: u32) -> Option<RuntimeCheckedVariantCase> {
        match self {
            Self::Variant { cases, .. } => usize::try_from(ordinal)
                .ok()
                .and_then(|ordinal| cases.get(ordinal))
                .cloned(),
            Self::Result { ok, error } => {
                let schema = RuntimeBuiltinVariantIdentity::Result.case_at(ordinal)?;
                let payload = match schema.identity() {
                    RuntimeBuiltinVariantCaseIdentity::ResultOk => Some(ok.clone()),
                    RuntimeBuiltinVariantCaseIdentity::ResultErr => Some(error.clone()),
                    _ => return None,
                };
                Some(RuntimeCheckedVariantCase {
                    name: schema.name().to_owned(),
                    payload,
                })
            }
            Self::Option(item) => {
                let schema = RuntimeBuiltinVariantIdentity::Option.case_at(ordinal)?;
                let payload = match schema.identity() {
                    RuntimeBuiltinVariantCaseIdentity::OptionSome => Some(item.clone()),
                    RuntimeBuiltinVariantCaseIdentity::OptionNone => None,
                    _ => return None,
                };
                Some(RuntimeCheckedVariantCase {
                    name: schema.name().to_owned(),
                    payload,
                })
            }
            _ => None,
        }
    }

    /// Returns whether a runtime value satisfies this exact closed predicate.
    #[must_use]
    pub fn accepts_value(&self, value: &RuntimeValue) -> bool {
        self.accepts_value_at_depth(value, 0)
    }

    fn accepts_value_at_depth(&self, value: &RuntimeValue, depth: usize) -> bool {
        if depth > crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH {
            return false;
        }
        match (value, self) {
            (RuntimeValue::Unit, Self::Unit)
            | (RuntimeValue::Bool(_), Self::Bool)
            | (RuntimeValue::F32(_), Self::F32)
            | (RuntimeValue::F64(_), Self::F64)
            | (RuntimeValue::String(_), Self::String)
            | (RuntimeValue::Char(_), Self::Char)
            | (RuntimeValue::Duration(_), Self::Duration)
            | (RuntimeValue::Progress(_), Self::Progress)
            | (RuntimeValue::EntityRef(_), Self::EntityReference) => true,
            (value, Self::AgentValue) => runtime_value_is_agent_value(value, depth),
            (RuntimeValue::Int(value), Self::Signed(width)) => value.width() == *width,
            (RuntimeValue::UInt(value), Self::Unsigned(width)) => value.width() == *width,
            (RuntimeValue::Seq(sequence), Self::Bytes) => sequence
                .clone()
                .into_values()
                .iter()
                .all(|value| matches!(value, RuntimeValue::UInt(value) if value.width() == RuntimeUnsignedIntWidth::U8)),
            (RuntimeValue::Seq(sequence), Self::Sequence(item)) => sequence
                .clone()
                .into_values()
                .iter()
                .all(|value| item.accepts_value_at_depth(value, depth + 1)),
            (RuntimeValue::Tuple(values), Self::Tuple(items)) => {
                values.len() == items.len()
                    && values
                        .iter()
                        .zip(items)
                        .all(|(value, item)| item.accepts_value_at_depth(value, depth + 1))
            }
            (value, Self::Choice(alternatives)) => alternatives
                .iter()
                .any(|alternative| alternative.accepts_value_at_depth(value, depth + 1)),
            (RuntimeValue::Opaque(value), Self::Opaque { owner }) => {
                owner.accepts_opaque_value(value)
            }
            (RuntimeValue::Reduction(value), Self::Opaque { owner }) => {
                owner.accepts_owner(value.owner())
            }
            (
                RuntimeValue::NominalRecord(record),
                Self::Nominal {
                    nominal, layout, ..
                },
            ) => record.type_id() == nominal && record.layout() == *layout,
            (
                RuntimeValue::Variant {
                    owner,
                    ordinal,
                    name,
                    payload,
                },
                Self::Variant { .. },
            ) => self.accepts_nominal_variant_at_depth(
                owner,
                *ordinal,
                name,
                payload.as_deref(),
                depth,
            ),
            (value @ RuntimeValue::Variant { .. }, Self::Result { ok, error }) => {
                match value.builtin_variant_case() {
                    Some((RuntimeBuiltinVariantCaseIdentity::ResultOk, Some(value))) => {
                        ok.accepts_value_at_depth(value, depth + 1)
                    }
                    Some((RuntimeBuiltinVariantCaseIdentity::ResultErr, Some(value))) => {
                        error.accepts_value_at_depth(value, depth + 1)
                    }
                    _ => false,
                }
            }
            (value @ RuntimeValue::Variant { .. }, Self::Option(item)) => {
                match value.builtin_variant_case() {
                    Some((RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value))) => {
                        item.accepts_value_at_depth(value, depth + 1)
                    }
                    Some((RuntimeBuiltinVariantCaseIdentity::OptionNone, None)) => true,
                    _ => false,
                }
            }
            (RuntimeValue::Agent(value), Self::Agent(expected)) => {
                value.operational_type() == *expected
            }
            (RuntimeValue::Record(_), Self::Agent(expected)) => expected.accepts_protocol_record(),
            _ => false,
        }
    }

    fn accepts_nominal_variant_at_depth(
        &self,
        owner: &RuntimeVariantIdentity,
        ordinal: u32,
        name: &str,
        payload: Option<&RuntimeValue>,
        depth: usize,
    ) -> bool {
        let Self::Variant {
            owner: expected_owner,
            cases,
            ..
        } = self
        else {
            return false;
        };
        if owner != expected_owner {
            return false;
        }
        usize::try_from(ordinal)
            .ok()
            .and_then(|ordinal| cases.get(ordinal))
            .is_some_and(|case| {
                case.name == name
                    && match (case.payload.as_deref(), payload) {
                        (None, None) => true,
                        (Some(expected), Some(actual)) => {
                            expected.accepts_value_at_depth(actual, depth + 1)
                        }
                        (None, Some(_)) | (Some(_), None) => false,
                    }
            })
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the closed checked-type algebra has one canonical transcript owner"
)]
fn write_checked_type_identity(
    encoder: &mut RuntimeSemanticTypeIdentityEncoder,
    ty: &RuntimeCheckedType,
) {
    match ty {
        RuntimeCheckedType::Never => encoder.write_tag(0),
        RuntimeCheckedType::Unit => encoder.write_tag(1),
        RuntimeCheckedType::Bool => encoder.write_tag(2),
        RuntimeCheckedType::Signed(width) => {
            encoder.write_tag(3);
            encoder.write_u8(match width {
                RuntimeSignedIntWidth::I8 => 0,
                RuntimeSignedIntWidth::I16 => 1,
                RuntimeSignedIntWidth::I32 => 2,
                RuntimeSignedIntWidth::I64 => 3,
                RuntimeSignedIntWidth::I128 => 4,
                RuntimeSignedIntWidth::ISize => 5,
            });
        }
        RuntimeCheckedType::Unsigned(width) => {
            encoder.write_tag(4);
            encoder.write_u8(match width {
                RuntimeUnsignedIntWidth::U8 => 0,
                RuntimeUnsignedIntWidth::U16 => 1,
                RuntimeUnsignedIntWidth::U32 => 2,
                RuntimeUnsignedIntWidth::U64 => 3,
                RuntimeUnsignedIntWidth::U128 => 4,
                RuntimeUnsignedIntWidth::USize => 5,
            });
        }
        RuntimeCheckedType::F32 => encoder.write_tag(5),
        RuntimeCheckedType::F64 => encoder.write_tag(6),
        RuntimeCheckedType::String => encoder.write_tag(7),
        RuntimeCheckedType::Char => encoder.write_tag(8),
        RuntimeCheckedType::Duration => encoder.write_tag(9),
        RuntimeCheckedType::Progress => encoder.write_tag(10),
        RuntimeCheckedType::EntityReference => encoder.write_tag(11),
        RuntimeCheckedType::Bytes => encoder.write_tag(12),
        RuntimeCheckedType::Sequence(inner) => {
            encoder.write_tag(13);
            write_checked_type_identity(encoder, inner);
        }
        RuntimeCheckedType::Tuple(items) => {
            encoder.write_tag(14);
            encoder.write_len(items.len());
            for item in items {
                write_checked_type_identity(encoder, item);
            }
        }
        RuntimeCheckedType::Choice(items) => {
            encoder.write_tag(15);
            encoder.write_len(items.len());
            for item in items {
                write_checked_type_identity(encoder, item);
            }
        }
        RuntimeCheckedType::Nominal {
            semantic_identity,
            layout,
            arguments,
            ..
        } => {
            encoder.write_tag(16);
            encoder.write_bytes(semantic_identity.as_bytes());
            encoder.write_bytes(layout.as_bytes());
            encoder.write_len(arguments.len());
            for argument in arguments {
                write_checked_type_identity(encoder, argument);
            }
        }
        RuntimeCheckedType::Opaque { owner } => {
            encoder.write_tag(17);
            encoder.write_str(owner.producer().as_str());
            encoder.write_bytes(owner.semantic_identity().as_bytes());
            encoder.write_u8(owner.admission() as u8);
            encoder.write_u8(owner.value_class().semantic_tag());
            encoder.write_u8(owner.persistence().semantic_tag());
        }
        RuntimeCheckedType::Variant {
            owner,
            arguments,
            cases,
        } => {
            encoder.write_tag(18);
            write_variant_identity(encoder, owner);
            encoder.write_len(arguments.len());
            for argument in arguments {
                write_checked_type_identity(encoder, argument);
            }
            encoder.write_len(cases.len());
            for case in cases {
                encoder.write_str(&case.name);
                match &case.payload {
                    Some(payload) => {
                        encoder.write_u8(1);
                        write_checked_type_identity(encoder, payload);
                    }
                    None => encoder.write_u8(0),
                }
            }
        }
        RuntimeCheckedType::Result { ok, error } => {
            encoder.write_tag(19);
            write_checked_type_identity(encoder, ok);
            write_checked_type_identity(encoder, error);
        }
        RuntimeCheckedType::Option(inner) => {
            encoder.write_tag(20);
            write_checked_type_identity(encoder, inner);
        }
        RuntimeCheckedType::Agent(agent) => {
            encoder.write_tag(21);
            encoder.write_u8(agent.semantic_tag());
        }
        RuntimeCheckedType::AgentValue => encoder.write_tag(22),
    }
}

fn runtime_value_is_agent_value(value: &RuntimeValue, depth: usize) -> bool {
    if depth > crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH {
        return false;
    }
    match value {
        RuntimeValue::Unit
        | RuntimeValue::Bool(_)
        | RuntimeValue::String(_)
        | RuntimeValue::EntityRef(_) => true,
        RuntimeValue::Int(value) => value.width() == RuntimeSignedIntWidth::I64,
        RuntimeValue::UInt(value) => value.width() == RuntimeUnsignedIntWidth::U64,
        RuntimeValue::F64(value) => value.is_finite(),
        RuntimeValue::Seq(values) => values
            .clone()
            .into_values()
            .iter()
            .all(|value| runtime_value_is_agent_value(value, depth + 1)),
        RuntimeValue::Record(fields) => fields
            .iter()
            .all(|field| runtime_value_is_agent_value(field.value(), depth + 1)),
        RuntimeValue::F32(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_)
        | RuntimeValue::Char(_)
        | RuntimeValue::Duration(_)
        | RuntimeValue::Progress(_)
        | RuntimeValue::Range(_)
        | RuntimeValue::Iterator(_)
        | RuntimeValue::Tuple(_)
        | RuntimeValue::NominalRecord(_)
        | RuntimeValue::Opaque(_)
        | RuntimeValue::Reduction(_)
        | RuntimeValue::Agent(_)
        | RuntimeValue::Function(_)
        | RuntimeValue::Variant { .. } => false,
    }
}

fn write_variant_identity(
    encoder: &mut RuntimeSemanticTypeIdentityEncoder,
    owner: &RuntimeVariantIdentity,
) {
    match owner {
        RuntimeVariantIdentity::Nominal {
            semantic_identity, ..
        } => {
            encoder.write_u8(0);
            encoder.write_bytes(semantic_identity.as_bytes());
        }
        RuntimeVariantIdentity::Builtin(owner) => {
            encoder.write_u8(1);
            encoder.write_u8(owner.semantic_tag());
        }
    }
}

/// One field inside a runtime record pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeRecordPatternField {
    field: RuntimeRecordFieldId,
    pattern: RuntimePattern,
}

impl RuntimeRecordPatternField {
    pub(crate) const fn from_admitted_parts(
        field: RuntimeRecordFieldId,
        pattern: RuntimePattern,
    ) -> Self {
        Self { field, pattern }
    }

    #[must_use]
    pub const fn field(&self) -> RuntimeRecordFieldId {
        self.field
    }

    #[must_use]
    pub const fn pattern(&self) -> &RuntimePattern {
        &self.pattern
    }
}

pub(crate) fn match_runtime_pattern(
    plan: &RuntimePlan,
    pattern: &RuntimePattern,
    value: &RuntimeValue,
) -> Result<Option<Vec<RuntimeLocalBinding>>, RuntimePatternMatchError> {
    validate_runtime_pattern(plan, pattern)?;
    if !plan.value_matches_type(pattern.ty(), value)? {
        return Ok(None);
    }
    let mut bindings = Vec::with_capacity(pattern_binding_capacity(pattern));
    if collect_pattern_bindings(plan, pattern, value, &mut bindings)? {
        Ok(Some(bindings))
    } else {
        Ok(None)
    }
}

pub(crate) fn pattern_binding_capacity(pattern: &RuntimePattern) -> usize {
    let direct = match pattern.kind() {
        RuntimePatternKind::Bind { .. } | RuntimePatternKind::Typed { .. } => 1,
        RuntimePatternKind::Discard
        | RuntimePatternKind::Literal(_)
        | RuntimePatternKind::Entity(_) => 0,
        RuntimePatternKind::Tuple(patterns)
        | RuntimePatternKind::Sequence {
            items: patterns, ..
        } => patterns.iter().map(pattern_binding_capacity).sum(),
        RuntimePatternKind::Record { fields, .. } => fields
            .iter()
            .map(|field| pattern_binding_capacity(field.pattern()))
            .sum(),
        RuntimePatternKind::Variant { payload, .. } => {
            payload.as_deref().map_or(0, pattern_binding_capacity)
        }
        RuntimePatternKind::Whole { pattern, .. } => pattern_binding_capacity(pattern) + 1,
    };
    direct
        + usize::from(matches!(
            pattern.kind(),
            RuntimePatternKind::Record {
                rest: RuntimePatternRest::Bind(_),
                ..
            } | RuntimePatternKind::Sequence {
                rest: RuntimePatternRest::Bind(_),
                ..
            }
        ))
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimePatternMatchError {
    #[error(transparent)]
    ValueType(#[from] RuntimePlanValueTypeError),
    #[error("runtime pattern references unknown plan type {ty}")]
    UnknownType { ty: RuntimePlanTypeId },
    #[error("runtime pattern kind is incompatible with plan type {ty}")]
    InvalidKind { ty: RuntimePlanTypeId },
    #[error("runtime pattern child has type {actual}, expected {expected}")]
    ChildType {
        expected: RuntimePlanTypeId,
        actual: RuntimePlanTypeId,
    },
    #[error("runtime pattern references unknown local declaration {local}")]
    UnknownLocal { local: RuntimeLocalDeclarationId },
    #[error("runtime pattern local {local} has type {actual}, expected {expected}")]
    LocalType {
        local: RuntimeLocalDeclarationId,
        expected: RuntimePlanTypeId,
        actual: RuntimePlanTypeId,
    },
    #[error("runtime pattern local {local} has a coordinate inconsistent with its node")]
    InvalidCoordinate { local: RuntimeLocalDeclarationId },
    #[error("runtime pattern binds local {local} more than once")]
    DuplicateLocal { local: RuntimeLocalDeclarationId },
    #[error("runtime record pattern type {ty} has no nominal-record domain")]
    MissingRecordDomain { ty: RuntimePlanTypeId },
    #[error("runtime record pattern type {ty} references unknown field {field}")]
    UnknownRecordField {
        ty: RuntimePlanTypeId,
        field: RuntimeRecordFieldId,
    },
    #[error("runtime record pattern type {ty} repeats field {field}")]
    DuplicateRecordField {
        ty: RuntimePlanTypeId,
        field: RuntimeRecordFieldId,
    },
    #[error("runtime variant pattern type {ty} has no case {ordinal}")]
    UnknownVariantCase { ty: RuntimePlanTypeId, ordinal: u32 },
    #[error("runtime variant pattern type {ty} has no nominal-variant domain")]
    MissingVariantDomain { ty: RuntimePlanTypeId },
    #[error("runtime variant pattern type {ty} has an invalid payload shape for case {ordinal}")]
    VariantPayload { ty: RuntimePlanTypeId, ordinal: u32 },
}

fn validate_runtime_pattern(
    plan: &RuntimePlan,
    pattern: &RuntimePattern,
) -> Result<(), RuntimePatternMatchError> {
    let mut locals = BTreeSet::new();
    validate_pattern_node(plan, pattern, &[], &mut locals)
}

fn validate_pattern_node(
    plan: &RuntimePlan,
    pattern: &RuntimePattern,
    path: &[RuntimePatternBindingStep],
    locals: &mut BTreeSet<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimePatternMatchError> {
    let declaration = plan
        .type_table()
        .get(pattern.ty())
        .ok_or(RuntimePatternMatchError::UnknownType { ty: pattern.ty() })?;
    match pattern.kind() {
        RuntimePatternKind::Bind { binding, .. } | RuntimePatternKind::Typed { binding } => {
            validate_binding(plan, pattern.ty(), path, binding, locals)
        }
        RuntimePatternKind::Discard => Ok(()),
        RuntimePatternKind::Literal(value) => {
            if plan.value_matches_type(pattern.ty(), value)? {
                Ok(())
            } else {
                Err(RuntimePatternMatchError::InvalidKind { ty: pattern.ty() })
            }
        }
        RuntimePatternKind::Entity(_) => match declaration.projection() {
            RuntimePlanTypeProjection::EntityReference => Ok(()),
            _ => Err(RuntimePatternMatchError::InvalidKind { ty: pattern.ty() }),
        },
        RuntimePatternKind::Tuple(patterns) => {
            validate_tuple_pattern(plan, pattern.ty(), patterns, path, locals)
        }
        RuntimePatternKind::Record { fields, rest } => {
            validate_record_pattern(plan, pattern.ty(), fields, rest, path, locals)
        }
        RuntimePatternKind::Sequence { items, rest } => {
            validate_sequence_pattern(plan, pattern.ty(), items, rest, path, locals)
        }
        RuntimePatternKind::Variant { ordinal, payload } => validate_variant_pattern(
            plan,
            pattern.ty(),
            *ordinal,
            payload.as_deref(),
            path,
            locals,
        ),
        RuntimePatternKind::Whole {
            binding,
            pattern: inner,
        } => {
            validate_child_type(pattern.ty(), inner.ty())?;
            validate_binding(plan, pattern.ty(), path, binding, locals)?;
            validate_pattern_node(plan, inner, path, locals)
        }
    }
}

fn validate_tuple_pattern(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    patterns: &[RuntimePattern],
    path: &[RuntimePatternBindingStep],
    locals: &mut BTreeSet<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimePatternMatchError> {
    let Some(RuntimePlanTypeProjection::Tuple(types)) = plan
        .type_table()
        .get(ty)
        .map(crate::plan::RuntimePlanTypeDeclaration::projection)
    else {
        return Err(RuntimePatternMatchError::InvalidKind { ty });
    };
    if patterns.len() != types.len() {
        return Err(RuntimePatternMatchError::InvalidKind { ty });
    }
    for (ordinal, (pattern, expected)) in patterns.iter().zip(types).enumerate() {
        validate_child_type(*expected, pattern.ty())?;
        let ordinal =
            u32::try_from(ordinal).map_err(|_| RuntimePatternMatchError::InvalidKind { ty })?;
        let mut child_path = path.to_vec();
        child_path.push(RuntimePatternBindingStep::TupleElement(ordinal));
        validate_pattern_node(plan, pattern, &child_path, locals)?;
    }
    Ok(())
}

fn validate_record_pattern(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    fields: &[RuntimeRecordPatternField],
    rest: &RuntimePatternRest,
    path: &[RuntimePatternBindingStep],
    locals: &mut BTreeSet<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimePatternMatchError> {
    if !matches!(
        plan.type_table()
            .get(ty)
            .map(crate::plan::RuntimePlanTypeDeclaration::projection),
        Some(RuntimePlanTypeProjection::ProjectNominal { .. })
    ) {
        return Err(RuntimePatternMatchError::InvalidKind { ty });
    }
    let domain = plan
        .nominal_record_domains()
        .get(ty)
        .ok_or(RuntimePatternMatchError::MissingRecordDomain { ty })?;
    let mut seen_fields = BTreeSet::new();
    for (ordinal, field) in fields.iter().enumerate() {
        if !seen_fields.insert(field.field()) {
            return Err(RuntimePatternMatchError::DuplicateRecordField {
                ty,
                field: field.field(),
            });
        }
        let expected = usize::try_from(field.field().zero_based())
            .ok()
            .and_then(|index| domain.fields().get(index))
            .ok_or(RuntimePatternMatchError::UnknownRecordField {
                ty,
                field: field.field(),
            })?
            .ty();
        validate_child_type(expected, field.pattern().ty())?;
        let ordinal =
            u32::try_from(ordinal).map_err(|_| RuntimePatternMatchError::InvalidKind { ty })?;
        let mut child_path = path.to_vec();
        child_path.push(RuntimePatternBindingStep::RecordField(ordinal));
        validate_pattern_node(plan, field.pattern(), &child_path, locals)?;
    }
    if let Some(binding) = rest.binding() {
        let mut rest_path = path.to_vec();
        rest_path.push(RuntimePatternBindingStep::RecordRest);
        validate_binding(plan, ty, &rest_path, binding, locals)?;
    }
    Ok(())
}

fn validate_sequence_pattern(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    items: &[RuntimePattern],
    rest: &RuntimePatternRest,
    path: &[RuntimePatternBindingStep],
    locals: &mut BTreeSet<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimePatternMatchError> {
    let declaration = plan
        .type_table()
        .get(ty)
        .ok_or(RuntimePatternMatchError::UnknownType { ty })?;
    let item_ty = match declaration.projection() {
        RuntimePlanTypeProjection::Sequence { item, .. } => *item,
        RuntimePlanTypeProjection::Array { item, .. }
            if !matches!(rest, RuntimePatternRest::Bind(_)) =>
        {
            *item
        }
        _ => return Err(RuntimePatternMatchError::InvalidKind { ty }),
    };
    for (ordinal, item) in items.iter().enumerate() {
        validate_child_type(item_ty, item.ty())?;
        let ordinal =
            u32::try_from(ordinal).map_err(|_| RuntimePatternMatchError::InvalidKind { ty })?;
        let mut child_path = path.to_vec();
        child_path.push(RuntimePatternBindingStep::SequenceElement(ordinal));
        validate_pattern_node(plan, item, &child_path, locals)?;
    }
    if let Some(binding) = rest.binding() {
        let mut rest_path = path.to_vec();
        rest_path.push(RuntimePatternBindingStep::SequenceRest);
        validate_binding(plan, ty, &rest_path, binding, locals)?;
    }
    Ok(())
}

fn validate_variant_pattern(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    ordinal: u32,
    payload: Option<&RuntimePattern>,
    path: &[RuntimePatternBindingStep],
    locals: &mut BTreeSet<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimePatternMatchError> {
    match (variant_payload_type(plan, ty, ordinal)?, payload) {
        (Some(expected), Some(payload)) => {
            validate_child_type(expected, payload.ty())?;
            let mut child_path = path.to_vec();
            child_path.push(RuntimePatternBindingStep::VariantPayload);
            validate_pattern_node(plan, payload, &child_path, locals)
        }
        (None, None) => Ok(()),
        (Some(_), None) | (None, Some(_)) => {
            Err(RuntimePatternMatchError::VariantPayload { ty, ordinal })
        }
    }
}

fn validate_binding(
    plan: &RuntimePlan,
    expected_ty: RuntimePlanTypeId,
    path: &[RuntimePatternBindingStep],
    binding: &RuntimePatternBindingCoordinate,
    locals: &mut BTreeSet<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimePatternMatchError> {
    let local = plan.local_declarations().get(binding.local()).ok_or(
        RuntimePatternMatchError::UnknownLocal {
            local: binding.local(),
        },
    )?;
    if local.ty() != expected_ty {
        return Err(RuntimePatternMatchError::LocalType {
            local: binding.local(),
            expected: expected_ty,
            actual: local.ty(),
        });
    }
    let expected_path = if path.is_empty() {
        &[RuntimePatternBindingStep::Whole][..]
    } else {
        path
    };
    if binding.path().steps() != expected_path {
        return Err(RuntimePatternMatchError::InvalidCoordinate {
            local: binding.local(),
        });
    }
    if !locals.insert(binding.local()) {
        return Err(RuntimePatternMatchError::DuplicateLocal {
            local: binding.local(),
        });
    }
    Ok(())
}

fn validate_child_type(
    expected: RuntimePlanTypeId,
    actual: RuntimePlanTypeId,
) -> Result<(), RuntimePatternMatchError> {
    if expected == actual {
        Ok(())
    } else {
        Err(RuntimePatternMatchError::ChildType { expected, actual })
    }
}

fn variant_payload_type(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    ordinal: u32,
) -> Result<Option<RuntimePlanTypeId>, RuntimePatternMatchError> {
    let declaration = plan
        .type_table()
        .get(ty)
        .ok_or(RuntimePatternMatchError::UnknownType { ty })?;
    match declaration.projection() {
        RuntimePlanTypeProjection::Result { value, error } => match ordinal {
            0 => Ok(Some(*value)),
            1 => Ok(Some(*error)),
            _ => Err(RuntimePatternMatchError::UnknownVariantCase { ty, ordinal }),
        },
        RuntimePlanTypeProjection::Option(item) => match ordinal {
            0 => Ok(Some(*item)),
            1 => Ok(None),
            _ => Err(RuntimePatternMatchError::UnknownVariantCase { ty, ordinal }),
        },
        RuntimePlanTypeProjection::ProjectNominal { .. }
        | RuntimePlanTypeProjection::Opaque { .. } => {
            let domain = plan
                .variant_domains()
                .get(ty)
                .ok_or(RuntimePatternMatchError::MissingVariantDomain { ty })?;
            domain
                .case(ordinal)
                .map(crate::plan::RuntimeVariantCase::payload)
                .ok_or(RuntimePatternMatchError::UnknownVariantCase { ty, ordinal })
        }
        _ => Err(RuntimePatternMatchError::InvalidKind { ty }),
    }
}

fn collect_pattern_bindings(
    plan: &RuntimePlan,
    pattern: &RuntimePattern,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeLocalBinding>,
) -> Result<bool, RuntimePatternMatchError> {
    match pattern.kind() {
        RuntimePatternKind::Bind { binding, .. } | RuntimePatternKind::Typed { binding } => {
            bindings.push(RuntimeLocalBinding {
                local: binding.local(),
                value: value.clone(),
            });
            Ok(true)
        }
        RuntimePatternKind::Discard => Ok(true),
        RuntimePatternKind::Literal(expected) => Ok(expected == value),
        RuntimePatternKind::Entity(expected) => Ok(matches!(
            value,
            RuntimeValue::EntityRef(actual) if actual == expected
        )),
        RuntimePatternKind::Tuple(patterns) => {
            let RuntimeValue::Tuple(values) = value else {
                return Ok(false);
            };
            collect_pattern_list(plan, patterns, values, bindings)
        }
        RuntimePatternKind::Record { fields, rest } => {
            collect_record_pattern_bindings(plan, pattern.ty(), fields, rest, value, bindings)
        }
        RuntimePatternKind::Sequence { items, rest } => {
            collect_sequence_pattern_bindings(plan, items, rest, value, bindings)
        }
        RuntimePatternKind::Variant {
            ordinal, payload, ..
        } => {
            let RuntimeValue::Variant {
                ordinal: actual_ordinal,
                payload: actual_payload,
                ..
            } = value
            else {
                return Ok(false);
            };
            if ordinal != actual_ordinal {
                return Ok(false);
            }
            match (payload.as_deref(), actual_payload.as_deref()) {
                (Some(pattern), Some(value)) => {
                    collect_pattern_bindings(plan, pattern, value, bindings)
                }
                (None, None) => Ok(true),
                (Some(_), None) | (None, Some(_)) => Ok(false),
            }
        }
        RuntimePatternKind::Whole {
            binding,
            pattern: inner,
        } => {
            if !collect_pattern_bindings(plan, inner, value, bindings)? {
                return Ok(false);
            }
            bindings.push(RuntimeLocalBinding {
                local: binding.local(),
                value: value.clone(),
            });
            Ok(true)
        }
    }
}

fn collect_record_pattern_bindings(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    fields: &[RuntimeRecordPatternField],
    rest: &RuntimePatternRest,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeLocalBinding>,
) -> Result<bool, RuntimePatternMatchError> {
    let RuntimeValue::NominalRecord(record) = value else {
        return Ok(false);
    };
    if !rest.accepts_len(fields.len(), record.fields().len()) {
        return Ok(false);
    }
    for field in fields {
        let Some(value) = record.field(field.field()) else {
            return Ok(false);
        };
        if !collect_pattern_bindings(plan, field.pattern(), value, bindings)? {
            return Ok(false);
        }
    }
    if let Some(binding) = rest.binding() {
        bindings.push(RuntimeLocalBinding {
            local: binding.local(),
            value: value.clone(),
        });
    }
    debug_assert!(plan.nominal_record_domains().get(ty).is_some());
    Ok(true)
}

fn collect_sequence_pattern_bindings(
    plan: &RuntimePlan,
    items: &[RuntimePattern],
    rest: &RuntimePatternRest,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeLocalBinding>,
) -> Result<bool, RuntimePatternMatchError> {
    let RuntimeValue::Seq(sequence) = value else {
        return Ok(false);
    };
    if !rest.accepts_len(items.len(), sequence.len()) {
        return Ok(false);
    }
    for (index, pattern) in items.iter().enumerate() {
        if !collect_pattern_bindings(plan, pattern, &sequence.value_at(index), bindings)? {
            return Ok(false);
        }
    }
    if let Some(binding) = rest.binding() {
        bindings.push(RuntimeLocalBinding {
            local: binding.local(),
            value: RuntimeValue::Seq(sequence.tail_from(items.len())),
        });
    }
    Ok(true)
}

fn collect_pattern_list(
    plan: &RuntimePlan,
    patterns: &[RuntimePattern],
    values: &[RuntimeValue],
    bindings: &mut Vec<RuntimeLocalBinding>,
) -> Result<bool, RuntimePatternMatchError> {
    for (pattern, value) in patterns.iter().zip(values) {
        if !collect_pattern_bindings(plan, pattern, value, bindings)? {
            return Ok(false);
        }
    }
    Ok(true)
}

impl RuntimePlan {
    /// Checks one value against the sole admitted plan type graph.
    ///
    /// Pattern matching and structured closure capture/application share this
    /// context-owned operation instead of maintaining parallel predicates.
    pub(crate) fn value_matches_type(
        &self,
        ty: RuntimePlanTypeId,
        value: &RuntimeValue,
    ) -> Result<bool, RuntimePlanValueTypeError> {
        if self.type_table().get(ty).is_none() {
            return Err(RuntimePlanValueTypeError::UnknownType { ty });
        }
        Ok(runtime_value_matches_type_inner(self, ty, value, 0))
    }
}

fn runtime_value_matches_type_inner(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    value: &RuntimeValue,
    depth: usize,
) -> bool {
    if depth > crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH {
        return false;
    }
    let Some(declaration) = plan.type_table().get(ty) else {
        return false;
    };
    match (declaration.projection(), value) {
        (RuntimePlanTypeProjection::Unit, RuntimeValue::Unit)
        | (RuntimePlanTypeProjection::Bool, RuntimeValue::Bool(_))
        | (RuntimePlanTypeProjection::F32, RuntimeValue::F32(_))
        | (RuntimePlanTypeProjection::F64, RuntimeValue::F64(_))
        | (RuntimePlanTypeProjection::String, RuntimeValue::String(_))
        | (RuntimePlanTypeProjection::Char, RuntimeValue::Char(_))
        | (RuntimePlanTypeProjection::Duration, RuntimeValue::Duration(_))
        | (RuntimePlanTypeProjection::Progress, RuntimeValue::Progress(_))
        | (RuntimePlanTypeProjection::EntityReference, RuntimeValue::EntityRef(_))
        | (RuntimePlanTypeProjection::Range(_), RuntimeValue::Range(_))
        | (RuntimePlanTypeProjection::Iterator(_), RuntimeValue::Iterator(_))
        | (RuntimePlanTypeProjection::Function { .. }, RuntimeValue::Function(_)) => true,
        (RuntimePlanTypeProjection::Signed(expected), RuntimeValue::Int(actual)) => {
            *expected == actual.width()
        }
        (RuntimePlanTypeProjection::Unsigned(expected), RuntimeValue::UInt(actual)) => {
            *expected == actual.width()
        }
        (RuntimePlanTypeProjection::Bytes, RuntimeValue::Seq(sequence)) => {
            runtime_sequence_is_bytes(sequence)
        }
        (RuntimePlanTypeProjection::Sequence { item, .. }, RuntimeValue::Seq(sequence)) => {
            runtime_sequence_matches_type(plan, *item, sequence, None, depth)
        }
        (RuntimePlanTypeProjection::Array { item, length }, RuntimeValue::Seq(sequence)) => {
            runtime_sequence_matches_type(plan, *item, sequence, Some(*length), depth)
        }
        (RuntimePlanTypeProjection::Tuple(types), RuntimeValue::Tuple(values)) => {
            runtime_tuple_matches_type(plan, types, values, depth)
        }
        (RuntimePlanTypeProjection::Choice(types), value) => {
            runtime_choice_matches_type(plan, types, value, depth)
        }
        (RuntimePlanTypeProjection::Result { value: ok, error }, value) => {
            runtime_result_matches_type(plan, *ok, *error, value, depth)
        }
        (RuntimePlanTypeProjection::Option(item), value) => {
            runtime_option_matches_type(plan, *item, value, depth)
        }
        (RuntimePlanTypeProjection::BuiltinVariant { owner, cases }, value) => {
            runtime_builtin_variant_matches_type(plan, *owner, cases, value, depth)
        }
        (RuntimePlanTypeProjection::AgentValue, value) => {
            RuntimeCheckedType::AgentValue.accepts_value(value)
        }
        (
            RuntimePlanTypeProjection::ProjectNominal {
                nominal, layout, ..
            },
            RuntimeValue::NominalRecord(record),
        ) => runtime_nominal_record_matches_type(plan, ty, nominal, *layout, record, depth),
        (
            RuntimePlanTypeProjection::ProjectNominal { .. }
            | RuntimePlanTypeProjection::Opaque { .. },
            value @ RuntimeValue::Variant {
                owner: RuntimeVariantIdentity::Nominal { .. },
                ..
            },
        ) => runtime_nominal_variant_matches_type(plan, ty, declaration, value, depth),
        (
            RuntimePlanTypeProjection::Opaque {
                producer,
                admission,
                ..
            },
            RuntimeValue::Opaque(value),
        ) => runtime_opaque_matches_type(declaration, producer, *admission, value),
        (
            RuntimePlanTypeProjection::Shared(inner) | RuntimePlanTypeProjection::Reference(inner),
            value,
        ) => runtime_value_matches_type_inner(plan, *inner, value, depth + 1),
        (RuntimePlanTypeProjection::Agent(agent), RuntimeValue::Agent(value)) => {
            agent.operational_type() == value.operational_type()
        }
        _ => false,
    }
}

fn runtime_sequence_is_bytes(sequence: &RuntimeSeq) -> bool {
    sequence.clone().into_values().iter().all(
        |value| matches!(value, RuntimeValue::UInt(value) if value.width() == RuntimeUnsignedIntWidth::U8),
    )
}

fn runtime_sequence_matches_type(
    plan: &RuntimePlan,
    item: RuntimePlanTypeId,
    sequence: &RuntimeSeq,
    expected_length: Option<u64>,
    depth: usize,
) -> bool {
    if expected_length.is_some_and(|length| usize::try_from(length).ok() != Some(sequence.len())) {
        return false;
    }
    sequence
        .clone()
        .into_values()
        .iter()
        .all(|value| runtime_value_matches_type_inner(plan, item, value, depth + 1))
}

fn runtime_tuple_matches_type(
    plan: &RuntimePlan,
    types: &[RuntimePlanTypeId],
    values: &[RuntimeValue],
    depth: usize,
) -> bool {
    types.len() == values.len()
        && types
            .iter()
            .zip(values)
            .all(|(ty, value)| runtime_value_matches_type_inner(plan, *ty, value, depth + 1))
}

fn runtime_choice_matches_type(
    plan: &RuntimePlan,
    types: &[RuntimePlanTypeId],
    value: &RuntimeValue,
    depth: usize,
) -> bool {
    types
        .iter()
        .any(|ty| runtime_value_matches_type_inner(plan, *ty, value, depth + 1))
}

fn runtime_result_matches_type(
    plan: &RuntimePlan,
    ok: RuntimePlanTypeId,
    error: RuntimePlanTypeId,
    value: &RuntimeValue,
    depth: usize,
) -> bool {
    match value.builtin_variant_case() {
        Some((RuntimeBuiltinVariantCaseIdentity::ResultOk, Some(value))) => {
            runtime_value_matches_type_inner(plan, ok, value, depth + 1)
        }
        Some((RuntimeBuiltinVariantCaseIdentity::ResultErr, Some(value))) => {
            runtime_value_matches_type_inner(plan, error, value, depth + 1)
        }
        _ => false,
    }
}

fn runtime_option_matches_type(
    plan: &RuntimePlan,
    item: RuntimePlanTypeId,
    value: &RuntimeValue,
    depth: usize,
) -> bool {
    match value.builtin_variant_case() {
        Some((RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value))) => {
            runtime_value_matches_type_inner(plan, item, value, depth + 1)
        }
        Some((RuntimeBuiltinVariantCaseIdentity::OptionNone, None)) => true,
        _ => false,
    }
}

fn runtime_builtin_variant_matches_type(
    plan: &RuntimePlan,
    owner: RuntimeBuiltinVariantIdentity,
    cases: &[Option<RuntimePlanTypeId>],
    value: &RuntimeValue,
    depth: usize,
) -> bool {
    let Some((case, payload)) = value.builtin_variant_case() else {
        return false;
    };
    if case.owner() != owner {
        return false;
    }
    let Some((ordinal, _)) = owner.resolve_case(case) else {
        return false;
    };
    let Some(expected_payload) = usize::try_from(ordinal)
        .ok()
        .and_then(|ordinal| cases.get(ordinal))
    else {
        return false;
    };
    match (expected_payload, payload) {
        (Some(expected), Some(payload)) => {
            runtime_value_matches_type_inner(plan, *expected, payload, depth + 1)
        }
        (None, None) => true,
        _ => false,
    }
}

fn runtime_nominal_record_matches_type(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    nominal: &RuntimeNominalTypeId,
    layout: TypeLayoutHash,
    record: &RuntimeNominalRecordValue,
    depth: usize,
) -> bool {
    plan.nominal_record_domains().get(ty).is_some_and(|domain| {
        record.type_id() == nominal
            && record.layout() == layout
            && record.fields().len() == domain.fields().len()
            && record
                .fields()
                .iter()
                .zip(domain.fields())
                .all(|(value, field)| {
                    runtime_value_matches_type_inner(plan, field.ty(), value, depth + 1)
                })
    })
}

fn runtime_nominal_variant_matches_type(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    declaration: &RuntimePlanTypeDeclaration,
    value: &RuntimeValue,
    depth: usize,
) -> bool {
    let RuntimeValue::Variant {
        owner:
            RuntimeVariantIdentity::Nominal {
                nominal: actual_nominal,
                semantic_identity,
            },
        ordinal,
        name,
        payload,
    } = value
    else {
        return false;
    };
    let Some(domain) = plan.variant_domains().get(ty) else {
        return false;
    };
    actual_nominal == domain.nominal()
        && *semantic_identity == declaration.semantic_identity()
        && domain.case(*ordinal).is_some_and(|case| {
            case.name() == name
                && match (case.payload(), payload.as_deref()) {
                    (Some(ty), Some(value)) => {
                        runtime_value_matches_type_inner(plan, ty, value, depth + 1)
                    }
                    (None, None) => true,
                    _ => false,
                }
        })
}

fn runtime_opaque_matches_type(
    declaration: &RuntimePlanTypeDeclaration,
    producer: &RuntimeOpaqueTypeProducerId,
    admission: RuntimeOpaqueTypeAdmission,
    value: &RuntimeOpaqueValue,
) -> bool {
    let owner = match admission {
        RuntimeOpaqueTypeAdmission::ExactIdentity => {
            RuntimeOpaqueTypeOwner::exact(producer.clone(), declaration.semantic_identity())
        }
        RuntimeOpaqueTypeAdmission::ProducerWide => {
            RuntimeOpaqueTypeOwner::producer_wide(producer.clone(), declaration.semantic_identity())
        }
    };
    owner.accepts_opaque_value(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{
        RuntimeLocalDeclarationSeed, RuntimeNominalRecordDomainFieldSeed,
        RuntimeNominalRecordDomainSeed, RuntimePatternSeed, RuntimePatternSeedKind,
        RuntimePlanBuildError, RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
        RuntimeRecordFieldSeedId, RuntimeRecordPatternFieldSeed,
    };
    use crate::value::{
        RuntimeBuiltinVariantValueError, RuntimeNominalRecordValue, runtime_sequence_values,
    };

    fn identity(marker: u8) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes([marker; 32])
    }

    #[test]
    fn builtin_variant_owner_is_the_sole_tag_case_and_payload_authority() {
        let owners = [
            RuntimeBuiltinVariantIdentity::Option,
            RuntimeBuiltinVariantIdentity::Result,
            RuntimeBuiltinVariantIdentity::AgentResourceBody,
            RuntimeBuiltinVariantIdentity::AgentBinaryEncoding,
        ];
        for owner in owners {
            assert_eq!(
                RuntimeBuiltinVariantIdentity::from_wire_tag(owner.semantic_tag()),
                Some(owner)
            );
            for (expected_ordinal, schema) in owner.cases().iter().copied().enumerate() {
                let (ordinal, resolved) = owner
                    .resolve_case(schema.identity())
                    .expect("schema identity resolves through its owner");
                assert_eq!(usize::try_from(ordinal).ok(), Some(expected_ordinal));
                assert_eq!(resolved, schema);
                assert_eq!(owner.case_at(ordinal), Some(schema));

                let payload = schema.has_payload().then_some(RuntimeValue::Unit);
                let value = RuntimeValue::try_builtin_variant(schema.identity(), payload)
                    .expect("schema-derived payload cardinality is accepted");
                assert_eq!(
                    value.builtin_variant_case().map(|(case, _)| case),
                    Some(schema.identity())
                );
            }
        }
        assert_eq!(RuntimeBuiltinVariantIdentity::from_wire_tag(u8::MAX), None);
        assert_eq!(
            RuntimeValue::try_builtin_variant(
                RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyJson,
                None,
            ),
            Err(RuntimeBuiltinVariantValueError::InvalidPayloadPresence)
        );
        assert_eq!(
            RuntimeValue::try_builtin_variant(
                RuntimeBuiltinVariantCaseIdentity::AgentBinaryEncodingBase64,
                Some(RuntimeValue::Unit),
            ),
            Err(RuntimeBuiltinVariantValueError::InvalidPayloadPresence)
        );
    }

    #[test]
    fn resource_body_builtin_rejects_unknown_wrong_and_flat_legacy_shapes() {
        let owner = RuntimeBuiltinVariantIdentity::AgentResourceBody;
        let cases = owner
            .cases()
            .iter()
            .copied()
            .map(|schema| RuntimeCheckedVariantCase {
                name: schema.name().to_owned(),
                payload: Some(Box::new(match schema.identity() {
                    RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyJson => {
                        RuntimeCheckedType::AgentValue
                    }
                    RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyText => {
                        RuntimeCheckedType::String
                    }
                    RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyBytesBase64 => {
                        RuntimeCheckedType::Agent(
                            crate::plan::RuntimeAgentOperationalType::BinaryResourceBody,
                        )
                    }
                    _ => unreachable!("resource body schema only contains resource body cases"),
                })),
            })
            .collect();
        let expected = RuntimeCheckedType::Variant {
            owner: RuntimeVariantIdentity::Builtin(owner),
            arguments: Vec::new(),
            cases,
        };
        let json = RuntimeValue::try_builtin_variant(
            RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyJson,
            Some(
                RuntimeValue::try_record(vec![("enabled".to_owned(), RuntimeValue::Bool(true))])
                    .expect("fixture JSON object"),
            ),
        )
        .expect("typed Json resource body");
        assert!(expected.accepts_value(&json));

        let (_, json_schema) = owner
            .resolve_case(RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyJson)
            .expect("Json case");
        let wrong_name = RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Builtin(owner),
            ordinal: owner
                .resolve_case(RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyJson)
                .expect("Json case")
                .0,
            name: "not-a-resource-case".to_owned(),
            payload: Some(Box::new(RuntimeValue::Unit)),
        };
        assert!(wrong_name.builtin_variant_case().is_none());
        assert!(!expected.accepts_value(&wrong_name));
        assert!(owner.case_at(u32::MAX).is_none());
        assert_eq!(
            json_schema.identity(),
            RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyJson
        );

        let flat_legacy = RuntimeValue::try_record(vec![
            ("kind".to_owned(), RuntimeValue::String("json".to_owned())),
            ("json".to_owned(), RuntimeValue::String("{}".to_owned())),
            ("text".to_owned(), RuntimeValue::String(String::new())),
            ("base64".to_owned(), RuntimeValue::String(String::new())),
        ])
        .expect("legacy flat record is structurally a record");
        assert!(!expected.accepts_value(&flat_legacy));

        let wrong_payload = RuntimeValue::try_builtin_variant(
            RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyText,
            Some(RuntimeValue::Bool(false)),
        )
        .expect("case cardinality is valid before type checking");
        assert!(!expected.accepts_value(&wrong_payload));
    }

    #[test]
    fn checked_runtime_type_recursively_validates_host_payload_shape() {
        let owner = RuntimeOpaqueTypeOwner::exact(
            RuntimeOpaqueTypeProducerId::try_new("fixture.host-result")
                .expect("fixture producer identity"),
            identity(90),
        );
        let expected = RuntimeCheckedType::Result {
            ok: Box::new(RuntimeCheckedType::Tuple(vec![
                RuntimeCheckedType::Sequence(Box::new(RuntimeCheckedType::Option(Box::new(
                    RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U16),
                )))),
                RuntimeCheckedType::Opaque {
                    owner: owner.clone(),
                },
            ])),
            error: Box::new(RuntimeCheckedType::String),
        };
        let opaque = owner
            .try_wrap(RuntimeValue::String("opaque".to_owned()))
            .expect("exact owner wraps its payload");
        let valid = RuntimeValue::result_ok(RuntimeValue::Tuple(vec![
            runtime_sequence_values(vec![
                RuntimeValue::option_some(RuntimeValue::u16(7)),
                RuntimeValue::option_none(),
            ]),
            opaque,
        ]));

        assert!(expected.accepts_value(&valid));
        assert!(
            expected.accepts_value(&RuntimeValue::result_err(RuntimeValue::String(
                "domain error".to_owned(),
            )))
        );

        let wrong_nested_width = RuntimeValue::result_ok(RuntimeValue::Tuple(vec![
            runtime_sequence_values(vec![RuntimeValue::option_some(RuntimeValue::u32(7))]),
            owner
                .try_wrap(RuntimeValue::String("opaque".to_owned()))
                .expect("exact owner wraps its payload"),
        ]));
        assert!(!expected.accepts_value(&wrong_nested_width));

        let foreign_owner = RuntimeOpaqueTypeOwner::exact(
            RuntimeOpaqueTypeProducerId::try_new("fixture.foreign-result")
                .expect("foreign fixture producer identity"),
            identity(90),
        );
        let wrong_opaque_owner = RuntimeValue::result_ok(RuntimeValue::Tuple(vec![
            runtime_sequence_values(vec![RuntimeValue::option_none()]),
            foreign_owner
                .try_wrap(RuntimeValue::String("opaque".to_owned()))
                .expect("foreign owner wraps its own payload"),
        ]));
        assert!(!expected.accepts_value(&wrong_opaque_owner));
        assert!(!expected.accepts_value(&RuntimeValue::result_err(RuntimeValue::Bool(false))));
    }

    #[test]
    fn tuple_pattern_binds_plan_local_ids() {
        let mut builder = RuntimePlanBuilder::new();
        let admitted = builder
            .admit_semantic_batch(
                [
                    RuntimePlanTypeSeed::new(identity(1), RuntimePlanTypeProjection::Bool),
                    RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::String),
                    RuntimePlanTypeSeed::new(
                        identity(3),
                        RuntimePlanTypeProjection::Tuple(
                            vec![identity(1), identity(2)].into_boxed_slice(),
                        ),
                    ),
                ],
                [
                    RuntimeLocalDeclarationSeed::new(identity(1)),
                    RuntimeLocalDeclarationSeed::new(identity(2)),
                ],
                [],
                [],
            )
            .expect("typed tuple admission");
        let pattern = builder
            .lower_pattern_seed_for_test(RuntimePatternSeed::new(
                identity(3),
                RuntimePatternSeedKind::Tuple(
                    vec![
                        RuntimePatternSeed::new(
                            identity(1),
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: admitted.local_ids()[0].clone(),
                            },
                        ),
                        RuntimePatternSeed::new(
                            identity(2),
                            RuntimePatternSeedKind::Typed {
                                local: admitted.local_ids()[1].clone(),
                            },
                        ),
                    ]
                    .into_boxed_slice(),
                ),
            ))
            .expect("admitted tuple pattern");
        let RuntimePatternKind::Tuple(items) = pattern.kind() else {
            panic!("tuple pattern kind");
        };
        let RuntimePatternKind::Bind {
            binding: bool_binding,
            ..
        } = items[0].kind()
        else {
            panic!("bool binding kind");
        };
        let RuntimePatternKind::Typed {
            binding: string_binding,
        } = items[1].kind()
        else {
            panic!("string binding kind");
        };
        let bool_local = bool_binding.local();
        let string_local = string_binding.local();
        let plan = builder.finish().expect("plan");
        let value = RuntimeValue::Tuple(vec![
            RuntimeValue::Bool(true),
            RuntimeValue::String("ready".to_owned()),
        ]);

        assert_eq!(
            match_runtime_pattern(&plan, &pattern, &value),
            Ok(Some(vec![
                RuntimeLocalBinding {
                    local: bool_local,
                    value: RuntimeValue::Bool(true),
                },
                RuntimeLocalBinding {
                    local: string_local,
                    value: RuntimeValue::String("ready".to_owned()),
                },
            ]))
        );
    }

    #[test]
    fn nominal_record_pattern_uses_owner_domain_and_field_id() {
        let nominal = RuntimeNominalTypeId::try_new("game.Pair").unwrap();
        let layout = TypeLayoutHash::from_bytes([7; 32]);
        let mut builder = RuntimePlanBuilder::new();
        let admitted = builder
            .admit_semantic_batch(
                [
                    RuntimePlanTypeSeed::new(
                        identity(1),
                        RuntimePlanTypeProjection::ProjectNominal {
                            nominal: nominal.clone(),
                            layout,
                            arguments: Box::new([]),
                        },
                    ),
                    RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::Bool),
                    RuntimePlanTypeSeed::new(identity(3), RuntimePlanTypeProjection::String),
                ],
                [RuntimeLocalDeclarationSeed::new(identity(2))],
                [RuntimeNominalRecordDomainSeed::new(
                    identity(1),
                    [
                        RuntimeNominalRecordDomainFieldSeed::new("alpha", identity(2)),
                        RuntimeNominalRecordDomainFieldSeed::new("zeta", identity(3)),
                    ],
                )],
                [],
            )
            .expect("record admission");
        let pattern = builder
            .lower_pattern_seed_for_test(RuntimePatternSeed::new(
                identity(1),
                RuntimePatternSeedKind::Record {
                    fields: Box::new([RuntimeRecordPatternFieldSeed::new(
                        RuntimeRecordFieldSeedId::from_zero_based(0),
                        RuntimePatternSeed::new(
                            identity(2),
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: admitted.local_ids()[0].clone(),
                            },
                        ),
                    )]),
                    rest: crate::plan::RuntimePatternRestSeed::Ignore,
                },
            ))
            .expect("admitted record pattern");
        let RuntimePatternKind::Record { fields, .. } = pattern.kind() else {
            panic!("record pattern kind");
        };
        let RuntimePatternKind::Bind { binding, .. } = fields[0].pattern().kind() else {
            panic!("record binding kind");
        };
        let local = binding.local();
        let plan = builder.finish().expect("plan");
        let value = RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
            nominal,
            layout,
            vec![
                RuntimeValue::Bool(true),
                RuntimeValue::String("tail".to_owned()),
            ],
        ));

        assert_eq!(
            match_runtime_pattern(&plan, &pattern, &value),
            Ok(Some(vec![RuntimeLocalBinding {
                local,
                value: RuntimeValue::Bool(true),
            }]))
        );
    }

    #[test]
    fn binding_local_type_must_equal_the_pattern_node_type() {
        let mut builder = RuntimePlanBuilder::new();
        let admitted = builder
            .admit_semantic_batch(
                [
                    RuntimePlanTypeSeed::new(identity(1), RuntimePlanTypeProjection::Bool),
                    RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::String),
                ],
                [RuntimeLocalDeclarationSeed::new(identity(2))],
                [],
                [],
            )
            .expect("local admission");
        let result = builder.lower_pattern_seed_for_test(RuntimePatternSeed::new(
            identity(1),
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local: admitted.local_ids()[0].clone(),
            },
        ));
        let plan = builder.finish().expect("plan");
        let bool_ty = plan
            .type_table()
            .id_for_semantic(identity(1))
            .expect("bool type");
        let string_ty = plan
            .type_table()
            .id_for_semantic(identity(2))
            .expect("string type");

        assert_eq!(
            result,
            Err(RuntimePlanBuildError::TypeMismatch {
                context: "pattern binding local",
                expected: bool_ty,
                actual: string_ty,
            })
        );
    }
}
