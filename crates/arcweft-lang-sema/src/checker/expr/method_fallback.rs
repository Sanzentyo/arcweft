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
        let signature = self.function_signature(method_name)?.clone();
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
