use super::support::FixedLiteralSpreadSlot;
use super::{
    CallArg, EntityKind, Expr, TypeCheckError, TypeChecker, TypeExpressionId, TypeKind,
    TypedLoweringEvidence, TypedLoweringEvidenceKind,
};
use crate::checker::helpers::first_arg_type;
use crate::checker::{CurriedSignatureCallValue, PendingCurriedHigherOrderArg};
use crate::effect_model::CallableId;
use crate::effect_row::EffectRow;
use crate::env::FunctionParam;

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
            effects,
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
            effects,
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
        if let Some(curried) = curried_signature_call {
            let retains_partial_group = fixed_positional_slots
                && supplied_arg_count < params.len()
                && curried_group_params.as_ref().is_some_and(|group| {
                    curried.group_arg_offset + supplied_arg_count < group.len()
                })
                && matches!(result_ty, TypeKind::Function { .. });
            self.finish_curried_function_value_call(CurriedCallCompletion {
                value: curried,
                result_ty: &result_ty,
                supplied_arg_count,
                supplied_higher_order_args,
                completes_group: finishes_curried_group,
                retains_partial_group,
            });
        } else {
            self.record_function_value_effect_call(
                callee,
                effect_callable,
                supplied_arg_count,
                params.len(),
            );
            self.last_checked_curried_signature_call = None;
        }
        let partial = supplied_arg_count < params.len();
        self.record_typed_lowering_evidence(TypedLoweringEvidence {
            expression_id,
            kind: TypedLoweringEvidenceKind::FunctionValueCall {
                callee: callee.map(str::to_owned),
                callee_ty,
                result_ty: result_ty.clone(),
                arg_count: args.len(),
                partial,
            },
        });
        result_ty
    }

    fn finish_curried_function_value_call(&mut self, completion: CurriedCallCompletion<'_>) {
        let CurriedCallCompletion {
            value,
            result_ty,
            supplied_arg_count,
            supplied_higher_order_args,
            completes_group,
            retains_partial_group,
        } = completion;
        let final_group_timing = self.uses_final_group_effect_timing(&value.function_name);
        let has_next_group = self
            .function_signature(&value.function_name)
            .and_then(|signature| signature.remaining_param_group(value.remaining_group_index))
            .is_some();
        let mut pending = value.pending_higher_order_args.clone();
        pending.extend(supplied_higher_order_args);

        if !final_group_timing {
            self.check_function_effects(&value.function_name);
            self.record_pending_curried_higher_order_arg_effect_calls(
                &value.function_name,
                &pending,
            );
            pending.clear();
        } else if completes_group && !has_next_group {
            self.check_function_effects(&value.function_name);
        }

        if completes_group && has_next_group {
            self.last_checked_closure_effect_callable =
                self.source_function_effect_callable(&value.function_name);
            self.record_curried_signature_result(
                &value.function_name,
                value.remaining_group_index + 1,
                result_ty,
                true,
                true,
                pending,
            );
        } else if completes_group {
            if final_group_timing {
                self.record_pending_curried_higher_order_arg_effect_calls(
                    &value.function_name,
                    &pending,
                );
                self.record_function_return_effect_result(&value.function_name, result_ty);
            } else {
                self.last_checked_closure_effect_callable = None;
            }
            self.last_checked_curried_signature_call = None;
        } else if retains_partial_group {
            self.last_checked_closure_effect_callable =
                self.source_function_effect_callable(&value.function_name);
            self.last_checked_curried_signature_call = Some(CurriedSignatureCallValue {
                function_name: value.function_name.clone(),
                remaining_group_index: value.remaining_group_index,
                group_arg_offset: value.group_arg_offset + supplied_arg_count,
                current_group_params: value.current_group_params.clone(),
                pending_higher_order_args: pending,
            });
        } else {
            self.last_checked_curried_signature_call = None;
        }
    }

    fn remaining_curried_group_params(
        &self,
        curried_signature_call: Option<&CurriedSignatureCallValue>,
    ) -> Option<Vec<FunctionParam>> {
        curried_signature_call.and_then(|curried| {
            if let Some(params) = &curried.current_group_params {
                return Some(params.clone());
            }
            let signature = self.function_signature(&curried.function_name)?;
            if curried.remaining_group_index == 0 {
                Some(signature.params().to_vec())
            } else {
                signature
                    .remaining_param_group(curried.remaining_group_index - 1)
                    .map(<[_]>::to_vec)
            }
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
            effects,
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
            TypeKind::function_with_effects(
                params[supplied.len()..].to_vec(),
                return_type.clone(),
                effects.clone(),
            )
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
                CallArg::Positional(value) => supplied.push(FixedLiteralSpreadSlot::Expr(value)),
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
                        self.reserve_fixed_literal_spread_container_expr(value);
                        supplied.extend(items.iter().map(FixedLiteralSpreadSlot::Expr));
                    }
                    Expr::NumericBracketSeq(seq) => {
                        self.reserve_fixed_literal_spread_container_expr(value);
                        supplied.extend(seq.literals().iter().map(FixedLiteralSpreadSlot::Int));
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
        slot: &FixedLiteralSpreadSlot<'_>,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        self.check_fixed_literal_spread_slot(*slot, expected)
    }
}

#[derive(Clone, Copy)]
struct FunctionValueCallInput<'a> {
    callee: Option<&'a str>,
    args: &'a [CallArg],
    params: &'a [TypeKind],
    return_type: &'a TypeKind,
    effects: &'a EffectRow,
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

struct CurriedCallCompletion<'a> {
    value: &'a CurriedSignatureCallValue,
    result_ty: &'a TypeKind,
    supplied_arg_count: usize,
    supplied_higher_order_args: Vec<PendingCurriedHigherOrderArg>,
    completes_group: bool,
    retains_partial_group: bool,
}

struct FunctionValueArgSlots<'a> {
    supplied: Vec<FixedLiteralSpreadSlot<'a>>,
    unsupported_arg_syntax: bool,
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
