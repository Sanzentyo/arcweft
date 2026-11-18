//! Close a child task's result from its own selected Return operands.

use arcweft_lang_hir::{project::HirReturnContext, stmt::HirStatementChildRole};

use crate::{
    final_analysis::{CheckedThread, completion::ResultTypeJoin, prepared::PreparedThread},
    semantic_coordinate::CheckedSemanticPathStep,
};

use super::*;

pub(super) fn seal(
    owner: ExprId,
    thread: &PreparedThread,
    shell: &PreparedExpressionShell,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    callable_identities: &BTreeMap<ExprId, super::super::CheckedImplicitCallableIdentity>,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    types: &mut PublicationTypes<'_, '_>,
) -> Result<CheckedThread, FinalSemanticAnalysisError> {
    let Some(TypeKind::ThreadHandle(prepared_result)) = shell.source_value_type() else {
        return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
    };
    let thread_path = coordinates
        .expression(owner)
        .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
    let mut joined = TypeKind::Never;
    for returned in &thread.returns {
        types.visit_binding()?;
        let unavailable = || FinalSemanticAnalysisError::ReturnBoundaryUnavailable {
            owner: returned.statement,
        };
        let (context, _) = coordinates
            .return_context(returned.statement)
            .map_err(|_| unavailable())?;
        let statement = coordinates
            .statement(returned.statement)
            .map_err(|_| unavailable())?;
        let value = coordinates
            .expression(returned.value)
            .map_err(|_| unavailable())?;
        if context != HirReturnContext::Thread(owner)
            || value.root() != statement.path().root()
            || value.steps().strip_prefix(statement.path().steps())
                != Some(&[CheckedSemanticPathStep::Statement(
                    HirStatementChildRole::Value,
                )])
        {
            return Err(unavailable());
        }
        let implicit = coordinates
            .enclosing_implicit_callable(returned.value, |site| {
                callable_identities.contains_key(&site)
            })
            .map_err(|_| unavailable())?;
        if let Some(site) = implicit
            && site != owner
            && coordinates
                .expression(site)
                .map_err(|_| unavailable())?
                .is_at_or_below(&thread_path)
        {
            return Err(unavailable());
        }
        let value_type = expressions
            .get(&returned.value)
            .and_then(PreparedExpressionFact::value_type)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                owner: returned.value,
            })?;
        let actual = types.source(returned.value)?.canonical(value_type)?;
        joined = join_return(owner, joined, actual)?;
    }
    if thread.normal.is_unit() {
        joined = join_return(owner, joined, TypeKind::Unit)?;
    }

    let mut source = types.source(owner)?;
    let result = source.canonical(prepared_result)?;
    if result != joined {
        return Err(result_mismatch(owner, result, joined));
    }
    if let Some(expected) = &thread.expected {
        let expected = source.canonical(expected)?;
        if !source.accepts_canonical_type(&expected, &result)? {
            return Err(result_mismatch(owner, expected, result));
        }
    }
    Ok(CheckedThread::from_prepared(thread, result))
}

fn join_return(
    owner: ExprId,
    joined: TypeKind,
    actual: TypeKind,
) -> Result<TypeKind, FinalSemanticAnalysisError> {
    match ResultTypeJoin::for_values(&joined, &actual) {
        Some(ResultTypeJoin::Left) => Ok(joined),
        Some(ResultTypeJoin::Right) => Ok(actual),
        Some(ResultTypeJoin::Combined(ty)) => Ok(ty),
        None => Err(result_mismatch(owner, joined, actual)),
    }
}

fn result_mismatch(
    owner: ExprId,
    expected: TypeKind,
    actual: TypeKind,
) -> FinalSemanticAnalysisError {
    FinalSemanticAnalysisError::ThreadResultTypeMismatch {
        owner,
        expected: Box::new(expected),
        actual: Box::new(actual),
    }
}
