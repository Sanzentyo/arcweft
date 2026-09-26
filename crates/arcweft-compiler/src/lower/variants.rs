use super::*;

/// Projects the already selected manifest look through its accepted enum owner.
/// The manifest-owned id selects the source ordinal; diagnostic spelling is not
/// used to resolve a Character or reconstruct the enum identity.
pub(super) fn runtime_stage_look(
    look: &arcweft_lang_sema::final_analysis::CheckedStageLook,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeResolvedVariant, RuntimeSemanticProjectionError> {
    let invalid = || RuntimeSemanticProjectionError::Type {
        reason: "checked Character look no longer matches its accepted variant owner".to_owned(),
    };
    let owner = analysis
        .accepted_closed_variant_owner(look.character_nominal())
        .filter(|owner| owner.ty() == TypeKind::character_look(look.character().clone()))
        .ok_or_else(invalid)?;
    let manifest = world
        .environment()
        .character_manifest(look.character())
        .ok_or_else(invalid)?;
    let ordinal = manifest
        .looks()
        .iter()
        .position(|candidate| candidate.id() == look.look_id())
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .ok_or_else(invalid)?;
    let selected = owner.case(ordinal).ok_or_else(invalid)?;
    if selected.diagnostic_name() != Some(look.look_id().as_str()) {
        return Err(invalid());
    }
    let identity = RuntimeSemanticTypeId::from_bytes(*look.character_nominal().as_bytes());
    RuntimeResolvedVariant::character(
        identity,
        RuntimeNominalTypeId::from_checked_digest(*identity.as_bytes()),
        analysis
            .project_runtime_nominal_graph(world, &owner.ty(), Default::default())
            .map_err(
                |source| RuntimeSemanticProjectionError::NominalSchemaProjection {
                    nominal: owner.ty().source_label(),
                    source: NominalSchemaProjectionError::SourceGraph(Box::new(source)),
                },
            )?,
        runtime_checked_variant_cases_under(owner, symbols, world, analysis, None)?,
        ordinal,
        look.look_id().as_str(),
    )
    .map_err(|error| runtime_variant_projection_error(&error))
}

pub(super) fn runtime_variant(
    variant: &CheckedVariantResolution,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<
    arcweft_runtime_plan::semantic_facts::RuntimeResolvedVariant,
    RuntimeSemanticProjectionError,
> {
    runtime_variant_under(variant, symbols, world, analysis, None)
}

pub(super) fn runtime_variant_under(
    variant: &CheckedVariantResolution,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    enclosing: Option<ProjectInstanceTypes<'_>>,
) -> Result<
    arcweft_runtime_plan::semantic_facts::RuntimeResolvedVariant,
    RuntimeSemanticProjectionError,
> {
    if let TypeKind::Named(type_name) = variant.owner().ty()
        && let Some(policy_owner) =
            CharacterDialoguePolicyTypeGraph::owner_for_language_type(&type_name)
    {
        let policy_types = Arc::clone(
            world
                .environment()
                .character_dialogue_roles()
                .policy_types(),
        );
        return RuntimeResolvedVariant::character_dialogue_policy(
            policy_types,
            policy_owner,
            variant.ordinal(),
            checked_variant_selected_name(variant)?,
        )
        .map_err(|error| runtime_variant_projection_error(&error));
    }
    let semantic_type = variant.owner().semantic_type();
    let projected = match variant.owner().kind() {
        CheckedVariantOwnerKind::Project { nominal } => {
            let nominal_type = variant.owner().ty();
            let closed = enclosing.map_or_else(
                || Ok(nominal_type.clone()),
                |solution| solution.instantiate_type(&nominal_type),
            )?;
            let semantic_type = closed.semantic_identity_digest()?;
            let projection = analysis
                .runtime_nominal_projection(semantic_type)
                .filter(|projection| {
                    projection.kind()
                        == arcweft_lang_sema::final_analysis::RuntimeProjectNominalKind::Variant
                })
                .ok_or_else(|| RuntimeSemanticProjectionError::NominalSchemaProjection {
                    nominal: nominal.declaration().qualified_name(),
                    source: NominalSchemaProjectionError::MissingCachedProjection { semantic_type },
                })?;
            if projection.variant_cases().len() != variant.owner().cases().len() {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason: "checked project variant case inventory is incomplete".to_owned(),
                });
            }
            let normalized = runtime_type(&closed, symbols, world, analysis)?;
            let RuntimeTypeShape::Nominal {
                nominal: runtime_nominal,
                arguments,
            } = normalized.shape()
            else {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason: "closed checked project variant is not a project nominal".to_owned(),
                });
            };
            RuntimeResolvedVariant::project(
                runtime_nominal.clone(),
                arguments.clone(),
                variant.ordinal(),
                checked_variant_selected_name(variant)?,
                runtime_checked_variant_cases_under(
                    variant.owner(),
                    symbols,
                    world,
                    analysis,
                    enclosing,
                )?,
            )
            .map_err(|error| runtime_variant_projection_error(&error))?
        }
        CheckedVariantOwnerKind::CharacterNominal { .. } => RuntimeResolvedVariant::character(
            RuntimeSemanticTypeId::from_bytes(*semantic_type.as_bytes()),
            RuntimeNominalTypeId::from_checked_digest(*semantic_type.as_bytes()),
            analysis
                .project_runtime_nominal_graph(world, &variant.owner().ty(), Default::default())
                .map_err(
                    |source| RuntimeSemanticProjectionError::NominalSchemaProjection {
                        nominal: variant.owner().ty().source_label(),
                        source: NominalSchemaProjectionError::SourceGraph(Box::new(source)),
                    },
                )?,
            runtime_checked_variant_cases_under(
                variant.owner(),
                symbols,
                world,
                analysis,
                enclosing,
            )?,
            variant.ordinal(),
            checked_variant_selected_name(variant)?,
        )
        .map_err(|error| runtime_variant_projection_error(&error))?,
        CheckedVariantOwnerKind::BuiltinClosed { nominal, .. } => {
            RuntimeResolvedVariant::builtin_closed(
                RuntimeSemanticTypeId::from_bytes(*semantic_type.as_bytes()),
                RuntimeNominalTypeId::try_new(nominal.as_str().to_owned()).map_err(|error| {
                    RuntimeSemanticProjectionError::Type {
                        reason: format!(
                            "checked base-environment enum identity is invalid: {error}"
                        ),
                    }
                })?,
                analysis
                    .project_runtime_nominal_graph(world, &variant.owner().ty(), Default::default())
                    .map_err(
                        |source| RuntimeSemanticProjectionError::NominalSchemaProjection {
                            nominal: variant.owner().ty().source_label(),
                            source: NominalSchemaProjectionError::SourceGraph(Box::new(source)),
                        },
                    )?,
                runtime_checked_variant_cases_under(
                    variant.owner(),
                    symbols,
                    world,
                    analysis,
                    enclosing,
                )?,
                variant.ordinal(),
                checked_variant_selected_name(variant)?,
            )
            .map_err(|error| runtime_variant_projection_error(&error))?
        }
        CheckedVariantOwnerKind::RuntimeBuiltin { owner, .. } => {
            RuntimeResolvedVariant::runtime_builtin(
                RuntimeSemanticTypeId::from_bytes(*semantic_type.as_bytes()),
                *owner,
                runtime_checked_variant_cases_under(
                    variant.owner(),
                    symbols,
                    world,
                    analysis,
                    enclosing,
                )?,
                variant.ordinal(),
                checked_variant_selected_name(variant)?,
            )
            .map_err(|error| runtime_variant_projection_error(&error))?
        }
        CheckedVariantOwnerKind::Option { .. } => {
            let owner_type = variant.owner().ty();
            let closed_owner = enclosing.map_or_else(
                || Ok(owner_type.clone()),
                |solution| solution.instantiate_type(&owner_type),
            )?;
            let TypeKind::Option(closed_item) = &closed_owner else {
                unreachable!("frozen substitution preserves Option")
            };
            let item = runtime_type(closed_item, symbols, world, analysis)?;
            let normalized_cases = runtime_checked_variant_cases_under(
                variant.owner(),
                symbols,
                world,
                analysis,
                enclosing,
            )?;
            RuntimeResolvedVariant::option(
                RuntimeSemanticTypeId::from_bytes(
                    *closed_owner.semantic_identity_digest()?.as_bytes(),
                ),
                item,
                normalized_cases,
                variant.ordinal(),
                checked_variant_selected_name(variant)?,
            )
            .map_err(|error| runtime_variant_projection_error(&error))?
        }
        CheckedVariantOwnerKind::Result { .. } => {
            let owner_type = variant.owner().ty();
            let closed_owner = enclosing.map_or_else(
                || Ok(owner_type.clone()),
                |solution| solution.instantiate_type(&owner_type),
            )?;
            let TypeKind::Result {
                ok: closed_ok,
                error: closed_error,
            } = &closed_owner
            else {
                unreachable!("frozen substitution preserves Result")
            };
            let ok = runtime_type(closed_ok, symbols, world, analysis)?;
            let error = runtime_type(closed_error, symbols, world, analysis)?;
            let normalized_cases = runtime_checked_variant_cases_under(
                variant.owner(),
                symbols,
                world,
                analysis,
                enclosing,
            )?;
            RuntimeResolvedVariant::result(
                RuntimeSemanticTypeId::from_bytes(
                    *closed_owner.semantic_identity_digest()?.as_bytes(),
                ),
                ok,
                error,
                normalized_cases,
                variant.ordinal(),
                checked_variant_selected_name(variant)?,
            )
            .map_err(|error| runtime_variant_projection_error(&error))?
        }
    };
    Ok(projected)
}

fn checked_variant_selected_name(
    variant: &CheckedVariantResolution,
) -> Result<&str, RuntimeSemanticProjectionError> {
    variant
        .selected()
        .diagnostic_name()
        .ok_or_else(|| RuntimeSemanticProjectionError::Type {
            reason: "checked variant case has no diagnostic name authority".to_owned(),
        })
}

pub(super) fn runtime_checked_variant_cases_under(
    owner: &CheckedVariantOwner,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    enclosing: Option<ProjectInstanceTypes<'_>>,
) -> Result<Box<[RuntimeNormalizedVariantCase]>, RuntimeSemanticProjectionError> {
    owner
        .cases()
        .iter()
        .map(|case| {
            let name =
                case.diagnostic_name()
                    .ok_or_else(|| RuntimeSemanticProjectionError::Type {
                        reason: "checked variant case has no diagnostic name authority".to_owned(),
                    })?;
            let payload = owner
                .case_payload_type(case.ordinal())
                .ok_or_else(|| RuntimeSemanticProjectionError::Type {
                    reason: "checked variant case has an invalid payload schema".to_owned(),
                })?
                .map(|payload| {
                    runtime_type_under(&payload, enclosing, symbols, world, analysis)
                        .and_then(retain_checked_variant_payload)
                })
                .transpose()?;
            Ok(RuntimeNormalizedVariantCase::new(name.to_owned(), payload))
        })
        .collect()
}

fn retain_checked_variant_payload(
    payload: RuntimeNormalizedType,
) -> Result<RuntimeNormalizedType, RuntimeSemanticProjectionError> {
    payload
        .checked_type()
        .map_err(|reason| RuntimeSemanticProjectionError::Type {
            reason: reason.to_string(),
        })?;
    Ok(payload)
}

fn runtime_variant_projection_error(
    error: &arcweft_runtime_plan::semantic_facts::RuntimeResolvedVariantError,
) -> RuntimeSemanticProjectionError {
    RuntimeSemanticProjectionError::Type {
        reason: error.to_string(),
    }
}
