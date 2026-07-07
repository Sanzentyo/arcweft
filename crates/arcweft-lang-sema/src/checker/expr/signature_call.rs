use super::builtin::CapabilityFunctionSpec;
use super::support::{signature_param_label, spread_item_type};
use super::{CallArg, Expr, FunctionSignature, TypeCheckError, TypeChecker, TypeKind};
use crate::env::FunctionParam;

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
                CallArg::Spread { value } => {
                    self.check_signature_spread_arg(name, value, rest, &fixed, &provided_fixed);
                }
                CallArg::Positional(positional) => {
                    while positional_index < fixed.len() && provided_fixed[positional_index] {
                        positional_index += 1;
                    }
                    if let Some(param) = fixed.get(positional_index) {
                        provided_fixed[positional_index] = true;
                        let label = signature_param_label(param, positional_index);
                        positional_index += 1;
                        self.expect_signature_arg_type(name, &label, positional, &param.ty);
                    } else if let Some(param) = rest {
                        let label = param.name.as_deref().unwrap_or("#rest");
                        self.expect_signature_arg_type(name, label, positional, &param.ty);
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

    pub(super) fn check_partial_pure_signature_call(
        &mut self,
        name: &str,
        signature: &FunctionSignature,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        if !self.is_global_pure_function(name) || !signature.checks_args() || args.is_empty() {
            return None;
        }
        let params = signature.params();
        if args.len() >= params.len()
            || params
                .iter()
                .any(|param| param.is_rest() || param.has_default())
        {
            return None;
        }
        let positional = args
            .iter()
            .map(|arg| match arg {
                CallArg::Positional(value) => Some(value),
                CallArg::Named { .. } | CallArg::Spread { .. } => None,
            })
            .collect::<Option<Vec<_>>>()?;

        for (index, (value, param)) in positional.iter().zip(params).enumerate() {
            let label = signature_param_label(param, index);
            self.expect_signature_arg_type(name, &label, value, param.ty());
        }

        Some(TypeKind::Function {
            params: params[positional.len()..]
                .iter()
                .map(|param| param.ty().clone())
                .collect(),
            return_type: Box::new(signature.return_type().clone()),
        })
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
        self.expect_signature_arg_type(function_name, arg_name, value, &fixed[index].ty);
    }

    pub(super) fn expect_signature_arg_type(
        &mut self,
        function_name: &str,
        arg_label: &str,
        arg: &Expr,
        expected: &TypeKind,
    ) {
        let actual = self.check_expr_with_expected(arg, Some(expected));
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
    }
}
