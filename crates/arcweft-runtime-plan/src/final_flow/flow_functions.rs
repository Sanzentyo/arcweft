//! Original Flow function reservations shared by roots and Include calls.

use super::*;
use arcweft_lang_hir::symbol::CallableDeclarationDigest;

pub(super) struct ReservedFlowFunction {
    pub function: RuntimeFunctionSiteSeedId,
    pub invoke: Option<RuntimeFlowOpSeed>,
}

pub(super) type ReservedFlowFunctions = BTreeMap<CallableDeclarationDigest, ReservedFlowFunction>;

pub(super) fn reserve(
    project: HirAnalysisProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
    builder: &mut RuntimePlanBuilder,
) -> Result<(ReservedFlowFunctions, Vec<RuntimeFlowSchema>), Vec<RuntimePlanLowerError>> {
    let mut needed = BTreeSet::new();
    facts.visit_includes(&mut |target| {
        needed.insert(target.declaration());
    });
    let mut functions = BTreeMap::new();
    let mut schemas = Vec::new();
    for item in project.items() {
        let HirItemKind::Flow(flow) = item.item().kind() else {
            continue;
        };
        if !facts.contains_runtime_owner(&HirRuntimeExecutableOwner::Item(item.id())) {
            continue;
        }
        let result = (|| {
            let fact = facts.flow(item.id()).ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "checked runtime Flow identity is missing for final-HIR item {:?}",
                    item.id()
                ))
            })?;
            let schema = flow_invocation_schema(
                item.module(),
                flow,
                fact.identity(),
                fact.definition(),
                facts,
            )?;
            let declaration = declaration(flow, fact, &schema, locals)?;
            let function = builder
                .reserve_function_site_seed(declaration)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
            builder
                .bind_flow_function_seed(fact.identity().clone(), &function)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
            let key = fact.declaration().semantic_digest();
            let invoke = if needed.contains(&key) {
                Some(invocation(fact, function.clone(), builder)?)
            } else {
                None
            };
            if functions
                .insert(key, ReservedFlowFunction { function, invoke })
                .is_some()
            {
                return Err(RuntimePlanLowerError::new(
                    "duplicate accepted Flow declaration reservation",
                ));
            }
            schemas.push(schema);
            Ok(())
        })();
        result.map_err(|error| vec![error])?;
    }
    if needed.iter().any(|target| !functions.contains_key(target)) {
        return Err(vec![RuntimePlanLowerError::new(
            "checked Include target has no original Flow function reservation",
        )]);
    }
    Ok((functions, schemas))
}

fn declaration(
    flow: &HirFlowItem,
    fact: &crate::semantic_facts::RuntimeFlowFact,
    schema: &RuntimeFlowSchema,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
) -> Result<RuntimeFunctionSiteDeclarationSeed, RuntimePlanLowerError> {
    let RuntimeTypeShape::Function {
        parameters, result, ..
    } = fact.function_type().shape()
    else {
        return Err(RuntimePlanLowerError::new(
            "accepted Flow has no normalized function signature",
        ));
    };
    if parameters.len() != schema.parameters.len() {
        return Err(RuntimePlanLowerError::new(
            "Flow invocation locals differ from its accepted input signature",
        ));
    }
    let inputs = flow
        .parameters()
        .iter()
        .zip(parameters)
        .zip(fact.definition().parameters())
        .zip(&schema.parameters)
        .map(|(((parameter, ty), formal), schema)| {
            let [local] = parameter.locals() else {
                return Err(RuntimePlanLowerError::new(
                    "Flow formal must retain its admitted direct binding",
                ));
            };
            let local = locals.get(local).ok_or_else(|| {
                RuntimePlanLowerError::new("Flow parameter has no admitted local")
            })?;
            Ok(RuntimeFunctionInputBindingSeed {
                transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::Formal,
                origin: formal.runtime_input_origin(),
                source: RuntimeFunctionInputSource::Parameter {
                    position: schema.coordinate.position(),
                    passing: schema.passing,
                },
                input_local: local.clone(),
                pattern: RuntimePatternSeed::new(
                    ty.identity(),
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local: local.clone(),
                    },
                ),
                ownership: RuntimeValueOwnershipRequirement::Owned,
                unrestricted_bindings: Box::new([]),
            })
        })
        .collect::<Result<Box<[_]>, RuntimePlanLowerError>>()?;
    Ok(RuntimeFunctionSiteDeclarationSeed {
        definition: fact.definition().definition_identity().runtime_identity(),
        role: arcweft_core::plan::RuntimeFunctionSemanticRole::Flow,
        function_type: Some(fact.function_type().identity()),
        inputs,
        result: result.identity(),
        body_kind: RuntimeFunctionSiteBodyKind::Executable,
        effects: fact.effects().clone(),
    })
}

fn invocation(
    fact: &crate::semantic_facts::RuntimeFlowFact,
    function: RuntimeFunctionSiteSeedId,
    builder: &mut RuntimePlanBuilder,
) -> Result<RuntimeFlowOpSeed, RuntimePlanLowerError> {
    use arcweft_core::plan::{
        RuntimeCallableAttachedContract, RuntimeCallablePosition, RuntimeCallableStateDefinition,
        RuntimeCallableTransition,
    };
    let RuntimeTypeShape::Function {
        parameters, result, ..
    } = fact.function_type().shape()
    else {
        return Err(RuntimePlanLowerError::new(
            "Include target has no accepted function type",
        ));
    };
    if !parameters.is_empty() || !fact.definition().inputs().is_empty() {
        return Err(RuntimePlanLowerError::new(format!(
            "Include target Flow {} requires a zero-input invocation ABI",
            fact.identity()
        )));
    }
    let state = builder
        .reserve_callable_state_seed()
        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
    builder
        .define_callable_state_seed(
            &state,
            RuntimeCallableStateDefinition {
                function_type: fact.function_type().identity(),
                origin: state.clone(),
                position: RuntimeCallablePosition::Unapplied,
                retained: Box::new([]),
                parameters: Box::new([]),
                result: result.identity(),
                attached: RuntimeCallableAttachedContract::None,
                transition: RuntimeCallableTransition::Invoke {
                    function,
                    captures: Box::new([]),
                    arguments: Box::new([]),
                },
                partials: Box::new([]),
            },
        )
        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
    Ok(RuntimeFlowOpSeed::ApplyFunction {
        callee: RuntimeExprSeed::new(
            fact.function_type().identity(),
            RuntimeExprSeedKind::MakeCallable {
                state,
                captures: Box::new([]),
            },
        ),
        args: Box::new([]),
        result: RuntimePatternSeed::new(result.identity(), RuntimePatternSeedKind::Discard),
    })
}
