//! Canonical semantic identity encoding for checked types.

use arcweft_core::{
    pattern::{RuntimeCheckedType, RuntimeSemanticTypeId, RuntimeSemanticTypeIdentityEncoder},
    value::{RuntimeSignedIntWidth, RuntimeUnsignedIntWidth},
};
use arcweft_lang_hir::{
    leaf::{HirPath, HirPathRoot, HirPathSegment},
    symbol::{
        CallableDeclarationKey, ProjectSymbolWorldId,
        nominal::{ProjectNominalDeclarationId, ProjectNominalDeclarationKind},
    },
};
use arcweft_lang_syntax::{
    ast::module_path::{CanonicalModulePath, ModulePathRoot},
    reference::BorrowKind,
    types::TypePath,
};
use arcweft_source::SourceSpan;

mod traversal;
use traversal::EncodingTask;

use crate::{
    effect_row::{EffectRow, EffectRowTail},
    env::nominal::{AcceptedNominalId, AcceptedNominalOwnerId, OpenNominalRuleId},
};

use super::{
    ArrayLength, CharacterNominalType, CompileTimeCallableType, CompileTimeEnumType,
    CompileTimeFxType, CompileTimeScalarType, EntityKind, GenericConstParameterId,
    GenericConstReference, GenericParameterKind, GenericParameterOwnerId, GenericScope,
    GenericScopeError, GenericTypeReference, HandleState, IteratorStateKind, LifetimeScopeKind,
    MapKind, StageActorHandleType, TypeKind, ViewCallableId,
};

/// Stable semantic identity of one complete checked type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticTypeDigest([u8; 32]);

impl super::AcceptedVariantCaseSemanticId {
    pub(in crate::types) fn write_payload_type_identity(
        self,
        encoder: &mut RuntimeSemanticTypeIdentityEncoder,
    ) {
        encoder.write_tag(87);
        encoder.write_bytes(self.as_bytes());
    }

    pub(in crate::types) fn payload_type_identity(self) -> SemanticTypeDigest {
        let mut encoder = RuntimeSemanticTypeIdentityEncoder::new();
        self.write_payload_type_identity(&mut encoder);
        SemanticTypeDigest::from_bytes(*encoder.finish().as_bytes())
    }
}

impl SemanticTypeDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<SemanticTypeDigest> for RuntimeSemanticTypeId {
    fn from(identity: SemanticTypeDigest) -> Self {
        Self::from_bytes(*identity.as_bytes())
    }
}

impl ArrayLength {
    /// Canonical checked bytes for an array-length child embedded by another
    /// semantic owner. Recovery/inference-only lengths have no checked form.
    /// This owner method is the sole raw ArrayLength encoder; callable and
    /// runtime identities must not reconstruct a generic-constant owner.
    pub(crate) fn canonical_checked_bytes(&self) -> Result<Vec<u8>, super::TypeInstantiationError> {
        self.canonical_checked_bytes_in_scope(&GenericScope::default())
    }

    pub(in crate::types) fn canonical_checked_bytes_in_scope(
        &self,
        scope: &GenericScope,
    ) -> Result<Vec<u8>, super::TypeInstantiationError> {
        self.encode_checked_bytes(scope, &mut (), &|()| Ok(()), &|()| Ok(()))
    }

    pub(in crate::types) fn canonical_checked_bytes_in_scope_with_control<
        C: super::TypeProjectionControl,
    >(
        &self,
        scope: &GenericScope,
        control: &mut C,
    ) -> Result<Vec<u8>, super::TypeProjectionError<C::Error>> {
        self.encode_checked_bytes(
            scope,
            control,
            &|control| {
                control
                    .check()
                    .map_err(super::TypeProjectionError::Control)?;
                control
                    .visit_node(super::TypeProjectionNodeKind::Const, 1)
                    .map_err(super::TypeProjectionError::Control)
            },
            &|control| {
                control
                    .check()
                    .map_err(super::TypeProjectionError::Control)?;
                control
                    .visit_binding()
                    .map_err(super::TypeProjectionError::Control)
            },
        )
    }

    fn encode_checked_bytes<C, E: From<super::TypeInstantiationError>>(
        &self,
        scope: &GenericScope,
        control: &mut C,
        scalar: &impl Fn(&mut C) -> Result<(), E>,
        binding: &impl Fn(&mut C) -> Result<(), E>,
    ) -> Result<Vec<u8>, E> {
        let mut encoder = ArrayLengthCanonicalEncoder::default();
        if !scope.binders().is_empty() {
            encoder.tag(3);
            encoder.len(scope.binders().len())?;
            for binder in scope.binders() {
                binding(control)?;
                encoder.u16(binder.types());
                encoder.u16(binder.const_lengths());
                encoder.u32(binder.effects());
            }
        }
        scalar(control)?;
        match self {
            Self::Const(value) => {
                encoder.tag(0);
                encoder.u64(
                    u64::try_from(*value)
                        .map_err(|_| super::TypeInstantiationError::EncodingLengthOverflow)?,
                );
            }
            Self::Generic(GenericConstReference::Free(parameter)) => {
                encoder.tag(1);
                encoder.tag(0);
                encoder.generic_const(parameter)?;
            }
            Self::Generic(GenericConstReference::Bound(parameter)) => {
                scope
                    .bound_const(parameter.depth(), parameter.slot())
                    .map_err(super::TypeInstantiationError::from)?;
                encoder.tag(1);
                encoder.tag(1);
                encoder.u32(parameter.depth());
                encoder.u16(parameter.slot());
            }
            Self::Generic(GenericConstReference::Inference(_)) => {
                return Err(super::TypeInstantiationError::from(
                    GenericScopeError::EscapedInference {
                        kind: GenericParameterKind::Const,
                    },
                )
                .into());
            }
            Self::Error(_) | Self::Inferred => {
                return Err(super::TypeInstantiationError::UnresolvedType.into());
            }
        }
        Ok(encoder.finish())
    }
}

#[derive(Default)]
struct ArrayLengthCanonicalEncoder(Vec<u8>);

impl ArrayLengthCanonicalEncoder {
    fn finish(self) -> Vec<u8> {
        self.0
    }
    fn tag(&mut self, value: u8) {
        self.0.push(value);
    }
    fn u16(&mut self, value: u16) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn digest(&mut self, value: &[u8; 32]) {
        self.0.extend_from_slice(value);
    }
    fn len(&mut self, value: usize) -> Result<(), super::TypeInstantiationError> {
        self.u64(
            u64::try_from(value)
                .map_err(|_| super::TypeInstantiationError::EncodingLengthOverflow)?,
        );
        Ok(())
    }
    fn string(&mut self, value: &str) -> Result<(), super::TypeInstantiationError> {
        self.len(value.len())?;
        self.0.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn generic_const(
        &mut self,
        parameter: &GenericConstParameterId,
    ) -> Result<(), super::TypeInstantiationError> {
        // This marker keeps the type- and const-parameter namespaces disjoint.
        self.tag(0xC0);
        self.generic_owner(parameter.owner())?;
        self.u16(parameter.ordinal());
        Ok(())
    }

    fn generic_owner(
        &mut self,
        owner: &GenericParameterOwnerId,
    ) -> Result<(), super::TypeInstantiationError> {
        match owner {
            GenericParameterOwnerId::Callable(id) => {
                self.tag(0);
                self.digest(id.semantic_digest().as_bytes());
            }
            GenericParameterOwnerId::Nominal(id) => {
                self.tag(1);
                self.project_nominal(id)?;
            }
            GenericParameterOwnerId::AcceptedNominal(id) => {
                self.tag(2);
                self.accepted_nominal(id)?;
            }
            GenericParameterOwnerId::AcceptedSource(source) => {
                self.tag(3);
                self.source_span(source)?;
            }
            GenericParameterOwnerId::Detached(id) => {
                self.tag(4);
                self.u64(id.value());
            }
            GenericParameterOwnerId::LanguageIntrinsic(owner) => {
                self.tag(5);
                self.tag(owner.semantic_tag());
            }
        }
        Ok(())
    }

    fn project_nominal(
        &mut self,
        id: &ProjectNominalDeclarationId,
    ) -> Result<(), super::TypeInstantiationError> {
        self.string(id.world().package().as_str())?;
        self.string(id.world().root_document().as_str())?;
        self.string(id.world().profile())?;
        self.digest(id.revision().as_source_set().as_bytes());
        self.module_path(id.module())?;
        self.tag(match id.kind() {
            ProjectNominalDeclarationKind::Struct => 0,
            ProjectNominalDeclarationKind::Enum => 1,
            ProjectNominalDeclarationKind::TypeAlias => 2,
        });
        self.len(id.owner_path().len())?;
        for segment in id.owner_path() {
            self.string(segment.as_str())?;
        }
        self.string(id.name().as_str())?;
        Ok(())
    }

    fn accepted_nominal(
        &mut self,
        id: &AcceptedNominalId,
    ) -> Result<(), super::TypeInstantiationError> {
        match id.owner() {
            AcceptedNominalOwnerId::Standard => self.tag(0),
            AcceptedNominalOwnerId::Environment(owner) => {
                self.tag(1);
                self.string(owner.as_str())?;
            }
            AcceptedNominalOwnerId::RustPackage(package) => {
                self.tag(2);
                self.string(package.as_str())?;
            }
            AcceptedNominalOwnerId::Character(character) => {
                self.tag(3);
                self.string(character.as_str())?;
            }
        }
        self.module_root(id.canonical_path().root())?;
        self.len(id.canonical_path().segments().len())?;
        for segment in id.canonical_path().segments() {
            self.string(segment.as_str())?;
        }
        Ok(())
    }

    fn module_path(
        &mut self,
        path: &CanonicalModulePath,
    ) -> Result<(), super::TypeInstantiationError> {
        self.len(path.segments().len())?;
        for segment in path.segments() {
            self.string(segment.as_str())?;
        }
        Ok(())
    }

    fn module_root(&mut self, root: ModulePathRoot) -> Result<(), super::TypeInstantiationError> {
        match root {
            ModulePathRoot::ImplicitCrate => self.tag(0),
            ModulePathRoot::Crate => self.tag(1),
            ModulePathRoot::SelfModule => self.tag(2),
            ModulePathRoot::Super(levels) => {
                self.tag(3);
                self.u64(
                    u64::try_from(levels)
                        .map_err(|_| super::TypeInstantiationError::EncodingLengthOverflow)?,
                );
            }
        }
        Ok(())
    }

    fn source_span(&mut self, source: &SourceSpan) -> Result<(), super::TypeInstantiationError> {
        self.string(source.source().id().as_str())?;
        self.digest(source.source().revision().as_bytes());
        self.u64(source.source().source_len());
        let range = source.range();
        self.u64(
            u64::try_from(range.start())
                .map_err(|_| super::TypeInstantiationError::EncodingLengthOverflow)?,
        );
        self.u64(
            u64::try_from(range.end())
                .map_err(|_| super::TypeInstantiationError::EncodingLengthOverflow)?,
        );
        Ok(())
    }
}

impl TypeKind {
    /// Returns the canonical typed identity digest used by semantic caches.
    #[must_use]
    pub fn semantic_identity_digest(&self) -> Result<SemanticTypeDigest, GenericScopeError> {
        self.semantic_identity_digest_in_scope(&GenericScope::default())
    }

    /// A scoped term's identity includes its incoming lexical binders. Active
    /// inference references cannot produce stable bytes or a digest.
    pub fn semantic_identity_digest_in_scope(
        &self,
        scope: &GenericScope,
    ) -> Result<SemanticTypeDigest, GenericScopeError> {
        Encoder::encode(self, scope, &mut (), &|(), _, _| Ok(()), &|()| Ok(()))
    }

    /// Encodes the same scoped version-1 identity while admitting each type,
    /// constant, effect and binder to the consumer's transaction.
    pub fn semantic_identity_digest_in_scope_with_control<C: super::TypeProjectionControl>(
        &self,
        scope: &GenericScope,
        control: &mut C,
    ) -> Result<SemanticTypeDigest, super::TypeProjectionError<C::Error>> {
        Encoder::encode(
            self,
            scope,
            control,
            &|control, kind, depth| {
                control
                    .check()
                    .map_err(super::TypeProjectionError::Control)?;
                control
                    .visit_node(kind, depth)
                    .map_err(super::TypeProjectionError::Control)
            },
            &|control| {
                control
                    .check()
                    .map_err(super::TypeProjectionError::Control)?;
                control
                    .visit_binding()
                    .map_err(super::TypeProjectionError::Control)
            },
        )
    }
}

impl super::GenericTypeParameterId {
    /// A declaration reference has no lexical or application-local children.
    /// Its free-reference identity uses the same canonical encoder as TypeKind.
    pub fn semantic_identity_digest(&self) -> SemanticTypeDigest {
        let mut encoder = Encoder::new(GenericScope::default());
        encoder.tag(63);
        encoder.free_generic_parameter(self);
        SemanticTypeDigest(*encoder.bytes.finish().as_bytes())
    }
}

impl EffectRow {
    /// Version-1 row identity is the canonical nullary Unit function carrying
    /// this row. Encode that borrowed row without constructing a copied type.
    pub(crate) fn semantic_identity_digest(&self) -> SemanticTypeDigest {
        Encoder::effect_identity(self, &mut (), &|(), _, _| {
            Ok::<(), std::convert::Infallible>(())
        })
        .unwrap_or_else(|never| match never {})
    }

    pub(crate) fn semantic_identity_digest_with_control<C: super::TypeProjectionControl>(
        &self,
        control: &mut C,
    ) -> Result<SemanticTypeDigest, super::TypeProjectionError<C::Error>> {
        Encoder::effect_identity(self, control, &|control, kind, depth| {
            control
                .check()
                .map_err(super::TypeProjectionError::Control)?;
            control
                .visit_node(kind, depth)
                .map_err(super::TypeProjectionError::Control)
        })
    }
}

impl AcceptedNominalId {
    /// Canonical declaration identity using the nominal encoding without type
    /// arguments. Instantiated type identities are owned by `TypeKind`.
    pub(crate) fn semantic_digest(&self) -> SemanticTypeDigest {
        let mut encoder = Encoder::new(GenericScope::default());
        encoder.tag(65);
        encoder.accepted_nominal_id(self);
        encoder.len(0);
        SemanticTypeDigest(*encoder.bytes.finish().as_bytes())
    }
}

struct Encoder {
    bytes: RuntimeSemanticTypeIdentityEncoder,
    scope: GenericScope,
    error: Option<GenericScopeError>,
}

impl Encoder {
    fn effect_identity<C, E>(
        row: &EffectRow,
        control: &mut C,
        node: &impl Fn(&mut C, super::TypeProjectionNodeKind, u64) -> Result<(), E>,
    ) -> Result<SemanticTypeDigest, E> {
        let mut encoder = Self::new(GenericScope::default());
        node(control, super::TypeProjectionNodeKind::Type, 1)?;
        encoder.function_header(super::GenericBinder::EMPTY, 0);
        node(control, super::TypeProjectionNodeKind::Type, 2)?;
        encoder.checked(&RuntimeCheckedType::Unit);
        encoder.effect_row(row, 2, control, node)?;
        Ok(SemanticTypeDigest(*encoder.bytes.finish().as_bytes()))
    }

    fn function_header(&mut self, binder: super::GenericBinder, parameters: usize) {
        if binder.is_empty() {
            self.tag(62);
        } else {
            self.tag(95);
            self.u16(binder.types());
            self.u16(binder.const_lengths());
            self.u32(binder.effects());
        }
        self.len(parameters);
    }

    fn new(scope: GenericScope) -> Self {
        Self {
            bytes: RuntimeSemanticTypeIdentityEncoder::new(),
            scope,
            error: None,
        }
    }

    fn finish(self) -> Result<arcweft_core::pattern::RuntimeSemanticTypeId, GenericScopeError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        Ok(self.bytes.finish())
    }

    fn tag(&mut self, value: u16) {
        self.bytes.write_tag(value);
    }

    fn byte(&mut self, value: u8) {
        self.bytes.write_u8(value);
    }

    fn bool(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    fn u16(&mut self, value: u16) {
        self.bytes.write_u16(value);
    }

    fn u32(&mut self, value: u32) {
        self.bytes.write_u32(value);
    }

    fn u64(&mut self, value: u64) {
        self.bytes.write_u64(value);
    }

    fn bytes(&mut self, value: &[u8]) {
        self.bytes.write_bytes(value);
    }

    fn len(&mut self, value: usize) {
        self.bytes.write_len(value);
    }

    fn string(&mut self, value: &str) {
        self.bytes.write_str(value);
    }

    fn option<T>(&mut self, value: Option<&T>, encode: impl FnOnce(&mut Self, &T)) {
        match value {
            Some(value) => {
                self.byte(1);
                encode(self, value);
            }
            None => self.byte(0),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the stable semantic digest intentionally keeps one exhaustive fixed-tag match so a new TypeKind variant cannot bypass identity encoding"
    )]
    fn ty<'ty, C, E>(
        &mut self,
        ty: &'ty TypeKind,
        depth: usize,
        tasks: &mut Vec<EncodingTask<'ty>>,
        control: &mut C,
        binding: &impl Fn(&mut C) -> Result<(), E>,
    ) -> Result<Option<&'ty super::VariantPayloadType>, E> {
        // Every deeper term requires another owned node in the input. Its
        // nesting therefore fits the host's addressable node count.
        let child_depth = depth.checked_add(1).expect("owned type nesting fits usize");
        match ty {
            TypeKind::Bool => self.checked(&RuntimeCheckedType::Bool),
            TypeKind::I8 => self.checked(&RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I8)),
            TypeKind::I16 => {
                self.checked(&RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I16));
            }
            TypeKind::I32 => {
                self.checked(&RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I32));
            }
            TypeKind::I64 => {
                self.checked(&RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64));
            }
            TypeKind::I128 => {
                self.checked(&RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I128));
            }
            TypeKind::ISize => {
                self.checked(&RuntimeCheckedType::Signed(RuntimeSignedIntWidth::ISize));
            }
            TypeKind::U8 => {
                self.checked(&RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U8));
            }
            TypeKind::U16 => {
                self.checked(&RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U16));
            }
            TypeKind::U32 => {
                self.checked(&RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U32));
            }
            TypeKind::U64 => {
                self.checked(&RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U64));
            }
            TypeKind::U128 => {
                self.checked(&RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U128));
            }
            TypeKind::USize => {
                self.checked(&RuntimeCheckedType::Unsigned(
                    RuntimeUnsignedIntWidth::USize,
                ));
            }
            TypeKind::F32 => self.checked(&RuntimeCheckedType::F32),
            TypeKind::F64 => self.checked(&RuntimeCheckedType::F64),
            TypeKind::String => self.checked(&RuntimeCheckedType::String),
            TypeKind::Char => self.checked(&RuntimeCheckedType::Char),
            TypeKind::Bytes => self.checked(&RuntimeCheckedType::Bytes),
            TypeKind::TextCluster => self.tag(19),
            TypeKind::Duration => self.checked(&RuntimeCheckedType::Duration),
            TypeKind::Range(inner) => {
                self.tag(21);
                tasks.push(EncodingTask::Type(inner, child_depth));
            }
            TypeKind::IteratorState { family, item } => {
                self.tag(22);
                self.iterator_family(*family);
                tasks.push(EncodingTask::Type(item, child_depth));
            }
            TypeKind::DisplayText => self.tag(23),
            TypeKind::DebugStatePath => self.tag(24),
            TypeKind::ObservationFieldPath => self.tag(25),
            TypeKind::Ref(entity) => {
                self.tag(26);
                self.entity_kind(entity.kind());
                match entity.value() {
                    Some(value) => {
                        self.byte(1);
                        tasks.push(EncodingTask::Type(value, child_depth));
                    }
                    None => self.byte(0),
                }
            }
            TypeKind::Probe(inner) => {
                self.tag(27);
                tasks.push(EncodingTask::Type(inner, child_depth));
            }
            TypeKind::Predicate => self.tag(28),
            TypeKind::Observation => self.tag(29),
            TypeKind::ObservedObject => self.tag(30),
            TypeKind::AgentBBox => self.tag(31),
            TypeKind::ActionName => self.tag(32),
            TypeKind::ActionTarget => self.tag(33),
            TypeKind::ActionResult => self.tag(34),
            TypeKind::AgentValue => self.tag(35),
            TypeKind::DataFormat => self.tag(36),
            TypeKind::DataShape => self.tag(37),
            TypeKind::AgentEntityMetadata => self.tag(38),
            TypeKind::AgentSourceAnchor => self.tag(39),
            TypeKind::AgentProjectGraphNeighborhood => self.tag(40),
            TypeKind::AgentProjectGraphSymbol => self.tag(41),
            TypeKind::AgentProjectGraphEdge => self.tag(42),
            TypeKind::CaptureTarget => self.tag(43),
            TypeKind::CaptureRef => self.tag(44),
            TypeKind::AgentResource => self.tag(45),
            TypeKind::AgentResourceBody => self.tag(46),
            TypeKind::RagContextPack => self.tag(47),
            TypeKind::Vec(inner) => {
                self.tag(48);
                tasks.push(EncodingTask::Type(inner, child_depth));
            }
            TypeKind::Array { item, len } => {
                self.tag(49);
                tasks.push(EncodingTask::Length(len, child_depth));
                tasks.push(EncodingTask::Type(item, child_depth));
            }
            TypeKind::Slice(inner) => {
                self.tag(50);
                tasks.push(EncodingTask::Type(inner, child_depth));
            }
            TypeKind::Seq(inner) => {
                self.tag(51);
                tasks.push(EncodingTask::Type(inner, child_depth));
            }
            TypeKind::Map { kind, key, value } => {
                self.tag(52);
                self.map_kind(*kind);
                tasks.push(EncodingTask::Type(value, child_depth));
                tasks.push(EncodingTask::Type(key, child_depth));
            }
            TypeKind::BorrowRef {
                kind,
                lifetime,
                inner,
            } => {
                self.tag(53);
                self.borrow_kind(*kind);
                self.option(lifetime.as_ref(), Self::lifetime);
                tasks.push(EncodingTask::Type(inner, child_depth));
            }
            TypeKind::Need(value) => {
                self.tag(54);
                tasks.push(EncodingTask::Type(value, child_depth));
            }
            TypeKind::Stream { item, error } => {
                self.tag(55);
                tasks.push(EncodingTask::Type(error, child_depth));
                tasks.push(EncodingTask::Type(item, child_depth));
            }
            TypeKind::Parser { item, error } => {
                self.tag(56);
                tasks.push(EncodingTask::Type(error, child_depth));
                tasks.push(EncodingTask::Type(item, child_depth));
            }
            TypeKind::Result { ok, error } => {
                self.tag(57);
                tasks.push(EncodingTask::Type(error, child_depth));
                tasks.push(EncodingTask::Type(ok, child_depth));
            }
            TypeKind::Option(inner) => {
                self.tag(58);
                tasks.push(EncodingTask::Type(inner, child_depth));
            }
            TypeKind::Handle {
                name,
                lifetime,
                state,
                must_drop,
            } => {
                self.tag(59);
                self.string(name);
                self.lifetime(lifetime);
                self.handle_state(*state);
                self.bool(*must_drop);
            }
            TypeKind::ThreadHandle(inner) => {
                self.tag(60);
                tasks.push(EncodingTask::Type(inner, child_depth));
            }
            TypeKind::Shared(inner) => {
                self.tag(61);
                tasks.push(EncodingTask::Type(inner, child_depth));
            }
            TypeKind::Function {
                binder,
                params,
                return_type,
                effects,
            } => {
                if !binder.is_empty() {
                    binding(control)?;
                }
                self.function_header(*binder, params.len());
                let nested = self.scope.with_binder(*binder);
                let enclosing = std::mem::replace(&mut self.scope, nested);
                tasks.push(EncodingTask::FunctionEnd {
                    effects,
                    enclosing,
                    depth: child_depth,
                });
                tasks.push(EncodingTask::Type(return_type, child_depth));
                tasks.push(EncodingTask::Types(params.iter(), child_depth));
            }
            TypeKind::GenericParam(parameter) => {
                self.tag(63);
                self.generic_parameter(parameter);
            }
            TypeKind::ProjectNominal(nominal) => {
                self.tag(64);
                self.project_nominal_declaration(nominal.declaration());
                self.len(nominal.arguments().len());
                tasks.push(EncodingTask::Types(nominal.arguments().iter(), child_depth));
            }
            TypeKind::AcceptedNominal(nominal) => {
                self.tag(65);
                self.accepted_nominal_id(nominal.declaration());
                self.len(nominal.arguments().len());
                tasks.push(EncodingTask::Types(nominal.arguments().iter(), child_depth));
            }
            TypeKind::OpenNominal(nominal) => {
                self.tag(66);
                self.open_rule(nominal.rule());
                self.hir_path(nominal.path());
                self.len(nominal.arguments().len());
                tasks.push(EncodingTask::Types(nominal.arguments().iter(), child_depth));
            }
            TypeKind::Error(poison) => {
                self.tag(67);
                self.u32(poison.index());
            }
            TypeKind::Projection {
                subject,
                trait_name,
                assoc,
            } => {
                self.tag(68);
                tasks.push(EncodingTask::ProjectionTail { trait_name, assoc });
                tasks.push(EncodingTask::Type(subject, child_depth));
            }
            TypeKind::CharacterDialogue(dialogue) => {
                dialogue.encode_runtime_semantic_identity(&mut self.bytes);
            }
            TypeKind::DialogueLine(result) => {
                self.tag(70);
                tasks.push(EncodingTask::Type(result, child_depth));
            }
            TypeKind::CharacterPatch(kind) => {
                self.tag(71);
                self.entity_kind(kind);
            }
            TypeKind::FocusPatch => self.tag(72),
            TypeKind::CharacterNominal(nominal) => {
                self.tag(73);
                self.character_nominal(nominal);
            }
            TypeKind::Named(name) => {
                self.tag(74);
                self.string(name);
            }
            TypeKind::Tuple(items) => {
                self.tag(75);
                self.len(items.len());
                tasks.push(EncodingTask::Types(items.iter(), child_depth));
            }
            TypeKind::Choice(items) => {
                self.tag(76);
                self.len(items.len());
                tasks.push(EncodingTask::Types(items.iter(), child_depth));
            }
            TypeKind::VariantPayload(payload) => {
                return Ok(Some(payload));
            }
            TypeKind::Unit => self.checked(&RuntimeCheckedType::Unit),
            TypeKind::Never => self.checked(&RuntimeCheckedType::Never),
            TypeKind::AgentBuiltin(builtin) => {
                self.tag(79);
                self.agent_builtin(*builtin);
            }
            TypeKind::ViewValue => self.tag(80),
            TypeKind::Progress => self.checked(&RuntimeCheckedType::Progress),
            TypeKind::StageApi(character) => {
                self.tag(82);
                self.string(character.as_str());
            }
            TypeKind::LineContext => self.tag(83),
            TypeKind::StageActorHandle(handle) => {
                self.tag(84);
                match handle {
                    StageActorHandleType::Any => self.byte(0),
                    StageActorHandleType::Exact(character) => {
                        self.byte(1);
                        self.string(character.as_str());
                    }
                }
            }
            TypeKind::CueHandle => self.tag(85),
            TypeKind::VoiceHandle => self.tag(86),
            TypeKind::StatementIngress(ingress) => {
                self.tag(88);
                self.byte(ingress.semantic_tag());
            }
            TypeKind::CompileTimeCallable(callable) => {
                self.tag(89);
                self.compile_time_callable(callable);
            }
            TypeKind::MetaType(inner) => {
                self.tag(90);
                tasks.push(EncodingTask::Type(inner, child_depth));
            }
            TypeKind::CompileTimeScalar(scalar) => {
                self.tag(91);
                self.compile_time_scalar(scalar);
            }
            TypeKind::CompileTimeEnum(enum_type) => {
                self.tag(92);
                self.compile_time_enum(enum_type);
            }
            TypeKind::CompileTimeFx(fx) => {
                self.tag(93);
                self.compile_time_fx(fx);
            }
            TypeKind::FixedVector(vector) => {
                self.tag(94);
                self.byte(match vector.dimensions() {
                    crate::callable::VectorDimensions::Two => 2,
                    crate::callable::VectorDimensions::Three => 3,
                    crate::callable::VectorDimensions::Four => 4,
                });
                tasks.push(EncodingTask::Type(vector.component(), child_depth));
            }
        }
        Ok(None)
    }

    fn checked(&mut self, ty: &RuntimeCheckedType) {
        ty.encode_semantic_identity(&mut self.bytes);
    }

    fn agent_builtin(&mut self, builtin: super::AgentBuiltinType) {
        self.tag(match builtin {
            super::AgentBuiltinType::ObservedObjectId => 1,
            super::AgentBuiltinType::CaptureFormat => 2,
            super::AgentBuiltinType::CaptureKind => 3,
            super::AgentBuiltinType::Diagnostics => 4,
            super::AgentBuiltinType::WaitError => 5,
            super::AgentBuiltinType::ViewportPoint => 6,
            super::AgentBuiltinType::PointerButton => 7,
            super::AgentBuiltinType::RagError => 8,
            super::AgentBuiltinType::AgentSourcePosition => 9,
            super::AgentBuiltinType::AgentProjectFlowControlSummary => 10,
            super::AgentBuiltinType::AgentProjectGraphSummary => 11,
            super::AgentBuiltinType::AgentBinaryBody => 12,
            super::AgentBuiltinType::AgentBinaryEncoding => 13,
            super::AgentBuiltinType::AgentBinaryData => 14,
        });
    }

    fn compile_time_callable(&mut self, callable: &CompileTimeCallableType) {
        match callable {
            CompileTimeCallableType::View(id) => {
                self.byte(0);
                match id {
                    ViewCallableId::Element(element) => {
                        self.byte(0);
                        self.byte(element.semantic_tag());
                    }
                    ViewCallableId::Text => self.byte(1),
                    ViewCallableId::RichText => self.byte(2),
                }
            }
            CompileTimeCallableType::Style(id) => {
                self.byte(1);
                self.byte(id.semantic_tag());
            }
        }
    }

    fn compile_time_scalar(&mut self, scalar: &CompileTimeScalarType) {
        self.accepted_nominal_id(scalar.declaration());
        self.byte(scalar.kind().semantic_tag());
    }

    fn compile_time_enum(&mut self, enum_type: &CompileTimeEnumType) {
        self.bytes(&enum_type.domain().canonical_bytes());
        match (enum_type.exact_variant(), enum_type.allowed_variants()) {
            (Some(variant), _) => {
                self.byte(1);
                self.u16(variant);
            }
            (None, Some(allowed)) => {
                self.byte(2);
                self.len(allowed.len());
                for variant in allowed {
                    self.u16(*variant);
                }
            }
            (None, None) => self.byte(0),
        }
    }

    fn compile_time_fx(&mut self, fx: &CompileTimeFxType) {
        match fx {
            CompileTimeFxType::Abstract => self.byte(0),
            CompileTimeFxType::Constructor(id) => {
                self.byte(1);
                self.byte(id.semantic_tag());
            }
            CompileTimeFxType::Builtin(id) => {
                self.byte(2);
                self.byte(id.semantic_tag());
            }
            CompileTimeFxType::Registered(id) => {
                self.byte(3);
                self.string(id.package());
                self.string(id.function());
            }
        }
    }

    fn array_length(&mut self, length: &ArrayLength) {
        match length {
            ArrayLength::Const(value) => {
                self.byte(0);
                self.u64(u64::try_from(*value).expect("array lengths fit u64"));
            }
            ArrayLength::Generic(parameter) => {
                self.byte(1);
                self.generic_const_parameter(parameter);
            }
            ArrayLength::Error(poison) => {
                self.byte(2);
                self.u32(poison.index());
            }
            ArrayLength::Inferred => self.byte(3),
        }
    }

    fn generic_parameter(&mut self, reference: &GenericTypeReference) {
        match reference {
            GenericTypeReference::Free(parameter) => {
                self.free_generic_parameter(parameter);
            }
            GenericTypeReference::Bound(parameter) => {
                if let Err(error) = self.scope.bound_type(parameter.depth(), parameter.slot()) {
                    self.error.get_or_insert(error);
                    return;
                }
                self.byte(1);
                self.u32(parameter.depth());
                self.u16(parameter.slot());
            }
            GenericTypeReference::Inference(_) => {
                self.error
                    .get_or_insert(GenericScopeError::EscapedInference {
                        kind: GenericParameterKind::Type,
                    });
            }
        }
    }

    fn free_generic_parameter(&mut self, parameter: &super::GenericTypeParameterId) {
        self.byte(0);
        self.generic_owner(parameter.owner());
        self.u16(parameter.ordinal());
    }

    fn generic_const_parameter(&mut self, reference: &GenericConstReference) {
        // The tag separates the type and constant parameter namespaces even
        // when a declaration happens to use the same ordinal in both.
        self.byte(0xC0);
        match reference {
            GenericConstReference::Free(parameter) => {
                self.byte(0);
                self.generic_owner(parameter.owner());
                self.u16(parameter.ordinal());
            }
            GenericConstReference::Bound(parameter) => {
                if let Err(error) = self.scope.bound_const(parameter.depth(), parameter.slot()) {
                    self.error.get_or_insert(error);
                    return;
                }
                self.byte(1);
                self.u32(parameter.depth());
                self.u16(parameter.slot());
            }
            GenericConstReference::Inference(_) => {
                self.error
                    .get_or_insert(GenericScopeError::EscapedInference {
                        kind: GenericParameterKind::Const,
                    });
            }
        }
    }

    fn generic_owner(&mut self, owner: &GenericParameterOwnerId) {
        match owner {
            GenericParameterOwnerId::Callable(id) => {
                self.byte(0);
                self.callable_declaration(id);
            }
            GenericParameterOwnerId::Nominal(id) => {
                self.byte(1);
                self.project_nominal_declaration(id);
            }
            GenericParameterOwnerId::AcceptedNominal(id) => {
                self.byte(2);
                self.accepted_nominal_id(id);
            }
            GenericParameterOwnerId::AcceptedSource(source) => {
                self.byte(3);
                self.source_span(source);
            }
            GenericParameterOwnerId::Detached(id) => {
                self.byte(4);
                self.u64(id.value());
            }
            GenericParameterOwnerId::LanguageIntrinsic(owner) => {
                self.byte(5);
                self.byte(owner.semantic_tag());
            }
        }
    }

    fn callable_declaration(&mut self, id: &CallableDeclarationKey) {
        self.bytes.write_bytes(id.semantic_digest().as_bytes());
    }

    fn project_nominal_declaration(&mut self, id: &ProjectNominalDeclarationId) {
        self.project_world(id.world());
        self.bytes
            .write_bytes(id.revision().as_source_set().as_bytes());
        self.module_path(id.module());
        self.byte(match id.kind() {
            ProjectNominalDeclarationKind::Struct => 0,
            ProjectNominalDeclarationKind::Enum => 1,
            ProjectNominalDeclarationKind::TypeAlias => 2,
        });
        self.len(id.owner_path().len());
        for segment in id.owner_path() {
            self.string(segment.as_str());
        }
        self.string(id.name().as_str());
    }

    fn project_world(&mut self, world: &ProjectSymbolWorldId) {
        self.string(world.package().as_str());
        self.string(world.root_document().as_str());
        self.string(world.profile());
    }

    fn accepted_nominal_id(&mut self, id: &AcceptedNominalId) {
        match id.owner() {
            AcceptedNominalOwnerId::Standard => self.byte(0),
            AcceptedNominalOwnerId::Environment(owner) => {
                self.byte(1);
                self.string(owner.as_str());
            }
            AcceptedNominalOwnerId::RustPackage(package) => {
                self.byte(2);
                self.string(package.as_str());
            }
            AcceptedNominalOwnerId::Character(character) => {
                self.byte(3);
                self.string(character.as_str());
            }
        }
        self.type_path(id.canonical_path());
    }

    fn open_rule(&mut self, id: &OpenNominalRuleId) {
        self.string(id.owner().as_str());
        self.u32(id.ordinal());
    }

    fn type_path(&mut self, path: &TypePath) {
        self.module_root(path.root());
        self.len(path.segments().len());
        for segment in path.segments() {
            self.string(segment.as_str());
        }
    }

    fn hir_path(&mut self, path: &HirPath) {
        match path.root() {
            HirPathRoot::ImplicitCrate => self.byte(0),
            HirPathRoot::Crate => self.byte(1),
            HirPathRoot::SelfModule => self.byte(2),
            HirPathRoot::Super { depth } => {
                self.byte(3);
                self.len(depth);
            }
        }
        self.len(path.segments().len());
        for segment in path.segments() {
            match segment {
                HirPathSegment::Identifier(name) => {
                    self.byte(0);
                    self.string(name.as_str());
                }
                HirPathSegment::ProjectSymbol(name) => {
                    self.byte(1);
                    self.string(name.as_str());
                }
            }
        }
    }

    fn module_path(&mut self, path: &CanonicalModulePath) {
        self.len(path.segments().len());
        for segment in path.segments() {
            self.string(segment.as_str());
        }
    }

    fn source_span(&mut self, source: &SourceSpan) {
        self.string(source.source().id().as_str());
        self.bytes
            .write_bytes(source.source().revision().as_bytes());
        self.u64(source.source().source_len());
        let range = source.range();
        self.u64(u64::try_from(range.start()).expect("source offsets fit u64"));
        self.u64(u64::try_from(range.end()).expect("source offsets fit u64"));
    }

    fn effect_row<C, E>(
        &mut self,
        row: &EffectRow,
        depth: usize,
        control: &mut C,
        node: &impl Fn(&mut C, super::TypeProjectionNodeKind, u64) -> Result<(), E>,
    ) -> Result<(), E> {
        use super::TypeProjectionNodeKind;
        node(
            control,
            TypeProjectionNodeKind::Effect,
            traversal::depth_u64(depth),
        )?;
        self.len(row.concrete().iter().len());
        for effect in row.concrete().iter() {
            node(
                control,
                TypeProjectionNodeKind::Effect,
                traversal::depth_u64(depth + 1),
            )?;
            self.string(effect.as_str());
        }
        match row.tail() {
            EffectRowTail::Closed => self.byte(0),
            EffectRowTail::Variable(variable) => {
                node(
                    control,
                    TypeProjectionNodeKind::Effect,
                    traversal::depth_u64(depth + 1),
                )?;
                self.byte(1);
                self.bytes(variable.issuer().as_bytes());
                self.u32(variable.index());
            }
            EffectRowTail::Unknown => {
                node(
                    control,
                    TypeProjectionNodeKind::Effect,
                    traversal::depth_u64(depth + 1),
                )?;
                self.byte(2);
            }
        }
        Ok(())
    }

    fn character_nominal(&mut self, nominal: &CharacterNominalType) {
        match nominal {
            CharacterNominalType::Look { character } => {
                self.byte(0);
                self.string(character.as_str());
            }
            CharacterNominalType::Part { character } => {
                self.byte(1);
                self.string(character.as_str());
            }
            CharacterNominalType::Variant { character, part } => {
                self.byte(2);
                self.string(character.as_str());
                self.string(part.as_str());
            }
        }
    }

    fn module_root(&mut self, root: ModulePathRoot) {
        match root {
            ModulePathRoot::ImplicitCrate => self.byte(0),
            ModulePathRoot::Crate => self.byte(1),
            ModulePathRoot::SelfModule => self.byte(2),
            ModulePathRoot::Super(levels) => {
                self.byte(3);
                self.u64(u64::try_from(levels).expect("module parent depth fits u64"));
            }
        }
    }

    fn iterator_family(&mut self, family: IteratorStateKind) {
        self.byte(match family {
            IteratorStateKind::Range => 0,
            IteratorStateKind::Seq => 1,
            IteratorStateKind::Stream => 2,
            IteratorStateKind::Vec => 3,
            IteratorStateKind::Array => 4,
            IteratorStateKind::Slice => 5,
        });
    }

    fn map_kind(&mut self, kind: MapKind) {
        self.byte(match kind {
            MapKind::Ordered => 0,
            MapKind::Sorted => 1,
            MapKind::BTree => 2,
        });
    }

    fn borrow_kind(&mut self, kind: BorrowKind) {
        self.byte(match kind {
            BorrowKind::Shared => 0,
            BorrowKind::Mutable => 1,
        });
    }

    fn lifetime(&mut self, lifetime: &LifetimeScopeKind) {
        match lifetime {
            LifetimeScopeKind::Frame => self.byte(0),
            LifetimeScopeKind::Tick => self.byte(1),
            LifetimeScopeKind::Cue => self.byte(2),
            LifetimeScopeKind::Line => self.byte(3),
            LifetimeScopeKind::Scene => self.byte(4),
            LifetimeScopeKind::Flow => self.byte(5),
            LifetimeScopeKind::Session => self.byte(6),
            LifetimeScopeKind::Global => self.byte(7),
            LifetimeScopeKind::Persistent => self.byte(8),
            LifetimeScopeKind::Named(name) => {
                self.byte(9);
                self.string(name);
            }
        }
    }

    fn handle_state(&mut self, state: HandleState) {
        self.byte(match state {
            HandleState::Live => 0,
            HandleState::Dropped => 1,
            HandleState::Detached => 2,
            HandleState::MovedOut => 3,
        });
    }

    #[allow(
        clippy::too_many_lines,
        reason = "fixed entity-family tags are the canonical identity table and must remain exhaustive"
    )]
    fn entity_kind(&mut self, kind: &EntityKind) {
        if let EntityKind::Other(value) = kind {
            self.u16(37);
            self.string(value);
            return;
        }
        self.u16(match kind {
            EntityKind::Agent => 0,
            EntityKind::Entry => 1,
            EntityKind::Flow => 2,
            EntityKind::Choice => 3,
            EntityKind::ChoiceOption => 4,
            EntityKind::Character => 5,
            EntityKind::View => 6,
            EntityKind::Action => 7,
            EntityKind::Activity => 8,
            EntityKind::DialogueLine => 9,
            EntityKind::Text => 10,
            EntityKind::Content => 11,
            EntityKind::Input => 12,
            EntityKind::Button => 13,
            EntityKind::Style => 14,
            EntityKind::Asset => 15,
            EntityKind::Image => 16,
            EntityKind::Animation => 17,
            EntityKind::Capture => 18,
            EntityKind::Hook => 19,
            EntityKind::Signal => 20,
            EntityKind::Metric => 21,
            EntityKind::Scene => 22,
            EntityKind::Test => 24,
            EntityKind::Bench => 25,
            EntityKind::Layer => 26,
            EntityKind::Voice => 27,
            EntityKind::Se => 28,
            EntityKind::Bgm => 29,
            EntityKind::AudioBus => 30,
            EntityKind::MixerSnapshot => 31,
            EntityKind::Ducking => 32,
            EntityKind::Motion => 33,
            EntityKind::Rig => 34,
            EntityKind::Slot => 35,
            EntityKind::Target => 36,
            EntityKind::Other(_) => unreachable!("custom entity family handled above"),
        });
    }
}

#[cfg(test)]
mod tests;
