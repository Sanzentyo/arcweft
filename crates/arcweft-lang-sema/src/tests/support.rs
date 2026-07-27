pub(super) use crate::check::{
    DataLastMethodFallbackArg, TypeJudgmentExpected, TypeJudgmentRule, TypeJudgmentSubject,
    TypedLoweringEvidenceKind, analyze_types, typecheck_hir, validate_typecheck_ready,
};
pub(super) use crate::diagnostics::{TypeCheckErrorKind, TypeCheckWarningKind};
pub(super) use crate::env::{EnumVariantPayload, FunctionParam, FunctionSignature, TypeCheckEnv};
pub(super) use crate::resolve::{
    NameRegistry, registry_from_hir, registry_from_hir_and_env, validate_hir_references,
};
pub(super) use crate::semantic::{
    SemanticDischarge, SemanticMode, SemanticObligationKind, SemanticPolicy, SemanticReport,
    analyze_semantics,
};
pub(super) use crate::symbols::{SymbolUseKind, collect_symbol_uses};
pub(super) use crate::types::{EntityKind, MapKind, TypeKind};
pub(super) use arcweft_lang_hir::lower::lower_document_to_hir;
pub(super) use arcweft_lang_hir::model::{HirFlowItem, HirTopLevelDecl};
pub(super) use arcweft_lang_syntax::{
    ast::{
        choice::{ChoiceAction, ChoiceItem, ChoicePlanItem},
        common::Visibility,
        dialogue::DialogueToken,
        flow::{
            AuthoredExpr, AwaitBranchKind, ContractClause, FlowItem, SelectBranchHead, Stmt,
            WaitTarget,
        },
        items::{EntityDeclKind, ImplMember, Item, TraitMember},
        line_plan::{DeferOutcome, LinePlanItem},
        pattern::{Pattern, VariantPatternPayload},
        proof::{ProofClause, ProofTrust, TestKind},
        source::{
            SourceBackpressurePolicy, SourceEventPattern, SourceHeader, SourcePrivacyPolicy,
            SourceReplayPolicy,
        },
    },
    expr::{
        AwaitPropagation, BinaryOp, CallArg, ComputationBlockKind, Expr, LifetimeScopeKind,
        Literal, Placeholder, UnaryOp, parse_expr,
    },
    lint::{SyntaxLintCode, lint_id_policy},
    parser::{ParseOptions, parse_document_with_source, recovery::ParseError},
    reference::BorrowKind,
    text::{parse_dialogue_text, parse_dialogue_tokens},
    types::{TypeRef, parse_fn_signature, parse_type_ref},
};

pub(super) fn parse_ok(source: impl Into<String>) -> arcweft_lang_syntax::source::ParsedSource {
    let document = std::sync::Arc::new(
        arcweft_source::SourceDocument::try_new(
            arcweft_source::SourceDocumentId::try_new("memory:///sema/parse-ok.arcw")
                .expect("valid test document ID"),
            arcweft_source::SourceName::Generated,
            source.into(),
        )
        .expect("valid test source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());
    assert!(
        parsed.errors().is_empty(),
        "expected source to parse without errors, got {:?}",
        parsed.errors()
    );
    parsed
}

pub(super) fn lower_bound_hir(label: &str, source: &str) -> arcweft_lang_hir::model::HirModule {
    let document = std::sync::Arc::new(
        arcweft_source::SourceDocument::try_new(
            arcweft_source::SourceDocumentId::try_new(format!("memory:///{label}.arcw"))
                .expect("valid test document ID"),
            arcweft_source::SourceName::Generated,
            source,
        )
        .expect("valid test source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());
    assert!(
        parsed.errors().is_empty(),
        "expected {label} source to parse without errors, got {:?}",
        parsed.errors()
    );
    lower_document_to_hir(parsed.document(), parsed.typed_tree())
        .unwrap_or_else(|errors| panic!("{label} document-bound test source fails: {errors:?}"))
}

pub(super) fn parse_errors(source: impl Into<String>) -> Vec<ParseError> {
    parse_recovered(source).errors().to_vec()
}

pub(super) fn parse_recovered(
    source: impl Into<String>,
) -> arcweft_lang_syntax::source::ParsedSource {
    let document = std::sync::Arc::new(
        arcweft_source::SourceDocument::try_new(
            arcweft_source::SourceDocumentId::try_new("memory:///sema/parse-recovered.arcw")
                .expect("valid test document ID"),
            arcweft_source::SourceName::Generated,
            source.into(),
        )
        .expect("valid test source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());
    assert!(
        !parsed.errors().is_empty(),
        "expected source to produce parse errors"
    );
    parsed
}

pub(super) fn typecheck_registered_source(
    profile: &str,
    source: &str,
    environment: TypeCheckEnv,
) -> Result<(), Vec<crate::diagnostics::TypeCheckError>> {
    let (document, project, world) =
        crate::test_support::character_project::root_project_source(profile, source);
    let facts = crate::registration::ProjectRegistrationFacts::try_new(
        world,
        vec![document],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registered type-check fixture facts");
    let registered =
        crate::test_support::character_project::register(&project, &facts, environment, None)
            .expect("registered type-check fixture world");
    crate::checker::analyze_registered_project_types(&project.linked_module(), &registered)
        .into_result()
}

pub(super) fn flow_source(body: &str) -> String {
    let mut source = String::from("flow fixture {\n");
    for line in body.lines() {
        source.push_str("    ");
        source.push_str(line);
        source.push('\n');
    }
    source.push_str("}\n");
    source
}

pub(super) fn parse_flow_body_ok(
    body: impl AsRef<str>,
) -> arcweft_lang_syntax::source::ParsedSource {
    parse_ok(flow_source(body.as_ref()))
}

pub(super) fn flow_body(parsed: &arcweft_lang_syntax::source::ParsedSource) -> &[FlowItem] {
    let [Item::Flow(flow)] = parsed.typed_tree().items() else {
        panic!("expected one flow declaration");
    };
    flow.body()
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
        Expr::Select(select) => {
            expected
                .rsplit_once('.')
                .is_some_and(|(prefix, expected_member)| {
                    expected_member == select.member().as_str()
                        && expr_path_eq(select.target(), prefix)
                })
        }
        _ => false,
    }
}

pub(super) fn selected_call_member(expr: &Expr) -> Option<&str> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let Expr::Select(select) = call.callee() else {
        return None;
    };
    Some(select.member().as_str())
}

pub(super) fn selected_call_args(expr: &Expr) -> Option<&[CallArg]> {
    let Expr::Call(call) = expr else {
        return None;
    };
    matches!(call.callee(), Expr::Select(_)).then_some(call.args())
}

pub(super) fn borrow_capture_env() -> TypeCheckEnv {
    TypeCheckEnv::standard()
        .with_symbol("bg", TypeKind::Named("ImageHandle".to_owned()))
        .with_method(
            TypeKind::Named("ImageHandle".to_owned()),
            "pixels",
            pixel_borrow_ty(),
        )
        .with_function("load_avatar", load_avatar_need_ty())
}

pub(super) fn borrow_capture_read_text_env() -> TypeCheckEnv {
    borrow_capture_env()
        .with_function_signature(
            "adapter.read_text",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("path", TypeKind::String)],
            ),
        )
        .with_function_effects("adapter.read_text", ["fs.read".to_owned()])
}

pub(super) fn read_text_env() -> TypeCheckEnv {
    TypeCheckEnv::standard()
        .with_function_signature(
            "adapter.read_text",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("path", TypeKind::String)],
            ),
        )
        .with_function_effects("adapter.read_text", ["fs.read".to_owned()])
}

pub(super) fn pixel_borrow_ty() -> TypeKind {
    TypeKind::BorrowRef {
        kind: BorrowKind::Shared,
        lifetime: Some(LifetimeScopeKind::Named("asset".to_owned())),
        inner: Box::new(TypeKind::Slice(Box::new(TypeKind::Named(
            "Rgba8".to_owned(),
        )))),
    }
}

pub(super) fn load_avatar_need_ty() -> TypeKind {
    TypeKind::Need {
        ready: Box::new(TypeKind::Unit),
        error: Box::new(TypeKind::Named("AssetError".to_owned())),
    }
}
