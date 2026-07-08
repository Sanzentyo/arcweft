use super::support::{
    call_arg_spread_value, fixed_literal_spread_slot_count, fixed_literal_spread_slots,
};
use super::{
    CallArg, Expr, FunctionSignature, TypeCheckError, TypeChecker, TypeExpressionId, TypeKind,
    TypedLoweringEvidence, TypedLoweringEvidenceKind,
};
use crate::checker::DataLastMethodFallbackArg;
use crate::diagnostics::TypeCheckWarning;
use crate::env::FunctionParam;

impl TypeChecker<'_> {
    pub(super) fn check_data_last_method_fallback(
        &mut self,
        receiver_type: &TypeKind,
        method_name: &str,
        args: &[CallArg],
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        let candidates =
            self.data_last_method_fallback_candidates(receiver_type, method_name, args);
        if candidates.len() > 1 {
            for arg in args {
                self.check_expr(arg.value());
            }
            self.errors
                .push(TypeCheckError::ambiguous_data_last_method_fallback(
                    method_name,
                    receiver_type.clone(),
                    candidates.iter().map(|candidate| candidate.label.clone()),
                ));
            return Some(TypeKind::Named("_".to_owned()));
        }
        let signature = candidates.into_iter().next()?.signature;
        if !signature.checks_args() {
            return None;
        }
        if let Some(reason) = unsupported_fallback_arg_reason(args)
            && self.is_data_last_fallback_candidate(receiver_type, &signature)
        {
            for arg in args {
                self.check_expr(arg.value());
            }
            self.errors
                .push(TypeCheckError::unsupported_data_last_method_fallback(
                    method_name,
                    reason,
                ));
            return Some(TypeKind::Named("_".to_owned()));
        }
        let params = signature.params();
        let supplied_arg_count = data_last_fallback_supplied_arg_count(args);
        if params.len() != supplied_arg_count + 1 {
            return None;
        }
        let (call_params, receiver_params) = params.split_at(supplied_arg_count);
        let receiver_param = &receiver_params[0];
        if !self.types_compatible(receiver_param.ty(), receiver_type) {
            self.errors.push(TypeCheckError::argument_type_mismatch(
                method_name,
                receiver_param.name().unwrap_or("#receiver"),
                receiver_param.ty().clone(),
                receiver_type.clone(),
            ));
            return Some(TypeKind::Named("_".to_owned()));
        }
        let arg_order = self.check_data_last_fallback_args(method_name, args, call_params);
        self.check_function_effects(method_name);
        self.record_typed_lowering_evidence(TypedLoweringEvidence {
            expression_id,
            kind: TypedLoweringEvidenceKind::DataLastMethodFallback {
                method: method_name.to_owned(),
                arg_count: args.len(),
                arg_order,
            },
        });
        Some(signature.return_type().clone())
    }

    pub(super) fn warn_if_data_last_method_fallback_shadowed(
        &mut self,
        receiver_type: &TypeKind,
        method_name: &str,
        args: &[CallArg],
        selected_source: &str,
        selected_signature: &FunctionSignature,
    ) {
        if unsupported_fallback_arg_reason(args).is_some() {
            return;
        }
        let candidates =
            self.data_last_method_fallback_candidates(receiver_type, method_name, args);
        if candidates.is_empty() {
            return;
        }
        self.warnings
            .push(TypeCheckWarning::shadowed_data_last_method_fallback(
                method_name,
                receiver_type.clone(),
                selected_method_label(
                    selected_source,
                    receiver_type,
                    method_name,
                    selected_signature,
                ),
                candidates.iter().map(|candidate| candidate.label.clone()),
            ));
    }

    fn data_last_method_fallback_candidates(
        &mut self,
        receiver_type: &TypeKind,
        method_name: &str,
        args: &[CallArg],
    ) -> Vec<DataLastMethodFallbackCandidate> {
        let mut candidates = Vec::new();
        if let Some(signature) = self.global_function_signatures.get(method_name).cloned()
            && self.is_data_last_fallback_shape(receiver_type, &signature, args)
        {
            candidates.push(DataLastMethodFallbackCandidate::new(
                "module",
                method_name,
                signature,
            ));
        }
        if let Some(signature) = self.env.function_signature(method_name).cloned()
            && self.is_data_last_fallback_shape(receiver_type, &signature, args)
            && !candidates
                .iter()
                .any(|candidate| candidate.signature == signature)
        {
            candidates.push(DataLastMethodFallbackCandidate::new(
                "environment",
                method_name,
                signature,
            ));
        }
        candidates
    }

    fn is_data_last_fallback_candidate(
        &mut self,
        receiver_type: &TypeKind,
        signature: &FunctionSignature,
    ) -> bool {
        let Some(param) = signature.params().last() else {
            return false;
        };
        self.types_compatible(param.ty(), receiver_type)
    }

    fn is_data_last_fallback_shape(
        &mut self,
        receiver_type: &TypeKind,
        signature: &FunctionSignature,
        args: &[CallArg],
    ) -> bool {
        signature.checks_args()
            && signature.params().len() == data_last_fallback_supplied_arg_count(args) + 1
            && self.is_data_last_fallback_candidate(receiver_type, signature)
    }

    fn check_data_last_fallback_args(
        &mut self,
        method_name: &str,
        args: &[CallArg],
        call_params: &[FunctionParam],
    ) -> Vec<DataLastMethodFallbackArg> {
        let mut provided = vec![None; call_params.len()];
        let mut positional_index = 0usize;

        for (arg_index, arg) in args.iter().enumerate() {
            match arg {
                CallArg::Positional(value) => {
                    self.check_data_last_positional_fallback_arg(
                        method_name,
                        arg_index,
                        value,
                        call_params,
                        &mut provided,
                        &mut positional_index,
                    );
                }
                CallArg::Named { name, value } => {
                    self.check_data_last_named_fallback_arg(
                        method_name,
                        arg_index,
                        name,
                        value,
                        call_params,
                        &mut provided,
                    );
                }
                CallArg::Spread { value } => {
                    self.check_data_last_spread_fallback_arg(
                        method_name,
                        arg_index,
                        value,
                        call_params,
                        &mut provided,
                        &mut positional_index,
                    );
                }
            }
        }

        for (index, param) in call_params.iter().enumerate() {
            if provided[index].is_none() && !param.has_default() {
                let label = param
                    .name()
                    .map_or_else(|| format!("#{index}"), ToOwned::to_owned);
                self.errors.push(TypeCheckError::new(format!(
                    "function `{method_name}` missing required argument `{label}`"
                )));
            }
        }

        provided
            .into_iter()
            .filter_map(|provided| match provided {
                Some(ProvidedDataLastArg::Source { arg_index }) => {
                    Some(DataLastMethodFallbackArg::CallArg { index: arg_index })
                }
                Some(ProvidedDataLastArg::SpreadTail) | None => None,
            })
            .chain(std::iter::once(DataLastMethodFallbackArg::Receiver))
            .collect()
    }

    fn check_data_last_positional_fallback_arg(
        &mut self,
        method_name: &str,
        arg_index: usize,
        value: &Expr,
        call_params: &[FunctionParam],
        provided: &mut [Option<ProvidedDataLastArg>],
        positional_index: &mut usize,
    ) {
        while *positional_index < provided.len() && provided[*positional_index].is_some() {
            *positional_index += 1;
        }
        let Some(param) = call_params.get(*positional_index) else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{method_name}` received too many positional arguments"
            )));
            self.check_expr(value);
            return;
        };
        provided[*positional_index] = Some(ProvidedDataLastArg::Source { arg_index });
        let label = param
            .name()
            .map_or_else(|| format!("#{}", *positional_index), ToOwned::to_owned);
        *positional_index += 1;
        let actual = self.expect_signature_arg_type(method_name, &label, value, param.ty());
        self.record_pending_higher_order_signature_arg_effect_call(
            method_name,
            param,
            value,
            actual.as_ref(),
        );
    }

    fn check_data_last_named_fallback_arg(
        &mut self,
        method_name: &str,
        arg_index: usize,
        name: &str,
        value: &Expr,
        call_params: &[FunctionParam],
        provided: &mut [Option<ProvidedDataLastArg>],
    ) {
        let Some(index) = call_params
            .iter()
            .position(|param| !param.is_rest() && param.name() == Some(name))
        else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{method_name}` has no parameter named `{name}`"
            )));
            self.check_expr(value);
            return;
        };
        if provided[index].is_some() {
            self.errors.push(TypeCheckError::new(format!(
                "function `{method_name}` argument `{name}` was provided more than once"
            )));
        }
        provided[index] = Some(ProvidedDataLastArg::Source { arg_index });
        let actual =
            self.expect_signature_arg_type(method_name, name, value, call_params[index].ty());
        self.record_pending_higher_order_signature_arg_effect_call(
            method_name,
            &call_params[index],
            value,
            actual.as_ref(),
        );
    }

    fn check_data_last_spread_fallback_arg(
        &mut self,
        method_name: &str,
        arg_index: usize,
        value: &Expr,
        call_params: &[FunctionParam],
        provided: &mut [Option<ProvidedDataLastArg>],
        positional_index: &mut usize,
    ) {
        let Some(slots) = fixed_literal_spread_slots(value) else {
            self.check_expr(value);
            return;
        };
        self.reserve_fixed_literal_spread_container_expr(value);
        let mut first_slot = true;
        for slot_value in slots {
            while *positional_index < provided.len() && provided[*positional_index].is_some() {
                *positional_index += 1;
            }
            let Some(param) = call_params.get(*positional_index) else {
                self.errors.push(TypeCheckError::new(format!(
                    "function `{method_name}` received too many spread arguments"
                )));
                self.check_fixed_literal_spread_slot(slot_value, None);
                continue;
            };
            provided[*positional_index] = Some(if first_slot {
                first_slot = false;
                ProvidedDataLastArg::Source { arg_index }
            } else {
                ProvidedDataLastArg::SpreadTail
            });
            let label = param
                .name()
                .map_or_else(|| format!("#{}", *positional_index), ToOwned::to_owned);
            *positional_index += 1;
            let actual = self.check_fixed_literal_spread_slot(slot_value, Some(param.ty()));
            if let Some(actual) = actual.as_ref()
                && !self.types_compatible(param.ty(), actual)
            {
                self.errors.push(TypeCheckError::argument_type_mismatch(
                    method_name,
                    &label,
                    param.ty().clone(),
                    actual.clone(),
                ));
            }
            if let Some(expr) = slot_value.source_expr() {
                self.record_pending_higher_order_signature_arg_effect_call(
                    method_name,
                    param,
                    expr,
                    actual.as_ref(),
                );
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ProvidedDataLastArg {
    Source { arg_index: usize },
    SpreadTail,
}

#[derive(Clone, Debug)]
struct DataLastMethodFallbackCandidate {
    signature: FunctionSignature,
    label: String,
}

impl DataLastMethodFallbackCandidate {
    fn new(source: &str, method_name: &str, signature: FunctionSignature) -> Self {
        let label = format!(
            "{source} fn `{method_name}` {}",
            function_signature_label(&signature)
        );
        Self { signature, label }
    }
}

fn unsupported_fallback_arg_reason(args: &[CallArg]) -> Option<&'static str> {
    if !args.iter().any(CallArg::is_spread) {
        return None;
    }
    if fixed_literal_fallback_spread_args(args) {
        return None;
    }
    if args.iter().filter(|arg| arg.is_spread()).count() > 1 {
        return Some(
            "multiple spread arguments are not supported in data-last fallback; runtime expansion ranges are not specified",
        );
    }
    if spread_is_followed_by_fixed_fallback_arg(args) {
        return Some(
            "spread arguments cannot be followed by fixed data-last fallback arguments; runtime argument order is not specified",
        );
    }
    Some("spread arguments are not supported; use positional arguments")
}

fn data_last_fallback_supplied_arg_count(args: &[CallArg]) -> usize {
    args.iter()
        .map(|arg| {
            call_arg_spread_value(arg)
                .and_then(fixed_literal_spread_slot_count)
                .unwrap_or(1)
        })
        .sum()
}

fn fixed_literal_fallback_spread_args(args: &[CallArg]) -> bool {
    args.iter()
        .filter_map(call_arg_spread_value)
        .all(|value| fixed_literal_spread_slot_count(value).is_some())
}

fn spread_is_followed_by_fixed_fallback_arg(args: &[CallArg]) -> bool {
    let Some(spread_index) = args.iter().position(CallArg::is_spread) else {
        return false;
    };
    args.iter()
        .skip(spread_index + 1)
        .any(|arg| !arg.is_spread())
}

fn function_signature_label(signature: &FunctionSignature) -> String {
    let params = signature
        .params()
        .iter()
        .map(|param| {
            param.name().map_or_else(
                || param.ty().source_label(),
                |name| format!("{name}: {}", param.ty().source_label()),
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("fn({params}) -> {}", signature.return_type().source_label())
}

fn selected_method_label(
    source: &str,
    receiver_type: &TypeKind,
    method_name: &str,
    signature: &FunctionSignature,
) -> String {
    format!(
        "{source} method `{}.{method_name}` {}",
        receiver_type.source_label(),
        function_signature_label(signature)
    )
}
