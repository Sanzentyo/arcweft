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
        let exact_curried_group_call = curried_signature_call.is_some()
            && curried_group_params
                .as_ref()
                .is_some_and(|group| group.len() == params.len())
            && args.len() == params.len()
            && args.iter().all(|arg| matches!(arg, CallArg::Positional(_)));
        let result_ty = self.check_function_value_call(
            args,
            params,
            return_type,
            curried_signature_call,
            curried_group_params.as_deref(),
            exact_curried_group_call,
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
            self.record_curried_signature_result(
                &curried.function_name,
                curried.remaining_group_index + 1,
                &result_ty,
                has_next_group_metadata,
                exact_curried_group_call,
            );
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
        exact_curried_group_call: bool,
    ) -> TypeKind {
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
            if let (Some(curried), Some(group_params), true) = (
                curried_signature_call,
                curried_group_params,
                exact_curried_group_call,
            ) && let Some(param) = group_params.get(index)
            {
                self.record_pending_higher_order_signature_arg_effect_call(
                    &curried.function_name,
                    param,
                    value,
                    actual.as_ref(),
                );
            } else {
                self.last_checked_closure_effect_callable = None;
            }
        }
        if positional.len() >= params.len() {
            return_type.clone()
        } else {
            TypeKind::Function {
                params: params[positional.len()..].to_vec(),
                return_type: Box::new(return_type.clone()),
            }
        }
    }
}
