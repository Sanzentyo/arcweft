//! Catalog construction and semantic checks for HIR Style.

use std::collections::{BTreeSet, btree_map::Entry};

use arcweft_lang_hir::{
    model::{HirModule, HirTopLevelDecl},
    style::{HirStyleAssignOp, HirStyleDecl, HirStylePatch, HirStyleSelector},
};
use arcweft_view::{
    ViewElementKind, ViewPartName,
    style::{
        ViewAlignment, ViewElementState, ViewInteractionSelector, ViewOverflow, ViewPropertyKind,
        ViewSpecifiedValue, ViewStyleCombinator, ViewStylePatchId, ViewStylePredicate,
        ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSheetId, ViewStyleTokenId,
    },
};

use super::{
    catalog::{
        CheckedViewStyleCatalog, CheckedViewStyleDeclaration, CheckedViewStylePatch,
        CheckedViewStyleRule, CheckedViewStyleSheet, CheckedViewStyleToken,
    },
    diagnostic::{StyleDiagnostic, StyleDiagnosticCode},
    token_graph::token_dependency_order,
    value::{CheckedTokenKinds, annotation_kind, check_value, infer_value_kind},
};

/// Checks every named sheet and inline patch in one HIR module.
pub fn check_view_styles(module: &HirModule) -> (CheckedViewStyleCatalog, Vec<StyleDiagnostic>) {
    let mut diagnostics = Vec::new();
    let mut sheets = Vec::new();
    for declaration in module.declarations() {
        let HirTopLevelDecl::Style(style) = declaration else {
            continue;
        };
        if let Some(sheet) = check_sheet(style, &mut diagnostics) {
            sheets.push(sheet);
        }
    }

    let mut global_tokens = CheckedTokenKinds::new();
    let mut ambiguous_tokens = BTreeSet::new();
    for sheet in &sheets {
        for token in sheet.tokens() {
            let name = token.id().public_id().as_str().to_owned();
            if ambiguous_tokens.contains(&name) {
                continue;
            }
            match global_tokens.entry(name) {
                Entry::Occupied(entry) => {
                    let name = entry.key().clone();
                    entry.remove();
                    ambiguous_tokens.insert(name);
                }
                Entry::Vacant(entry) => {
                    entry.insert((token.id().clone(), token.value_kind()));
                }
            }
        }
    }
    let inline_patches = module
        .style_patches()
        .iter()
        .map(|patch| check_patch(patch, &global_tokens, &mut diagnostics))
        .collect();
    (
        CheckedViewStyleCatalog::new(sheets, inline_patches),
        diagnostics,
    )
}

fn check_sheet(
    style: &HirStyleDecl,
    diagnostics: &mut Vec<StyleDiagnostic>,
) -> Option<CheckedViewStyleSheet> {
    let id = ViewStyleSheetId::try_new(style.id().body().to_owned()).map_err(|error| {
        diagnostics.push(
            StyleDiagnostic::new(
                StyleDiagnosticCode::ScopeReferenceNotFound,
                format!("invalid style sheet id: {error}"),
                style.range(),
            )
            .with_subject(style.id().body()),
        );
    });
    let Ok(id) = id else {
        return None;
    };
    let sheet = style.sheet();
    let (tokens, token_kinds) = check_tokens(id.public_id().as_str(), sheet.tokens(), diagnostics);
    let rules = sheet
        .rules()
        .iter()
        .enumerate()
        .filter_map(|(source_order, rule)| {
            let selector = check_selector(rule.selector(), diagnostics);
            let target = selector.as_ref().and_then(|selector| {
                selector
                    .sequences()
                    .last()
                    .and_then(ViewStyleSelectorSequence::element)
            });
            let declarations = rule
                .declarations()
                .iter()
                .filter_map(|declaration| {
                    check_declaration(
                        declaration,
                        &token_kinds,
                        target,
                        Some(id.public_id().as_str()),
                        diagnostics,
                    )
                })
                .collect();
            let selector = selector?;
            Some(CheckedViewStyleRule::new(
                selector,
                declarations,
                u32::try_from(source_order).unwrap_or(u32::MAX),
                rule.range(),
            ))
        })
        .collect();
    Some(CheckedViewStyleSheet::new(id, tokens, rules, style.range()))
}

fn check_tokens(
    owner_sheet: &str,
    tokens: &[arcweft_lang_hir::style::HirStyleTokenDecl],
    diagnostics: &mut Vec<StyleDiagnostic>,
) -> (Vec<CheckedViewStyleToken>, CheckedTokenKinds) {
    let graph = token_dependency_order(owner_sheet, tokens);
    diagnostics.extend(graph.diagnostics);
    let mut checked = Vec::new();
    let mut kinds = CheckedTokenKinds::new();
    for index in graph.order {
        let token = &tokens[index];
        let id = match ViewStyleTokenId::try_new(token.public_id().to_owned()) {
            Ok(id) => id,
            Err(error) => {
                diagnostics.push(
                    StyleDiagnostic::new(
                        StyleDiagnosticCode::InvalidValueType,
                        format!("invalid style token id: {error}"),
                        token.range(),
                    )
                    .with_subject(token.public_id()),
                );
                continue;
            }
        };
        let annotated = token.value_type().and_then(annotation_kind);
        if token.value_type().is_some() && annotated.is_none() {
            diagnostics.push(
                StyleDiagnostic::new(
                    StyleDiagnosticCode::InvalidValueType,
                    format!(
                        "unknown native style token type for `{}`",
                        token.public_id()
                    ),
                    token.range(),
                )
                .with_subject(token.public_id()),
            );
            continue;
        }
        let Some(kind) = annotated.or_else(|| infer_value_kind(token.value(), &kinds)) else {
            diagnostics.push(
                StyleDiagnostic::new(
                    StyleDiagnosticCode::InvalidValueType,
                    format!(
                        "style token `{}` needs an explicit closed value type",
                        token.public_id()
                    ),
                    token.value().range(),
                )
                .with_subject(token.public_id()),
            );
            continue;
        };
        match check_value(token.value(), kind, &kinds) {
            Ok(value) => {
                kinds.insert(token.public_id().to_owned(), (id.clone(), kind));
                checked.push(CheckedViewStyleToken::new(id, kind, value, token.range()));
            }
            Err(error) => diagnostics.push(error.with_owner_sheet(owner_sheet)),
        }
    }
    (checked, kinds)
}

fn check_selector(
    selector: &HirStyleSelector,
    diagnostics: &mut Vec<StyleDiagnostic>,
) -> Option<ViewStyleSelector> {
    let sequences = selector
        .sequences()
        .iter()
        .map(|sequence| {
            let element = sequence.element().and_then(|name| {
                ViewElementKind::from_source_name(name.text()).or_else(|| {
                    diagnostics.push(
                        StyleDiagnostic::new(
                            StyleDiagnosticCode::UnknownElement,
                            format!("unknown View style element `{}`", name.text()),
                            name.range(),
                        )
                        .with_subject(name.text())
                        .with_valid_inventory(
                            ViewElementKind::ALL
                                .iter()
                                .map(|element| element.source_name().to_owned())
                                .collect(),
                        ),
                    );
                    None
                })
            });
            let part = sequence.part().and_then(|name| {
                ViewPartName::try_new(name.text().to_owned()).map_or_else(
                    |error| {
                        diagnostics.push(
                            StyleDiagnostic::new(
                                StyleDiagnosticCode::MalformedSelector,
                                format!("invalid View part name: {error}"),
                                name.range(),
                            )
                            .with_subject(name.text()),
                        );
                        None
                    },
                    Some,
                )
            });
            let predicates = sequence
                .predicates()
                .iter()
                .filter_map(|predicate| {
                    ViewInteractionSelector::from_source_name(predicate.text())
                        .map(ViewStylePredicate::Interaction)
                        .or_else(|| {
                            ViewElementState::from_source_name(predicate.text())
                                .map(ViewStylePredicate::ElementState)
                        })
                        .or_else(|| {
                            diagnostics.push(
                                StyleDiagnostic::new(
                                    StyleDiagnosticCode::UnknownState,
                                    format!("unknown View style state `{}`", predicate.text()),
                                    predicate.range(),
                                )
                                .with_subject(predicate.text()),
                            );
                            None
                        })
                })
                .collect();
            let relation = sequence
                .relation_to_previous()
                .map(|relation| match relation {
                    arcweft_lang_hir::style::HirStyleCombinator::Descendant => {
                        ViewStyleCombinator::Descendant
                    }
                    arcweft_lang_hir::style::HirStyleCombinator::Child => {
                        ViewStyleCombinator::Child
                    }
                });
            ViewStyleSelectorSequence::new(relation, element, part, predicates).or_else(|| {
                diagnostics.push(StyleDiagnostic::new(
                    StyleDiagnosticCode::MalformedSelector,
                    "empty View style selector sequence",
                    sequence.range(),
                ));
                None
            })
        })
        .collect::<Option<Vec<_>>>()?;
    ViewStyleSelector::new(sequences).or_else(|| {
        diagnostics.push(StyleDiagnostic::new(
            StyleDiagnosticCode::MalformedSelector,
            "invalid View style selector relation sequence",
            selector.range(),
        ));
        None
    })
}

fn check_declaration(
    declaration: &arcweft_lang_hir::style::HirStyleDeclaration,
    tokens: &CheckedTokenKinds,
    target: Option<ViewElementKind>,
    owner_sheet: Option<&str>,
    diagnostics: &mut Vec<StyleDiagnostic>,
) -> Option<CheckedViewStyleDeclaration> {
    let Some(property) = ViewPropertyKind::from_source_name(declaration.property().text()) else {
        diagnostics.push(
            StyleDiagnostic::new(
                StyleDiagnosticCode::UnknownProperty,
                format!(
                    "unknown native View style property `{}`",
                    declaration.property().text()
                ),
                declaration.property().range(),
            )
            .with_subject(declaration.property().text())
            .with_nearest_names(nearest_property_names(declaration.property().text())),
        );
        return None;
    };
    let append = declaration.op() == HirStyleAssignOp::Append;
    if append && !property.is_appendable() {
        diagnostics.push(
            StyleDiagnostic::new(
                StyleDiagnosticCode::InvalidAppend,
                format!(
                    "property `{}` does not support append",
                    property.source_name()
                ),
                declaration.range(),
            )
            .with_subject(property.source_name()),
        );
        return None;
    }
    if let Err(error) = check_property_applicability(property, target, declaration.range()) {
        diagnostics.push(error);
        return None;
    }
    let value = match check_value(declaration.value(), property.value_kind(), tokens) {
        Ok(value) => value,
        Err(mut error) => {
            if let Some(owner_sheet) = owner_sheet {
                error = error.with_owner_sheet(owner_sheet);
            }
            diagnostics.push(error);
            return None;
        }
    };
    if let Err(error) = check_alignment_applicability(property, &value, declaration.value().range())
    {
        diagnostics.push(error);
        return None;
    }
    if matches!(
        property,
        ViewPropertyKind::TranslateInline | ViewPropertyKind::TranslateBlock
    ) && matches!(
        value,
        ViewSpecifiedValue::Length { value } if !value.is_axis_sign_reversible()
    ) {
        diagnostics.push(
            StyleDiagnostic::new(
                StyleDiagnosticCode::LogicalTranslationNotSignReversible,
                format!(
                    "logical translation `{}` cannot be represented reversibly in every box-axis mode",
                    property.source_name()
                ),
                declaration.value().range(),
            )
            .with_subject(property.source_name()),
        );
        return None;
    }
    if let Err(error) = check_interactive_overflow(property, target, &value, declaration.range()) {
        diagnostics.push(error);
        return None;
    }
    Some(CheckedViewStyleDeclaration::new(
        property,
        value,
        append,
        declaration.range(),
    ))
}

fn check_property_applicability(
    property: ViewPropertyKind,
    target: Option<ViewElementKind>,
    range: arcweft_lang_syntax::ast::common::TextRange,
) -> Result<(), StyleDiagnostic> {
    let Some(element) = target.filter(|element| !property.applies_to(*element)) else {
        return Ok(());
    };
    Err(StyleDiagnostic::new(
        StyleDiagnosticCode::PropertyNotApplicable,
        format!(
            "property `{}` does not apply to `{}`",
            property.source_name(),
            element.source_name()
        ),
        range,
    )
    .with_subject(property.source_name())
    .with_types("applicable View element", element.source_name()))
}

fn check_alignment_applicability(
    property: ViewPropertyKind,
    value: &ViewSpecifiedValue,
    range: arcweft_lang_syntax::ast::common::TextRange,
) -> Result<(), StyleDiagnostic> {
    let ViewSpecifiedValue::Alignment { value: alignment } = value else {
        return Ok(());
    };
    if alignment.applies_to(property) {
        return Ok(());
    }
    let expected = ViewAlignment::ALL
        .iter()
        .copied()
        .filter(|alignment| alignment.applies_to(property))
        .map(ViewAlignment::source_name)
        .collect::<Vec<_>>()
        .join(", ");
    Err(StyleDiagnostic::new(
        StyleDiagnosticCode::InvalidValueType,
        format!(
            "alignment `{}` is not valid for property `{}`",
            alignment.source_name(),
            property.source_name()
        ),
        range,
    )
    .with_subject(property.source_name())
    .with_types(expected, alignment.source_name()))
}

fn check_interactive_overflow(
    property: ViewPropertyKind,
    target: Option<ViewElementKind>,
    value: &ViewSpecifiedValue,
    range: arcweft_lang_syntax::ast::common::TextRange,
) -> Result<(), StyleDiagnostic> {
    let Some(element) = target.filter(|element| *element != ViewElementKind::Scroll) else {
        return Ok(());
    };
    let interactive_property = matches!(
        property,
        ViewPropertyKind::Overflow
            | ViewPropertyKind::OverflowX
            | ViewPropertyKind::OverflowY
            | ViewPropertyKind::OverflowInline
            | ViewPropertyKind::OverflowBlock
    );
    let interactive_value = matches!(
        value,
        ViewSpecifiedValue::Overflow {
            value: ViewOverflow::Auto | ViewOverflow::Scroll
        }
    );
    if !interactive_property || !interactive_value {
        return Ok(());
    }
    Err(StyleDiagnostic::new(
        StyleDiagnosticCode::InteractiveOverflowRequiresScroll,
        "interactive overflow requires a structural `Scroll` element",
        range,
    )
    .with_subject(property.source_name())
    .with_types(ViewElementKind::Scroll.source_name(), element.source_name()))
}

fn nearest_property_names(value: &str) -> Vec<String> {
    let mut names = ViewPropertyKind::ALL
        .iter()
        .map(|property| {
            let name = property.source_name();
            (edit_distance(value, name), name)
        })
        .collect::<Vec<_>>();
    names.sort_unstable();
    names
        .into_iter()
        .take(3)
        .map(|(_, name)| name.to_owned())
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];
    for (left_index, left_byte) in left.bytes().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_byte) in right.bytes().enumerate() {
            current[right_index + 1] = if left_byte == right_byte {
                previous[right_index]
            } else {
                previous[right_index]
                    .min(previous[right_index + 1])
                    .min(current[right_index])
                    + 1
            };
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

fn check_patch(
    patch: &HirStylePatch,
    tokens: &CheckedTokenKinds,
    diagnostics: &mut Vec<StyleDiagnostic>,
) -> CheckedViewStylePatch {
    let id = ViewStylePatchId::new(patch.ordinal());
    let declarations = patch
        .declarations()
        .iter()
        .filter_map(|declaration| check_declaration(declaration, tokens, None, None, diagnostics))
        .collect();
    CheckedViewStylePatch::new(id, declarations, patch.range())
}
