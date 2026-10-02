//! Closed DisplayText method selection and semantic fact projection.

use super::*;

pub(super) fn runtime_display_method_key(
    conformance: &CheckedDisplayConformance,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<(RuntimeTraitMethodInstanceKey, RuntimeNormalizedType), RuntimeSemanticProjectionError>
{
    let self_type = runtime_type(conformance.target(), symbols, world, analysis)?;
    Ok((
        RuntimeTraitMethodInstanceKey::new(
            conformance.method_declaration().clone(),
            self_type.identity(),
        ),
        self_type,
    ))
}

pub(super) fn runtime_display_method_content_type(
    conformance: &CheckedDisplayConformance,
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeNormalizedType, RuntimeSemanticProjectionError> {
    let owner = conformance.implementation();
    let error = |reason: &str| RuntimeSemanticProjectionError::TraitMethodInstance {
        owner,
        reason: reason.to_owned(),
    };
    let module = project
        .modules()
        .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module))
        .ok_or_else(|| error("implementation module is absent"))?;
    let item = module
        .resolve_item(owner)
        .map_err(|_| error("implementation item is absent"))?;
    let HirItemKind::Impl(implementation) = item.kind() else {
        return Err(error("selected DisplayText owner is not an Impl"));
    };
    let Some(HirImplMember::Function(method)) = implementation
        .members()
        .get(usize::from(conformance.method_ordinal()))
    else {
        return Err(error("selected DisplayText member is not a function"));
    };
    let return_type = method
        .return_type()
        .and_then(|id| analysis.ty(id))
        .ok_or_else(|| error("selected DisplayText method has no checked return type"))?;
    let TypeKind::Result { ok, .. } = conformance
        .instantiate_type(return_type)
        .map_err(|source| error(&source.to_string()))?
    else {
        return Err(error("selected DisplayText method does not return Result"));
    };
    let content = runtime_type(ok.as_ref(), symbols, world, analysis)?;
    if content.identity()
        != arcweft_core::value::RuntimeDialogueOpaqueRole::Content.semantic_identity()
    {
        return Err(error("selected DisplayText result is not Content"));
    }
    Ok(content)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one selected method use joins its exact lexical scope and conformance"
)]
fn register_runtime_display_method(
    selected: &mut BTreeMap<
        RuntimeTraitMethodInstanceKey,
        (
            CheckedDisplayConformance,
            BTreeSet<RuntimeTraitMethodInstanceUse>,
        ),
    >,
    conformance: &CheckedDisplayConformance,
    use_scope: Option<RuntimeTraitMethodUseScope>,
    expression: ExprId,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), RuntimeSemanticProjectionError> {
    let (key, _) = runtime_display_method_key(conformance, symbols, world, analysis)?;
    let row = selected
        .entry(key)
        .or_insert_with(|| (conformance.clone(), BTreeSet::new()));
    if row.0 != *conformance {
        return Err(RuntimeSemanticProjectionError::Call {
            owner: expression,
            reason: "one closed DisplayText method key has conflicting selected conformances"
                .to_owned(),
        });
    }
    if let Some(scope) = use_scope {
        row.1
            .insert(RuntimeTraitMethodInstanceUse::new(scope, expression));
    }
    Ok(())
}

fn display_method_use_scope(
    scope: RuntimeExecutableSemanticScope<'_>,
) -> Option<RuntimeTraitMethodUseScope> {
    match scope {
        RuntimeExecutableSemanticScope::Global => None,
        RuntimeExecutableSemanticScope::Program(program) => {
            Some(RuntimeTraitMethodUseScope::Program(program))
        }
        RuntimeExecutableSemanticScope::ProjectFunction(key) => {
            Some(RuntimeTraitMethodUseScope::ProjectFunction(key.clone()))
        }
        RuntimeExecutableSemanticScope::Closure(key) => {
            Some(RuntimeTraitMethodUseScope::Closure(key.clone()))
        }
        RuntimeExecutableSemanticScope::TraitMethod(key) => {
            Some(RuntimeTraitMethodUseScope::TraitMethod(key.clone()))
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "selected method instances join exact global, closed function, and dialogue use scopes"
)]
pub(super) fn materialize_runtime_display_methods(
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    runtime_calls: &BTreeMap<ExprId, RuntimeResolvedCall>,
    project_instances: &[RuntimeProjectFunctionInstanceFact],
    root_closures: &[RuntimeClosureInstanceFact],
    programs: &[(
        arcweft_id::runtime_program::RuntimePureProgramId,
        RuntimeProjectFunctionInstanceSemanticFacts,
    )],
    dialogue: &RuntimeDialogueProjectionCatalog,
    instances: &DiscoveredProjectInstances,
) -> Result<Vec<RuntimeTraitMethodFact>, RuntimeSemanticProjectionError> {
    let mut selected = BTreeMap::new();
    let mut note_call = |scope, expression, call: &RuntimeResolvedCall| {
        let RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Format(formatted)) =
            call.dispatch()
        else {
            return Ok(());
        };
        let Some(conformance) = formatted.display_witness().project_conformance() else {
            return Ok(());
        };
        register_runtime_display_method(
            &mut selected,
            conformance,
            display_method_use_scope(scope),
            expression,
            symbols,
            world,
            analysis,
        )
    };
    for (expression, call) in runtime_calls {
        note_call(RuntimeExecutableSemanticScope::Global, *expression, call)?;
    }
    let mut projection_error = None;
    for instance in project_instances {
        instance.visit_scoped_calls(&mut |scope, expression, call| {
            if projection_error.is_none() {
                projection_error = note_call(scope.scope(), expression, call).err();
            }
        });
    }
    for closure in root_closures {
        closure.visit_scoped_calls(&mut |scope, expression, call| {
            if projection_error.is_none() {
                projection_error = note_call(scope.scope(), expression, call).err();
            }
        });
    }
    for (program, semantics) in programs {
        semantics.visit_scoped_calls(
            arcweft_runtime_plan::semantic_facts::RuntimeScopedExecutableSemanticFactView::program(
                *program, semantics,
            ),
            &mut |scope, expression, call| {
                if projection_error.is_none() {
                    projection_error = note_call(scope.scope(), expression, call).err();
                }
            },
        );
    }
    if let Some(error) = projection_error {
        return Err(error);
    }
    drop(note_call);

    for ((scope, _), fragment) in &dialogue.fragments {
        let instance = match scope {
            RuntimeDialogueProjectionScope::Program(program) => {
                instances.program_types(*program)?
            }
            RuntimeDialogueProjectionScope::ProjectInstance(key) => {
                let node = instances
                    .nodes()
                    .find_map(|(candidate, node)| (candidate == key).then_some(node))
                    .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
                        owner: Some(fragment.source()),
                        reason: "closed dialogue scope has no instance solution".to_owned(),
                    })?;
                Some(instances.types(node))
            }
            RuntimeDialogueProjectionScope::Global
            | RuntimeDialogueProjectionScope::TraitMethod(_) => None,
        };
        for value in fragment.values() {
            let Some(project_display) = value.project_display() else {
                continue;
            };
            let expression = value.expression();
            let source_ty = analysis
                .expression(expression)
                .and_then(|checked| checked.value_type())
                .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(expression),
                    reason: "project display source has no checked type".to_owned(),
                })?;
            let closed = instance
                .map(|instance| instance.instantiate_type(source_ty))
                .transpose()?
                .unwrap_or_else(|| source_ty.clone());
            let witness = analysis
                .display_witness_for_interpolation_type(&closed)
                .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(expression),
                    reason: error.to_string(),
                })?
                .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(expression),
                    reason: "closed project interpolation lost its DisplayText witness".to_owned(),
                })?;
            let conformance = witness.project_conformance().ok_or_else(|| {
                RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(expression),
                    reason: "project display slot does not have a project conformance".to_owned(),
                }
            })?;
            let use_scope = match scope {
                RuntimeDialogueProjectionScope::Global => None,
                RuntimeDialogueProjectionScope::Program(program) => {
                    Some(RuntimeTraitMethodUseScope::Program(*program))
                }
                RuntimeDialogueProjectionScope::ProjectInstance(key) => {
                    Some(RuntimeTraitMethodUseScope::ProjectFunction(key.clone()))
                }
                RuntimeDialogueProjectionScope::TraitMethod(key) => {
                    Some(RuntimeTraitMethodUseScope::TraitMethod(key.clone()))
                }
            };
            let (key, _) = runtime_display_method_key(conformance, symbols, world, analysis)?;
            if key != *project_display.method() {
                return Err(RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(expression),
                    reason: "project display slot selected a different method instance".to_owned(),
                });
            }
            register_runtime_display_method(
                &mut selected,
                conformance,
                use_scope,
                expression,
                symbols,
                world,
                analysis,
            )?;
        }
    }

    let mut complete = Vec::new();
    let mut emitted = BTreeSet::new();
    while let Some((key, (conformance, uses))) = selected
        .iter()
        .find(|(key, _)| !emitted.contains(*key))
        .map(|(key, row)| (key.clone(), row.clone()))
    {
        emitted.insert(key.clone());
        let method = build_runtime_display_method(
            &key,
            &conformance,
            uses,
            project,
            symbols,
            world,
            analysis,
            runtime_owners,
            dialogue,
            instances,
        )?;
        if let Some(semantics) = method.closed_semantics() {
            let mut nested_error = None;
            semantics.visit_scoped_calls(
                arcweft_runtime_plan::semantic_facts::RuntimeScopedExecutableSemanticFactView::trait_method(method.key(), semantics),
                &mut |scope, expression, call| {
                    let RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Format(formatted)) = call.dispatch() else { return };
                    let Some(conformance) = formatted.display_witness().project_conformance() else { return };
                    if nested_error.is_none() {
                        nested_error = register_runtime_display_method(
                            &mut selected, conformance, display_method_use_scope(scope.scope()), expression,
                            symbols, world, analysis,
                        ).err();
                    }
                },
            );
            if let Some(error) = nested_error {
                return Err(error);
            }
        }
        complete.push(method);
    }
    Ok(complete
        .into_iter()
        .map(|method| {
            let uses = selected
                .get(method.key())
                .expect("every projected DisplayText fact has one selected conformance")
                .1
                .iter()
                .cloned()
                .collect();
            RuntimeTraitMethodFact::new_closed(
                method.declaration().clone(),
                method.implementation(),
                method.member(),
                RuntimeTraitIdentity::StandardDisplayText,
                method.self_type().clone(),
                method
                    .closed_callable()
                    .expect("closed method callable")
                    .clone(),
                method
                    .closed_semantics()
                    .expect("closed method semantics")
                    .clone(),
                uses,
            )
        })
        .collect())
}

#[allow(
    clippy::too_many_arguments,
    reason = "one selected method instance closes its HIR partition and complete semantic facts"
)]
fn build_runtime_display_method(
    key: &RuntimeTraitMethodInstanceKey,
    conformance: &CheckedDisplayConformance,
    uses: BTreeSet<RuntimeTraitMethodInstanceUse>,
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    dialogue: &RuntimeDialogueProjectionCatalog,
    instances: &DiscoveredProjectInstances,
) -> Result<RuntimeTraitMethodFact, RuntimeSemanticProjectionError> {
    let owner = conformance.implementation();
    let origin = ProjectInstantiationOrigin::TraitMethod(owner);
    let (expected_key, self_type) =
        runtime_display_method_key(conformance, symbols, world, analysis)?;
    if expected_key != *key {
        return Err(origin.error("selected DisplayText method key changed during projection"));
    }
    let mut projection_error = None;
    let selected = runtime_owners.selected_method_owners(
        conformance.method_declaration(),
        analysis.hir_topology().as_ref(),
        |owner| match analysis.expression(owner)?.resolution() {
            CheckedExpressionResolution::PostfixBracket(resolution) => Some(resolution.candidate()),
            _ => None,
        },
        |owner| {
            analysis
                .selected_call_expression_inventory(owner)
                .cloned()
                .map(HirSelectedCallExpressionDisposition::Callable)
        },
        |owner| reachability::runtime_select_target_disposition(analysis, owner),
        |owner| {
            if projection_error.is_some() {
                return None;
            }
            match reachability::runtime_expression_projection_for_owner(analysis, owner) {
                Ok(value) => Some(value),
                Err(error) => {
                    projection_error = Some(error);
                    None
                }
            }
        },
    )?;
    if let Some(error) = projection_error {
        return Err(RuntimeSemanticProjectionError::TraitMethodInstance {
            owner,
            reason: error.to_string(),
        });
    }
    let partition = analysis
        .execution_projection()
        .selected_method_fact_partition(&selected)?;
    let lexical = RuntimeExecutableInstantiation::Display {
        key,
        conformance,
        selected: &selected,
    };
    let semantics = runtime_executable_semantic_facts(
        origin,
        lexical,
        partition,
        [],
        project,
        symbols,
        world,
        analysis,
        runtime_owners,
        dialogue,
        &mut ProjectInstanceProjection::Materialize {
            graph: instances,
            caller: None,
        },
    )?;
    let checked = analysis
        .checked_callables()
        .project_callable(&CallableDeclarationKey::ImplMethod(
            conformance.method_declaration().clone(),
        ))
        .map_err(|error| {
            origin.error(format!(
                "selected method lacks checked callable identity: {error:?}"
            ))
        })?;
    let callable = arcweft_core::entry::RuntimeCallableId::from_checked_digest(
        checked.id().semantic_digest().into_bytes(),
    );
    Ok(RuntimeTraitMethodFact::new_closed(
        conformance.method_declaration().clone(),
        owner,
        conformance.method_ordinal(),
        RuntimeTraitIdentity::StandardDisplayText,
        self_type,
        callable,
        semantics,
        uses.into_iter().collect(),
    ))
}
