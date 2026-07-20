use super::partial::expr_contains_partial_placeholder;
use super::support::{
    FixedLiteralSpreadSlot, call_arg_spread_value, fixed_literal_spread_slot_count,
    fixed_literal_spread_slots, signature_param_label, spread_item_type,
};
use super::{
    CallArg, Expr, FunctionSignature, TypeCheckError, TypeChecker, TypeExpressionId, TypeKind,
    TypedLoweringEvidence, TypedLoweringEvidenceKind,
};
use crate::checker::PendingCurriedHigherOrderArg;
use crate::env::FunctionParam;

impl TypeChecker<'_> {
    pub(super) fn check_named_signature_call(
        &mut self,
        expression_id: TypeExpressionId,
        name: &str,
        result_ty: TypeKind,
        signature: &FunctionSignature,
        args: &[CallArg],
        expected: Option<&TypeKind>,
    ) -> TypeKind {
        let final_group_timing = self.uses_final_group_effect_timing(name);
        if !final_group_timing {
            // Suspending callable kinds retain their pre-07.8 eager graph
            // behavior until their invocation ABI is specified.
            self.check_function_effects(name);
        }
        if let Some(partial_ty) =
            self.check_partial_signature_call(expression_id, name, signature, args, expected)
        {
            if !final_group_timing {
                let pending = self
                    .last_checked_curried_signature_call
                    .as_mut()
                    .map(|value| std::mem::take(&mut value.pending_higher_order_args))
                    .unwrap_or_default();
                self.record_pending_curried_higher_order_arg_effect_calls(name, &pending);
            }
            return partial_ty;
        }

        let mut pending = self.check_signature_call_args(name, signature, args);
        let completes_group = Self::signature_call_supplies_current_group(signature, args);
        if final_group_timing && signature.remaining_call_groups() == 0 && completes_group {
            self.check_function_effects(name);
            self.record_pending_curried_higher_order_arg_effect_calls(name, &pending);
        } else if !final_group_timing {
            self.record_pending_curried_higher_order_arg_effect_calls(name, &pending);
            pending.clear();
        }
        self.record_curried_signature_result(
            name,
            1,
            &result_ty,
            signature.remaining_param_group(0).is_some(),
            completes_group,
            pending,
        );
        if signature.remaining_param_group(0).is_some()
            && matches!(result_ty, TypeKind::Function { .. })
        {
            self.last_checked_closure_effect_callable = self.source_function_effect_callable(name);
        } else {
            self.record_function_return_effect_result(name, &result_ty);
        }
        result_ty
    }

    pub(super) fn check_untyped_function_args(&mut self, args: &[CallArg]) {
        for arg in args {
            self.check_expr(arg.value());
        }
    }

    pub(super) fn check_signature_call_args(
        &mut self,
        name: &str,
        signature: &FunctionSignature,
        args: &[CallArg],
    ) -> Vec<PendingCurriedHigherOrderArg> {
        let fixed = signature
            .params
            .iter()
            .filter(|param| !param.is_rest())
            .collect::<Vec<_>>();
        let rest = signature.params.iter().find(|param| param.is_rest());
        let mut state = SignatureArgCheckState::new(fixed.len());

        for arg in args {
            match arg {
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.check_named_signature_arg(name, arg_name, value, &fixed, rest, &mut state);
                }
                CallArg::Spread { value } => match fixed_literal_spread_slots(value) {
                    Some(slots) if rest.is_none() => {
                        self.reserve_fixed_literal_spread_container_expr(value);
                        for slot_value in slots {
                            self.check_positional_signature_arg_slot(
                                name, slot_value, &fixed, &mut state,
                            );
                        }
                    }
                    Some(_) | None => {
                        self.check_signature_spread_arg(
                            name,
                            value,
                            rest,
                            &fixed,
                            &state.provided_fixed,
                        );
                    }
                },
                CallArg::Positional(positional) => {
                    if state.positional_index < fixed.len() {
                        self.check_positional_signature_arg_slot(
                            name,
                            FixedLiteralSpreadSlot::Expr(positional),
                            &fixed,
                            &mut state,
                        );
                    } else if let Some(param) = rest {
                        let label = param.name.as_deref().unwrap_or("#rest");
                        let actual =
                            self.expect_signature_arg_type(name, label, positional, &param.ty);
                        state.pending_higher_order_args.extend(
                            self.higher_order_signature_arg_effect_calls(
                                param,
                                positional,
                                actual.as_ref(),
                            ),
                        );
                    } else {
                        let message = if signature.remaining_call_groups() > 0 {
                            format!(
                                "function `{name}` received too many positional arguments for this call group; use separate calls for curried parameter groups"
                            )
                        } else {
                            format!("function `{name}` received too many positional arguments")
                        };
                        self.errors.push(TypeCheckError::new(message));
                        self.check_expr(positional);
                    }
                }
            }
        }

        for (index, param) in fixed.iter().enumerate() {
            if !state.provided_fixed[index] && !param.has_default {
                let label = param
                    .name
                    .as_deref()
                    .map_or_else(|| format!("#{index}"), ToOwned::to_owned);
                self.errors.push(TypeCheckError::new(format!(
                    "function `{name}` missing required argument `{label}`"
                )));
            }
        }
        state.pending_higher_order_args
    }

    fn check_positional_signature_arg_slot(
        &mut self,
        function_name: &str,
        value: FixedLiteralSpreadSlot<'_>,
        fixed: &[&FunctionParam],
        state: &mut SignatureArgCheckState,
    ) {
        while state.positional_index < fixed.len() && state.provided_fixed[state.positional_index] {
            state.positional_index += 1;
        }
        let Some(param) = fixed.get(state.positional_index) else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` received too many positional arguments"
            )));
            self.check_fixed_literal_spread_slot(value, None);
            return;
        };
        state.provided_fixed[state.positional_index] = true;
        let label = signature_param_label(param, state.positional_index);
        state.positional_index += 1;
        let actual = self.expect_signature_arg_slot_type(function_name, &label, value, &param.ty);
        if let Some(expr) = value.source_expr() {
            state
                .pending_higher_order_args
                .extend(self.higher_order_signature_arg_effect_calls(param, expr, actual.as_ref()));
        }
    }

    pub(super) fn check_partial_signature_call(
        &mut self,
        expression_id: TypeExpressionId,
        name: &str,
        signature: &FunctionSignature,
        args: &[CallArg],
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        if !signature.checks_args() || args.is_empty() {
            return None;
        }
        match expected {
            Some(TypeKind::Function { .. }) => {}
            None if self.allow_inferred_signature_partial_calls => {}
            Some(_) | None => return None,
        }
        let params = signature.params();
        if params
            .iter()
            .any(|param| param.is_rest() || param.has_default())
        {
            return None;
        }
        if let Some(reason) = unsupported_signature_partial_spread_reason(params, args) {
            self.errors
                .push(TypeCheckError::unsupported_signature_partial_call(
                    name, reason,
                ));
            for arg in args {
                if !expr_contains_partial_placeholder(arg.value()) {
                    self.check_expr(arg.value());
                }
            }
            return Some(TypeKind::Named("_".to_owned()));
        }
        let provided = partial_signature_call_args(params, args)?;
        let missing = provided
            .iter()
            .enumerate()
            .filter_map(|(index, value)| value.is_none().then_some(index))
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return None;
        }

        let mut pending_higher_order_args = Vec::new();
        for (index, (value, param)) in provided.iter().zip(params).enumerate() {
            let Some(value) = value else {
                continue;
            };
            let label = signature_param_label(param, index);
            let actual = self.expect_signature_arg_slot_type(name, &label, *value, param.ty());
            if let Some(expr) = value.source_expr() {
                pending_higher_order_args.extend(self.higher_order_signature_arg_effect_calls(
                    param,
                    expr,
                    actual.as_ref(),
                ));
            }
        }

        let callable_ty = if self.uses_final_group_effect_timing(name) {
            signature.function_value_type_with_effects(self.function_effect_row(name))
        } else {
            signature.function_value_type()
        }
        .expect("checked signature partial call owns parameter metadata");
        let TypeKind::Function {
            return_type,
            effects,
            ..
        } = callable_ty
        else {
            unreachable!("signature function value must retain a function type");
        };
        let result_ty = TypeKind::function_with_effects(
            missing.iter().map(|index| params[*index].ty().clone()),
            *return_type,
            effects,
        );
        self.record_typed_lowering_evidence(TypedLoweringEvidence::new(
            expression_id,
            TypedLoweringEvidenceKind::SignaturePartialCall {
                callee: name.to_owned(),
                result_ty: result_ty.clone(),
                arg_count: args.len(),
            },
        ));
        self.last_checked_curried_signature_call = Some(super::super::CurriedSignatureCallValue {
            function_name: name.to_owned(),
            remaining_group_index: 0,
            group_arg_offset: 0,
            current_group_params: Some(
                missing.iter().map(|index| params[*index].clone()).collect(),
            ),
            pending_higher_order_args,
            resolved: None,
        });
        self.last_checked_closure_effect_callable = self.source_function_effect_callable(name);
        Some(result_ty)
    }

    pub(super) fn signature_call_supplies_current_group(
        signature: &FunctionSignature,
        args: &[CallArg],
    ) -> bool {
        let fixed = signature
            .params()
            .iter()
            .filter(|param| !param.is_rest())
            .collect::<Vec<_>>();
        if signature.params().iter().any(FunctionParam::is_rest) {
            return false;
        }

        let mut provided_fixed = vec![false; fixed.len()];
        let mut positional_index = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(_) => {
                    while positional_index < fixed.len() && provided_fixed[positional_index] {
                        positional_index += 1;
                    }
                    let Some(provided) = provided_fixed.get_mut(positional_index) else {
                        return false;
                    };
                    *provided = true;
                    positional_index += 1;
                }
                CallArg::Named { name, .. } => {
                    let Some(index) = fixed
                        .iter()
                        .position(|param| param.name() == Some(name.as_str()))
                    else {
                        return false;
                    };
                    if provided_fixed[index] {
                        return false;
                    }
                    provided_fixed[index] = true;
                }
                CallArg::Spread { value } => {
                    let Some(slots) = fixed_literal_spread_slots(value) else {
                        return false;
                    };
                    for _ in slots {
                        while positional_index < fixed.len() && provided_fixed[positional_index] {
                            positional_index += 1;
                        }
                        let Some(provided) = provided_fixed.get_mut(positional_index) else {
                            return false;
                        };
                        *provided = true;
                        positional_index += 1;
                    }
                }
            }
        }

        fixed
            .iter()
            .zip(provided_fixed)
            .all(|(param, provided)| provided || param.has_default())
    }

    fn check_signature_spread_arg(
        &mut self,
        function_name: &str,
        value: &Expr,
        rest: Option<&FunctionParam>,
        fixed: &[&FunctionParam],
        provided_fixed: &[bool],
    ) {
        let Some(rest) = rest else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` does not accept spread arguments"
            )));
            self.check_expr(value);
            return;
        };
        if fixed
            .iter()
            .zip(provided_fixed.iter().copied())
            .any(|(param, provided)| !provided && !param.has_default)
        {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` spread argument must appear after required fixed arguments"
            )));
            self.check_expr(value);
            return;
        }
        let actual = self.check_expr(value);
        let Some(actual) = actual.as_ref() else {
            return;
        };
        let Some(item) = spread_item_type(actual) else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` spread argument must have sequence type for rest parameter `{}`",
                rest.name.as_deref().unwrap_or("#rest")
            )));
            return;
        };
        if !self.types_compatible(&rest.ty, item) {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` spread items must have type {:?}, found {:?}",
                rest.ty, item
            )));
        }
    }

    fn check_named_signature_arg(
        &mut self,
        function_name: &str,
        arg_name: &str,
        value: &Expr,
        fixed: &[&FunctionParam],
        rest: Option<&FunctionParam>,
        state: &mut SignatureArgCheckState,
    ) {
        if rest.and_then(|param| param.name.as_deref()) == Some(arg_name) {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` rest parameter `{arg_name}` is positional-only"
            )));
            self.check_expr(value);
            return;
        }
        let Some(index) = fixed
            .iter()
            .position(|param| param.name.as_deref() == Some(arg_name))
        else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` has no parameter named `{arg_name}`"
            )));
            self.check_expr(value);
            return;
        };
        if state.provided_fixed[index] {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` argument `{arg_name}` was provided more than once"
            )));
        }
        state.provided_fixed[index] = true;
        let actual =
            self.expect_signature_arg_type(function_name, arg_name, value, &fixed[index].ty);
        state
            .pending_higher_order_args
            .extend(self.higher_order_signature_arg_effect_calls(
                fixed[index],
                value,
                actual.as_ref(),
            ));
    }

    pub(super) fn expect_signature_arg_type(
        &mut self,
        function_name: &str,
        arg_label: &str,
        arg: &Expr,
        expected: &TypeKind,
    ) -> Option<TypeKind> {
        self.expect_signature_arg_slot_type(
            function_name,
            arg_label,
            FixedLiteralSpreadSlot::Expr(arg),
            expected,
        )
    }

    fn expect_signature_arg_slot_type(
        &mut self,
        function_name: &str,
        arg_label: &str,
        arg: FixedLiteralSpreadSlot<'_>,
        expected: &TypeKind,
    ) -> Option<TypeKind> {
        let actual = self.check_fixed_literal_spread_slot(arg, Some(expected));
        if let Some(actual) = actual.as_ref()
            && !self.types_compatible(expected, actual)
        {
            self.errors.push(TypeCheckError::argument_type_mismatch(
                function_name,
                arg_label,
                expected.clone(),
                actual.clone(),
            ));
        }
        actual
    }

    pub(super) fn check_fixed_literal_spread_slot(
        &mut self,
        arg: FixedLiteralSpreadSlot<'_>,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        match arg {
            FixedLiteralSpreadSlot::Expr(expr) => self.check_expr_with_expected(expr, expected),
            FixedLiteralSpreadSlot::Int(literal) => {
                Some(self.check_fixed_literal_spread_int_slot(literal, expected))
            }
        }
    }

    pub(super) fn reserve_fixed_literal_spread_container_expr(&mut self, value: &Expr) {
        if matches!(value, Expr::BracketSeq(_) | Expr::NumericBracketSeq(_)) {
            self.stats.expressions += 1;
        }
    }

    fn check_fixed_literal_spread_int_slot(
        &mut self,
        literal: &arcweft_lang_syntax::expr::IntLiteral,
        expected: Option<&TypeKind>,
    ) -> TypeKind {
        let ty = literal.suffix().map_or_else(
            || {
                expected
                    .filter(|ty| ty.is_integer())
                    .cloned()
                    .unwrap_or_else(|| {
                        self.record_numeric_fallback_in_inferred_closure(
                            "integer spread slot",
                            TypeKind::I32,
                        );
                        TypeKind::I32
                    })
            },
            TypeKind::from,
        );
        self.validate_integer_literal(literal, &ty);
        ty
    }
}

struct SignatureArgCheckState {
    provided_fixed: Vec<bool>,
    positional_index: usize,
    pending_higher_order_args: Vec<PendingCurriedHigherOrderArg>,
}

impl SignatureArgCheckState {
    fn new(fixed_param_count: usize) -> Self {
        Self {
            provided_fixed: vec![false; fixed_param_count],
            positional_index: 0,
            pending_higher_order_args: Vec::new(),
        }
    }
}

fn fixed_literal_spread_args(args: &[CallArg]) -> bool {
    args.iter().filter_map(call_arg_spread_value).all(|value| {
        fixed_literal_spread_slot_count(value).is_some()
            && !expr_contains_partial_placeholder(value)
    })
}

fn partial_signature_call_args<'a>(
    params: &[FunctionParam],
    args: &'a [CallArg],
) -> Option<Vec<Option<FixedLiteralSpreadSlot<'a>>>> {
    let mut provided = std::iter::repeat_with(|| None)
        .take(params.len())
        .collect::<Vec<_>>();
    let mut positional_index = 0usize;

    for arg in args {
        match arg {
            CallArg::Positional(value) => {
                while positional_index < provided.len() && provided[positional_index].is_some() {
                    positional_index += 1;
                }
                let slot = provided.get_mut(positional_index)?;
                *slot = Some(FixedLiteralSpreadSlot::Expr(value));
                positional_index += 1;
            }
            CallArg::Named { name, value } => {
                let index = params
                    .iter()
                    .position(|param| param.name() == Some(name.as_str()))?;
                if provided[index].is_some() {
                    return None;
                }
                provided[index] = Some(FixedLiteralSpreadSlot::Expr(value));
            }
            CallArg::Spread { value } => {
                let slots = fixed_literal_spread_slots(value)?;
                for value in slots {
                    while positional_index < provided.len() && provided[positional_index].is_some()
                    {
                        positional_index += 1;
                    }
                    let slot = provided.get_mut(positional_index)?;
                    *slot = Some(value);
                    positional_index += 1;
                }
            }
        }
    }

    Some(provided)
}

fn unsupported_signature_partial_spread_reason(
    params: &[FunctionParam],
    args: &[CallArg],
) -> Option<&'static str> {
    if !args.iter().any(CallArg::is_spread) {
        return None;
    }
    if args
        .iter()
        .any(|arg| expr_contains_partial_placeholder(arg.value()))
    {
        return Some("spread arguments cannot be mixed with `_` placeholder partial calls");
    }
    if fixed_literal_spread_args(args) {
        return None;
    }
    if args.iter().filter(|arg| arg.is_spread()).count() > 1 {
        return Some(
            "multiple spread arguments cannot be used in partial-call construction; runtime expansion ranges are not specified",
        );
    }
    if spread_is_followed_by_fixed_call_arg(args) {
        return Some(
            "spread arguments cannot be followed by fixed partial-call arguments; runtime expansion order is not specified",
        );
    }
    let missing_fixed = fixed_signature_missing_inputs_ignoring_spread(params, args)?;
    missing_fixed.then_some(
        "spread arguments cannot be mixed with missing-input partial calls; supply fixed arguments explicitly or define a rest-parameter contract",
    )
}

fn spread_is_followed_by_fixed_call_arg(args: &[CallArg]) -> bool {
    let Some(spread_index) = args.iter().position(CallArg::is_spread) else {
        return false;
    };
    args.iter()
        .skip(spread_index + 1)
        .any(|arg| !arg.is_spread())
}

fn fixed_signature_missing_inputs_ignoring_spread(
    params: &[FunctionParam],
    args: &[CallArg],
) -> Option<bool> {
    let mut provided = vec![false; params.len()];
    let mut positional_index = 0usize;

    for arg in args {
        match arg {
            CallArg::Positional(_) => {
                while positional_index < provided.len() && provided[positional_index] {
                    positional_index += 1;
                }
                let slot = provided.get_mut(positional_index)?;
                *slot = true;
                positional_index += 1;
            }
            CallArg::Named { name, .. } => {
                let index = params
                    .iter()
                    .position(|param| param.name() == Some(name.as_str()))?;
                if provided[index] {
                    return None;
                }
                provided[index] = true;
            }
            CallArg::Spread { .. } => {}
        }
    }

    Some(provided.iter().any(|provided| !provided))
}
