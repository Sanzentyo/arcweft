//! Project-record inference and exact field checking.

use super::{
    Analyzer, BTreeSet, CheckedTypeSelection, ExprId, FinalSemanticAnalysisError,
    GenericTypeOwnerId, GenericTypeParameterId, HirRecordField, ProjectNominalBody,
    ProjectNominalDeclaration, ProjectNominalType, TypeKind, TypeParameterSubstitutions,
};

impl Analyzer<'_, '_, '_> {
    pub(super) fn check_project_record_fields(
        &mut self,
        owner: ExprId,
        declaration: &ProjectNominalDeclaration,
        authored: &[HirRecordField],
        expected: Option<&TypeKind>,
    ) -> Result<(TypeKind, CheckedTypeSelection), FinalSemanticAnalysisError> {
        let ProjectNominalBody::Struct { fields: declared } = declaration.body() else {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        };
        if authored.len() != declared.len() {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        }

        let expected_nominal = expected_project_record_nominal(owner, declaration, expected)?;
        let (parameter_ids, mut substitutions) =
            prepare_record_substitutions(owner, declaration, expected_nominal)?;

        let mut seen = BTreeSet::new();
        for field in authored {
            self.check_project_record_field(owner, field, declared, &mut substitutions, &mut seen)?;
        }
        if declared
            .iter()
            .any(|field| !seen.contains(field.name().as_str()))
        {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        }

        let arguments = if let Some(nominal) = expected_nominal {
            nominal.arguments().to_vec()
        } else {
            parameter_ids
                .into_iter()
                .map(|parameter| {
                    let generic = TypeKind::GenericParam(parameter);
                    let resolved = substitutions.apply(&generic);
                    (resolved != generic)
                        .then_some(resolved)
                        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        Ok((
            TypeKind::ProjectNominal(ProjectNominalType::new(declaration.id().clone(), arguments)),
            if expected_nominal.is_some() {
                CheckedTypeSelection::Expected
            } else {
                CheckedTypeSelection::Inferred
            },
        ))
    }

    fn check_project_record_field(
        &mut self,
        owner: ExprId,
        field: &HirRecordField,
        declared: &[arcweft_lang_hir::symbol::nominal::ProjectNominalField],
        substitutions: &mut TypeParameterSubstitutions,
        seen: &mut BTreeSet<String>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let name = field
            .name()
            .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
        if !seen.insert(name.as_str().to_owned()) {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        }
        let declared_field = declared
            .iter()
            .find(|candidate| candidate.name().as_str() == name.as_str())
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
        let declared_ty = self.types.get(&declared_field.ty()).cloned().ok_or(
            FinalSemanticAnalysisError::TypeResolutionFailed {
                owner: declared_field.ty(),
            },
        )?;
        let field_expected = substitutions.apply_resolved(&declared_ty);
        let actual = match field {
            HirRecordField::Explicit { value, .. } => self
                .check_expression(*value, field_expected.as_ref())?
                .ty()
                .clone(),
            HirRecordField::Shorthand { local, .. } => self
                .facts
                .locals()
                .get(local)
                .cloned()
                .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: *local })?,
            HirRecordField::Invalid { .. } => {
                return Err(FinalSemanticAnalysisError::RecoveredOwner);
            }
        };
        if !substitutions.observe(&declared_ty, &actual) {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        }
        let resolved_declared = substitutions
            .apply_resolved(&declared_ty)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
        if !resolved_declared.accepts(&actual) {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        }
        Ok(())
    }
}

fn expected_project_record_nominal<'a>(
    owner: ExprId,
    declaration: &ProjectNominalDeclaration,
    expected: Option<&'a TypeKind>,
) -> Result<Option<&'a ProjectNominalType>, FinalSemanticAnalysisError> {
    match expected {
        Some(TypeKind::ProjectNominal(nominal)) if nominal.declaration() == declaration.id() => {
            Ok(Some(nominal))
        }
        Some(_) => Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner }),
        None => Ok(None),
    }
}

fn prepare_record_substitutions(
    owner: ExprId,
    declaration: &ProjectNominalDeclaration,
    expected: Option<&ProjectNominalType>,
) -> Result<(Vec<GenericTypeParameterId>, TypeParameterSubstitutions), FinalSemanticAnalysisError> {
    if expected
        .is_some_and(|nominal| nominal.arguments().len() != declaration.type_parameters().len())
    {
        return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
    }
    let parameters = declaration
        .type_parameters()
        .iter()
        .map(|parameter| {
            GenericTypeParameterId::new(
                GenericTypeOwnerId::Nominal(declaration.id().clone()),
                parameter.ordinal(),
            )
        })
        .collect::<Vec<_>>();
    let mut substitutions = TypeParameterSubstitutions::default();
    if let Some(nominal) = expected {
        for (parameter, argument) in parameters.iter().zip(nominal.arguments()) {
            if !substitutions.observe(&TypeKind::GenericParam(parameter.clone()), argument) {
                return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
            }
        }
    }
    Ok((parameters, substitutions))
}
