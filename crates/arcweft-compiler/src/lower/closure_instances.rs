//! Closed closure roots outside ordinary project-function instantiations.

use super::*;

pub(super) fn materialize_root_closures(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    dialogue: &RuntimeDialogueProjectionCatalog,
    instances: &DiscoveredProjectInstances,
) -> Result<Vec<RuntimeClosureInstanceFact>, RuntimeSemanticProjectionError> {
    let execution = analysis.execution_projection();
    let mut roots = BTreeMap::new();
    let mut projection = ProjectInstanceProjection::Materialize {
        graph: instances,
        caller: None,
    };
    for executable in runtime_owners.reachable_executables() {
        if matches!(executable, HirRuntimeExecutableOwner::Closure(_)) {
            continue;
        }
        if let HirRuntimeExecutableOwner::Item(owner) = executable {
            let origin = ProjectInstantiationOrigin::Root(*owner);
            let module = project
                .modules()
                .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module))
                .ok_or_else(|| origin.error("closure root's executable module is absent"))?;
            let item = module
                .resolve_item(*owner)
                .map_err(|_| origin.error("closure root's executable item is absent"))?;
            if matches!(item.kind(), HirItemKind::Function(_)) {
                continue;
            }
        }
        let partition = execution.runtime_fact_partition(runtime_owners, executable)?;
        for expression in partition.expressions() {
            if expression.family() != CheckedExecutableRuntimeExpressionFactFamily::Closure {
                continue;
            }
            let owner = expression.owner();
            let origin = ProjectInstantiationOrigin::Call(owner);
            instances.check_cancelled(origin)?;
            let closure = runtime_closure_instance_fact(
                origin,
                RuntimeExecutableInstantiation::Global,
                owner,
                project,
                symbols,
                world,
                analysis,
                runtime_owners,
                dialogue,
                &mut projection,
            )?;
            if roots.insert(owner, closure).is_some() {
                return Err(origin.error("closure has more than one root executable parent"));
            }
        }
    }
    Ok(roots.into_values().collect())
}
