pub(super) use crate::{
    EntityKind, NameRegistry, SemanticDischarge, SemanticMode, SemanticObligationKind,
    SemanticPolicy, SemanticReport, SymbolUseKind, TypeCheckEnv, TypeKind, analyze_semantics,
    collect_symbol_uses, registry_from_hir, typecheck_hir, validate_hir_references,
    validate_typecheck_ready,
};
pub(super) use arcweft_core::{
    FlowOp, FlowRuntimeId, LineEffectRequest, LineOutRequest, LineTaskNode, LineTaskTrigger,
    RuntimeAssignment, RuntimeCall, RuntimeLog,
};
pub(super) use arcweft_lang_hir::{HirFlowItem, HirTopLevelDecl, lower_to_hir};
pub(super) use arcweft_lang_syntax::{
    AwaitBranchKind, BinaryOp, BlockStyle, CallableKind, ChoiceAction, ChoiceItem, ChoicePlanItem,
    ComputationBlockKind, ContractClause, DeferOutcome, DialogueToken, EntityDeclKind, Expr,
    FlowItem, FlowKind, FunctionKind, ImplMember, Item, LifetimeScopeKind, LinePlanItem, Literal,
    Pattern, Placeholder, SelectBranchHead, Stmt, SyntaxLintCode, TestKind, TraitMember, TypeRef,
    UnaryOp, VariantPatternPayload, Visibility, lint_id_policy, parse_dialogue_tokens, parse_expr,
    parse_fn_signature, parse_source, parse_type_ref,
};
pub(super) use arcweft_runtime_plan::{lower_line_task_groups, lower_runtime_plan};

pub(super) fn parse_ok(source: impl Into<String>) -> arcweft_lang_syntax::TypedSyntaxTree {
    let parsed = parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "expected source to parse without errors, got {:?}",
        parsed.errors()
    );
    parsed.into_typed_tree()
}

pub(super) fn parse_errors(source: impl Into<String>) -> Vec<arcweft_lang_syntax::ParseError> {
    let parsed = parse_source(source);
    assert!(
        !parsed.errors().is_empty(),
        "expected source to produce parse errors"
    );
    parsed.errors().to_vec()
}

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
