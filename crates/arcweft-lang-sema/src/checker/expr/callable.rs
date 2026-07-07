use super::{CallArg, TypeCheckError, TypeChecker, TypeKind};
use crate::checker::helpers::type_kind_label;

impl TypeChecker<'_> {
    pub(super) fn check_function_value_call(
        &mut self,
        args: &[CallArg],
        params: &[TypeKind],
        return_type: &TypeKind,
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
        if positional.len() != params.len() {
            self.errors.push(TypeCheckError::new(format!(
                "function value expected {} positional argument(s), got {}",
                params.len(),
                positional.len()
            )));
        }
        for (index, (value, expected)) in positional.iter().zip(params).enumerate() {
            let actual = self.check_expr(value);
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
        }
        return_type.clone()
    }
}
