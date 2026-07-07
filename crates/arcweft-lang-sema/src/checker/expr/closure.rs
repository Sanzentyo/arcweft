use super::support::spread_item_type;
use super::{CallArg, Expr, TypeCheckError, TypeChecker, TypeExpressionId, TypeKind};
use crate::checker::helpers::{type_kind_label, type_ref_kind};
use arcweft_lang_syntax::expr::ClosureParam;
use arcweft_lang_syntax::types::TypeRef;

impl TypeChecker<'_> {
    pub(super) fn check_closure_expr(
        &mut self,
        params: &[ClosureParam],
        declared_return_type: Option<&TypeRef>,
        body: &Expr,
        expected: Option<&TypeKind>,
        expression_id: TypeExpressionId,
    ) -> TypeKind {
        let expected_function = match expected {
            Some(TypeKind::Function {
                params,
                return_type,
            }) => Some((params.as_slice(), return_type.as_ref())),
            _ => None,
        };
        if let Some((expected_params, _)) = expected_function
            && expected_params.len() != params.len()
        {
            self.errors.push(TypeCheckError::new(format!(
                "closure expected {} parameter(s), found {}",
                expected_params.len(),
                params.len()
            )));
        }

        let mut bindings = Vec::new();
        let mut function_params = Vec::new();
        for (index, param) in params.iter().enumerate() {
            let Some(name) = param.simple_ident() else {
                self.errors.push(TypeCheckError::new(
                    "closure parameter pattern must currently bind a simple identifier".to_owned(),
                ));
                continue;
            };
            let expected_param = expected_function.and_then(|(params, _)| params.get(index));
            let ty = param
                .ty()
                .map(type_ref_kind)
                .or_else(|| expected_param.cloned())
                .unwrap_or(TypeKind::I64);
            if let Some(expected_param) = expected_param
                && !self.types_compatible(expected_param, &ty)
            {
                self.errors.push(TypeCheckError::new(format!(
                    "closure parameter `{name}` expects {}, but expected function parameter is {}",
                    type_kind_label(&ty),
                    type_kind_label(expected_param)
                )));
            }
            bindings.push((name.to_owned(), ty.clone()));
            function_params.push(ty);
        }
        self.push_closure_capture_frame(
            expression_id,
            bindings.iter().map(|(name, _)| name.clone()),
        );
        let local_snapshot = self.insert_scoped_locals(bindings);
        let declared_return_type = declared_return_type.map(type_ref_kind);
        if let (Some(expected_return), Some(declared_return_type)) = (
            expected_function.map(|(_, return_type)| return_type),
            declared_return_type.as_ref(),
        ) && !self.types_compatible(expected_return, declared_return_type)
        {
            self.errors.push(TypeCheckError::new(format!(
                "closure return type declares {}, but expected function return is {}",
                type_kind_label(declared_return_type),
                type_kind_label(expected_return)
            )));
        }
        let expected_return = declared_return_type
            .as_ref()
            .or_else(|| expected_function.map(|(_, return_type)| return_type));
        self.expected_returns.push(expected_return.cloned());
        let body_type = self.check_expr_with_expected(body, expected_return);
        self.expected_returns.pop();
        self.restore_scoped_locals(local_snapshot);
        self.pop_closure_capture_frame();
        if let (Some(expected_return), Some(body_type)) = (expected_return, body_type.as_ref())
            && !self.types_compatible(expected_return, body_type)
        {
            self.errors.push(TypeCheckError::new(format!(
                "closure body must return {}, found {}",
                type_kind_label(expected_return),
                type_kind_label(body_type)
            )));
        }
        let return_type = expected_return
            .filter(|return_type| !is_unknown_type(return_type))
            .cloned()
            .or(body_type)
            .unwrap_or(TypeKind::Unit);
        TypeKind::Function {
            params: function_params,
            return_type: Box::new(return_type),
        }
    }

    pub(super) fn check_vec_map_method_call(
        &mut self,
        receiver_type: &TypeKind,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        let Some(item) = spread_item_type(receiver_type) else {
            self.errors.push(TypeCheckError::new(format!(
                "map receiver must be an iterable sequence, found {}",
                type_kind_label(receiver_type)
            )));
            for arg in args {
                self.check_expr(arg.value());
            }
            return None;
        };
        let [arg] = args else {
            self.errors.push(TypeCheckError::new(
                "map requires exactly one closure".to_owned(),
            ));
            for arg in args {
                self.check_expr(arg.value());
            }
            return None;
        };
        if arg.name().is_some() || arg.is_spread() {
            self.errors.push(TypeCheckError::new(
                "map requires one positional function argument".to_owned(),
            ));
            self.check_expr(arg.value());
            return None;
        }
        let expected = TypeKind::Function {
            params: vec![item.clone()],
            return_type: Box::new(TypeKind::Named("_".to_owned())),
        };
        let Some(actual) = self.check_expr_with_expected(arg.value(), Some(&expected)) else {
            self.errors.push(TypeCheckError::new(
                "map requires a closure or `_` placeholder function argument".to_owned(),
            ));
            return None;
        };
        match actual {
            TypeKind::Function {
                params,
                return_type,
            } if params.as_slice() == [item.clone()] => Some(TypeKind::Vec(return_type)),
            TypeKind::Function { params, .. } => {
                self.errors.push(TypeCheckError::new(format!(
                    "map function parameter must be {}, found ({})",
                    type_kind_label(item),
                    params
                        .iter()
                        .map(type_kind_label)
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
                None
            }
            other => {
                self.errors.push(TypeCheckError::new(format!(
                    "map requires a function argument, found {}",
                    type_kind_label(&other)
                )));
                None
            }
        }
    }

    pub(super) fn check_vec_filter_method_call(
        &mut self,
        receiver_type: &TypeKind,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        let Some(item) = spread_item_type(receiver_type) else {
            self.errors.push(TypeCheckError::new(format!(
                "filter receiver must be an iterable sequence, found {}",
                type_kind_label(receiver_type)
            )));
            for arg in args {
                self.check_expr(arg.value());
            }
            return None;
        };
        let [arg] = args else {
            self.errors.push(TypeCheckError::new(
                "filter requires exactly one function argument".to_owned(),
            ));
            for arg in args {
                self.check_expr(arg.value());
            }
            return None;
        };
        if arg.name().is_some() || arg.is_spread() {
            self.errors.push(TypeCheckError::new(
                "filter requires one positional function argument".to_owned(),
            ));
            self.check_expr(arg.value());
            return None;
        }
        let expected = TypeKind::Function {
            params: vec![item.clone()],
            return_type: Box::new(TypeKind::Bool),
        };
        let Some(actual) = self.check_expr_with_expected(arg.value(), Some(&expected)) else {
            self.errors.push(TypeCheckError::new(
                "filter requires a closure or `_` placeholder function argument".to_owned(),
            ));
            return None;
        };
        match actual {
            TypeKind::Function {
                params,
                return_type,
            } if params.as_slice() == [item.clone()] && return_type.as_ref() == &TypeKind::Bool => {
                Some(TypeKind::Vec(Box::new(item.clone())))
            }
            TypeKind::Function {
                params,
                return_type,
            } => {
                self.errors.push(TypeCheckError::new(format!(
                    "filter function must be {} -> bool, found ({}) -> {}",
                    type_kind_label(item),
                    params
                        .iter()
                        .map(type_kind_label)
                        .collect::<Vec<_>>()
                        .join(", "),
                    type_kind_label(return_type.as_ref())
                )));
                None
            }
            other => {
                self.errors.push(TypeCheckError::new(format!(
                    "filter requires a function argument, found {}",
                    type_kind_label(&other)
                )));
                None
            }
        }
    }
}

fn is_unknown_type(ty: &TypeKind) -> bool {
    matches!(ty, TypeKind::Named(name) if name == "_")
}
