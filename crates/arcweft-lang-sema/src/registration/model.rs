use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_character::{
    id::CharacterId,
    manifest::{
        CharacterManifest, CharacterManifestFingerprint, registration::CharacterManifestTokenPath,
    },
    registration_catalog::SourceBackedCharacterCatalog,
};
use arcweft_lang_hir::{
    project::HirProject,
    symbol::{
        ExternalDeclarationId, ExternalDeclarationSeed, ExternalDeclarationSeedId,
        ProjectExternalDeclarations, ProjectExternalDeclarationsError, ProjectSymbolRevision,
        ProjectSymbolTable, ProjectSymbolTargetId, ProjectSymbolWorldId,
    },
};
use arcweft_lang_syntax::ast::symbol_path::SymbolPath;
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceDocumentIdentity, SourceRange, SourceSpan,
};
use thiserror::Error;

use crate::{
    env::TypeCheckEnv,
    types::{CharacterNominalType, TypeKind},
};

use super::{
    diagnostic::{
        CharacterRegistrationDiagnostic, CharacterRegistrationDiagnosticKind,
        CharacterRegistrationReport, RequiredCharacterToken,
    },
    limits::{CharacterRegistrationLimitKind, CharacterRegistrationLimits},
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnvironmentBindingId(String);

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EnvironmentBindingIdError {
    #[error("environment binding identity must not be empty")]
    Empty,
    #[error("environment binding identity contains a control character at byte {byte}")]
    Control { byte: usize },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RegisteredExternalOwner {
    Character(CharacterId),
    Environment(EnvironmentBindingId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RegisteredExternalOwnerKind {
    Character,
    Environment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalRegistrationFact {
    declaration: ExternalDeclarationSeed,
    target: RegisteredExternalOwner,
    owner_source: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExternalOwnerContribution {
    pub(crate) seed: ExternalDeclarationSeedId,
    pub(crate) target: RegisteredExternalOwner,
    pub(crate) owner_source: SourceSpan,
}

#[derive(Clone, Debug)]
pub struct ProjectRegistrationFacts {
    world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    documents: BTreeMap<SourceDocumentId, Arc<SourceDocument>>,
    external_declarations: ProjectExternalDeclarations,
    external_owners: Vec<ExternalOwnerContribution>,
    catalogs: Vec<SourceBackedCharacterCatalog>,
    manifest_owner_sources: BTreeMap<(usize, usize), SourceSpan>,
}

pub struct CharacterRegistrationRequest<'a> {
    pub(crate) base: Arc<TypeCheckEnv>,
    pub(crate) project: &'a HirProject,
    pub(crate) facts: &'a ProjectRegistrationFacts,
    pub(crate) previous: Option<&'a RegisteredTypeCheckEnv>,
}

#[derive(Clone, Debug)]
pub struct RegisteredSemanticWorld {
    pub(crate) symbols: Arc<ProjectSymbolTable>,
    pub(crate) environment: Arc<RegisteredTypeCheckEnv>,
}

pub struct CharacterRegistrar;

pub(super) struct RegistrationDocumentView<'a> {
    identity: &'a SourceDocumentIdentity,
    text: &'a str,
    primary: SourceSpan,
}

impl<'a> RegistrationDocumentView<'a> {
    pub(super) fn new(document: &'a SourceDocument) -> Self {
        Self {
            identity: document.identity(),
            text: document.text(),
            primary: full_span(document),
        }
    }

    #[cfg(test)]
    pub(super) fn with_injected_text(
        identity: &'a SourceDocumentIdentity,
        text: &'a str,
        primary: SourceSpan,
    ) -> Self {
        Self {
            identity,
            text,
            primary,
        }
    }
}

pub(super) fn registration_document_diagnostics(
    documents: &[RegistrationDocumentView<'_>],
) -> Vec<CharacterRegistrationDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut by_id = BTreeMap::<&SourceDocumentId, &RegistrationDocumentView<'_>>::new();
    for document in documents {
        if let Some(first) = by_id.get(document.identity.id()) {
            if first.identity != document.identity {
                diagnostics.push(CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::WrongRevision {
                        expected: first.identity.revision(),
                        actual: document.identity.revision(),
                    },
                    document.primary.clone(),
                    [first.primary.clone()],
                ));
            } else if first.text != document.text {
                diagnostics.push(CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::SourceDigestCollision {
                        id: document.identity.id().clone(),
                        revision: document.identity.revision(),
                    },
                    document.primary.clone(),
                    [first.primary.clone()],
                ));
            }
            continue;
        }
        by_id.insert(document.identity.id(), document);
    }
    diagnostics
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterInventoryDigest(pub(crate) [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterInventoryRevision(pub(crate) u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterInventoryDescriptorV1 {
    pub(crate) characters: Vec<(CharacterId, CharacterManifestFingerprint)>,
    pub(crate) externals: Vec<(ExternalDeclarationId, SymbolPath, CharacterId)>,
}

#[derive(Clone, Debug)]
pub struct RegisteredTypeCheckEnv {
    pub(crate) base: Arc<TypeCheckEnv>,
    pub(crate) characters: BTreeMap<CharacterId, CharacterManifest>,
    pub(crate) character_variants: BTreeMap<CharacterNominalType, BTreeSet<String>>,
    pub(crate) external_owners: ExternalOwnerRegistry,
    pub(crate) world: ProjectSymbolWorldId,
    pub(crate) symbol_revision: ProjectSymbolRevision,
    pub(crate) character_descriptor: CharacterInventoryDescriptorV1,
    pub(crate) character_digest: CharacterInventoryDigest,
    pub(crate) character_revision: CharacterInventoryRevision,
}

#[derive(Clone, Debug)]
pub(crate) struct ExternalOwnerRegistry {
    pub(crate) world: ProjectSymbolWorldId,
    pub(crate) revision: ProjectSymbolRevision,
    pub(crate) owners: BTreeMap<ExternalDeclarationId, RegisteredExternalOwner>,
}

#[allow(
    clippy::large_enum_variant,
    reason = "stale-world errors retain both complete typed world identities and revisions"
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExternalOwnerLookupError {
    Stale {
        expected_world: ProjectSymbolWorldId,
        actual_world: ProjectSymbolWorldId,
        expected_revision: ProjectSymbolRevision,
        actual_revision: ProjectSymbolRevision,
    },
    Unknown {
        declaration: ExternalDeclarationId,
    },
    WrongKind {
        declaration: ExternalDeclarationId,
        expected: RegisteredExternalOwnerKind,
        actual: RegisteredExternalOwnerKind,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RegisteredCharacterResolutionError {
    #[error(transparent)]
    Symbol(#[from] arcweft_lang_hir::symbol::ProjectSymbolResolutionError),
    #[error("resolved project symbol is not external")]
    NotExternal { actual: ProjectSymbolTargetId },
    #[error("resolved external declaration is not a current character owner")]
    Owner(ExternalOwnerLookupError),
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterInventoryIntegrityError {
    #[error("project symbol table is stale for the registered environment")]
    Stale {
        expected_world: ProjectSymbolWorldId,
        actual_world: ProjectSymbolWorldId,
        expected_revision: ProjectSymbolRevision,
        actual_revision: ProjectSymbolRevision,
    },
    #[error("registered external declaration is absent from the project symbol table")]
    MissingExternalSymbol { declaration: ExternalDeclarationId },
    #[error("registered character external has a non-character owner")]
    WrongOwnerKind {
        declaration: ExternalDeclarationId,
        actual: RegisteredExternalOwnerKind,
    },
    #[error("registered character external owner does not match the descriptor record")]
    OwnerMismatch {
        declaration: ExternalDeclarationId,
        expected: CharacterId,
        actual: CharacterId,
    },
    #[error("character inventory descriptor was tampered")]
    DescriptorTamper {
        expected: CharacterInventoryDigest,
        actual: CharacterInventoryDigest,
    },
}

impl EnvironmentBindingId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, EnvironmentBindingIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(EnvironmentBindingIdError::Empty);
        }
        if let Some((byte, _)) = value
            .char_indices()
            .find(|(_, character)| character.is_control())
        {
            return Err(EnvironmentBindingIdError::Control { byte });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl RegisteredExternalOwner {
    pub const fn kind(&self) -> RegisteredExternalOwnerKind {
        match self {
            Self::Character(_) => RegisteredExternalOwnerKind::Character,
            Self::Environment(_) => RegisteredExternalOwnerKind::Environment,
        }
    }
}

impl ExternalRegistrationFact {
    pub fn new(
        declaration: ExternalDeclarationSeed,
        target: RegisteredExternalOwner,
        owner_source: SourceSpan,
    ) -> Self {
        Self {
            declaration,
            target,
            owner_source,
        }
    }

    pub const fn declaration(&self) -> &ExternalDeclarationSeed {
        &self.declaration
    }

    pub const fn target(&self) -> &RegisteredExternalOwner {
        &self.target
    }

    pub const fn owner_source(&self) -> &SourceSpan {
        &self.owner_source
    }
}

impl ProjectRegistrationFacts {
    #[allow(
        clippy::too_many_lines,
        reason = "fact construction validates and freezes one atomic revision-bound registration input"
    )]
    pub fn try_new(
        world: ProjectSymbolWorldId,
        documents: Vec<Arc<SourceDocument>>,
        mut externals: Vec<ExternalRegistrationFact>,
        catalogs: Vec<SourceBackedCharacterCatalog>,
    ) -> Result<Self, CharacterRegistrationReport> {
        if documents.is_empty() {
            return Err(CharacterRegistrationReport::from_diagnostics(Vec::new()).with_omitted(1));
        }

        let document_views = documents
            .iter()
            .map(|document| RegistrationDocumentView::new(document))
            .collect::<Vec<_>>();
        let mut diagnostics = registration_document_diagnostics(&document_views);
        let mut by_id = BTreeMap::<SourceDocumentId, Arc<SourceDocument>>::new();
        for document in documents {
            by_id
                .entry(document.identity().id().clone())
                .or_insert(document);
        }

        let Some(first_document) = by_id.values().next().cloned() else {
            return Err(CharacterRegistrationReport::from_diagnostics(Vec::new()).with_omitted(1));
        };
        let last_document = by_id
            .values()
            .next_back()
            .map_or_else(|| Arc::clone(&first_document), Arc::clone);

        let observed_documents = u64::try_from(by_id.len()).unwrap_or(u64::MAX);
        if observed_documents > CharacterRegistrationLimits::PRODUCTION.documents() {
            let primary = full_span(&last_document);
            diagnostics.push(CharacterRegistrationDiagnostic::new(
                CharacterRegistrationDiagnosticKind::Limit {
                    kind: CharacterRegistrationLimitKind::Documents,
                    observed: observed_documents,
                    maximum: CharacterRegistrationLimits::PRODUCTION.documents(),
                },
                primary,
                [],
            ));
        }

        if !by_id.contains_key(world.root_document()) {
            let actual = first_document.identity().id().clone();
            let primary = full_span(&first_document);
            diagnostics.push(CharacterRegistrationDiagnostic::new(
                CharacterRegistrationDiagnosticKind::WrongDocument {
                    expected: world.root_document().clone(),
                    actual,
                },
                primary,
                [],
            ));
        }

        for fact in &externals {
            validate_span(fact.declaration().declaration(), &by_id, &mut diagnostics);
            validate_span(fact.owner_source(), &by_id, &mut diagnostics);
            for binding in fact.declaration().direct_bindings() {
                validate_span(binding.source(), &by_id, &mut diagnostics);
            }
        }
        let mut manifest_owner_sources = BTreeMap::new();
        for (catalog_index, catalog) in catalogs.iter().enumerate() {
            for (manifest_index, manifest) in catalog.manifests().enumerate() {
                let path = CharacterManifestTokenPath::Root(
                    arcweft_character::manifest::registration::CharacterManifestRootField::Character,
                );
                if let Some(token) = manifest.source_map().token(&path) {
                    validate_span(token.value(), &by_id, &mut diagnostics);
                    manifest_owner_sources
                        .insert((catalog_index, manifest_index), token.value().clone());
                } else {
                    diagnostics.push(CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::MissingProvenance {
                            token: RequiredCharacterToken::Manifest(path),
                        },
                        by_id
                            .get(manifest.source_map().document().id())
                            .map_or_else(
                                || full_span(&first_document),
                                |document| full_span(document),
                            ),
                        [],
                    ));
                }
            }
        }

        let source_bytes = catalogs.iter().try_fold(0_u64, |total, catalog| {
            let total = total.checked_add(catalog.source().source_len())?;
            catalog.manifests().try_fold(total, |total, manifest| {
                total.checked_add(manifest.source_map().document().source_len())
            })
        });
        match source_bytes {
            None => diagnostics.push(CharacterRegistrationDiagnostic::new(
                CharacterRegistrationDiagnosticKind::ArithmeticOverflow {
                    counter: CharacterRegistrationLimitKind::SourceBytes,
                },
                full_span(&first_document),
                [],
            )),
            Some(source_bytes)
                if source_bytes > CharacterRegistrationLimits::PRODUCTION.source_bytes() =>
            {
                let primary = catalogs
                    .iter()
                    .flat_map(SourceBackedCharacterCatalog::manifests)
                    .find_map(|manifest| {
                        manifest.source_map().token(&CharacterManifestTokenPath::Root(
                            arcweft_character::manifest::registration::CharacterManifestRootField::Character,
                        ))
                    })
                    .map_or_else(
                        || full_span(&first_document),
                        |token| token.value().clone(),
                    );
                diagnostics.push(CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::ManifestBytesLimit {
                        observed: source_bytes,
                        maximum: CharacterRegistrationLimits::PRODUCTION.source_bytes(),
                    },
                    primary,
                    [],
                ));
            }
            Some(_) => {}
        }

        if !diagnostics.is_empty() {
            return Err(CharacterRegistrationReport::from_diagnostics(diagnostics));
        }

        let symbol_revision = ProjectSymbolRevision::try_for_documents(
            by_id.values().map(|document| document.identity()),
        )
        .map_err(|_| {
            CharacterRegistrationReport::from_diagnostics(vec![
                CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::ArithmeticOverflow {
                        counter: CharacterRegistrationLimitKind::Documents,
                    },
                    full_span(&first_document),
                    [],
                ),
            ])
        })?;

        externals.sort_by(|left, right| {
            left.declaration()
                .canonical_path()
                .cmp(right.declaration().canonical_path())
                .then_with(|| {
                    left.declaration()
                        .declaration()
                        .source()
                        .id()
                        .cmp(right.declaration().declaration().source().id())
                })
                .then_with(|| {
                    left.declaration()
                        .declaration()
                        .range()
                        .cmp(&right.declaration().declaration().range())
                })
                .then_with(|| left.target().cmp(right.target()))
                .then_with(|| {
                    left.owner_source()
                        .range()
                        .cmp(&right.owner_source().range())
                })
                .then_with(|| left.declaration().cmp(right.declaration()))
                .then_with(|| left.owner_source().cmp(right.owner_source()))
        });
        let mut seeds = externals
            .iter()
            .map(|fact| fact.declaration().clone())
            .collect::<Vec<_>>();
        seeds.sort();
        seeds.dedup();
        let external_declarations =
            ProjectExternalDeclarations::try_new(world.clone(), symbol_revision, seeds).map_err(
                |ProjectExternalDeclarationsError::SeedCountOverflow { count }| {
                    CharacterRegistrationReport::from_diagnostics(vec![
                        CharacterRegistrationDiagnostic::new(
                            CharacterRegistrationDiagnosticKind::ArithmeticOverflow {
                                counter: CharacterRegistrationLimitKind::Owners,
                            },
                            externals.first().map_or_else(
                                || full_span(&first_document),
                                |fact| fact.owner_source().clone(),
                            ),
                            [],
                        ),
                    ])
                    .with_omitted(u64::try_from(count).unwrap_or(u64::MAX).saturating_sub(1))
                },
            )?;
        let mut external_owners = Vec::with_capacity(externals.len());
        for fact in externals {
            let Some(seed) = external_declarations.seed_id(fact.declaration()) else {
                return Err(CharacterRegistrationReport::from_diagnostics(vec![
                    CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::MissingProvenance {
                            token: RequiredCharacterToken::ExternalDeclaration,
                        },
                        fact.owner_source,
                        [],
                    ),
                ]));
            };
            external_owners.push(ExternalOwnerContribution {
                seed,
                target: fact.target,
                owner_source: fact.owner_source,
            });
        }

        Ok(Self {
            world,
            symbol_revision,
            documents: by_id,
            external_declarations,
            external_owners,
            catalogs,
            manifest_owner_sources,
        })
    }

    pub const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    pub const fn symbol_revision(&self) -> &ProjectSymbolRevision {
        &self.symbol_revision
    }

    pub fn documents(&self) -> impl ExactSizeIterator<Item = &Arc<SourceDocument>> {
        self.documents.values()
    }

    pub const fn external_declarations(&self) -> &ProjectExternalDeclarations {
        &self.external_declarations
    }

    pub fn catalogs(&self) -> impl ExactSizeIterator<Item = &SourceBackedCharacterCatalog> {
        self.catalogs.iter()
    }

    pub(crate) fn external_owner_contributions(
        &self,
    ) -> impl ExactSizeIterator<Item = &ExternalOwnerContribution> {
        self.external_owners.iter()
    }

    pub(crate) fn document(&self, id: &SourceDocumentId) -> Option<&SourceDocument> {
        self.documents.get(id).map(AsRef::as_ref)
    }

    pub(crate) fn manifest_owner_source(
        &self,
        catalog: usize,
        manifest: usize,
    ) -> Option<&SourceSpan> {
        self.manifest_owner_sources.get(&(catalog, manifest))
    }

    #[cfg(test)]
    pub(super) fn remove_first_manifest_owner_source_for_test(&mut self) {
        let Some(key) = self.manifest_owner_sources.keys().next().copied() else {
            return;
        };
        self.manifest_owner_sources.remove(&key);
    }

    #[cfg(test)]
    pub(super) fn replace_first_manifest_owner_source_for_test(&mut self, source: SourceSpan) {
        let Some(key) = self.manifest_owner_sources.keys().next().copied() else {
            return;
        };
        self.manifest_owner_sources.insert(key, source);
    }

    #[cfg(test)]
    pub(super) fn replace_symbol_revision_for_test(&mut self, revision: ProjectSymbolRevision) {
        self.symbol_revision = revision;
    }

    #[cfg(test)]
    pub(super) fn clear_external_owner_contributions_for_test(&mut self) {
        self.external_owners.clear();
    }
}

impl<'a> CharacterRegistrationRequest<'a> {
    pub fn new(
        base: Arc<TypeCheckEnv>,
        project: &'a HirProject,
        facts: &'a ProjectRegistrationFacts,
        previous: Option<&'a RegisteredTypeCheckEnv>,
    ) -> Self {
        Self {
            base,
            project,
            facts,
            previous,
        }
    }
}

impl RegisteredSemanticWorld {
    pub fn symbols(&self) -> &ProjectSymbolTable {
        &self.symbols
    }

    pub fn environment(&self) -> &RegisteredTypeCheckEnv {
        &self.environment
    }

    pub fn into_parts(self) -> (Arc<ProjectSymbolTable>, Arc<RegisteredTypeCheckEnv>) {
        (self.symbols, self.environment)
    }
}

impl CharacterInventoryDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl CharacterInventoryRevision {
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl RegisteredTypeCheckEnv {
    pub const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    pub const fn symbol_revision(&self) -> &ProjectSymbolRevision {
        &self.symbol_revision
    }

    pub const fn character_digest(&self) -> CharacterInventoryDigest {
        self.character_digest
    }

    pub const fn character_revision(&self) -> CharacterInventoryRevision {
        self.character_revision
    }

    pub fn character_enum_variants(
        &self,
        nominal: &CharacterNominalType,
    ) -> Option<&BTreeSet<String>> {
        self.character_variants.get(nominal)
    }

    pub fn environment_binding(&self, id: &EnvironmentBindingId) -> Option<&TypeKind> {
        self.base.environment_binding(id)
    }

    pub(crate) fn base(&self) -> &TypeCheckEnv {
        &self.base
    }

    pub fn character_manifest(&self, id: &CharacterId) -> Option<&CharacterManifest> {
        self.characters.get(id)
    }

    pub fn characters(&self) -> impl ExactSizeIterator<Item = (&CharacterId, &CharacterManifest)> {
        self.characters.iter()
    }
}

fn full_span(document: &SourceDocument) -> SourceSpan {
    document
        .span(SourceRange::new(0, document.text().len()))
        .expect("a complete source document range is valid")
}

fn validate_span(
    span: &SourceSpan,
    documents: &BTreeMap<SourceDocumentId, Arc<SourceDocument>>,
    diagnostics: &mut Vec<CharacterRegistrationDiagnostic>,
) {
    let Some(document) = documents.get(span.source().id()) else {
        let expected = documents
            .keys()
            .next()
            .expect("registration facts contain at least one document")
            .clone();
        diagnostics.push(CharacterRegistrationDiagnostic::new(
            CharacterRegistrationDiagnosticKind::WrongDocument {
                expected,
                actual: span.source().id().clone(),
            },
            span.clone(),
            [],
        ));
        return;
    };
    if document.identity().revision() != span.source().revision() {
        diagnostics.push(CharacterRegistrationDiagnostic::new(
            CharacterRegistrationDiagnosticKind::WrongRevision {
                expected: document.identity().revision(),
                actual: span.source().revision(),
            },
            span.clone(),
            [full_span(document)],
        ));
    }
}
