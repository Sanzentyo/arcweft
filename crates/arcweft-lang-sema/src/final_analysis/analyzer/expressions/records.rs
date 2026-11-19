//! Project-record inference and exact field checking.

use super::super::expression_error::{AnalyzerExpressionContext, AnalyzerExpressionError};
use super::{
    Analyzer, AnalyzerExpressionExpectation, BTreeSet, CheckedTypeSelection, ExprId,
    FinalSemanticAnalysisError, HirRecordField, ProjectNominalBody, ProjectNominalDeclaration,
    ProjectNominalType, TypeKind,
};
use crate::final_analysis::{PreparedProjectRecordExpressionField, PreparedRecordValueSource};
use crate::types::{
    PreparedEffects, PreparedNominalArgumentInference, PreparedTypeCompatibility,
    TypeProjectionError,
};

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
            ProjectNominalType<PreparedEffects>,
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
        let source_types = self
            .source_type_preparation(context.work())
            .map_err(AnalyzerExpressionError::fatal)?;
        let mut inference = source_types
            .nominal_argument_inference(owner, declaration)
            .map_err(AnalyzerExpressionError::fatal)?;
        if let ExpectedProjectRecordNominal::Complete(nominal)
        | ExpectedProjectRecordNominal::Parametric(nominal) = expected_nominal
        {
            if nominal.arguments().len() != declaration.type_parameters().len() {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                ));
            }
            let effects = self
                .facts
                .effect_declarations()
                .map_err(AnalyzerExpressionError::fact)?;
            for (position, argument) in nominal.arguments().iter().enumerate() {
                if matches!(
                    expected_nominal,
                    ExpectedProjectRecordNominal::Parametric(_)
                ) && !matches!(
                    self.project_expression_expectation(
                        context,
                        owner,
                        expectation,
                        Some(argument)
                    )?,
                    AnalyzerExpressionExpectation::Complete(_)
                ) {
                    continue;
                }
                let argument = source_types
                    .retain_prepared(owner, argument, effects.read())
                    .map_err(AnalyzerExpressionError::fatal)?;
                inference.seed(position, argument).map_err(|error| {
                    AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::source_type_projection(
                            owner.into(),
                            TypeProjectionError::Instantiation(error),
                        ),
                    )
                })?;
            }
        }

        drop(source_types);
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
                &mut inference,
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

        Ok((
            inference
                .into_nominal()
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?,
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
        inference: &mut PreparedNominalArgumentInference,
        seen: &mut BTreeSet<String>,
    ) -> Result<(u32, TypeKind<PreparedEffects>, PreparedRecordValueSource), AnalyzerExpressionError>
    {
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
        let annotation = declared_field.ty();
        let field_expected =
            self.project_record_field_type(context, owner, annotation, inference)?;
        let value_fact;
        let (source, actual) = match field {
            HirRecordField::Explicit { value, .. } => {
                value_fact = self.evaluate_expression(context, *value, field_expected.as_ref())?;
                let actual = value_fact.value_type().ok_or_else(|| {
                    AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: *value },
                    )
                })?;
                (PreparedRecordValueSource::Expression(*value), actual)
            }
            HirRecordField::Shorthand { local, .. } => {
                let actual = self.facts.locals().get(local).ok_or_else(|| {
                    AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::LocalTypeUnavailable { owner: *local },
                    )
                })?;
                (PreparedRecordValueSource::Local(*local), actual)
            }
            HirRecordField::UnresolvedShorthand { .. } => {
                return Err(AnalyzerExpressionError::rejected(owner));
            }
            HirRecordField::Invalid { .. } => {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::RecoveredOwner,
                ));
            }
        };
        let mut work = context.work();
        let retained;
        let (expected, actual) = if let Some(expected) = field_expected {
            (expected, actual)
        } else {
            let effects = self
                .facts
                .effect_declarations()
                .map_err(AnalyzerExpressionError::fact)?;
            retained = self
                .source_type_preparation(context.work())
                .and_then(|source_types| {
                    source_types.retain_prepared(owner, actual, effects.read())
                })
                .map_err(AnalyzerExpressionError::fatal)?;
            let template = self.types.get(&annotation).ok_or_else(|| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::TypeResolutionFailed {
                    owner: annotation,
                })
            })?;
            let (graph, _) = self
                .facts
                .effect_declarations_mut()
                .map_err(AnalyzerExpressionError::fact)?
                .relation();
            if !inference
                .observe_field(template, &retained, graph, &mut work)
                .map_err(|error| {
                    AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::source_type_effect_projection(
                            owner.into(),
                            error,
                        ),
                    )
                })?
            {
                return Err(AnalyzerExpressionError::rejected(owner));
            }
            let expected = self
                .project_record_field_type(context, owner, annotation, inference)?
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
            (expected, &retained)
        };
        // Publication compares the projected field and the original source
        // again with their completed effect rows.
        if expected
            .check_prepared_compatibility(actual, &mut work)
            .map_err(AnalyzerExpressionError::fatal)?
            == PreparedTypeCompatibility::Rejected
        {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        Ok((declaration_ordinal, expected, source))
    }

    fn project_record_field_type(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owner: ExprId,
        annotation: arcweft_lang_hir::identity::TypeId,
        inference: &PreparedNominalArgumentInference,
    ) -> Result<Option<TypeKind<PreparedEffects>>, AnalyzerExpressionError> {
        let template = self.types.get(&annotation).ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::TypeResolutionFailed {
                owner: annotation,
            })
        })?;
        let (graph, _) = self
            .facts
            .effect_declarations_mut()
            .map_err(AnalyzerExpressionError::fact)?
            .relation();
        inference
            .project_field(template, graph, &mut context.work())
            .map_err(|error| {
                AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::source_type_effect_projection(owner.into(), error),
                )
            })
    }
}

#[derive(Clone, Copy)]
enum ExpectedProjectRecordNominal<'a> {
    None,
    Complete(&'a ProjectNominalType<PreparedEffects>),
    Parametric(&'a ProjectNominalType<PreparedEffects>),
}

fn expected_project_record_nominal<'a>(
    owner: ExprId,
    declaration: &ProjectNominalDeclaration,
    expectation: &AnalyzerExpressionExpectation<'a>,
) -> Result<ExpectedProjectRecordNominal<'a>, AnalyzerExpressionError> {
    match expectation {
        AnalyzerExpressionExpectation::Complete(TypeKind::ProjectNominal(nominal))
        | AnalyzerExpressionExpectation::AnnotatedBinding(TypeKind::ProjectNominal(nominal))
        | AnalyzerExpressionExpectation::EnumConstructorHead(TypeKind::ProjectNominal(nominal))
            if nominal.declaration() == declaration.id() =>
        {
            Ok(ExpectedProjectRecordNominal::Complete(nominal))
        }
        AnalyzerExpressionExpectation::Complete(_)
        | AnalyzerExpressionExpectation::AnnotatedBinding(_)
        | AnalyzerExpressionExpectation::CompileTimePublicId(_)
        | AnalyzerExpressionExpectation::PublicIdReference { .. }
        | AnalyzerExpressionExpectation::EnumConstructorHead(_) => {
            Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
            ))
        }
        AnalyzerExpressionExpectation::Parametric {
            expected: TypeKind::ProjectNominal(nominal),
            ..
        } if nominal.declaration() == declaration.id() => {
            Ok(ExpectedProjectRecordNominal::Parametric(nominal))
        }
        AnalyzerExpressionExpectation::Unconstrained
        | AnalyzerExpressionExpectation::ApplicationCallee(_)
        | AnalyzerExpressionExpectation::FunctionValueSource(_)
        | AnalyzerExpressionExpectation::Parametric { .. } => {
            Ok(ExpectedProjectRecordNominal::None)
        }
    }
}
