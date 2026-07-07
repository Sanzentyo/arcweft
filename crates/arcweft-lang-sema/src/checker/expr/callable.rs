use super::{
    CallArg, EntityKind, TypeCheckError, TypeChecker, TypeExpressionId, TypeKind,
    TypedLoweringEvidence, TypedLoweringEvidenceKind,
};
use crate::checker::CurriedSignatureCallValue;
use crate::checker::helpers::{first_arg_type, type_kind_label};
use crate::effect_model::CallableId;

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
        let positional_arg_count = args
            .iter()
            .filter(|arg| matches!(arg, CallArg::Positional(_)))
            .count();
        let curried_group_params = curried_signature_call.and_then(|curried| {
            self.function_signature(&curried.function_name)
                .and_then(|signature| {
                    signature.remaining_param_group(curried.remaining_group_index - 1)
                })
                .map(<[_]>::to_vec)
        });
        let all_positional_args = args.iter().all(|arg| matches!(arg, CallArg::Positional(_)));
        let curried_group_arg_offset = curried_signature_call
            .map(|curried| curried.group_arg_offset)
            .unwrap_or_default();
        let finishes_curried_group = curried_signature_call.is_some()
            && all_positional_args
            && args.len() == params.len()
            && curried_group_params
                .as_ref()
                .is_some_and(|group| curried_group_arg_offset + params.len() == group.len());
        let (result_ty, supplied_higher_order_args) = self.check_function_value_call(
            args,
            params,
            return_type,
            curried_signature_call,
            curried_group_params.as_deref(),
            curried_group_arg_offset,
        );
        self.record_function_value_effect_call(
            callee,
            effect_callable,
            positional_arg_count,
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
            } else if all_positional_args
                && positional_arg_count < params.len()
                && curried_group_params.as_ref().is_some_and(|group| {
                    curried.group_arg_offset + positional_arg_count < group.len()
                })
                && matches!(result_ty, TypeKind::Function { .. })
            {
                self.last_checked_curried_signature_call = Some(CurriedSignatureCallValue {
                    function_name: curried.function_name.clone(),
                    remaining_group_index: curried.remaining_group_index,
                    group_arg_offset: curried.group_arg_offset + positional_arg_count,
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

    pub(super) fn check_function_value_call(
        &mut self,
        args: &[CallArg],
        params: &[TypeKind],
        return_type: &TypeKind,
        curried_signature_call: Option<&CurriedSignatureCallValue>,
        curried_group_params: Option<&[crate::env::FunctionParam]>,
        curried_group_arg_offset: usize,
    ) -> (TypeKind, Vec<crate::checker::PendingCurriedHigherOrderArg>) {
        let mut supplied_higher_order_args = Vec::new();
        for arg in args {
            if let CallArg::Named { name, value } = arg {
                self.errors.push(TypeCheckError::new(format!(
                    "function value calls do not accept named argument `{name}`"
                )));
                self.check_expr(value);
            }
            if let CallArg::Spread { value } = arg {
                self.errors.push(TypeCheckError::new(
                    "function value calls do not accept spread arguments".to_owned(),
                ));
                self.check_expr(value);
            }
        }
        let positional = args
            .iter()
            .filter_map(|arg| match arg {
                CallArg::Positional(value) => Some(value),
                CallArg::Named { .. } | CallArg::Spread { .. } => None,
            })
            .collect::<Vec<_>>();
        if positional.len() > params.len() {
            self.errors.push(TypeCheckError::new(format!(
                "function value expected at most {} positional argument(s), got {}",
                params.len(),
                positional.len()
            )));
        }
        for (index, (value, expected)) in positional.iter().zip(params).enumerate() {
            let actual = self.check_expr_with_expected(value, Some(expected));
            if actual
                .as_ref()
                .is_some_and(|actual| !self.types_compatible(expected, actual))
            {
                self.errors.push(TypeCheckError::new(format!(
                    "function value argument #{index} must have type {}, found {}",
                    type_kind_label(expected),
                    type_kind_label(actual.as_ref().expect("checked above"))
                )));
            }
            if curried_signature_call.is_some()
                && let Some(group_params) = curried_group_params
                && let Some(param) = group_params.get(curried_group_arg_offset + index)
            {
                if let Some(arg) =
                    self.higher_order_signature_arg_effect_call(param, value, actual.as_ref())
                {
                    supplied_higher_order_args.push(arg);
                }
            } else {
                self.last_checked_closure_effect_callable = None;
            }
        }
        let result_ty = if positional.len() >= params.len() {
            return_type.clone()
        } else {
            TypeKind::Function {
                params: params[positional.len()..].to_vec(),
                return_type: Box::new(return_type.clone()),
            }
        };
        (result_ty, supplied_higher_order_args)
    }
}
