//! Runtime defer function sites, issued from checked statement facts.

use super::*;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct DeferSiteKey {
    scope: crate::semantic_facts::RuntimeExecutableSiteScope,
    statement: StmtId,
}

impl DeferSiteKey {
    pub(super) fn new(
        scope: crate::semantic_facts::RuntimeExecutableSemanticScope<'_>,
        statement: StmtId,
    ) -> Self {
        Self {
            scope: scope.into(),
            statement,
        }
    }
}

pub(super) struct ReservedDeferDefinition<'facts> {
    scope: RuntimeScopedExecutableSemanticFactView<'facts>,
    statement: StmtId,
    body: ExprId,
    function: RuntimeFunctionSiteSeedId,
    effects: RuntimeEffectSet,
    locals: ProjectFunctionFrameLocals,
}

pub(super) fn reserve_defer_sites<'facts>(
    context: &FinalLoweringContext<'_, 'facts>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    BTreeMap<DeferSiteKey, RuntimeDeferSiteId>,
    Vec<ReservedDeferDefinition<'facts>>,
) {
    let mut sites = BTreeMap::new();
    let mut definitions = Vec::new();
    let mut scoped = Vec::new();
    context
        .facts
        .visit_scoped_defers(&mut |scope, statement, fact| scoped.push((scope, statement, fact)));
    let specs = scoped
        .iter()
        .flat_map(|(scope, statement, fact)| {
            let key = DeferSiteKey::new(scope.scope(), *statement);
            fact.captures()
                .iter()
                .enumerate()
                .map(move |(position, capture)| {
                    capture
                        .checked_origin()
                        .runtime_local_declaration_source()
                        .map(|origin| {
                            (
                                key.clone(),
                                position,
                                RuntimeLocalDeclarationSeed::new(origin, capture.ty().identity()),
                            )
                        })
                        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))
                })
        })
        .collect::<Result<Vec<_>, _>>();
    let specs = match specs {
        Ok(specs) => specs,
        Err(error) => {
            errors.push(error);
            return (sites, definitions);
        }
    };
    let admission = match builder.admit_type_batch([], specs.iter().map(|(_, _, seed)| *seed)) {
        Ok(admission) => admission,
        Err(error) => {
            errors.push(RuntimePlanLowerError::new(error.to_string()));
            return (sites, definitions);
        }
    };
    let input_locals = specs
        .iter()
        .zip(admission.local_ids())
        .map(|((statement, position, _), local)| ((statement.clone(), *position), local.clone()))
        .collect::<BTreeMap<_, _>>();

    for (scope, statement, fact) in scoped {
        let key = DeferSiteKey::new(scope.scope(), statement);
        let mut frame = match ProjectFunctionFrameLocals::admit_view(scope.facts(), None, builder) {
            Ok(frame) => frame,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let declaration = (|| -> Result<_, RuntimePlanLowerError> {
            let result = scope.expression_type(fact.body()).ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "defer {statement:?} has no checked Unit body type"
                ))
            })?;
            let effects = RuntimeEffectSet::try_from_effects(fact.effects().iter().cloned())
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
            let inputs = fact
                .captures()
                .iter()
                .enumerate()
                .map(|(position, capture)| {
                    frame.hir.get(&capture.local()).ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "defer {statement:?} capture {:?} has no admitted local",
                            capture.local()
                        ))
                    })?;
                    let input_local = input_locals
                        .get(&(key.clone(), position))
                        .cloned()
                        .ok_or_else(|| {
                            RuntimePlanLowerError::new(format!(
                                "defer {statement:?} capture {position} has no input local"
                            ))
                        })?;
                    frame.hir.insert(capture.local(), input_local.clone());
                    let position = u32::try_from(position).map_err(|_| {
                        RuntimePlanLowerError::new("defer capture position exceeds checked limits")
                    })?;
                    Ok(RuntimeFunctionInputBindingSeed {
                        transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::Transferred(
                            capture
                                .transfer()
                                .runtime_capture_mode()
                                .expect("accepted defer capture is a value transfer"),
                        ),
                        origin: capture
                            .origin()
                            .runtime_input_origin()
                            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?,
                        ownership: capture.input_ownership(),
                        unrestricted_bindings: if matches!(
                            capture.input_ownership(),
                            RuntimeValueOwnershipRequirement::Unrestricted
                        ) {
                            Box::new([input_local.clone()])
                        } else {
                            Box::new([])
                        },
                        source: RuntimeFunctionInputSource::Capture { position },
                        input_local: input_local.clone(),
                        pattern: RuntimePatternSeed::new(
                            capture.ty().identity(),
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: input_local,
                            },
                        ),
                    })
                })
                .collect::<Result<Vec<_>, RuntimePlanLowerError>>()?;
            Ok((
                RuntimeFunctionSiteDeclarationSeed {
                    definition: fact.definition_identity().runtime_identity(),
                    role: arcweft_core::plan::RuntimeFunctionSemanticRole::Effect,
                    function_type: None,
                    inputs: inputs.into_boxed_slice(),
                    result: result.identity(),
                    body_kind: RuntimeFunctionSiteBodyKind::Executable,
                    effects: effects.clone(),
                },
                effects,
            ))
        })();
        let (declaration, effects) = match declaration {
            Ok(declaration) => declaration,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let function = match builder.reserve_function_site_seed(declaration) {
            Ok(function) => function,
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(error.to_string()));
                continue;
            }
        };
        let site = match builder.reserve_defer_site_seed(&function) {
            Ok(site) => site,
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(error.to_string()));
                continue;
            }
        };
        sites.insert(key, site);
        definitions.push(ReservedDeferDefinition {
            scope,
            statement,
            body: fact.body(),
            function,
            effects,
            locals: frame,
        });
    }
    (sites, definitions)
}

pub(super) fn define_defer_sites<'facts>(
    context: &FinalLoweringContext<'_, 'facts>,
    definitions: &[ReservedDeferDefinition<'facts>],
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> Vec<RuntimeAssertionSite> {
    let mut assertions = Vec::new();
    for definition in definitions {
        let result = (|| -> Result<_, RuntimePlanLowerError> {
            let module =
                module_by_id(context.project, definition.body.module()).ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "defer {:?} body module is absent",
                        definition.statement
                    ))
                })?;
            let HirExprKind::Block(block) = module
                .resolve_expr(definition.body)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?
                .kind()
            else {
                return Err(RuntimePlanLowerError::new(format!(
                    "defer {:?} body is not a checked Block",
                    definition.statement
                )));
            };
            let mut lowerer = FinalFlowLowerer::new(
                module,
                context,
                RuntimeAssertionOwner::Defer(definition.statement),
            )
            .with_executable_scope(
                definition.scope,
                &definition.locals.control,
                &definition.locals.hir,
                &definition.locals.specialized_operands,
            );
            let ops = lowerer.lower_statement_ids_with_tail(
                block.statements(),
                RuntimeFlowTail::Value {
                    expression: block.tail(),
                    continuation: Box::new(RuntimeFlowValueContinuation::Return),
                    overrides: super::RuntimeExpressionOverrides::new(),
                },
            )?;
            let assertions = lowerer.into_assertion_sites();
            builder
                .define_function_site_seed(
                    &definition.function,
                    RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                        effects: definition.effects.clone(),
                        ops: ops.into_boxed_slice(),
                    }),
                )
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
            Ok(assertions)
        })();
        match result {
            Ok(sites) => assertions.extend(sites),
            Err(error) => errors.push(error),
        }
    }
    assertions
}
