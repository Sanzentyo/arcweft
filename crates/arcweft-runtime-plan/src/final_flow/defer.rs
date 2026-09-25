//! Runtime defer function sites, issued from checked statement facts.

use super::*;

pub(super) struct ReservedGlobalDeferDefinition {
    statement: StmtId,
    body: ExprId,
    function: RuntimeFunctionSiteSeedId,
    effects: RuntimeEffectSet,
}

pub(super) fn reserve_global_defer_sites(
    context: &FinalLoweringContext<'_, '_>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    BTreeMap<StmtId, RuntimeDeferSiteId>,
    Vec<ReservedGlobalDeferDefinition>,
) {
    let mut sites = BTreeMap::new();
    let mut definitions = Vec::new();
    let specs = context
        .facts
        .defers()
        .flat_map(|(statement, fact)| {
            fact.captures()
                .iter()
                .enumerate()
                .map(move |(position, capture)| (statement, position, capture.ty().identity()))
        })
        .collect::<Vec<_>>();
    let admission = match builder.admit_type_batch(
        [],
        specs
            .iter()
            .map(|(_, _, ty)| RuntimeLocalDeclarationSeed::new(*ty)),
    ) {
        Ok(admission) => admission,
        Err(error) => {
            errors.push(RuntimePlanLowerError::new(error.to_string()));
            return (sites, definitions);
        }
    };
    let input_locals = specs
        .iter()
        .zip(admission.local_ids())
        .map(|((statement, position, _), local)| ((*statement, *position), local.clone()))
        .collect::<BTreeMap<_, _>>();

    for (statement, fact) in context.facts.defers() {
        let declaration = (|| -> Result<_, RuntimePlanLowerError> {
            let result = context.facts.expression_type(fact.body()).ok_or_else(|| {
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
                    let source = context.locals.get(&capture.local()).ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "defer {statement:?} capture {:?} has no admitted local",
                            capture.local()
                        ))
                    })?;
                    let input_local = input_locals
                        .get(&(statement, position))
                        .cloned()
                        .ok_or_else(|| {
                            RuntimePlanLowerError::new(format!(
                                "defer {statement:?} capture {position} has no input local"
                            ))
                        })?;
                    let position = u32::try_from(position).map_err(|_| {
                        RuntimePlanLowerError::new("defer capture position exceeds checked limits")
                    })?;
                    Ok(RuntimeFunctionInputBindingSeed {
                        source: RuntimeFunctionInputSource::Capture { position },
                        input_local,
                        pattern: RuntimePatternSeed::new(
                            capture.ty().identity(),
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: source.clone(),
                            },
                        ),
                    })
                })
                .collect::<Result<Vec<_>, RuntimePlanLowerError>>()?;
            Ok((
                RuntimeFunctionSiteDeclarationSeed {
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
        sites.insert(statement, site);
        definitions.push(ReservedGlobalDeferDefinition {
            statement,
            body: fact.body(),
            function,
            effects,
        });
    }
    (sites, definitions)
}

pub(super) fn define_global_defer_sites(
    context: &FinalLoweringContext<'_, '_>,
    definitions: &[ReservedGlobalDeferDefinition],
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
            );
            let ops = lowerer.lower_statement_ids_with_tail(
                block.statements(),
                RuntimeFlowTail::Value {
                    expression: block.tail(),
                    continuation: Box::new(RuntimeFlowValueContinuation::Return),
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
