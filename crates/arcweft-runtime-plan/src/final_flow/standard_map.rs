//! Source Map callbacks enter the ordinary same-fiber callable continuation.
//! Operands and iteration values stay in admitted environment locals, so the
//! existing ownership, rollback, operation budget and cleanup rules apply.

use super::*;
#[path = "standard_map/function.rs"]
mod function;
use crate::semantic_facts::{
    RuntimeStandardMapCall, RuntimeStandardMapCollection, RuntimeStandardMapFamily,
    RuntimeStandardMapOperandOrder,
};
use arcweft_core::plan::{RuntimeAssignmentSeed, RuntimeCallArgumentSeed, RuntimeMutablePlaceSeed};
use arcweft_core::value::{
    RuntimeCallTarget, RuntimePlaceDisplacement, RuntimePlaceInitialization,
};
pub(super) use function::admit_prefix_functions;

pub(super) struct MapFlowInput<'a> {
    receiver: &'a RuntimeNormalizedType,
    result: &'a RuntimeNormalizedType,
    mapping: &'a RuntimeNormalizedType,
    input: &'a RuntimeNormalizedType,
    output: &'a RuntimeNormalizedType,
    locals: super::control_locals::MapLocalSeeds,
    callable: RuntimeLocalSeedId,
    source: RuntimeExprSeed,
}

impl MapFlowInput<'_> {
    fn callback(&self, item: RuntimeLocalSeedId, mapped: RuntimeLocalSeedId) -> RuntimeFlowOpSeed {
        RuntimeFlowOpSeed::ApplyFunction {
            callee: local_seed(
                self.mapping,
                self.callable.clone(),
                RuntimeLocalReadMode::Copy,
            ),
            args: Box::new([RuntimeCallArgumentSeed::new(
                local_seed(self.input, item, RuntimeLocalReadMode::Move),
                RuntimeCallArgumentMode::Value,
                0,
            )]),
            result: bind_seed(self.output, mapped),
        }
    }
}

impl FinalFlowLowerer<'_> {
    pub(super) fn lower_standard_map_value(
        &mut self,
        owner: ExprId,
        map: RuntimeStandardMapCall,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let operands = match map.order() {
            RuntimeStandardMapOperandOrder::MappingThenReceiver => [map.mapping(), map.receiver()],
            RuntimeStandardMapOperandOrder::ReceiverThenMapping => [map.receiver(), map.mapping()],
        };
        for child in operands {
            if !overrides.contains_key(&child) {
                return self.lower_flow_value_with_overrides(
                    child,
                    RuntimeFlowValueContinuation::Compose {
                        owner,
                        child,
                        overrides,
                        outer: Box::new(continuation),
                    },
                    BTreeMap::new(),
                );
            }
        }
        let receiver = self.expression_source_type(map.receiver())?.clone();
        let result = self.expression_source_type(owner)?.clone();
        let mapping = self.expression_source_type(map.mapping())?.clone();
        let (input, output) = map.item_types(&receiver, &result).ok_or_else(|| {
            RuntimePlanLowerError::new("Map source/result do not match the accepted family")
        })?;
        let locals = self.control.maps.get(&owner).cloned().ok_or_else(|| {
            RuntimePlanLowerError::new("Map has no admitted invocation temporaries")
        })?;
        let callable = locals.callable.clone();
        let mapping_value = overrides
            .get(&map.mapping())
            .cloned()
            .ok_or_else(|| RuntimePlanLowerError::new("Map callback operand was not retained"))?;
        let source = overrides
            .get(&map.receiver())
            .cloned()
            .ok_or_else(|| RuntimePlanLowerError::new("Map receiver operand was not retained"))?;
        let source_local = self
            .control
            .expression_values
            .get(&map.receiver())
            .cloned()
            .ok_or_else(|| RuntimePlanLowerError::new("Map source local is absent"))?;
        let result_local = self
            .control
            .expression_values
            .get(&owner)
            .cloned()
            .ok_or_else(|| RuntimePlanLowerError::new("Map result local is absent"))?;
        let prepare_mapping = RuntimeFlowOpSeed::Let {
            pattern: bind_seed(&mapping, callable.clone()),
            expr: mapping_value,
        };
        // A control-transferring receiver may already have published precisely
        // this admitted value slot. Preserve that declaration and its ownership.
        let prepare_source = (source
            != local_seed(&receiver, source_local.clone(), RuntimeLocalReadMode::Move))
        .then(|| RuntimeFlowOpSeed::Let {
            pattern: bind_seed(&receiver, source_local.clone()),
            expr: source,
        });
        let mut ops = match map.order() {
            RuntimeStandardMapOperandOrder::MappingThenReceiver => std::iter::once(prepare_mapping)
                .chain(prepare_source)
                .collect::<Vec<_>>(),
            RuntimeStandardMapOperandOrder::ReceiverThenMapping => prepare_source
                .into_iter()
                .chain(std::iter::once(prepare_mapping))
                .collect::<Vec<_>>(),
        };
        let input = MapFlowInput {
            receiver: &receiver,
            result: &result,
            mapping: &mapping,
            input,
            output,
            locals,
            callable: callable.clone(),
            source: local_seed(&receiver, source_local.clone(), RuntimeLocalReadMode::Move),
        };
        let body =
            lower_standard_map_body(map.family(), map.collection(), input, result_local.clone())?;
        ops.push(RuntimeFlowOpSeed::StandardMap(
            arcweft_core::plan::RuntimeStandardMapControlSeed::collection(
                map.family().into(),
                source_local,
                callable,
                result_local.clone(),
                body,
            ),
        ));
        ops.extend(self.apply_value_continuation(
            local_seed(&result, result_local, RuntimeLocalReadMode::Move),
            continuation,
        )?);
        Ok(ops)
    }
}

pub(super) fn lower_standard_map_body(
    family: RuntimeStandardMapFamily,
    collection: Option<&RuntimeStandardMapCollection>,
    input: MapFlowInput<'_>,
    result: RuntimeLocalSeedId,
) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
    match family {
        RuntimeStandardMapFamily::Vec
        | RuntimeStandardMapFamily::Seq
        | RuntimeStandardMapFamily::Slice
        | RuntimeStandardMapFamily::Array => lower_sequence_map_value(collection, input, result),
        RuntimeStandardMapFamily::Option | RuntimeStandardMapFamily::Result => {
            lower_variant_map_value(input, result)
        }
    }
}

fn lower_sequence_map_value(
    collection: Option<&RuntimeStandardMapCollection>,
    input: MapFlowInput<'_>,
    result_local: RuntimeLocalSeedId,
) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
    let MapFlowInput {
        result,
        output,
        ref locals,
        ..
    } = input;
    let mut ops = Vec::new();
    let collection = collection.ok_or_else(|| {
        RuntimePlanLowerError::new("sequence Map has no admitted iterator contract")
    })?;
    let iteration = collection.iteration();
    let accumulator_type = collection.accumulator();
    let accumulator = locals
        .accumulator
        .clone()
        .unwrap_or_else(|| result_local.clone());
    let iterator = locals
        .iterator
        .clone()
        .ok_or_else(|| RuntimePlanLowerError::new("Map iterator local is absent"))?;
    let next_iterator = locals
        .next_iterator
        .clone()
        .ok_or_else(|| RuntimePlanLowerError::new("Map next iterator local is absent"))?;
    let item = locals.item.clone();
    let mapped = locals.mapped.clone();
    ops.extend([
        RuntimeFlowOpSeed::Let {
            pattern: RuntimePatternSeed::new(
                accumulator_type.identity(),
                RuntimePatternSeedKind::Bind {
                    local: accumulator.clone(),
                    mutable: true,
                },
            ),
            expr: RuntimeExprSeed::new(
                accumulator_type.identity(),
                RuntimeExprSeedKind::BracketSeq(Box::new([])),
            ),
        },
        RuntimeFlowOpSeed::Let {
            pattern: RuntimePatternSeed::new(
                iteration.iterator().identity(),
                RuntimePatternSeedKind::Bind {
                    local: iterator.clone(),
                    mutable: true,
                },
            ),
            expr: map_intrinsic(
                iteration.iterator(),
                RuntimeIntrinsic::builtin_iterator_into_iter(iteration.family()),
                input.source.clone(),
            ),
        },
    ]);
    let pattern = RuntimePatternSeed::new(
        iteration.step().identity(),
        RuntimePatternSeedKind::Tuple(Box::new([
            bind_seed(iteration.iterator(), next_iterator.clone()),
            normalized_variant_binding_pattern_seed(iteration.next_value(), 0, Some(item.clone()))
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?,
        ])),
    );
    let unit = arcweft_core::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    ops.push(RuntimeFlowOpSeed::WhileLet {
        pattern,
        expr: map_intrinsic(
            iteration.step(),
            RuntimeIntrinsic::CoreIterNext,
            local_seed(
                iteration.iterator(),
                iterator.clone(),
                RuntimeLocalReadMode::Move,
            ),
        ),
        guard: None,
        body: vec![
            RuntimeFlowOpSeed::Assign {
                place: RuntimeAssignmentSeed {
                    place: RuntimeMutablePlaceSeed::Local(iterator),
                    displacement: RuntimePlaceDisplacement::Reachable {
                        initialization: RuntimePlaceInitialization::Uninitialized,
                        fields: Box::new([]),
                    },
                },
                value: local_seed(
                    iteration.iterator(),
                    next_iterator,
                    RuntimeLocalReadMode::Move,
                ),
            },
            input.callback(item, mapped.clone()),
            RuntimeFlowOpSeed::Let {
                pattern: RuntimePatternSeed::new(unit, RuntimePatternSeedKind::Discard),
                expr: RuntimeExprSeed::new(
                    unit,
                    RuntimeExprSeedKind::SequencePush {
                        place: RuntimeMutablePlaceSeed::Local(accumulator.clone()),
                        value: Box::new(local_seed(output, mapped, RuntimeLocalReadMode::Move)),
                    },
                ),
            },
        ],
    });
    if locals.accumulator.is_some() {
        ops.push(RuntimeFlowOpSeed::Let {
            pattern: bind_seed(result, result_local),
            expr: RuntimeExprSeed::new(
                result.identity(),
                RuntimeExprSeedKind::ArrayFromVec {
                    value: Box::new(local_seed(
                        accumulator_type,
                        accumulator,
                        RuntimeLocalReadMode::Move,
                    )),
                },
            ),
        });
    }
    Ok(ops)
}

fn lower_variant_map_value(
    input: MapFlowInput<'_>,
    result_local: RuntimeLocalSeedId,
) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
    let MapFlowInput {
        receiver,
        result,
        output,
        ref locals,
        ..
    } = input;
    let mut ops = Vec::new();
    let item = locals.item.clone();
    let mapped = locals.mapped.clone();
    let mut success = vec![input.callback(item.clone(), mapped.clone())];
    let value = normalized_variant_expression_seed(
        result,
        0,
        Some(local_seed(output, mapped, RuntimeLocalReadMode::Move)),
    )
    .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
    success.push(RuntimeFlowOpSeed::Break {
        depth: 0,
        value: Some(value),
    });
    let (residual_pattern, residual_value) = match receiver.shape() {
        RuntimeTypeShape::Option { .. } => (
            normalized_variant_binding_pattern_seed(receiver, 1, None),
            normalized_variant_expression_seed(result, 1, None),
        ),
        RuntimeTypeShape::Result { error, .. } => {
            let residual = locals
                .residual
                .clone()
                .ok_or_else(|| RuntimePlanLowerError::new("Result Map residual local is absent"))?;
            (
                normalized_variant_binding_pattern_seed(receiver, 1, Some(residual.clone())),
                normalized_variant_expression_seed(
                    result,
                    1,
                    Some(local_seed(error, residual, RuntimeLocalReadMode::Move)),
                ),
            )
        }
        _ => {
            return Err(RuntimePlanLowerError::new(
                "variant Map source has a different family",
            ));
        }
    };
    ops.push(RuntimeFlowOpSeed::Match {
        scrutinee: input.source,
        arms: vec![
            RuntimeFlowMatchArmSeed {
                pattern: normalized_variant_binding_pattern_seed(receiver, 0, Some(item))
                    .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?,
                guard: None,
                ops: success,
            },
            RuntimeFlowMatchArmSeed {
                pattern: residual_pattern
                    .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?,
                guard: None,
                ops: vec![RuntimeFlowOpSeed::Break {
                    depth: 0,
                    value: Some(
                        residual_value
                            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?,
                    ),
                }],
            },
        ],
    });
    // The existing value-producing loop owns the join. Each selected
    // variant transfers its value through Break before its branch scope
    // is destroyed; no branch-local result escapes its lifetime.
    Ok(vec![RuntimeFlowOpSeed::Loop {
        result: Some(bind_seed(result, result_local)),
        body: ops,
    }])
}

fn map_intrinsic(
    result: &RuntimeNormalizedType,
    intrinsic: RuntimeIntrinsic,
    value: RuntimeExprSeed,
) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        result.identity(),
        RuntimeExprSeedKind::Call {
            callee: RuntimeCallTarget::Intrinsic(intrinsic),
            args: Box::new([RuntimeCallArgumentSeed::new(
                value,
                RuntimeCallArgumentMode::Value,
                0,
            )]),
        },
    )
}
