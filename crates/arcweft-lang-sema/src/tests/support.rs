pub(super) use crate::check::{typecheck_hir, validate_typecheck_ready};
pub(super) use crate::env::TypeCheckEnv;
pub(super) use crate::resolve::{NameRegistry, registry_from_hir, validate_hir_references};
pub(super) use crate::semantic::{
    SemanticDischarge, SemanticMode, SemanticObligationKind, SemanticPolicy, SemanticReport,
    analyze_semantics,
};
pub(super) use crate::symbols::{SymbolUseKind, collect_symbol_uses};
pub(super) use crate::types::{EntityKind, MapKind, TypeKind};
pub(super) use arcweft_core::{
    effect::{
        LineEffectRequest, RuntimeAssertionProfile, RuntimeAssignment, RuntimeCall, RuntimeLog,
    },
    line_task::{LineChildTask, LineOutRequest, LineTaskNode, LineTaskTrigger},
    plan::{FlowOp, FlowRuntimeId},
    source::{SourceHandlerPlan, SourceOp},
    stream::StreamOp,
    time::LogicalDuration,
};
pub(super) use arcweft_lang_hir::lower::lower_to_hir;
pub(super) use arcweft_lang_hir::model::{HirFlowItem, HirTopLevelDecl};
pub(super) use arcweft_lang_syntax::{
    ast::{
        choice::{ChoiceAction, ChoiceItem, ChoicePlanItem},
        common::Visibility,
        dialogue::DialogueToken,
        flow::{AwaitBranchKind, ContractClause, FlowItem, FlowKind, SelectBranchHead, Stmt},
        items::{
            CallableKind, EntityDeclKind, FunctionKind, ImplMember, Item, RawSyntaxFamily,
            TraitMember,
        },
        line_plan::{BlockStyle, DeferOutcome, LinePlanItem},
        pattern::{Pattern, VariantPatternPayload},
        proof::{ProofClause, TestKind},
        source::{
            SourceBackpressurePolicy, SourceEventPattern, SourceHeader, SourcePrivacyPolicy,
            SourceReplayPolicy,
        },
    },
    expr::{
        BinaryOp, ComputationBlockKind, Expr, LifetimeScopeKind, Literal, Placeholder, UnaryOp,
        parse_expr,
    },
    lint::{SyntaxLintCode, lint_id_policy},
    parser::{parse_source, recovery::ParseError},
    text::parse_dialogue_tokens,
    types::{TypeRef, parse_fn_signature, parse_type_ref},
};
pub(super) use arcweft_runtime_plan::{
    flow::lower_runtime_plan, line_task::lower_line_task_groups,
};

pub(super) fn parse_ok(
    source: impl Into<String>,
) -> arcweft_lang_syntax::ast::items::TypedSyntaxTree {
    let parsed = parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "expected source to parse without errors, got {:?}",
        parsed.errors()
    );
    parsed.into_typed_tree()
}

pub(super) fn parse_errors(source: impl Into<String>) -> Vec<ParseError> {
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
