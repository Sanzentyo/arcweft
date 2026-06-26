use crate::awbc_lower::expr::AwbcExprLowerer;
use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::{
    AwbcInventory, source_handler_kind, source_policy, stream_op_family,
};
use crate::awbc_lower::{table_index, table_range_len};
use arcweft_core::awbc::schema::{
    AwbcBindMode, AwbcBlock, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind,
    AwbcInstruction, AwbcIntrinsic, AwbcIntrinsicId, AwbcRegisterId, AwbcSafePointKind,
    AwbcSourceHandler, AwbcSourcePlan, AwbcSourcePlanId, AwbcStreamPlan, AwbcStreamPlanId,
    AwbcTableRange, AwbcTerminator,
};
use arcweft_core::plan::RuntimePlan;
use arcweft_core::source::{SourceHandlerPlan, SourceOp};
use arcweft_core::stream::{StreamOp, StreamPlan};

/// Lowers source and stream declarations after flow functions exist.
pub struct AwbcSourceStreamLowerer<'a> {
    inventory: &'a mut AwbcInventory,
}

impl<'a> AwbcSourceStreamLowerer<'a> {
    pub fn new(inventory: &'a mut AwbcInventory) -> Self {
        Self { inventory }
    }

    pub fn lower_plan(&mut self, plan: &RuntimePlan) {
        for stream in &plan.stream_plans {
            self.lower_stream(stream);
        }
        for source in &plan.source_plans {
            let public_id = self.inventory.intern_string(&source.id.0);
            let open = self.inventory.synthetic_empty_function("source.open");
            let handlers = source
                .handlers
                .iter()
                .map(|handler| AwbcSourceHandler {
                    kind: source_handler_kind(handler),
                    function: self.lower_source_handler(handler),
                })
                .collect();
            let item_type = self.inventory.dynamic_ty();
            let error_type = self.inventory.dynamic_ty();
            let policy = source_policy(&source.policy);
            self.inventory.program.source_plans.push(AwbcSourcePlan {
                public_id,
                item_type,
                error_type,
                open,
                policy,
                handlers,
            });
        }
    }

    fn lower_stream(&mut self, stream: &StreamPlan) {
        let function = self.lower_stream_function(stream);
        let public_id = self.inventory.intern_string(&stream.id.0);
        let item_type = self.inventory.dynamic_ty();
        let error_type = self.inventory.dynamic_ty();
        self.inventory.program.stream_plans.push(AwbcStreamPlan {
            public_id,
            item_type,
            error_type,
            transform: function,
        });
    }

    fn lower_stream_function(&mut self, stream: &StreamPlan) -> AwbcFunctionId {
        let mut frame = FrameBuilder::new();
        let body_start = table_index(self.inventory.program.instructions.len());
        for op in &stream.ops {
            self.lower_stream_op(&mut frame, op);
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
        let layout = self
            .inventory
            .intern_frame_layout(format!("stream:{}", stream.id.0), frame.finish());
        let signature = self.inventory.intern_unit_signature();
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

    fn lower_stream_op(&mut self, frame: &mut FrameBuilder, op: &StreamOp) {
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
                    .push_instruction(AwbcInstruction::StreamYield {
                        stream: AwbcStreamPlanId(0),
                        value,
                    });
            }
            StreamOp::Close { source } => {
                let _ = AwbcExprLowerer::new(self.inventory, frame, "stream.close").lower(source);
                self.push_intrinsic_call(stream_op_family(op), Vec::new());
            }
            StreamOp::If {
                condition,
                then_ops,
                else_ops,
            } => {
                let _ = AwbcExprLowerer::new(self.inventory, frame, "stream.if").lower(condition);
                for op in then_ops {
                    self.lower_stream_op(frame, op);
                }
                for op in else_ops {
                    self.lower_stream_op(frame, op);
                }
            }
            StreamOp::Match { scrutinee, arms } => {
                let _ =
                    AwbcExprLowerer::new(self.inventory, frame, "stream.match").lower(scrutinee);
                for arm in arms {
                    for op in &arm.ops {
                        self.lower_stream_op(frame, op);
                    }
                }
            }
            StreamOp::ForNext { source, body, .. } => {
                let _ =
                    AwbcExprLowerer::new(self.inventory, frame, "stream.for_next").lower(source);
                for op in body {
                    self.lower_stream_op(frame, op);
                }
            }
            StreamOp::Return => {}
            StreamOp::Noop => {
                self.inventory.push_instruction(AwbcInstruction::Nop);
            }
        }
    }

    fn lower_source_handler(&mut self, handler: &SourceHandlerPlan) -> AwbcFunctionId {
        let mut frame = FrameBuilder::new();
        let body_start = table_index(self.inventory.program.instructions.len());
        let ops = match handler {
            SourceHandlerPlan::Item { ops, .. }
            | SourceHandlerPlan::Error { ops, .. }
            | SourceHandlerPlan::Progress { ops, .. }
            | SourceHandlerPlan::Disconnected { ops }
            | SourceHandlerPlan::PermissionRevoked { ops }
            | SourceHandlerPlan::End { ops } => ops,
        };
        for op in ops {
            self.lower_source_op(&mut frame, op);
        }
        let owner = AwbcFunctionId(table_index(self.inventory.program.functions.len()));
        let block = self.inventory.push_block(AwbcBlock {
            owner,
            instructions: AwbcTableRange::new(
                body_start,
                table_range_len(body_start, self.inventory.program.instructions.len()),
            ),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::Return,
            source_map: None,
        });
        let layout = self
            .inventory
            .intern_frame_layout("source.handler".to_owned(), frame.finish());
        let signature = self.inventory.intern_unit_signature();
        self.inventory.push_function(
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
        )
    }

    fn lower_source_op(&mut self, frame: &mut FrameBuilder, op: &SourceOp) {
        match op {
            SourceOp::Yield(expr) => {
                let value = AwbcExprLowerer::new(self.inventory, frame, "source.yield").lower(expr);
                self.inventory
                    .push_instruction(AwbcInstruction::StreamYield {
                        stream: AwbcStreamPlanId(0),
                        value,
                    });
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
            SourceOp::Close(_) => {
                self.inventory
                    .push_instruction(AwbcInstruction::SourceClose {
                        source: AwbcSourcePlanId(0),
                    });
            }
            SourceOp::Noop => {
                self.inventory.push_instruction(AwbcInstruction::Nop);
            }
        }
    }

    fn intrinsic(&mut self, label: &str) -> AwbcIntrinsicId {
        let signature = self.inventory.intern_unit_signature();
        let id = AwbcIntrinsicId(table_index(self.inventory.program.intrinsics.len()));
        let public_id = self.inventory.intern_string(label);
        self.inventory.program.intrinsics.push(AwbcIntrinsic {
            public_id,
            registry_code: 0,
            signature,
            revision: 1,
        });
        id
    }

    fn push_intrinsic_call(&mut self, label: &str, args: Vec<AwbcRegisterId>) {
        let intrinsic = self.intrinsic(label);
        self.inventory
            .push_instruction(AwbcInstruction::CallIntrinsic {
                dst: None,
                intrinsic,
                args,
            });
    }
}
