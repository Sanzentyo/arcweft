//! Source Map callbacks enter the ordinary same-fiber callable continuation.
//! Operands and iteration values stay in admitted environment locals, so the
//! existing ownership, rollback, operation budget and cleanup rules apply.

use super::*;
use crate::semantic_facts::{
    RuntimeStandardMapCall, RuntimeStandardMapFamily, RuntimeStandardMapOperandOrder,
};
use arcweft_core::plan::{
    RuntimeAssignmentSeed, RuntimeCallArgumentSeed, RuntimeMutablePlaceSeed, RuntimePatternRestSeed,
};
use arcweft_core::value::{
    RuntimeCallTarget, RuntimePlaceDisplacement, RuntimePlaceInitialization,
};

struct MapFlowInput<'a> {
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
        let callable = locals
            .callable
            .clone()
            .ok_or_else(|| RuntimePlanLowerError::new("Map callback local is absent"))?;
        let mapping_value = overrides
            .get(&map.mapping())
            .cloned()
            .ok_or_else(|| RuntimePlanLowerError::new("Map callback operand was not retained"))?;
        let source = overrides
            .get(&map.receiver())
            .cloned()
            .ok_or_else(|| RuntimePlanLowerError::new("Map receiver operand was not retained"))?;
        let mut ops = vec![RuntimeFlowOpSeed::Let {
            pattern: bind_seed(&mapping, callable.clone()),
            expr: mapping_value,
        }];
        let input = MapFlowInput {
            receiver: &receiver,
            result: &result,
            mapping: &mapping,
            input,
            output,
            locals,
            callable,
            source,
        };
        ops.extend(match map.family() {
            RuntimeStandardMapFamily::Vec
            | RuntimeStandardMapFamily::Seq
            | RuntimeStandardMapFamily::Slice => {
                self.lower_sequence_map_value(owner, &map, input, continuation)?
            }
            RuntimeStandardMapFamily::Array => self.lower_array_map_value(input, continuation)?,
            RuntimeStandardMapFamily::Option | RuntimeStandardMapFamily::Result => {
                self.lower_variant_map_value(input, continuation)?
            }
        });
        Ok(ops)
    }

    fn lower_sequence_map_value(
        &mut self,
        owner: ExprId,
        map: &RuntimeStandardMapCall,
        input: MapFlowInput<'_>,
        continuation: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let MapFlowInput {
            result,
            output,
            ref locals,
            ..
        } = input;
        let mut ops = Vec::new();
        let iteration = map.iteration().ok_or_else(|| {
            RuntimePlanLowerError::new("sequence Map has no admitted iterator contract")
        })?;
        let iterator = locals
            .iterator
            .clone()
            .ok_or_else(|| RuntimePlanLowerError::new("Map iterator local is absent"))?;
        let next_iterator = locals
            .next_iterator
            .clone()
            .ok_or_else(|| RuntimePlanLowerError::new("Map next iterator local is absent"))?;
        let item = locals
            .items
            .first()
            .cloned()
            .ok_or_else(|| RuntimePlanLowerError::new("Map item local is absent"))?;
        let mapped = locals
            .results
            .first()
            .cloned()
            .ok_or_else(|| RuntimePlanLowerError::new("Map result item local is absent"))?;
        let accumulator = self
            .control
            .expression_values
            .get(&owner)
            .cloned()
            .ok_or_else(|| RuntimePlanLowerError::new("Map result local is absent"))?;
        ops.extend([
            RuntimeFlowOpSeed::Let {
                pattern: RuntimePatternSeed::new(
                    result.identity(),
                    RuntimePatternSeedKind::Bind {
                        local: accumulator.clone(),
                        mutable: true,
                    },
                ),
                expr: RuntimeExprSeed::new(
                    result.identity(),
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
                expr: Self::map_intrinsic(
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
                normalized_variant_binding_pattern_seed(
                    iteration.next_value(),
                    0,
                    Some(item.clone()),
                )
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?,
            ])),
        );
        let unit = arcweft_core::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
        ops.push(RuntimeFlowOpSeed::WhileLet {
            pattern,
            expr: Self::map_intrinsic(
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
        ops.extend(self.apply_value_continuation(
            local_seed(result, accumulator, RuntimeLocalReadMode::Move),
            continuation,
        )?);
        Ok(ops)
    }

    fn lower_array_map_value(
        &mut self,
        input: MapFlowInput<'_>,
        continuation: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let MapFlowInput {
            receiver,
            result,
            output,
            ref locals,
            ..
        } = input;
        let mut ops = Vec::new();
        let RuntimeTypeShape::Array { length, .. } = receiver.shape() else {
            return Err(RuntimePlanLowerError::new(
                "Array Map source is not an Array",
            ));
        };
        let length = length
            .constant()
            .and_then(|length| usize::try_from(length).ok())
            .ok_or_else(|| RuntimePlanLowerError::new("Array Map length is not closed"))?;
        if locals.items.len() != length || locals.results.len() != length {
            return Err(RuntimePlanLowerError::new(
                "Array Map local inventory does not retain its exact length",
            ));
        }
        ops.push(RuntimeFlowOpSeed::Let {
            pattern: RuntimePatternSeed::new(
                receiver.identity(),
                RuntimePatternSeedKind::Sequence {
                    items: locals
                        .items
                        .iter()
                        .cloned()
                        .map(|item| bind_seed(input.input, item))
                        .collect(),
                    rest: RuntimePatternRestSeed::Exact,
                },
            ),
            expr: input.source.clone(),
        });
        for (item, mapped) in locals.items.iter().zip(&locals.results) {
            ops.push(input.callback(item.clone(), mapped.clone()));
        }
        let value = RuntimeExprSeed::new(
            result.identity(),
            RuntimeExprSeedKind::BracketSeq(
                locals
                    .results
                    .iter()
                    .cloned()
                    .map(|mapped| local_seed(output, mapped, RuntimeLocalReadMode::Move))
                    .collect(),
            ),
        );
        ops.extend(self.apply_value_continuation(value, continuation)?);
        Ok(ops)
    }

    fn lower_variant_map_value(
        &mut self,
        input: MapFlowInput<'_>,
        continuation: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let MapFlowInput {
            receiver,
            result,
            output,
            ref locals,
            ..
        } = input;
        let mut ops = Vec::new();
        let item = locals
            .items
            .first()
            .cloned()
            .ok_or_else(|| RuntimePlanLowerError::new("variant Map item local is absent"))?;
        let mapped = locals
            .results
            .first()
            .cloned()
            .ok_or_else(|| RuntimePlanLowerError::new("variant Map result local is absent"))?;
        let mut success = vec![input.callback(item.clone(), mapped.clone())];
        let value = normalized_variant_expression_seed(
            result,
            0,
            Some(local_seed(output, mapped, RuntimeLocalReadMode::Move)),
        )
        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        success.extend(self.apply_value_continuation(value, continuation.clone())?);
        let (residual_pattern, residual_value) = match receiver.shape() {
            RuntimeTypeShape::Option { .. } => (
                normalized_variant_binding_pattern_seed(receiver, 1, None),
                normalized_variant_expression_seed(result, 1, None),
            ),
            RuntimeTypeShape::Result { error, .. } => {
                let residual = locals.residual.clone().ok_or_else(|| {
                    RuntimePlanLowerError::new("Result Map residual local is absent")
                })?;
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
                    ops: self.apply_value_continuation(
                        residual_value
                            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?,
                        continuation,
                    )?,
                },
            ],
        });
        Ok(ops)
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
}
