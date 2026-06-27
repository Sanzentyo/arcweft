use crate::awbc_lower::expr::AwbcExprLowerer;
use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::{
    AwbcInventory, AwbcLowerDiagnostic, source_handler_kind, source_policy,
};
use crate::awbc_lower::{table_index, table_range_len};
use arcweft_core::awbc::schema::{
    AwbcBindMode, AwbcBlock, AwbcEffectSetId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId,
    AwbcFunctionKind, AwbcInstruction, AwbcPatternId, AwbcSafePointKind, AwbcSourceHandler,
    AwbcSourcePlan, AwbcSourcePlanId, AwbcStreamPlan, AwbcStreamPlanId, AwbcTableRange,
    AwbcTerminator,
};
use arcweft_core::plan::RuntimePlan;
use arcweft_core::source::{SourceHandlerPlan, SourceId, SourceOp};
use arcweft_core::stream::{StreamOp, StreamPlan};
use arcweft_core::value::{RuntimeExpr, RuntimeValue};
use std::collections::BTreeSet;

/// Lowers source and stream declarations after flow functions exist.
pub struct AwbcSourceStreamLowerer<'a> {
    inventory: &'a mut AwbcInventory,
}

#[derive(Clone, Copy, Debug)]
enum StaticQueueTarget<'a> {
    Source(&'a str),
    Stream(&'a str),
}

#[derive(Clone, Copy, Debug)]
struct LoweredSourceHandler {
    pattern: Option<AwbcPatternId>,
    function: AwbcFunctionId,
}

impl<'a> AwbcSourceStreamLowerer<'a> {
    pub fn new(inventory: &'a mut AwbcInventory) -> Self {
        Self { inventory }
    }

    pub fn lower_plan(&mut self, plan: &RuntimePlan) {
        let stream_start = self.inventory.program.stream_plans.len();
        for (offset, stream) in plan.stream_plans.iter().enumerate() {
            self.inventory.reserve_stream_plan_id(
                stream.id.clone(),
                AwbcStreamPlanId(table_index(stream_start + offset)),
            );
        }
        let source_start = self.inventory.program.source_plans.len();
        for (offset, source) in plan.source_plans.iter().enumerate() {
            self.inventory.reserve_source_plan_id(
                source.id.clone(),
                AwbcSourcePlanId(table_index(source_start + offset)),
            );
        }
        for stream in &plan.stream_plans {
            self.lower_stream(stream);
        }
        for source in &plan.source_plans {
            let source_id = self
                .inventory
                .source_plan_id(&source.id)
                .unwrap_or(AwbcSourcePlanId(0));
            let public_id = self.inventory.intern_string(&source.id.0);
            let open = self.inventory.source_open_function("source.open");
            let handlers = source
                .handlers
                .iter()
                .map(|handler| {
                    let lowered = self.lower_source_handler(source_id, handler);
                    AwbcSourceHandler {
                        kind: source_handler_kind(handler),
                        pattern: lowered.pattern,
                        function: lowered.function,
                    }
                })
                .collect();
            let item_type = self.inventory.dynamic_ty();
            let error_type = self.inventory.dynamic_ty();
            let policy = source_policy(&source.policy);
            self.inventory.push_source_plan(
                source.id.clone(),
                AwbcSourcePlan {
                    public_id,
                    item_type,
                    error_type,
                    open,
                    policy,
                    handlers,
                },
            );
        }
    }

    fn lower_stream(&mut self, stream: &StreamPlan) {
        let stream_id = self
            .inventory
            .stream_plan_id(&stream.id)
            .unwrap_or(AwbcStreamPlanId(0));
        let function = self.lower_stream_function(stream, stream_id);
        let public_id = self.inventory.intern_string(&stream.id.0);
        let item_type = self.inventory.dynamic_ty();
        let error_type = self.inventory.dynamic_ty();
        self.inventory.push_stream_plan(
            stream.id.clone(),
            AwbcStreamPlan {
                public_id,
                item_type,
                error_type,
                transform: function,
            },
        );
    }

    fn lower_stream_function(
        &mut self,
        stream: &StreamPlan,
        stream_id: AwbcStreamPlanId,
    ) -> AwbcFunctionId {
        let mut frame = FrameBuilder::new();
        for parameter in Self::stream_source_parameters(&stream.ops) {
            let name = self.inventory.intern_string(&parameter);
            frame.parameter(&parameter, name, self.inventory.dynamic_ty());
        }
        let body_start = table_index(self.inventory.program.instructions.len());
        for op in &stream.ops {
            self.lower_stream_op(&mut frame, stream_id, op);
        }
        let block_owner = AwbcFunctionId(table_index(self.inventory.program.functions.len()));
        let block = self.inventory.push_block(AwbcBlock {
            owner: block_owner,
            instructions: AwbcTableRange::new(
                body_start,
                table_range_len(body_start, self.inventory.program.instructions.len()),
            ),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        });
        let frame_layout = frame.finish();
        let params = frame_layout
            .slots
            .iter()
            .take_while(|slot| {
                slot.role == arcweft_core::awbc::schema::AwbcFrameSlotRole::Parameter
            })
            .map(|slot| slot.ty)
            .collect();
        let layout = self
            .inventory
            .intern_frame_layout(format!("stream:{}", stream.id.0), frame_layout);
        let signature = self
            .inventory
            .intern_signature(params, None, AwbcEffectSetId(0));
        let public_id = self.inventory.intern_string(&stream.id.0);
        self.inventory.push_function(
            Some(stream.id.0.as_str()),
            AwbcFunction {
                public_id: Some(public_id),
                kind: AwbcFunctionKind::StreamTransform,
                signature,
                frame_layout: layout,
                blocks: AwbcTableRange::new(block.0, 1),
                entry_block: block,
                flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
            },
        )
    }

    fn lower_stream_op(
        &mut self,
        frame: &mut FrameBuilder,
        stream: AwbcStreamPlanId,
        op: &StreamOp,
    ) {
        match op {
            StreamOp::Let { pattern, expr } => {
                let value = AwbcExprLowerer::new(self.inventory, frame, "stream.let").lower(expr);
                let pattern =
                    crate::awbc_lower::pattern::lower_pattern(self.inventory, frame, pattern);
                self.inventory
                    .push_instruction(AwbcInstruction::BindPattern {
                        pattern,
                        value,
                        mode: AwbcBindMode::Declare,
                    });
            }
            StreamOp::Yield { expr } => {
                let value = AwbcExprLowerer::new(self.inventory, frame, "stream.yield").lower(expr);
                self.inventory
                    .push_instruction(AwbcInstruction::StreamYield { stream, value });
            }
            StreamOp::Close { source } => match self.resolve_static_queue_target(source) {
                Some(StaticQueueTarget::Source(source)) => {
                    if let Some(source) =
                        self.inventory.source_plan_id(&SourceId(source.to_owned()))
                    {
                        self.inventory
                            .push_instruction(AwbcInstruction::SourceClose { source });
                    } else {
                        self.lower_unknown_stream_close_target(source);
                    }
                }
                Some(StaticQueueTarget::Stream(target)) => {
                    if let Some(stream) = self
                        .inventory
                        .stream_plan_id(&arcweft_core::stream::StreamRuntimeId(target.to_owned()))
                    {
                        self.inventory
                            .push_instruction(AwbcInstruction::StreamClose { stream });
                    } else {
                        self.lower_unknown_stream_close_target(target);
                    }
                }
                None => {
                    let _ =
                        AwbcExprLowerer::new(self.inventory, frame, "stream.close").lower(source);
                    self.inventory.push_instruction(AwbcInstruction::Nop);
                    self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                        "stream.close",
                        "dynamic stream close target is not representable in AWBC tables",
                    ));
                }
            },
            StreamOp::If {
                condition,
                then_ops,
                else_ops,
            } => {
                let _ = AwbcExprLowerer::new(self.inventory, frame, "stream.if").lower(condition);
                for op in then_ops {
                    self.lower_stream_op(frame, stream, op);
                }
                for op in else_ops {
                    self.lower_stream_op(frame, stream, op);
                }
            }
            StreamOp::Match { scrutinee, arms } => {
                let _ =
                    AwbcExprLowerer::new(self.inventory, frame, "stream.match").lower(scrutinee);
                for arm in arms {
                    for op in &arm.ops {
                        self.lower_stream_op(frame, stream, op);
                    }
                }
            }
            StreamOp::ForNext {
                pattern,
                source,
                body,
            } => {
                let value =
                    AwbcExprLowerer::new(self.inventory, frame, "stream.for_next").lower(source);
                let pattern =
                    crate::awbc_lower::pattern::lower_pattern(self.inventory, frame, pattern);
                self.inventory
                    .push_instruction(AwbcInstruction::BindPattern {
                        pattern,
                        value,
                        mode: AwbcBindMode::Declare,
                    });
                for op in body {
                    self.lower_stream_op(frame, stream, op);
                }
            }
            StreamOp::Return => {}
            StreamOp::Noop => {
                self.inventory.push_instruction(AwbcInstruction::Nop);
            }
        }
    }

    fn stream_source_parameters(ops: &[StreamOp]) -> BTreeSet<String> {
        let mut parameters = BTreeSet::new();
        Self::collect_stream_source_parameters(ops, &mut parameters);
        parameters
    }

    fn collect_stream_source_parameters(ops: &[StreamOp], parameters: &mut BTreeSet<String>) {
        for op in ops {
            match op {
                StreamOp::ForNext { source, body, .. } => {
                    if let RuntimeExpr::Local(name) = source {
                        parameters.insert(name.clone());
                    }
                    Self::collect_stream_source_parameters(body, parameters);
                }
                StreamOp::If {
                    then_ops, else_ops, ..
                } => {
                    Self::collect_stream_source_parameters(then_ops, parameters);
                    Self::collect_stream_source_parameters(else_ops, parameters);
                }
                StreamOp::Match { arms, .. } => {
                    for arm in arms {
                        Self::collect_stream_source_parameters(&arm.ops, parameters);
                    }
                }
                StreamOp::Let { .. }
                | StreamOp::Yield { .. }
                | StreamOp::Close { .. }
                | StreamOp::Return
                | StreamOp::Noop => {}
            }
        }
    }

    fn resolve_static_queue_target<'b>(
        &self,
        expr: &'b RuntimeExpr,
    ) -> Option<StaticQueueTarget<'b>> {
        let target = match expr {
            RuntimeExpr::Value(RuntimeValue::String(target) | RuntimeValue::EntityRef(target))
            | RuntimeExpr::EntityRef(target) => target.as_str(),
            _ => return None,
        };
        if self
            .inventory
            .source_plan_id(&SourceId(target.to_owned()))
            .is_some()
        {
            Some(StaticQueueTarget::Source(target))
        } else {
            Some(StaticQueueTarget::Stream(target))
        }
    }

    fn lower_unknown_stream_close_target(&mut self, target: &str) {
        self.inventory.push_instruction(AwbcInstruction::Nop);
        self.inventory.diagnostic(AwbcLowerDiagnostic::error(
            "stream.close",
            format!("unknown stream/source close target '{target}'"),
        ));
    }

    fn lower_source_handler(
        &mut self,
        source: AwbcSourcePlanId,
        handler: &SourceHandlerPlan,
    ) -> LoweredSourceHandler {
        let mut frame = FrameBuilder::new();
        let body_start = table_index(self.inventory.program.instructions.len());
        let pattern = self.lower_source_handler_pattern(&mut frame, handler);
        let ops = match handler {
            SourceHandlerPlan::Item { ops, .. }
            | SourceHandlerPlan::Error { ops, .. }
            | SourceHandlerPlan::Progress { ops, .. }
            | SourceHandlerPlan::Disconnected { ops }
            | SourceHandlerPlan::PermissionRevoked { ops }
            | SourceHandlerPlan::End { ops } => ops,
        };
        for op in ops {
            self.lower_source_op(&mut frame, source, op);
        }
        let owner = AwbcFunctionId(table_index(self.inventory.program.functions.len()));
        let block = self.inventory.push_block(AwbcBlock {
            owner,
            instructions: AwbcTableRange::new(
                body_start,
                table_range_len(body_start, self.inventory.program.instructions.len()),
            ),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        });
        let layout = self
            .inventory
            .intern_frame_layout("source.handler".to_owned(), frame.finish());
        let signature = if pattern.is_some() {
            let dynamic_ty = self.inventory.dynamic_ty();
            self.inventory
                .intern_signature(vec![dynamic_ty], None, AwbcEffectSetId(0))
        } else {
            self.inventory.intern_unit_signature()
        };
        let function = self.inventory.push_function(
            None,
            AwbcFunction {
                public_id: None,
                kind: AwbcFunctionKind::SourceHandler,
                signature,
                frame_layout: layout,
                blocks: AwbcTableRange::new(block.0, 1),
                entry_block: block,
                flags: AwbcFunctionFlags(
                    AwbcFunctionFlags::DETERMINISTIC | AwbcFunctionFlags::MAY_SUSPEND,
                ),
            },
        );
        LoweredSourceHandler { pattern, function }
    }

    fn lower_source_handler_pattern(
        &mut self,
        frame: &mut FrameBuilder,
        handler: &SourceHandlerPlan,
    ) -> Option<AwbcPatternId> {
        let pattern = match handler {
            SourceHandlerPlan::Item { pattern, .. }
            | SourceHandlerPlan::Error { pattern, .. }
            | SourceHandlerPlan::Progress { pattern, .. } => pattern,
            SourceHandlerPlan::Disconnected { .. }
            | SourceHandlerPlan::PermissionRevoked { .. }
            | SourceHandlerPlan::End { .. } => return None,
        };
        let name = self.inventory.intern_string("$source_event");
        let value = frame.parameter("$source_event", name, self.inventory.dynamic_ty());
        let pattern = crate::awbc_lower::pattern::lower_pattern(self.inventory, frame, pattern);
        self.inventory
            .push_instruction(AwbcInstruction::BindPattern {
                pattern,
                value,
                mode: AwbcBindMode::Declare,
            });
        Some(pattern)
    }

    fn lower_source_op(
        &mut self,
        frame: &mut FrameBuilder,
        source: AwbcSourcePlanId,
        op: &SourceOp,
    ) {
        match op {
            SourceOp::Yield(expr) => {
                let value = AwbcExprLowerer::new(self.inventory, frame, "source.yield").lower(expr);
                self.inventory
                    .push_instruction(AwbcInstruction::SourceYield { source, value });
            }
            SourceOp::Effect(effect) => {
                let effect = self.inventory.intern_effect(effect);
                self.inventory
                    .push_instruction(AwbcInstruction::EmitEffect {
                        effect,
                        args: Vec::new(),
                    });
            }
            SourceOp::SignalWrite(write) => {
                let effect = self.inventory.intern_effect(
                    &arcweft_core::effect::LineEffectRequest::SignalWrite(write.clone()),
                );
                self.inventory
                    .push_instruction(AwbcInstruction::EmitEffect {
                        effect,
                        args: Vec::new(),
                    });
            }
            SourceOp::Log(log) => {
                let effect = self
                    .inventory
                    .intern_effect(&arcweft_core::effect::LineEffectRequest::Log(log.clone()));
                self.inventory
                    .push_instruction(AwbcInstruction::EmitEffect {
                        effect,
                        args: Vec::new(),
                    });
            }
            SourceOp::Close(source) => {
                if let Some(source) = self.inventory.source_plan_id(source) {
                    self.inventory
                        .push_instruction(AwbcInstruction::SourceClose { source });
                } else {
                    self.inventory.push_instruction(AwbcInstruction::Nop);
                    self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                        "source.close",
                        format!("unknown source plan '{}'", source.0),
                    ));
                }
            }
            SourceOp::Noop => {
                self.inventory.push_instruction(AwbcInstruction::Nop);
            }
        }
    }
}
