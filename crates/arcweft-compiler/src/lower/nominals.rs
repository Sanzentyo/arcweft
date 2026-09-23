//! Executable definitions projected from their original accepted source types.

use super::*;
use arcweft_lang_sema::env::nominal::{
    AcceptedEnvironmentRecord, AcceptedNominalOrigin, AcceptedNominalOwnerId,
};
use arcweft_lang_sema::{
    env::rust_metadata::{AcceptedRustStructShape, AcceptedRustTypeMetadataKind},
    final_analysis::RuntimeAcceptedRustNominalProjection,
    types::AcceptedNominalType,
};
use arcweft_runtime_plan::semantic_facts::{
    RuntimeNominalDefinition, RuntimeResolvedNominalSource,
};

pub(super) fn accepted_type(
    nominal: &AcceptedNominalType,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    path: &RuntimeTypeProjectionPath,
) -> Result<RuntimeTypeShape, RuntimeSemanticProjectionError> {
    let record = world
        .environment()
        .nominal_catalog()
        .exact(nominal.declaration().canonical_path())
        .filter(|record| {
            record.id() == nominal.declaration()
                && usize::from(record.arity()) == nominal.arguments().len()
        })
        .ok_or_else(|| RuntimeSemanticProjectionError::Type {
            reason: "accepted nominal runtime carrier is absent or stale".to_owned(),
        })?;
    let rust = matches!(record.semantics(), AcceptedNominalSemantics::RustAdt);
    let arguments = nominal
        .arguments()
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            let step = if rust {
                RuntimeTypeProjectionStep::NominalArgument(projection_index(index))
            } else {
                RuntimeTypeProjectionStep::OpaqueArgument(projection_index(index))
            };
            runtime_type_at(argument, symbols, world, analysis, &path.pushed(step))
        })
        .collect::<Result<Box<[_]>, _>>()?;
    match record.semantics() {
        AcceptedNominalSemantics::RustAdt => {
            let projection = analysis
                .project_accepted_rust_nominal(world, nominal, Default::default())
                .map_err(
                    |source| RuntimeSemanticProjectionError::NominalSchemaProjection {
                        nominal: nominal.declaration().canonical_path().to_string(),
                        source: NominalSchemaProjectionError::SourceGraph(Box::new(source)),
                    },
                )?;
            Ok(RuntimeTypeShape::Nominal {
                nominal: RuntimeResolvedNominal::accepted_rust(projection),
                arguments,
            })
        }
        AcceptedNominalSemantics::Opaque(carrier) => Ok(RuntimeTypeShape::Opaque {
            producer: carrier.producer().clone(),
            admission: arcweft_core::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: carrier.value_class(),
            persistence: carrier.persistence(),
            arguments,
        }),
        _ => Err(RuntimeSemanticProjectionError::Type {
            reason: "accepted nominal has no executable carrier".to_owned(),
        }),
    }
}

pub(super) fn environment_enum_type(
    ty: &TypeKind,
    identity: RuntimeSemanticTypeId,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeTypeShape, RuntimeSemanticProjectionError> {
    let semantic_type = ty.semantic_identity_digest()?;
    let owner = analysis
        .accepted_closed_variant_owner(semantic_type)
        .filter(|owner| owner.ty() == *ty)
        .ok_or_else(|| RuntimeSemanticProjectionError::Type {
            reason: format!(
                "closed environment enum `{}` has no exact accepted variant owner",
                ty.source_label()
            ),
        })?;
    let arcweft_lang_sema::final_analysis::CheckedVariantOwnerKind::BuiltinClosed {
        nominal: owner_id,
        ..
    } = owner.kind()
    else {
        return Err(RuntimeSemanticProjectionError::Type {
            reason: format!(
                "closed environment enum `{}` is not owned by a closed environment binding",
                ty.source_label()
            ),
        });
    };
    let source_graph = analysis
        .project_runtime_nominal_graph(world, ty, Default::default())
        .map_err(
            |source| RuntimeSemanticProjectionError::NominalSchemaProjection {
                nominal: owner_id.as_str().to_owned(),
                source: NominalSchemaProjectionError::SourceGraph(Box::new(source)),
            },
        )?;
    let layout = source_graph.try_layout_hash(identity).map_err(|source| {
        RuntimeSemanticProjectionError::NominalSchemaProjection {
            nominal: owner_id.as_str().to_owned(),
            source: NominalSchemaProjectionError::InvalidRuntimeSchema {
                nominal: owner_id.as_str().to_owned(),
                reason: source.to_string(),
            },
        }
    })?;
    let runtime_nominal =
        RuntimeNominalTypeId::try_new(owner_id.as_str().to_owned()).map_err(|source| {
            RuntimeSemanticProjectionError::Type {
                reason: format!("checked environment enum identity is invalid: {source}"),
            }
        })?;
    Ok(RuntimeTypeShape::Nominal {
        nominal: RuntimeResolvedNominal::builtin_closed(
            owner_id.clone(),
            owner.clone(),
            runtime_nominal,
            identity,
            layout,
            source_graph,
        ),
        arguments: Box::new([]),
    })
}

pub(super) fn environment_record_type(
    ty: &TypeKind,
    identity: RuntimeSemanticTypeId,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeTypeShape, RuntimeSemanticProjectionError> {
    let semantic_type = ty.semantic_identity_digest()?;
    let owner = world
        .environment()
        .nominal_catalog()
        .environment_record_for_semantic_type(semantic_type)
        .filter(|record| {
            record.environment_record().is_some_and(|semantics| {
                semantics.ty() == ty && semantics.semantic_type() == semantic_type
            }) && record.origin() == AcceptedNominalOrigin::Domain
                && record.id().owner() == &AcceptedNominalOwnerId::Standard
        })
        .ok_or_else(|| RuntimeSemanticProjectionError::Type {
            reason: format!(
                "standard environment record `{}` has no exact accepted owner",
                ty.source_label()
            ),
        })?;
    let source_graph = analysis
        .project_runtime_nominal_graph(world, ty, Default::default())
        .map_err(
            |source| RuntimeSemanticProjectionError::NominalSchemaProjection {
                nominal: owner.id().source_label(),
                source: NominalSchemaProjectionError::SourceGraph(Box::new(source)),
            },
        )?;
    let layout = source_graph.try_layout_hash(identity).map_err(|source| {
        RuntimeSemanticProjectionError::NominalSchemaProjection {
            nominal: owner.id().source_label(),
            source: NominalSchemaProjectionError::InvalidRuntimeSchema {
                nominal: owner.id().source_label(),
                reason: source.to_string(),
            },
        }
    })?;
    let runtime_nominal =
        RuntimeNominalTypeId::try_new(owner.id().source_label()).map_err(|source| {
            RuntimeSemanticProjectionError::Type {
                reason: format!("checked standard record identity is invalid: {source}"),
            }
        })?;
    Ok(RuntimeTypeShape::Nominal {
        nominal: RuntimeResolvedNominal::builtin_record(
            owner.id().clone(),
            runtime_nominal,
            identity,
            layout,
            source_graph,
        ),
        arguments: Box::new([]),
    })
}

pub(super) fn environment_pattern_record(
    accepted: &AcceptedEnvironmentRecord,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeResolvedNominalRecord, RuntimeSemanticProjectionError> {
    let normalized = runtime_type(accepted.semantics().ty(), symbols, world, analysis)?;
    let RuntimeNominalDefinition::Record(record) =
        definition(&normalized, symbols, world, analysis)?
    else {
        return Err(RuntimeSemanticProjectionError::Type {
            reason: format!(
                "accepted environment record `{}` has no executable record definition",
                accepted.nominal().source_label()
            ),
        });
    };
    let RuntimeResolvedNominalSource::BuiltinRecord { owner } = record.nominal().source() else {
        return Err(RuntimeSemanticProjectionError::Type {
            reason: format!(
                "accepted environment record `{}` resolved to a foreign nominal owner",
                accepted.nominal().source_label()
            ),
        });
    };
    if owner != accepted.nominal()
        || record.nominal().identity()
            != RuntimeSemanticTypeId::from_bytes(*accepted.semantic_type().as_bytes())
    {
        return Err(RuntimeSemanticProjectionError::Type {
            reason: format!(
                "accepted environment record `{}` changed after record-pattern checking",
                accepted.nominal().source_label()
            ),
        });
    }
    Ok(record)
}

pub(super) fn definition(
    ty: &RuntimeNormalizedType,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeNominalDefinition, RuntimeSemanticProjectionError> {
    let RuntimeTypeShape::Nominal { nominal, arguments } = ty.shape() else {
        return Err(arcweft_runtime_plan::semantic_facts::RuntimeSemanticFactsError::NominalDefinitionMismatch {
            identity: ty.identity(),
        }.into());
    };
    match nominal.source() {
        RuntimeResolvedNominalSource::BuiltinClosed { proof, .. } => {
            let semantic_type = SemanticTypeDigest::from_bytes(*ty.identity().as_bytes());
            if proof.semantic_type() != semantic_type {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason: "accepted closed-enum proof changed after nominal projection"
                        .to_owned(),
                });
            }
            let cases = super::variants::runtime_checked_variant_cases_under(
                proof, symbols, world, analysis, None,
            )?;
            Ok(RuntimeNominalDefinition::nominal_variant(ty, cases)?)
        }
        RuntimeResolvedNominalSource::BuiltinRecord { owner } => {
            let record = world
                .environment()
                .nominal_catalog()
                .exact(owner.canonical_path())
                .filter(|record| {
                    record.id() == owner
                        && record.origin() == AcceptedNominalOrigin::Domain
                        && matches!(record.id().owner(), AcceptedNominalOwnerId::Standard)
                })
                .and_then(|record| record.environment_record())
                .filter(|record| {
                    record.ty().semantic_identity_digest().ok()
                        == Some(SemanticTypeDigest::from_bytes(*ty.identity().as_bytes()))
                })
                .ok_or_else(|| RuntimeSemanticProjectionError::Type {
                    reason: format!(
                        "standard environment record `{}` changed after its nominal projection",
                        owner.source_label()
                    ),
                })?;
            let fields = record
                .fields()
                .iter()
                .enumerate()
                .map(|(ordinal, field)| {
                    let expected_ordinal = u32::try_from(ordinal).map_err(|_| {
                        RuntimeSemanticProjectionError::Type {
                            reason: format!(
                                "standard environment record `{}` exceeds runtime field ordinals",
                                owner.source_label()
                            ),
                        }
                    })?;
                    if field.ordinal() != expected_ordinal {
                        return Err(RuntimeSemanticProjectionError::Type {
                            reason: format!(
                                "standard environment record `{}` has a non-canonical field order",
                                owner.source_label()
                            ),
                        });
                    }
                    let field_id = RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                        .map_err(|source| RuntimeSemanticProjectionError::Type {
                            reason: format!(
                                "standard environment record `{}` has an invalid field ordinal: {source}",
                                owner.source_label()
                            ),
                        })?;
                    let normalized = runtime_type(field.ty(), symbols, world, analysis)?;
                    let checked = normalized.checked_type().map_err(|source| {
                        RuntimeSemanticProjectionError::Type {
                            reason: source.to_string(),
                        }
                    })?;
                    Ok((
                        Some(field.diagnostic_name().to_owned()),
                        normalized,
                        RuntimeNominalRecordLayoutField::new(
                            field_id,
                            Some(field.diagnostic_name().to_owned()),
                            checked,
                        ),
                    ))
                })
                .collect::<Result<Vec<_>, RuntimeSemanticProjectionError>>()?;
            let layout = RuntimeNominalRecordLayout::try_from_checked_projection(
                nominal.runtime_nominal_id(),
                nominal.identity(),
                nominal.layout(),
                RuntimeNominalRecordShape::Record,
                Vec::new(),
                fields.iter().map(|(_, _, field)| field.clone()).collect(),
            )
            .map(Arc::new)
            .map_err(
                |source| RuntimeSemanticProjectionError::NominalRecordLayout {
                    nominal: owner.source_label(),
                    source,
                },
            )?;
            let record = RuntimeResolvedNominalRecord::try_new(
                nominal.clone(),
                layout,
                fields
                    .into_iter()
                    .map(|(name, normalized, _)| (name, normalized)),
            )
            .map_err(|source| RuntimeSemanticProjectionError::NominalRecordFact {
                nominal: owner.source_label(),
                source,
            })?;
            Ok(RuntimeNominalDefinition::Record(record))
        }
        RuntimeResolvedNominalSource::AcceptedRust(projection) => {
            projection.validate_for(analysis, world).map_err(|source| {
                RuntimeSemanticProjectionError::NominalSchemaProjection {
                    nominal: projection
                        .nominal_type()
                        .declaration()
                        .canonical_path()
                        .to_string(),
                    source: NominalSchemaProjectionError::SourceGraph(Box::new(source)),
                }
            })?;
            match projection.metadata().kind() {
                AcceptedRustTypeMetadataKind::Enum { variants } => {
                    let cases = variants
                        .iter()
                        .enumerate()
                        .map(|(ordinal, variant)| {
                            let ordinal = projection_index(ordinal);
                            let payload = projection
                                .case_payload_type(ordinal)
                                .map_err(|source| {
                                    RuntimeSemanticProjectionError::RustVariantPayload {
                                        ordinal,
                                        source,
                                    }
                                })?
                                .as_ref()
                                .map(|payload| runtime_type(payload, symbols, world, analysis))
                                .transpose()?;
                            Ok(RuntimeNormalizedVariantCase::new(
                                variant.name().to_owned(),
                                payload,
                            ))
                        })
                        .collect::<Result<Box<[_]>, RuntimeSemanticProjectionError>>()?;
                    Ok(RuntimeNominalDefinition::nominal_variant(ty, cases)?)
                }
                _ => rust_record(nominal, arguments, projection, symbols, world, analysis)
                    .map(RuntimeNominalDefinition::Record),
            }
        }
        RuntimeResolvedNominalSource::Project { .. } => {
            let semantic_type = SemanticTypeDigest::from_bytes(*ty.identity().as_bytes());
            let projection = analysis
                .runtime_nominal_projection(semantic_type)
                .ok_or_else(|| RuntimeSemanticProjectionError::NominalSchemaProjection {
                    nominal: format!("{:?}", ty.identity()),
                    source: NominalSchemaProjectionError::MissingCachedProjection { semantic_type },
                })?;
            match projection.kind() {
                RuntimeProjectNominalKind::Record => {
                    runtime_nominal_record(projection.checked(), symbols, world, analysis)
                        .map(RuntimeNominalDefinition::Record)
                }
                RuntimeProjectNominalKind::Variant => {
                    let owner =
                        analysis
                            .project_variant_owner(semantic_type)?
                            .ok_or_else(|| RuntimeSemanticProjectionError::Type {
                                reason: "source nominal variant definition is missing".to_owned(),
                            })?;
                    Ok(RuntimeNominalDefinition::nominal_variant(
                        ty,
                        variants::runtime_checked_variant_cases_under(
                            &owner, symbols, world, analysis, None,
                        )?,
                    )?)
                }
            }
        }
    }
}

fn rust_record(
    nominal: &RuntimeResolvedNominal,
    arguments: &[RuntimeNormalizedType],
    projection: &RuntimeAcceptedRustNominalProjection,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeResolvedNominalRecord, RuntimeSemanticProjectionError> {
    use RuntimeNominalRecordShape as Shape;
    let (shape, fields): (_, Vec<(Option<&str>, &TypeKind)>) = match projection.metadata().kind() {
        AcceptedRustTypeMetadataKind::Struct {
            shape: AcceptedRustStructShape::Unit,
        } => (Shape::Unit, vec![]),
        AcceptedRustTypeMetadataKind::Struct {
            shape: AcceptedRustStructShape::Tuple(fields),
        } => (Shape::Tuple, fields.iter().map(|ty| (None, ty)).collect()),
        AcceptedRustTypeMetadataKind::Struct {
            shape: AcceptedRustStructShape::Record(fields),
        } => (
            Shape::Record,
            fields
                .iter()
                .map(|field| (Some(field.name()), field.ty()))
                .collect(),
        ),
        AcceptedRustTypeMetadataKind::Newtype { inner } => (Shape::Newtype, vec![(None, inner)]),
        AcceptedRustTypeMetadataKind::Enum { .. } => {
            unreachable!("variant definitions use the case projection")
        }
    };
    let fields = fields
        .into_iter()
        .map(|(name, ty)| {
            runtime_type(ty, symbols, world, analysis).map(|ty| (name.map(str::to_owned), ty))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let checked_fields = fields
        .iter()
        .enumerate()
        .map(|(ordinal, (name, ty))| {
            Ok(RuntimeNominalRecordLayoutField::new(
                RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                    .expect("source fields are bounded"),
                name.clone(),
                ty.checked_type()?,
            ))
        })
        .collect::<Result<Vec<_>, RuntimeCheckedTypeProjectionError>>()?;
    let checked_arguments = arguments
        .iter()
        .map(RuntimeNormalizedType::checked_type)
        .collect::<Result<Vec<_>, _>>()?;
    let name = projection
        .nominal_type()
        .declaration()
        .canonical_path()
        .to_string();
    let layout = RuntimeNominalRecordLayout::try_from_checked_projection(
        nominal.runtime_nominal_id(),
        nominal.identity(),
        nominal.layout(),
        shape,
        checked_arguments,
        checked_fields,
    )
    .map_err(
        |source| RuntimeSemanticProjectionError::NominalRecordLayout {
            nominal: name.clone(),
            source,
        },
    )?;
    RuntimeResolvedNominalRecord::try_new(nominal.clone(), Arc::new(layout), fields).map_err(
        |source| RuntimeSemanticProjectionError::NominalRecordFact {
            nominal: name,
            source,
        },
    )
}
