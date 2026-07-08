use super::builtin::CapabilityFunctionSpec;
use super::partial::expr_contains_partial_placeholder;
use super::support::{signature_param_label, spread_item_type};
use super::{
    CallArg, Expr, FunctionSignature, TypeCheckError, TypeChecker, TypeExpressionId, TypeKind,
    TypedLoweringEvidence, TypedLoweringEvidenceKind,
};
use crate::env::FunctionParam;
use arcweft_lang_syntax::expr::Literal;

impl TypeChecker<'_> {
    pub(super) fn check_untyped_function_args(&mut self, name: &str, args: &[CallArg]) {
        let checked_args = if let Some(spec) = CapabilityFunctionSpec::resolve(name) {
            args.iter()
                .skip(spec.unchecked_prefix_args())
                .collect::<Vec<_>>()
        } else {
            args.iter().collect::<Vec<_>>()
        };
        for arg in checked_args {
            self.check_expr(arg.value());
        }
    }

    pub(super) fn check_signature_call_args(
        &mut self,
        name: &str,
        signature: &FunctionSignature,
        args: &[CallArg],
    ) {
        let fixed = signature
            .params
            .iter()
            .filter(|param| !param.is_rest())
            .collect::<Vec<_>>();
        let rest = signature.params.iter().find(|param| param.is_rest());
        let mut provided_fixed = vec![false; fixed.len()];
        let mut positional_index = 0;

        for arg in args {
            match arg {
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.check_named_signature_arg(
                        name,
                        arg_name,
                        value,
                        &fixed,
                        rest,
                        &mut provided_fixed,
                    );
                }
                CallArg::Spread { value } => match fixed_literal_spread_slots(value) {
                    Some(slots) if rest.is_none() => {
                        for slot_value in slots {
                            self.check_positional_signature_arg_slot(
                                name,
                                slot_value,
                                &fixed,
                                &mut provided_fixed,
                                &mut positional_index,
                            );
                        }
                    }
                    Some(_) | None => {
                        self.check_signature_spread_arg(name, value, rest, &fixed, &provided_fixed);
                    }
                },
                CallArg::Positional(positional) => {
                    if positional_index < fixed.len() {
                        self.check_positional_signature_arg_slot(
                            name,
                            SignatureArgSlot::Expr(positional),
                            &fixed,
                            &mut provided_fixed,
                            &mut positional_index,
                        );
                    } else if let Some(param) = rest {
                        let label = param.name.as_deref().unwrap_or("#rest");
                        let actual =
                            self.expect_signature_arg_type(name, label, positional, &param.ty);
                        self.record_pending_higher_order_signature_arg_effect_call(
                            name,
                            param,
                            positional,
                            actual.as_ref(),
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
            if !provided_fixed[index] && !param.has_default {
                let label = param
                    .name
                    .as_deref()
                    .map_or_else(|| format!("#{index}"), ToOwned::to_owned);
                self.errors.push(TypeCheckError::new(format!(
                    "function `{name}` missing required argument `{label}`"
                )));
            }
        }
    }

    fn check_positional_signature_arg_slot(
        &mut self,
        function_name: &str,
        value: SignatureArgSlot<'_>,
        fixed: &[&FunctionParam],
        provided_fixed: &mut [bool],
        positional_index: &mut usize,
    ) {
        while *positional_index < fixed.len() && provided_fixed[*positional_index] {
            *positional_index += 1;
        }
        let Some(param) = fixed.get(*positional_index) else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` received too many positional arguments"
            )));
            self.check_signature_arg_slot(value, None);
            return;
        };
        provided_fixed[*positional_index] = true;
        let label = signature_param_label(param, *positional_index);
        *positional_index += 1;
        let actual = self.expect_signature_arg_slot_type(function_name, &label, value, &param.ty);
        if let Some(expr) = value.source_expr() {
            self.record_pending_higher_order_signature_arg_effect_call(
                function_name,
                param,
                expr,
                actual.as_ref(),
            );
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

        for (index, (value, param)) in provided.iter().zip(params).enumerate() {
            let Some(value) = value else {
                continue;
            };
            let label = signature_param_label(param, index);
            let _ = self.expect_signature_arg_slot_type(name, &label, *value, param.ty());
        }

        let result_ty = TypeKind::Function {
            params: missing
                .iter()
                .map(|index| params[*index].ty().clone())
                .collect(),
            return_type: Box::new(signature.return_type().clone()),
        };
        self.record_typed_lowering_evidence(TypedLoweringEvidence {
            expression_id,
            kind: TypedLoweringEvidenceKind::SignaturePartialCall {
                callee: name.to_owned(),
                result_ty: result_ty.clone(),
                arg_count: args.len(),
            },
        });
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
        provided_fixed: &mut [bool],
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
        if provided_fixed[index] {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` argument `{arg_name}` was provided more than once"
            )));
        }
        provided_fixed[index] = true;
        let actual =
            self.expect_signature_arg_type(function_name, arg_name, value, &fixed[index].ty);
        self.record_pending_higher_order_signature_arg_effect_call(
            function_name,
            fixed[index],
            value,
            actual.as_ref(),
        );
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
            SignatureArgSlot::Expr(arg),
            expected,
        )
    }

    fn expect_signature_arg_slot_type(
        &mut self,
        function_name: &str,
        arg_label: &str,
        arg: SignatureArgSlot<'_>,
        expected: &TypeKind,
    ) -> Option<TypeKind> {
        let actual = self.check_signature_arg_slot(arg, Some(expected));
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

    fn check_signature_arg_slot(
        &mut self,
        arg: SignatureArgSlot<'_>,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        match arg {
            SignatureArgSlot::Expr(expr) => self.check_expr_with_expected(expr, expected),
            SignatureArgSlot::Int { value, suffix } => {
                let raw =
                    suffix.map_or_else(|| value.to_string(), |suffix| format!("{value}{suffix}"));
                let expr = Expr::Literal(Literal::Int {
                    raw,
                    value,
                    suffix: suffix.map(str::to_owned),
                });
                self.check_expr_with_expected(&expr, expected)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum SignatureArgSlot<'a> {
    Expr(&'a Expr),
    Int { value: i64, suffix: Option<&'a str> },
}

impl<'a> SignatureArgSlot<'a> {
    const fn source_expr(self) -> Option<&'a Expr> {
        match self {
            Self::Expr(expr) => Some(expr),
            Self::Int { .. } => None,
        }
    }
}

fn fixed_literal_spread_slots(value: &Expr) -> Option<Vec<SignatureArgSlot<'_>>> {
    match value {
        Expr::BracketSeq(items) => Some(items.iter().map(SignatureArgSlot::Expr).collect()),
        Expr::NumericBracketSeq(seq) => Some(
            seq.values()
                .iter()
                .map(|value| SignatureArgSlot::Int {
                    value: *value,
                    suffix: seq.suffix(),
                })
                .collect(),
        ),
        _ => None,
    }
}

fn fixed_literal_spread_slot_count(value: &Expr) -> Option<usize> {
    match value {
        Expr::BracketSeq(items) => Some(items.len()),
        Expr::NumericBracketSeq(seq) => Some(seq.len()),
        _ => None,
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
) -> Option<Vec<Option<SignatureArgSlot<'a>>>> {
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
                *slot = Some(SignatureArgSlot::Expr(value));
                positional_index += 1;
            }
            CallArg::Named { name, value } => {
                let index = params
                    .iter()
                    .position(|param| param.name() == Some(name.as_str()))?;
                if provided[index].is_some() {
                    return None;
                }
                provided[index] = Some(SignatureArgSlot::Expr(value));
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

fn call_arg_spread_value(arg: &CallArg) -> Option<&Expr> {
    match arg {
        CallArg::Spread { value } => Some(value),
        CallArg::Positional(_) | CallArg::Named { .. } => None,
    }
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
