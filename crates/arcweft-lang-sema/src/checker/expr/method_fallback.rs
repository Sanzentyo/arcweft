use super::{
    CallArg, FunctionSignature, TypeCheckError, TypeChecker, TypeExpressionId, TypeKind,
    TypedLoweringEvidence, TypedLoweringEvidenceKind,
};

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
        if params.len() != args.len() + 1 {
            return None;
        }
        let receiver_param = &params[args.len()];
        if !self.types_compatible(receiver_param.ty(), receiver_type) {
            self.errors.push(TypeCheckError::argument_type_mismatch(
                method_name,
                receiver_param.name().unwrap_or("#receiver"),
                receiver_param.ty().clone(),
                receiver_type.clone(),
            ));
            return Some(TypeKind::Named("_".to_owned()));
        }
        for (index, (arg, param)) in args.iter().zip(params.iter()).enumerate() {
            let CallArg::Positional(value) = arg else {
                unreachable!("data-last fallback accepts only positional args");
            };
            self.expect_signature_arg_type(
                method_name,
                param
                    .name()
                    .unwrap_or(if index == 0 { "#0" } else { "#arg" }),
                value,
                param.ty(),
            );
        }
        self.check_function_effects(method_name);
        self.record_typed_lowering_evidence(TypedLoweringEvidence {
            expression_id,
            kind: TypedLoweringEvidenceKind::DataLastMethodFallback {
                method: method_name.to_owned(),
                arg_count: args.len(),
            },
        });
        Some(signature.return_type().clone())
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
            && signature.params().len() == args.len() + 1
            && self.is_data_last_fallback_candidate(receiver_type, signature)
    }
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
    let has_named = args.iter().any(|arg| arg.name().is_some());
    let has_spread = args.iter().any(CallArg::is_spread);
    match (has_named, has_spread) {
        (true, true) => {
            Some("named and spread arguments are not supported; use positional arguments")
        }
        (true, false) => Some("named arguments are not supported; use positional arguments"),
        (false, true) => Some("spread arguments are not supported; use positional arguments"),
        (false, false) => None,
    }
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
