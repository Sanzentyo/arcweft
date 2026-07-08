use super::{
    CallArg, EntityKind, Expr, TypeCheckError, TypeChecker, TypeExpressionId, TypeKind,
    TypedLoweringEvidence, TypedLoweringEvidenceKind,
};
use crate::checker::helpers::first_arg_type;
use crate::checker::{CurriedSignatureCallValue, PendingCurriedHigherOrderArg};
use crate::effect_model::CallableId;
use crate::env::FunctionParam;
use arcweft_lang_syntax::expr::Literal;

impl TypeChecker<'_> {
    pub(super) fn check_path_call_expr(
        &mut self,
        name: &str,
        args: &[CallArg],
        expected: Option<&TypeKind>,
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        if matches!(name, "promote" | "promote_unchecked") {
            for arg in args.iter().filter_map(|arg| match arg {
                CallArg::Named { value, .. } => Some(value.as_ref()),
                CallArg::Positional(_) | CallArg::Spread { .. } => None,
            }) {
                self.check_expr(arg);
            }
            return Some(TypeKind::Named("Promoted".to_owned()));
        }
        if name == "assume" {
            return Some(TypeKind::Unit);
        }
        let symbol_ty = self.symbol_type_with_capture(name);
        if symbol_ty.as_ref() == Some(&TypeKind::entity_ref(EntityKind::Character)) {
            for arg in args {
                self.check_expr(arg.value());
            }
            return Some(TypeKind::SpeakerPreset(EntityKind::Character));
        }
        if symbol_ty.as_ref() == Some(&TypeKind::SpeakerPreset(EntityKind::Character)) {
            for arg in args {
                self.check_expr(arg.value());
            }
            return Some(TypeKind::SpeakerPreset(EntityKind::Character));
        }
        if let Some(ty) = self.check_result_constructor_call(name, args, expected) {
            return Some(ty);
        }
        if name == "Some" {
            if let Some(expected @ TypeKind::Option(item)) = expected {
                for arg in args {
                    self.expect_expr_type(arg.value(), item, "Some payload");
                }
                return Some(expected.clone());
            }
            let arg_types = args
                .iter()
                .map(|arg| self.check_expr(arg.value()))
                .collect::<Vec<_>>();
            return Some(TypeKind::Option(Box::new(first_arg_type(&arg_types))));
        }
        if let Some(callee_ty @ TypeKind::Function { .. }) = symbol_ty {
            let curried_signature_call = self.local_curried_signature_calls.get(name).cloned();
            return Some(self.check_known_function_value_call(
                expression_id,
                Some(name),
                None,
                curried_signature_call.as_ref(),
                args,
                callee_ty,
            ));
        }
        self.function_type(name).cloned().or_else(|| {
            self.errors
                .push(TypeCheckError::new(format!("unknown function `{name}`")));
            None
        })
    }

    pub(super) fn check_known_function_value_call(
        &mut self,
        expression_id: TypeExpressionId,
        callee: Option<&str>,
        effect_callable: Option<CallableId>,
        curried_signature_call: Option<&CurriedSignatureCallValue>,
        args: &[CallArg],
        callee_ty: TypeKind,
    ) -> TypeKind {
        let TypeKind::Function {
            params,
            return_type,
        } = &callee_ty
        else {
            unreachable!("function value call evidence must receive a function type");
        };
        let curried_group_params = self.remaining_curried_group_params(curried_signature_call);
        let curried_group_arg_offset = curried_signature_call
            .map(|curried| curried.group_arg_offset)
            .unwrap_or_default();
        let FunctionValueCallCheck {
            result_ty,
            supplied_arg_count,
            supplied_higher_order_args,
            unsupported_arg_syntax,
            arity_mismatch,
        } = self.check_function_value_call(FunctionValueCallInput {
            callee,
            args,
            params,
            return_type,
            curried_signature_call,
            curried_group_params: curried_group_params.as_deref(),
            curried_group_arg_offset,
        });
        if unsupported_arg_syntax || arity_mismatch {
            self.last_checked_closure_effect_callable = None;
            self.last_checked_curried_signature_call = None;
            return TypeKind::Named("_".to_owned());
        }
        let fixed_positional_slots = function_value_args_have_fixed_positional_slots(args);
        let finishes_curried_group = curried_signature_call.is_some()
            && fixed_positional_slots
            && supplied_arg_count == params.len()
            && curried_group_params
                .as_ref()
                .is_some_and(|group| curried_group_arg_offset + params.len() == group.len());
        self.record_function_value_effect_call(
            callee,
            effect_callable,
            supplied_arg_count,
            params.len(),
        );
        if let Some(curried) = curried_signature_call {
            let has_next_group_metadata = self
                .function_signature(&curried.function_name)
                .and_then(|signature| {
                    signature.remaining_param_group(curried.remaining_group_index)
                })
                .is_some();
            let mut pending_higher_order_args = curried.pending_higher_order_args.clone();
            pending_higher_order_args.extend(supplied_higher_order_args);
            if finishes_curried_group {
                self.record_pending_curried_higher_order_arg_effect_calls(
                    &curried.function_name,
                    &pending_higher_order_args,
                );
                self.record_curried_signature_result(
                    &curried.function_name,
                    curried.remaining_group_index + 1,
                    &result_ty,
                    has_next_group_metadata,
                    true,
                );
            } else if fixed_positional_slots
                && supplied_arg_count < params.len()
                && curried_group_params.as_ref().is_some_and(|group| {
                    curried.group_arg_offset + supplied_arg_count < group.len()
                })
                && matches!(result_ty, TypeKind::Function { .. })
            {
                self.last_checked_curried_signature_call = Some(CurriedSignatureCallValue {
                    function_name: curried.function_name.clone(),
                    remaining_group_index: curried.remaining_group_index,
                    group_arg_offset: curried.group_arg_offset + supplied_arg_count,
                    pending_higher_order_args,
                });
            } else {
                self.last_checked_curried_signature_call = None;
            }
        } else {
            self.last_checked_curried_signature_call = None;
        }
        self.record_typed_lowering_evidence(TypedLoweringEvidence {
            expression_id,
            kind: TypedLoweringEvidenceKind::FunctionValueCall {
                callee: callee.map(str::to_owned),
                callee_ty,
                result_ty: result_ty.clone(),
                arg_count: args.len(),
            },
        });
        result_ty
    }

    fn remaining_curried_group_params(
        &self,
        curried_signature_call: Option<&CurriedSignatureCallValue>,
    ) -> Option<Vec<FunctionParam>> {
        curried_signature_call.and_then(|curried| {
            self.function_signature(&curried.function_name)
                .and_then(|signature| {
                    signature.remaining_param_group(curried.remaining_group_index - 1)
                })
                .map(<[_]>::to_vec)
        })
    }

    fn check_function_value_call(
        &mut self,
        input: FunctionValueCallInput<'_>,
    ) -> FunctionValueCallCheck {
        let FunctionValueCallInput {
            callee,
            args,
            params,
            return_type,
            curried_signature_call,
            curried_group_params,
            curried_group_arg_offset,
        } = input;
        let mut supplied_higher_order_args = Vec::new();
        let mut arity_mismatch = false;
        let FunctionValueArgSlots {
            supplied,
            unsupported_arg_syntax,
        } = self.collect_function_value_arg_slots(callee, args);
        if supplied.len() > params.len() {
            arity_mismatch = true;
            self.errors
                .push(TypeCheckError::function_value_arity_mismatch(
                    callee,
                    params.len(),
                    supplied.len(),
                ));
        }
        for (index, (value, expected)) in supplied.iter().zip(params).enumerate() {
            let actual = self.check_function_value_arg_slot(value, Some(expected));
            if let Some(actual) = actual.as_ref()
                && !self.types_compatible(expected, actual)
            {
                self.errors.push(TypeCheckError::argument_type_mismatch(
                    function_value_call_label(callee),
                    format!("#{index}"),
                    expected.clone(),
                    actual.clone(),
                ));
            }
            if curried_signature_call.is_some()
                && let Some(group_params) = curried_group_params
                && let Some(param) = group_params.get(curried_group_arg_offset + index)
                && let Some(value_expr) = value.source_expr()
            {
                supplied_higher_order_args.extend(self.higher_order_signature_arg_effect_calls(
                    param,
                    value_expr,
                    actual.as_ref(),
                ));
            } else {
                self.last_checked_closure_effect_callable = None;
            }
        }
        for value in supplied.iter().skip(params.len()) {
            self.check_function_value_arg_slot(value, None);
        }
        let result_ty = if supplied.len() >= params.len() {
            return_type.clone()
        } else {
            TypeKind::Function {
                params: params[supplied.len()..].to_vec(),
                return_type: Box::new(return_type.clone()),
            }
        };
        FunctionValueCallCheck {
            result_ty,
            supplied_arg_count: supplied.len(),
            supplied_higher_order_args,
            unsupported_arg_syntax,
            arity_mismatch,
        }
    }

    fn collect_function_value_arg_slots<'a>(
        &mut self,
        callee: Option<&str>,
        args: &'a [CallArg],
    ) -> FunctionValueArgSlots<'a> {
        let mut supplied = Vec::new();
        let mut unsupported_arg_syntax = false;
        for arg in args {
            match arg {
                CallArg::Positional(value) => supplied.push(FunctionValueArgSlot::Expr(value)),
                CallArg::Named { name, value } => {
                    unsupported_arg_syntax = true;
                    self.errors
                        .push(TypeCheckError::unsupported_function_value_call(
                            callee,
                            format!(
                                "named argument `{name}` is not supported; use positional arguments"
                            ),
                        ));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => match value.as_ref() {
                    Expr::BracketSeq(items) => {
                        supplied.extend(items.iter().map(FunctionValueArgSlot::Expr));
                    }
                    Expr::NumericBracketSeq(seq) => {
                        supplied.extend(seq.values().iter().map(|value| {
                            FunctionValueArgSlot::Int {
                                value: *value,
                                suffix: seq.suffix(),
                            }
                        }));
                    }
                    _ => {
                        unsupported_arg_syntax = true;
                        self.errors
                            .push(TypeCheckError::unsupported_function_value_call(
                                callee,
                                "spread arguments require an inline fixed-length sequence literal in function-value calls",
                            ));
                        self.check_expr(value);
                    }
                },
            }
        }
        FunctionValueArgSlots {
            supplied,
            unsupported_arg_syntax,
        }
    }

    fn check_function_value_arg_slot(
        &mut self,
        slot: &FunctionValueArgSlot<'_>,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        match slot {
            FunctionValueArgSlot::Expr(expr) => self.check_expr_with_expected(expr, expected),
            FunctionValueArgSlot::Int { value, suffix } => {
                let raw =
                    suffix.map_or_else(|| value.to_string(), |suffix| format!("{value}{suffix}"));
                let expr = Expr::Literal(Literal::Int {
                    raw,
                    value: *value,
                    suffix: suffix.map(str::to_owned),
                });
                self.check_expr_with_expected(&expr, expected)
            }
        }
    }
}

#[derive(Clone, Copy)]
struct FunctionValueCallInput<'a> {
    callee: Option<&'a str>,
    args: &'a [CallArg],
    params: &'a [TypeKind],
    return_type: &'a TypeKind,
    curried_signature_call: Option<&'a CurriedSignatureCallValue>,
    curried_group_params: Option<&'a [FunctionParam]>,
    curried_group_arg_offset: usize,
}

struct FunctionValueCallCheck {
    result_ty: TypeKind,
    supplied_arg_count: usize,
    supplied_higher_order_args: Vec<PendingCurriedHigherOrderArg>,
    unsupported_arg_syntax: bool,
    arity_mismatch: bool,
}

enum FunctionValueArgSlot<'a> {
    Expr(&'a Expr),
    Int { value: i64, suffix: Option<&'a str> },
}

struct FunctionValueArgSlots<'a> {
    supplied: Vec<FunctionValueArgSlot<'a>>,
    unsupported_arg_syntax: bool,
}

impl<'a> FunctionValueArgSlot<'a> {
    fn source_expr(&self) -> Option<&'a Expr> {
        match self {
            Self::Expr(expr) => Some(expr),
            Self::Int { .. } => None,
        }
    }
}

fn function_value_args_have_fixed_positional_slots(args: &[CallArg]) -> bool {
    args.iter().all(|arg| match arg {
        CallArg::Positional(_) => true,
        CallArg::Named { .. } => false,
        CallArg::Spread { value } => {
            matches!(
                value.as_ref(),
                Expr::BracketSeq(_) | Expr::NumericBracketSeq(_)
            )
        }
    })
}

fn function_value_call_label(callee: Option<&str>) -> String {
    callee.map_or_else(
        || "function value".to_owned(),
        |callee| format!("function value `{callee}`"),
    )
}
