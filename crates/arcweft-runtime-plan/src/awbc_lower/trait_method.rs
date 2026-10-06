use crate::awbc_lower::expr::AwbcExprLowerer;
use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::{AwbcInventory, AwbcLowerDiagnostic};
use crate::awbc_lower::pattern::{admitted_local_type, admitted_plan_type};
use crate::awbc_lower::{table_index, table_range_len};
use arcweft_core::awbc::schema::{
    AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcFunction, AwbcFunctionFlag, AwbcFunctionFlags,
    AwbcFunctionId, AwbcFunctionInputOwnership, AwbcFunctionKind, AwbcInstruction, AwbcRegisterId,
    AwbcSafePointKind, AwbcTableRange, AwbcTerminator, AwbcTraitMethod, AwbcTraitMethodId,
    AwbcTraitReceiverMode,
};
use arcweft_core::plan::{RuntimePlan, RuntimeReceiverMode, RuntimeTraitMethod};
use arcweft_core::value::RuntimeExpr;

pub(crate) struct AwbcTraitMethodLowerer<'a, 'plan> {
    inventory: &'a mut AwbcInventory,
    plan: &'plan RuntimePlan,
}

impl<'a, 'plan> AwbcTraitMethodLowerer<'a, 'plan> {
    pub(crate) fn new(inventory: &'a mut AwbcInventory, plan: &'plan RuntimePlan) -> Self {
        Self { inventory, plan }
    }

    pub(crate) fn lower_plan(&mut self) {
        for method in self.plan.trait_methods() {
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
        let mut parameters = Vec::with_capacity(method.input_locals.len());
        for input in &method.input_locals {
            let ty = admitted_local_type(self.inventory, self.plan, *input);
            frame.parameter(*input, ty);
            parameters.push(ty);
        }

        let mut body = TraitMethodBodyBuilder::new(self.inventory, owner);
        body.lower_returning_expr(
            self.inventory,
            &mut frame,
            self.plan,
            &method.body,
            trait_method_path(method),
        );
        let body = body.finish(self.inventory);
        let layout = self.inventory.intern_frame_layout(
            format!("trait_method.{}:frame", method.id.0),
            frame.finish(),
        );
        let public_id = self.inventory.intern_string(&public_label);
        let result = admitted_plan_type(self.inventory, self.plan, method.body.ty());
        let signature =
            self.inventory
                .intern_signature(parameters, Some(result), AwbcEffectSetId(0));
        let function = self.inventory.replace_function(
            owner,
            AwbcFunction {
                semantic_role: arcweft_core::plan::RuntimeFunctionSemanticRole::Ordinary,
                public_id: Some(public_id),
                kind: AwbcFunctionKind::TraitMethod,
                signature,
                type_context: None,
                input_ownership: vec![
                    AwbcFunctionInputOwnership::default();
                    method.input_locals.len()
                ],
                frame_layout: layout,
                blocks: body.blocks,
                entry_block: body.entry_block,
                flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
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
        self.inventory
            .record_trait_method(method.id, AwbcTraitMethodId(table_index(expected)));
    }
}

struct TraitMethodBody {
    entry_block: AwbcBlockId,
    blocks: AwbcTableRange,
}

struct TraitMethodBodyBuilder {
    block_start: u32,
}

impl TraitMethodBodyBuilder {
    fn new(inventory: &mut AwbcInventory, owner: AwbcFunctionId) -> Self {
        Self {
            block_start: inventory
                .begin_function_blocks(owner, AwbcSafePointKind::CallableBoundary)
                .0,
        }
    }

    fn lower_returning_expr(
        &mut self,
        inventory: &mut AwbcInventory,
        frame: &mut FrameBuilder,
        plan: &RuntimePlan,
        expr: &RuntimeExpr,
        path: String,
    ) {
        match expr.kind() {
            arcweft_core::value::RuntimeExprKind::If {
                condition,
                then_expr,
                else_expr,
            } => {
                let condition =
                    AwbcExprLowerer::new(inventory, frame, path.clone(), plan).lower(condition);
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
                self.lower_returning_expr(
                    inventory,
                    frame,
                    plan,
                    then_expr,
                    format!("{path}.then"),
                );
                let else_block = AwbcBlockId(table_index(inventory.program.blocks.len()));
                patch_branch_else_block(inventory, branch_block, else_block);
                self.lower_returning_expr(
                    inventory,
                    frame,
                    plan,
                    else_expr,
                    format!("{path}.else"),
                );
            }
            arcweft_core::value::RuntimeExprKind::Let {
                binding,
                expr,
                body,
            } => {
                let value = AwbcExprLowerer::new(inventory, frame, path.clone(), plan).lower(expr);
                let ty = admitted_local_type(inventory, plan, *binding);
                let local = frame.local(*binding, ty);
                inventory.push_instruction(AwbcInstruction::Move {
                    dst: local,
                    src: value,
                });
                self.lower_returning_expr(
                    inventory,
                    frame,
                    plan,
                    body,
                    format!("{path}.let.{binding}"),
                );
            }
            arcweft_core::value::RuntimeExprKind::Assign { place, expr, body } => {
                AwbcExprLowerer::new(inventory, frame, path.clone(), plan)
                    .lower_assignment(place, expr);
                self.lower_returning_expr(inventory, frame, plan, body, format!("{path}.assign"));
            }
            _ => {
                let value = AwbcExprLowerer::new(inventory, frame, path, plan).lower(expr);
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
        inventory.close_function_block(terminator, safe_point)
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
