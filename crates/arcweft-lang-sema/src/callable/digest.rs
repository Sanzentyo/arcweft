//! Canonical digest encodings for accepted callable schemas and catalogs.

use arcweft_lang_hir::symbol::CallableDeclarationKey;
use arcweft_source::SourceSpan;

use crate::{effect_row::EffectRowTail, registration::AcceptedNominalWorldStamp};

use super::{
    CallableArgumentPolicy, CallableAuthorityRank, CallableDocumentation, CallableEffectSchema,
    CallableEvaluatedEffect, CallableGroupKind, CallableLogLevel, CallableLookupKey,
    CallableParameterPassing, CallableParameterPresence, CallableParameterType, CallableProviderId,
    CallableSignatureSchema, CallableSource, CallableValidator, DocumentationProvenance,
    EnvironmentCallableId, EnvironmentCallableKind, EnvironmentCallableOwner,
    LanguageDocumentationFamily, RustCallableProvenance, RustCallablePurity, RustPackageProvenance,
    SpreadArgumentPolicy, StandardEnvironmentId, UnknownNamedArgumentPolicy,
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
                self.option(parameter.name(), |encoder, name| {
                    encoder.string(name.as_str());
                });
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
                match parameter.ty() {
                    CallableParameterType::Exact(ty) => {
                        self.tag(0);
                        self.bytes(ty.semantic_identity_digest().as_bytes());
                    }
                    CallableParameterType::Unchecked => self.tag(1),
                }
            }
        }
        self.bytes(schema.result().semantic_identity_digest().as_bytes());
        self.effect_schema(schema.effects());
        self.argument_policy(schema.argument_policy());
        self.validator(schema.validator());
        self.option(schema.evaluated_effect().as_ref(), |encoder, effect| {
            encoder.evaluated_effect(*effect);
        });
        self.option(schema.extension_receiver().as_ref(), |encoder, receiver| {
            encoder.usize(receiver.group().get());
            encoder.usize(receiver.parameter().get());
        });
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
            UnknownNamedArgumentPolicy::OpenChecked => 1,
            UnknownNamedArgumentPolicy::OpenUnchecked => 2,
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
            CallableValidator::Untyped => 1,
            CallableValidator::Fx(_) => 2,
            CallableValidator::UnknownFxMember { .. } => 3,
            CallableValidator::EnumConstructor(_) => 4,
            CallableValidator::ResultConstructor(_) => 5,
            CallableValidator::OptionConstructor(_) => 6,
            CallableValidator::ReductionConstructor(_) => 7,
            CallableValidator::Builtin(_) => 8,
            CallableValidator::Agent(_) => 9,
            CallableValidator::Presentation(_) => 10,
            CallableValidator::Dialogue(_) => 11,
            CallableValidator::Collection(_) => 12,
            CallableValidator::PresentationHandle(_) => 13,
            CallableValidator::Integer(_) => 14,
            CallableValidator::Domain(_) => 15,
            CallableValidator::Method(_) => 16,
            CallableValidator::Capacity(_) => 17,
            CallableValidator::Stage(_) => 18,
            CallableValidator::Drop => 19,
            CallableValidator::Promotion(_) => 20,
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
            EnvironmentCallableKind::UntypedMethodFallback => 2,
            EnvironmentCallableKind::RustFunction => 3,
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
    use crate::callable::{CallableMethodRole, CallableValidator};

    #[test]
    fn method_validator_replaces_reserved_tag_sixteen_with_exact_role_subtag() {
        for (role, role_tag) in [
            (CallableMethodRole::TraitRequirement, 0_u16),
            (CallableMethodRole::TraitImplementation, 1_u16),
            (CallableMethodRole::Inherent, 2_u16),
        ] {
            let mut encoder = CanonicalEncoder::default();
            encoder.validator(&CallableValidator::Method(role));

            let mut expected = Vec::new();
            expected.extend_from_slice(&16_u16.to_le_bytes());
            expected.extend_from_slice(&role_tag.to_le_bytes());
            assert_eq!(encoder.into_bytes(), expected);
        }
    }
}
