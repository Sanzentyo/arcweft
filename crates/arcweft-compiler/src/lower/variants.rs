use super::*;

pub(super) fn runtime_variant(
    variant: &CheckedVariantResolution,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<
    arcweft_runtime_plan::semantic_facts::RuntimeResolvedVariant,
    RuntimeSemanticProjectionError,
> {
    let projected = match variant.owner() {
        CheckedVariantOwner::Project {
            nominal,
            semantic_type,
            cases,
        } => {
            if nominal.identity() != *semantic_type {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason: "checked project variant semantic identity is inconsistent".to_owned(),
                });
            }
            let projection = analysis
                .runtime_nominal_projection(*semantic_type)
                .filter(|projection| {
                    projection.kind()
                        == arcweft_lang_sema::final_analysis::RuntimeProjectNominalKind::Variant
                })
                .ok_or_else(|| RuntimeSemanticProjectionError::NominalSchemaProjection {
                    nominal: nominal.declaration().qualified_name(),
                    source: NominalSchemaProjectionError::MissingCachedProjection {
                        semantic_type: *semantic_type,
                    },
                })?;
            if projection.variant_cases().len() != cases.len() {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason: "checked project variant case inventory is incomplete".to_owned(),
                });
            }
            let runtime_nominal = runtime_nominal(nominal, analysis)?;
            RuntimeResolvedVariant::project(
                runtime_nominal,
                nominal
                    .arguments()
                    .iter()
                    .map(|argument| runtime_type(argument, symbols, world, analysis))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
                variant.ordinal(),
                checked_variant_selected_name(variant)?,
                runtime_checked_variant_cases(variant.owner(), symbols, world, analysis)?,
            )
            .map_err(|error| runtime_variant_projection_error(&error))?
        }
        CheckedVariantOwner::CharacterNominal {
            nominal,
            semantic_type,
            ..
        } => {
            if TypeKind::CharacterNominal(nominal.clone()).semantic_identity_digest()
                != *semantic_type
            {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason: "checked character variant semantic identity is inconsistent"
                        .to_owned(),
                });
            }
            RuntimeResolvedVariant::character(
                RuntimeSemanticTypeId::from_bytes(*semantic_type.as_bytes()),
                RuntimeNominalTypeId::from_checked_digest(*semantic_type.as_bytes()),
                runtime_checked_variant_cases(variant.owner(), symbols, world, analysis)?,
                variant.ordinal(),
                checked_variant_selected_name(variant)?,
            )
            .map_err(|error| runtime_variant_projection_error(&error))?
        }
        CheckedVariantOwner::BuiltinClosed {
            nominal,
            semantic_type,
            ..
        } => RuntimeResolvedVariant::builtin_closed(
            RuntimeSemanticTypeId::from_bytes(*semantic_type.as_bytes()),
            RuntimeNominalTypeId::try_new(nominal.as_str().to_owned()).map_err(|error| {
                RuntimeSemanticProjectionError::Type {
                    reason: format!("checked base-environment enum identity is invalid: {error}"),
                }
            })?,
            runtime_checked_variant_cases(variant.owner(), symbols, world, analysis)?,
            variant.ordinal(),
            checked_variant_selected_name(variant)?,
        )
        .map_err(|error| runtime_variant_projection_error(&error))?,
        CheckedVariantOwner::RuntimeBuiltin {
            owner,
            semantic_type,
            ..
        } => RuntimeResolvedVariant::runtime_builtin(
            RuntimeSemanticTypeId::from_bytes(*semantic_type.as_bytes()),
            *owner,
            runtime_checked_variant_cases(variant.owner(), symbols, world, analysis)?,
            variant.ordinal(),
            checked_variant_selected_name(variant)?,
        )
        .map_err(|error| runtime_variant_projection_error(&error))?,
        CheckedVariantOwner::Option { item, .. } => {
            let item = runtime_type(item, symbols, world, analysis)?;
            let normalized_cases =
                runtime_checked_variant_cases(variant.owner(), symbols, world, analysis)?;
            RuntimeResolvedVariant::option(
                RuntimeSemanticTypeId::from_bytes(*variant.owner().semantic_type().as_bytes()),
                item,
                normalized_cases,
                variant.ordinal(),
                checked_variant_selected_name(variant)?,
            )
            .map_err(|error| runtime_variant_projection_error(&error))?
        }
        CheckedVariantOwner::Result { ok, error, .. } => {
            let ok = runtime_type(ok, symbols, world, analysis)?;
            let error = runtime_type(error, symbols, world, analysis)?;
            let normalized_cases =
                runtime_checked_variant_cases(variant.owner(), symbols, world, analysis)?;
            RuntimeResolvedVariant::result(
                RuntimeSemanticTypeId::from_bytes(*variant.owner().semantic_type().as_bytes()),
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

fn runtime_checked_variant_cases(
    owner: &CheckedVariantOwner,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
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
                    runtime_type(&payload, symbols, world, analysis)
                        .and_then(retain_checked_variant_payload)
                })
                .transpose()?;
            Ok(RuntimeNormalizedVariantCase::new(name.to_owned(), payload))
        })
        .collect()
}

#[expect(
    clippy::too_many_lines,
    reason = "constructor instantiations form a closed semantic validation matrix"
)]
pub(super) fn runtime_variant_constructor(
    owner: ExprId,
    application: &CheckedCallApplication,
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<Option<RuntimeResolvedVariant>, RuntimeSemanticProjectionError> {
    let invalid = |reason: &str| RuntimeSemanticProjectionError::Call {
        owner,
        reason: reason.to_owned(),
    };
    let selected = application.core().candidates().selected();
    match selected.instantiation() {
        ResolvedCallableBaseInstantiation::Result { kind } => {
            let TypeKind::Result { ok, error } = application.result().ty() else {
                return Err(invalid(
                    "Result constructor did not retain its exact checked Result type",
                ));
            };
            let checked_owner =
                CheckedVariantOwner::result(ok.as_ref().clone(), error.as_ref().clone());
            let ok = runtime_type(ok, symbols, world, analysis)?;
            let error = runtime_type(error, symbols, world, analysis)?;
            let (ordinal, name) = match kind {
                arcweft_lang_sema::callable::ResultConstructorKind::Ok => (0, "Ok"),
                arcweft_lang_sema::callable::ResultConstructorKind::Err => (1, "Err"),
            };
            RuntimeResolvedVariant::result(
                RuntimeSemanticTypeId::from_bytes(
                    *application
                        .result()
                        .ty()
                        .semantic_identity_digest()
                        .as_bytes(),
                ),
                ok,
                error,
                runtime_checked_variant_cases(&checked_owner, symbols, world, analysis)?,
                ordinal,
                name,
            )
            .map(Some)
            .map_err(|error| invalid(&error.to_string()))
        }
        ResolvedCallableBaseInstantiation::Option => {
            let TypeKind::Option(item) = application.result().ty() else {
                return Err(invalid(
                    "Option constructor did not retain its exact checked Option type",
                ));
            };
            if !matches!(
                selected.id(),
                arcweft_lang_sema::callable::CallableCandidateId::Option(
                    arcweft_lang_sema::callable::OptionConstructorKind::Some
                )
            ) {
                return Err(invalid(
                    "Option constructor instantiation has a non-Option candidate identity",
                ));
            }
            let owner = CheckedVariantOwner::option(item.as_ref().clone());
            RuntimeResolvedVariant::option(
                RuntimeSemanticTypeId::from_bytes(
                    *application
                        .result()
                        .ty()
                        .semantic_identity_digest()
                        .as_bytes(),
                ),
                runtime_type(item, symbols, world, analysis)?,
                runtime_checked_variant_cases(&owner, symbols, world, analysis)?,
                0,
                "Some",
            )
            .map(Some)
            .map_err(|error| invalid(&error.to_string()))
        }
        ResolvedCallableBaseInstantiation::ExpectedEnum { expected } => {
            if application.result().ty() != expected {
                return Err(invalid(
                    "enum constructor result differs from its checked enum type",
                ));
            }
            let arcweft_lang_sema::callable::CallableCandidateId::EnumVariant(candidate) =
                selected.id()
            else {
                return Err(invalid(
                    "enum constructor instantiation has a non-enum candidate identity",
                ));
            };
            let module = project
                .modules()
                .find_map(|(_, module)| {
                    (module.module_id() == owner.module()).then_some(module.as_ref())
                })
                .ok_or_else(|| invalid("enum constructor call module is unavailable"))?;
            let expression = module
                .resolve_expr(owner)
                .map_err(|_| invalid("enum constructor call expression is unavailable"))?;
            let HirExprKind::Call(call) = expression.kind() else {
                return Err(invalid("enum constructor owner is not a final-HIR Call"));
            };
            let HirCallCallee::Value { value } = call.callee() else {
                return Err(invalid("enum constructor has no value callee expression"));
            };
            let CheckedExpressionResolution::Variant(variant) = analysis
                .expression(*value)
                .ok_or_else(|| invalid("enum constructor callee has no final expression fact"))?
                .resolution()
            else {
                return Err(invalid(
                    "enum constructor callee has no checked variant authority",
                ));
            };
            if candidate.owner() != variant.owner().semantic_type()
                || candidate.case() != variant.ordinal()
                || expected.semantic_identity_digest() != variant.owner().semantic_type()
            {
                return Err(invalid(
                    "enum constructor candidate differs from its checked variant authority",
                ));
            }
            runtime_variant(variant, symbols, world, analysis).map(Some)
        }
        ResolvedCallableBaseInstantiation::None
        | ResolvedCallableBaseInstantiation::Character { .. }
        | ResolvedCallableBaseInstantiation::Receiver { .. }
        | ResolvedCallableBaseInstantiation::TypeReceiver { .. }
        | ResolvedCallableBaseInstantiation::Extension { .. } => Ok(None),
    }
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
