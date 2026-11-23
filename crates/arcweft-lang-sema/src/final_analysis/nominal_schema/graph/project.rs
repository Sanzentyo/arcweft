//! Source-ordered work for project declarations and their instantiated members.

use std::{collections::BTreeMap, rc::Rc};

use arcweft_core::entry::{
    RuntimeNominalRecordShape, RuntimeNominalSchemaIdentity, RuntimeNominalTypeId,
};
use arcweft_lang_hir::symbol::nominal::{
    ProjectNominalBody, ProjectNominalDeclaration, ProjectNominalField, ProjectNominalVariant,
};

use super::{
    Error, NominalGraphProjection, Schema, TypeId, VisitState,
    walk::{Body, Definition, Task, Walk},
};
use crate::{
    final_analysis::{
        nominal_schema::{NominalSchemaPathStep, NominalSchemaProjectionError as ProjectError},
        nominal_semantic::{ProjectNominalSemanticCase, ProjectNominalSemanticDefinition},
    },
    types::{
        GenericParameterOwnerId, GenericTypeParameterId, ProjectNominalType, TypeKind,
        TypeProjectionError,
    },
};

type Substitutions<'walk> = BTreeMap<&'walk GenericTypeParameterId, &'walk TypeKind>;

impl<'source> NominalGraphProjection<'source> {
    pub(super) fn project_nominal<'walk>(
        &mut self,
        walk: &mut Walk<'walk>,
        ty: &TypeKind,
        nominal: &'walk ProjectNominalType,
        depth: u64,
    ) -> Result<(), Error>
    where
        'source: 'walk,
    {
        self.budget.edge()?;
        let semantic = self.semantic_identity(ty)?;
        let id = nominal.declaration();
        if id.world() != self.symbols.world() || id.revision() != *self.symbols.revision() {
            return Err(ProjectError::GenerationMismatch.into());
        }
        let declaration =
            self.symbols
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
            walk.emit(Schema::NominalRef(identity.clone()));
            return Ok(());
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
        let identity = RuntimeNominalSchemaIdentity::new(
            RuntimeNominalTypeId::from_checked_digest(*semantic.as_bytes()),
            semantic.into(),
        );
        self.states
            .insert(semantic, VisitState::Visiting(identity.clone()));
        self.active_depth += 1;
        walk.push(Task::ProjectBody(ProjectBody {
            definition: Definition {
                semantic,
                identity,
                declaration: nominal.declaration_identity(),
                arguments: nominal.arguments().len(),
                rust: None,
            },
            declaration,
            nominal,
            depth: depth + 1,
        }));
        walk.push(Task::Types(nominal.arguments(), depth + 1));
        Ok(())
    }

    fn project_declaration_type(
        &mut self,
        owner: TypeId,
        substitutions: &Substitutions<'_>,
    ) -> Result<TypeKind, Error> {
        self.types
            .get(&owner)
            .ok_or(ProjectError::MissingTypeFact { ty: owner })?
            .instantiate_type_parameters_with_control(substitutions, &self.scope, &mut self.budget)
            .map_err(|error| match error {
                TypeProjectionError::Instantiation(error) => Error::Instantiation(error),
                TypeProjectionError::Control(error) => error,
            })
    }
}

pub(super) struct ProjectBody<'walk> {
    definition: Definition<'walk>,
    declaration: &'walk ProjectNominalDeclaration,
    nominal: &'walk ProjectNominalType,
    depth: u64,
}

impl<'walk> ProjectBody<'walk> {
    pub(super) fn schedule<'source: 'walk>(
        self,
        projection: &NominalGraphProjection<'source>,
        walk: &mut Walk<'walk>,
    ) -> Result<(), Error> {
        match self.declaration.body() {
            ProjectNominalBody::Struct { fields } => {
                let parameters: &[GenericTypeParameterId] = if self.nominal.arguments().is_empty() {
                    &[]
                } else {
                    walk.storage
                        .parameters
                        .insert(
                            self.declaration
                                .type_parameters()
                                .iter()
                                .map(|parameter| {
                                    GenericTypeParameterId::new(
                                        GenericParameterOwnerId::Nominal(
                                            self.nominal.declaration().clone(),
                                        ),
                                        parameter.ordinal(),
                                    )
                                })
                                .collect(),
                        )
                        .1
                };
                let substitutions =
                    Rc::new(parameters.iter().zip(self.nominal.arguments()).collect());
                walk.push(Task::FinishDefinition(
                    self.definition,
                    Body::Record(RuntimeNominalRecordShape::Record, fields.len()),
                ));
                for (ordinal, field) in fields.iter().enumerate().rev() {
                    walk.push(Task::ProjectField(ProjectField {
                        field,
                        ordinal,
                        substitutions: Rc::clone(&substitutions),
                        depth: self.depth,
                    }));
                }
            }
            ProjectNominalBody::Enum { variants } => {
                let accepted = projection
                    .project_nominals
                    .get(self.definition.semantic)
                    .ok_or_else(|| {
                        ProjectError::new("project enum has no accepted semantic definition")
                    })?;
                let cases = accepted
                    .cases()
                    .filter(|cases| cases.len() == variants.len())
                    .ok_or_else(|| {
                        ProjectError::new("project enum semantic case inventory is incomplete")
                    })?;
                walk.push(Task::FinishDefinition(
                    self.definition,
                    Body::Variant(cases.len()),
                ));
                for (ordinal, (variant, case)) in variants.iter().zip(cases).enumerate().rev() {
                    walk.push(Task::ProjectCase(ProjectCase {
                        accepted,
                        variant,
                        case,
                        ordinal: u32::try_from(ordinal).expect("cases were bounded"),
                        depth: self.depth,
                    }));
                }
            }
            ProjectNominalBody::TypeAlias { .. } => {
                unreachable!("aliases were rejected before staging")
            }
        }
        Ok(())
    }
}

pub(super) struct ProjectField<'walk> {
    field: &'walk ProjectNominalField,
    ordinal: usize,
    substitutions: Rc<Substitutions<'walk>>,
    depth: u64,
}

impl<'walk> ProjectField<'walk> {
    pub(super) fn visit<'source: 'walk>(
        self,
        projection: &mut NominalGraphProjection<'source>,
        walk: &mut Walk<'walk>,
    ) -> Result<(), Error> {
        let ty = projection.project_declaration_type(self.field.ty(), &self.substitutions)?;
        walk.push(Task::Field {
            ordinal: self.ordinal,
            name: Some(self.field.name().as_str()),
        });
        walk.visit_instantiated(
            projection,
            ty,
            self.depth,
            Some(NominalSchemaPathStep::Field {
                ordinal: u32::try_from(self.ordinal).expect("fields were bounded"),
                name: self.field.name().clone(),
            }),
        )
    }
}

pub(super) struct ProjectCase<'walk> {
    accepted: &'walk ProjectNominalSemanticDefinition,
    variant: &'walk ProjectNominalVariant,
    case: &'walk ProjectNominalSemanticCase,
    ordinal: u32,
    depth: u64,
}

impl<'walk> ProjectCase<'walk> {
    pub(super) fn visit<'source: 'walk>(
        self,
        projection: &mut NominalGraphProjection<'source>,
        walk: &mut Walk<'walk>,
    ) -> Result<(), Error> {
        let name = self.variant.name().as_str();
        if self.case.ordinal() != self.ordinal || self.case.diagnostic_name() != name {
            return Err(ProjectError::new(
                "project enum case differs from its accepted semantic definition",
            )
            .into());
        }
        let payload = self
            .case
            .payload_type(self.accepted.nominal())
            .map_err(|_| {
                ProjectError::new(
                    "project enum payload cannot be projected from its accepted semantic case",
                )
            })?;
        walk.push(Task::Case {
            ordinal: self.ordinal,
            name,
            has_payload: payload.is_some(),
        });
        if let Some(ty) = payload {
            walk.visit_instantiated(
                projection,
                ty,
                self.depth,
                Some(NominalSchemaPathStep::VariantPayload {
                    ordinal: self.ordinal,
                    name: self.variant.name().clone(),
                }),
            )?;
        }
        Ok(())
    }
}
