use super::{
    Engine, FlowFiberStatus, RuntimeDiagnostic, RuntimeEvalError, RuntimeExpr, RuntimeExprMatchArm,
    RuntimeMatchArm, RuntimeMatchSelection, RuntimePattern, RuntimeSeq, RuntimeStepOutput,
    RuntimeValue, evaluate_binary, evaluate_unary, match_runtime_pattern,
    runtime_sequence_dense_i64, runtime_sequence_from_literal_values,
    runtime_sequence_repeat_value, runtime_sequence_values, runtime_value_into_sequence_values,
    runtime_value_label, sum_i64_sequence_ref,
};
use crate::pattern::{RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeVariantIdentity};
use crate::plan::{
    FlowRuntimeId, RuntimePlanTypeDeclaration, RuntimePlanTypeProjection, RuntimePureInputType,
    RuntimePureOutputType,
};
use crate::pure::{RuntimeCallBackend, RuntimeI64Args, VmRuntimePureCallBackend};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::value::RuntimeBinaryOp;
use crate::value::{
    RuntimeAgentExpr, RuntimeAgentValue, RuntimeCallArgumentMode, RuntimeCallTarget,
    RuntimeEntityReferenceField, RuntimeExprKind, RuntimeFieldProjection, RuntimeIntrinsic,
    evaluate_core_iter_into_iter_intrinsic, evaluate_core_iter_next_intrinsic,
    evaluate_core_option_is_some_intrinsic, evaluate_core_option_unwrap_intrinsic,
};
use crate::value::{RuntimeLocalBinding, RuntimeNominalRecordExpr};
use crate::value::{
    RuntimeReductionValue, evaluate_core_iter_collect_intrinsic, evaluate_core_range_intrinsic,
    evaluate_index_intrinsic, evaluate_std_float_intrinsic, evaluate_string_intrinsic,
};

mod calls;
mod function;
mod sequence;

impl Engine {
    pub(super) fn evaluate_let_with_backend(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        match self
            .evaluate_expr_with_backend(expr, pure_backend)
            .and_then(|value| {
                self.try_bind_pattern(pattern, &value)
                    .map(|matched| (matched, value))
            }) {
            Ok((true, _)) => {}
            Ok((false, value)) => {
                self.fail_eval(
                    RuntimeEvalError::PatternMismatch(runtime_value_label(&value)),
                    output,
                );
            }
            Err(error) => self.fail_eval(error, output),
        }
    }

    pub(super) fn evaluate_if_let_with_backend(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<Vec<RuntimeLocalBinding>>, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        let Some(bindings) = match_runtime_pattern(&self.plan, pattern, &value)? else {
            return Ok(None);
        };
        if let Some(guard) = guard {
            let matched = self.with_temp_bindings_ref(&bindings, |this| {
                this.evaluate_bool_with_backend(guard, pure_backend)
            })?;
            if !matched {
                return Ok(None);
            }
        }
        Ok(Some(bindings))
    }

    pub(super) fn evaluate_match_with_backend(
        &mut self,
        scrutinee: &RuntimeExpr,
        arms: Vec<RuntimeMatchArm>,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeMatchSelection, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(scrutinee, pure_backend)?;
        for arm in arms {
            let Some(bindings) = match_runtime_pattern(&self.plan, &arm.pattern, &value)? else {
                continue;
            };
            if let Some(guard) = arm.guard.as_ref()
                && !self.with_temp_bindings_ref(&bindings, |this| {
                    this.evaluate_bool_with_backend(guard, pure_backend)
                })?
            {
                continue;
            }
            return Ok(Some((bindings, arm.ops)));
        }
        Ok(None)
    }

    pub(super) fn evaluate_expr(
        &mut self,
        expr: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut pure_backend = VmRuntimePureCallBackend::default();
        self.evaluate_expr_with_backend(expr, &mut pure_backend)
    }

    pub(super) fn evaluate_expr_with_backend(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        match expr.kind() {
            RuntimeExprKind::Value(value) => Ok(value.clone()),
            RuntimeExprKind::Agent(agent) => self.evaluate_agent_expr(agent, pure_backend),
            RuntimeExprKind::Local(local) => self
                .fiber
                .env
                .get(*local)
                .cloned()
                .ok_or(RuntimeEvalError::UnknownLocal(*local)),
            RuntimeExprKind::EntityRef(target) => {
                Ok(RuntimeValue::EntityRef(target.runtime_label()))
            }
            RuntimeExprKind::Let {
                binding,
                expr,
                body,
            } => self.evaluate_let_expr(*binding, expr, body, pure_backend),
            RuntimeExprKind::Tuple(_)
            | RuntimeExprKind::BracketSeq(_)
            | RuntimeExprKind::RepeatSeq { .. }
            | RuntimeExprKind::Range { .. }
            | RuntimeExprKind::NominalRecord(_)
            | RuntimeExprKind::Variant { .. }
            | RuntimeExprKind::Field { .. }
            | RuntimeExprKind::ProjectTuple { .. }
            | RuntimeExprKind::ProjectRecord { .. }
            | RuntimeExprKind::AssignNominalField { .. } => {
                self.evaluate_data_expr(expr, pure_backend)
            }
            RuntimeExprKind::Call { callee, args } => {
                self.evaluate_call_expr(callee, args, pure_backend)
            }
            RuntimeExprKind::Function(site) => self.evaluate_function_expr(*site),
            RuntimeExprKind::Apply { callee, args } => {
                self.evaluate_apply_expr(callee, args, pure_backend)
            }
            RuntimeExprKind::TraitCall {
                callable,
                receiver,
                receiver_mode,
                args,
            } => self
                .evaluate_trait_method_call(*callable, *receiver_mode, receiver, args, pure_backend)
                .map(|outcome| outcome.value),
            RuntimeExprKind::PureCall { helper, args } => {
                self.evaluate_pure_call_expr(*helper, args, pure_backend)
            }
            RuntimeExprKind::Map {
                source,
                param,
                body,
            } => self.evaluate_map_expr(source, *param, body, pure_backend),
            RuntimeExprKind::Filter {
                source,
                param,
                body,
            } => self.evaluate_filter_expr(source, *param, body, pure_backend),
            RuntimeExprKind::Sum { source } => self.evaluate_sum_expr(source, pure_backend),
            RuntimeExprKind::Unary { op, expr } => {
                self.evaluate_unary_expr(*op, expr, pure_backend)
            }
            RuntimeExprKind::Binary { lhs, op, rhs } => {
                self.evaluate_binary_expr(lhs, *op, rhs, pure_backend)
            }
            RuntimeExprKind::If {
                condition,
                then_expr,
                else_expr,
            } => self.evaluate_if_expr(condition, then_expr, else_expr, pure_backend),
            RuntimeExprKind::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => self.evaluate_if_let_expr(
                pattern,
                expr,
                guard.as_deref(),
                then_expr,
                else_expr,
                pure_backend,
            ),
            RuntimeExprKind::Match { scrutinee, arms } => {
                self.evaluate_match_expr(scrutinee, arms, pure_backend)
            }
            RuntimeExprKind::ReductionUnchanged { state } => {
                self.evaluate_reduction_unchanged(expr.ty(), state, pure_backend)
            }
        }
    }

    fn evaluate_unary_expr(
        &mut self,
        op: crate::value::RuntimeUnaryOp,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        evaluate_unary(op, value)
    }

    fn evaluate_binary_expr(
        &mut self,
        lhs: &RuntimeExpr,
        op: RuntimeBinaryOp,
        rhs: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let lhs = self.evaluate_expr_with_backend(lhs, pure_backend)?;
        let rhs = self.evaluate_expr_with_backend(rhs, pure_backend)?;
        evaluate_binary(lhs, op, rhs)
    }

    fn evaluate_agent_expr(
        &mut self,
        agent: &RuntimeAgentExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut operands = Vec::new();
        if let Some(choice) = agent.choice() {
            operands.push(RuntimeValue::EntityRef(choice.as_str().to_owned()));
        }
        for operand in agent.operands() {
            operands.push(self.evaluate_expr_with_backend(operand, pure_backend)?);
        }
        RuntimeAgentValue::try_construct(agent.constructor(), operands)
            .map(RuntimeValue::Agent)
            .map_err(|error| RuntimeEvalError::AgentConstruction(error.to_string()))
    }

    fn evaluate_data_expr(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        match expr.kind() {
            RuntimeExprKind::Tuple(items) => items
                .iter()
                .map(|item| self.evaluate_expr_with_backend(item, pure_backend))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::Tuple),
            RuntimeExprKind::BracketSeq(items) => {
                self.evaluate_bracket_seq_expr(items, pure_backend)
            }
            RuntimeExprKind::RepeatSeq { value, len } => {
                self.evaluate_repeat_seq_expr(value, *len, pure_backend)
            }
            RuntimeExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                self.evaluate_range_expr(start.as_deref(), end.as_deref(), *inclusive, pure_backend)
            }
            RuntimeExprKind::NominalRecord(record) => {
                self.evaluate_nominal_record_expr(expr.ty(), record, pure_backend)
            }
            RuntimeExprKind::Variant { ordinal, payload } => {
                self.evaluate_variant_expr(expr.ty(), *ordinal, payload.as_deref(), pure_backend)
            }
            RuntimeExprKind::Field { target, field } => {
                self.evaluate_field_expr(target, *field, pure_backend)
            }
            RuntimeExprKind::ProjectTuple { target, ordinal } => {
                self.evaluate_project_tuple_expr(target, *ordinal, pure_backend)
            }
            RuntimeExprKind::ProjectRecord { target, ordinal } => {
                self.evaluate_project_record_expr(target, *ordinal, pure_backend)
            }
            RuntimeExprKind::AssignNominalField {
                base,
                field,
                expr,
                body,
            } => self.evaluate_assign_field_expr(*base, *field, expr, body, pure_backend),
            _ => unreachable!("data expression helper received non-data expression"),
        }
    }

    fn evaluate_bracket_seq_expr(
        &mut self,
        items: &[RuntimeExpr],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let Some((helper_id, arity)) = self.bracket_seq_i64_batch_shape(items) {
            let mut flat_inputs = std::mem::take(&mut self.pure_i64_batch_inputs);
            let collect_result =
                self.collect_i64_pure_batch_inputs(items, arity, pure_backend, &mut flat_inputs);
            if let Err(error) = collect_result {
                self.pure_i64_batch_inputs = flat_inputs;
                return Err(error);
            }
            let batch_result = self.call_i64_flat_batch_with_outputs(
                helper_id,
                &flat_inputs,
                arity,
                items.len(),
                pure_backend,
                <[i64]>::to_vec,
            );
            self.pure_i64_batch_inputs = flat_inputs;
            let values = batch_result?;
            return Ok(runtime_sequence_dense_i64(values));
        }
        items
            .iter()
            .map(|item| self.evaluate_expr_with_backend(item, pure_backend))
            .collect::<Result<Vec<_>, _>>()
            .map(runtime_sequence_from_literal_values)
    }

    fn evaluate_range_expr(
        &mut self,
        start: Option<&RuntimeExpr>,
        end: Option<&RuntimeExpr>,
        inclusive: bool,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let start = start
            .map(|expr| self.evaluate_expr_with_backend(expr, pure_backend))
            .transpose()?;
        let end = end
            .map(|expr| self.evaluate_expr_with_backend(expr, pure_backend))
            .transpose()?;
        crate::value::RuntimeRange::new(start, end, inclusive).map(RuntimeValue::Range)
    }

    fn evaluate_repeat_seq_expr(
        &mut self,
        value: &RuntimeExpr,
        len: usize,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let RuntimeExprKind::Value(value) = value.kind() {
            return Ok(runtime_sequence_repeat_value(value, len));
        }
        (0..len)
            .map(|_| self.evaluate_expr_with_backend(value, pure_backend))
            .collect::<Result<Vec<_>, _>>()
            .map(runtime_sequence_values)
    }

    fn bracket_seq_i64_batch_shape(
        &self,
        items: &[RuntimeExpr],
    ) -> Option<(crate::plan::RuntimePureHelperId, usize)> {
        let (first_helper, first_args) = match items.first()?.kind() {
            RuntimeExprKind::PureCall { helper, args } => (*helper, args),
            _ => return None,
        };
        if first_helper.0 >= self.plan.pure_helpers.len() || first_args.len() > RuntimeI64Args::MAX
        {
            return None;
        }
        if !self
            .pure_helper_i64_call_shapes
            .get(first_helper.0)
            .copied()
            .unwrap_or(false)
        {
            return None;
        }
        let arity = first_args.len();
        items
            .iter()
            .all(|item| match item.kind() {
                RuntimeExprKind::PureCall { helper, args } => {
                    *helper == first_helper
                        && args.len() == arity
                        && args
                            .iter()
                            .all(|arg| arg.mode() == RuntimeCallArgumentMode::Value)
                }
                _ => false,
            })
            .then_some((first_helper, arity))
    }

    fn collect_i64_pure_batch_inputs(
        &mut self,
        items: &[RuntimeExpr],
        arity: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        flat_inputs: &mut Vec<i64>,
    ) -> Result<(), RuntimeEvalError> {
        flat_inputs.clear();
        flat_inputs.reserve(items.len().saturating_mul(arity));
        for item in items {
            let RuntimeExprKind::PureCall { args, .. } = item.kind() else {
                unreachable!("i64 pure batch shape checked before row collection");
            };
            for arg in args.iter().take(arity) {
                flat_inputs.push(self.evaluate_i64_arg_with_backend(arg.value(), pure_backend)?);
            }
        }
        Ok(())
    }

    fn call_i64_flat_batch_with_outputs<T>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[i64],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        map_outputs: impl FnOnce(&[i64]) -> T,
    ) -> Result<T, RuntimeEvalError> {
        let mut out = std::mem::take(&mut self.pure_i64_batch_outputs);
        out.resize(row_count, 0);
        let helper = crate::pure::RuntimePureHelperRef::resolve(&self.plan, helper_id)?;
        let batch_result = pure_backend.call_i64_flat_batch(helper, flat_inputs, arity, &mut out);
        if let Err(error) = batch_result {
            self.pure_i64_batch_outputs = out;
            return Err(error);
        }
        let result = map_outputs(&out);
        out.clear();
        self.pure_i64_batch_outputs = out;
        Ok(result)
    }

    fn evaluate_nominal_record_expr(
        &mut self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        record: &RuntimeNominalRecordExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let plan = std::sync::Arc::clone(&self.plan);
        let declaration = plan
            .type_table()
            .get(ty)
            .ok_or(RuntimeEvalError::UnknownPlanType(ty))?;
        let RuntimePlanTypeProjection::ProjectNominal {
            nominal, layout, ..
        } = declaration.projection()
        else {
            return Err(RuntimeEvalError::InvalidExpressionType(ty));
        };
        let domain = plan
            .nominal_record_domains()
            .get(ty)
            .ok_or(RuntimeEvalError::MissingNominalRecordDomain(ty))?;
        let mut fields = std::iter::repeat_with(|| None)
            .take(domain.fields().len())
            .collect::<Vec<_>>();
        for initializer in record.initializers() {
            let value = self.evaluate_expr_with_backend(initializer.value(), pure_backend)?;
            let ordinal = usize::try_from(initializer.field().zero_based())
                .map_err(|_| RuntimeEvalError::InvalidExpressionType(ty))?;
            let Some(field) = domain.fields().get(ordinal) else {
                return Err(RuntimeEvalError::InvalidExpressionType(ty));
            };
            if !plan.value_matches_type(field.ty(), &value)? {
                return Err(RuntimeEvalError::InvalidExpressionType(
                    initializer.value().ty(),
                ));
            }
            fields[ordinal] = Some(value);
        }
        let fields = fields
            .into_iter()
            .enumerate()
            .map(|(ordinal, field)| {
                let field_id =
                    crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                        .map_err(|_| RuntimeEvalError::InvalidExpressionType(ty))?;
                field.ok_or(RuntimeEvalError::MissingRecordInitializer {
                    ty,
                    field: field_id,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RuntimeValue::NominalRecord(
            crate::value::RuntimeNominalRecordValue::new(nominal.clone(), *layout, fields),
        ))
    }

    fn evaluate_variant_expr(
        &mut self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        ordinal: u32,
        payload: Option<&RuntimeExpr>,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let plan = std::sync::Arc::clone(&self.plan);
        let declaration = plan
            .type_table()
            .get(ty)
            .ok_or(RuntimeEvalError::UnknownPlanType(ty))?;
        let (owner, name) = match declaration.projection() {
            RuntimePlanTypeProjection::Option(_) => match ordinal {
                0 => (RuntimeVariantIdentity::Option, "Some".to_owned()),
                1 => (RuntimeVariantIdentity::Option, "None".to_owned()),
                _ => return Err(RuntimeEvalError::UnknownVariantCase { ty, ordinal }),
            },
            RuntimePlanTypeProjection::Result { .. } => match ordinal {
                0 => (RuntimeVariantIdentity::Result, "Ok".to_owned()),
                1 => (RuntimeVariantIdentity::Result, "Err".to_owned()),
                _ => return Err(RuntimeEvalError::UnknownVariantCase { ty, ordinal }),
            },
            RuntimePlanTypeProjection::ProjectNominal { .. }
            | RuntimePlanTypeProjection::Opaque { .. } => {
                let domain = plan
                    .variant_domains()
                    .get(ty)
                    .ok_or(RuntimeEvalError::MissingVariantDomain(ty))?;
                let case = domain
                    .case(ordinal)
                    .ok_or(RuntimeEvalError::UnknownVariantCase { ty, ordinal })?;
                (
                    RuntimeVariantIdentity::Nominal {
                        nominal: domain.nominal().clone(),
                        semantic_identity: declaration.semantic_identity(),
                    },
                    case.name().to_owned(),
                )
            }
            _ => return Err(RuntimeEvalError::InvalidExpressionType(ty)),
        };
        Ok(RuntimeValue::Variant {
            owner,
            ordinal,
            name,
            payload: payload
                .map(|expr| {
                    self.evaluate_expr_with_backend(expr, pure_backend)
                        .map(Box::new)
                })
                .transpose()?,
        })
    }

    fn evaluate_reduction_unchanged(
        &mut self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        state: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let plan = std::sync::Arc::clone(&self.plan);
        let declaration = plan
            .type_table()
            .get(ty)
            .ok_or(RuntimeEvalError::UnknownPlanType(ty))?;
        let RuntimePlanTypeProjection::Opaque {
            producer,
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: crate::value::RuntimeOpaqueValueClass::Plain,
            persistence: crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments,
        } = declaration.projection()
        else {
            return Err(RuntimeEvalError::InvalidExpressionType(ty));
        };
        let state_ty = match plan
            .type_table()
            .get(state.ty())
            .map(RuntimePlanTypeDeclaration::projection)
        {
            Some(RuntimePlanTypeProjection::Reference(inner)) => *inner,
            _ => state.ty(),
        };
        if arguments.as_ref() != [state_ty] {
            return Err(RuntimeEvalError::InvalidExpressionType(ty));
        }
        let producer = producer.clone();
        let semantic_identity = declaration.semantic_identity();
        let state = self.evaluate_expr_with_backend(state, pure_backend)?;
        if !plan.value_matches_type(state_ty, &state)? {
            return Err(RuntimeEvalError::InvalidExpressionType(ty));
        }
        RuntimeReductionValue::try_unchanged(
            RuntimeOpaqueTypeOwner::exact(producer, semantic_identity),
            state,
        )
        .map(RuntimeValue::Reduction)
        .map_err(|_| RuntimeEvalError::InvalidExpressionType(ty))
    }

    fn evaluate_field_expr(
        &mut self,
        target: &RuntimeExpr,
        field: RuntimeFieldProjection,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(target, pure_backend)?;
        match (field, value) {
            (RuntimeFieldProjection::Nominal(field), RuntimeValue::NominalRecord(record)) => record
                .field(field)
                .cloned()
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: field.zero_based().to_string(),
                    value: "nominal record".to_owned(),
                }),
            (RuntimeFieldProjection::EntityReference(field), RuntimeValue::EntityRef(id)) => {
                Ok(Self::entity_ref_field(&id, field))
            }
            (RuntimeFieldProjection::Agent(field), RuntimeValue::Agent(value)) => value
                .project_typed_field(field)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: field.as_label().to_owned(),
                    value: value.label().to_owned(),
                }),
            (RuntimeFieldProjection::Agent(field), RuntimeValue::Record(fields))
                if field.permits_protocol_record() =>
            {
                fields
                    .iter()
                    .find(|entry| entry.name() == field.as_label())
                    .map(|entry| entry.value().clone())
                    .ok_or_else(|| RuntimeEvalError::MissingField {
                        field: field.as_label().to_owned(),
                        value: "Agent protocol record".to_owned(),
                    })
            }
            (RuntimeFieldProjection::Progress(field), RuntimeValue::Progress(progress)) => {
                Ok(match field {
                    crate::value::RuntimeProgressField::Ratio => {
                        RuntimeValue::F32(progress.ratio())
                    }
                    crate::value::RuntimeProgressField::Label => progress
                        .label()
                        .map_or_else(RuntimeValue::option_none, |label| {
                            RuntimeValue::option_some(RuntimeValue::String(label.to_owned()))
                        }),
                })
            }
            value => Err(RuntimeEvalError::MissingField {
                field: field.label(),
                value: runtime_value_label(&value.1),
            }),
        }
    }

    fn entity_ref_field(id: &str, field: RuntimeEntityReferenceField) -> RuntimeValue {
        RuntimeValue::String(match field {
            RuntimeEntityReferenceField::Id => id.to_owned(),
            RuntimeEntityReferenceField::Family => Self::entity_ref_family(id).to_owned(),
            RuntimeEntityReferenceField::Name => Self::entity_ref_name(id).to_owned(),
        })
    }

    fn entity_ref_family(id: &str) -> &str {
        id.split_once('.').map_or(id, |(family, _)| family)
    }

    fn entity_ref_name(id: &str) -> &str {
        id.split_once('.').map_or("", |(_, name)| name)
    }

    fn evaluate_project_tuple_expr(
        &mut self,
        target: &RuntimeExpr,
        ordinal: usize,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(target, pure_backend)?;
        match value {
            RuntimeValue::Tuple(items) => {
                items
                    .into_iter()
                    .nth(ordinal)
                    .ok_or_else(|| RuntimeEvalError::MissingField {
                        field: ordinal.to_string(),
                        value: "tuple".to_owned(),
                    })
            }
            RuntimeValue::Seq(RuntimeSeq::TupleColumns(columns)) => columns
                .column(ordinal)
                .cloned()
                .map(RuntimeValue::Seq)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: ordinal.to_string(),
                    value: "tuple sequence".to_owned(),
                }),
            value => Err(RuntimeEvalError::MissingField {
                field: ordinal.to_string(),
                value: runtime_value_label(&value),
            }),
        }
    }

    fn evaluate_project_record_expr(
        &mut self,
        target: &RuntimeExpr,
        ordinal: usize,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(target, pure_backend)?;
        match value {
            RuntimeValue::Record(fields) => fields.into_iter().nth(ordinal).map_or_else(
                || {
                    Err(RuntimeEvalError::MissingField {
                        field: ordinal.to_string(),
                        value: "record".to_owned(),
                    })
                },
                |field| Ok(field.into_value()),
            ),
            RuntimeValue::Seq(RuntimeSeq::RecordColumns(records)) => records
                .field_by_ordinal(ordinal)
                .cloned()
                .map(RuntimeValue::Seq)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: ordinal.to_string(),
                    value: "record sequence".to_owned(),
                }),
            value => Err(RuntimeEvalError::MissingField {
                field: ordinal.to_string(),
                value: runtime_value_label(&value),
            }),
        }
    }

    fn evaluate_let_expr(
        &mut self,
        binding: RuntimeLocalDeclarationId,
        expr: &RuntimeExpr,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        self.fiber.env.push_scope_with_capacity(1);
        self.fiber.env.set(binding, value);
        let result = self.evaluate_expr_with_backend(body, pure_backend);
        self.fiber.env.pop_scope();
        result
    }

    fn evaluate_assign_field_expr(
        &mut self,
        base: RuntimeLocalDeclarationId,
        field: crate::value::RuntimeRecordFieldId,
        expr: &RuntimeExpr,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        self.fiber
            .env
            .set_record_field(base, field, value)
            .map_err(|target| RuntimeEvalError::InvalidFieldAssignment {
                field: field.zero_based().to_string(),
                value: runtime_value_label(&target),
            })?;
        self.evaluate_expr_with_backend(body, pure_backend)
    }

    fn evaluate_if_expr(
        &mut self,
        condition: &RuntimeExpr,
        then_expr: &RuntimeExpr,
        else_expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if self.evaluate_bool_with_backend(condition, pure_backend)? {
            self.evaluate_expr_with_backend(then_expr, pure_backend)
        } else {
            self.evaluate_expr_with_backend(else_expr, pure_backend)
        }
    }

    pub(super) fn evaluate_if_let_expr(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
        then_expr: &RuntimeExpr,
        else_expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        let Some(bindings) = match_runtime_pattern(&self.plan, pattern, &value)? else {
            return self.evaluate_expr_with_backend(else_expr, pure_backend);
        };
        let guard_matched = if let Some(guard) = guard {
            self.with_temp_bindings_ref(&bindings, |this| {
                this.evaluate_bool_with_backend(guard, pure_backend)
            })?
        } else {
            true
        };
        if guard_matched {
            self.with_temp_bindings(bindings, |this| {
                this.evaluate_expr_with_backend(then_expr, pure_backend)
            })
        } else {
            self.evaluate_expr_with_backend(else_expr, pure_backend)
        }
    }

    pub(super) fn evaluate_match_expr(
        &mut self,
        scrutinee: &RuntimeExpr,
        arms: &[RuntimeExprMatchArm],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(scrutinee, pure_backend)?;
        for arm in arms {
            let Some(bindings) = match_runtime_pattern(&self.plan, arm.pattern(), &value)? else {
                continue;
            };
            if let Some(guard) = arm.guard()
                && !self.with_temp_bindings_ref(&bindings, |this| {
                    this.evaluate_bool_with_backend(guard, pure_backend)
                })?
            {
                continue;
            }
            return self.with_temp_bindings(bindings, |this| {
                this.evaluate_expr_with_backend(arm.value(), pure_backend)
            });
        }
        Err(RuntimeEvalError::PatternMismatch(runtime_value_label(
            &value,
        )))
    }

    pub(super) fn evaluate_bool_with_backend(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<bool, RuntimeEvalError> {
        match self.evaluate_expr_with_backend(expr, pure_backend)? {
            RuntimeValue::Bool(value) => Ok(value),
            value => Err(RuntimeEvalError::ExpectedBool(runtime_value_label(&value))),
        }
    }

    pub(super) fn with_temp_bindings<I, T>(
        &mut self,
        bindings: I,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T
    where
        I: IntoIterator<Item = RuntimeLocalBinding>,
        I::IntoIter: ExactSizeIterator,
    {
        let bindings = bindings.into_iter();
        self.fiber.env.push_scope_with_capacity(bindings.len());
        self.fiber.env.bind_all(bindings);
        let result = f(self);
        self.fiber.env.pop_scope();
        result
    }

    pub(super) fn with_temp_bindings_ref<T>(
        &mut self,
        bindings: &[RuntimeLocalBinding],
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.fiber.env.push_scope_with_capacity(bindings.len());
        self.fiber.env.bind_all_ref(bindings);
        let result = f(self);
        self.fiber.env.pop_scope();
        result
    }

    pub(super) fn with_temp_binding_ref<T>(
        &mut self,
        local: RuntimeLocalDeclarationId,
        value: &RuntimeValue,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.fiber.env.push_scope_with_capacity(1);
        self.fiber.env.set_ref(local, value);
        let result = f(self);
        self.fiber.env.pop_scope();
        result
    }

    pub(super) fn evaluate_entity_target(
        &mut self,
        expr: &RuntimeExpr,
    ) -> Result<FlowRuntimeId, RuntimeEvalError> {
        match self.evaluate_expr(expr)? {
            RuntimeValue::EntityRef(target) | RuntimeValue::String(target) => self
                .plan
                .resolve_flow_target_value(&target)
                .map_err(|error| RuntimeEvalError::InvalidEntityTarget {
                    target,
                    reason: error.to_string(),
                }),
            value => Err(RuntimeEvalError::ExpectedEntityRef(runtime_value_label(
                &value,
            ))),
        }
    }

    pub(super) fn try_bind_pattern(
        &mut self,
        pattern: &RuntimePattern,
        value: &RuntimeValue,
    ) -> Result<bool, RuntimeEvalError> {
        let Some(bindings) = match_runtime_pattern(&self.plan, pattern, value)? else {
            return Ok(false);
        };
        self.fiber.env.bind_all(bindings);
        Ok(true)
    }

    pub(super) fn fail_eval(
        &mut self,
        error: impl std::fmt::Display,
        output: &mut RuntimeStepOutput,
    ) {
        let message = error.to_string();
        self.fiber.status = FlowFiberStatus::Failed(message.clone());
        output.diagnostics.push(RuntimeDiagnostic::new(message));
    }
}

pub(super) fn pure_helper_has_i64_call_shape(helper: &crate::plan::RuntimePureHelper) -> bool {
    helper.scalar_eval_supported
        && helper.output_type == RuntimePureOutputType::I64
        && pure_helper_has_only_inputs(helper, RuntimePureInputType::I64)
}

pub(super) fn pure_helper_has_u32_call_shape(helper: &crate::plan::RuntimePureHelper) -> bool {
    helper.scalar_eval_supported
        && helper.output_type == RuntimePureOutputType::U32
        && pure_helper_has_only_inputs(helper, RuntimePureInputType::U32)
}

fn pure_helper_has_only_inputs(
    helper: &crate::plan::RuntimePureHelper,
    expected: RuntimePureInputType,
) -> bool {
    helper.input_locals.len() == helper.input_types.len()
        && helper.input_types.iter().all(|ty| *ty == expected)
}

fn spread_runtime_values(value: RuntimeValue) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
    match runtime_value_into_sequence_values(value) {
        Ok(items) => Ok(items),
        Err(value) => Err(RuntimeEvalError::InvalidSpread(runtime_value_label(&value))),
    }
}

fn evaluate_core_iterator_intrinsic(
    intrinsic: RuntimeIntrinsic,
    args: &[RuntimeValue],
) -> Option<RuntimeValue> {
    match (intrinsic, args) {
        (RuntimeIntrinsic::CoreIterCollect, [value]) => Some(
            evaluate_core_iter_collect_intrinsic(value.clone()).unwrap_or_else(|error| {
                RuntimeValue::String(format!("core.iter.collect({error})"))
            }),
        ),
        (RuntimeIntrinsic::CoreIterIntoIter, [value, evidence]) => Some(
            evaluate_core_iter_into_iter_intrinsic(value.clone(), evidence).unwrap_or_else(
                |error| RuntimeValue::String(format!("core.iter.into_iter({error})")),
            ),
        ),
        (RuntimeIntrinsic::CoreIterNext, [value]) => Some(
            evaluate_core_iter_next_intrinsic(value.clone())
                .unwrap_or_else(|error| RuntimeValue::String(format!("core.iter.next({error})"))),
        ),
        (RuntimeIntrinsic::CoreOptionIsSome, [value]) => Some(
            evaluate_core_option_is_some_intrinsic(value).unwrap_or_else(|error| {
                RuntimeValue::String(format!("core.option.is_some({error})"))
            }),
        ),
        (RuntimeIntrinsic::CoreOptionUnwrap, [value]) => Some(
            evaluate_core_option_unwrap_intrinsic(value.clone()).unwrap_or_else(|error| {
                RuntimeValue::String(format!("core.option.unwrap({error})"))
            }),
        ),
        _ => None,
    }
}

pub(crate) fn evaluate_runtime_call(
    callee: &RuntimeCallTarget,
    args: &[RuntimeValue],
    pure_backend: &mut impl RuntimeCallBackend,
) -> RuntimeValue {
    if let Some(intrinsic) = callee.as_intrinsic()
        && let Ok(Some(value)) = evaluate_std_float_intrinsic(intrinsic, args)
    {
        return value;
    }
    if let Some(intrinsic) = callee.as_intrinsic()
        && let Ok(Some(value)) = evaluate_string_intrinsic(intrinsic, args)
    {
        return value;
    }
    if let Some(intrinsic) = callee.as_intrinsic()
        && let Ok(Some(value)) = evaluate_index_intrinsic(intrinsic, args)
    {
        return value;
    }
    if let Some(intrinsic) = callee.as_intrinsic()
        && let Some(value) = evaluate_core_iterator_intrinsic(intrinsic, args)
    {
        return value;
    }
    match (callee.as_intrinsic(), args) {
        (Some(RuntimeIntrinsic::Add), [RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)]) => {
            evaluate_binary(
                RuntimeValue::Int(*lhs),
                RuntimeBinaryOp::Add,
                RuntimeValue::Int(*rhs),
            )
            .unwrap_or_else(|_| RuntimeValue::String("add(<unsupported>)".to_owned()))
        }
        (Some(RuntimeIntrinsic::CoreRange), _) => evaluate_core_range_intrinsic(args)
            .unwrap_or_else(|error| RuntimeValue::String(format!("core.range({error})"))),
        (
            Some(RuntimeIntrinsic::MathMatmulF32),
            [RuntimeValue::MatrixF32(lhs), RuntimeValue::MatrixF32(rhs)],
        ) => pure_backend.call_math_matmul_f32(lhs, rhs).map_or_else(
            |error| RuntimeValue::String(format!("math.matmul_f32({error})")),
            RuntimeValue::matrix_f32,
        ),
        (
            Some(RuntimeIntrinsic::MathMatrixAddF32),
            [RuntimeValue::MatrixF32(lhs), RuntimeValue::MatrixF32(rhs)],
        ) => pure_backend.call_math_matrix_add_f32(lhs, rhs).map_or_else(
            |error| RuntimeValue::String(format!("math.matrix_add_f32({error})")),
            RuntimeValue::matrix_f32,
        ),
        (
            Some(RuntimeIntrinsic::MathTensorAddF32),
            [RuntimeValue::TensorF32(lhs), RuntimeValue::TensorF32(rhs)],
        ) => pure_backend.call_math_tensor_add_f32(lhs, rhs).map_or_else(
            |error| RuntimeValue::String(format!("math.tensor_add_f32({error})")),
            RuntimeValue::tensor_f32,
        ),
        (
            Some(RuntimeIntrinsic::MathMatmulF64),
            [RuntimeValue::MatrixF64(lhs), RuntimeValue::MatrixF64(rhs)],
        ) => pure_backend.call_math_matmul_f64(lhs, rhs).map_or_else(
            |error| RuntimeValue::String(format!("math.matmul_f64({error})")),
            RuntimeValue::matrix_f64,
        ),
        (
            Some(RuntimeIntrinsic::MathMatrixAddF64),
            [RuntimeValue::MatrixF64(lhs), RuntimeValue::MatrixF64(rhs)],
        ) => pure_backend.call_math_matrix_add_f64(lhs, rhs).map_or_else(
            |error| RuntimeValue::String(format!("math.matrix_add_f64({error})")),
            RuntimeValue::matrix_f64,
        ),
        (
            Some(RuntimeIntrinsic::MathTensorAddF64),
            [RuntimeValue::TensorF64(lhs), RuntimeValue::TensorF64(rhs)],
        ) => pure_backend.call_math_tensor_add_f64(lhs, rhs).map_or_else(
            |error| RuntimeValue::String(format!("math.tensor_add_f64({error})")),
            RuntimeValue::tensor_f64,
        ),
        (
            Some(
                intrinsic @ (RuntimeIntrinsic::PathSave
                | RuntimeIntrinsic::PathAsset
                | RuntimeIntrinsic::PathTemp
                | RuntimeIntrinsic::PathExport),
            ),
            [RuntimeValue::String(path)],
        ) => {
            let space = intrinsic.path_space().unwrap_or(intrinsic.as_label());
            RuntimeValue::String(format!("{space}:{path}"))
        }
        _ => pure_backend.call_external(callee, args).map_or_else(
            || {
                RuntimeValue::String(format!(
                    "{}({})",
                    callee.as_label(),
                    args.iter()
                        .map(runtime_value_label)
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            },
            |result| {
                result.unwrap_or_else(|error| {
                    RuntimeValue::String(format!("{}({error})", callee.as_label()))
                })
            },
        ),
    }
}
