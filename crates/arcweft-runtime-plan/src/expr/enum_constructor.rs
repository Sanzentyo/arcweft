use super::{
    RuntimePureHelperLookup, constructor_path, lower_runtime_expr_strict_with_helpers,
    lower_runtime_record_expr_strict,
};
use arcweft_core::value::RuntimeExpr;
use arcweft_lang_hir::syntax::{
    expr::{CallArg, Expr},
    types::TypeRef,
};

pub(super) fn lower_constructor_call(
    callee: &Expr,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Option<RuntimeExpr> {
    let callee = constructor_callee_label(callee)?;
    let (path, name) = constructor_path(&callee)?;
    if args.len() > 1 {
        return None;
    }
    let payload = args
        .first()
        .and_then(|arg| match arg {
            CallArg::Positional(value) => Some(value),
            CallArg::Named { .. } | CallArg::Spread { .. } => None,
        })
        .map(|payload| lower_runtime_expr_strict_with_helpers(payload, helpers))
        .transpose()
        .ok()?
        .map(Box::new);
    Some(RuntimeExpr::Variant {
        path,
        name,
        payload,
    })
}

pub(super) fn lower_expected_enum_record_constructor(
    expr: &Expr,
    expected_ty: Option<&TypeRef>,
    helpers: RuntimePureHelperLookup<'_, '_, '_>,
) -> Option<Result<RuntimeExpr, String>> {
    let Expr::Record { path, fields } = expr else {
        return None;
    };
    let (path, name) = expected_enum_record_constructor_path(path, expected_ty?)?;
    helpers.next_expression_id();
    Some(
        lower_runtime_record_expr_strict(fields, Some(helpers)).map(|payload| {
            RuntimeExpr::Variant {
                path,
                name,
                payload: Some(Box::new(payload)),
            }
        }),
    )
}

fn constructor_callee_label(callee: &Expr) -> Option<String> {
    match callee {
        Expr::ShortVariant(name) => Some(format!(".{name}")),
        _ => callee.dotted_selector_label(),
    }
}

fn expected_enum_record_constructor_path(
    constructor: &str,
    expected_ty: &TypeRef,
) -> Option<(Option<String>, String)> {
    let expected = expected_type_path(expected_ty)?;
    let (path, name) = constructor_path(constructor)?;
    if path
        .as_deref()
        .is_some_and(|path| !same_type_path(path, expected))
    {
        return None;
    }
    if path.is_none() && same_type_path(&name, expected) {
        return None;
    }
    Some((path, name))
}

fn expected_type_path(ty: &TypeRef) -> Option<&str> {
    match ty {
        TypeRef::Path(path) => Some(path.as_str()),
        TypeRef::Reference(reference) => expected_type_path(reference.referent()),
        _ => None,
    }
}

fn same_type_path(lhs: &str, rhs: &str) -> bool {
    lhs == rhs || type_path_tail(lhs) == type_path_tail(rhs)
}

fn type_path_tail(path: &str) -> &str {
    path.rsplit_once("::").map_or(path, |(_, name)| name)
}
