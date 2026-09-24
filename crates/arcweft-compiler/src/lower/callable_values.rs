//! Declaration-value roots and edges in the existing closed-instance worklist.

use super::*;

pub(super) fn resolve(
    owner: ExprId,
    declaration: &arcweft_lang_sema::final_analysis::CheckedProjectCallable,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    instances: &mut ProjectInstanceProjection<'_>,
) -> Result<RuntimeResolvedValue, RuntimeSemanticProjectionError> {
    let origin = ProjectInstantiationOrigin::CallableValue(owner);
    let selection = arcweft_lang_sema::callable::select_project_function_value_runtime(
        declaration.declaration(),
        analysis.checked_callables(),
    )
    .map_err(|error| match error {
        arcweft_lang_sema::callable::CheckedProjectFunctionRuntimeSelectionError::OpenRootInstantiation => RuntimeSemanticProjectionError::UnclosedCallableScheme { owner },
        error => origin.error(error.to_string()),
    })?;
    let callable = runtime_project_callable(declaration.declaration(), symbols, world, analysis)
        .map_err(|reason| origin.error(reason))?;
    let solution = selection.solution().clone();
    let key = RuntimeProjectFunctionInstanceKey::new(
        callable.runtime().clone(),
        solution.instantiation(),
        selection.group(),
    );
    ensure_runtime_project_function_instance(
        origin,
        key.clone(),
        callable.clone(),
        ProjectInstanceSelection::from_root(&selection),
        solution,
        instances,
    )?;
    Ok(RuntimeResolvedValue::ProjectCallable {
        callable,
        instance: key,
    })
}

pub(super) fn discover_roots(
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    instance_owned: &BTreeSet<ExprId>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    instances: &mut ProjectInstanceProjection<'_>,
) -> Result<BTreeMap<ExprId, RuntimeResolvedValue>, RuntimeSemanticProjectionError> {
    let mut values = BTreeMap::new();
    for executable in runtime_owners.reachable_executables() {
        let partition = analysis
            .execution_projection()
            .runtime_fact_partition(runtime_owners, executable)?;
        for row in partition.expressions() {
            if row.family() != CheckedExecutableRuntimeExpressionFactFamily::Value
                || instance_owned.contains(&row.owner())
            {
                continue;
            }
            let owner = row.owner();
            let Some(checked) = analysis.expression(owner) else {
                continue;
            };
            let CheckedExpressionResolution::Value(CheckedValueResolution::ProjectCallable(
                declaration,
            )) = checked.resolution()
            else {
                continue;
            };
            if declaration.declaration().owner()
                != arcweft_lang_hir::symbol::CallableDeclarationOwner::Function
            {
                continue;
            }
            values.insert(
                owner,
                resolve(owner, declaration, symbols, world, analysis, instances)?,
            );
        }
    }
    Ok(values)
}
