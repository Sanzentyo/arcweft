pub(super) use crate::{
    AwaitBranchKind, BinaryOp, CallableKind, ChoiceAction, ChoiceItem, ChoicePlanItem,
    ComputationBlockKind, ContractClause, DialogueToken, EntityDeclKind, EntityKind, EntityRef,
    Expr, FlowItem, FlowKind, FunctionKind, HirFlowItem, HirTopLevelDecl, ImplMember, Item,
    LinePlanItem, Literal, NameRegistry, Pattern, Placeholder, SelectBranchHead, Stmt,
    SymbolUseKind, TraitMember, TypeCheckEnv, TypeKind, TypeRef, UnaryOp, VariantPatternPayload,
    Visibility, collect_symbol_uses, lower_to_hir, parse_dialogue_tokens, parse_expr,
    parse_fn_signature, parse_source, parse_stub, parse_type_ref, registry_from_hir, typecheck_hir,
    validate_hir_references, validate_typecheck_ready,
};

pub(super) fn variant_tuple_binding(pattern: &Pattern, variant: &str, binding: &str) -> bool {
    matches!(
        pattern,
        Pattern::Variant {
            path: None,
            name,
            payload: Some(VariantPatternPayload::Tuple(items)),
        } if name == variant && matches!(items.as_slice(), [Pattern::Ident(name)] if name == binding)
    )
}

pub(super) fn ident_pattern(pattern: &Pattern, expected: &str) -> bool {
    matches!(pattern, Pattern::Ident(name) if name == expected)
}

pub(super) fn expr_path_eq(expr: &Expr, expected: &str) -> bool {
    match expr {
        Expr::Path(path) => path == expected,
        Expr::Field { target, field } => {
            expected
                .rsplit_once('.')
                .is_some_and(|(prefix, expected_field)| {
                    expected_field == field && expr_path_eq(target, prefix)
                })
        }
        _ => false,
    }
}
