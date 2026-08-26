//! Project-record inference and exact field checking.

use super::super::expression_error::{AnalyzerExpressionContext, AnalyzerExpressionError};
use super::{
    Analyzer, AnalyzerExpressionExpectation, BTreeSet, CheckedTypeSelection, ExprId,
    FinalSemanticAnalysisError, GenericParameterOwnerId, GenericTypeParameterId, HirRecordField,
    ProjectNominalBody, ProjectNominalDeclaration, ProjectNominalType, TypeKind,
    TypeParameterSubstitutions,
};
use crate::final_analysis::{PreparedProjectRecordExpressionField, PreparedRecordValueSource};

impl Analyzer<'_, '_, '_> {
    pub(super) fn check_project_record_fields(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owner: ExprId,
        declaration: &ProjectNominalDeclaration,
        authored: &[HirRecordField],
        expectation: &AnalyzerExpressionExpectation<'_>,
    ) -> Result<
        (
            TypeKind,
            CheckedTypeSelection,
            Box<[PreparedProjectRecordExpressionField]>,
        ),
        AnalyzerExpressionError,
    > {
        let ProjectNominalBody::Struct { fields: declared } = declaration.body() else {
            return Err(AnalyzerExpressionError::rejected(owner));
        };
        if authored.len() != declared.len() {
            return Err(AnalyzerExpressionError::rejected(owner));
        }

        let expected_nominal = expected_project_record_nominal(owner, declaration, expectation)?;
        let (parameter_ids, mut substitutions) =
            prepare_record_substitutions(owner, declaration, expectation, expected_nominal)?;

        let mut seen = BTreeSet::new();
        let mut fields = Vec::with_capacity(authored.len());
        for (source_ordinal, field) in authored.iter().enumerate() {
            let source_ordinal = u32::try_from(source_ordinal).map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::AccountingOverflow)
            })?;
            let (declaration_ordinal, field_type, source) = self.check_project_record_field(
                context,
                owner,
                field,
                declared,
                &mut substitutions,
                &mut seen,
            )?;
            fields.push(PreparedProjectRecordExpressionField::new(
                source_ordinal,
                declaration_ordinal,
                field_type,
                source,
            ));
        }
        if declared
            .iter()
            .any(|field| !seen.contains(field.name().as_str()))
        {
            return Err(AnalyzerExpressionError::rejected(owner));
        }

        let arguments = if let ExpectedProjectRecordNominal::Complete(nominal) = expected_nominal {
            nominal.arguments().to_vec()
        } else {
            parameter_ids
                .into_iter()
                .map(|parameter| {
                    let generic = TypeKind::GenericParam(parameter);
                    let resolved = substitutions.apply(&generic);
                    (resolved != generic)
                        .then_some(resolved)
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        Ok((
            TypeKind::ProjectNominal(ProjectNominalType::new(declaration.id().clone(), arguments)),
            if matches!(expected_nominal, ExpectedProjectRecordNominal::Complete(_)) {
                CheckedTypeSelection::Expected
            } else {
                CheckedTypeSelection::Inferred
            },
            fields.into_boxed_slice(),
        ))
    }

    fn check_project_record_field(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owner: ExprId,
        field: &HirRecordField,
        declared: &[arcweft_lang_hir::symbol::nominal::ProjectNominalField],
        substitutions: &mut TypeParameterSubstitutions,
        seen: &mut BTreeSet<String>,
    ) -> Result<(u32, TypeKind, PreparedRecordValueSource), AnalyzerExpressionError> {
        let name = field.name().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RecoveredOwner)
        })?;
        if !seen.insert(name.as_str().to_owned()) {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        let (declaration_ordinal, declared_field) = declared
            .iter()
            .enumerate()
            .find(|(_, candidate)| candidate.name().as_str() == name.as_str())
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let declaration_ordinal = u32::try_from(declaration_ordinal).map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::AccountingOverflow)
        })?;
        let declared_ty = self
            .types
            .get(&declared_field.ty())
            .cloned()
            .ok_or_else(|| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::TypeResolutionFailed {
                    owner: declared_field.ty(),
                })
            })?;
        let field_expected = substitutions.apply_resolved(&declared_ty);
        let (source, actual) = match field {
            HirRecordField::Explicit { value, .. } => (
                PreparedRecordValueSource::Expression(*value),
                self.evaluate_expression(context, *value, field_expected.as_ref())?
                    .ty()
                    .clone(),
            ),
            HirRecordField::Shorthand { local, .. } => {
                let actual = self.facts.locals().get(local).cloned().ok_or_else(|| {
                    AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::LocalTypeUnavailable { owner: *local },
                    )
                })?;
                self.record_implicit_capture(owner, *local)
                    .map_err(AnalyzerExpressionError::fatal)?;
                (PreparedRecordValueSource::Local(*local), actual)
            }
            HirRecordField::Invalid { .. } => {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::RecoveredOwner,
                ));
            }
        };
        if !substitutions.observe(&declared_ty, &actual) {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        let resolved_declared = substitutions
            .apply_resolved(&declared_ty)
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        if !resolved_declared.accepts(&actual) {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        Ok((declaration_ordinal, resolved_declared, source))
    }
}

#[derive(Clone, Copy)]
enum ExpectedProjectRecordNominal<'a> {
    None,
    Complete(&'a ProjectNominalType),
    Parametric(&'a ProjectNominalType),
}

fn expected_project_record_nominal<'a>(
    owner: ExprId,
    declaration: &ProjectNominalDeclaration,
    expectation: &'a AnalyzerExpressionExpectation<'a>,
) -> Result<ExpectedProjectRecordNominal<'a>, AnalyzerExpressionError> {
    match expectation {
        AnalyzerExpressionExpectation::Complete(TypeKind::ProjectNominal(nominal))
            if nominal.declaration() == declaration.id() =>
        {
            Ok(ExpectedProjectRecordNominal::Complete(nominal))
        }
        AnalyzerExpressionExpectation::Complete(_) => Err(AnalyzerExpressionError::fatal(
            FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
        )),
        AnalyzerExpressionExpectation::Parametric {
            expected: TypeKind::ProjectNominal(nominal),
            ..
        } if nominal.declaration() == declaration.id() => {
            Ok(ExpectedProjectRecordNominal::Parametric(nominal))
        }
        AnalyzerExpressionExpectation::Unconstrained
        | AnalyzerExpressionExpectation::Parametric { .. } => {
            Ok(ExpectedProjectRecordNominal::None)
        }
    }
}

fn prepare_record_substitutions(
    owner: ExprId,
    declaration: &ProjectNominalDeclaration,
    expectation: &AnalyzerExpressionExpectation<'_>,
    expected: ExpectedProjectRecordNominal<'_>,
) -> Result<(Vec<GenericTypeParameterId>, TypeParameterSubstitutions), AnalyzerExpressionError> {
    let nominal = match expected {
        ExpectedProjectRecordNominal::None => None,
        ExpectedProjectRecordNominal::Complete(nominal)
        | ExpectedProjectRecordNominal::Parametric(nominal) => Some(nominal),
    };
    if nominal
        .is_some_and(|nominal| nominal.arguments().len() != declaration.type_parameters().len())
    {
        return Err(AnalyzerExpressionError::fatal(
            FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
        ));
    }
    let parameters = declaration
        .type_parameters()
        .iter()
        .map(|parameter| {
            GenericTypeParameterId::new(
                GenericParameterOwnerId::Nominal(declaration.id().clone()),
                parameter.ordinal(),
            )
        })
        .collect::<Vec<_>>();
    let mut substitutions = TypeParameterSubstitutions::default();
    if let Some(nominal) = nominal {
        for (parameter, argument) in parameters.iter().zip(nominal.arguments()) {
            if matches!(expected, ExpectedProjectRecordNominal::Parametric(_))
                && !matches!(
                    expectation.project_checked(owner, Some(argument))?,
                    AnalyzerExpressionExpectation::Complete(_)
                )
            {
                continue;
            }
            if !substitutions.observe(&TypeKind::GenericParam(parameter.clone()), argument) {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                ));
            }
        }
    }
    Ok((parameters, substitutions))
}
