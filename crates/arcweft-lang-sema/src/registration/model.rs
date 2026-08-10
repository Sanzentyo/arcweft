use std::{collections::BTreeMap, sync::Arc};

use arcweft_character::{
    id::CharacterId,
    manifest::{
        CharacterManifest, CharacterManifestFingerprint, registration::CharacterManifestTokenPath,
    },
    registration_catalog::SourceBackedCharacterCatalog,
};
use arcweft_lang_hir::{
    project::HirProjectView,
    proof_return::{HirProofReturnHeaderProjectView, HirProofReturnProjectGeneration},
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
    callable::RegisteredCallableCatalog,
    character_dialogue::CharacterDialogueCustomFieldRegistry,
    env::{
        AcceptedRustTypeMetadataCatalog, TypeCheckEnv,
        identity::EnvironmentBindingId,
        nominal::{
            AcceptedNominalCatalog, AcceptedNominalCatalogDigest, AcceptedNominalCatalogError,
            AcceptedNominalId, AcceptedNominalRecord,
        },
    },
    registration::EnvironmentPublicationItemId,
    types::{CharacterNominalType, TypeKind},
};

use super::environment_input::{
    BoundEnvironmentRegistrationInput, SourceBackedEnvironmentRegistrationInput,
};
use super::{
    diagnostic::{
        CharacterRegistrationDiagnostic, CharacterRegistrationDiagnosticKind,
        CharacterRegistrationReport, RequiredCharacterToken,
    },
    limits::{CharacterRegistrationLimitKind, CharacterRegistrationLimits},
    source_index::CharacterDefinitionIndex,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RegisteredExternalOwner {
    Character(CharacterId),
    Environment(RegisteredEnvironmentExternalOwner),
}

/// Semantic owner and value binding selected for one environment-backed external.
///
/// The nominal owner participates in accepted type identity. The value binding
/// selects the concrete type exposed by the external symbol. They are distinct
/// for adapter exports: every nominal in an adapter shares `adapter:<id>` as its
/// semantic owner while each exported symbol has its own binding.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RegisteredEnvironmentExternalOwner {
    nominal_owner: EnvironmentBindingId,
    value_binding: EnvironmentBindingId,
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
    environment_inputs: Box<[BoundEnvironmentRegistrationInput]>,
    manifest_owner_sources: BTreeMap<(usize, usize), SourceSpan>,
}

pub struct CharacterRegistrationRequest<'a> {
    pub(crate) base: Arc<TypeCheckEnv>,
    pub(crate) project: HirProjectView<'a>,
    pub(crate) facts: &'a ProjectRegistrationFacts,
    pub(crate) previous: Option<&'a RegisteredTypeCheckEnv>,
}

/// Exact pre-publication registration input for one paused Proof-return HIR
/// generation.
pub struct ProofReturnRegistrationRequest<'a> {
    pub(crate) base: Arc<TypeCheckEnv>,
    pub(crate) generation: Arc<HirProofReturnProjectGeneration>,
    pub(crate) project: HirProofReturnHeaderProjectView<'a, 'a>,
    pub(crate) facts: &'a ProjectRegistrationFacts,
    pub(crate) previous: Option<&'a RegisteredTypeCheckEnv>,
}

/// Registration state frozen against the paused HIR header view. The same
/// symbol table and nominal world are consumed by Proof classification and by
/// final registration after atomic HIR publication.
pub struct ProofReturnRegistrationPrelude {
    pub(crate) generation: Arc<HirProofReturnProjectGeneration>,
    pub(crate) symbols: Arc<ProjectSymbolTable>,
    pub(crate) nominal_world: Arc<AcceptedNominalWorld>,
    pub(crate) rust_metadata: Arc<AcceptedRustTypeMetadataCatalog>,
    pub(crate) characters: BTreeMap<CharacterId, CharacterManifest>,
    pub(crate) character_variants: BTreeMap<CharacterNominalType, Box<[String]>>,
    pub(crate) character_descriptor: CharacterInventoryDescriptorV1,
    pub(crate) character_digest: CharacterInventoryDigest,
    pub(crate) character_revision: CharacterInventoryRevision,
}

#[derive(Clone, Debug)]
pub struct RegisteredSemanticWorld {
    pub(crate) symbols: Arc<ProjectSymbolTable>,
    pub(crate) environment: Arc<RegisteredTypeCheckEnv>,
    pub(crate) character_definitions: Arc<CharacterDefinitionIndex>,
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

/// Stable identity of one completely accepted semantic environment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RegisteredEnvironmentDigest(pub(crate) [u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterInventoryDescriptorV1 {
    pub(crate) characters: Vec<(CharacterId, CharacterManifestFingerprint)>,
    pub(crate) externals: Vec<(ExternalDeclarationId, SymbolPath, CharacterId)>,
}

#[derive(Clone, Debug)]
pub struct RegisteredTypeCheckEnv {
    pub(crate) nominal_world: Arc<AcceptedNominalWorld>,
    pub(crate) character_dialogue_fields: Arc<CharacterDialogueCustomFieldRegistry>,
    pub(crate) rust_metadata: Arc<AcceptedRustTypeMetadataCatalog>,
    pub(crate) callables: Arc<RegisteredCallableCatalog>,
    pub(crate) characters: BTreeMap<CharacterId, CharacterManifest>,
    pub(crate) character_variants: BTreeMap<CharacterNominalType, Box<[String]>>,
    pub(crate) character_descriptor: CharacterInventoryDescriptorV1,
    pub(crate) character_digest: CharacterInventoryDigest,
    pub(crate) character_revision: CharacterInventoryRevision,
    pub(crate) environment_digest: RegisteredEnvironmentDigest,
}

/// Accepted nominal-resolution world available before callable publication.
///
/// This carrier owns the exact environment facts and external-owner mapping
/// needed by authored type resolution without depending on the callable
/// catalog whose signatures are being built.
#[derive(Clone, Debug)]
pub struct AcceptedNominalWorld {
    base: Arc<TypeCheckEnv>,
    external_owners: ExternalOwnerRegistry,
    visibility: Arc<AcceptedNominalVisibilityIndex>,
}

/// Stable identity of the exact nominal world used for semantic projection.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AcceptedNominalWorldStamp {
    world: ProjectSymbolWorldId,
    revision: ProjectSymbolRevision,
    catalog_digest: AcceptedNominalCatalogDigest,
}

/// Source evidence for one visible or intentionally inaccessible nominal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedNominalSource {
    declaration: SourceSpan,
    item: EnvironmentPublicationItemId,
}

/// Visibility of exact source-backed nominal declarations in one accepted world.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AcceptedNominalVisibilityIndex {
    visible: BTreeMap<AcceptedNominalId, AcceptedNominalSource>,
    inaccessible: BTreeMap<AcceptedNominalId, AcceptedNominalSource>,
}

/// Failure to look up one exact nominal identity in an accepted world.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AcceptedNominalWorldLookupError {
    #[error("accepted nominal `{requested:?}` is not present in this world")]
    Unknown { requested: Box<AcceptedNominalId> },
    #[error("accepted nominal `{requested:?}` is private in this world")]
    Inaccessible { requested: Box<AcceptedNominalId> },
    #[error("accepted nominal path belongs to `{visible:?}`, not `{requested:?}`")]
    OwnerMismatch {
        requested: Box<AcceptedNominalId>,
        visible: Box<AcceptedNominalId>,
    },
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

impl RegisteredExternalOwner {
    pub fn environment(
        nominal_owner: EnvironmentBindingId,
        value_binding: EnvironmentBindingId,
    ) -> Self {
        Self::Environment(RegisteredEnvironmentExternalOwner::new(
            nominal_owner,
            value_binding,
        ))
    }

    pub const fn kind(&self) -> RegisteredExternalOwnerKind {
        match self {
            Self::Character(_) => RegisteredExternalOwnerKind::Character,
            Self::Environment(_) => RegisteredExternalOwnerKind::Environment,
        }
    }
}

impl RegisteredEnvironmentExternalOwner {
    pub fn new(nominal_owner: EnvironmentBindingId, value_binding: EnvironmentBindingId) -> Self {
        Self {
            nominal_owner,
            value_binding,
        }
    }

    pub const fn nominal_owner(&self) -> &EnvironmentBindingId {
        &self.nominal_owner
    }

    pub const fn value_binding(&self) -> &EnvironmentBindingId {
        &self.value_binding
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
        mut environment_inputs: Vec<SourceBackedEnvironmentRegistrationInput>,
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
        for input in &environment_inputs {
            match by_id.get(input.source().id()) {
                None => diagnostics.push(CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::WrongDocument {
                        expected: world.root_document().clone(),
                        actual: input.source().id().clone(),
                    },
                    full_span(&first_document),
                    [],
                )),
                Some(document) if document.identity() != input.source() => {
                    diagnostics.push(CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::WrongRevision {
                            expected: document.identity().revision(),
                            actual: input.source().revision(),
                        },
                        full_span(document),
                        [],
                    ));
                }
                Some(_) => {}
            }
            for span in input.source_spans() {
                validate_span(span, &by_id, &mut diagnostics);
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

        environment_inputs.sort_by(|left, right| {
            left.owner()
                .cmp(right.owner())
                .then_with(|| left.source().id().cmp(right.source().id()))
                .then_with(|| left.source().revision().cmp(&right.source().revision()))
        });
        let environment_inputs = environment_inputs
            .into_iter()
            .map(|input| input.bind_world(world.clone()))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Ok(Self {
            world,
            symbol_revision,
            documents: by_id,
            external_declarations,
            external_owners,
            catalogs,
            environment_inputs,
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

    pub(crate) fn environment_inputs(
        &self,
    ) -> impl ExactSizeIterator<Item = &BoundEnvironmentRegistrationInput> {
        self.environment_inputs.iter()
    }

    pub(crate) fn declares_environment_binding(&self, id: &EnvironmentBindingId) -> bool {
        self.environment_inputs.iter().any(|input| {
            input
                .input()
                .value_bindings()
                .iter()
                .any(|binding| binding.id() == id)
        })
    }

    pub(crate) fn external_owner_contributions(
        &self,
    ) -> impl ExactSizeIterator<Item = &ExternalOwnerContribution> {
        self.external_owners.iter()
    }

    pub(crate) fn document(&self, id: &SourceDocumentId) -> Option<&SourceDocument> {
        self.documents.get(id).map(AsRef::as_ref)
    }

    pub(crate) fn document_arc(&self, id: &SourceDocumentId) -> Option<&Arc<SourceDocument>> {
        self.documents.get(id)
    }

    pub(crate) fn manifest_owner_source(
        &self,
        catalog: usize,
        manifest: usize,
    ) -> Option<&SourceSpan> {
        self.manifest_owner_sources.get(&(catalog, manifest))
    }
}

impl<'a> CharacterRegistrationRequest<'a> {
    pub fn new(
        base: Arc<TypeCheckEnv>,
        project: HirProjectView<'a>,
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

impl<'a> ProofReturnRegistrationRequest<'a> {
    pub fn new(
        base: Arc<TypeCheckEnv>,
        generation: Arc<HirProofReturnProjectGeneration>,
        project: HirProofReturnHeaderProjectView<'a, 'a>,
        facts: &'a ProjectRegistrationFacts,
        previous: Option<&'a RegisteredTypeCheckEnv>,
    ) -> Self {
        Self {
            base,
            generation,
            project,
            facts,
            previous,
        }
    }
}

impl ProofReturnRegistrationPrelude {
    pub const fn generation(&self) -> &Arc<HirProofReturnProjectGeneration> {
        &self.generation
    }

    pub fn symbols(&self) -> &ProjectSymbolTable {
        &self.symbols
    }

    /// Returns the sole project-symbol allocation frozen by this registration transaction.
    pub const fn symbol_lease(&self) -> &Arc<ProjectSymbolTable> {
        &self.symbols
    }

    pub fn nominal_world(&self) -> &AcceptedNominalWorld {
        &self.nominal_world
    }
}

impl RegisteredSemanticWorld {
    pub fn symbols(&self) -> &ProjectSymbolTable {
        &self.symbols
    }

    pub fn environment(&self) -> &RegisteredTypeCheckEnv {
        &self.environment
    }

    pub fn character_definition_index(&self) -> &CharacterDefinitionIndex {
        &self.character_definitions
    }

    pub fn into_parts(
        self,
    ) -> (
        Arc<ProjectSymbolTable>,
        Arc<RegisteredTypeCheckEnv>,
        Arc<CharacterDefinitionIndex>,
    ) {
        (self.symbols, self.environment, self.character_definitions)
    }

    #[cfg(test)]
    pub(crate) fn with_callable_catalog_for_test(
        mut self,
        callables: Arc<RegisteredCallableCatalog>,
    ) -> Self {
        assert_eq!(
            callables.nominal_world(),
            &self.environment.nominal_world.stamp(),
            "test callable replacement must preserve the accepted nominal world",
        );
        let mut environment = (*self.environment).clone();
        environment.environment_digest =
            super::environment_digest::derive_test_callable_replacement(
                environment.environment_digest,
                callables.digest().as_bytes(),
            );
        environment.callables = callables;
        self.environment = Arc::new(environment);
        self
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

impl RegisteredEnvironmentDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl RegisteredTypeCheckEnv {
    /// Immutable callable catalog accepted with this exact semantic world.
    pub fn callable_catalog(&self) -> &RegisteredCallableCatalog {
        &self.callables
    }

    /// Exact accepted callable-catalog allocation retained by this world.
    ///
    /// Checked semantic publication uses this crate-private lease to prove
    /// pointer identity. Public query callers continue to borrow the catalog
    /// through [`Self::callable_catalog`] and cannot substitute another equal
    /// allocation.
    pub(crate) const fn callable_catalog_arc(&self) -> &Arc<RegisteredCallableCatalog> {
        &self.callables
    }

    /// Immutable Rust ADT metadata accepted with this exact semantic world.
    pub fn rust_metadata(&self) -> &AcceptedRustTypeMetadataCatalog {
        &self.rust_metadata
    }

    /// Exact nominal world accepted before and retained with callable publication.
    pub fn nominal_world(&self) -> &AcceptedNominalWorld {
        &self.nominal_world
    }

    pub fn character_dialogue_fields(&self) -> &CharacterDialogueCustomFieldRegistry {
        &self.character_dialogue_fields
    }

    pub fn world(&self) -> &ProjectSymbolWorldId {
        self.nominal_world.world()
    }

    pub fn symbol_revision(&self) -> &ProjectSymbolRevision {
        self.nominal_world.symbol_revision()
    }

    pub const fn character_digest(&self) -> CharacterInventoryDigest {
        self.character_digest
    }

    pub const fn character_revision(&self) -> CharacterInventoryRevision {
        self.character_revision
    }

    /// Canonical identity of this complete, successfully registered world.
    pub const fn environment_digest(&self) -> RegisteredEnvironmentDigest {
        self.environment_digest
    }

    pub fn character_enum_variants(&self, nominal: &CharacterNominalType) -> Option<&[String]> {
        self.character_variants.get(nominal).map(AsRef::as_ref)
    }

    pub fn environment_binding(&self, id: &EnvironmentBindingId) -> Option<&TypeKind> {
        self.nominal_world.environment_binding(id)
    }

    /// Exact base type-check environment accepted with this registered world.
    pub fn typecheck_env(&self) -> &TypeCheckEnv {
        self.nominal_world.typecheck_env()
    }

    /// Immutable exact/open nominal catalog accepted with this semantic world.
    pub fn nominal_catalog(&self) -> &AcceptedNominalCatalog {
        self.nominal_world.nominal_catalog()
    }

    pub fn character_manifest(&self, id: &CharacterId) -> Option<&CharacterManifest> {
        self.characters.get(id)
    }

    pub fn characters(&self) -> impl ExactSizeIterator<Item = (&CharacterId, &CharacterManifest)> {
        self.characters.iter()
    }
}

impl AcceptedNominalWorld {
    pub(crate) fn new(
        base: Arc<TypeCheckEnv>,
        world: ProjectSymbolWorldId,
        symbol_revision: ProjectSymbolRevision,
        owners: BTreeMap<ExternalDeclarationId, RegisteredExternalOwner>,
        visibility: AcceptedNominalVisibilityIndex,
    ) -> Self {
        Self {
            base,
            external_owners: ExternalOwnerRegistry {
                world,
                revision: symbol_revision,
                owners,
            },
            visibility: Arc::new(visibility),
        }
    }

    pub(crate) fn try_with_environment_bindings(
        mut self,
        bindings: impl IntoIterator<Item = (EnvironmentBindingId, TypeKind)>,
        aliases: impl IntoIterator<Item = AcceptedNominalRecord>,
    ) -> Result<Self, AcceptedNominalCatalogError> {
        let mut base = bindings
            .into_iter()
            .fold((*self.base).clone(), |environment, (id, ty)| {
                environment.with_symbol(id.as_str(), ty)
            });
        for alias in aliases {
            base.try_insert_nominal_record(alias)?;
        }
        self.base = Arc::new(base);
        Ok(self)
    }

    /// Exact world/revision/catalog identity required by projected publications.
    pub fn stamp(&self) -> AcceptedNominalWorldStamp {
        AcceptedNominalWorldStamp {
            world: self.external_owners.world.clone(),
            revision: self.external_owners.revision,
            catalog_digest: self.base.nominal_catalog().digest(),
        }
    }

    pub const fn world(&self) -> &ProjectSymbolWorldId {
        &self.external_owners.world
    }

    pub const fn symbol_revision(&self) -> &ProjectSymbolRevision {
        &self.external_owners.revision
    }

    pub fn environment_binding(&self, id: &EnvironmentBindingId) -> Option<&TypeKind> {
        self.base.environment_binding(id)
    }

    pub fn typecheck_env(&self) -> &TypeCheckEnv {
        &self.base
    }

    pub fn nominal_catalog(&self) -> &AcceptedNominalCatalog {
        self.base.nominal_catalog()
    }

    pub fn visibility(&self) -> &AcceptedNominalVisibilityIndex {
        &self.visibility
    }

    pub(crate) fn accepted_record(
        &self,
        requested: &AcceptedNominalId,
    ) -> Result<&AcceptedNominalRecord, AcceptedNominalWorldLookupError> {
        if self.visibility.inaccessible.contains_key(requested) {
            return Err(AcceptedNominalWorldLookupError::Inaccessible {
                requested: Box::new(requested.clone()),
            });
        }
        let Some(record) = self
            .base
            .nominal_catalog()
            .exact(requested.canonical_path())
        else {
            return Err(AcceptedNominalWorldLookupError::Unknown {
                requested: Box::new(requested.clone()),
            });
        };
        if record.id() != requested {
            return Err(AcceptedNominalWorldLookupError::OwnerMismatch {
                requested: Box::new(requested.clone()),
                visible: Box::new(record.id().clone()),
            });
        }
        Ok(record)
    }

    pub(crate) fn external_owners(
        &self,
    ) -> &BTreeMap<ExternalDeclarationId, RegisteredExternalOwner> {
        &self.external_owners.owners
    }
}

impl AcceptedNominalWorldStamp {
    pub const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    pub const fn revision(&self) -> ProjectSymbolRevision {
        self.revision
    }

    pub const fn catalog_digest(&self) -> AcceptedNominalCatalogDigest {
        self.catalog_digest
    }
}

impl AcceptedNominalSource {
    pub const fn new(declaration: SourceSpan, item: EnvironmentPublicationItemId) -> Self {
        Self { declaration, item }
    }

    pub const fn declaration(&self) -> &SourceSpan {
        &self.declaration
    }

    pub const fn item(&self) -> &EnvironmentPublicationItemId {
        &self.item
    }
}

impl AcceptedNominalVisibilityIndex {
    pub(crate) fn visible_entries(
        &self,
    ) -> impl ExactSizeIterator<Item = (&AcceptedNominalId, &AcceptedNominalSource)> {
        self.visible.iter()
    }

    pub(crate) fn inaccessible_entries(
        &self,
    ) -> impl ExactSizeIterator<Item = (&AcceptedNominalId, &AcceptedNominalSource)> {
        self.inaccessible.iter()
    }

    pub fn visible(&self, id: &AcceptedNominalId) -> Option<&AcceptedNominalSource> {
        self.visible.get(id)
    }

    pub fn inaccessible(&self, id: &AcceptedNominalId) -> Option<&AcceptedNominalSource> {
        self.inaccessible.get(id)
    }

    pub(crate) fn from_parts(
        visible: BTreeMap<AcceptedNominalId, AcceptedNominalSource>,
        inaccessible: BTreeMap<AcceptedNominalId, AcceptedNominalSource>,
    ) -> Self {
        Self {
            visible,
            inaccessible,
        }
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
