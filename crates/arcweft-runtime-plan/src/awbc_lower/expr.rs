use crate::awbc_lower::frame::{FrameBuilder, FrameCaptureSlot};
use crate::awbc_lower::inventory::{AwbcInventory, AwbcLowerDiagnostic, PendingAwbcClosure};
use crate::awbc_lower::pattern::{
    admitted_plan_type, admitted_variant_case_name, function_input_ownership, lower_pattern,
};
use crate::awbc_lower::{table_index, table_range_len};
use arcweft_core::awbc::schema::{
    AwbcBinaryOp, AwbcBindMode, AwbcBlockId, AwbcDialogueContentEffectBinding,
    AwbcDialogueValueBinding, AwbcDialogueValueRole, AwbcEffectSetId, AwbcFieldProjection,
    AwbcFormatAttemptOperand, AwbcFormatOperand, AwbcFunction, AwbcFunctionFlag, AwbcFunctionFlags,
    AwbcFunctionInputOwnership, AwbcFunctionKind, AwbcInstruction, AwbcIntrinsic, AwbcIntrinsicId,
    AwbcMutablePlace, AwbcPattern, AwbcPatternId, AwbcPatternRest, AwbcPureHelperId,
    AwbcRegisterId, AwbcRuntimeTypeShape, AwbcSafePointKind, AwbcScopeId, AwbcTableRange,
    AwbcTerminator, AwbcTraitMethodId, AwbcTrapCode, AwbcUnaryOp, AwbcUnsignedIntKind,
};
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableTransition, RuntimeFunctionSiteBody,
    RuntimePlan, RuntimePlanTypeProjection, RuntimeReceiverMode,
};
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeCallTarget, RuntimeExpr, RuntimeExprKind, RuntimeExprMatchArm,
    RuntimeFieldProjection, RuntimeIntrinsic, RuntimeLocalReadMode, RuntimeMutablePlace,
    RuntimeUnaryOp,
};
use arcweft_interaction_model::dialogue::{
    CharacterDialoguePatchField, CharacterDialoguePatchOperation,
};
use std::collections::BTreeSet;

mod value_control;

/// Expression lowerer used by flow/source/stream builders.
pub struct AwbcExprLowerer<'a, 'b, 'plan> {
    pub inventory: &'a mut AwbcInventory,
    pub frame: &'b mut FrameBuilder,
    pub plan: &'plan RuntimePlan,
    pub path: String,
}

impl<'a, 'b, 'plan> AwbcExprLowerer<'a, 'b, 'plan> {
    pub fn new(
        inventory: &'a mut AwbcInventory,
        frame: &'b mut FrameBuilder,
        path: impl Into<String>,
        plan: &'plan RuntimePlan,
    ) -> Self {
        Self {
            inventory,
            frame,
            plan,
            path: path.into(),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "This mirrors the RuntimeExpr enum one arm at a time until expression lowering is split by expression family."
    )]
    /// Lowers one admitted runtime expression into the current AWBC frame.
    ///
    /// # Panics
    ///
    /// Panics only when an already-admitted runtime method-call identity cannot
    /// be reconstructed as the typed core callable identity it originated from.
    pub fn lower(&mut self, expr: &RuntimeExpr) -> AwbcRegisterId {
        match expr.kind() {
            RuntimeExprKind::Value(value) => {
                let ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let dst = self.frame.temp(ty);
                let constant = self.inventory.constant_runtime_value_typed(value, ty);
                self.inventory
                    .push_instruction(AwbcInstruction::LoadConst { dst, constant });
                dst
            }
            RuntimeExprKind::Local(read) => {
                let source = self.frame.register_for_local(read.local()).unwrap_or_else(|| {
                    panic!(
                        "admitted local `{}` is read before it is allocated in AWBC frame at {}",
                        read.local(),
                        self.path
                    )
                });
                let ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let destination = self.frame.temp(ty);
                if !read.fields().is_empty() {
                    self.inventory.push_instruction(AwbcInstruction::ReadPlace {
                        dst: destination,
                        root: source,
                        fields: read.fields().to_vec(),
                        mode: match read.mode() {
                            RuntimeLocalReadMode::Copy => {
                                arcweft_core::awbc::schema::AwbcPlaceReadMode::Copy
                            }
                            RuntimeLocalReadMode::Move => {
                                arcweft_core::awbc::schema::AwbcPlaceReadMode::Move
                            }
                        },
                    });
                    return destination;
                }
                self.inventory.push_instruction(match read.mode() {
                    RuntimeLocalReadMode::Copy => AwbcInstruction::CopyValue {
                        dst: destination,
                        src: source,
                    },
                    RuntimeLocalReadMode::Move => AwbcInstruction::Move {
                        dst: destination,
                        src: source,
                    },
                });
                destination
            }
            RuntimeExprKind::SequencePopFront { place } => {
                let place = self.lower_mutable_place(place, "Vec.pop_front");
                let dst = self
                    .frame
                    .temp(admitted_plan_type(self.inventory, self.plan, expr.ty()));
                self.inventory
                    .push_instruction(AwbcInstruction::SequencePopFront { dst, place });
                dst
            }
            RuntimeExprKind::SequencePush { place, value } => {
                let value = self.lower(value);
                let place = self.lower_mutable_place(place, "sequence append");
                self.inventory
                    .push_instruction(AwbcInstruction::SequenceAppend { place, value });
                let ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                self.load_runtime_const(&arcweft_core::value::RuntimeValue::Unit, ty)
            }
            RuntimeExprKind::SequencePopBack { place } => {
                let place = self.lower_mutable_place(place, "Vec.pop");
                let dst = self
                    .frame
                    .temp(admitted_plan_type(self.inventory, self.plan, expr.ty()));
                self.inventory
                    .push_instruction(AwbcInstruction::VecPop { dst, place });
                dst
            }
            RuntimeExprKind::EntityRef(value) => {
                let ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let dst = self.frame.temp(ty);
                let constant = self.inventory.constant_runtime_value_typed(
                    &arcweft_core::value::RuntimeValue::EntityRef(value.clone()),
                    ty,
                );
                self.inventory
                    .push_instruction(AwbcInstruction::LoadConst { dst, constant });
                dst
            }
            RuntimeExprKind::Agent(agent) => {
                let mut operands = Vec::with_capacity(
                    agent.operands().count() + usize::from(agent.choice().is_some()),
                );
                if let Some(choice) = agent.choice() {
                    let entity_ty = self.inventory.intern_type(AwbcRuntimeTypeShape::String);
                    operands.push(self.load_runtime_const(
                        &arcweft_core::value::RuntimeValue::String(choice.as_str().to_owned()),
                        entity_ty,
                    ));
                }
                operands.extend(
                    agent
                        .operands()
                        .into_iter()
                        .map(|operand| self.lower(operand)),
                );
                let constructor = agent.constructor();
                let ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let dst = self.frame.temp(ty);
                self.inventory.push_instruction(AwbcInstruction::MakeAgent {
                    dst,
                    constructor,
                    operands,
                });
                dst
            }
            RuntimeExprKind::Let {
                binding,
                expr,
                body,
            } => {
                let value = self.lower(expr);
                let local = self.frame.local(
                    *binding,
                    admitted_plan_type(self.inventory, self.plan, local_type(self.plan, *binding)),
                );
                self.inventory.push_instruction(AwbcInstruction::Move {
                    dst: local,
                    src: value,
                });
                self.lower(body)
            }
            RuntimeExprKind::Scope { identity, body } => {
                let scope = self.frame.enter_scope_with_identity(identity.clone());
                self.inventory
                    .push_instruction(AwbcInstruction::EnterScope { scope });
                let scoped_value = self.lower(body);
                let ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let value = self.frame.parent_temp(ty);
                self.inventory.push_instruction(AwbcInstruction::Move {
                    dst: value,
                    src: scoped_value,
                });
                self.inventory
                    .push_instruction(AwbcInstruction::ExitScope { scope });
                self.frame.exit_scope();
                value
            }
            RuntimeExprKind::DialogueContent {
                template,
                values,
                effects,
            } => {
                let manifest = self
                    .plan
                    .dialogue_content_templates()
                    .get(*template)
                    .expect("admitted DialogueContent expression references a plan template");
                let values = manifest
                    .slots()
                    .iter()
                    .zip(values)
                    .map(|(slot, value)| AwbcDialogueValueBinding {
                        slot: slot.slot(),
                        role: match slot.role() {
                            arcweft_core::plan::RuntimeDialogueValueRole::Interpolation => {
                                AwbcDialogueValueRole::Interpolation
                            }
                            arcweft_core::plan::RuntimeDialogueValueRole::Content => {
                                AwbcDialogueValueRole::Content
                            }
                            arcweft_core::plan::RuntimeDialogueValueRole::Formatted => {
                                AwbcDialogueValueRole::Formatted
                            }
                        },
                        value: self.lower(value),
                    })
                    .collect();
                let effects = effects
                    .iter()
                    .map(|effect| AwbcDialogueContentEffectBinding {
                        site: effect.site,
                        state: effect.state,
                        captures: effect
                            .captures
                            .iter()
                            .map(|capture| self.lower(capture))
                            .collect(),
                    })
                    .collect();
                let destination =
                    self.frame
                        .temp(admitted_plan_type(self.inventory, self.plan, expr.ty()));
                self.inventory
                    .push_instruction(AwbcInstruction::MakeDialogueContent {
                        destination,
                        template: *template,
                        values,
                        effects,
                    });
                destination
            }
            RuntimeExprKind::FormatContent {
                template,
                attempt,
                operands,
                project_method,
                project_option,
            } => {
                let operands = operands
                    .iter()
                    .map(|operand| {
                        let expression = operand.expression();
                        let captures = self.control_expr_captures(expression);
                        let captures = self.materialize_control_expr_captures(captures);
                        let definition = operand.definition();
                        let function = self.inventory.reserve_function_slot(definition);
                        self.inventory
                            .push_pending_closure(PendingAwbcClosure::FormatOperand {
                                definition,
                                function,
                                captures: captures
                                    .iter()
                                    .map(|capture| (capture.local, capture.mode))
                                    .collect(),
                                result: expression.ty(),
                                expression: expression.clone(),
                                path: format!("{}.fmt.{}", self.path, function.0),
                            });
                        AwbcFormatOperand {
                            parameter: operand.parameter(),
                            function,
                            captures: captures.iter().map(|capture| capture.register).collect(),
                        }
                    })
                    .collect();
                let attempt_operands = attempt.map_or_else(Vec::new, |attempt_id| {
                    let manifest = self.plan.format_attempt(attempt_id).unwrap_or_else(|| {
                        panic!(
                            "admitted format attempt refers to a missing plan manifest at {}",
                            self.path
                        )
                    });
                    manifest
                        .operands()
                        .iter()
                        .map(|operand| AwbcFormatAttemptOperand {
                            parameter: operand.parameter(),
                            ty: admitted_plan_type(self.inventory, self.plan, operand.ty()),
                        })
                        .collect()
                });
                let (project_method, project_result) = match project_method {
                    Some(method) => {
                        let lowered = self
                            .inventory
                            .trait_method(*method)
                            .unwrap_or_else(|| {
                                panic!(
                                    "admitted DisplayText selection refers to an unlowered trait method at {}",
                                    self.path
                                )
                            });
                        let body = self
                            .plan
                            .trait_methods()
                            .get(method.0)
                            .filter(|body| body.id == *method)
                            .unwrap_or_else(|| {
                                panic!(
                                    "admitted DisplayText selection refers to a missing RuntimePlan method at {}",
                                    self.path
                                )
                            });
                        let result = self.frame.temp(admitted_plan_type(
                            self.inventory,
                            self.plan,
                            body.result,
                        ));
                        (Some(lowered), Some(result))
                    }
                    None => {
                        assert!(
                            !project_option,
                            "project DisplayText option mode has no selected method at {}",
                            self.path
                        );
                        (None, None)
                    }
                };
                let destination =
                    self.frame
                        .temp(admitted_plan_type(self.inventory, self.plan, expr.ty()));
                self.inventory
                    .push_instruction(AwbcInstruction::FormatContent {
                        destination,
                        template: *template,
                        attempt: *attempt,
                        attempt_operands,
                        project_method,
                        project_option: *project_option,
                        project_result,
                        operands,
                    });
                destination
            }
            RuntimeExprKind::CharacterDialogue {
                operation,
                target,
                fields,
            } => {
                let target = self.lower(target);
                let fields = fields
                    .iter()
                    .map(|field| CharacterDialoguePatchField {
                        coordinate: field.coordinate.clone(),
                        operation: match &field.operation {
                            CharacterDialoguePatchOperation::Set(value) => {
                                CharacterDialoguePatchOperation::Set(self.lower(value))
                            }
                            CharacterDialoguePatchOperation::Clear => {
                                CharacterDialoguePatchOperation::Clear
                            }
                        },
                    })
                    .collect();
                let destination =
                    self.frame
                        .temp(admitted_plan_type(self.inventory, self.plan, expr.ty()));
                self.inventory
                    .push_instruction(AwbcInstruction::CharacterDialogue {
                        destination,
                        operation: *operation,
                        target,
                        fields,
                    });
                destination
            }
            RuntimeExprKind::Tuple(items) => {
                let registers = items.iter().map(|item| self.lower(item)).collect();
                let ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let dst = self.frame.temp(ty);
                self.inventory.push_instruction(AwbcInstruction::MakeTuple {
                    dst,
                    items: registers,
                });
                dst
            }
            RuntimeExprKind::BracketSeq(items) => {
                let registers = items.iter().map(|item| self.lower(item)).collect();
                let ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let dst = self.frame.temp(ty);
                self.inventory
                    .push_instruction(AwbcInstruction::MakeSequence {
                        dst,
                        items: registers,
                    });
                dst
            }
            RuntimeExprKind::RepeatSeq { value, len } => {
                let value = self.lower(value);
                let len_ty = self
                    .inventory
                    .intern_type(AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::USize));
                let len_reg = self.frame.temp(len_ty);
                let constant = self.inventory.constant_runtime_value_typed(
                    &arcweft_core::value::RuntimeValue::usize(*len as u64),
                    len_ty,
                );
                self.inventory.push_instruction(AwbcInstruction::LoadConst {
                    dst: len_reg,
                    constant,
                });
                let ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let dst = self.frame.temp(ty);
                self.inventory
                    .push_instruction(AwbcInstruction::RepeatSequence {
                        dst,
                        value,
                        len: len_reg,
                    });
                dst
            }
            RuntimeExprKind::ArrayFromVec { value } => {
                let value = self.lower(value);
                let ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let dst = self.frame.temp(ty);
                self.inventory
                    .push_instruction(AwbcInstruction::ArrayFromVec { dst, value });
                dst
            }
            RuntimeExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                let (start, start_ty) = self.lower_optional_range_bound(start.as_deref());
                let (end, end_ty) = self.lower_optional_range_bound(end.as_deref());
                let bool_ty = self.inventory.bool_ty();
                let inclusive = self.load_runtime_const(
                    &arcweft_core::value::RuntimeValue::Bool(*inclusive),
                    bool_ty,
                );
                let result_ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let dst = self.frame.temp(result_ty);
                let intrinsic = self.intern_intrinsic(
                    &RuntimeCallTarget::intrinsic(arcweft_core::value::RuntimeIntrinsic::CoreRange),
                    &[start_ty, end_ty, bool_ty],
                    Some(result_ty),
                );
                self.inventory
                    .push_instruction(AwbcInstruction::CallIntrinsic {
                        dst: Some(dst),
                        intrinsic,
                        args: vec![start, end, inclusive],
                    });
                dst
            }
            RuntimeExprKind::NominalRecord(record) => {
                let Some(domain) = self.plan.nominal_record_domains().get(expr.ty()) else {
                    panic!(
                        "admitted nominal record expression type {} has no RuntimePlan record domain at {}",
                        expr.ty(),
                        self.path
                    );
                };
                let ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let mut registers = vec![None; domain.fields().len()];
                for initializer in record.initializers() {
                    let value = self.lower(initializer.value());
                    registers[initializer.field().zero_based() as usize] = Some(value);
                }
                let registers = registers
                    .into_iter()
                    .map(|register| {
                        register.expect(
                            "checked nominal record expression must initialize every layout field",
                        )
                    })
                    .collect();
                let dst = self.frame.temp(ty);
                self.inventory
                    .push_instruction(AwbcInstruction::MakeRecord {
                        dst,
                        ty,
                        fields: registers,
                    });
                dst
            }
            RuntimeExprKind::Variant { ordinal, payload } => {
                let ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let dst = self.frame.temp(ty);
                let payload = payload.as_deref().map(|payload| self.lower(payload));
                let case_name =
                    admitted_variant_case_name(self.inventory, self.plan, expr.ty(), *ordinal);
                self.inventory
                    .push_instruction(AwbcInstruction::MakeVariant {
                        dst,
                        ty,
                        case: *ordinal,
                        case_name,
                        payload,
                    });
                dst
            }
            RuntimeExprKind::Field { target, field } => {
                let target_ty = admitted_plan_type(self.inventory, self.plan, target.ty());
                let target = match target {
                    arcweft_core::value::RuntimeFieldTarget::Value(value) => {
                        arcweft_core::awbc::schema::AwbcFieldTarget::Value(self.lower(value))
                    }
                    arcweft_core::value::RuntimeFieldTarget::Inspect { place, .. } => {
                        arcweft_core::awbc::schema::AwbcFieldTarget::Inspect(
                            self.lower_mutable_place(place, "field inspection"),
                        )
                    }
                };
                let field_type = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let dst = self.frame.temp(field_type);
                match field {
                    RuntimeFieldProjection::Nominal(field) => {
                        let instruction = match target {
                            arcweft_core::awbc::schema::AwbcFieldTarget::Value(target) => {
                                AwbcInstruction::ProjectRecord {
                                    dst,
                                    target,
                                    ordinal: field.zero_based(),
                                }
                            }
                            arcweft_core::awbc::schema::AwbcFieldTarget::Inspect(place) => {
                                let mut fields = place.fields().to_vec();
                                fields.push(*field);
                                AwbcInstruction::ReadPlace {
                                    dst,
                                    root: place.base(),
                                    fields,
                                    mode: arcweft_core::awbc::schema::AwbcPlaceReadMode::Copy,
                                }
                            }
                        };
                        self.inventory.push_instruction(instruction);
                    }
                    RuntimeFieldProjection::OpaqueRecord { field, .. } => {
                        self.inventory
                            .push_instruction(AwbcInstruction::ProjectField {
                                dst,
                                target,
                                field: AwbcFieldProjection::OpaqueRecord {
                                    owner: target_ty,
                                    field: field.zero_based(),
                                    field_type,
                                },
                            });
                    }
                    _ => {
                        let field = AwbcFieldProjection::Named(
                            self.inventory.intern_string(&field.label()),
                        );
                        self.inventory
                            .push_instruction(AwbcInstruction::ProjectField { dst, target, field });
                    }
                }
                dst
            }
            RuntimeExprKind::ProjectTuple { target, ordinal } => {
                let target = self.lower(target);
                let dst = self
                    .frame
                    .temp(admitted_plan_type(self.inventory, self.plan, expr.ty()));
                self.inventory
                    .push_instruction(AwbcInstruction::ProjectTuple {
                        dst,
                        target,
                        ordinal: table_index(*ordinal),
                    });
                dst
            }
            RuntimeExprKind::ProjectRecord { target, ordinal } => {
                let target = self.lower(target);
                let dst = self
                    .frame
                    .temp(admitted_plan_type(self.inventory, self.plan, expr.ty()));
                self.inventory
                    .push_instruction(AwbcInstruction::ProjectRecord {
                        dst,
                        target,
                        ordinal: table_index(*ordinal),
                    });
                dst
            }
            RuntimeExprKind::Assign { place, expr, body } => {
                self.lower_assignment(place, expr);
                self.lower(body)
            }
            RuntimeExprKind::Call { callee, args } => self.lower_call(expr.ty(), callee, args),
            RuntimeExprKind::MakeCallable { state, captures } => {
                let captures = captures.iter().map(|capture| self.lower(capture)).collect();
                let dst = self
                    .frame
                    .temp(admitted_plan_type(self.inventory, self.plan, expr.ty()));
                self.inventory
                    .push_instruction(AwbcInstruction::MakeCallable {
                        dst,
                        state: *state,
                        captures,
                    });
                dst
            }
            RuntimeExprKind::SpecializeCallable {
                value,
                specialization,
            } => {
                let src = self.lower(value);
                let dst = self
                    .frame
                    .temp(admitted_plan_type(self.inventory, self.plan, expr.ty()));
                self.inventory
                    .push_instruction(AwbcInstruction::SpecializeCallable {
                        dst,
                        src,
                        specialization: *specialization,
                    });
                dst
            }
            RuntimeExprKind::ApplyGroup { callee, args } => {
                self.lower_function_application(callee, args, expr.ty())
            }
            RuntimeExprKind::TraitCall {
                callable,
                receiver,
                receiver_mode,
                args,
            } => {
                let receiver_register = if *receiver_mode == RuntimeReceiverMode::MutRef
                    && let RuntimeExprKind::Local(read) = receiver.kind()
                {
                    self.frame.register_for_local(read.local()).unwrap_or_else(|| {
                        panic!(
                            "admitted mutable receiver local `{}` is not in the AWBC frame at {}",
                            read.local(),
                            self.path
                        )
                    })
                } else {
                    self.lower(receiver)
                };
                let args = self.lower_argument_registers(args);
                let dst = self
                    .frame
                    .temp(admitted_plan_type(self.inventory, self.plan, expr.ty()));
                let receiver_out = (*receiver_mode == RuntimeReceiverMode::MutRef).then(|| {
                    if matches!(receiver.kind(), RuntimeExprKind::Local(_)) {
                        receiver_register
                    } else {
                        self.frame.temp(admitted_plan_type(
                            self.inventory,
                            self.plan,
                            receiver.ty(),
                        ))
                    }
                });
                self.inventory
                    .push_instruction(AwbcInstruction::CallTraitMethod {
                        dst,
                        method: AwbcTraitMethodId(table_index(callable.0)),
                        receiver: receiver_register,
                        args,
                        receiver_out,
                    });
                dst
            }
            RuntimeExprKind::PureCall { helper, args } => {
                let args = self.lower_argument_registers(args);
                let dst = self
                    .frame
                    .temp(admitted_plan_type(self.inventory, self.plan, expr.ty()));
                self.inventory
                    .push_instruction(AwbcInstruction::CallPureHelper {
                        dst,
                        helper: AwbcPureHelperId(table_index(helper.0)),
                        args,
                    });
                dst
            }
            RuntimeExprKind::Sum { source } => {
                let source_ty = admitted_plan_type(self.inventory, self.plan, source.ty());
                let source = self.lower(source);
                let result_ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let dst = self.frame.temp(result_ty);
                let intrinsic = self.intern_intrinsic(
                    &RuntimeCallTarget::intrinsic(RuntimeIntrinsic::CoreSeqSum),
                    &[source_ty],
                    Some(result_ty),
                );
                self.inventory
                    .push_instruction(AwbcInstruction::CallIntrinsic {
                        dst: Some(dst),
                        intrinsic,
                        args: vec![source],
                    });
                dst
            }
            RuntimeExprKind::Unary { op, expr: operand } => {
                let src = self.lower(operand);
                let dst = self
                    .frame
                    .temp(admitted_plan_type(self.inventory, self.plan, expr.ty()));
                self.inventory.push_instruction(AwbcInstruction::Unary {
                    dst,
                    op: unary_op(*op),
                    src,
                });
                dst
            }
            RuntimeExprKind::Binary { lhs, op, rhs } => {
                let lhs = self.lower(lhs);
                let rhs = self.lower(rhs);
                let dst = self
                    .frame
                    .temp(admitted_plan_type(self.inventory, self.plan, expr.ty()));
                self.inventory.push_instruction(AwbcInstruction::Binary {
                    dst,
                    op: binary_op(*op),
                    lhs,
                    rhs,
                });
                dst
            }
            RuntimeExprKind::If { .. }
            | RuntimeExprKind::IfLet { .. }
            | RuntimeExprKind::Match { .. } => self.lower_value_control_expr(expr),
            RuntimeExprKind::ReductionUnchanged { state } => {
                let state = self.lower(state);
                let ty = admitted_plan_type(self.inventory, self.plan, expr.ty());
                let dst = self.frame.temp(ty);
                self.inventory
                    .push_instruction(AwbcInstruction::MakeReductionUnchanged { dst, ty, state });
                dst
            }
        }
    }

    fn lower_call(
        &mut self,
        result_type: arcweft_core::runtime_id::RuntimePlanTypeId,
        callee: &RuntimeCallTarget,
        args: &[arcweft_core::value::RuntimeCallArgument],
    ) -> AwbcRegisterId {
        let args = self.lower_argument_registers(args);
        let argument_types = args
            .iter()
            .map(|register| {
                self.frame
                    .register_type(*register)
                    .expect("lowered argument register belongs to this typed frame")
            })
            .collect::<Vec<_>>();
        let result_type = admitted_plan_type(self.inventory, self.plan, result_type);
        let dst = self.frame.temp(result_type);
        let intrinsic = self.intern_intrinsic(callee, &argument_types, Some(result_type));
        self.inventory
            .push_instruction(AwbcInstruction::CallIntrinsic {
                dst: Some(dst),
                intrinsic,
                args,
            });
        dst
    }

    fn lower_optional_range_bound(
        &mut self,
        expr: Option<&RuntimeExpr>,
    ) -> (AwbcRegisterId, arcweft_core::awbc::schema::AwbcTypeId) {
        if let Some(expr) = expr {
            (
                self.lower(expr),
                admitted_plan_type(self.inventory, self.plan, expr.ty()),
            )
        } else {
            let ty = self.inventory.unit_ty();
            (
                self.load_runtime_const(&arcweft_core::value::RuntimeValue::Unit, ty),
                ty,
            )
        }
    }

    /// Evaluates source operands once, then assembles the admitted positional
    /// ABI for the VM's callable-group instruction.
    pub(super) fn lower_function_application(
        &mut self,
        callee: &RuntimeExpr,
        args: &[arcweft_core::value::RuntimeCallArgument],
        result_type: arcweft_core::runtime_id::RuntimePlanTypeId,
    ) -> AwbcRegisterId {
        let callee = self.lower(callee);
        let args = self.lower_argument_registers(args);
        let dst = self
            .frame
            .temp(admitted_plan_type(self.inventory, self.plan, result_type));
        self.inventory
            .push_instruction(AwbcInstruction::ApplyGroup { dst, callee, args });
        dst
    }

    /// All call families evaluate and expand operands in source order, then
    /// assemble registers in the original admitted ABI order.
    fn lower_argument_registers(
        &mut self,
        args: &[arcweft_core::value::RuntimeCallArgument],
    ) -> Vec<AwbcRegisterId> {
        let mut groups = Vec::with_capacity(args.len());
        for argument in args {
            let value = self.lower(argument.value());
            let mut values = Vec::new();
            match argument.mode() {
                arcweft_core::value::RuntimeCallArgumentMode::Value => values.push(value),
                arcweft_core::value::RuntimeCallArgumentMode::Spread => {
                    match plan_type_projection(self.plan, argument.value().ty()) {
                        RuntimePlanTypeProjection::Tuple(items) => {
                            let mut patterns = Vec::with_capacity(items.len());
                            for item in items {
                                let item_type =
                                    admitted_plan_type(self.inventory, self.plan, *item);
                                let dst = self.frame.temp(item_type);
                                patterns.push(self.inventory.intern_pattern(AwbcPattern::Bind {
                                    target: dst,
                                    mutable: false,
                                    expected: Some(item_type),
                                }));
                                values.push(dst);
                            }
                            let pattern =
                                self.inventory.intern_pattern(AwbcPattern::Tuple(patterns));
                            self.inventory
                                .push_instruction(AwbcInstruction::BindPattern {
                                    pattern,
                                    value,
                                    mode: AwbcBindMode::Declare,
                                });
                        }
                        RuntimePlanTypeProjection::Array { item, length } => {
                            let length = usize::try_from(
                                length
                                    .constant()
                                    .expect("admitted executable array length is closed"),
                            )
                            .expect("admitted executable array length fits host usize");
                            let mut patterns = Vec::with_capacity(length);
                            for _ in 0..length {
                                let item_type =
                                    admitted_plan_type(self.inventory, self.plan, *item);
                                let dst = self.frame.temp(item_type);
                                patterns.push(self.inventory.intern_pattern(AwbcPattern::Bind {
                                    target: dst,
                                    mutable: false,
                                    expected: Some(item_type),
                                }));
                                values.push(dst);
                            }
                            let pattern = self.inventory.intern_pattern(AwbcPattern::Sequence {
                                items: patterns,
                                rest: AwbcPatternRest::Exact,
                            });
                            self.inventory
                                .push_instruction(AwbcInstruction::BindPattern {
                                    pattern,
                                    value,
                                    mode: AwbcBindMode::Declare,
                                });
                        }
                        _ => unreachable!(
                            "admitted function application spreads only tuples and fixed arrays"
                        ),
                    }
                }
            }
            groups.push((argument.abi_position(), values));
        }
        groups.sort_by_key(|(position, _)| *position);
        groups.into_iter().flat_map(|(_, values)| values).collect()
    }

    fn control_expr_captures(&self, expr: &RuntimeExpr) -> Vec<FrameCaptureSlot> {
        expr.evaluation_free_local_reads(self.plan)
            .unwrap_or_else(|error| {
                panic!(
                    "admitted control expression has invalid free-local authority at {}: {error}",
                    self.path
                )
            })
            .iter()
            .map(|(local, mode)| FrameCaptureSlot {
                local: *local,
                mode: *mode,
                register: self.frame.register_for_local(*local).unwrap_or_else(|| {
                    panic!(
                        "admitted control expression reads local `{local}` outside the AWBC frame at {}",
                        self.path
                    )
                }),
            })
            .collect()
    }

    fn materialize_control_expr_captures(
        &mut self,
        captures: Vec<FrameCaptureSlot>,
    ) -> Vec<FrameCaptureSlot> {
        captures
            .into_iter()
            .map(|capture| {
                let ty = admitted_plan_type(
                    self.inventory,
                    self.plan,
                    local_type(self.plan, capture.local),
                );
                let register = self.frame.temp(ty);
                let instruction = match capture.mode {
                    RuntimeLocalReadMode::Copy => AwbcInstruction::CopyValue {
                        dst: register,
                        src: capture.register,
                    },
                    RuntimeLocalReadMode::Move => AwbcInstruction::Move {
                        dst: register,
                        src: capture.register,
                    },
                };
                self.inventory.push_instruction(instruction);
                FrameCaptureSlot {
                    register,
                    ..capture
                }
            })
            .collect()
    }

    fn load_runtime_const(
        &mut self,
        value: &arcweft_core::value::RuntimeValue,
        ty: arcweft_core::awbc::schema::AwbcTypeId,
    ) -> AwbcRegisterId {
        let dst = self.frame.temp(ty);
        let constant = self.inventory.constant_runtime_value_typed(value, ty);
        self.inventory
            .push_instruction(AwbcInstruction::LoadConst { dst, constant });
        dst
    }

    pub(super) fn lower_assignment(
        &mut self,
        assignment: &arcweft_core::value::RuntimeAssignment,
        value: &RuntimeExpr,
    ) {
        let place = self.lower_mutable_place(&assignment.place(), "assignment");
        let value = self.lower(value);
        self.inventory.push_instruction(AwbcInstruction::Assign {
            place,
            value,
            displacement: assignment.displacement().clone(),
        });
    }

    fn lower_mutable_place(
        &self,
        place: &RuntimeMutablePlace,
        operation: &str,
    ) -> AwbcMutablePlace {
        let local_register = |local| {
            self.frame.register_for_local(local).unwrap_or_else(|| {
                panic!(
                    "admitted {operation} receiver local `{local}` is not in the AWBC frame at {}",
                    self.path
                )
            })
        };
        match place {
            RuntimeMutablePlace::Local(local) => AwbcMutablePlace::Local(local_register(*local)),
            RuntimeMutablePlace::Fields { base, fields } => AwbcMutablePlace::Fields {
                base: local_register(*base),
                fields: fields.clone(),
            },
        }
    }

    fn intern_intrinsic(
        &mut self,
        identity: &RuntimeCallTarget,
        parameters: &[arcweft_core::awbc::schema::AwbcTypeId],
        result: Option<arcweft_core::awbc::schema::AwbcTypeId>,
    ) -> AwbcIntrinsicId {
        let signature =
            self.inventory
                .intern_signature(parameters.to_vec(), result, AwbcEffectSetId(0));
        if let Some((index, _)) =
            self.inventory
                .program
                .intrinsics
                .iter()
                .enumerate()
                .find(|(_, candidate)| {
                    candidate.identity == *identity && candidate.signature == signature
                })
        {
            return AwbcIntrinsicId(table_index(index));
        }
        let id = AwbcIntrinsicId(table_index(self.inventory.program.intrinsics.len()));
        self.inventory.program.intrinsics.push(AwbcIntrinsic {
            identity: identity.clone(),
            signature,
            revision: 1,
        });
        id
    }
}

pub(crate) fn lower_pending_closures(inventory: &mut AwbcInventory, plan: &RuntimePlan) {
    let mut callable_sites = BTreeSet::new();
    callable_sites.extend(plan.pure_programs().iter().map(|binding| binding.site()));
    for (_, state) in plan.callable_states().iter_with_ids() {
        if let RuntimeCallableTransition::Invoke { function, .. } = &state.transition {
            callable_sites.insert(*function);
        }
        if let RuntimeCallableAttachedContract::Defaulted {
            default: arcweft_core::plan::RuntimeCallableDefault::Body { function, .. },
            ..
        } = &state.attached
        {
            callable_sites.insert(*function);
        }
    }
    for site in callable_sites {
        let _ = ensure_function_site(inventory, plan, site, &format!("function site body {site}"));
    }

    while let Some(closure) = inventory.pop_pending_closure() {
        match closure {
            PendingAwbcClosure::ProgramTask {
                source,
                function,
                path,
            } => {
                if let Some(source) = inventory.program_task_body(plan, &source, &path) {
                    super::flow::AwbcFlowLowerer::new(inventory, plan)
                        .lower_program_task(function, &source, &path);
                }
            }
            PendingAwbcClosure::TaskRequest {
                definition,
                function,
                inputs,
                arguments,
                path,
            } => {
                let mut frame = FrameBuilder::new();
                let params = inputs
                    .iter()
                    .map(|local| {
                        let ty = admitted_plan_type(inventory, plan, local_type(plan, *local));
                        let name = inventory.local_name(*local);
                        frame.named_parameter(*local, ty, name);
                        ty
                    })
                    .collect::<Vec<_>>();
                let block =
                    inventory.begin_function_blocks(function, AwbcSafePointKind::CallableBoundary);
                let result_items = arguments
                    .iter()
                    .map(|argument| admitted_plan_type(inventory, plan, argument.ty()))
                    .collect();
                let result_type = inventory.intern_type(AwbcRuntimeTypeShape::Tuple(result_items));
                let items = arguments
                    .iter()
                    .map(|argument| {
                        AwbcExprLowerer::new(inventory, &mut frame, &path, plan).lower(argument)
                    })
                    .collect();
                let value = frame.temp(result_type);
                inventory.push_instruction(AwbcInstruction::MakeTuple { dst: value, items });
                inventory.close_function_block(
                    AwbcTerminator::Return { value: Some(value) },
                    AwbcSafePointKind::Return,
                );
                let layout = inventory.intern_frame_layout(function, frame.finish());
                let signature =
                    inventory.intern_signature(params, Some(result_type), AwbcEffectSetId(0));
                inventory.replace_function(
                    function,
                    AwbcFunction {
                        definition,
                        semantic_role: arcweft_core::plan::RuntimeFunctionSemanticRole::Ordinary,
                        public_id: None,
                        kind: AwbcFunctionKind::Ordinary,
                        signature,
                        type_context: None,
                        input_ownership: inputs
                            .iter()
                            .enumerate()
                            .map(|(position, local)| {
                                AwbcFunctionInputOwnership::capture(
                                    plan.local_declarations()
                                        .get(*local)
                                        .expect("admitted TaskRequest capture declaration")
                                        .origin(),
                                    table_index(position),
                                    arcweft_core::plan::RuntimeFunctionCaptureMode::Copy,
                                )
                            })
                            .collect(),
                        frame_layout: layout,
                        blocks: AwbcTableRange::new(
                            block.0,
                            table_range_len(block.0, inventory.program.blocks.len()),
                        ),
                        entry_block: block,
                        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
                    },
                );
            }
            PendingAwbcClosure::FunctionSite {
                site,
                function,
                path,
            } => {
                let declaration = plan
                    .function_sites()
                    .get(site)
                    .expect("reserved AWBC function site belongs to its RuntimePlan");
                if declaration.role() == arcweft_core::plan::RuntimeFunctionSemanticRole::Flow {
                    let flow = plan
                        .flows()
                        .iter()
                        .find(|flow| flow.function_site() == site)
                        .expect("Flow function site owns a Root Flow");
                    super::flow::AwbcFlowLowerer::new(inventory, plan).lower_flow(flow);
                    continue;
                }
                let definition = declaration.definition();
                let role = declaration.role();
                let function_type = declaration.function_type();
                let inputs = declaration.inputs();
                let result = declaration.result();
                match declaration.body() {
                    RuntimeFunctionSiteBody::Executable(_) => {
                        super::flow::AwbcFlowLowerer::new(inventory, plan)
                            .lower_executable_function_site(function, declaration, &path);
                    }
                    RuntimeFunctionSiteBody::Expression(expression) => {
                        let mut frame = FrameBuilder::new();
                        let mut abi_parameters = Vec::with_capacity(inputs.len());
                        for (position, input) in inputs.iter().enumerate() {
                            let input_local = input.input_local();
                            let name = inventory.local_name(input_local);
                            let input_ty =
                                admitted_plan_type(inventory, plan, local_type(plan, input_local));
                            abi_parameters.push(frame.abi_parameter(position, input_ty, name));
                        }
                        let mut body = ExprBodyBuilder::new(inventory, function);
                        let mut input_ownership = Vec::with_capacity(inputs.len());
                        for (input, value) in inputs.iter().zip(abi_parameters) {
                            let pattern =
                                lower_pattern(inventory, plan, &mut frame, input.pattern());
                            input_ownership.push(function_input_ownership(
                                inventory, &frame, input, pattern, &path,
                            ));
                            inventory.push_instruction(AwbcInstruction::BindPattern {
                                pattern,
                                value,
                                mode: AwbcBindMode::Declare,
                            });
                        }

                        lower_closure_body(
                            inventory,
                            &mut frame,
                            plan,
                            &mut body,
                            &expression,
                            &path,
                        );
                        let layout = inventory.intern_frame_layout(function, frame.finish());
                        let block = body.block_start;
                        let block_len = table_range_len(block.0, inventory.program.blocks.len());
                        let params = inputs
                            .iter()
                            .map(|input| {
                                admitted_plan_type(
                                    inventory,
                                    plan,
                                    local_type(plan, input.input_local()),
                                )
                            })
                            .collect();
                        let result = admitted_plan_type(inventory, plan, result);
                        let effects = inventory.intern_effect_set(declaration.invocation_effects());
                        let signature = inventory.intern_signature(params, Some(result), effects);
                        let type_context = function_type.map(|ty| {
                            arcweft_core::awbc::schema::AwbcFunctionTypeContext::Signature(
                                admitted_plan_type(inventory, plan, ty),
                            )
                        });
                        inventory.replace_function(
                            function,
                            AwbcFunction {
                                definition,
                                semantic_role: role,
                                public_id: None,
                                kind: AwbcFunctionKind::Ordinary,
                                signature,
                                type_context,
                                input_ownership,
                                frame_layout: layout,
                                blocks: AwbcTableRange::new(block.0, block_len),
                                entry_block: block,
                                flags: AwbcFunctionFlags::empty()
                                    .with(AwbcFunctionFlag::Deterministic),
                            },
                        );
                    }
                }
            }
            PendingAwbcClosure::FormatOperand {
                definition,
                function,
                captures,
                result,
                expression,
                path,
            } => {
                let mut frame = FrameBuilder::new();
                for (local, _) in &captures {
                    let name = inventory.local_name(*local);
                    frame.named_parameter(
                        *local,
                        admitted_plan_type(inventory, plan, local_type(plan, *local)),
                        name,
                    );
                }
                let mut body = ExprBodyBuilder::new(inventory, function);
                lower_closure_body(inventory, &mut frame, plan, &mut body, &expression, &path);
                let layout = inventory.intern_frame_layout(function, frame.finish());
                let block = body.block_start;
                let block_len = table_range_len(block.0, inventory.program.blocks.len());
                let params = captures
                    .iter()
                    .map(|(local, _)| admitted_plan_type(inventory, plan, local_type(plan, *local)))
                    .collect();
                let result = admitted_plan_type(inventory, plan, result);
                let signature =
                    inventory.intern_signature(params, Some(result), AwbcEffectSetId(0));
                inventory.replace_function(
                    function,
                    AwbcFunction {
                        definition,
                        semantic_role: arcweft_core::plan::RuntimeFunctionSemanticRole::Ordinary,
                        public_id: None,
                        kind: AwbcFunctionKind::Synthetic,
                        signature,
                        type_context: None,
                        input_ownership: captures
                            .iter()
                            .enumerate()
                            .map(|(position, (local, mode))| {
                                AwbcFunctionInputOwnership::capture(
                                    plan.local_declarations()
                                        .get(*local)
                                        .expect("admitted formatter capture declaration")
                                        .origin(),
                                    table_index(position),
                                    match mode {
                                        RuntimeLocalReadMode::Copy => {
                                            arcweft_core::plan::RuntimeFunctionCaptureMode::Copy
                                        }
                                        RuntimeLocalReadMode::Move => {
                                            arcweft_core::plan::RuntimeFunctionCaptureMode::Move
                                        }
                                    },
                                )
                            })
                            .collect(),
                        frame_layout: layout,
                        blocks: AwbcTableRange::new(block.0, block_len),
                        entry_block: block,
                        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
                    },
                );
            }
        }
    }
}

fn ensure_function_site(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
    site: arcweft_core::runtime_id::RuntimeFunctionSiteId,
    path: &str,
) -> Option<arcweft_core::awbc::schema::AwbcFunctionId> {
    let Some(declaration) = plan.function_sites().get(site) else {
        inventory.diagnostic(AwbcLowerDiagnostic::error(
            path,
            format!("runtime function site {site} is absent from the RuntimePlan"),
        ));
        return None;
    };
    let already_reserved = inventory.function_site_function(site).is_some();
    let function = inventory.reserve_function_site_slot(site, declaration.definition());
    if !already_reserved {
        inventory.push_pending_closure(PendingAwbcClosure::FunctionSite {
            site,
            function,
            path: format!("{path}.function.{site}"),
        });
    }
    Some(function)
}

fn local_type(
    plan: &RuntimePlan,
    local: arcweft_core::runtime_id::RuntimeLocalDeclarationId,
) -> arcweft_core::runtime_id::RuntimePlanTypeId {
    plan.local_declarations().get(local).map_or_else(
        || panic!("admitted RuntimePlan local {local} is absent"),
        arcweft_core::plan::RuntimeLocalDeclaration::ty,
    )
}

struct ExprBodyBuilder {
    block_start: AwbcBlockId,
    terminated: bool,
}

impl ExprBodyBuilder {
    fn new(
        inventory: &mut AwbcInventory,
        owner: arcweft_core::awbc::schema::AwbcFunctionId,
    ) -> Self {
        let block_start =
            inventory.begin_function_blocks(owner, AwbcSafePointKind::CallableBoundary);
        Self {
            block_start,
            terminated: false,
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

    fn reopen_after_terminated_branch(&mut self, inventory: &mut AwbcInventory) -> AwbcBlockId {
        self.terminated = false;
        inventory.reopen_function_block()
    }

    fn terminate(
        &mut self,
        inventory: &mut AwbcInventory,
        terminator: AwbcTerminator,
        safe_point: AwbcSafePointKind,
    ) {
        if self.terminated {
            return;
        }
        self.close_block(inventory, terminator, safe_point);
        self.terminated = true;
    }
}

fn lower_closure_body(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    plan: &RuntimePlan,
    body: &mut ExprBodyBuilder,
    expr: &RuntimeExpr,
    path: &str,
) {
    match expr.kind() {
        RuntimeExprKind::If {
            condition,
            then_expr,
            else_expr,
        } => lower_if_value_expr(
            inventory,
            frame,
            plan,
            body,
            IfValueExprInput {
                condition,
                then_expr,
                else_expr,
                path,
            },
        ),
        RuntimeExprKind::IfLet {
            pattern,
            expr,
            guard,
            then_expr,
            else_expr,
        } => lower_if_let_value_expr(
            inventory,
            frame,
            plan,
            body,
            IfLetValueExprInput {
                pattern,
                expr,
                guard: guard.as_deref(),
                then_expr,
                else_expr,
                path,
            },
        ),
        RuntimeExprKind::Match { scrutinee, arms } => {
            lower_match_value_expr(inventory, frame, plan, body, scrutinee, arms, path);
        }
        _ => terminate_return_expr(inventory, frame, plan, body, expr, path, None),
    }
}

fn plan_type_projection(
    plan: &RuntimePlan,
    ty: arcweft_core::runtime_id::RuntimePlanTypeId,
) -> &RuntimePlanTypeProjection<arcweft_core::runtime_id::RuntimePlanTypeId> {
    plan.type_table()
        .get(ty)
        .unwrap_or_else(|| panic!("admitted RuntimePlan type {ty} is absent"))
        .projection()
}

#[derive(Clone, Copy)]
struct IfValueExprInput<'a> {
    condition: &'a RuntimeExpr,
    then_expr: &'a RuntimeExpr,
    else_expr: &'a RuntimeExpr,
    path: &'a str,
}

fn lower_if_value_expr(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    plan: &RuntimePlan,
    body: &mut ExprBodyBuilder,
    input: IfValueExprInput<'_>,
) {
    let condition = AwbcExprLowerer::new(inventory, frame, input.path, plan).lower(input.condition);
    let then_block = AwbcBlockId(table_index(
        inventory.program.blocks.len().saturating_add(1),
    ));
    let branch_block = body.close_block(
        inventory,
        AwbcTerminator::Branch {
            condition,
            then_block,
            else_block: then_block,
        },
        AwbcSafePointKind::CallableBoundary,
    );
    terminate_return_expr(
        inventory,
        frame,
        plan,
        body,
        input.then_expr,
        &format!("{}.then", input.path),
        None,
    );
    let else_block = body.reopen_after_terminated_branch(inventory);
    patch_branch_else_block(inventory, branch_block, else_block);
    terminate_return_expr(
        inventory,
        frame,
        plan,
        body,
        input.else_expr,
        &format!("{}.else", input.path),
        None,
    );
}

#[derive(Clone, Copy)]
struct IfLetValueExprInput<'a> {
    pattern: &'a RuntimePattern,
    expr: &'a RuntimeExpr,
    guard: Option<&'a RuntimeExpr>,
    then_expr: &'a RuntimeExpr,
    else_expr: &'a RuntimeExpr,
    path: &'a str,
}

fn lower_if_let_value_expr(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    plan: &RuntimePlan,
    body: &mut ExprBodyBuilder,
    input: IfLetValueExprInput<'_>,
) {
    let value = AwbcExprLowerer::new(inventory, frame, input.path, plan).lower(input.expr);
    let pattern = lower_branch_pattern(inventory, plan, frame, input.pattern);
    let matched = frame.temp(inventory.bool_ty());
    inventory.push_instruction(AwbcInstruction::TestPattern {
        dst: matched,
        pattern,
        value,
    });
    let candidate_block = AwbcBlockId(table_index(
        inventory.program.blocks.len().saturating_add(1),
    ));
    let branch_block = body.close_block(
        inventory,
        AwbcTerminator::Branch {
            condition: matched,
            then_block: candidate_block,
            else_block: candidate_block,
        },
        AwbcSafePointKind::CallableBoundary,
    );

    if let Some(guard) = input.guard {
        let scope = enter_guard_pattern_scope(inventory, plan, frame, pattern, value, guard);
        let guard = AwbcExprLowerer::new(inventory, frame, format!("{}.guard", input.path), plan)
            .lower(guard);
        let then_block = AwbcBlockId(table_index(
            inventory.program.blocks.len().saturating_add(1),
        ));
        let guard_branch_block = body.close_block(
            inventory,
            AwbcTerminator::Branch {
                condition: guard,
                then_block,
                else_block: then_block,
            },
            AwbcSafePointKind::CallableBoundary,
        );
        inventory.push_instruction(AwbcInstruction::ExitScope { scope });
        frame.exit_scope();
        let body_scope = enter_pattern_scope(inventory, frame, pattern, value);
        terminate_return_expr(
            inventory,
            frame,
            plan,
            body,
            input.then_expr,
            &format!("{}.then", input.path),
            Some(body_scope),
        );
        let guard_false_block = body.reopen_after_terminated_branch(inventory);
        patch_branch_else_block(inventory, guard_branch_block, guard_false_block);
        inventory.push_instruction(AwbcInstruction::ExitScope { scope });
        let guard_false_jump = body.close_block(
            inventory,
            AwbcTerminator::Jump {
                target: AwbcBlockId::default(),
            },
            AwbcSafePointKind::CallableBoundary,
        );
        let else_block = AwbcBlockId(table_index(inventory.program.blocks.len()));
        patch_branch_else_block(inventory, branch_block, else_block);
        patch_jump_target(inventory, guard_false_jump, else_block);
        terminate_return_expr(
            inventory,
            frame,
            plan,
            body,
            input.else_expr,
            &format!("{}.else", input.path),
            None,
        );
    } else {
        let scope = enter_pattern_scope(inventory, frame, pattern, value);
        terminate_return_expr(
            inventory,
            frame,
            plan,
            body,
            input.then_expr,
            &format!("{}.then", input.path),
            Some(scope),
        );
        let else_block = body.reopen_after_terminated_branch(inventory);
        patch_branch_else_block(inventory, branch_block, else_block);
        terminate_return_expr(
            inventory,
            frame,
            plan,
            body,
            input.else_expr,
            &format!("{}.else", input.path),
            None,
        );
    }
}

fn lower_match_value_expr(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    plan: &RuntimePlan,
    body: &mut ExprBodyBuilder,
    scrutinee: &RuntimeExpr,
    arms: &[RuntimeExprMatchArm],
    path: &str,
) {
    let scrutinee = AwbcExprLowerer::new(inventory, frame, path, plan).lower(scrutinee);
    for (index, arm) in arms.iter().enumerate() {
        let pattern = lower_branch_pattern(inventory, plan, frame, arm.pattern());
        let matched = frame.temp(inventory.bool_ty());
        inventory.push_instruction(AwbcInstruction::TestPattern {
            dst: matched,
            pattern,
            value: scrutinee,
        });
        let candidate_block = AwbcBlockId(table_index(
            inventory.program.blocks.len().saturating_add(1),
        ));
        let branch_block = body.close_block(
            inventory,
            AwbcTerminator::Branch {
                condition: matched,
                then_block: candidate_block,
                else_block: candidate_block,
            },
            AwbcSafePointKind::CallableBoundary,
        );

        if let Some(guard) = arm.guard() {
            let scope =
                enter_guard_pattern_scope(inventory, plan, frame, pattern, scrutinee, guard);
            let guard =
                AwbcExprLowerer::new(inventory, frame, format!("{path}.arm.{index}.guard"), plan)
                    .lower(guard);
            let body_block = AwbcBlockId(table_index(
                inventory.program.blocks.len().saturating_add(1),
            ));
            let guard_branch_block = body.close_block(
                inventory,
                AwbcTerminator::Branch {
                    condition: guard,
                    then_block: body_block,
                    else_block: body_block,
                },
                AwbcSafePointKind::CallableBoundary,
            );
            inventory.push_instruction(AwbcInstruction::ExitScope { scope });
            frame.exit_scope();
            let body_scope = enter_pattern_scope(inventory, frame, pattern, scrutinee);
            terminate_return_expr(
                inventory,
                frame,
                plan,
                body,
                arm.value(),
                &format!("{path}.arm.{index}.value"),
                Some(body_scope),
            );
            let guard_false_block = body.reopen_after_terminated_branch(inventory);
            patch_branch_else_block(inventory, guard_branch_block, guard_false_block);
            inventory.push_instruction(AwbcInstruction::ExitScope { scope });
            let guard_false_jump = body.close_block(
                inventory,
                AwbcTerminator::Jump {
                    target: AwbcBlockId::default(),
                },
                AwbcSafePointKind::CallableBoundary,
            );
            let next_arm_block = AwbcBlockId(table_index(inventory.program.blocks.len()));
            patch_branch_else_block(inventory, branch_block, next_arm_block);
            patch_jump_target(inventory, guard_false_jump, next_arm_block);
        } else {
            let scope = enter_pattern_scope(inventory, frame, pattern, scrutinee);
            terminate_return_expr(
                inventory,
                frame,
                plan,
                body,
                arm.value(),
                &format!("{path}.arm.{index}.value"),
                Some(scope),
            );
            let next_arm_block = body.reopen_after_terminated_branch(inventory);
            patch_branch_else_block(inventory, branch_block, next_arm_block);
        }
    }
    let message = inventory.intern_string("match pattern did not match");
    body.terminate(
        inventory,
        AwbcTerminator::Trap {
            code: AwbcTrapCode::PatternMismatch,
            message: Some(message),
        },
        AwbcSafePointKind::CallableBoundary,
    );
}

fn terminate_return_expr(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    plan: &RuntimePlan,
    body: &mut ExprBodyBuilder,
    expr: &RuntimeExpr,
    path: &str,
    exit_scope: Option<AwbcScopeId>,
) {
    let mut value = AwbcExprLowerer::new(inventory, frame, path, plan).lower(expr);
    if let Some(scope) = exit_scope {
        let scoped_value = value;
        value = frame.root_temp(admitted_plan_type(inventory, plan, expr.ty()));
        inventory.push_instruction(AwbcInstruction::Move {
            dst: value,
            src: scoped_value,
        });
        inventory.push_instruction(AwbcInstruction::ExitScope { scope });
        frame.exit_scope();
    }
    body.terminate(
        inventory,
        AwbcTerminator::Return { value: Some(value) },
        AwbcSafePointKind::CallableBoundary,
    );
}

fn lower_branch_pattern(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
    frame: &mut FrameBuilder,
    pattern: &RuntimePattern,
) -> AwbcPatternId {
    let restored_scopes = frame.scope_checkpoint();
    let _ = frame.enter_scope();
    let pattern = lower_pattern(inventory, plan, frame, pattern);
    frame.restore_scopes_after_branch(restored_scopes);
    pattern
}

fn enter_pattern_scope(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    pattern: AwbcPatternId,
    value: AwbcRegisterId,
) -> AwbcScopeId {
    let scope = frame.enter_scope();
    inventory.push_instruction(AwbcInstruction::EnterScope { scope });
    inventory.push_instruction(AwbcInstruction::BindPattern {
        pattern,
        value,
        mode: AwbcBindMode::Declare,
    });
    scope
}

pub(super) fn enter_guard_pattern_scope(
    inventory: &mut AwbcInventory,
    _plan: &RuntimePlan,
    frame: &mut FrameBuilder,
    pattern: AwbcPatternId,
    value: AwbcRegisterId,
    guard: &RuntimeExpr,
) -> AwbcScopeId {
    enter_guard_pattern_scope_for_locals(
        inventory,
        frame,
        pattern,
        value,
        guard.guard_copy_locals(),
    )
}

pub(super) fn enter_guard_pattern_scope_for_locals(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    pattern: AwbcPatternId,
    value: AwbcRegisterId,
    locals: &[arcweft_core::runtime_id::RuntimeLocalDeclarationId],
) -> AwbcScopeId {
    let used = locals
        .iter()
        .map(|local| {
            frame
                .register_for_local(*local)
                .expect("checked guard local has a frame register")
        })
        .collect();
    let pattern = crate::awbc_lower::pattern::guard_pattern(inventory, pattern, &used);
    let scope = frame.enter_scope();
    inventory.push_instruction(AwbcInstruction::EnterScope { scope });
    inventory.push_instruction(AwbcInstruction::BindPattern {
        pattern,
        value,
        mode: AwbcBindMode::Guard,
    });
    scope
}

fn patch_branch_else_block(
    inventory: &mut AwbcInventory,
    branch_block: AwbcBlockId,
    else_block: AwbcBlockId,
) {
    let Some(block) = inventory.program.blocks.get_mut(branch_block.index()) else {
        return;
    };
    let AwbcTerminator::Branch {
        else_block: target, ..
    } = &mut block.terminator
    else {
        return;
    };
    *target = else_block;
}

fn patch_jump_target(
    inventory: &mut AwbcInventory,
    jump_block: AwbcBlockId,
    target_block: AwbcBlockId,
) {
    let Some(block) = inventory.program.blocks.get_mut(jump_block.index()) else {
        return;
    };
    let AwbcTerminator::Jump { target } = &mut block.terminator else {
        return;
    };
    *target = target_block;
}

fn unary_op(op: RuntimeUnaryOp) -> AwbcUnaryOp {
    match op {
        RuntimeUnaryOp::Not => AwbcUnaryOp::Not,
        RuntimeUnaryOp::Neg => AwbcUnaryOp::Neg,
    }
}

fn binary_op(op: RuntimeBinaryOp) -> AwbcBinaryOp {
    match op {
        RuntimeBinaryOp::Eq => AwbcBinaryOp::Eq,
        RuntimeBinaryOp::Ne => AwbcBinaryOp::Ne,
        RuntimeBinaryOp::Lt => AwbcBinaryOp::Lt,
        RuntimeBinaryOp::Le => AwbcBinaryOp::Le,
        RuntimeBinaryOp::Gt => AwbcBinaryOp::Gt,
        RuntimeBinaryOp::Ge => AwbcBinaryOp::Ge,
        RuntimeBinaryOp::Add => AwbcBinaryOp::Add,
        RuntimeBinaryOp::Sub => AwbcBinaryOp::Sub,
        RuntimeBinaryOp::Mul => AwbcBinaryOp::Mul,
        RuntimeBinaryOp::Div => AwbcBinaryOp::Div,
        RuntimeBinaryOp::And => AwbcBinaryOp::And,
        RuntimeBinaryOp::Or => AwbcBinaryOp::Or,
    }
}
