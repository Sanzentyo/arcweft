//! Project declarations reached through the same instantiated nominal graph.

use std::collections::BTreeMap;

use super::{Error, NominalGraphProjection, Schema, TypeId, VisitState};
use crate::{
    final_analysis::nominal_schema::NominalSchemaProjectionError as ProjectError,
    types::{
        GenericParameterOwnerId, GenericTypeParameterId, ProjectNominalType, TypeKind,
        TypeProjectionError,
    },
};
use arcweft_core::{
    entry::{
        RuntimeNominalRecordShape, RuntimeNominalSchemaBody, RuntimeNominalSchemaCase,
        RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField, RuntimeNominalSchemaIdentity,
        RuntimeNominalTypeId,
    },
    value::RuntimeRecordFieldId,
};
use arcweft_lang_hir::symbol::nominal::ProjectNominalBody;

impl NominalGraphProjection<'_> {
    pub(super) fn project_nominal(
        &mut self,
        ty: &TypeKind,
        nominal: &ProjectNominalType,
        depth: u64,
    ) -> Result<Schema, Error> {
        self.budget.edge()?;
        let semantic = self.semantic_identity(ty)?;
        let id = nominal.declaration();
        if id.world() != self.symbols.world() || id.revision() != *self.symbols.revision() {
            return Err(ProjectError::GenerationMismatch.into());
        }
        let symbols = self.symbols;
        let declaration = symbols
            .nominal(id)
            .ok_or_else(|| ProjectError::MissingDeclaration {
                nominal: id.qualified_name(),
            })?;
        if declaration.type_parameters().len() != nominal.arguments().len() {
            return Err(ProjectError::WrongArity {
                nominal: id.qualified_name(),
                expected: declaration.type_parameters().len(),
                actual: nominal.arguments().len(),
            }
            .into());
        }
        if let Some((budget, control)) = &mut self.project_budget {
            budget.charge_generic_arguments(nominal.arguments().len(), *control)?;
        }
        if let Some(VisitState::Visiting(identity) | VisitState::Complete(identity)) =
            self.states.get(&semantic)
        {
            return Ok(Schema::NominalRef(identity.clone()));
        }
        self.budget.definition(self.active_depth + 1)?;
        match declaration.body() {
            ProjectNominalBody::Struct { fields } => {
                self.budget.members(fields.len())?;
                for field in fields {
                    self.budget.name(field.name().as_str())?;
                }
            }
            ProjectNominalBody::Enum { variants } => {
                self.budget.members(variants.len())?;
                for variant in variants {
                    self.budget.name(variant.name().as_str())?;
                }
            }
            ProjectNominalBody::TypeAlias { .. } => {
                return Err(ProjectError::UnsupportedDeclaration {
                    nominal: id.qualified_name(),
                }
                .into());
            }
        }
        let runtime = RuntimeNominalTypeId::from_checked_digest(*semantic.as_bytes());
        let identity = RuntimeNominalSchemaIdentity::new(runtime, semantic.into());
        self.states
            .insert(semantic, VisitState::Visiting(identity.clone()));
        self.active_depth += 1;
        let parameters = declaration
            .type_parameters()
            .iter()
            .map(|parameter| {
                GenericTypeParameterId::new(
                    GenericParameterOwnerId::Nominal(id.clone()),
                    parameter.ordinal(),
                )
            })
            .collect::<Vec<_>>();
        let substitutions = parameters
            .iter()
            .zip(nominal.arguments())
            .collect::<BTreeMap<_, _>>();
        let arguments = self.sequence(nominal.arguments(), depth + 1)?;
        let body = match declaration.body() {
            ProjectNominalBody::Struct { fields } => RuntimeNominalSchemaBody::Record {
                shape: RuntimeNominalRecordShape::Record,
                fields: fields
                    .iter()
                    .enumerate()
                    .map(|(ordinal, field)| {
                        let ty = self.project_declaration_type(field.ty(), &substitutions)?;
                        Ok(RuntimeNominalSchemaField::new(
                            RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                                .expect("fields were bounded"),
                            Some(field.name().as_str().to_owned()),
                            self.schema(&ty, depth + 1).map_err(|error| match error {
                                Error::Project(error) => Error::Project(error.within_step(
                                    super::super::NominalSchemaPathStep::Field {
                                        ordinal:
                                            u32::try_from(ordinal).expect("fields were bounded"),
                                        name: field.name().clone(),
                                    },
                                )),
                                other => other,
                            })?,
                        ))
                    })
                    .collect::<Result<_, Error>>()?,
            },
            ProjectNominalBody::Enum { variants } => RuntimeNominalSchemaBody::Variant {
                cases: {
                    let accepted =
                        self.project_nominals
                            .get(semantic)
                            .cloned()
                            .ok_or_else(|| {
                                ProjectError::new(
                                    "project enum has no accepted semantic definition",
                                )
                            })?;
                    let accepted_cases = accepted
                        .cases()
                        .filter(|cases| cases.len() == variants.len())
                        .ok_or_else(|| {
                            ProjectError::new("project enum semantic case inventory is incomplete")
                        })?;
                    variants
                    .iter()
                    .zip(accepted_cases)
                    .enumerate()
                    .map(|(ordinal, (variant, case))| {
                        let ordinal = u32::try_from(ordinal).expect("cases were bounded");
                        if case.ordinal() != ordinal
                            || case.diagnostic_name() != variant.name().as_str()
                        {
                            return Err(ProjectError::new(
                                "project enum case differs from its accepted semantic definition",
                            )
                            .into());
                        }
                        let payload = case
                            .payload_type(accepted.nominal())
                            .map_err(|_| {
                                ProjectError::new(
                                    "project enum payload cannot be projected from its accepted semantic case",
                                )
                            })?
                            .map(|ty| {
                                self.schema(&ty, depth + 1).map_err(|error| match error {
                                    Error::Project(error) => Error::Project(error.within_step(
                                        super::super::NominalSchemaPathStep::VariantPayload {
                                            ordinal,
                                            name: variant.name().clone(),
                                        },
                                    )),
                                    other => other,
                                })
                            })
                            .transpose()?;
                        Ok(RuntimeNominalSchemaCase::new(
                            ordinal,
                            variant.name().as_str().to_owned(),
                            payload,
                        ))
                    })
                    .collect::<Result<_, Error>>()?
                },
            },
            ProjectNominalBody::TypeAlias { .. } => {
                unreachable!("aliases were rejected before staging")
            }
        };
        self.definitions.push(RuntimeNominalSchemaDefinition::new(
            identity.clone(),
            arguments,
            body,
        ));
        self.active_depth -= 1;
        self.states
            .insert(semantic, VisitState::Complete(identity.clone()));
        Ok(Schema::NominalRef(identity))
    }

    fn project_declaration_type(
        &mut self,
        owner: TypeId,
        substitutions: &BTreeMap<&GenericTypeParameterId, &TypeKind>,
    ) -> Result<TypeKind, Error> {
        self.types
            .get(&owner)
            .ok_or(ProjectError::MissingTypeFact { ty: owner })?
            .instantiate_type_parameters_with_control(substitutions, &mut self.budget)
            .map_err(|error| match error {
                TypeProjectionError::Instantiation(error) => Error::Instantiation(error),
                TypeProjectionError::Control(error) => error,
            })
    }
}
