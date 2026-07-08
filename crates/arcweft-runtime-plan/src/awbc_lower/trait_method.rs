use crate::awbc_lower::expr::AwbcExprLowerer;
use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::{AwbcInventory, AwbcLowerDiagnostic};
use crate::awbc_lower::{table_index, table_range_len};
use arcweft_core::awbc::schema::{
    AwbcBlock, AwbcBlockId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind,
    AwbcInstruction, AwbcRegisterId, AwbcSafePointKind, AwbcTableRange, AwbcTerminator,
    AwbcTraitMethod, AwbcTraitReceiverMode,
};
use arcweft_core::plan::{RuntimePlan, RuntimeReceiverMode, RuntimeTraitMethod};
use arcweft_core::value::RuntimeExpr;

pub(crate) struct AwbcTraitMethodLowerer<'a> {
    inventory: &'a mut AwbcInventory,
}

impl<'a> AwbcTraitMethodLowerer<'a> {
    pub(crate) fn new(inventory: &'a mut AwbcInventory) -> Self {
        Self { inventory }
    }

    pub(crate) fn lower_plan(&mut self, plan: &RuntimePlan) {
        for method in &plan.trait_methods {
            self.lower_method(method);
        }
    }

    fn lower_method(&mut self, method: &RuntimeTraitMethod) {
        let expected = self.inventory.program.trait_methods.len();
        if method.id.0 != expected {
            self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                trait_method_path(method),
                format!(
                    "trait method `{}` has id {}, expected contiguous id {}",
                    method.identity.method_name, method.id.0, expected
                ),
            ));
            return;
        }

        let public_label = trait_method_label(method);
        let owner = self.inventory.reserve_function_slot();
        let mut frame = FrameBuilder::new();
        let dynamic_ty = self.inventory.dynamic_ty();
        for input in &method.input_names {
            let name = self.inventory.intern_string(input);
            frame.parameter(input, name, dynamic_ty);
        }

        let mut body = TraitMethodBodyBuilder::new(self.inventory, owner);
        body.lower_returning_expr(
            self.inventory,
            &mut frame,
            &method.body,
            trait_method_path(method),
        );
        let body = body.finish(self.inventory);
        let layout = self.inventory.intern_frame_layout(
            format!("trait_method.{}:frame", method.id.0),
            frame.finish(),
        );
        let public_id = self.inventory.intern_string(&public_label);
        let signature = self
            .inventory
            .intern_dynamic_value_signature(method.input_names.len());
        let function = self.inventory.replace_function(
            owner,
            AwbcFunction {
                public_id: Some(public_id),
                kind: AwbcFunctionKind::TraitMethod,
                signature,
                frame_layout: layout,
                blocks: body.blocks,
                entry_block: body.entry_block,
                flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
            },
        );
        self.inventory.program.trait_methods.push(AwbcTraitMethod {
            public_id,
            signature,
            function,
            receiver: receiver_mode(method.receiver),
            receiver_state_slot: (method.receiver == RuntimeReceiverMode::MutRef)
                .then_some(AwbcRegisterId(0)),
        });
    }
}

struct TraitMethodBody {
    entry_block: AwbcBlockId,
    blocks: AwbcTableRange,
}

struct TraitMethodBodyBuilder {
    owner: AwbcFunctionId,
    block_start: u32,
    instruction_start: u32,
}

impl TraitMethodBodyBuilder {
    fn new(inventory: &AwbcInventory, owner: AwbcFunctionId) -> Self {
        Self {
            owner,
            block_start: table_index(inventory.program.blocks.len()),
            instruction_start: table_index(inventory.program.instructions.len()),
        }
    }

    fn lower_returning_expr(
        &mut self,
        inventory: &mut AwbcInventory,
        frame: &mut FrameBuilder,
        expr: &RuntimeExpr,
        path: String,
    ) {
        match expr {
            RuntimeExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                let condition =
                    AwbcExprLowerer::new(inventory, frame, path.clone()).lower(condition);
                let then_block = AwbcBlockId(table_index(
                    inventory.program.blocks.len().saturating_add(1),
                ));
                let branch_block = self.close_block(
                    inventory,
                    AwbcTerminator::Branch {
                        condition,
                        then_block,
                        else_block: then_block,
                    },
                    AwbcSafePointKind::None,
                );
                self.lower_returning_expr(inventory, frame, then_expr, format!("{path}.then"));
                let else_block = AwbcBlockId(table_index(inventory.program.blocks.len()));
                patch_branch_else_block(inventory, branch_block, else_block);
                self.lower_returning_expr(inventory, frame, else_expr, format!("{path}.else"));
            }
            RuntimeExpr::Let { name, expr, body } => {
                let value = AwbcExprLowerer::new(inventory, frame, path.clone()).lower(expr);
                let local =
                    frame.local(name, inventory.intern_string(name), inventory.dynamic_ty());
                inventory.push_instruction(AwbcInstruction::Move {
                    dst: local,
                    src: value,
                });
                self.lower_returning_expr(inventory, frame, body, format!("{path}.let.{name}"));
            }
            RuntimeExpr::AssignField {
                target,
                field,
                expr,
                body,
            } => {
                let value = AwbcExprLowerer::new(inventory, frame, path.clone()).lower(expr);
                if let RuntimeExpr::Local(name) = target.as_ref() {
                    if let Some(target) = frame.register_for_local(name) {
                        let field = inventory.intern_string(field);
                        inventory.push_instruction(AwbcInstruction::AssignField {
                            target,
                            field,
                            value,
                        });
                    } else {
                        inventory.diagnostic(AwbcLowerDiagnostic::error(
                            path.clone(),
                            format!("field assignment target `{name}` is not a local register"),
                        ));
                    }
                } else {
                    inventory.diagnostic(AwbcLowerDiagnostic::error(
                        path.clone(),
                        "field assignment lowering requires a local receiver target",
                    ));
                }
                self.lower_returning_expr(
                    inventory,
                    frame,
                    body,
                    format!("{path}.assign_field.{field}"),
                );
            }
            _ => {
                let value = AwbcExprLowerer::new(inventory, frame, path).lower(expr);
                self.close_block(
                    inventory,
                    AwbcTerminator::Return { value: Some(value) },
                    AwbcSafePointKind::Return,
                );
            }
        }
    }

    fn close_block(
        &mut self,
        inventory: &mut AwbcInventory,
        terminator: AwbcTerminator,
        safe_point: AwbcSafePointKind,
    ) -> AwbcBlockId {
        let block = AwbcBlockId(table_index(inventory.program.blocks.len()));
        let instruction_len =
            table_range_len(self.instruction_start, inventory.program.instructions.len());
        let safe_point = if block.0 == self.block_start {
            AwbcSafePointKind::CallableBoundary
        } else {
            safe_point
        };
        inventory.push_block(AwbcBlock {
            owner: self.owner,
            instructions: AwbcTableRange::new(self.instruction_start, instruction_len),
            terminator,
            safe_point,
            source_map: None,
        });
        self.instruction_start = table_index(inventory.program.instructions.len());
        block
    }

    fn finish(self, inventory: &mut AwbcInventory) -> TraitMethodBody {
        TraitMethodBody {
            entry_block: AwbcBlockId(self.block_start),
            blocks: AwbcTableRange::new(
                self.block_start,
                table_range_len(self.block_start, inventory.program.blocks.len()),
            ),
        }
    }
}

fn receiver_mode(mode: RuntimeReceiverMode) -> AwbcTraitReceiverMode {
    match mode {
        RuntimeReceiverMode::Owned => AwbcTraitReceiverMode::Owned,
        RuntimeReceiverMode::SharedRef => AwbcTraitReceiverMode::SharedRef,
        RuntimeReceiverMode::MutRef => AwbcTraitReceiverMode::MutRef,
    }
}

fn trait_method_label(method: &RuntimeTraitMethod) -> String {
    let trait_name = method.identity.trait_name.as_deref().unwrap_or("inherent");
    format!(
        "trait.{trait_name}.impl.{}.{}",
        method.identity.impl_id, method.identity.method_name
    )
}

fn trait_method_path(method: &RuntimeTraitMethod) -> String {
    format!("trait_method#{}", method.id.0)
}

fn patch_branch_else_block(
    inventory: &mut AwbcInventory,
    branch_block: AwbcBlockId,
    else_block: AwbcBlockId,
) {
    if let Some(AwbcBlock {
        terminator: AwbcTerminator::Branch {
            else_block: target, ..
        },
        ..
    }) = inventory.program.blocks.get_mut(branch_block.index())
    {
        *target = else_block;
    }
}
