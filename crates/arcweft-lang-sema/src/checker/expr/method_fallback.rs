use super::{
    CallArg, TypeCheckError, TypeChecker, TypeExpressionId, TypeKind, TypedLoweringEvidence,
    TypedLoweringEvidenceKind,
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
        if args
            .iter()
            .any(|arg| arg.name().is_some() || arg.is_spread())
        {
            return None;
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
}
