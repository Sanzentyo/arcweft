//! Canonical digest encodings for accepted callable schemas and catalogs.

use arcweft_lang_hir::symbol::CallableDeclarationKey;
use arcweft_source::SourceSpan;

use crate::{
    effect_row::EffectRowTail,
    registration::AcceptedNominalWorldStamp,
    types::{ArrayLength, TypeKind},
};

use super::schema::CallableParameterSemanticBinding;
use super::{
    CallableArgumentPolicy, CallableArgumentSemanticAction, CallableAuthorityRank,
    CallableDocumentation, CallableEffectSchema, CallableEvaluatedEffect, CallableGenericConstUse,
    CallableGenericFirstUse, CallableGenericParameterInventory, CallableGenericTypeUse,
    CallableGroupKind, CallableLogLevel, CallableLookupKey, CallableParameterAdmission,
    CallableParameterConsumer, CallableParameterPassing, CallableParameterPresence,
    CallableParameterValueAlternative, CallableProviderId, CallableSchemaGenericRole,
    CallableSemanticValueGuard, CallableSignatureSchema, CallableSource, CallableValidator,
    DocumentationProvenance, EnvironmentCallableId, EnvironmentCallableKind,
    EnvironmentCallableOwner, LanguageDocumentationFamily, ParameterExpectedTypeProjection,
    RustCallableProvenance, RustCallablePurity, RustPackageProvenance, SpreadArgumentPolicy,
    StandardEnvironmentId, UnknownNamedArgumentPolicy, VariantPayloadRequirement,
};

const SCHEMA_DOMAIN: &[u8] = b"arcweft.callable-signature.semantic.v1\0";

/// Stable semantic identity of one checked callable signature.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableSignatureSchemaDigest([u8; 32]);

/// Stable identity of one world-bound environment publication.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnvironmentCallablePublicationDigest([u8; 32]);

/// Stable identity of one complete registered callable catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RegisteredCallableCatalogDigest([u8; 32]);

macro_rules! digest_bytes {
    ($name:ident) => {
        impl $name {
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

digest_bytes!(CallableSignatureSchemaDigest);
digest_bytes!(EnvironmentCallablePublicationDigest);
digest_bytes!(RegisteredCallableCatalogDigest);

impl EnvironmentCallableId {
    /// Canonical bytes used to order an environment declaration inside typed
    /// checked identities. This is not a display spelling and is never parsed
    /// back into a callable.
    pub(crate) fn canonical_identity_bytes(&self) -> Vec<u8> {
        let mut encoder = CanonicalEncoder::default();
        encoder.environment_id(self);
        encoder.into_bytes()
    }
}

impl CallableSignatureSchema {
    /// Returns the canonical semantic digest of this checked signature.
    #[must_use]
    pub fn semantic_digest(&self) -> CallableSignatureSchemaDigest {
        let mut encoder = CanonicalEncoder::default();
        encoder.schema(self);
        CallableSignatureSchemaDigest(encoder.finish(SCHEMA_DOMAIN))
    }
}

#[derive(Default)]
pub(super) struct CanonicalEncoder {
    bytes: Vec<u8>,
}

impl CanonicalEncoder {
    pub(super) fn finish(self, domain: &[u8]) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain);
        hasher.update(&self.bytes);
        *hasher.finalize().as_bytes()
    }

    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub(super) fn tag(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn bool(&mut self, value: bool) {
        self.bytes.push(u8::from(value));
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn usize(&mut self, value: usize) {
        self.u32(
            u32::try_from(value)
                .expect("accepted callable digest sequences fit the checked u32 contract"),
        );
    }

    pub(super) fn bytes(&mut self, value: &[u8]) {
        self.usize(value.len());
        self.bytes.extend_from_slice(value);
    }

    pub(super) fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    pub(super) fn option<T: ?Sized>(
        &mut self,
        value: Option<&T>,
        encode: impl FnOnce(&mut Self, &T),
    ) {
        match value {
            Some(value) => {
                self.bool(true);
                encode(self, value);
            }
            None => self.bool(false),
        }
    }

    pub(super) fn schema(&mut self, schema: &CallableSignatureSchema) {
        self.usize(schema.groups().len());
        for group in schema.groups() {
            self.usize(group.index().get());
            self.tag(match group.kind() {
                CallableGroupKind::Initial => 0,
                CallableGroupKind::Curried => 1,
            });
            self.usize(group.parameters().len());
            for parameter in group.parameters() {
                self.usize(parameter.index().get());
                match parameter.semantic_binding() {
                    CallableParameterSemanticBinding::Coordinate => self.tag(0),
                    CallableParameterSemanticBinding::Named(name) => {
                        self.tag(1);
                        self.string(name.as_str());
                    }
                    CallableParameterSemanticBinding::AcceptedVariantPayloadField(field) => {
                        self.tag(2);
                        self.bytes(field.as_bytes());
                    }
                }
                self.tag(match parameter.passing() {
                    CallableParameterPassing::PositionalOnly => 0,
                    CallableParameterPassing::PositionalOrNamed => 1,
                    CallableParameterPassing::NamedOnly => 2,
                    CallableParameterPassing::RestPositional => 3,
                    CallableParameterPassing::RestNamed => 4,
                });
                self.tag(match parameter.presence() {
                    CallableParameterPresence::Required => 0,
                    CallableParameterPresence::Optional => 1,
                    CallableParameterPresence::Defaulted => 2,
                });
                self.admission(parameter.admission(), parameter.consumer());
            }
        }
        self.generic_inventory(schema.generic_inventory());
        self.bytes(schema.result().semantic_identity_digest().as_bytes());
        self.effect_schema(schema.effects());
        self.argument_policy(schema.argument_policy());
        self.usize(schema.reserved_open_names().len());
        for name in schema.reserved_open_names() {
            self.string(name.as_str());
        }
        self.validator(schema.validator());
        self.option(schema.evaluated_effect().as_ref(), |encoder, effect| {
            encoder.evaluated_effect(*effect);
        });
        self.option(schema.extension_receiver().as_ref(), |encoder, receiver| {
            encoder.usize(receiver.group().get());
            encoder.usize(receiver.parameter().get());
        });
    }

    fn generic_inventory(&mut self, inventory: &CallableGenericParameterInventory) {
        self.usize(inventory.types().len());
        for entry in inventory.types() {
            self.generic_type_use(entry);
        }
        self.usize(inventory.consts().len());
        for entry in inventory.consts() {
            self.generic_const_use(entry);
        }
    }

    fn generic_type_use(&mut self, entry: &CallableGenericTypeUse) {
        self.bytes(
            TypeKind::GenericParam(entry.parameter().clone())
                .semantic_identity_digest()
                .as_bytes(),
        );
        self.tag(match entry.role() {
            CallableSchemaGenericRole::Candidate => 0,
            CallableSchemaGenericRole::RigidReference => 1,
        });
        self.generic_first_use(entry.first_use());
    }

    fn generic_const_use(&mut self, entry: &CallableGenericConstUse) {
        self.bytes(
            TypeKind::Array {
                item: Box::new(TypeKind::Unit),
                len: ArrayLength::Generic(entry.parameter().clone()),
            }
            .semantic_identity_digest()
            .as_bytes(),
        );
        self.tag(match entry.role() {
            CallableSchemaGenericRole::Candidate => 0,
            CallableSchemaGenericRole::RigidReference => 1,
        });
        self.generic_first_use(entry.first_use());
    }

    fn generic_first_use(&mut self, first_use: CallableGenericFirstUse) {
        match first_use {
            CallableGenericFirstUse::Group(group) => {
                self.tag(0);
                self.usize(group.get());
            }
            CallableGenericFirstUse::Result => self.tag(1),
        }
    }

    fn admission(
        &mut self,
        admission: &CallableParameterAdmission,
        consumer: &CallableParameterConsumer,
    ) {
        match admission {
            CallableParameterAdmission::Checked { declared, rule } => {
                self.tag(0);
                self.bytes(declared.semantic_identity_digest().as_bytes());
                self.consumer(consumer);
                self.usize(rule.len());
                for alternative in rule.alternatives() {
                    self.value_alternative(alternative);
                }
            }
            CallableParameterAdmission::UncheckedSupply => self.tag(1),
        }
    }

    fn value_alternative(&mut self, alternative: CallableParameterValueAlternative<'_>) {
        match alternative.guard() {
            Some(CallableSemanticValueGuard::VariantCase {
                owner,
                ordinal,
                payload,
            }) => {
                self.tag(0);
                self.expected_projection(owner);
                self.u32(*ordinal);
                self.tag(match payload {
                    VariantPayloadRequirement::Unit => 0,
                    VariantPayloadRequirement::Present => 1,
                });
            }
            None => self.tag(1),
        }
        self.expected_projection(alternative.expected());
        self.tag(match alternative.action() {
            CallableArgumentSemanticAction::Supply => 0,
            CallableArgumentSemanticAction::Clear => 1,
        });
    }

    fn expected_projection(&mut self, projection: &ParameterExpectedTypeProjection) {
        match projection {
            ParameterExpectedTypeProjection::Identity => self.tag(0),
            ParameterExpectedTypeProjection::ApplyUnary(constructor) => {
                self.tag(1);
                self.tag(match constructor {
                    super::CallableUnaryTypeConstructor::Option => 0,
                });
            }
        }
    }

    fn consumer(&mut self, consumer: &CallableParameterConsumer) {
        match consumer {
            CallableParameterConsumer::Value => self.tag(0),
            CallableParameterConsumer::DialoguePatch(coordinate) => {
                self.tag(1);
                self.dialogue_field_coordinate(coordinate);
            }
            CallableParameterConsumer::DialogueApplicationMetadata(coordinate) => {
                self.tag(2);
                self.tag(match coordinate {
                    super::DialogueApplicationMetadataCoordinate::Id => 0,
                    super::DialogueApplicationMetadataCoordinate::TextKey => 1,
                });
            }
        }
    }

    fn dialogue_field_coordinate(
        &mut self,
        coordinate: &crate::character_dialogue::CharacterDialogueFieldCoordinate,
    ) {
        use crate::character_dialogue::CharacterDialogueFieldCoordinate;
        self.tag(match coordinate {
            CharacterDialogueFieldCoordinate::Voice => 0,
            CharacterDialogueFieldCoordinate::Look => 1,
            CharacterDialogueFieldCoordinate::Stage => 2,
            CharacterDialogueFieldCoordinate::Portrait => 3,
            CharacterDialogueFieldCoordinate::Focus => 4,
            CharacterDialogueFieldCoordinate::Cleanup => 5,
            CharacterDialogueFieldCoordinate::View => 6,
            CharacterDialogueFieldCoordinate::SourceLocale => 7,
            CharacterDialogueFieldCoordinate::Hooks => 8,
            CharacterDialogueFieldCoordinate::Style => 9,
            CharacterDialogueFieldCoordinate::RichText => 10,
            CharacterDialogueFieldCoordinate::InlineFailure => 11,
            CharacterDialogueFieldCoordinate::Custom(_) => 12,
        });
        if let CharacterDialogueFieldCoordinate::Custom(id) = coordinate {
            self.string(id.as_str());
        }
    }

    fn evaluated_effect(&mut self, effect: CallableEvaluatedEffect) {
        match effect {
            CallableEvaluatedEffect::Log(level) => {
                self.tag(0);
                self.tag(match level {
                    CallableLogLevel::Trace => 0,
                    CallableLogLevel::Debug => 1,
                    CallableLogLevel::Info => 2,
                    CallableLogLevel::Warn => 3,
                    CallableLogLevel::Error => 4,
                });
            }
            CallableEvaluatedEffect::SignalWrite => self.tag(1),
            CallableEvaluatedEffect::MetricWrite => self.tag(2),
            CallableEvaluatedEffect::EmitEvent => self.tag(3),
            CallableEvaluatedEffect::Panic => self.tag(4),
            CallableEvaluatedEffect::Fail => self.tag(5),
            CallableEvaluatedEffect::Bail => self.tag(6),
            CallableEvaluatedEffect::Ensure => self.tag(7),
            CallableEvaluatedEffect::Drop(operation) => {
                self.tag(8);
                self.tag(match operation {
                    super::DropCallableId::Drop => 0,
                    super::DropCallableId::DropWithPolicy => 1,
                    super::DropCallableId::DropOptional => 2,
                    super::DropCallableId::OnDrop => 3,
                });
            }
        }
    }

    fn effect_schema(&mut self, effects: &CallableEffectSchema) {
        match effects {
            CallableEffectSchema::Fixed(row) => {
                self.tag(0);
                self.usize(row.concrete().len());
                for effect in row.concrete().iter() {
                    self.string(effect.as_str());
                }
                match row.tail() {
                    EffectRowTail::Closed => self.tag(0),
                    EffectRowTail::Variable(variable) => {
                        self.tag(1);
                        self.bytes(variable.issuer().as_bytes());
                        self.u32(variable.index());
                    }
                    EffectRowTail::Unknown => self.tag(2),
                }
            }
            CallableEffectSchema::Project { declaration } => {
                self.tag(1);
                self.project_declaration(declaration);
            }
            CallableEffectSchema::Detached { declaration } => {
                self.tag(2);
                self.tag(declaration.owner().digest_tag().into());
                self.u32(declaration.source_ordinal());
            }
        }
    }

    fn argument_policy(&mut self, policy: CallableArgumentPolicy) {
        self.tag(match policy.unknown_named() {
            UnknownNamedArgumentPolicy::Reject => 0,
            UnknownNamedArgumentPolicy::OpenSupply => 1,
        });
        self.tag(match policy.spread() {
            SpreadArgumentPolicy::Reject => 0,
            SpreadArgumentPolicy::FixedLiteralOnly => 1,
            SpreadArgumentPolicy::TypedRest => 2,
            SpreadArgumentPolicy::Unchecked => 3,
        });
    }

    fn validator(&mut self, validator: &CallableValidator) {
        self.tag(match validator {
            CallableValidator::Ordinary => 0,
            CallableValidator::Fx(_) => 1,
            CallableValidator::UnknownFxMember { .. } => 2,
            CallableValidator::EnumConstructor(_) => 3,
            CallableValidator::ResultConstructor(_) => 4,
            CallableValidator::OptionConstructor(_) => 5,
            CallableValidator::ReductionConstructor(_) => 6,
            CallableValidator::Builtin(_) => 7,
            CallableValidator::Agent(_) => 8,
            CallableValidator::Presentation(_) => 9,
            CallableValidator::Dialogue(_) => 10,
            CallableValidator::Collection(_) => 11,
            CallableValidator::PresentationHandle(_) => 12,
            CallableValidator::Integer(_) => 13,
            CallableValidator::Domain(_) => 14,
            CallableValidator::Method(_) => 15,
            CallableValidator::Capacity(_) => 16,
            CallableValidator::Stage(_) => 17,
            CallableValidator::Drop(_) => 18,
            CallableValidator::Promotion(_) => 19,
            CallableValidator::LineContext(_) => 20,
            CallableValidator::ViewModifier(_) => 21,
            CallableValidator::StandardMap(_) => 22,
        });
        if let CallableValidator::Method(role) = validator {
            self.tag(match role {
                super::CallableMethodRole::TraitRequirement => 0,
                super::CallableMethodRole::TraitImplementation => 1,
                super::CallableMethodRole::Inherent => 2,
            });
        }
        if let CallableValidator::Dialogue(id) = validator {
            self.tag(match id {
                super::DialogueCallableId::CharacterFactory => 0,
                super::DialogueCallableId::CharacterReconfigure => 1,
                super::DialogueCallableId::ContentApplication => 2,
                super::DialogueCallableId::ContentCall => 3,
            });
        }
        if let CallableValidator::ViewModifier(modifier) = validator {
            self.tag(u16::from(modifier.semantic_tag()));
        }
        if let CallableValidator::Drop(operation) = validator {
            self.tag(match operation {
                super::DropCallableId::Drop => 0,
                super::DropCallableId::DropWithPolicy => 1,
                super::DropCallableId::DropOptional => 2,
                super::DropCallableId::OnDrop => 3,
            });
        }
        if let CallableValidator::StandardMap(family) = validator {
            self.tag(u16::from(family.intrinsic_owner_tag()));
        }
    }

    pub(super) fn nominal_world(&mut self, stamp: &AcceptedNominalWorldStamp) {
        self.string(stamp.world().package().as_str());
        self.string(stamp.world().root_document().as_str());
        self.string(stamp.world().profile());
        self.bytes(stamp.revision().as_source_set().as_bytes());
        self.bytes(stamp.catalog_digest().as_bytes());
    }

    pub(super) fn environment_owner(&mut self, owner: &EnvironmentCallableOwner) {
        match owner {
            EnvironmentCallableOwner::Standard(owner) => {
                self.tag(0);
                self.standard_owner(*owner);
            }
            EnvironmentCallableOwner::Adapter(owner) => {
                self.tag(1);
                self.string(owner.as_str());
            }
        }
    }

    fn standard_owner(&mut self, owner: StandardEnvironmentId) {
        self.tag(match owner {
            StandardEnvironmentId::Core => 0,
            StandardEnvironmentId::SansIo => 1,
            StandardEnvironmentId::NativeHttp => 2,
            StandardEnvironmentId::InferenceTensor => 3,
            StandardEnvironmentId::SystemInfo => 4,
            StandardEnvironmentId::NativeFile => 5,
            StandardEnvironmentId::Math => 6,
        });
    }

    pub(super) fn environment_kind(&mut self, kind: EnvironmentCallableKind) {
        self.tag(match kind {
            EnvironmentCallableKind::Function => 0,
            EnvironmentCallableKind::Method => 1,
            EnvironmentCallableKind::RustFunction => 2,
        });
    }

    pub(super) fn lookup_key(&mut self, key: &CallableLookupKey) {
        match key {
            CallableLookupKey::Free(path) => {
                self.tag(0);
                self.usize(path.segments().len());
                for segment in path.segments() {
                    self.string(segment.as_str());
                }
            }
            CallableLookupKey::Method(method) => {
                self.tag(1);
                self.bytes(method.receiver().semantic_identity_digest().as_bytes());
                self.string(method.method().as_str());
            }
        }
    }

    pub(super) fn environment_id(&mut self, id: &EnvironmentCallableId) {
        self.environment_owner(id.owner());
        self.environment_kind(id.kind());
        self.lookup_key(id.key());
        self.usize(id.overload().get());
    }

    pub(super) fn project_declaration(&mut self, id: &CallableDeclarationKey) {
        self.bytes(id.semantic_digest().as_bytes());
    }

    pub(super) fn authority(&mut self, authority: CallableAuthorityRank) {
        self.tag(match authority {
            CallableAuthorityRank::Project => 0,
            CallableAuthorityRank::Standard => 1,
            CallableAuthorityRank::Adapter => 2,
        });
    }

    pub(super) fn provider(&mut self, provider: &CallableProviderId) {
        match provider {
            CallableProviderId::Project(package) => {
                self.tag(0);
                self.string(package.as_str());
            }
            CallableProviderId::Standard(owner) => {
                self.tag(1);
                self.standard_owner(*owner);
            }
            CallableProviderId::Adapter(owner) => {
                self.tag(2);
                self.string(owner.as_str());
            }
        }
    }

    pub(super) fn rust_provenance(&mut self, rust: &RustCallableProvenance) {
        self.string(rust.adapter().as_str());
        self.rust_package(rust.package());
        self.string(rust.rust_path().as_str());
        self.tag(match rust.purity() {
            RustCallablePurity::External => 0,
            RustCallablePurity::Pure => 1,
            RustCallablePurity::Task => 2,
        });
    }

    fn rust_package(&mut self, package: &RustPackageProvenance) {
        self.string(package.name());
        self.string(package.version());
        self.option(package.metadata_hash(), |encoder, hash| {
            encoder.string(hash);
        });
    }

    pub(super) fn documentation(&mut self, documentation: &CallableDocumentation) {
        self.option(documentation.summary(), |encoder, summary| {
            encoder.string(summary);
        });
        self.option(documentation.details(), |encoder, details| {
            encoder.string(details);
        });
        let mut parameters = documentation.parameters().iter().collect::<Vec<_>>();
        parameters.sort_by_key(|entry| (entry.group(), entry.parameter()));
        self.usize(parameters.len());
        for parameter in parameters {
            self.usize(parameter.group().get());
            self.usize(parameter.parameter().get());
            self.string(parameter.text());
        }
        match documentation.provenance() {
            DocumentationProvenance::Missing => self.tag(0),
            DocumentationProvenance::ProjectSource { declaration } => {
                self.tag(1);
                self.project_declaration(declaration);
            }
            DocumentationProvenance::AdapterTooling { package } => {
                self.tag(2);
                self.string(package.as_str());
            }
            DocumentationProvenance::RustMetadata {
                adapter,
                package,
                item,
            } => {
                self.tag(3);
                self.string(adapter.as_str());
                self.rust_package(package);
                self.string(item.as_str());
            }
            DocumentationProvenance::Language { family } => {
                self.tag(4);
                self.tag(match family {
                    LanguageDocumentationFamily::Builtin => 0,
                    LanguageDocumentationFamily::Fx => 1,
                    LanguageDocumentationFamily::Agent => 2,
                    LanguageDocumentationFamily::Presentation => 3,
                    LanguageDocumentationFamily::Collection => 4,
                    LanguageDocumentationFamily::Domain => 5,
                    LanguageDocumentationFamily::Integer => 6,
                    LanguageDocumentationFamily::Capacity => 7,
                    LanguageDocumentationFamily::Trait => 8,
                    LanguageDocumentationFamily::Constructor => 9,
                });
            }
        }
    }

    pub(super) fn source(&mut self, source: &CallableSource) {
        self.option(source.declaration(), Self::project_declaration);
        self.option(source.signature(), Self::source_span);
        self.option(source.name(), Self::source_span);
        self.option(source.result(), Self::source_span);
        let mut parameters = source.parameters().iter().collect::<Vec<_>>();
        parameters.sort_by_key(|entry| (entry.group(), entry.parameter()));
        self.usize(parameters.len());
        for parameter in parameters {
            self.usize(parameter.group().get());
            self.usize(parameter.parameter().get());
            self.source_span(parameter.whole());
            self.option(parameter.name(), Self::source_span);
            self.option(parameter.ty(), Self::source_span);
            self.option(parameter.default(), Self::source_span);
        }
    }

    pub(super) fn source_span(&mut self, span: &SourceSpan) {
        self.string(span.source().id().as_str());
        self.bytes(span.source().revision().as_bytes());
        self.u64(span.source().source_len());
        self.usize(span.range().start());
        self.usize(span.range().end());
    }
}

#[cfg(test)]
mod tests {
    use super::CanonicalEncoder;
    use crate::callable::{
        CallableMethodRole, CallableParameterAdmission, CallableParameterConsumer,
        CallableParameterValueRule, CallableValidator,
    };
    use crate::character_dialogue::CharacterDialogueFieldCoordinate;
    use crate::types::TypeKind;

    #[test]
    fn method_validator_uses_compact_family_tag_and_exact_role_subtag() {
        for (role, role_tag) in [
            (CallableMethodRole::TraitRequirement, 0_u16),
            (CallableMethodRole::TraitImplementation, 1_u16),
            (CallableMethodRole::Inherent, 2_u16),
        ] {
            let mut encoder = CanonicalEncoder::default();
            encoder.validator(&CallableValidator::Method(role));

            let mut expected = Vec::new();
            expected.extend_from_slice(&15_u16.to_le_bytes());
            expected.extend_from_slice(&role_tag.to_le_bytes());
            assert_eq!(encoder.into_bytes(), expected);
        }
    }

    #[test]
    fn checked_admission_golden_order_is_declared_consumer_then_alternatives() {
        let admission = CallableParameterAdmission::checked_with_rule(
            TypeKind::String,
            CallableParameterValueRule::clearable_option(),
        );
        let consumer =
            CallableParameterConsumer::DialoguePatch(CharacterDialogueFieldCoordinate::Voice);
        let mut actual = CanonicalEncoder::default();
        actual.admission(&admission, &consumer);

        let CallableParameterAdmission::Checked { declared, rule } = &admission else {
            unreachable!("test admission is checked")
        };
        let mut expected = CanonicalEncoder::default();
        expected.tag(0);
        expected.bytes(declared.semantic_identity_digest().as_bytes());
        expected.consumer(&consumer);
        expected.usize(rule.len());
        for alternative in rule.alternatives() {
            expected.value_alternative(alternative);
        }
        assert_eq!(actual.into_bytes(), expected.into_bytes());
    }

    #[test]
    fn unchecked_admission_golden_bytes_have_no_consumer_payload() {
        let consumer =
            CallableParameterConsumer::DialoguePatch(CharacterDialogueFieldCoordinate::Voice);
        let mut encoder = CanonicalEncoder::default();
        encoder.admission(&CallableParameterAdmission::UncheckedSupply, &consumer);
        assert_eq!(encoder.into_bytes(), vec![1, 0]);
    }

    #[test]
    fn alternative_and_consumer_tampering_change_schema_admission_bytes() {
        let consumer = CallableParameterConsumer::Value;
        let supply = CallableParameterAdmission::checked(TypeKind::String);
        let clear = CallableParameterAdmission::checked_with_rule(
            TypeKind::String,
            CallableParameterValueRule::clearable_option(),
        );
        let mut supply_bytes = CanonicalEncoder::default();
        supply_bytes.admission(&supply, &consumer);
        let mut clear_bytes = CanonicalEncoder::default();
        clear_bytes.admission(&clear, &consumer);
        assert_ne!(supply_bytes.into_bytes(), clear_bytes.into_bytes());

        let mut value_bytes = CanonicalEncoder::default();
        value_bytes.admission(&supply, &CallableParameterConsumer::Value);
        let mut dialogue_bytes = CanonicalEncoder::default();
        dialogue_bytes.admission(
            &supply,
            &CallableParameterConsumer::DialoguePatch(CharacterDialogueFieldCoordinate::Voice),
        );
        assert_ne!(value_bytes.into_bytes(), dialogue_bytes.into_bytes());
    }
}
