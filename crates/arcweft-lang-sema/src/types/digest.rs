//! Canonical semantic identity encoding for checked types.

use arcweft_core::{
    pattern::{RuntimeCheckedType, RuntimeSemanticTypeIdentityEncoder},
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

use crate::{
    effect_row::{EffectRow, EffectRowTail},
    env::nominal::{AcceptedNominalId, AcceptedNominalOwnerId, OpenNominalRuleId},
};

use super::{
    AcceptedNominalType, ArrayLength, CharacterNominalType, EntityKind, GenericConstParameterId,
    GenericParameterOwnerId, GenericTypeParameterId, HandleState, IteratorStateKind,
    LifetimeScopeKind, MapKind, OpenNominalType, ProjectNominalType, StageActorHandleType,
    TypeKind,
};

/// Stable semantic identity of one complete checked type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticTypeDigest([u8; 32]);

impl SemanticTypeDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl ArrayLength {
    /// Canonical checked bytes for an array-length child embedded by another
    /// semantic owner. Recovery/inference-only lengths have no checked form.
    /// This owner method is the sole raw ArrayLength encoder; callable and
    /// runtime identities must not reconstruct a generic-constant owner.
    pub(crate) fn canonical_checked_bytes(&self) -> Option<Vec<u8>> {
        let mut encoder = ArrayLengthCanonicalEncoder::default();
        match self {
            Self::Const(value) => {
                encoder.tag(0);
                encoder.u64(u64::try_from(*value).ok()?);
            }
            Self::Generic(parameter) => {
                encoder.tag(1);
                encoder.generic_const(parameter)?;
            }
            Self::Error(_) | Self::Inferred => return None,
        }
        Some(encoder.finish())
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
    fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn digest(&mut self, value: &[u8; 32]) {
        self.0.extend_from_slice(value);
    }
    fn len(&mut self, value: usize) -> Option<()> {
        self.u64(u64::try_from(value).ok()?);
        Some(())
    }
    fn string(&mut self, value: &str) -> Option<()> {
        self.len(value.len())?;
        self.0.extend_from_slice(value.as_bytes());
        Some(())
    }

    fn generic_const(&mut self, parameter: &GenericConstParameterId) -> Option<()> {
        // This marker keeps the type- and const-parameter namespaces disjoint.
        self.tag(0xC0);
        self.generic_owner(parameter.owner())?;
        self.u16(parameter.ordinal());
        Some(())
    }

    fn generic_owner(&mut self, owner: &GenericParameterOwnerId) -> Option<()> {
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
        Some(())
    }

    fn project_nominal(&mut self, id: &ProjectNominalDeclarationId) -> Option<()> {
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
        Some(())
    }

    fn accepted_nominal(&mut self, id: &AcceptedNominalId) -> Option<()> {
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
        Some(())
    }

    fn module_path(&mut self, path: &CanonicalModulePath) -> Option<()> {
        self.len(path.segments().len())?;
        for segment in path.segments() {
            self.string(segment.as_str())?;
        }
        Some(())
    }

    fn module_root(&mut self, root: ModulePathRoot) -> Option<()> {
        match root {
            ModulePathRoot::ImplicitCrate => self.tag(0),
            ModulePathRoot::Crate => self.tag(1),
            ModulePathRoot::SelfModule => self.tag(2),
            ModulePathRoot::Super(levels) => {
                self.tag(3);
                self.u64(u64::try_from(levels).ok()?);
            }
        }
        Some(())
    }

    fn source_span(&mut self, source: &SourceSpan) -> Option<()> {
        self.string(source.source().id().as_str())?;
        self.digest(source.source().revision().as_bytes());
        self.u64(source.source().source_len());
        let range = source.range();
        self.u64(u64::try_from(range.start()).ok()?);
        self.u64(u64::try_from(range.end()).ok()?);
        Some(())
    }
}

impl TypeKind {
    /// Returns the canonical typed identity digest used by semantic caches.
    #[must_use]
    pub fn semantic_identity_digest(&self) -> SemanticTypeDigest {
        let mut encoder = Encoder::new();
        encoder.ty(self);
        SemanticTypeDigest(*encoder.finish().as_bytes())
    }
}

pub(crate) fn accepted_nominal_semantic_identity_digest(
    declaration: &AcceptedNominalId,
    arguments: &[TypeKind],
) -> SemanticTypeDigest {
    let mut encoder = Encoder::new();
    encoder.tag(65);
    encoder.accepted_nominal_id(declaration);
    encoder.types(arguments);
    SemanticTypeDigest(*encoder.finish().as_bytes())
}

struct Encoder(RuntimeSemanticTypeIdentityEncoder);

impl Encoder {
    fn new() -> Self {
        Self(RuntimeSemanticTypeIdentityEncoder::new())
    }

    fn finish(self) -> arcweft_core::pattern::RuntimeSemanticTypeId {
        self.0.finish()
    }

    fn tag(&mut self, value: u16) {
        self.0.write_tag(value);
    }

    fn byte(&mut self, value: u8) {
        self.0.write_u8(value);
    }

    fn bool(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    fn u16(&mut self, value: u16) {
        self.0.write_u16(value);
    }

    fn u32(&mut self, value: u32) {
        self.0.write_u32(value);
    }

    fn u64(&mut self, value: u64) {
        self.0.write_u64(value);
    }

    fn bytes(&mut self, value: &[u8]) {
        self.0.write_bytes(value);
    }

    fn len(&mut self, value: usize) {
        self.0.write_len(value);
    }

    fn string(&mut self, value: &str) {
        self.0.write_str(value);
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
    fn ty(&mut self, ty: &TypeKind) {
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
                self.ty(inner);
            }
            TypeKind::IteratorState { family, item } => {
                self.tag(22);
                self.iterator_family(*family);
                self.ty(item);
            }
            TypeKind::DisplayText => self.tag(23),
            TypeKind::DebugStatePath => self.tag(24),
            TypeKind::ObservationFieldPath => self.tag(25),
            TypeKind::Ref(entity) => {
                self.tag(26);
                self.entity_kind(entity.kind());
                self.option(entity.value(), Self::ty);
            }
            TypeKind::Probe(inner) => {
                self.tag(27);
                self.ty(inner);
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
                self.ty(inner);
            }
            TypeKind::Array { item, len } => {
                self.tag(49);
                self.ty(item);
                self.array_length(len);
            }
            TypeKind::Slice(inner) => {
                self.tag(50);
                self.ty(inner);
            }
            TypeKind::Seq(inner) => {
                self.tag(51);
                self.ty(inner);
            }
            TypeKind::Map { kind, key, value } => {
                self.tag(52);
                self.map_kind(*kind);
                self.ty(key);
                self.ty(value);
            }
            TypeKind::BorrowRef {
                kind,
                lifetime,
                inner,
            } => {
                self.tag(53);
                self.borrow_kind(*kind);
                self.option(lifetime.as_ref(), Self::lifetime);
                self.ty(inner);
            }
            TypeKind::Need(value) => {
                self.tag(54);
                self.ty(value);
            }
            TypeKind::Stream { item, error } => {
                self.tag(55);
                self.ty(item);
                self.ty(error);
            }
            TypeKind::Parser { item, error } => {
                self.tag(56);
                self.ty(item);
                self.ty(error);
            }
            TypeKind::Result { ok, error } => {
                self.tag(57);
                self.ty(ok);
                self.ty(error);
            }
            TypeKind::Option(inner) => {
                self.tag(58);
                self.ty(inner);
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
                self.ty(inner);
            }
            TypeKind::Shared(inner) => {
                self.tag(61);
                self.ty(inner);
            }
            TypeKind::Function {
                params,
                return_type,
                effects,
            } => {
                self.tag(62);
                self.types(params);
                self.ty(return_type);
                self.effect_row(effects);
            }
            TypeKind::GenericParam(parameter) => {
                self.tag(63);
                self.generic_parameter(parameter);
            }
            TypeKind::ProjectNominal(nominal) => {
                self.tag(64);
                self.project_nominal(nominal);
            }
            TypeKind::AcceptedNominal(nominal) => {
                self.tag(65);
                self.accepted_nominal(nominal);
            }
            TypeKind::OpenNominal(nominal) => {
                self.tag(66);
                self.open_nominal(nominal);
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
                self.ty(subject);
                self.option(trait_name.as_ref(), |encoder, value| encoder.string(value));
                self.string(assoc);
            }
            TypeKind::CharacterDialogue(dialogue) => {
                dialogue.encode_runtime_semantic_identity(&mut self.0);
            }
            TypeKind::DialogueLine(result) => {
                self.tag(70);
                self.ty(result);
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
                self.types(items);
            }
            TypeKind::Choice(items) => {
                self.tag(76);
                self.types(items);
            }
            TypeKind::VariantPayload(payload) => {
                self.tag(87);
                self.bytes(payload.case().as_bytes());
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
        }
    }

    fn checked(&mut self, ty: &RuntimeCheckedType) {
        ty.encode_semantic_identity(&mut self.0);
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

    fn types(&mut self, types: &[TypeKind]) {
        self.len(types.len());
        for ty in types {
            self.ty(ty);
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

    fn generic_parameter(&mut self, parameter: &GenericTypeParameterId) {
        self.generic_owner(parameter.owner());
        self.u16(parameter.ordinal());
    }

    fn generic_const_parameter(&mut self, parameter: &GenericConstParameterId) {
        // The tag separates the type and constant parameter namespaces even
        // when a declaration happens to use the same ordinal in both.
        self.byte(0xC0);
        self.generic_owner(parameter.owner());
        self.u16(parameter.ordinal());
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

    fn project_nominal(&mut self, nominal: &ProjectNominalType) {
        self.project_nominal_declaration(nominal.declaration());
        self.types(nominal.arguments());
    }

    fn accepted_nominal(&mut self, nominal: &AcceptedNominalType) {
        self.accepted_nominal_id(nominal.declaration());
        self.types(nominal.arguments());
    }

    fn open_nominal(&mut self, nominal: &OpenNominalType) {
        self.open_rule(nominal.rule());
        self.hir_path(nominal.path());
        self.types(nominal.arguments());
    }

    fn callable_declaration(&mut self, id: &CallableDeclarationKey) {
        self.0.write_bytes(id.semantic_digest().as_bytes());
    }

    fn project_nominal_declaration(&mut self, id: &ProjectNominalDeclarationId) {
        self.project_world(id.world());
        self.0.write_bytes(id.revision().as_source_set().as_bytes());
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
        self.0.write_bytes(source.source().revision().as_bytes());
        self.u64(source.source().source_len());
        let range = source.range();
        self.u64(u64::try_from(range.start()).expect("source offsets fit u64"));
        self.u64(u64::try_from(range.end()).expect("source offsets fit u64"));
    }

    fn effect_row(&mut self, row: &EffectRow) {
        self.len(row.concrete().iter().len());
        for effect in row.concrete().iter() {
            self.string(effect.as_str());
        }
        match row.tail() {
            EffectRowTail::Closed => self.byte(0),
            EffectRowTail::Variable(variable) => {
                self.byte(1);
                self.bytes(variable.issuer().as_bytes());
                self.u32(variable.index());
            }
            EffectRowTail::Unknown => self.byte(2),
        }
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
mod tests {
    use arcweft_character::id::CharacterId;
    use arcweft_lang_syntax::{
        ast::{
            module_path::ModulePathRoot,
            symbol_path::{ProjectSymbolPath, ProjectSymbolSegment},
        },
        types::TypePath,
    };

    use crate::{
        env::{
            identity::EnvironmentBindingId,
            nominal::{AcceptedNominalId, AcceptedNominalOwnerId},
        },
        types::{AcceptedNominalType, CharacterDialogueType, TypeKind},
    };

    fn path(name: &str) -> TypePath {
        ProjectSymbolPath::new(
            ModulePathRoot::ImplicitCrate,
            [ProjectSymbolSegment::try_new(name).expect("segment")],
        )
        .expect("path")
        .into()
    }

    #[test]
    fn accepted_owner_and_nested_arguments_participate_in_identity() {
        let first = TypeKind::AcceptedNominal(AcceptedNominalType::new(
            AcceptedNominalId::new(
                AcceptedNominalOwnerId::Environment(
                    EnvironmentBindingId::try_new("adapter:first").expect("owner"),
                ),
                path("Value"),
            ),
            [TypeKind::Vec(Box::new(TypeKind::I32))],
        ));
        let owner_changed = TypeKind::AcceptedNominal(AcceptedNominalType::new(
            AcceptedNominalId::new(
                AcceptedNominalOwnerId::Environment(
                    EnvironmentBindingId::try_new("adapter:second").expect("owner"),
                ),
                path("Value"),
            ),
            [TypeKind::Vec(Box::new(TypeKind::I32))],
        ));
        let argument_changed = TypeKind::AcceptedNominal(AcceptedNominalType::new(
            AcceptedNominalId::new(
                AcceptedNominalOwnerId::Environment(
                    EnvironmentBindingId::try_new("adapter:first").expect("owner"),
                ),
                path("Value"),
            ),
            [TypeKind::Vec(Box::new(TypeKind::I64))],
        ));

        assert_eq!(
            first.semantic_identity_digest(),
            first.clone().semantic_identity_digest()
        );
        assert_ne!(
            first.semantic_identity_digest(),
            owner_changed.semantic_identity_digest()
        );
        assert_ne!(
            first.semantic_identity_digest(),
            argument_changed.semantic_identity_digest()
        );
    }

    #[test]
    fn character_dialogue_producer_and_type_kind_share_one_identity_authority() {
        let exact = CharacterDialogueType::exact(
            CharacterId::try_new("character.alice").expect("character ID"),
        );
        let any = CharacterDialogueType::any();
        assert_eq!(
            TypeKind::CharacterDialogue(exact.clone())
                .semantic_identity_digest()
                .as_bytes(),
            exact.runtime_semantic_identity().as_bytes()
        );
        assert_eq!(
            TypeKind::CharacterDialogue(any.clone())
                .semantic_identity_digest()
                .as_bytes(),
            any.runtime_semantic_identity().as_bytes()
        );
    }
}
