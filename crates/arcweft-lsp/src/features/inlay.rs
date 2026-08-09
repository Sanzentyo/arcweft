use crate::documents::DocumentSnapshot;
use crate::profiles::LspProfile;
use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::ExprId,
    module::HirModule,
    pattern::{HirPatternBinding, HirPatternKind},
    source_index::{
        HirExprSourceRole, HirPatternSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite,
    },
    stmt::HirStmtKind,
};
use arcweft_lang_sema::{
    final_analysis::{CheckedTypeSelection, FinalSemanticAnalysis},
    types::TypeKind,
};
use arcweft_lang_syntax::ast::common::TextRange;
use lsp_types::{InlayHint, InlayHintKind, InlayHintLabel};
use std::collections::HashSet;

/// Computes Arcweft inlay hints for one source snapshot.
pub fn hints(profile: &LspProfile, document: &DocumentSnapshot) -> Vec<InlayHint> {
    let mut hints = inferred_let_type_inlay_hints(profile, document);
    hints.sort_by_key(inlay_sort_key);
    hints
}

fn inferred_let_type_inlay_hints(
    profile: &LspProfile,
    document: &DocumentSnapshot,
) -> Vec<InlayHint> {
    let Some(accepted) = profile.accepted_environment() else {
        return Vec::new();
    };
    let Some(executable) = accepted.executable() else {
        return Vec::new();
    };
    let project = accepted.project();
    let Some(module) = project.hir_for_open_document(document.uri(), document.source_document())
    else {
        return Vec::new();
    };
    let module = module.as_ref();
    let analysis = executable.final_analysis();

    let numeric_fallback_ranges = numeric_fallback_ranges(module, analysis);
    let sites = final_hir_let_sites(module);
    let mut hints = sites
        .into_iter()
        .filter_map(|site| {
            let checked = analysis.expression(site.initializer)?;
            let has_numeric_fallback = numeric_fallback_ranges.iter().any(|range| {
                range.start() >= site.expr_range.start() && range.end() <= site.expr_range.end()
            });
            let emit_resolved_type =
                matches!(checked.ty(), TypeKind::Function { .. }) || has_numeric_fallback;
            if !emit_resolved_type {
                return None;
            }
            Some(let_inlay_for_site(&site, checked.ty(), document))
        })
        .collect::<Vec<_>>();
    if profile.arbitrary_expression_type_inlays() {
        hints.extend(expression_type_inlay_hints(module, analysis, document));
    }
    hints
}

fn numeric_fallback_ranges(module: &HirModule, analysis: &FinalSemanticAnalysis) -> Vec<TextRange> {
    analysis
        .expressions()
        .filter(|(_, checked)| {
            checked.type_selection() == CheckedTypeSelection::DefaultNumericFallback
        })
        .map(|(id, _)| id)
        .filter(|id| id.module() == module.module_id())
        .filter_map(|id| expression_range(module, id))
        .collect()
}

fn let_inlay_for_site(
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

fn expression_type_inlay_hints(
    module: &HirModule,
    analysis: &FinalSemanticAnalysis,
    document: &DocumentSnapshot,
) -> Vec<InlayHint> {
    let mut emitted = HashSet::new();
    analysis
        .expressions()
        .filter_map(|(id, checked)| {
            if id.module() != module.module_id() {
                return None;
            }
            let expression = module.resolve_expr(id).ok()?;
            let source_range = expression_range(module, id)?;
            if source_range.start() >= source_range.end()
                || source_range.end() > document.text().len()
            {
                return None;
            }
            let source = document
                .text()
                .get(source_range.start()..source_range.end())?;
            if !should_emit_expression_type_inlay(expression.kind(), checked.ty(), source) {
                return None;
            }
            let label = checked.ty().source_label();
            if !emitted.insert((source_range.end(), label.clone())) {
                return None;
            }
            Some(expression_inlay_for_range(
                source_range,
                label.as_str(),
                document,
            ))
        })
        .collect()
}

fn should_emit_expression_type_inlay(kind: &HirExprKind, ty: &TypeKind, source: &str) -> bool {
    if matches!(ty, TypeKind::Function { .. } | TypeKind::Never) {
        return false;
    }
    if aggregate_literal_inlay_site(kind, ty, source) {
        return false;
    }
    !matches!(
        kind,
        HirExprKind::Unit
            | HirExprKind::Literal(_)
            | HirExprKind::EntityReference(_)
            | HirExprKind::LifetimePath(_)
            | HirExprKind::Path(_)
            | HirExprKind::ShortVariant(_)
            | HirExprKind::Placeholder(_)
            | HirExprKind::Tuple(_)
            | HirExprKind::BracketSequence(_)
            | HirExprKind::NumericBracketSequence(_)
            | HirExprKind::RecordLiteral(_)
            | HirExprKind::Error(_)
    )
}

fn aggregate_literal_inlay_site(kind: &HirExprKind, ty: &TypeKind, source: &str) -> bool {
    matches!(
        ty,
        TypeKind::ProjectNominal(_)
            | TypeKind::AcceptedNominal(_)
            | TypeKind::OpenNominal(_)
            | TypeKind::CharacterNominal(_)
            | TypeKind::Named(_)
    ) && matches!(
        kind,
        HirExprKind::Call(_) | HirExprKind::Record(_) | HirExprKind::RecordLiteral(_)
    ) && source.trim_end().ends_with('}')
        && source.contains('{')
}

fn expression_inlay_for_range(
    source_range: TextRange,
    type_label: &str,
    document: &DocumentSnapshot,
) -> InlayHint {
    InlayHint {
        position: document
            .line_index()
            .position_from_byte_offset(source_range.end()),
        label: InlayHintLabel::String(format!(": {type_label}")),
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
    initializer: ExprId,
    position: usize,
    expr_range: TextRange,
}

fn final_hir_let_sites(module: &HirModule) -> Vec<LetTypeInlaySite> {
    let mut sites = module
        .statements()
        .filter_map(|(_, statement)| {
            let HirStmtKind::Let {
                pattern,
                annotation: None,
                initializer,
                ..
            } = statement.kind()
            else {
                return None;
            };
            let pattern_value = module.resolve_pattern(*pattern).ok()?;
            let (HirPatternKind::Binding(binding) | HirPatternKind::MutableBinding(binding)) =
                pattern_value.kind()
            else {
                return None;
            };
            if !matches!(binding, HirPatternBinding::Bound { .. }) {
                return None;
            }
            let pattern_range = source_range(
                module,
                HirSourceQuery::Pattern {
                    owner: *pattern,
                    role: HirPatternSourceRole::Whole,
                },
            )?;
            let expr_range = expression_range(module, *initializer)?;
            Some(LetTypeInlaySite {
                initializer: *initializer,
                position: pattern_range.end(),
                expr_range,
            })
        })
        .collect::<Vec<_>>();
    sites.sort_by_key(|site| (site.expr_range.start(), site.expr_range.end()));
    sites
}

fn expression_range(module: &HirModule, owner: ExprId) -> Option<TextRange> {
    source_range(
        module,
        HirSourceQuery::Expr {
            owner,
            role: HirExprSourceRole::Whole,
        },
    )
}

fn source_range(module: &HirModule, query: HirSourceQuery) -> Option<TextRange> {
    let lookup = module
        .source_site(module.provenance().source_identity(), query)
        .ok()?;
    let HirSourcePresence::Present(HirSourceSite::Span(span)) = lookup.presence() else {
        return None;
    };
    Some(TextRange::new(span.range().start(), span.range().end()))
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
