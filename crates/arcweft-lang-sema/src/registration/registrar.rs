use std::{collections::BTreeMap, sync::Arc};

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
    callable::{
        EnvironmentPublicationProjectionErrorKind, EnvironmentPublicationProjectionReport,
        PRODUCTION_CALLABLE_LIMITS, RegisteredCallableCatalogBuilder,
    },
    character_dialogue::{
        CharacterDialogueCustomFieldDescriptor, CharacterDialogueCustomFieldRegistry,
    },
    env::{
        TypeCheckEnv,
        nominal::{
            AcceptedNominalCatalogError, AcceptedNominalId, AcceptedNominalOrigin,
            AcceptedNominalOwnerId, AcceptedNominalRecord, AcceptedNominalSemantics,
            OpenNominalEnvironment,
        },
    },
    nominal::{NominalAggregationLimits, NominalResolutionLimits},
    registration::{AcceptedNominalInputVisibility, AcceptedNominalSource},
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
        AcceptedNominalVisibilityIndex, AcceptedNominalWorld, CharacterInventoryRevision,
        CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts,
        ProofReturnRegistrationPrelude, ProofReturnRegistrationRequest, RegisteredExternalOwner,
        RegisteredSemanticWorld, RegisteredStatementIngressTypes, RegisteredTypeCheckEnv,
    },
    source_index::CharacterDefinitionIndex,
};

pub(super) struct ManifestRecord {
    manifest: CharacterManifest,
    fingerprint: CharacterManifestFingerprint,
    sources: Vec<SourceSpan>,
}

impl CharacterRegistrar {
    /// Freezes the exact project symbols and accepted nominal world needed to
    /// classify authored Proof returns while their bodies remain unallocated.
    #[allow(
        clippy::too_many_lines,
        clippy::needless_pass_by_value,
        reason = "the pre-publication registration transaction validates one complete accepted source world"
    )]
    pub fn prepare_proof_return_headers(
        request: ProofReturnRegistrationRequest<'_>,
    ) -> Result<ProofReturnRegistrationPrelude, CharacterRegistrationReport> {
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
        let mut diagnostics =
            validate_proof_return_project_sources(request.project, request.facts, &fallback);
        if !diagnostics.is_empty() {
            return Err(CharacterRegistrationReport::from_diagnostics(diagnostics));
        }

        let link = match ProjectSymbolTable::link_proof_return_headers(
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
        let manifests = collect_manifests(request.facts, &mut work, &mut diagnostics, &fallback);
        let owners = build_external_owners(
            &request.base,
            request.facts,
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

        let (accepted_base, statement_ingress_inputs, visibility) =
            match accepted_external_environment(&request.base, request.facts, &link, &owners) {
                Ok(environment) => environment,
                Err(error) => {
                    return Err(CharacterRegistrationReport::from_diagnostics(vec![
                        CharacterRegistrationDiagnostic::new(
                            CharacterRegistrationDiagnosticKind::AcceptedNominalCatalog { error },
                            fallback,
                            [],
                        ),
                    ]));
                }
            };
        let statement_ingress = RegisteredStatementIngressTypes::try_new(statement_ingress_inputs)
            .map_err(|error| statement_ingress_registration_report(error, fallback.clone()))?;
        let nominal_world = AcceptedNominalWorld::new(
            accepted_base,
            request.facts.world().clone(),
            *request.facts.symbol_revision(),
            owners,
            visibility,
        )
        .with_host_call_contracts(host_call_contracts(request.facts));
        let mut environment_bindings = BTreeMap::new();
        for input in request.facts.environment_inputs() {
            let projected = nominal_world
                .try_project_environment_bindings(
                    input,
                    NominalResolutionLimits::PRODUCTION,
                    NominalAggregationLimits::PRODUCTION,
                )
                .map_err(environment_projection_registration_report)?;
            for (id, ty) in projected {
                if environment_bindings.insert(id, ty).is_some() {
                    return Err(CharacterRegistrationReport::from_diagnostics(vec![
                        CharacterRegistrationDiagnostic::new(
                            CharacterRegistrationDiagnosticKind::CallableCatalog {
                                code:
                                    crate::callable::CallableDiagnosticCode::CorruptCallableCatalog,
                            },
                            fallback.clone(),
                            [],
                        ),
                    ]));
                }
            }
        }
        let environment_aliases = environment_external_alias_records(
            &request.base,
            request.facts,
            &link,
            nominal_world.external_owners(),
            &environment_bindings,
        )
        .map_err(|error| {
            CharacterRegistrationReport::from_diagnostics(vec![
                CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::AcceptedNominalCatalog { error },
                    fallback.clone(),
                    [],
                ),
            ])
        })?;
        let nominal_world = Arc::new(
            nominal_world
                .try_with_environment_bindings(environment_bindings, environment_aliases)
                .map_err(|error| {
                    CharacterRegistrationReport::from_diagnostics(vec![
                        CharacterRegistrationDiagnostic::new(
                            CharacterRegistrationDiagnosticKind::AcceptedNominalCatalog { error },
                            fallback.clone(),
                            [],
                        ),
                    ])
                })?,
        );
        let rust_metadata_inputs = request
            .facts
            .environment_inputs()
            .flat_map(|input| input.input().rust_metadata().iter().cloned())
            .collect::<Vec<_>>();
        let rust_metadata = Arc::new(
            nominal_world
                .try_project_rust_metadata(
                    &rust_metadata_inputs,
                    NominalResolutionLimits::PRODUCTION,
                    NominalAggregationLimits::PRODUCTION,
                )
                .map_err(environment_projection_registration_report)?,
        );
        let symbols = Arc::new(link.into_table());
        Ok(ProofReturnRegistrationPrelude {
            generation: request.generation,
            symbols,
            nominal_world,
            rust_metadata,
            statement_ingress,
            characters,
            character_variants,
            character_descriptor: descriptor,
            character_digest: digest,
            character_revision: revision,
        })
    }

    /// Completes registration from the exact prelude that classified Proof
    /// returns. The published project must be the same module/snapshot/source
    /// generation; symbols and nominal resolution are not rebuilt.
    #[allow(
        clippy::too_many_lines,
        clippy::needless_pass_by_value,
        reason = "one atomic continuation publishes callable and character consumers from the frozen prelude"
    )]
    pub fn finish_proof_return_registration(
        project: arcweft_lang_hir::project::HirProjectView<'_>,
        facts: &super::model::ProjectRegistrationFacts,
        prelude: ProofReturnRegistrationPrelude,
    ) -> Result<RegisteredSemanticWorld, CharacterRegistrationReport> {
        let Some(fallback) = facts
            .document(facts.world().root_document())
            .or_else(|| facts.documents().next().map(AsRef::as_ref))
            .map(full_span)
        else {
            return Err(CharacterRegistrationReport::from_diagnostics(Vec::new()).with_omitted(1));
        };
        let module_count = project.modules().count();
        if module_count != prelude.generation.modules().len()
            || project.modules().any(|(_, module)| {
                prelude
                    .generation
                    .validate_module_transaction(
                        module.key().package(),
                        module.key().path(),
                        module.snapshot_id(),
                        module.provenance().syntax_snapshot(),
                        module.provenance().source_identity(),
                    )
                    .is_err()
            })
        {
            return Err(CharacterRegistrationReport::from_diagnostics(vec![
                CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::StaleSource {
                        expected: prelude.generation.revision(),
                        actual: *facts.symbol_revision(),
                    },
                    fallback,
                    [],
                ),
            ]));
        }

        let ProofReturnRegistrationPrelude {
            generation: _,
            symbols,
            nominal_world,
            rust_metadata,
            statement_ingress,
            characters,
            character_variants,
            character_descriptor,
            character_digest,
            character_revision,
        } = prelude;
        let mut callable_builder = RegisteredCallableCatalogBuilder::for_nominal_world(
            &nominal_world,
            PRODUCTION_CALLABLE_LIMITS,
        );
        if let Err(error) = callable_builder.add_project(project, &symbols, &nominal_world) {
            return Err(CharacterRegistrationReport::from_diagnostics(vec![
                CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::CallableCatalog { code: error.code() },
                    fallback,
                    [],
                ),
            ]));
        }
        if let Err(error) =
            callable_builder.add_project_bindings(project, &symbols, |target| match target {
                ProjectSymbolTargetId::External(declaration) => {
                    match nominal_world.external_owners().get(declaration) {
                        Some(RegisteredExternalOwner::Character(_)) => {
                            Some(TypeKind::Ref(EntityType::new(EntityKind::Character, None)))
                        }
                        Some(RegisteredExternalOwner::Environment(owner)) => nominal_world
                            .environment_binding(owner.value_binding())
                            .cloned(),
                        None => None,
                    }
                }
                ProjectSymbolTargetId::Module(_) => Some(TypeKind::Named("Module".to_owned())),
                ProjectSymbolTargetId::Nominal(declaration) => {
                    Some(TypeKind::Named(declaration.name().as_str().to_owned()))
                }
                ProjectSymbolTargetId::Retained(public_id) => symbols
                    .retained(public_id)
                    .and_then(|symbol| {
                        EntityKind::from_declaration_identity_family(symbol.family())
                    })
                    .map(|kind| TypeKind::Ref(EntityType::new(kind, None))),
                ProjectSymbolTargetId::Callable(_)
                | ProjectSymbolTargetId::StructuralCallable(_) => None,
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
        let standard_publication = match nominal_world
            .typecheck_env()
            .standard_callable_publication(nominal_world.stamp(), &PRODUCTION_CALLABLE_LIMITS)
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
        for input in facts.environment_inputs() {
            let publication = match nominal_world.try_project_environment_publication(
                input,
                NominalResolutionLimits::PRODUCTION,
                NominalAggregationLimits::PRODUCTION,
                &PRODUCTION_CALLABLE_LIMITS,
            ) {
                Ok(publication) => publication,
                Err(report) => return Err(environment_projection_registration_report(report)),
            };
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
        let character_dialogue_fields =
            build_character_dialogue_fields(&nominal_world, facts, fallback.clone())?;
        let environment_digest = super::environment_digest::derive(
            &nominal_world,
            rust_metadata.digest().as_bytes(),
            callables.digest().as_bytes(),
            character_dialogue_fields.semantic_digest(),
            &statement_ingress,
            facts,
            character_digest,
            character_revision,
        );
        let environment = Arc::new(RegisteredTypeCheckEnv {
            nominal_world,
            character_dialogue_fields,
            rust_metadata,
            callables,
            statement_ingress,
            characters,
            character_variants,
            character_descriptor,
            character_digest,
            character_revision,
            environment_digest,
        });
        let character_definitions =
            match CharacterDefinitionIndex::try_build(facts, &symbols, &environment) {
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
        clippy::needless_pass_by_value,
        reason = "registration consumes one request as a fail-closed transaction into an accepted semantic world"
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

        let manifests = collect_manifests(request.facts, &mut work, &mut diagnostics, &fallback);
        let owners = build_external_owners(
            &request.base,
            request.facts,
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

        let (accepted_base, statement_ingress_inputs, visibility) =
            match accepted_external_environment(&request.base, request.facts, &link, &owners) {
                Ok(environment) => environment,
                Err(error) => {
                    return Err(CharacterRegistrationReport::from_diagnostics(vec![
                        CharacterRegistrationDiagnostic::new(
                            CharacterRegistrationDiagnosticKind::AcceptedNominalCatalog { error },
                            fallback,
                            [],
                        ),
                    ]));
                }
            };
        let statement_ingress = RegisteredStatementIngressTypes::try_new(statement_ingress_inputs)
            .map_err(|error| statement_ingress_registration_report(error, fallback.clone()))?;
        let nominal_world = AcceptedNominalWorld::new(
            accepted_base,
            request.facts.world().clone(),
            *request.facts.symbol_revision(),
            owners,
            visibility,
        )
        .with_host_call_contracts(host_call_contracts(request.facts));
        let mut environment_bindings = BTreeMap::new();
        for input in request.facts.environment_inputs() {
            let projected = nominal_world
                .try_project_environment_bindings(
                    input,
                    NominalResolutionLimits::PRODUCTION,
                    NominalAggregationLimits::PRODUCTION,
                )
                .map_err(environment_projection_registration_report)?;
            for (id, ty) in projected {
                if environment_bindings.insert(id, ty).is_some() {
                    return Err(CharacterRegistrationReport::from_diagnostics(vec![
                        CharacterRegistrationDiagnostic::new(
                            CharacterRegistrationDiagnosticKind::CallableCatalog {
                                code:
                                    crate::callable::CallableDiagnosticCode::CorruptCallableCatalog,
                            },
                            fallback.clone(),
                            [],
                        ),
                    ]));
                }
            }
        }
        let environment_aliases = environment_external_alias_records(
            &request.base,
            request.facts,
            &link,
            nominal_world.external_owners(),
            &environment_bindings,
        )
        .map_err(|error| {
            CharacterRegistrationReport::from_diagnostics(vec![
                CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::AcceptedNominalCatalog { error },
                    fallback.clone(),
                    [],
                ),
            ])
        })?;
        let nominal_world = Arc::new(
            nominal_world
                .try_with_environment_bindings(environment_bindings, environment_aliases)
                .map_err(|error| {
                    CharacterRegistrationReport::from_diagnostics(vec![
                        CharacterRegistrationDiagnostic::new(
                            CharacterRegistrationDiagnosticKind::AcceptedNominalCatalog { error },
                            fallback.clone(),
                            [],
                        ),
                    ])
                })?,
        );

        let rust_metadata_inputs = request
            .facts
            .environment_inputs()
            .flat_map(|input| input.input().rust_metadata().iter().cloned())
            .collect::<Vec<_>>();
        let rust_metadata = Arc::new(
            nominal_world
                .try_project_rust_metadata(
                    &rust_metadata_inputs,
                    NominalResolutionLimits::PRODUCTION,
                    NominalAggregationLimits::PRODUCTION,
                )
                .map_err(environment_projection_registration_report)?,
        );

        let mut callable_builder = RegisteredCallableCatalogBuilder::for_nominal_world(
            &nominal_world,
            PRODUCTION_CALLABLE_LIMITS,
        );
        if let Err(error) =
            callable_builder.add_project(request.project, link.table(), &nominal_world)
        {
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
                    ProjectSymbolTargetId::External(declaration) => {
                        match nominal_world.external_owners().get(declaration) {
                            Some(RegisteredExternalOwner::Character(_)) => {
                                Some(TypeKind::Ref(EntityType::new(EntityKind::Character, None)))
                            }
                            Some(RegisteredExternalOwner::Environment(owner)) => nominal_world
                                .environment_binding(owner.value_binding())
                                .cloned(),
                            None => None,
                        }
                    }
                    ProjectSymbolTargetId::Module(_) => Some(TypeKind::Named("Module".to_owned())),
                    ProjectSymbolTargetId::Nominal(declaration) => {
                        Some(TypeKind::Named(declaration.name().as_str().to_owned()))
                    }
                    ProjectSymbolTargetId::Retained(public_id) => link
                        .table()
                        .retained(public_id)
                        .and_then(|symbol| {
                            EntityKind::from_declaration_identity_family(symbol.family())
                        })
                        .map(|kind| TypeKind::Ref(EntityType::new(kind, None))),
                    ProjectSymbolTargetId::Callable(_)
                    | ProjectSymbolTargetId::StructuralCallable(_) => None,
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
        let standard_publication = match nominal_world
            .typecheck_env()
            .standard_callable_publication(nominal_world.stamp(), &PRODUCTION_CALLABLE_LIMITS)
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
        for input in request.facts.environment_inputs() {
            let publication = match nominal_world.try_project_environment_publication(
                input,
                NominalResolutionLimits::PRODUCTION,
                NominalAggregationLimits::PRODUCTION,
                &PRODUCTION_CALLABLE_LIMITS,
            ) {
                Ok(publication) => publication,
                Err(report) => return Err(environment_projection_registration_report(report)),
            };
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

        let character_dialogue_fields =
            build_character_dialogue_fields(&nominal_world, request.facts, fallback.clone())?;
        let environment_digest = super::environment_digest::derive(
            &nominal_world,
            rust_metadata.digest().as_bytes(),
            callables.digest().as_bytes(),
            character_dialogue_fields.semantic_digest(),
            &statement_ingress,
            request.facts,
            digest,
            revision,
        );

        let symbols = Arc::new(link.into_table());
        let environment = Arc::new(RegisteredTypeCheckEnv {
            nominal_world,
            character_dialogue_fields,
            rust_metadata,
            callables,
            statement_ingress,
            characters,
            character_variants,
            character_descriptor: descriptor,
            character_digest: digest,
            character_revision: revision,
            environment_digest,
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

fn build_character_dialogue_fields(
    nominal_world: &AcceptedNominalWorld,
    facts: &ProjectRegistrationFacts,
    fallback: SourceSpan,
) -> Result<Arc<CharacterDialogueCustomFieldRegistry>, CharacterRegistrationReport> {
    let mut descriptors = Vec::new();
    for environment in facts.environment_inputs() {
        for field in environment.input().character_dialogue_fields() {
            let value_type = nominal_world
                .try_project_character_dialogue_field_type(
                    field.value_type(),
                    field.item(),
                    NominalResolutionLimits::PRODUCTION,
                )
                .map_err(environment_projection_registration_report)?;
            descriptors.push(CharacterDialogueCustomFieldDescriptor::new(
                field.id().clone(),
                field.bindings().to_vec(),
                value_type,
                field.runtime_nominal_type().cloned(),
                field.runtime_layout(),
                field.clearable(),
                field.accepted_views().clone(),
                field.declaration().clone(),
            ));
        }
    }
    CharacterDialogueCustomFieldRegistry::try_new(nominal_world.stamp(), descriptors)
        .map(Arc::new)
        .map_err(|error| {
            CharacterRegistrationReport::from_diagnostics(vec![
                CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::CharacterDialogueCustomFields { error },
                    fallback,
                    [],
                ),
            ])
        })
}

fn statement_ingress_registration_report(
    error: super::model::StatementIngressRegistrationError,
    fallback: SourceSpan,
) -> CharacterRegistrationReport {
    CharacterRegistrationReport::from_diagnostics(vec![CharacterRegistrationDiagnostic::new(
        CharacterRegistrationDiagnosticKind::StatementIngress { error },
        fallback,
        [],
    )])
}

fn environment_projection_registration_report(
    report: EnvironmentPublicationProjectionReport,
) -> CharacterRegistrationReport {
    let (projection_diagnostics, omitted_diagnostics) = report.into_parts();
    let diagnostics = projection_diagnostics
        .iter()
        .map(|diagnostic| {
            let code = match diagnostic.kind() {
                EnvironmentPublicationProjectionErrorKind::LimitExceeded { .. } => {
                    crate::callable::CallableDiagnosticCode::ResourceExhausted
                }
                EnvironmentPublicationProjectionErrorKind::WorldMismatch => {
                    crate::callable::CallableDiagnosticCode::WorldMismatch
                }
                EnvironmentPublicationProjectionErrorKind::UnknownPath { .. }
                | EnvironmentPublicationProjectionErrorKind::InaccessibleExport { .. }
                | EnvironmentPublicationProjectionErrorKind::OwnerMismatch { .. }
                | EnvironmentPublicationProjectionErrorKind::WrongArity { .. }
                | EnvironmentPublicationProjectionErrorKind::InvalidAcceptedSemantics { .. }
                | EnvironmentPublicationProjectionErrorKind::FreeTypeParameterInCallable {
                    ..
                }
                | EnvironmentPublicationProjectionErrorKind::UnboundMetadataTypeParameter {
                    ..
                }
                | EnvironmentPublicationProjectionErrorKind::MetadataOwnerMismatch { .. }
                | EnvironmentPublicationProjectionErrorKind::RustMetadataCatalog { .. }
                | EnvironmentPublicationProjectionErrorKind::Callable { .. } => {
                    crate::callable::CallableDiagnosticCode::CorruptCallableCatalog
                }
            };
            CharacterRegistrationDiagnostic::new(
                CharacterRegistrationDiagnosticKind::CallableCatalog { code },
                diagnostic.primary().clone(),
                diagnostic
                    .related()
                    .iter()
                    .map(|related| related.source().clone()),
            )
        })
        .collect();
    CharacterRegistrationReport::from_diagnostics(diagnostics)
        .with_omitted(u64::try_from(omitted_diagnostics).unwrap_or(u64::MAX))
}

#[allow(
    clippy::result_large_err,
    reason = "accepted catalog construction preserves its complete typed atomic-failure evidence"
)]
fn accepted_external_environment(
    base: &Arc<TypeCheckEnv>,
    facts: &super::model::ProjectRegistrationFacts,
    link: &arcweft_lang_hir::symbol::ProjectSymbolLinkOutput,
    owners: &BTreeMap<ExternalDeclarationId, RegisteredExternalOwner>,
) -> Result<
    (
        Arc<TypeCheckEnv>,
        Box<[super::model::StatementIngressTypePublicationInput]>,
        AcceptedNominalVisibilityIndex,
    ),
    AcceptedNominalCatalogError,
> {
    base.nominal_catalog()
        .validate_scopes_for(OpenNominalEnvironment::Accepted)?;
    let mut environment = base.as_ref().clone();
    let statement_ingress_inputs = environment.take_statement_ingress_inputs();
    let mut visible = BTreeMap::new();
    let mut inaccessible = BTreeMap::new();
    for input in facts.environment_inputs() {
        for nominal in input.input().nominal_inventory() {
            environment.try_insert_nominal_record(AcceptedNominalRecord::try_new_opaque(
                nominal.id().clone(),
                nominal.arity(),
                nominal.runtime_carrier().producer().clone(),
                nominal.runtime_carrier().value_class(),
                nominal.runtime_carrier().persistence(),
                nominal.origin(),
                Some(nominal.source().clone()),
            )?)?;
            let source =
                AcceptedNominalSource::new(nominal.source().clone(), nominal.item().clone());
            match nominal.visibility() {
                AcceptedNominalInputVisibility::Visible => {
                    visible.insert(nominal.id().clone(), source);
                }
                AcceptedNominalInputVisibility::Inaccessible => {
                    inaccessible.insert(nominal.id().clone(), source);
                }
            }
        }
    }
    for (seed_id, declaration) in link.seed_declarations() {
        let Some(owner) = owners.get(&declaration) else {
            continue;
        };
        let seed = facts
            .external_declarations()
            .declaration(seed_id)
            .expect("linked external seeds belong to the accepted registration facts");
        let (accepted_owner, semantics, origin) = match owner {
            RegisteredExternalOwner::Environment(_) => continue,
            RegisteredExternalOwner::Character(character) => (
                AcceptedNominalOwnerId::Character(character.clone()),
                AcceptedNominalSemantics::Exact(TypeKind::Ref(EntityType::new(
                    EntityKind::Character,
                    None,
                ))),
                AcceptedNominalOrigin::Character,
            ),
        };
        for binding in seed.direct_bindings() {
            environment.try_insert_nominal_record(AcceptedNominalRecord::try_new(
                AcceptedNominalId::new(accepted_owner.clone(), binding.path().clone().into()),
                0,
                semantics.clone(),
                origin,
                Some(binding.source().clone()),
            )?)?;
        }
    }
    Ok((
        Arc::new(environment),
        statement_ingress_inputs,
        AcceptedNominalVisibilityIndex::from_parts(visible, inaccessible),
    ))
}

fn host_call_contracts(
    facts: &super::model::ProjectRegistrationFacts,
) -> Box<[crate::registration::EnvironmentHostCallContractInput]> {
    facts
        .environment_inputs()
        .flat_map(|input| input.input().host_call_contracts().iter().cloned())
        .collect()
}

fn environment_external_alias_records(
    base: &Arc<TypeCheckEnv>,
    facts: &super::model::ProjectRegistrationFacts,
    link: &arcweft_lang_hir::symbol::ProjectSymbolLinkOutput,
    owners: &BTreeMap<ExternalDeclarationId, RegisteredExternalOwner>,
    bindings: &BTreeMap<crate::env::identity::EnvironmentBindingId, TypeKind>,
) -> Result<Vec<AcceptedNominalRecord>, AcceptedNominalCatalogError> {
    let mut aliases = Vec::new();
    for (seed_id, declaration) in link.seed_declarations() {
        let Some(RegisteredExternalOwner::Environment(owner)) = owners.get(&declaration) else {
            continue;
        };
        let Some(ty) = bindings
            .get(owner.value_binding())
            .or_else(|| base.environment_binding(owner.value_binding()))
        else {
            continue;
        };
        if matches!(ty, TypeKind::AcceptedNominal(_)) {
            continue;
        }
        let seed = facts
            .external_declarations()
            .declaration(seed_id)
            .expect("linked external seeds belong to the accepted registration facts");
        for binding in seed.direct_bindings() {
            aliases.push(AcceptedNominalRecord::try_new(
                AcceptedNominalId::new(
                    AcceptedNominalOwnerId::Environment(owner.nominal_owner().clone()),
                    binding.path().clone().into(),
                ),
                0,
                AcceptedNominalSemantics::Exact(ty.clone()),
                AcceptedNominalOrigin::Adapter,
                Some(binding.source().clone()),
            )?);
        }
    }
    Ok(aliases)
}

fn validate_project_sources(
    request: &CharacterRegistrationRequest<'_>,
    fallback: &SourceSpan,
) -> Vec<CharacterRegistrationDiagnostic> {
    let mut diagnostics = Vec::new();
    for (_, module) in request.project.modules() {
        let identity = module.provenance().source_identity();
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

fn validate_proof_return_project_sources(
    project: arcweft_lang_hir::proof_return::HirProofReturnHeaderProjectView<'_, '_>,
    facts: &super::model::ProjectRegistrationFacts,
    fallback: &SourceSpan,
) -> Vec<CharacterRegistrationDiagnostic> {
    let mut diagnostics = Vec::new();
    for (_, module) in project.modules() {
        let identity = module.source_identity();
        let primary = facts
            .document(identity.id())
            .map_or_else(|| fallback.clone(), full_span);
        let Some(document) = facts.document(identity.id()) else {
            diagnostics.push(CharacterRegistrationDiagnostic::new(
                CharacterRegistrationDiagnosticKind::WrongDocument {
                    expected: facts.world().root_document().clone(),
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
    facts: &super::model::ProjectRegistrationFacts,
    work: &mut u64,
    diagnostics: &mut Vec<CharacterRegistrationDiagnostic>,
    fallback: &SourceSpan,
) -> BTreeMap<CharacterId, ManifestRecord> {
    let catalog_count = u64::try_from(facts.catalogs().len()).unwrap_or(u64::MAX);
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
    for (catalog_index, catalog) in facts.catalogs().enumerate() {
        charge(work, 1, fallback, diagnostics);
        for (manifest_index, source_backed) in catalog.manifests().enumerate() {
            occurrences = occurrences.saturating_add(1);
            let manifest = source_backed.manifest();
            let owner = manifest.character().clone();
            let source = facts
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
                    facts.document(expected_source.id()).map(full_span),
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
    base: &Arc<TypeCheckEnv>,
    facts: &super::model::ProjectRegistrationFacts,
    link: &arcweft_lang_hir::symbol::ProjectSymbolLinkOutput,
    manifests: &BTreeMap<CharacterId, ManifestRecord>,
    work: &mut u64,
    diagnostics: &mut Vec<CharacterRegistrationDiagnostic>,
    fallback: &SourceSpan,
) -> BTreeMap<ExternalDeclarationId, RegisteredExternalOwner> {
    let mut owners = BTreeMap::new();
    for contribution in facts.external_owner_contributions() {
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
            RegisteredExternalOwner::Environment(owner) => {
                base.environment_binding(owner.value_binding()).is_some()
                    || facts.declares_environment_binding(owner.value_binding())
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
                        [symbol.declaration_span().clone()],
                    ));
                }
                Ok(ResolvedProjectSymbol::StructuralCallable(symbol)) => {
                    diagnostics.push(CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::AliasCollision {
                            spelling: path,
                            expected: *declaration,
                            conflicting: vec![ProjectSymbolTargetId::StructuralCallable(
                                symbol.declaration().clone(),
                            )],
                        },
                        record.primary_source().clone(),
                        [symbol.declaration_span().clone()],
                    ));
                }
                Ok(ResolvedProjectSymbol::Nominal(symbol)) => {
                    diagnostics.push(CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::AliasCollision {
                            spelling: path,
                            expected: *declaration,
                            conflicting: vec![ProjectSymbolTargetId::Nominal(symbol.id().clone())],
                        },
                        record.primary_source().clone(),
                        [symbol.source().whole().clone()],
                    ));
                }
                Ok(ResolvedProjectSymbol::Retained(symbol)) => {
                    diagnostics.push(CharacterRegistrationDiagnostic::new(
                        CharacterRegistrationDiagnosticKind::AliasCollision {
                            spelling: path,
                            expected: *declaration,
                            conflicting: vec![ProjectSymbolTargetId::Retained(
                                symbol.public_id().clone(),
                            )],
                        },
                        record.primary_source().clone(),
                        [symbol.declaration_span().clone()],
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
) -> BTreeMap<CharacterNominalType, Box<[String]>> {
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
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        variants.insert(
            CharacterNominalType::Part {
                character: owner.clone(),
            },
            manifest
                .parts()
                .iter()
                .map(|part| part.id().as_str().to_owned())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
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
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
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
        ProjectSymbolLinkError::DuplicateDeclaration { sites, .. } => sites.last().cloned(),
        ProjectSymbolLinkError::DuplicatePublicId { duplicate, .. } => Some(duplicate.clone()),
        ProjectSymbolLinkError::InaccessibleImport { source, .. }
        | ProjectSymbolLinkError::VisibilityEscalation { source, .. }
        | ProjectSymbolLinkError::AmbiguousImport { source, .. }
        | ProjectSymbolLinkError::InvalidImportPath { source, .. }
        | ProjectSymbolLinkError::InvalidDeclaration { source, .. }
        | ProjectSymbolLinkError::UnknownImport { source, .. }
        | ProjectSymbolLinkError::CyclicImport { source, .. }
        | ProjectSymbolLinkError::ReservedTypeName { source, .. }
        | ProjectSymbolLinkError::InvalidNominalDeclaration { source, .. } => Some(source.clone()),
        ProjectSymbolLinkError::Limit { source, .. }
        | ProjectSymbolLinkError::WorkOverflow { source, .. } => source.clone(),
    }
}

fn full_span(document: &arcweft_source::SourceDocument) -> SourceSpan {
    document
        .span(SourceRange::new(0, document.text().len()))
        .expect("complete document range is valid")
}
