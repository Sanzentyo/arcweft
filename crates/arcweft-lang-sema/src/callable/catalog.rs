//! Immutable callable catalog records and read-only indexes.
use std::{cmp::Ordering, collections::HashMap, num::NonZeroU32, sync::Arc};

use arcweft_lang_hir::symbol::CallableDeclarationId;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::SourceDocumentIdentity;

use super::digest::CanonicalEncoder;
use super::{
    CallableAuthorityRank, CallableCandidateId, CallableCatalogError, CallableDocumentation,
    CallableLimits, CallableLookupKey, CallableProviderId, CallableSignatureSchema, CallableSource,
    EnvironmentCallableId, EnvironmentCallableKind, EnvironmentCallablePublicationDigest,
    ProjectCallablePath, ProjectNameBinding, RustCallableProvenance, SignatureOrigin,
};
use crate::registration::AcceptedNominalWorldStamp;

const CATALOG_DOMAIN: &[u8] = b"arcweft.registered-callable-catalog.v1\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnvironmentDeclarationOrdinal(u32);

impl EnvironmentDeclarationOrdinal {
    pub fn try_from_usize(value: usize) -> Result<Self, super::CallableScalarError> {
        u32::try_from(value)
            .map(Self)
            .map_err(|_| super::CallableScalarError::IndexOverflow {
                kind: super::CallableIndexKind::FunctionValue,
                value,
            })
    }
    pub const fn get(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableRecord {
    id: CallableCandidateId,
    key: CallableLookupKey,
    authority: CallableAuthorityRank,
    provider: CallableProviderId,
    schema: Arc<CallableSignatureSchema>,
    documentation: CallableDocumentation,
    source: Option<CallableSource>,
    rust: Option<RustCallableProvenance>,
    publication_digest: Option<EnvironmentCallablePublicationDigest>,
    declaration_order: EnvironmentDeclarationOrdinal,
}

impl CallableRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        id: CallableCandidateId,
        key: CallableLookupKey,
        authority: CallableAuthorityRank,
        provider: CallableProviderId,
        schema: Arc<CallableSignatureSchema>,
        documentation: CallableDocumentation,
        source: Option<CallableSource>,
        rust: Option<RustCallableProvenance>,
        publication_digest: Option<EnvironmentCallablePublicationDigest>,
        declaration_order: EnvironmentDeclarationOrdinal,
    ) -> Result<Self, CallableCatalogError> {
        match &id {
            CallableCandidateId::Project(declaration) => {
                if authority != CallableAuthorityRank::Project
                    || provider != CallableProviderId::Project(declaration.package().clone())
                {
                    return Err(CallableCatalogError::AuthorityProviderMismatch);
                }
                let CallableLookupKey::Free(path) = &key else {
                    return Err(CallableCatalogError::IdKeyMismatch);
                };
                if path.leaf().as_str() != declaration.name() {
                    return Err(CallableCatalogError::IdKeyMismatch);
                }
                if source.as_ref().and_then(CallableSource::declaration) != Some(declaration) {
                    return Err(CallableCatalogError::MissingProjectSource);
                }
                if rust.is_some() {
                    return Err(CallableCatalogError::UnexpectedProjectRustProvenance);
                }
                if publication_digest.is_some() {
                    return Err(CallableCatalogError::UnexpectedProjectPublicationDigest);
                }
            }
            CallableCandidateId::Environment(environment) => {
                if environment.key() != &key
                    || environment.owner().authority() != authority
                    || environment.owner().provider() != provider
                {
                    return Err(CallableCatalogError::IdKeyMismatch);
                }
                if environment.kind() == EnvironmentCallableKind::RustFunction && rust.is_none() {
                    return Err(CallableCatalogError::MissingRustProvenance);
                }
                if environment.kind() == EnvironmentCallableKind::UntypedMethodFallback
                    && (!matches!(key, CallableLookupKey::Method(_))
                        || !matches!(schema.validator(), super::CallableValidator::Untyped))
                {
                    return Err(CallableCatalogError::IdKeyMismatch);
                }
                if publication_digest.is_none() {
                    return Err(CallableCatalogError::MissingEnvironmentPublicationDigest);
                }
            }
            _ => return Err(CallableCatalogError::IdKeyMismatch),
        }
        validate_schema_evidence(&schema, &documentation, source.as_ref())?;
        Ok(Self {
            id,
            key,
            authority,
            provider,
            schema,
            documentation,
            source,
            rust,
            publication_digest,
            declaration_order,
        })
    }

    pub const fn id(&self) -> &CallableCandidateId {
        &self.id
    }
    pub const fn key(&self) -> &CallableLookupKey {
        &self.key
    }
    pub const fn authority(&self) -> CallableAuthorityRank {
        self.authority
    }
    pub const fn provider(&self) -> &CallableProviderId {
        &self.provider
    }
    pub fn schema(&self) -> &CallableSignatureSchema {
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
    pub const fn publication_digest(&self) -> Option<EnvironmentCallablePublicationDigest> {
        self.publication_digest
    }
    pub const fn declaration_order(&self) -> EnvironmentDeclarationOrdinal {
        self.declaration_order
    }
}

fn validate_schema_evidence(
    schema: &CallableSignatureSchema,
    documentation: &CallableDocumentation,
    source: Option<&CallableSource>,
) -> Result<(), CallableCatalogError> {
    if documentation.parameters().iter().any(|entry| {
        schema
            .group(entry.group())
            .and_then(|group| group.parameter(entry.parameter()))
            .is_none()
    }) {
        return Err(CallableCatalogError::IdKeyMismatch);
    }
    if source.is_some_and(|source| {
        source.parameters().iter().any(|entry| {
            schema
                .group(entry.group())
                .and_then(|group| group.parameter(entry.parameter()))
                .is_none()
        })
    }) {
        return Err(CallableCatalogError::IdKeyMismatch);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EquivalentCallableSource {
    id: CallableCandidateId,
    origin: SignatureOrigin,
    documentation: CallableDocumentation,
    source: Option<CallableSource>,
    rust: Option<RustCallableProvenance>,
}

impl EquivalentCallableSource {
    pub fn new(
        id: CallableCandidateId,
        origin: SignatureOrigin,
        documentation: CallableDocumentation,
        source: Option<CallableSource>,
        rust: Option<RustCallableProvenance>,
    ) -> Self {
        Self {
            id,
            origin,
            documentation,
            source,
            rust,
        }
    }
    pub const fn id(&self) -> &CallableCandidateId {
        &self.id
    }
    pub const fn origin(&self) -> &SignatureOrigin {
        &self.origin
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogCallableEntry {
    primary: Arc<CallableRecord>,
    equivalent_sources: Arc<[EquivalentCallableSource]>,
}

impl CatalogCallableEntry {
    pub(crate) fn try_new(
        primary: Arc<CallableRecord>,
        equivalent_sources: Vec<EquivalentCallableSource>,
        limits: &CallableLimits,
    ) -> Result<Self, CallableCatalogError> {
        if equivalent_sources.len().saturating_add(1) > limits.max_overloads_per_key() {
            return Err(CallableCatalogError::OverloadLimit {
                actual: equivalent_sources.len() + 1,
                limit: limits.max_overloads_per_key(),
            });
        }
        let mut ids = std::collections::HashSet::new();
        ids.insert(primary.id().clone());
        if equivalent_sources
            .iter()
            .any(|source| !ids.insert(source.id().clone()))
        {
            return Err(CallableCatalogError::CandidateSetKeyMismatch);
        }
        Ok(Self {
            primary,
            equivalent_sources: equivalent_sources.into(),
        })
    }
    pub const fn primary(&self) -> &Arc<CallableRecord> {
        &self.primary
    }
    pub fn equivalent_sources(&self) -> &[EquivalentCallableSource] {
        &self.equivalent_sources
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonEmptyCallableSet {
    entries: Arc<[CatalogCallableEntry]>,
}

impl NonEmptyCallableSet {
    pub(crate) fn try_new(
        mut entries: Vec<CatalogCallableEntry>,
        limits: &CallableLimits,
    ) -> Result<Self, CallableCatalogError> {
        let Some(first) = entries.first() else {
            return Err(CallableCatalogError::EmptyCandidateSet);
        };
        let key = first.primary.key().clone();
        if entries.len() > limits.max_overloads_per_key() {
            return Err(CallableCatalogError::OverloadLimit {
                actual: entries.len(),
                limit: limits.max_overloads_per_key(),
            });
        }
        if entries.iter().any(|entry| {
            entry.primary.key() != &key
                || entry.primary.authority() == CallableAuthorityRank::Project
        }) {
            return Err(CallableCatalogError::CandidateSetKeyMismatch);
        }
        let mut ids = std::collections::HashSet::new();
        if entries.iter().any(|entry| {
            !ids.insert(entry.primary.id().clone())
                || entry
                    .equivalent_sources()
                    .iter()
                    .any(|source| !ids.insert(source.id().clone()))
        }) {
            return Err(CallableCatalogError::CandidateSetKeyMismatch);
        }
        entries.sort_by(|left, right| record_order(left.primary(), right.primary()));
        Ok(Self {
            entries: entries.into(),
        })
    }
    pub fn first(&self) -> &CatalogCallableEntry {
        &self.entries[0]
    }
    pub fn as_slice(&self) -> &[CatalogCallableEntry] {
        &self.entries
    }
    pub fn len(&self) -> NonZeroU32 {
        let len = u32::try_from(self.entries.len()).unwrap_or(u32::MAX);
        NonZeroU32::new(len).unwrap_or(NonZeroU32::MIN)
    }
}

fn record_order(left: &CallableRecord, right: &CallableRecord) -> Ordering {
    authority_order(left.authority())
        .cmp(&authority_order(right.authority()))
        .then_with(|| provider_order(left.provider(), right.provider()))
        .then_with(|| environment_kind(left.id()).cmp(&environment_kind(right.id())))
        .then_with(|| environment_overload(left.id()).cmp(&environment_overload(right.id())))
        .then_with(|| left.declaration_order.cmp(&right.declaration_order))
}

const fn authority_order(rank: CallableAuthorityRank) -> u8 {
    match rank {
        CallableAuthorityRank::Project => 0,
        CallableAuthorityRank::Standard => 1,
        CallableAuthorityRank::Adapter => 2,
    }
}

fn provider_order(left: &CallableProviderId, right: &CallableProviderId) -> Ordering {
    match (left, right) {
        (CallableProviderId::Project(left), CallableProviderId::Project(right)) => left.cmp(right),
        (CallableProviderId::Standard(left), CallableProviderId::Standard(right)) => {
            left.cmp(right)
        }
        (CallableProviderId::Adapter(left), CallableProviderId::Adapter(right)) => left.cmp(right),
        (CallableProviderId::Project(_), _)
        | (CallableProviderId::Standard(_), CallableProviderId::Adapter(_)) => Ordering::Less,
        (_, CallableProviderId::Project(_))
        | (CallableProviderId::Adapter(_), CallableProviderId::Standard(_)) => Ordering::Greater,
    }
}

fn environment_kind(id: &CallableCandidateId) -> Option<EnvironmentCallableKind> {
    match id {
        CallableCandidateId::Environment(id) => Some(id.kind()),
        _ => None,
    }
}
fn environment_overload(id: &CallableCandidateId) -> Option<super::CallableOverloadIndex> {
    match id {
        CallableCandidateId::Environment(id) => Some(id.overload()),
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredProjectModuleCallables {
    module: CanonicalModulePath,
    source: SourceDocumentIdentity,
    declarations: Arc<[CallableDeclarationId]>,
}
impl RegisteredProjectModuleCallables {
    pub(crate) fn new(
        module: CanonicalModulePath,
        source: SourceDocumentIdentity,
        declarations: Vec<CallableDeclarationId>,
    ) -> Self {
        Self {
            module,
            source,
            declarations: declarations.into(),
        }
    }
    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }
    pub const fn source(&self) -> &SourceDocumentIdentity {
        &self.source
    }
    pub fn declarations(&self) -> &[CallableDeclarationId] {
        &self.declarations
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCallableCatalog {
    modules: Arc<[RegisteredProjectModuleCallables]>,
    by_declaration: HashMap<CallableDeclarationId, Arc<CallableRecord>>,
    bindings: HashMap<ProjectCallablePath, ProjectNameBinding>,
}
impl ProjectCallableCatalog {
    pub(crate) fn new(
        modules: Vec<RegisteredProjectModuleCallables>,
        by_declaration: HashMap<CallableDeclarationId, Arc<CallableRecord>>,
        bindings: HashMap<ProjectCallablePath, ProjectNameBinding>,
    ) -> Self {
        Self {
            modules: modules.into(),
            by_declaration,
            bindings,
        }
    }
    pub fn modules(&self) -> &[RegisteredProjectModuleCallables] {
        &self.modules
    }
    pub fn record(&self, id: &CallableDeclarationId) -> Option<&Arc<CallableRecord>> {
        self.by_declaration.get(id)
    }
    pub fn binding(&self, key: &ProjectCallablePath) -> Option<&ProjectNameBinding> {
        self.bindings.get(key)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallableCatalog {
    free: HashMap<super::CallablePath, NonEmptyCallableSet>,
    methods: HashMap<super::ReceiverMethodKey, NonEmptyCallableSet>,
    by_id: HashMap<EnvironmentCallableId, Arc<CallableRecord>>,
}
impl EnvironmentCallableCatalog {
    pub(crate) fn new(
        free: HashMap<super::CallablePath, NonEmptyCallableSet>,
        methods: HashMap<super::ReceiverMethodKey, NonEmptyCallableSet>,
        by_id: HashMap<EnvironmentCallableId, Arc<CallableRecord>>,
    ) -> Self {
        Self {
            free,
            methods,
            by_id,
        }
    }
    pub fn free(&self, path: &super::CallablePath) -> Option<&NonEmptyCallableSet> {
        self.free.get(path)
    }
    pub fn method(&self, key: &super::ReceiverMethodKey) -> Option<&NonEmptyCallableSet> {
        self.methods.get(key)
    }
    pub fn record(&self, id: &EnvironmentCallableId) -> Option<&Arc<CallableRecord>> {
        self.by_id.get(id)
    }

    pub(crate) fn rust_exports<'a>(
        &'a self,
        package: &'a str,
        export: &'a super::CallableName,
    ) -> Vec<&'a Arc<CallableRecord>> {
        let mut records = self
            .by_id
            .values()
            .filter(move |record| {
                matches!(
                    record.id(),
                    super::CallableCandidateId::Environment(id)
                        if id.kind() == EnvironmentCallableKind::RustFunction
                ) && matches!(
                    record.key(),
                    super::CallableLookupKey::Free(path)
                        if path.segments() == std::slice::from_ref(export)
                ) && record
                    .rust()
                    .is_some_and(|rust| rust.package().name() == package)
            })
            .collect::<Vec<_>>();
        records.sort_by(|left, right| record_order(left, right));
        records
    }

    pub(crate) fn has_rust_package(&self, package: &crate::env::nominal::RustPackageId) -> bool {
        self.by_id.values().any(|record| {
            matches!(
                record.id(),
                super::CallableCandidateId::Environment(id)
                    if id.kind() == EnvironmentCallableKind::RustFunction
            ) && record
                .rust()
                .is_some_and(|rust| rust.package().name() == package.as_str())
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredCallableCatalog {
    nominal_world: AcceptedNominalWorldStamp,
    project: ProjectCallableCatalog,
    environment: EnvironmentCallableCatalog,
    nominal_resolutions: crate::nominal::NominalResolutionIndex,
    digest: super::RegisteredCallableCatalogDigest,
}
impl RegisteredCallableCatalog {
    pub(crate) fn new(
        nominal_world: AcceptedNominalWorldStamp,
        project: ProjectCallableCatalog,
        environment: EnvironmentCallableCatalog,
        nominal_resolutions: crate::nominal::NominalResolutionIndex,
    ) -> Self {
        let digest = registered_catalog_digest(&nominal_world, &project, &environment);
        Self {
            nominal_world,
            project,
            environment,
            nominal_resolutions,
            digest,
        }
    }
    pub const fn nominal_world(&self) -> &AcceptedNominalWorldStamp {
        &self.nominal_world
    }
    pub const fn digest(&self) -> super::RegisteredCallableCatalogDigest {
        self.digest
    }
    pub const fn project(&self) -> &ProjectCallableCatalog {
        &self.project
    }
    pub const fn environment(&self) -> &EnvironmentCallableCatalog {
        &self.environment
    }
    /// Accepted source-backed nominal facts used to publish project signatures.
    pub const fn nominal_resolutions(&self) -> &crate::nominal::NominalResolutionIndex {
        &self.nominal_resolutions
    }
    pub fn project_binding(&self, key: &ProjectCallablePath) -> Option<&ProjectNameBinding> {
        self.project.binding(key)
    }
    pub fn project_record(&self, id: &CallableDeclarationId) -> Option<&Arc<CallableRecord>> {
        self.project.record(id)
    }
    pub fn free(&self, path: &super::CallablePath) -> Option<&NonEmptyCallableSet> {
        self.environment.free(path)
    }
    pub fn method(&self, key: &super::ReceiverMethodKey) -> Option<&NonEmptyCallableSet> {
        self.environment.method(key)
    }
    pub fn environment_record(&self, id: &EnvironmentCallableId) -> Option<&Arc<CallableRecord>> {
        self.environment.record(id)
    }
}

fn registered_catalog_digest(
    nominal_world: &AcceptedNominalWorldStamp,
    project: &ProjectCallableCatalog,
    environment: &EnvironmentCallableCatalog,
) -> super::RegisteredCallableCatalogDigest {
    let mut encoder = CanonicalEncoder::default();
    encoder.nominal_world(nominal_world);

    let mut project_records = project.by_declaration.values().collect::<Vec<_>>();
    project_records.sort_by_key(|record| record_identity_bytes(record));
    encoder.usize(project_records.len());
    for record in project_records {
        encode_record(&mut encoder, record);
    }

    let mut environment_records = environment.by_id.values().collect::<Vec<_>>();
    environment_records.sort_by_key(|record| record_identity_bytes(record));
    encoder.usize(environment_records.len());
    for record in environment_records {
        encode_record(&mut encoder, record);
    }

    let mut bindings = project.bindings.iter().collect::<Vec<_>>();
    bindings.sort_by_key(|(path, _)| project_path_bytes(path));
    encoder.usize(bindings.len());
    for (path, binding) in bindings {
        encode_project_path(&mut encoder, path);
        match binding {
            ProjectNameBinding::Callable(declaration) => {
                encoder.tag(0);
                encoder.project_declaration(declaration);
            }
            ProjectNameBinding::Environment(id) => {
                encoder.tag(1);
                encoder.environment_id(id);
            }
            ProjectNameBinding::NonCallable { path, ty } => {
                encoder.tag(2);
                encode_project_path(&mut encoder, path);
                encoder.bytes(ty.semantic_identity_digest().as_bytes());
            }
        }
    }

    let mut indexes = environment
        .free
        .iter()
        .map(|(path, set)| (CallableLookupKey::Free(path.clone()), set))
        .chain(
            environment
                .methods
                .iter()
                .map(|(method, set)| (CallableLookupKey::Method(method.clone()), set)),
        )
        .collect::<Vec<_>>();
    indexes.sort_by_key(|(key, _)| lookup_key_bytes(key));
    encoder.usize(indexes.len());
    for (key, set) in indexes {
        encoder.lookup_key(&key);
        encoder.usize(set.as_slice().len());
        for entry in set.as_slice() {
            encode_candidate(&mut encoder, entry.primary().id());
            let mut equivalents = entry.equivalent_sources().iter().collect::<Vec<_>>();
            equivalents.sort_by_key(|source| candidate_bytes(source.id()));
            encoder.usize(equivalents.len());
            for equivalent in equivalents {
                encode_candidate(&mut encoder, equivalent.id());
            }
        }
    }

    super::RegisteredCallableCatalogDigest::from_bytes(encoder.finish(CATALOG_DOMAIN))
}

fn encode_record(encoder: &mut CanonicalEncoder, record: &CallableRecord) {
    encode_candidate(encoder, record.id());
    encoder.lookup_key(record.key());
    encoder.authority(record.authority());
    encoder.provider(record.provider());
    encoder.bytes(record.schema().semantic_digest().as_bytes());
    encoder.option(record.publication_digest().as_ref(), |encoder, digest| {
        encoder.bytes(digest.as_bytes());
    });
    encoder.documentation(record.documentation());
    encoder.option(record.source(), CanonicalEncoder::source);
    encoder.option(record.rust(), CanonicalEncoder::rust_provenance);
    encoder.usize(record.declaration_order().get());
}

fn encode_candidate(encoder: &mut CanonicalEncoder, id: &CallableCandidateId) {
    match id {
        CallableCandidateId::Project(declaration) => {
            encoder.tag(0);
            encoder.project_declaration(declaration);
        }
        CallableCandidateId::Environment(environment) => {
            encoder.tag(1);
            encoder.environment_id(environment);
        }
        _ => unreachable!("registered catalogs contain only project and environment callables"),
    }
}

fn encode_project_path(encoder: &mut CanonicalEncoder, path: &ProjectCallablePath) {
    encoder.string(path.package().as_str());
    encoder.usize(path.module().segments().len());
    for segment in path.module().segments() {
        encoder.string(segment.as_str());
    }
    encoder.lookup_key(&CallableLookupKey::Free(path.path().clone()));
}

fn record_identity_bytes(record: &CallableRecord) -> Vec<u8> {
    candidate_bytes(record.id())
}

fn candidate_bytes(id: &CallableCandidateId) -> Vec<u8> {
    let mut encoder = CanonicalEncoder::default();
    encode_candidate(&mut encoder, id);
    encoder.into_bytes()
}

fn lookup_key_bytes(key: &CallableLookupKey) -> Vec<u8> {
    let mut encoder = CanonicalEncoder::default();
    encoder.lookup_key(key);
    encoder.into_bytes()
}

fn project_path_bytes(path: &ProjectCallablePath) -> Vec<u8> {
    let mut encoder = CanonicalEncoder::default();
    encode_project_path(&mut encoder, path);
    encoder.into_bytes()
}
