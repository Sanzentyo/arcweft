//! Validated environment callable publication records.

use crate::registration::{AcceptedNominalWorldStamp, EnvironmentManifestDigest};

use super::digest::CanonicalEncoder;
use super::{
    CallableDocumentation, CallableLimits, CallableLookupKey, CallablePublicationError,
    CallableSignatureSchema, CallableSource, EnvironmentCallableKind, EnvironmentCallableOwner,
    EnvironmentCallablePublicationDigest, EnvironmentDeclarationOrdinal, RustCallableProvenance,
};

const PUBLICATION_DOMAIN: &[u8] = b"arcweft.environment-publication.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallablePublication {
    owner: EnvironmentCallableOwner,
    nominal_world: AcceptedNominalWorldStamp,
    manifest_digest: EnvironmentManifestDigest,
    records: Box<[EnvironmentCallablePublicationRecord]>,
    digest: EnvironmentCallablePublicationDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallablePublicationRecord {
    kind: EnvironmentCallableKind,
    key: CallableLookupKey,
    overload: super::CallableOverloadIndex,
    schema: CallableSignatureSchema,
    documentation: CallableDocumentation,
    source: Option<CallableSource>,
    rust: Option<RustCallableProvenance>,
    declaration_order: EnvironmentDeclarationOrdinal,
}

impl EnvironmentCallablePublication {
    pub(crate) fn try_new_projected(
        owner: EnvironmentCallableOwner,
        nominal_world: AcceptedNominalWorldStamp,
        manifest_digest: EnvironmentManifestDigest,
        records: Vec<EnvironmentCallablePublicationRecord>,
        limits: &CallableLimits,
    ) -> Result<Self, CallablePublicationError> {
        if records.len() > limits.max_catalog_records() {
            return Err(super::CallableBuildLimitError::Records {
                actual: records.len(),
                limit: limits.max_catalog_records(),
            }
            .into());
        }
        let digest = publication_digest(&owner, &nominal_world, manifest_digest, &records);
        Ok(Self {
            owner,
            nominal_world,
            manifest_digest,
            records: records.into(),
            digest,
        })
    }

    pub const fn owner(&self) -> &EnvironmentCallableOwner {
        &self.owner
    }

    pub const fn nominal_world(&self) -> &AcceptedNominalWorldStamp {
        &self.nominal_world
    }

    pub const fn manifest_digest(&self) -> EnvironmentManifestDigest {
        self.manifest_digest
    }

    pub fn records(&self) -> &[EnvironmentCallablePublicationRecord] {
        &self.records
    }

    pub const fn digest(&self) -> EnvironmentCallablePublicationDigest {
        self.digest
    }
}

impl EnvironmentCallablePublicationRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        kind: EnvironmentCallableKind,
        key: CallableLookupKey,
        overload: super::CallableOverloadIndex,
        schema: CallableSignatureSchema,
        documentation: CallableDocumentation,
        source: Option<CallableSource>,
        rust: Option<RustCallableProvenance>,
        declaration_order: EnvironmentDeclarationOrdinal,
    ) -> Result<Self, CallablePublicationError> {
        if kind == EnvironmentCallableKind::RustFunction && rust.is_none() {
            return Err(super::CallableCatalogError::MissingRustProvenance.into());
        }
        Ok(Self {
            kind,
            key,
            overload,
            schema,
            documentation,
            source,
            rust,
            declaration_order,
        })
    }
    pub const fn kind(&self) -> EnvironmentCallableKind {
        self.kind
    }
    pub const fn key(&self) -> &CallableLookupKey {
        &self.key
    }
    pub const fn overload(&self) -> super::CallableOverloadIndex {
        self.overload
    }
    pub const fn schema(&self) -> &CallableSignatureSchema {
        &self.schema
    }
    pub const fn documentation(&self) -> &CallableDocumentation {
        &self.documentation
    }
    pub const fn source(&self) -> Option<&CallableSource> {
        self.source.as_ref()
    }
    pub const fn rust(&self) -> Option<&RustCallableProvenance> {
        self.rust.as_ref()
    }
    pub const fn declaration_order(&self) -> EnvironmentDeclarationOrdinal {
        self.declaration_order
    }
}

fn publication_digest(
    owner: &EnvironmentCallableOwner,
    nominal_world: &AcceptedNominalWorldStamp,
    manifest_digest: EnvironmentManifestDigest,
    records: &[EnvironmentCallablePublicationRecord],
) -> EnvironmentCallablePublicationDigest {
    let mut records = records.iter().collect::<Vec<_>>();
    records.sort_by(|left, right| {
        left.declaration_order()
            .cmp(&right.declaration_order())
            .then_with(|| lookup_key_bytes(left.key()).cmp(&lookup_key_bytes(right.key())))
            .then_with(|| left.overload().cmp(&right.overload()))
    });

    let mut encoder = CanonicalEncoder::default();
    encoder.nominal_world(nominal_world);
    encoder.environment_owner(owner);
    encoder.bytes(manifest_digest.as_bytes());
    encoder.usize(records.len());
    for record in records {
        encoder.environment_kind(record.kind());
        encoder.lookup_key(record.key());
        encoder.usize(record.overload().get());
        encoder.bytes(record.schema().semantic_digest().as_bytes());
        encoder.option(record.rust(), CanonicalEncoder::rust_provenance);
        encoder.documentation(record.documentation());
        encoder.option(record.source(), CanonicalEncoder::source);
        encoder.usize(record.declaration_order().get());
    }
    EnvironmentCallablePublicationDigest::from_bytes(encoder.finish(PUBLICATION_DOMAIN))
}

fn lookup_key_bytes(key: &CallableLookupKey) -> Vec<u8> {
    let mut encoder = CanonicalEncoder::default();
    encoder.lookup_key(key);
    encoder.into_bytes()
}
