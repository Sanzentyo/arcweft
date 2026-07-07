use crate::documents::DocumentSnapshot;
use crate::profiles::LspProfile;
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::check::{
    TypeCheckReport, TypeJudgmentSubject, analyze_types, validate_typecheck_ready,
};
use arcweft_lang_sema::resolve::{registry_from_hir, validate_hir_references};
use arcweft_lang_sema::types::TypeKind;
use arcweft_lang_syntax::ast::choice::ChoicePlanItem;
use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_lang_syntax::ast::flow::{FlowItem, Stmt};
use arcweft_lang_syntax::ast::items::{Item, TypedSyntaxTree};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_verify_lsp::inferred_id_inlay_hints_with_mapper;
use lsp_types::{InlayHint, InlayHintKind, InlayHintLabel};

/// Computes Arcweft inlay hints for one source snapshot.
pub fn hints(profile: &LspProfile, document: &DocumentSnapshot) -> Vec<InlayHint> {
    let mut hints = inferred_id_inlay_hints_with_mapper(document.text(), document.line_index());
    hints.extend(function_type_inlay_hints(profile, document));
    hints.sort_by_key(inlay_sort_key);
    hints
}

fn function_type_inlay_hints(profile: &LspProfile, document: &DocumentSnapshot) -> Vec<InlayHint> {
    let parsed = parse_source(document.text().to_owned());
    if !parsed.errors().is_empty() {
        return Vec::new();
    }
    let tree = parsed.typed_tree();
    let Ok(hir) = lower_to_hir(tree) else {
        return Vec::new();
    };
    let registry = registry_from_hir(&hir);
    if validate_hir_references(&hir, &registry).is_err() || validate_typecheck_ready(&hir).is_err()
    {
        return Vec::new();
    }
    let report = analyze_types(&hir, &profile.typecheck_env());
    if !report.diagnostics.is_empty() {
        return Vec::new();
    }

    let judgments = let_type_judgments(&report);
    let mut judgment_cursor = 0usize;
    function_let_sites(tree, document.text())
        .into_iter()
        .filter_map(|site| {
            let judgment = next_matching_let_judgment(
                &judgments,
                &mut judgment_cursor,
                site.pattern_debug.as_str(),
                site.expr_range,
            )?;
            if !site.emit {
                return None;
            }
            let TypeKind::Function { .. } = judgment.ty else {
                return None;
            };
            Some(inlay_for_site(&site, judgment.ty, document))
        })
        .collect()
}

fn let_type_judgments(report: &TypeCheckReport) -> Vec<LetTypeJudgment<'_>> {
    report
        .judgments
        .iter()
        .filter_map(|judgment| {
            let TypeJudgmentSubject::LetBinding { pattern } = &judgment.subject else {
                return None;
            };
            Some(LetTypeJudgment {
                pattern: pattern.as_str(),
                ty: &judgment.ty,
                source_range: judgment.source_range,
            })
        })
        .collect()
}

fn next_matching_let_judgment<'a>(
    judgments: &'a [LetTypeJudgment<'a>],
    cursor: &mut usize,
    pattern_debug: &str,
    expr_range: TextRange,
) -> Option<LetTypeJudgment<'a>> {
    let index = judgments.get(*cursor..)?.iter().position(|judgment| {
        judgment.pattern == pattern_debug
            && judgment
                .source_range
                .is_none_or(|source_range| source_range == expr_range)
    })?;
    let absolute = cursor.saturating_add(index);
    *cursor = absolute.saturating_add(1);
    judgments.get(absolute).copied()
}

fn inlay_for_site(
    site: &LetTypeInlaySite,
    ty: &TypeKind,
    document: &DocumentSnapshot,
) -> InlayHint {
    InlayHint {
        position: document
            .line_index()
            .position_from_byte_offset(site.position),
        label: InlayHintLabel::String(format!(": {}", ty.source_label())),
        kind: Some(InlayHintKind::TYPE),
        text_edits: None,
        tooltip: None,
        padding_left: None,
        padding_right: None,
        data: None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LetTypeInlaySite {
    pattern_debug: String,
    position: usize,
    expr_range: TextRange,
    emit: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LetTypeJudgment<'a> {
    pattern: &'a str,
    ty: &'a TypeKind,
    source_range: Option<TextRange>,
}

fn function_let_sites(tree: &TypedSyntaxTree, source: &str) -> Vec<LetTypeInlaySite> {
    let mut sites = Vec::new();

    for item in tree.items() {
        if let Item::Agent(agent) = item {
            collect_stmt_sites(agent.body_statements(), source, &mut sites);
        }
    }
    for item in tree.items() {
        if let Item::Flow(flow) = item {
            collect_flow_item_sites(flow.body(), source, &mut sites);
        }
    }
    for item in tree.items() {
        if let Item::Function(function) = item {
            collect_stmt_sites(function.body_statements(), source, &mut sites);
        }
    }
    for item in tree.items() {
        if let Item::FlowItem(item) = item {
            collect_flow_item_sites(std::slice::from_ref(item.as_ref()), source, &mut sites);
        }
    }

    sites
}

fn collect_flow_item_sites(items: &[FlowItem], source: &str, sites: &mut Vec<LetTypeInlaySite>) {
    for item in items {
        match item {
            FlowItem::Stmt(stmt) => collect_stmt_site(stmt, source, sites),
            FlowItem::Choice(choice) => {
                if let Some(plan) = choice.plan() {
                    for item in plan.items() {
                        collect_choice_plan_item_sites(item, source, sites);
                    }
                }
            }
            FlowItem::If(block) => {
                collect_flow_item_sites(block.body(), source, sites);
                collect_flow_item_sites(block.else_body(), source, sites);
            }
            FlowItem::IfLet(block) => {
                collect_flow_item_sites(block.body(), source, sites);
                collect_flow_item_sites(block.else_body(), source, sites);
            }
            FlowItem::Match(block) => {
                for arm in block.arms() {
                    collect_flow_item_sites(arm.body(), source, sites);
                }
            }
            FlowItem::Loop(block) => collect_flow_item_sites(block.body(), source, sites),
            FlowItem::While(block) => collect_flow_item_sites(block.body(), source, sites),
            FlowItem::WhileLet(block) => collect_flow_item_sites(block.body(), source, sites),
            FlowItem::For(block) => collect_flow_item_sites(block.body(), source, sites),
            FlowItem::Select(block) => {
                for branch in block.branches() {
                    collect_flow_item_sites(branch.body(), source, sites);
                }
            }
            FlowItem::BorrowBlock(block) => collect_flow_item_sites(block.body(), source, sites),
            FlowItem::SourceLocale(block) => collect_flow_item_sites(block.body(), source, sites),
            FlowItem::Scope(block) => collect_flow_item_sites(block.body(), source, sites),
            FlowItem::AwaitWith(await_with) => {
                for branch in await_with.branches() {
                    collect_flow_item_sites(branch.body(), source, sites);
                }
            }
            FlowItem::SpeakerLine(_)
            | FlowItem::ContentCall(_)
            | FlowItem::Include(_)
            | FlowItem::Raw(_) => {}
        }
    }
}

fn collect_choice_plan_item_sites(
    item: &ChoicePlanItem,
    source: &str,
    sites: &mut Vec<LetTypeInlaySite>,
) {
    match item {
        ChoicePlanItem::Timeout { body, .. }
        | ChoicePlanItem::Cancel { body, .. }
        | ChoicePlanItem::OnSelect { body, .. } => collect_stmt_sites(body, source, sites),
        ChoicePlanItem::Option { .. } | ChoicePlanItem::Raw(_) => {}
    }
}

fn collect_stmt_sites(statements: &[Stmt], source: &str, sites: &mut Vec<LetTypeInlaySite>) {
    for stmt in statements {
        collect_stmt_site(stmt, source, sites);
    }
}

fn collect_stmt_site(stmt: &Stmt, source: &str, sites: &mut Vec<LetTypeInlaySite>) {
    match stmt {
        Stmt::Let {
            pattern,
            ty,
            expr_range: Some(expr_range),
            ..
        } if pattern.simple_binding_name().is_some() => {
            if let Some(position) = let_pattern_end(source, expr_range.start()) {
                sites.push(LetTypeInlaySite {
                    pattern_debug: format!("{pattern:?}"),
                    position,
                    expr_range: *expr_range,
                    emit: ty.is_none(),
                });
            }
        }
        Stmt::LetElse { else_body, .. } => collect_stmt_sites(else_body, source, sites),
        Stmt::Thread(thread) => collect_flow_item_sites(thread.body(), source, sites),
        Stmt::DeferBlock { statements, .. }
        | Stmt::UnsafeLifetime {
            body: statements, ..
        } => {
            collect_stmt_sites(statements, source, sites);
        }
        Stmt::If {
            body, else_body, ..
        } => {
            collect_stmt_sites(body, source, sites);
            collect_stmt_sites(else_body, source, sites);
        }
        Stmt::On { body, .. }
        | Stmt::Loop { body }
        | Stmt::While { body, .. }
        | Stmt::WhileLet { body, .. }
        | Stmt::For { body, .. } => collect_stmt_sites(body, source, sites),
        Stmt::Match { arms, .. } => {
            for arm in arms {
                collect_stmt_sites(arm.body(), source, sites);
            }
        }
        Stmt::Let { .. }
        | Stmt::Assign { .. }
        | Stmt::LetChoice { .. }
        | Stmt::LetScope { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::LetActionReceive { .. }
        | Stmt::Return(_)
        | Stmt::Close(_)
        | Stmt::Expr(_)
        | Stmt::Select(_)
        | Stmt::Out { .. }
        | Stmt::Goto(_)
        | Stmt::Defer { .. }
        | Stmt::Yield(_)
        | Stmt::Signal { .. }
        | Stmt::LifetimeSet { .. }
        | Stmt::Wait(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Raw(_) => {}
    }
}

fn let_pattern_end(source: &str, expr_start: usize) -> Option<usize> {
    let prefix = source.get(..expr_start)?;
    let eq = prefix.rfind('=')?;
    let let_start = prefix.get(..eq)?.rfind("let ")? + "let ".len();
    let pattern_source = source.get(let_start..eq)?;
    Some(let_start + pattern_source.trim_end().len())
}

fn inlay_sort_key(hint: &InlayHint) -> (u32, u32, String) {
    let label = match &hint.label {
        InlayHintLabel::String(label) => label.clone(),
        InlayHintLabel::LabelParts(parts) => parts
            .iter()
            .map(|part| part.value.as_str())
            .collect::<String>(),
    };
    (hint.position.line, hint.position.character, label)
}
