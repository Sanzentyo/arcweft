//! A retained Map mapping is an ordinary executable callable frame. Its
//! receiver and output use the same admitted Map body as a direct source call.
use super::*;
use crate::semantic_facts::RuntimeExecutableSemanticFactView;
use arcweft_core::plan::{
    RuntimeExecutableBodySeed, RuntimeFunctionInputBindingSeed, RuntimeFunctionInputOrigin,
    RuntimeFunctionInputSource, RuntimeFunctionInputTransfer, RuntimeFunctionParameterPassing,
    RuntimeFunctionSemanticRole, RuntimeFunctionSiteBodyKind, RuntimeFunctionSiteBodySeed,
    RuntimeFunctionSiteDeclarationSeed, RuntimeFunctionSiteSeedId, RuntimeGeneratedFunctionRole,
    RuntimePlanBuilder, RuntimeValueOwnershipRequirement,
};

pub(in crate::flow) fn admit_prefix_functions(
    facts: RuntimeExecutableSemanticFactView<'_>,
    locals: &super::super::control_locals::ControlLocals,
    builder: &mut RuntimePlanBuilder,
) -> Result<BTreeMap<ExprId, RuntimeFunctionSiteSeedId>, RuntimePlanLowerError> {
    let error = |message: &str| RuntimePlanLowerError::new(message);
    let mut sites = BTreeMap::new();
    for (owner, frame) in &locals.map_frames {
        let Some(RuntimeResolvedCallDispatch::Static(
            RuntimeResolvedStaticCallTarget::StandardMapPrefix(map),
        )) = facts.call(*owner).map(|call| call.dispatch())
        else {
            return Err(error("Map function frame has no selected prefix"));
        };
        let Some(RuntimeTypeShape::Function {
            parameters,
            result,
            contract,
        }) = facts
            .expression_type(*owner)
            .map(RuntimeNormalizedType::shape)
        else {
            return Err(error("Map prefix function contract is absent"));
        };
        let [receiver] = parameters.as_ref() else {
            return Err(error("Map prefix must have one receiver formal"));
        };
        if contract.invocation().variables().next().is_some() {
            return Err(error("Map prefix invocation row is not closed"));
        }
        let effects = RuntimeEffectSet::try_from_effects(contract.invocation().constant_effects())
            .map_err(|cause| RuntimePlanLowerError::new(cause.to_string()))?;
        let mapping = facts
            .expression_type(map.mapping())
            .ok_or_else(|| error("Map mapping type is absent"))?;
        let body_locals = locals
            .maps
            .get(owner)
            .ok_or_else(|| error("Map body local inventory is absent"))?;
        let callable = body_locals
            .callable
            .clone()
            .ok_or_else(|| error("Map callback capture local is absent"))?;
        let definition = facts
            .expression_coordinate(*owner)
            .ok_or_else(|| error("Map prefix has no accepted coordinate"))?
            .runtime_function_definition()
            .map_err(|cause| RuntimePlanLowerError::new(cause.to_string()))?
            .generated_child(RuntimeGeneratedFunctionRole::StandardMapContinuation);
        let callback_origin = facts
            .expression_coordinate(map.mapping())
            .ok_or_else(|| error("Map mapping has no accepted coordinate"))?
            .runtime_function_definition()
            .map_err(|cause| RuntimePlanLowerError::new(cause.to_string()))?;
        let site = builder
            .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                definition,
                role: RuntimeFunctionSemanticRole::Closure,
                function_type: None,
                inputs: Box::new([
                    RuntimeFunctionInputBindingSeed {
                        transfer: RuntimeFunctionInputTransfer::Transferred(
                            arcweft_core::plan::RuntimeFunctionCaptureMode::Move,
                        ),
                        origin: RuntimeFunctionInputOrigin::EvaluatedResult(callback_origin),
                        source: RuntimeFunctionInputSource::Capture { position: 0 },
                        input_local: callable.clone(),
                        pattern: bind_seed(mapping, callable.clone()),
                        ownership: RuntimeValueOwnershipRequirement::Owned,
                        unrestricted_bindings: Box::new([]),
                    },
                    RuntimeFunctionInputBindingSeed {
                        transfer: RuntimeFunctionInputTransfer::Formal,
                        origin: RuntimeFunctionInputOrigin::Parameter(
                            definition.generated_parameter(0),
                        ),
                        source: RuntimeFunctionInputSource::Parameter {
                            position: 0,
                            passing: RuntimeFunctionParameterPassing::Value,
                        },
                        input_local: frame.source.clone(),
                        pattern: bind_seed(receiver, frame.source.clone()),
                        ownership: RuntimeValueOwnershipRequirement::Owned,
                        unrestricted_bindings: Box::new([]),
                    },
                ]),
                result: result.identity(),
                body_kind: RuntimeFunctionSiteBodyKind::Executable,
                effects: effects.clone(),
            })
            .map_err(|cause| RuntimePlanLowerError::new(cause.to_string()))?;
        let (input, output) = map
            .family()
            .item_types(receiver, result)
            .ok_or_else(|| error("Map prefix family disagrees with its closed signature"))?;
        let body = lower_standard_map_body(
            map.family(),
            map.iteration(),
            MapFlowInput {
                receiver,
                result,
                mapping,
                input,
                output,
                locals: body_locals.clone(),
                callable: callable.clone(),
                source: local_seed(receiver, frame.source.clone(), RuntimeLocalReadMode::Move),
            },
            frame.result.clone(),
        )?;
        builder
            .define_function_site_seed(
                &site,
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects,
                    ops: vec![
                        RuntimeFlowOpSeed::StandardMap {
                            family: map.family().into(),
                            source: frame.source.clone(),
                            mapping: callable,
                            result: frame.result.clone(),
                            body,
                        },
                        RuntimeFlowOpSeed::ReturnExpr(local_seed(
                            result,
                            frame.result.clone(),
                            RuntimeLocalReadMode::Move,
                        )),
                    ]
                    .into_boxed_slice(),
                }),
            )
            .map_err(|cause| RuntimePlanLowerError::new(cause.to_string()))?;
        sites.insert(*owner, site);
    }
    Ok(sites)
}
