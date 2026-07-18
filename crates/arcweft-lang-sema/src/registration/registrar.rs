use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_character::{
    id::CharacterId,
    manifest::{
        CharacterLook, CharacterManifest, CharacterManifestFingerprint, CharacterPart,
        CharacterPartSelection, CharacterVariant,
        registration::{CharacterManifestRootField, CharacterManifestTokenPath},
    },
};
use arcweft_lang_hir::symbol::{
    ExternalDeclarationId, ProjectSymbolLinkError, ProjectSymbolTable, ProjectSymbolTargetId,
    ResolvedProjectSymbol,
};
use arcweft_lang_syntax::ast::{
    module_path::{CanonicalModulePath, ModulePathRoot},
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
};
use arcweft_source::{SourceRange, SourceSpan};

use crate::{
    callable::{PRODUCTION_CALLABLE_LIMITS, RegisteredCallableCatalogBuilder},
    types::{CharacterNominalType, EntityKind, EntityType, TypeKind},
};

use super::{
    descriptor::{build_descriptor, descriptor_digest},
    diagnostic::{
        CharacterRegistrationDiagnostic, CharacterRegistrationDiagnosticKind,
        CharacterRegistrationReport, RequiredCharacterToken,
    },
    limits::{CharacterRegistrationLimitKind, CharacterRegistrationLimits},
    model::{
        CharacterInventoryRevision, CharacterRegistrar, CharacterRegistrationRequest,
        ExternalOwnerRegistry, RegisteredExternalOwner, RegisteredSemanticWorld,
        RegisteredTypeCheckEnv,
    },
    source_index::CharacterDefinitionIndex,
};

pub(super) struct ManifestRecord {
    manifest: CharacterManifest,
    fingerprint: CharacterManifestFingerprint,
    sources: Vec<SourceSpan>,
}

impl ManifestRecord {
    fn primary_source(&self) -> &SourceSpan {
        self.sources
            .first()
            .expect("every manifest record retains at least one source")
    }
}

impl CharacterRegistrar {
    #[allow(
        clippy::too_many_lines,
        reason = "registration is one fail-closed transaction from validated facts to an accepted semantic world"
    )]
    pub fn register(
        request: CharacterRegistrationRequest<'_>,
    ) -> Result<RegisteredSemanticWorld, CharacterRegistrationReport> {
        let Some(fallback) = request
            .facts
            .document(request.facts.world().root_document())
            .or_else(|| request.facts.documents().next().map(AsRef::as_ref))
            .map(full_span)
        else {
            return Err(CharacterRegistrationReport::from_diagnostics(Vec::new()).with_omitted(1));
        };
        if request.facts.symbol_revision() != request.facts.external_declarations().revision() {
            return Err(CharacterRegistrationReport::from_diagnostics(vec![
                CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::StaleSource {
                        expected: *request.facts.symbol_revision(),
                        actual: *request.facts.external_declarations().revision(),
                    },
                    fallback,
                    [],
                ),
            ]));
        }
        let mut diagnostics = validate_project_sources(&request, &fallback);
        if !diagnostics.is_empty() {
            return Err(CharacterRegistrationReport::from_diagnostics(diagnostics));
        }

        let link = match ProjectSymbolTable::link(
            request.project,
            request.facts.external_declarations(),
        ) {
            Ok(link) => link,
            Err(report) => {
                diagnostics.extend(report.diagnostics().iter().cloned().map(|error| {
                    let primary = link_error_source(&error).unwrap_or_else(|| fallback.clone());
                    CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::ProjectSymbol { error },
                        primary,
                        [],
                    )
                }));
                return Err(CharacterRegistrationReport::from_diagnostics(diagnostics)
                    .with_omitted(report.omitted_diagnostics()));
            }
        };
        let mut work = link.work_charged();

        let manifests = collect_manifests(&request, &mut work, &mut diagnostics, &fallback);
        let owners = build_external_owners(
            &request,
            &link,
            &manifests,
            &mut work,
            &mut diagnostics,
            &fallback,
        );
        for symbol in link.table().external_symbols() {
            if !owners.contains_key(&symbol.declaration()) {
                diagnostics.push(CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::ExternalUnknown {
                        declaration: symbol.declaration(),
                    },
                    symbol.declaration_span().clone(),
                    [],
                ));
            }
        }
        audit_character_spellings(link.table(), &owners, &manifests, &mut diagnostics);
        if !diagnostics.is_empty() {
            return Err(CharacterRegistrationReport::from_diagnostics(diagnostics));
        }

        let characters = manifests
            .into_iter()
            .map(|(owner, record)| (owner, record.manifest))
            .collect::<BTreeMap<_, _>>();
        let character_variants = character_variants(&characters);
        let Ok(descriptor) = build_descriptor(link.table(), &characters, &owners) else {
            return Err(CharacterRegistrationReport::from_diagnostics(vec![
                CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::DescriptorTamper {
                        expected: super::model::CharacterInventoryDigest([0; 32]),
                        actual: super::model::CharacterInventoryDigest([0; 32]),
                    },
                    fallback,
                    [],
                ),
            ]));
        };
        let digest = descriptor_digest(&descriptor);
        let revision = match next_revision(request.previous, request.facts.world(), digest) {
            Ok(revision) => revision,
            Err(previous) => {
                return Err(CharacterRegistrationReport::from_diagnostics(vec![
                    CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::RevisionOverflow { previous },
                        fallback,
                        [],
                    ),
                ]));
            }
        };
        if work > CharacterRegistrationLimits::PRODUCTION.work() {
            return Err(CharacterRegistrationReport::from_diagnostics(vec![
                CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::WorkOverflow {
                        attempted: work,
                        maximum: CharacterRegistrationLimits::PRODUCTION.work(),
                    },
                    fallback,
                    [],
                ),
            ]));
        }

        let mut callable_builder =
            RegisteredCallableCatalogBuilder::new(PRODUCTION_CALLABLE_LIMITS);
        if let Err(error) = callable_builder.add_project(request.project, link.table()) {
            return Err(CharacterRegistrationReport::from_diagnostics(vec![
                CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::CallableCatalog { code: error.code() },
                    fallback,
                    [],
                ),
            ]));
        }
        if let Err(error) =
            callable_builder.add_project_bindings(request.project, link.table(), |target| {
                match target {
                    ProjectSymbolTargetId::External(declaration) => match owners.get(declaration) {
                        Some(RegisteredExternalOwner::Character(_)) => {
                            Some(TypeKind::Ref(EntityType::new(EntityKind::Character, None)))
                        }
                        Some(RegisteredExternalOwner::Environment(id)) => {
                            request.base.environment_binding(id).cloned()
                        }
                        None => None,
                    },
                    ProjectSymbolTargetId::Module(_) => Some(TypeKind::Named("Module".to_owned())),
                    ProjectSymbolTargetId::Callable(_) => None,
                }
            })
        {
            return Err(CharacterRegistrationReport::from_diagnostics(vec![
                CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::CallableCatalog { code: error.code() },
                    fallback,
                    [],
                ),
            ]));
        }
        let standard_publication = match request
            .base
            .standard_callable_publication(&PRODUCTION_CALLABLE_LIMITS)
        {
            Ok(publication) => publication,
            Err(error) => {
                let error = crate::callable::CallableCatalogBuildError::from(error);
                return Err(CharacterRegistrationReport::from_diagnostics(vec![
                    CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::CallableCatalog { code: error.code() },
                        fallback,
                        [],
                    ),
                ]));
            }
        };
        if let Err(error) = callable_builder.add_environment(standard_publication) {
            return Err(CharacterRegistrationReport::from_diagnostics(vec![
                CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::CallableCatalog { code: error.code() },
                    fallback,
                    [],
                ),
            ]));
        }
        for publication in request.callable_publications.iter().cloned() {
            if let Err(error) = callable_builder.add_environment(publication) {
                return Err(CharacterRegistrationReport::from_diagnostics(vec![
                    CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::CallableCatalog { code: error.code() },
                        fallback,
                        [],
                    ),
                ]));
            }
        }
        let callables = match callable_builder.finish() {
            Ok(callables) => Arc::new(callables),
            Err(error) => {
                return Err(CharacterRegistrationReport::from_diagnostics(vec![
                    CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::CallableCatalog { code: error.code() },
                        fallback,
                        [],
                    ),
                ]));
            }
        };

        let symbols = Arc::new(link.into_table());
        let environment = Arc::new(RegisteredTypeCheckEnv {
            base: request.base,
            callables,
            characters,
            character_variants,
            external_owners: ExternalOwnerRegistry {
                world: request.facts.world().clone(),
                revision: *request.facts.symbol_revision(),
                owners,
            },
            world: request.facts.world().clone(),
            symbol_revision: *request.facts.symbol_revision(),
            character_descriptor: descriptor,
            character_digest: digest,
            character_revision: revision,
        });
        let character_definitions =
            match CharacterDefinitionIndex::try_build(request.facts, &symbols, &environment) {
                Ok(index) => Arc::new(index),
                Err(report) => {
                    let diagnostics = report
                        .errors()
                        .iter()
                        .map(|error| {
                            CharacterRegistrationDiagnostic::new(
                                CharacterRegistrationDiagnosticKind::DefinitionIndex {
                                    error: error.clone(),
                                },
                                error
                                    .primary_span()
                                    .cloned()
                                    .unwrap_or_else(|| fallback.clone()),
                                [],
                            )
                        })
                        .collect();
                    return Err(CharacterRegistrationReport::from_diagnostics(diagnostics)
                        .with_omitted(report.omitted_errors()));
                }
            };
        Ok(RegisteredSemanticWorld {
            symbols,
            environment,
            character_definitions,
        })
    }
}

fn validate_project_sources(
    request: &CharacterRegistrationRequest<'_>,
    fallback: &SourceSpan,
) -> Vec<CharacterRegistrationDiagnostic> {
    let mut diagnostics = Vec::new();
    for (module, _) in request.project.modules() {
        let identity = request
            .project
            .source(module)
            .expect("each HIR project module has a source identity");
        let primary = request
            .facts
            .document(identity.id())
            .map_or_else(|| fallback.clone(), full_span);
        let Some(document) = request.facts.document(identity.id()) else {
            diagnostics.push(CharacterRegistrationDiagnostic::new(
                CharacterRegistrationDiagnosticKind::WrongDocument {
                    expected: request.facts.world().root_document().clone(),
                    actual: identity.id().clone(),
                },
                primary,
                [],
            ));
            continue;
        };
        if document.identity().revision() != identity.revision() {
            diagnostics.push(CharacterRegistrationDiagnostic::new(
                CharacterRegistrationDiagnosticKind::WrongRevision {
                    expected: document.identity().revision(),
                    actual: identity.revision(),
                },
                primary,
                [full_span(document)],
            ));
        }
    }
    diagnostics
}

fn collect_manifests(
    request: &CharacterRegistrationRequest<'_>,
    work: &mut u64,
    diagnostics: &mut Vec<CharacterRegistrationDiagnostic>,
    fallback: &SourceSpan,
) -> BTreeMap<CharacterId, ManifestRecord> {
    let catalog_count = u64::try_from(request.facts.catalogs().len()).unwrap_or(u64::MAX);
    if catalog_count > CharacterRegistrationLimits::PRODUCTION.catalogs() {
        diagnostics.push(CharacterRegistrationDiagnostic::new(
            CharacterRegistrationDiagnosticKind::Limit {
                kind: CharacterRegistrationLimitKind::Catalogs,
                observed: catalog_count,
                maximum: CharacterRegistrationLimits::PRODUCTION.catalogs(),
            },
            fallback.clone(),
            [],
        ));
    }
    let mut occurrences = 0_u64;
    let mut manifests = BTreeMap::<CharacterId, ManifestRecord>::new();
    for (catalog_index, catalog) in request.facts.catalogs().enumerate() {
        charge(work, 1, fallback, diagnostics);
        for (manifest_index, source_backed) in catalog.manifests().enumerate() {
            occurrences = occurrences.saturating_add(1);
            let manifest = source_backed.manifest();
            let owner = manifest.character().clone();
            let source = request
                .facts
                .manifest_owner_source(catalog_index, manifest_index)
                .cloned()
                .unwrap_or_else(|| {
                    diagnostics.push(CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::MissingProvenance {
                            token: RequiredCharacterToken::Manifest(
                                CharacterManifestTokenPath::Root(
                                    CharacterManifestRootField::Character,
                                ),
                            ),
                        },
                        fallback.clone(),
                        [],
                    ));
                    fallback.clone()
                });
            let expected_source = source_backed.source_map().document();
            if source.source().id() != expected_source.id() {
                diagnostics.push(CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::WrongDocument {
                        expected: expected_source.id().clone(),
                        actual: source.source().id().clone(),
                    },
                    source.clone(),
                    [],
                ));
            } else if source.source().revision() != expected_source.revision() {
                diagnostics.push(CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::WrongRevision {
                        expected: expected_source.revision(),
                        actual: source.source().revision(),
                    },
                    source.clone(),
                    request.facts.document(expected_source.id()).map(full_span),
                ));
            }
            charge_manifest(work, manifest, &source, diagnostics);
            merge_manifest_occurrence(
                &mut manifests,
                owner,
                manifest,
                source_backed.fingerprint(),
                source,
                diagnostics,
            );
        }
    }
    if occurrences > CharacterRegistrationLimits::PRODUCTION.manifest_occurrences() {
        diagnostics.push(CharacterRegistrationDiagnostic::new(
            CharacterRegistrationDiagnosticKind::Limit {
                kind: CharacterRegistrationLimitKind::ManifestOccurrences,
                observed: occurrences,
                maximum: CharacterRegistrationLimits::PRODUCTION.manifest_occurrences(),
            },
            fallback.clone(),
            [],
        ));
    }
    let owners = u64::try_from(manifests.len()).unwrap_or(u64::MAX);
    if owners > CharacterRegistrationLimits::PRODUCTION.owners() {
        diagnostics.push(CharacterRegistrationDiagnostic::new(
            CharacterRegistrationDiagnosticKind::Limit {
                kind: CharacterRegistrationLimitKind::Owners,
                observed: owners,
                maximum: CharacterRegistrationLimits::PRODUCTION.owners(),
            },
            fallback.clone(),
            [],
        ));
    }
    manifests
}

pub(super) fn merge_manifest_occurrence(
    manifests: &mut BTreeMap<CharacterId, ManifestRecord>,
    owner: CharacterId,
    manifest: &CharacterManifest,
    fingerprint: CharacterManifestFingerprint,
    source: SourceSpan,
    diagnostics: &mut Vec<CharacterRegistrationDiagnostic>,
) {
    match manifests.get_mut(&owner) {
        None => {
            manifests.insert(
                owner,
                ManifestRecord {
                    manifest: manifest.clone(),
                    fingerprint,
                    sources: vec![source],
                },
            );
        }
        Some(first) if first.fingerprint != fingerprint => {
            diagnostics.push(CharacterRegistrationDiagnostic::new(
                CharacterRegistrationDiagnosticKind::ConflictingManifest {
                    owner,
                    first: first.fingerprint,
                    conflicting: fingerprint,
                },
                source,
                first.sources.clone(),
            ));
        }
        Some(first) if !semantic_manifest_equal(&first.manifest, manifest) => {
            diagnostics.push(CharacterRegistrationDiagnostic::new(
                CharacterRegistrationDiagnosticKind::DigestCollision {
                    owner,
                    digest: fingerprint,
                },
                source,
                first.sources.clone(),
            ));
        }
        Some(first) => first.sources.push(source),
    }
}

fn build_external_owners(
    request: &CharacterRegistrationRequest<'_>,
    link: &arcweft_lang_hir::symbol::ProjectSymbolLinkOutput,
    manifests: &BTreeMap<CharacterId, ManifestRecord>,
    work: &mut u64,
    diagnostics: &mut Vec<CharacterRegistrationDiagnostic>,
    fallback: &SourceSpan,
) -> BTreeMap<ExternalDeclarationId, RegisteredExternalOwner> {
    let mut owners = BTreeMap::new();
    for contribution in request.facts.external_owner_contributions() {
        charge(work, 1, &contribution.owner_source, diagnostics);
        let Some(declaration) = link.seed_declaration(contribution.seed) else {
            diagnostics.push(CharacterRegistrationDiagnostic::new(
                CharacterRegistrationDiagnosticKind::MissingProvenance {
                    token: RequiredCharacterToken::ExternalDeclaration,
                },
                contribution.owner_source.clone(),
                [],
            ));
            continue;
        };
        let valid_owner = match &contribution.target {
            RegisteredExternalOwner::Character(owner) => manifests.contains_key(owner),
            RegisteredExternalOwner::Environment(id) => {
                request.base.environment_binding(id).is_some()
            }
        };
        if !valid_owner {
            diagnostics.push(CharacterRegistrationDiagnostic::new(
                CharacterRegistrationDiagnosticKind::UnknownOwner {
                    owner: contribution.target.clone(),
                },
                contribution.owner_source.clone(),
                [],
            ));
            continue;
        }
        match owners.get(&declaration) {
            None => {
                owners.insert(declaration, contribution.target.clone());
            }
            Some(first) if first == &contribution.target => {
                diagnostics.push(CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::ExternalDuplicate {
                        declaration,
                        owner: contribution.target.clone(),
                    },
                    contribution.owner_source.clone(),
                    [link.table().external(declaration).map_or_else(
                        || fallback.clone(),
                        |symbol| symbol.declaration_span().clone(),
                    )],
                ));
            }
            Some(first) => {
                diagnostics.push(CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::ExternalConflict {
                        declaration,
                        first: first.clone(),
                        conflicting: contribution.target.clone(),
                    },
                    contribution.owner_source.clone(),
                    [link.table().external(declaration).map_or_else(
                        || fallback.clone(),
                        |symbol| symbol.declaration_span().clone(),
                    )],
                ));
            }
        }
    }
    owners
}

#[allow(
    clippy::too_many_lines,
    reason = "the audit exhaustively maps every typed symbol-resolution outcome into one stable registration diagnostic"
)]
fn audit_character_spellings(
    symbols: &ProjectSymbolTable,
    owners: &BTreeMap<ExternalDeclarationId, RegisteredExternalOwner>,
    manifests: &BTreeMap<CharacterId, ManifestRecord>,
    diagnostics: &mut Vec<CharacterRegistrationDiagnostic>,
) {
    let root = CanonicalModulePath::crate_root();
    for (declaration, owner) in owners {
        let RegisteredExternalOwner::Character(character) = owner else {
            continue;
        };
        let Some(record) = manifests.get(character) else {
            continue;
        };
        let expected = ProjectSymbolTargetId::External(*declaration);
        for collision in symbols.binding_collisions_for(&expected) {
            let spelling = SymbolPath::try_new(
                ModulePathRoot::Crate,
                collision.module().segments().to_vec(),
                collision.path().to_string(),
            )
            .expect("formatting a typed binding path produces a valid diagnostic leaf");
            let primary = collision
                .expected_sites()
                .first()
                .cloned()
                .unwrap_or_else(|| record.primary_source().clone());
            let secondary = collision
                .expected_sites()
                .iter()
                .skip(1)
                .chain(collision.conflicting_sites())
                .cloned()
                .collect::<Vec<_>>();
            diagnostics.push(CharacterRegistrationDiagnostic::new(
                CharacterRegistrationDiagnosticKind::AliasCollision {
                    spelling,
                    expected: *declaration,
                    conflicting: collision.conflicting().to_vec(),
                },
                primary,
                secondary,
            ));
        }
        let compact_segments = character
            .compact_segments()
            .map(|segment| {
                ProjectSymbolSegment::try_new(segment)
                    .expect("character compact segments are valid project symbol segments")
            })
            .collect::<Vec<_>>();
        let qualified = ProjectSymbolPath::new(
            ModulePathRoot::ImplicitCrate,
            std::iter::once(
                ProjectSymbolSegment::try_new("character")
                    .expect("character namespace is a valid project symbol segment"),
            )
            .chain(compact_segments.iter().cloned()),
        )
        .expect("character qualified paths have a valid implicit root");
        let compact = ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, compact_segments)
            .expect("character compact paths have a valid implicit root");
        for binding_path in [&qualified, &compact] {
            let path = SymbolPath::try_from(binding_path)
                .expect("typed character binding paths are valid resolution references");
            match symbols.resolve(&root, &path, record.primary_source()) {
                Ok(ResolvedProjectSymbol::External(symbol))
                    if symbol.declaration() == *declaration => {}
                Ok(ResolvedProjectSymbol::External(symbol)) => {
                    diagnostics.push(CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::AliasCollision {
                            spelling: path,
                            expected: *declaration,
                            conflicting: vec![ProjectSymbolTargetId::External(
                                symbol.declaration(),
                            )],
                        },
                        record.primary_source().clone(),
                        [symbol.declaration_span().clone()],
                    ));
                }
                Ok(ResolvedProjectSymbol::Callable(symbol)) => {
                    diagnostics.push(CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::AliasCollision {
                            spelling: path,
                            expected: *declaration,
                            conflicting: vec![ProjectSymbolTargetId::Callable(
                                symbol.declaration().clone(),
                            )],
                        },
                        record.primary_source().clone(),
                        [symbol.source().clone()],
                    ));
                }
                Ok(ResolvedProjectSymbol::Module(module)) => {
                    diagnostics.push(CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::AliasCollision {
                            spelling: path,
                            expected: *declaration,
                            conflicting: vec![ProjectSymbolTargetId::Module(module.clone())],
                        },
                        record.primary_source().clone(),
                        [],
                    ));
                }
                Err(arcweft_lang_hir::symbol::ProjectSymbolResolutionError::Ambiguous {
                    candidates,
                    ..
                }) if candidates.contains(&expected) => {}
                Err(arcweft_lang_hir::symbol::ProjectSymbolResolutionError::Ambiguous {
                    candidates,
                    ..
                }) => {
                    diagnostics.push(CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::AliasCollision {
                            spelling: path,
                            expected: *declaration,
                            conflicting: candidates,
                        },
                        record.primary_source().clone(),
                        [],
                    ));
                }
                Err(_) => diagnostics.push(CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::AliasCollision {
                        spelling: path,
                        expected: *declaration,
                        conflicting: Vec::new(),
                    },
                    record.primary_source().clone(),
                    [],
                )),
            }
        }
    }
}

fn character_variants(
    characters: &BTreeMap<CharacterId, CharacterManifest>,
) -> BTreeMap<CharacterNominalType, BTreeSet<String>> {
    let mut variants = BTreeMap::new();
    for (owner, manifest) in characters {
        variants.insert(
            CharacterNominalType::Look {
                character: owner.clone(),
            },
            manifest
                .looks()
                .iter()
                .map(|look| look.id().as_str().to_owned())
                .collect(),
        );
        variants.insert(
            CharacterNominalType::Part {
                character: owner.clone(),
            },
            manifest
                .parts()
                .iter()
                .map(|part| part.id().as_str().to_owned())
                .collect(),
        );
        for part in manifest.parts() {
            variants.insert(
                CharacterNominalType::Variant {
                    character: owner.clone(),
                    part: part.id().clone(),
                },
                part.variants()
                    .iter()
                    .map(|variant| variant.id().as_str().to_owned())
                    .collect(),
            );
        }
    }
    variants
}

fn next_revision(
    previous: Option<&RegisteredTypeCheckEnv>,
    world: &arcweft_lang_hir::symbol::ProjectSymbolWorldId,
    digest: super::model::CharacterInventoryDigest,
) -> Result<CharacterInventoryRevision, CharacterInventoryRevision> {
    let Some(previous) = previous.filter(|previous| previous.world() == world) else {
        return Ok(CharacterInventoryRevision(1));
    };
    if previous.character_digest() == digest {
        return Ok(previous.character_revision());
    }
    previous
        .character_revision()
        .get()
        .checked_add(1)
        .map(CharacterInventoryRevision)
        .ok_or(previous.character_revision())
}

pub(super) fn charge(
    work: &mut u64,
    units: u64,
    source: &SourceSpan,
    diagnostics: &mut Vec<CharacterRegistrationDiagnostic>,
) {
    let Some(attempted) = work.checked_add(units) else {
        diagnostics.push(CharacterRegistrationDiagnostic::new(
            CharacterRegistrationDiagnosticKind::ArithmeticOverflow {
                counter: CharacterRegistrationLimitKind::Work,
            },
            source.clone(),
            [],
        ));
        return;
    };
    if attempted > CharacterRegistrationLimits::PRODUCTION.work() {
        diagnostics.push(CharacterRegistrationDiagnostic::new(
            CharacterRegistrationDiagnosticKind::WorkOverflow {
                attempted,
                maximum: CharacterRegistrationLimits::PRODUCTION.work(),
            },
            source.clone(),
            [],
        ));
        return;
    }
    *work = attempted;
}

fn charge_manifest(
    work: &mut u64,
    manifest: &CharacterManifest,
    source: &SourceSpan,
    diagnostics: &mut Vec<CharacterRegistrationDiagnostic>,
) {
    charge(work, 1, source, diagnostics);
    for part in manifest.parts() {
        charge(work, 1, source, diagnostics);
        for _ in part.variants() {
            charge(work, 1, source, diagnostics);
        }
    }
    for look in manifest.looks() {
        charge(work, 1, source, diagnostics);
        for _ in look.selections() {
            charge(work, 1, source, diagnostics);
        }
    }
}

fn semantic_manifest_equal(left: &CharacterManifest, right: &CharacterManifest) -> bool {
    left.character() == right.character()
        && left.canvas() == right.canvas()
        && left.anchor() == right.anchor()
        && left.default_look() == right.default_look()
        && sorted_parts(left.parts()) == sorted_parts(right.parts())
        && sorted_looks(left.looks()) == sorted_looks(right.looks())
}

fn sorted_parts(parts: &[CharacterPart]) -> Vec<CanonicalPart<'_>> {
    let mut parts = parts
        .iter()
        .map(|part| {
            let mut variants = part
                .variants()
                .iter()
                .map(CanonicalVariant::from)
                .collect::<Vec<_>>();
            variants.sort();
            CanonicalPart {
                id: part.id().as_str(),
                z: part.z(),
                variants,
            }
        })
        .collect::<Vec<_>>();
    parts.sort();
    parts
}

fn sorted_looks(looks: &[CharacterLook]) -> Vec<CanonicalLook<'_>> {
    let mut looks = looks
        .iter()
        .map(|look| {
            let mut selections = look
                .selections()
                .iter()
                .map(CanonicalSelection::from)
                .collect::<Vec<_>>();
            selections.sort();
            CanonicalLook {
                id: look.id().as_str(),
                selections,
            }
        })
        .collect::<Vec<_>>();
    looks.sort();
    looks
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct CanonicalPart<'a> {
    id: &'a str,
    z: i32,
    variants: Vec<CanonicalVariant<'a>>,
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct CanonicalVariant<'a> {
    id: &'a str,
    asset: &'a str,
    rect: (i32, i32, u32, u32),
    opacity: u8,
    blend: u32,
    clipping: bool,
}

impl<'a> From<&'a CharacterVariant> for CanonicalVariant<'a> {
    fn from(variant: &'a CharacterVariant) -> Self {
        let rect = variant.rect();
        Self {
            id: variant.id().as_str(),
            asset: variant.asset().as_str(),
            rect: (rect.x(), rect.y(), rect.width(), rect.height()),
            opacity: variant.opacity(),
            blend: variant.blend().stable_code(),
            clipping: variant.clipping(),
        }
    }
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct CanonicalLook<'a> {
    id: &'a str,
    selections: Vec<CanonicalSelection<'a>>,
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct CanonicalSelection<'a> {
    part: &'a str,
    variant: &'a str,
}

impl<'a> From<&'a CharacterPartSelection> for CanonicalSelection<'a> {
    fn from(selection: &'a CharacterPartSelection) -> Self {
        Self {
            part: selection.part().as_str(),
            variant: selection.variant().as_str(),
        }
    }
}

fn link_error_source(error: &ProjectSymbolLinkError) -> Option<SourceSpan> {
    match error {
        ProjectSymbolLinkError::DuplicateDeclaration { duplicate, .. } => Some(duplicate.clone()),
        ProjectSymbolLinkError::InaccessibleImport { source, .. }
        | ProjectSymbolLinkError::VisibilityEscalation { source, .. }
        | ProjectSymbolLinkError::AmbiguousImport { source, .. }
        | ProjectSymbolLinkError::InvalidImportPath { source, .. }
        | ProjectSymbolLinkError::InvalidDeclaration { source, .. } => Some(source.clone()),
        ProjectSymbolLinkError::Limit { source, .. }
        | ProjectSymbolLinkError::WorkOverflow { source, .. } => source.clone(),
    }
}

fn full_span(document: &arcweft_source::SourceDocument) -> SourceSpan {
    document
        .span(SourceRange::new(0, document.text().len()))
        .expect("complete document range is valid")
}
