use std::{collections::BTreeSet, rc::Rc};

use arcweft_lang_hir::{
    expr::{HirCallArgument, HirCallValue, HirRecoveredName, HirRequiredTokenState},
    leaf::{HirLiteral, HirStringLiteral},
    module::HirModule,
};

use super::{
    Analyzer, AnalyzerExpressionContext, CallAnalysisFailure, CandidateFactTransactionViolation,
    CheckedTypeSelection, FinalSemanticAnalysisError, PreparedExpressionFact,
    PreparedExpressionShell, TypeKind,
    expression_error::{AnalyzerExpressionError, AnalyzerExpressionInvariant},
    state::{CandidateFactTransactionAction, CandidateFactTransactionOutcome},
};
use crate::final_analysis::PreparedCompileTimeScalarExpression;
use crate::{
    checked_text_proxy::{
        CheckedCompileTimeScalar, CheckedCompileTimeScalarKind, CheckedTextProxyFieldDefault,
        CompileTimeScalarReductionError, PreparedCheckedTextProxyCatalog,
        TextProxyDeclarationDiagnostic, TextProxyDeclarationDiagnosticCause,
        TextProxyDefaultExpectation, TextProxyMetadataRole,
    },
    registration::CompileTimeScalarTypeRoleId,
};

enum TextProxyDefaultAttempt {
    Accepted(CheckedCompileTimeScalar),
    Diagnostic(TextProxyDeclarationDiagnosticCause),
}

impl Analyzer<'_, '_, '_> {
    /// Replaces one already-evaluated scalar argument with the exact prepared
    /// compile-time carrier and returns that same fact. This runs inside the
    /// candidate transaction, so the original resolution and scalar reduction
    /// are rolled back together when the candidate is rejected.
    pub(super) fn materialize_compile_time_scalar_fact(
        &mut self,
        module: &HirModule,
        owner: arcweft_lang_hir::identity::ExprId,
        kind: &CheckedCompileTimeScalarKind,
        exact_type: TypeKind,
        fact: PreparedExpressionFact,
    ) -> Result<PreparedExpressionFact, AnalyzerExpressionError> {
        if let PreparedExpressionFact::CompileTimeScalar(prepared) = &fact {
            if prepared.shell().value_type() != Some(&exact_type) {
                return Err(AnalyzerExpressionError::rejected(owner));
            }
            return Ok(fact);
        }
        let scalar = self
            .reduce_compile_time_scalar_expression(module, owner, kind, &fact)
            .map_err(|_| AnalyzerExpressionError::rejected(owner))?;
        let shell = PreparedExpressionShell::value(
            exact_type,
            CheckedTypeSelection::Expected,
            fact.effects().clone(),
        );
        let prepared = PreparedCompileTimeScalarExpression::try_new(shell, scalar, fact)
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let fact = PreparedExpressionFact::from(prepared);
        self.facts
            .replace_existing_expression(owner, fact.clone())
            .map_err(|_| {
                AnalyzerExpressionError::fact(
                    CandidateFactTransactionViolation::ProjectionUnavailable,
                )
            })?;
        Ok(fact)
    }

    pub(super) fn prepare_text_proxy_catalog(&mut self) -> Result<(), FinalSemanticAnalysisError> {
        if self.text_proxies.is_some() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let mut prepared = PreparedCheckedTextProxyCatalog::build(
            self.executable,
            self.symbols,
            &self.types,
            &self.type_reports,
            self.catalogs.world().environment().compile_time_scalars(),
        );
        self.prepare_text_proxy_defaults(&mut prepared)?;
        self.text_proxies = Some(prepared);
        Ok(())
    }

    fn prepare_text_proxy_defaults(
        &mut self,
        catalog: &mut PreparedCheckedTextProxyCatalog,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut diagnostics = Vec::new();
        let mut last_expression = None;
        let outcome = self.run_candidate_fact_transaction::<_, AnalyzerExpressionError>(
            |this, _outer_expression_authority, _outer_transaction_authority| {
                for declaration in catalog.definition_ids() {
                    let Some(mut definition) = catalog.remove(&declaration) else {
                        return Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        ));
                    };
                    let arguments = definition.arguments().to_vec();
                    let mut seen = BTreeSet::new();
                    let mut valid = true;
                    for argument in arguments {
                        let expression = argument.value();
                        last_expression = Some(expression);
                        let span = {
                            let module = this
                                .module(expression.module())
                                .map_err(AnalyzerExpressionError::fatal)?;
                            super::statements::expression_span(module, expression)
                                .map_err(AnalyzerExpressionError::fatal)?
                        };
                        let default_name = argument
                            .resolved_name()
                            .map(|name| {
                                arcweft_lang_syntax::ast::module_path::ModuleSegment::new(
                                    name.as_str(),
                                )
                                .map_err(|_| {
                                    AnalyzerExpressionError::fatal(
                                        FinalSemanticAnalysisError::WrongPayloadFamily,
                                    )
                                })
                            })
                            .transpose()?;
                        let Some((name, expression)) = (match &argument {
                            HirCallArgument::Named {
                                name: HirRecoveredName::Valid(name),
                                equals: HirRequiredTokenState::Present,
                                value: HirCallValue::Present { value },
                            } => Some((name, *value)),
                            HirCallArgument::Named { .. }
                            | HirCallArgument::Positional { .. }
                            | HirCallArgument::Spread { .. } => None,
                        }) else {
                            diagnostics.push(TextProxyDeclarationDiagnostic::new(
                                definition.checked().declaration().clone(),
                                definition.checked().attribute_origin().clone(),
                                default_name,
                                expression,
                                span,
                                TextProxyDefaultExpectation::Unknown,
                                TextProxyDeclarationDiagnosticCause::InvalidArgument,
                            ));
                            valid = false;
                            continue;
                        };
                        if !seen.insert(name.clone()) {
                            diagnostics.push(TextProxyDeclarationDiagnostic::new(
                                definition.checked().declaration().clone(),
                                definition.checked().attribute_origin().clone(),
                                default_name,
                                expression,
                                span,
                                text_proxy_default_expectation(
                                    definition.checked(),
                                    name.as_str(),
                                ),
                                TextProxyDeclarationDiagnosticCause::DuplicateDefault,
                            ));
                            valid = false;
                            continue;
                        }

                        let field = definition
                            .checked()
                            .fields()
                            .iter()
                            .position(|field| field.diagnostic_name() == name.as_str());
                        let (kind, expected) = match name.as_str() {
                            "role" => (
                                CheckedCompileTimeScalarKind::PublicId,
                                TextProxyDefaultExpectation::Metadata(TextProxyMetadataRole::Role),
                            ),
                            "layer" => (
                                CheckedCompileTimeScalarKind::PublicId,
                                TextProxyDefaultExpectation::Metadata(
                                    TextProxyMetadataRole::Layer,
                                ),
                            ),
                            "depth" => (
                                CheckedCompileTimeScalarKind::Length,
                                TextProxyDefaultExpectation::Metadata(
                                    TextProxyMetadataRole::Depth,
                                ),
                            ),
                            "hit_test" => (
                                CheckedCompileTimeScalarKind::Bool,
                                TextProxyDefaultExpectation::Metadata(
                                    TextProxyMetadataRole::HitTest,
                                ),
                            ),
                            _ => match field {
                                Some(field) => {
                                    let kind = definition.checked().fields()[field].kind().clone();
                                    (
                                        kind.clone(),
                                        TextProxyDefaultExpectation::Scalar(kind),
                                    )
                                }
                                None => {
                                    diagnostics.push(TextProxyDeclarationDiagnostic::new(
                                        definition.checked().declaration().clone(),
                                        definition.checked().attribute_origin().clone(),
                                        default_name,
                                        expression,
                                        span,
                                        TextProxyDefaultExpectation::Unknown,
                                        TextProxyDeclarationDiagnosticCause::UnknownDefault,
                                    ));
                                    valid = false;
                                    continue;
                                }
                            },
                        };
                        let source_mode = this
                            .compile_time_scalar_source_mode(&kind)
                            .ok_or_else(|| {
                                AnalyzerExpressionError::fatal(
                                    FinalSemanticAnalysisError::WrongPayloadFamily,
                                )
                            })?;
                        let attempt = this.run_candidate_fact_transaction::<_, AnalyzerExpressionError>(
                            |this, authority, _transaction_authority| {
                                let context = AnalyzerExpressionContext::candidate(
                                    authority,
                                    Rc::clone(&this.call_frames),
                                );
                                let result = this.evaluate_compile_time_scalar_source(
                                    &context,
                                    expression,
                                    &source_mode,
                                );
                                drop(context);
                                let fact = match result {
                                    Ok(fact) => fact,
                                    Err(error) => {
                                        let Some(cause) = authored_default_cause(&error) else {
                                            return Err(error);
                                        };
                                        return Ok(CandidateFactTransactionAction::Rollback(
                                            TextProxyDefaultAttempt::Diagnostic(cause),
                                        ));
                                    }
                                };
                                if !fact.effects().is_empty() {
                                    return Ok(CandidateFactTransactionAction::Rollback(
                                        TextProxyDefaultAttempt::Diagnostic(
                                            TextProxyDeclarationDiagnosticCause::Expression,
                                        ),
                                    ));
                                }
                                let module = this
                                    .module(expression.module())
                                    .map_err(AnalyzerExpressionError::fatal)?;
                                let exact_rgb = if matches!(
                                    kind,
                                    CheckedCompileTimeScalarKind::Color
                                ) {
                                    let graph = this
                                        .facts
                                        .prepared_calls()
                                        .map_err(AnalyzerExpressionError::fact)?;
                                    graph
                                        .project_site_payload(
                                            crate::callable::CheckedCallSite::HirCall(expression),
                                            |prefix| {
                                                prefix.application().selected().id()
                                                    == &crate::callable::CallableCandidateId::Builtin(
                                                        crate::callable::BuiltinCallableId::Rgb,
                                                    )
                                            },
                                            |_| false,
                                        )
                                        .ok_or_else(|| {
                                            AnalyzerExpressionError::fact(
                                                CandidateFactTransactionViolation::PreparedCallGraph(
                                                    crate::callable::CallConstraintInvariant::MissingOrStalePreparedNode
                                                        .into(),
                                                ),
                                            )
                                        })?
                                } else {
                                    false
                                };
                                let scalar = match reduce_compile_time_scalar(
                                    module,
                                    expression,
                                    &kind,
                                    &fact,
                                    exact_rgb,
                                ) {
                                    Ok(value) => value,
                                    Err(cause) => {
                                        return Ok(CandidateFactTransactionAction::Rollback(
                                            TextProxyDefaultAttempt::Diagnostic(
                                                TextProxyDeclarationDiagnosticCause::Reduction(
                                                    cause,
                                                ),
                                            ),
                                        ));
                                    }
                                };
                                let shell = PreparedExpressionShell::value(
                                    this.compile_time_scalar_type(&kind),
                                    CheckedTypeSelection::Expected,
                                    fact.effects().clone(),
                                );
                                let prepared =
                                    PreparedCompileTimeScalarExpression::try_new(
                                        shell,
                                        scalar.clone(),
                                        fact,
                                    )
                                    .ok_or_else(|| {
                                        AnalyzerExpressionError::fatal(
                                            FinalSemanticAnalysisError::WrongPayloadFamily,
                                        )
                                    })?;
                                this.facts
                                    .replace_existing_expression(
                                        expression,
                                        PreparedExpressionFact::from(prepared),
                                    )
                                    .map_err(|_| {
                                        AnalyzerExpressionError::fact(
                                            CandidateFactTransactionViolation::ProjectionUnavailable,
                                        )
                                    })?;
                                Ok(CandidateFactTransactionAction::Commit(
                                    TextProxyDefaultAttempt::Accepted(scalar),
                                ))
                            },
                        );
                        match attempt {
                            Ok(CandidateFactTransactionOutcome::Committed(
                                TextProxyDefaultAttempt::Accepted(value),
                            )) => match name.as_str() {
                                "role" => {
                                    let CheckedCompileTimeScalar::PublicId(value) = value else {
                                        return Err(AnalyzerExpressionError::fatal(
                                            FinalSemanticAnalysisError::WrongPayloadFamily,
                                        ));
                                    };
                                    definition
                                        .checked_mut()
                                        .metadata_defaults_mut()
                                        .set_role(expression, value);
                                }
                                "layer" => {
                                    let CheckedCompileTimeScalar::PublicId(value) = value else {
                                        return Err(AnalyzerExpressionError::fatal(
                                            FinalSemanticAnalysisError::WrongPayloadFamily,
                                        ));
                                    };
                                    definition
                                        .checked_mut()
                                        .metadata_defaults_mut()
                                        .set_layer(expression, value);
                                }
                                "depth" => {
                                    let CheckedCompileTimeScalar::Length(value) = value else {
                                        return Err(AnalyzerExpressionError::fatal(
                                            FinalSemanticAnalysisError::WrongPayloadFamily,
                                        ));
                                    };
                                    if value.unit != crate::checked_rich_text::LengthUnit::Px {
                                        diagnostics.push(TextProxyDeclarationDiagnostic::new(
                                            definition.checked().declaration().clone(),
                                            definition.checked().attribute_origin().clone(),
                                            default_name,
                                            expression,
                                            span,
                                            expected,
                                            TextProxyDeclarationDiagnosticCause::Reduction(
                                                CompileTimeScalarReductionError::WrongUnit,
                                            ),
                                        ));
                                        valid = false;
                                        continue;
                                    }
                                    definition.checked_mut().metadata_defaults_mut().set_depth(
                                        expression,
                                        crate::checked_rich_text::CheckedObjectDepth::new(
                                            value.milli,
                                        ),
                                    );
                                }
                                "hit_test" => {
                                    let CheckedCompileTimeScalar::Bool(value) = value else {
                                        return Err(AnalyzerExpressionError::fatal(
                                            FinalSemanticAnalysisError::WrongPayloadFamily,
                                        ));
                                    };
                                    definition
                                        .checked_mut()
                                        .metadata_defaults_mut()
                                        .set_hit_test(expression, value);
                                }
                                _ => {
                                    let Some(field) = field else {
                                        return Err(AnalyzerExpressionError::fatal(
                                            FinalSemanticAnalysisError::WrongPayloadFamily,
                                        ));
                                    };
                                    if !definition.checked_mut().fields_mut()[field].set_default(
                                        CheckedTextProxyFieldDefault::new(expression, value),
                                    ) {
                                        return Err(AnalyzerExpressionError::fatal(
                                            FinalSemanticAnalysisError::WrongPayloadFamily,
                                        ));
                                    }
                                }
                            },
                            Ok(CandidateFactTransactionOutcome::RolledBack(
                                TextProxyDefaultAttempt::Diagnostic(cause),
                            )) => {
                                diagnostics.push(TextProxyDeclarationDiagnostic::new(
                                    definition.checked().declaration().clone(),
                                    definition.checked().attribute_origin().clone(),
                                    default_name,
                                    expression,
                                    span,
                                    expected,
                                    cause,
                                ));
                                valid = false;
                            }
                            Ok(CandidateFactTransactionOutcome::Committed(
                                TextProxyDefaultAttempt::Diagnostic(_),
                            ))
                            | Ok(CandidateFactTransactionOutcome::RolledBack(
                                TextProxyDefaultAttempt::Accepted(_),
                            ))
                            | Ok(CandidateFactTransactionOutcome::Extracted { .. }) => {
                                return Err(AnalyzerExpressionError::fatal(
                                    FinalSemanticAnalysisError::WrongPayloadFamily,
                                ));
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    if valid && !catalog.insert(declaration, definition) {
                        return Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        ));
                    }
                }
                if diagnostics.is_empty() {
                    Ok(CandidateFactTransactionAction::Commit(()))
                } else {
                    Ok(CandidateFactTransactionAction::Rollback(()))
                }
            },
        );
        match outcome {
            Ok(CandidateFactTransactionOutcome::Committed(())) => Ok(()),
            Ok(CandidateFactTransactionOutcome::RolledBack(())) => {
                diagnostics.sort_by(|left, right| {
                    left.span()
                        .cmp(right.span())
                        .then_with(|| left.declaration().cmp(right.declaration()))
                        .then_with(|| left.attribute().cmp(right.attribute()))
                        .then_with(|| left.expression().cmp(&right.expression()))
                });
                Err(FinalSemanticAnalysisError::InvalidTextProxyDeclarations {
                    diagnostics: diagnostics.into_boxed_slice(),
                })
            }
            Ok(CandidateFactTransactionOutcome::Extracted { .. }) => {
                Err(FinalSemanticAnalysisError::CandidateFactTransaction {
                    violation: CandidateFactTransactionViolation::UnrecoverableLedger,
                })
            }
            Err(error) => Err(into_text_proxy_public_error(error, last_expression)),
        }
    }

    pub(super) fn compile_time_scalar_source_mode(
        &self,
        kind: &CheckedCompileTimeScalarKind,
    ) -> Option<crate::checked_compile_time::CompileTimeScalarSourceMode> {
        use crate::checked_compile_time::CompileTimeScalarSourceMode;

        match kind {
            CheckedCompileTimeScalarKind::Milli
            | CheckedCompileTimeScalarKind::Ratio
            | CheckedCompileTimeScalarKind::Length
            | CheckedCompileTimeScalarKind::Angle => Some(CompileTimeScalarSourceMode::Literal),
            CheckedCompileTimeScalarKind::PublicId => Some(CompileTimeScalarSourceMode::PublicId),
            CheckedCompileTimeScalarKind::Bool
            | CheckedCompileTimeScalarKind::Int
            | CheckedCompileTimeScalarKind::Duration
            | CheckedCompileTimeScalarKind::ClosedEnum(_)
            | CheckedCompileTimeScalarKind::Text
            | CheckedCompileTimeScalarKind::Color => Some(CompileTimeScalarSourceMode::Typed(
                self.compile_time_scalar_type(kind),
            )),
        }
    }

    pub(super) fn evaluate_compile_time_scalar_source(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        expression: arcweft_lang_hir::identity::ExprId,
        source_mode: &crate::checked_compile_time::CompileTimeScalarSourceMode,
    ) -> Result<PreparedExpressionFact, AnalyzerExpressionError> {
        match source_mode {
            crate::checked_compile_time::CompileTimeScalarSourceMode::Literal => {
                self.evaluate_expression(context, expression, None)
            }
            crate::checked_compile_time::CompileTimeScalarSourceMode::PublicId => {
                let source_type = TypeKind::String;
                self.evaluate_expression_with_expectation(
                    context,
                    expression,
                    super::expressions::AnalyzerExpressionExpectation::compile_time_public_id(
                        &source_type,
                    ),
                )
            }
            crate::checked_compile_time::CompileTimeScalarSourceMode::Typed(source_type) => {
                self.evaluate_expression(context, expression, Some(source_type))
            }
        }
    }

    pub(super) fn compile_time_scalar_type(&self, kind: &CheckedCompileTimeScalarKind) -> TypeKind {
        let scalars = self.catalogs.world().environment().compile_time_scalars();
        match kind {
            CheckedCompileTimeScalarKind::Bool => {
                scalars.type_for(CompileTimeScalarTypeRoleId::Bool).clone()
            }
            CheckedCompileTimeScalarKind::Int => {
                scalars.type_for(CompileTimeScalarTypeRoleId::Int).clone()
            }
            CheckedCompileTimeScalarKind::Milli => {
                scalars.type_for(CompileTimeScalarTypeRoleId::Milli).clone()
            }
            CheckedCompileTimeScalarKind::Ratio => {
                scalars.type_for(CompileTimeScalarTypeRoleId::Ratio).clone()
            }
            CheckedCompileTimeScalarKind::Length => scalars
                .type_for(CompileTimeScalarTypeRoleId::Length)
                .clone(),
            CheckedCompileTimeScalarKind::Angle => {
                scalars.type_for(CompileTimeScalarTypeRoleId::Angle).clone()
            }
            CheckedCompileTimeScalarKind::Duration => scalars
                .type_for(CompileTimeScalarTypeRoleId::Duration)
                .clone(),
            CheckedCompileTimeScalarKind::PublicId => scalars
                .type_for(CompileTimeScalarTypeRoleId::PublicId)
                .clone(),
            CheckedCompileTimeScalarKind::Text => {
                scalars.type_for(CompileTimeScalarTypeRoleId::Text).clone()
            }
            CheckedCompileTimeScalarKind::Color => {
                scalars.type_for(CompileTimeScalarTypeRoleId::Color).clone()
            }
            CheckedCompileTimeScalarKind::ClosedEnum(schema) => {
                TypeKind::ProjectNominal(crate::types::ProjectNominalType::new(
                    schema.declaration().clone(),
                    Box::<[TypeKind]>::default(),
                ))
            }
        }
    }
}

fn text_proxy_default_expectation(
    definition: &crate::checked_text_proxy::CheckedTextProxyDefinition,
    name: &str,
) -> TextProxyDefaultExpectation {
    match name {
        "role" => TextProxyDefaultExpectation::Metadata(TextProxyMetadataRole::Role),
        "layer" => TextProxyDefaultExpectation::Metadata(TextProxyMetadataRole::Layer),
        "depth" => TextProxyDefaultExpectation::Metadata(TextProxyMetadataRole::Depth),
        "hit_test" => TextProxyDefaultExpectation::Metadata(TextProxyMetadataRole::HitTest),
        _ => definition
            .fields()
            .iter()
            .find(|field| field.diagnostic_name() == name)
            .map(|field| TextProxyDefaultExpectation::Scalar(field.kind().clone()))
            .unwrap_or(TextProxyDefaultExpectation::Unknown),
    }
}

fn authored_default_cause(
    error: &AnalyzerExpressionError,
) -> Option<TextProxyDeclarationDiagnosticCause> {
    match error {
        AnalyzerExpressionError::Rejected(_) => Some(TextProxyDeclarationDiagnosticCause::Type),
        AnalyzerExpressionError::Fatal(error) => authored_final_error_cause(error),
        AnalyzerExpressionError::Call {
            failure: CallAnalysisFailure::FatalSource(error),
            ..
        } => error
            .cause()
            .direct_final_semantic()
            .and_then(authored_final_error_cause),
        AnalyzerExpressionError::Call {
            failure: CallAnalysisFailure::Abort(_) | CallAnalysisFailure::Invariant(_),
            ..
        }
        | AnalyzerExpressionError::Abort(_)
        | AnalyzerExpressionError::Invariant(_) => None,
    }
}

fn authored_final_error_cause(
    error: &FinalSemanticAnalysisError,
) -> Option<TextProxyDeclarationDiagnosticCause> {
    match error {
        FinalSemanticAnalysisError::ExpressionTypeUnavailable { .. }
        | FinalSemanticAnalysisError::TypeResolutionInput { .. }
        | FinalSemanticAnalysisError::TypeResolutionFailed { .. }
        | FinalSemanticAnalysisError::TypeResolutionReportMismatch { .. } => {
            Some(TextProxyDeclarationDiagnosticCause::Type)
        }
        FinalSemanticAnalysisError::ValueResolutionFailed { .. } => {
            Some(TextProxyDeclarationDiagnosticCause::Expression)
        }
        FinalSemanticAnalysisError::UnknownCallTarget { .. }
        | FinalSemanticAnalysisError::CallResolutionFailed { .. } => {
            Some(TextProxyDeclarationDiagnosticCause::Call)
        }
        _ => None,
    }
}

fn reduce_compile_time_scalar(
    module: &HirModule,
    owner: arcweft_lang_hir::identity::ExprId,
    kind: &CheckedCompileTimeScalarKind,
    fact: &PreparedExpressionFact,
    exact_rgb: bool,
) -> Result<CheckedCompileTimeScalar, CompileTimeScalarReductionError> {
    if matches!(kind, CheckedCompileTimeScalarKind::PublicId)
        && let Some(crate::final_analysis::CheckedExpressionResolution::Value(
            crate::final_analysis::CheckedValueResolution::Constant(HirLiteral::String(
                HirStringLiteral::Value(value),
            )),
        )) = fact.checked_resolution()
    {
        return arcweft_id::PublicId::try_new(value.to_string())
            .map(CheckedCompileTimeScalar::PublicId)
            .map_err(|_| CompileTimeScalarReductionError::WrongLiteral);
    }
    if let Some(crate::final_analysis::CheckedExpressionResolution::Value(
        crate::final_analysis::CheckedValueResolution::ProjectItem(item),
    )) = fact.checked_resolution()
        && matches!(kind, CheckedCompileTimeScalarKind::PublicId)
    {
        return Ok(CheckedCompileTimeScalar::PublicId(item.public_id().clone()));
    }
    if let CheckedCompileTimeScalarKind::ClosedEnum(schema) = kind {
        let PreparedExpressionFact::Variant(variant) = fact else {
            return Err(CompileTimeScalarReductionError::WrongHir);
        };
        let nominal = variant
            .owner()
            .project_nominal()
            .ok_or(CompileTimeScalarReductionError::WrongEnumDeclaration)?;
        if nominal.declaration() != schema.declaration()
            || variant
                .owner()
                .ty()
                .semantic_identity_digest()
                .map_err(|_| CompileTimeScalarReductionError::WrongEnumDeclaration)?
                != schema.semantic_type()
        {
            return Err(CompileTimeScalarReductionError::WrongEnumDeclaration);
        }
        let ordinal = variant.selected_ordinal();
        let owner_case = variant
            .owner()
            .cases()
            .get(
                usize::try_from(ordinal)
                    .map_err(|_| CompileTimeScalarReductionError::WrongEnumCase)?,
            )
            .ok_or(CompileTimeScalarReductionError::WrongEnumCase)?;
        if owner_case.payload().is_some() {
            return Err(CompileTimeScalarReductionError::WrongEnumPayload);
        }
        let case = schema
            .cases()
            .get(
                usize::try_from(ordinal)
                    .map_err(|_| CompileTimeScalarReductionError::WrongEnumCase)?,
            )
            .ok_or(CompileTimeScalarReductionError::WrongEnumCase)?;
        if owner_case.diagnostic_name() != Some(case.diagnostic_name()) {
            return Err(CompileTimeScalarReductionError::WrongEnumCase);
        }
        return crate::checked_text_proxy::reduce_enum_value(
            schema,
            nominal.declaration(),
            case.semantic_id(),
            ordinal,
        );
    }
    if matches!(kind, CheckedCompileTimeScalarKind::Color) {
        if !exact_rgb {
            return Err(CompileTimeScalarReductionError::WrongBuiltinCall);
        }
        return crate::checked_text_proxy::reduce_color_argument(module, owner);
    }
    crate::checked_text_proxy::reduce_literal_expression(module, owner, kind)
}

impl Analyzer<'_, '_, '_> {
    /// Reduces one already-evaluated ordinary Object argument through the
    /// shared compile-time scalar algebra. The fact is supplied by the ordinary
    /// candidate callback, so this helper never reopens or reparses source.
    pub(super) fn reduce_compile_time_scalar_expression(
        &self,
        module: &HirModule,
        owner: arcweft_lang_hir::identity::ExprId,
        kind: &CheckedCompileTimeScalarKind,
        fact: &PreparedExpressionFact,
    ) -> Result<CheckedCompileTimeScalar, CompileTimeScalarReductionError> {
        if !fact.effects().is_empty() {
            return Err(CompileTimeScalarReductionError::WrongHir);
        }
        let exact_rgb = if matches!(kind, CheckedCompileTimeScalarKind::Color) {
            self.facts
                .prepared_calls()
                .ok()
                .and_then(|graph| {
                    graph.project_site_payload(
                        crate::callable::CheckedCallSite::HirCall(owner),
                        |prefix| {
                            prefix.application().selected().id()
                                == &crate::callable::CallableCandidateId::Builtin(
                                    crate::callable::BuiltinCallableId::Rgb,
                                )
                        },
                        |_| false,
                    )
                })
                .unwrap_or(false)
        } else {
            false
        };
        reduce_compile_time_scalar(module, owner, kind, fact, exact_rgb)
    }
}

fn into_text_proxy_public_error(
    error: AnalyzerExpressionError,
    fallback_owner: Option<arcweft_lang_hir::identity::ExprId>,
) -> FinalSemanticAnalysisError {
    if let Some(owner) = fallback_owner {
        return error.into_public(owner);
    }
    match error {
        AnalyzerExpressionError::Fatal(error) => *error,
        AnalyzerExpressionError::Abort(
            crate::types::constraints::TypeConstraintAbort::Cancelled,
        ) => FinalSemanticAnalysisError::Cancelled,
        AnalyzerExpressionError::Abort(
            crate::types::constraints::TypeConstraintAbort::ArithmeticOverflow,
        ) => FinalSemanticAnalysisError::AccountingOverflow,
        AnalyzerExpressionError::Invariant(AnalyzerExpressionInvariant::Fact(violation)) => {
            (*violation).into()
        }
        AnalyzerExpressionError::Invariant(AnalyzerExpressionInvariant::Semantic(error)) => *error,
        AnalyzerExpressionError::Rejected(_)
        | AnalyzerExpressionError::Abort(_)
        | AnalyzerExpressionError::Invariant(_)
        | AnalyzerExpressionError::Call { .. } => FinalSemanticAnalysisError::WrongPayloadFamily,
    }
}
