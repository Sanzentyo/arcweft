use super::support::spread_item_type;
use super::{CallArg, Expr, TypeCheckError, TypeChecker, TypeKind};
use crate::checker::helpers::{type_kind_label, type_ref_kind};
use arcweft_lang_syntax::expr::ClosureParam;

impl TypeChecker<'_> {
    pub(super) fn check_closure_expr(
        &mut self,
        params: &[ClosureParam],
        body: &Expr,
    ) -> Option<TypeKind> {
        let mut bindings = Vec::new();
        for param in params {
            let Some(name) = param.simple_ident() else {
                self.errors.push(TypeCheckError::new(
                    "closure parameter pattern must currently bind a simple identifier".to_owned(),
                ));
                continue;
            };
            let ty = param.ty().map_or(TypeKind::I64, type_ref_kind);
            bindings.push((name.to_owned(), ty));
        }
        let local_snapshot = self.insert_scoped_locals(bindings);
        self.check_expr(body);
        self.restore_scoped_locals(local_snapshot);
        None
    }

    pub(super) fn check_vec_map_method_call(
        &mut self,
        receiver_type: &TypeKind,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        let Some(item) = spread_item_type(receiver_type) else {
            self.errors.push(TypeCheckError::new(format!(
                "map receiver must be an iterable sequence, found {receiver_type:?}"
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
                "map requires one positional closure argument".to_owned(),
            ));
            self.check_expr(arg.value());
            return None;
        }
        let Expr::Closure { params, body } = arg.value() else {
            self.errors.push(TypeCheckError::new(
                "map requires a closure argument".to_owned(),
            ));
            self.check_expr(arg.value());
            return None;
        };
        let [param] = params.as_slice() else {
            self.errors.push(TypeCheckError::new(
                "map closures must bind exactly one parameter".to_owned(),
            ));
            return None;
        };
        let Some(param_name) = param.simple_ident() else {
            self.errors.push(TypeCheckError::new(
                "map closure parameter must bind a simple identifier".to_owned(),
            ));
            return None;
        };
        let param_type = param.ty().map_or_else(|| item.clone(), type_ref_kind);
        if !self.types_compatible(&param_type, item) {
            self.errors.push(TypeCheckError::new(format!(
                "map closure parameter `{param_name}` expects {}, but receiver items are {}",
                type_kind_label(&param_type),
                type_kind_label(item)
            )));
        }
        let snapshot = self.insert_scoped_locals([(param_name.to_owned(), param_type)]);
        let body_type = self.check_expr(body);
        self.restore_scoped_locals(snapshot);
        body_type.map(|ty| TypeKind::Vec(Box::new(ty)))
    }
}
