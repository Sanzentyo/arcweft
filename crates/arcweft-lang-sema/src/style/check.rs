//! Catalog construction and semantic checks for HIR Style.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
};

use arcweft_lang_hir::{
    model::{HirModule, HirTopLevelDecl},
    style::{
        HirStyleAssignOp, HirStyleBodyItem, HirStyleDecl, HirStyleEnvironmentBlock,
        HirStyleEnvironmentClause, HirStyleEnvironmentComparison, HirStyleEnvironmentField,
        HirStyleEnvironmentPercentage, HirStyleEnvironmentRecovery, HirStyleEnvironmentValue,
        HirStylePatch, HirStyleSelector,
    },
};
use arcweft_presentation::appearance::{
    ColorScheme, ContrastPreference, PresentationEnvironmentField, TextScaleMilli,
};
use arcweft_view::{
    ViewElementKind, ViewPartName,
    style::{
        ViewAlignment, ViewElementState, ViewInteractionSelector, ViewOverflow, ViewPropertyKind,
        ViewSpecifiedValue, ViewStyleCombinator, ViewStylePatchId, ViewStylePredicate,
        ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSheetId, ViewStyleTokenId,
        ViewTextScaleComparison,
    },
};

use super::{
    catalog::{
        CheckedStyleEnvironmentClause, CheckedStyleEnvironmentPath, CheckedStyleEnvironmentWrapper,
        CheckedStyleEnvironmentWrapperIndex, CheckedViewStyleCatalog, CheckedViewStyleDeclaration,
        CheckedViewStylePatch, CheckedViewStyleRule, CheckedViewStyleSheet, CheckedViewStyleToken,
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
    let mut rules = Vec::new();
    let mut source_order = 0usize;
    check_style_body(
        sheet.body(),
        &token_kinds,
        id.public_id().as_str(),
        &EnvironmentPathState::default(),
        &mut source_order,
        &mut rules,
        diagnostics,
    );
    Some(CheckedViewStyleSheet::new(id, tokens, rules, style.range()))
}

#[derive(Clone, Debug, Default)]
struct EnvironmentPathState {
    wrappers: Vec<CheckedStyleEnvironmentWrapper>,
    clauses: Vec<CheckedStyleEnvironmentClause>,
    fields: BTreeMap<PresentationEnvironmentField, arcweft_lang_syntax::ast::common::TextRange>,
    invalid: bool,
}

#[allow(clippy::too_many_arguments)]
fn check_style_body(
    body: &[HirStyleBodyItem],
    token_kinds: &CheckedTokenKinds,
    owner_sheet: &str,
    path: &EnvironmentPathState,
    source_order: &mut usize,
    rules: &mut Vec<CheckedViewStyleRule>,
    diagnostics: &mut Vec<StyleDiagnostic>,
) {
    for item in body {
        match item {
            HirStyleBodyItem::Rule(rule) => {
                let rule_source_order = *source_order;
                *source_order = source_order.saturating_add(1);
                if path.invalid {
                    diagnostics.push(StyleDiagnostic::new(
                        StyleDiagnosticCode::EnvironmentInvalidPath,
                        "invalid environment ancestor prevents executable Style lowering",
                        rule.range(),
                    ));
                    continue;
                }
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
                            token_kinds,
                            target,
                            Some(owner_sheet),
                            diagnostics,
                        )
                    })
                    .collect();
                let Some(selector) = selector else {
                    continue;
                };
                let environment = (!path.wrappers.is_empty()).then(|| {
                    let mut clauses = path.clauses.clone();
                    clauses.sort_by_key(|clause| clause.field());
                    CheckedStyleEnvironmentPath::new(
                        path.wrappers.clone().into_boxed_slice(),
                        clauses.into_boxed_slice(),
                    )
                });
                rules.push(CheckedViewStyleRule::new(
                    selector,
                    environment,
                    declarations,
                    u32::try_from(rule_source_order).unwrap_or(u32::MAX),
                    rule.range(),
                ));
            }
            HirStyleBodyItem::Environment(environment) => {
                let mut nested = path.clone();
                let Some(wrapper) =
                    CheckedStyleEnvironmentWrapperIndex::try_from_index(nested.wrappers.len())
                else {
                    nested.invalid = true;
                    check_style_body(
                        environment.body(),
                        token_kinds,
                        owner_sheet,
                        &nested,
                        source_order,
                        rules,
                        diagnostics,
                    );
                    continue;
                };
                nested.wrappers.push(CheckedStyleEnvironmentWrapper::new(
                    environment.predicate_range(),
                    environment.body_range(),
                    environment.scope_range(),
                ));
                if !check_environment_block(environment, wrapper, &mut nested, diagnostics) {
                    nested.invalid = true;
                }
                check_style_body(
                    environment.body(),
                    token_kinds,
                    owner_sheet,
                    &nested,
                    source_order,
                    rules,
                    diagnostics,
                );
            }
        }
    }
}

fn check_environment_block(
    environment: &HirStyleEnvironmentBlock,
    wrapper: CheckedStyleEnvironmentWrapperIndex,
    path: &mut EnvironmentPathState,
    diagnostics: &mut Vec<StyleDiagnostic>,
) -> bool {
    let mut valid = true;
    if environment.clauses().is_empty() {
        diagnostics.push(StyleDiagnostic::new(
            StyleDiagnosticCode::EnvironmentEmptyCondition,
            "environment wrapper condition cannot be empty",
            environment.predicate_range(),
        ));
        valid = false;
    }
    if environment.clauses().len() > 4 {
        diagnostics.push(StyleDiagnostic::new(
            StyleDiagnosticCode::EnvironmentConditionLimit,
            "environment wrapper contains more than four clauses",
            environment.predicate_range(),
        ));
        valid = false;
    }

    let mut local_fields = BTreeMap::new();
    for clause in environment.clauses() {
        let field = match clause.field() {
            HirStyleEnvironmentField::ColorScheme => PresentationEnvironmentField::ColorScheme,
            HirStyleEnvironmentField::Contrast => PresentationEnvironmentField::Contrast,
            HirStyleEnvironmentField::ReducedMotion => PresentationEnvironmentField::ReducedMotion,
            HirStyleEnvironmentField::TextScale => PresentationEnvironmentField::TextScale,
            HirStyleEnvironmentField::Recovered { spelling } => {
                diagnostics.push(
                    StyleDiagnostic::new(
                        StyleDiagnosticCode::EnvironmentExpectedField,
                        format!("unknown presentation-environment field `{spelling}`"),
                        clause.ranges().field(),
                    )
                    .with_subject(spelling.as_ref()),
                );
                valid = false;
                continue;
            }
        };

        if let Some(first) = local_fields.get(&field).copied() {
            diagnostics.push(
                StyleDiagnostic::new(
                    StyleDiagnosticCode::EnvironmentDuplicateField,
                    format!("environment condition repeats field {field:?}"),
                    clause.ranges().field(),
                )
                .with_related_range(first),
            );
            valid = false;
            continue;
        }
        local_fields.insert(field, clause.ranges().field());
        if let Some(ancestor) = path.fields.get(&field).copied() {
            diagnostics.push(
                StyleDiagnostic::new(
                    StyleDiagnosticCode::EnvironmentDuplicateFieldOnPath,
                    format!("nested environment path repeats field {field:?}"),
                    clause.ranges().field(),
                )
                .with_related_range(ancestor),
            );
            valid = false;
            continue;
        }

        match check_environment_clause(field, wrapper, clause, diagnostics) {
            Some(checked) => {
                path.fields.insert(field, clause.ranges().field());
                path.clauses.push(checked);
            }
            None => valid = false,
        }
    }
    valid
}

fn check_environment_clause(
    field: PresentationEnvironmentField,
    wrapper: CheckedStyleEnvironmentWrapperIndex,
    clause: &HirStyleEnvironmentClause,
    diagnostics: &mut Vec<StyleDiagnostic>,
) -> Option<CheckedStyleEnvironmentClause> {
    if clause.comparison() == HirStyleEnvironmentComparison::Recovered {
        let code = if clause.ranges().comparison().start() == clause.ranges().comparison().end() {
            StyleDiagnosticCode::EnvironmentExpectedComparison
        } else {
            StyleDiagnosticCode::EnvironmentInvalidComparison
        };
        diagnostics.push(StyleDiagnostic::new(
            code,
            "environment clause has an invalid comparison",
            clause.ranges().comparison(),
        ));
        return None;
    }
    if field != PresentationEnvironmentField::TextScale
        && clause.comparison() != HirStyleEnvironmentComparison::Equal
    {
        diagnostics.push(StyleDiagnostic::new(
            StyleDiagnosticCode::EnvironmentInvalidComparison,
            "enum and boolean environment fields support only `==`",
            clause.ranges().comparison(),
        ));
        return None;
    }

    match (field, clause.value()) {
        (
            PresentationEnvironmentField::ColorScheme,
            HirStyleEnvironmentValue::Identifier { spelling },
        ) => match spelling.as_ref() {
            "light" => Some(CheckedStyleEnvironmentClause::ColorScheme {
                value: ColorScheme::Light,
                wrapper,
                range: clause.ranges().clause(),
            }),
            "dark" => Some(CheckedStyleEnvironmentClause::ColorScheme {
                value: ColorScheme::Dark,
                wrapper,
                range: clause.ranges().clause(),
            }),
            _ => {
                invalid_environment_value(spelling, clause, diagnostics);
                None
            }
        },
        (
            PresentationEnvironmentField::Contrast,
            HirStyleEnvironmentValue::Identifier { spelling },
        ) => match spelling.as_ref() {
            "standard" => Some(CheckedStyleEnvironmentClause::Contrast {
                value: ContrastPreference::Standard,
                wrapper,
                range: clause.ranges().clause(),
            }),
            "more" => Some(CheckedStyleEnvironmentClause::Contrast {
                value: ContrastPreference::More,
                wrapper,
                range: clause.ranges().clause(),
            }),
            _ => {
                invalid_environment_value(spelling, clause, diagnostics);
                None
            }
        },
        (PresentationEnvironmentField::ReducedMotion, HirStyleEnvironmentValue::Boolean(value)) => {
            Some(CheckedStyleEnvironmentClause::ReducedMotion {
                value: *value,
                wrapper,
                range: clause.ranges().clause(),
            })
        }
        (
            PresentationEnvironmentField::TextScale,
            HirStyleEnvironmentValue::Percentage(percentage),
        ) => check_text_scale(percentage, clause, diagnostics).map(|value| {
            CheckedStyleEnvironmentClause::TextScale {
                comparison: checked_text_scale_comparison(clause.comparison()),
                value,
                wrapper,
                range: clause.ranges().clause(),
            }
        }),
        (_, HirStyleEnvironmentValue::Recovered(recovery)) => {
            recovered_environment_value(recovery, clause, diagnostics);
            None
        }
        (_, value) => {
            diagnostics.push(
                StyleDiagnostic::new(
                    StyleDiagnosticCode::EnvironmentInvalidValue,
                    "environment value does not belong to the selected field",
                    clause.ranges().value(),
                )
                .with_subject(format!("{value:?}")),
            );
            None
        }
    }
}

fn invalid_environment_value(
    spelling: &str,
    clause: &HirStyleEnvironmentClause,
    diagnostics: &mut Vec<StyleDiagnostic>,
) {
    diagnostics.push(
        StyleDiagnostic::new(
            StyleDiagnosticCode::EnvironmentInvalidValue,
            format!("unsupported environment value `{spelling}`"),
            clause.ranges().value(),
        )
        .with_subject(spelling),
    );
}

fn recovered_environment_value(
    recovery: &HirStyleEnvironmentRecovery,
    clause: &HirStyleEnvironmentClause,
    diagnostics: &mut Vec<StyleDiagnostic>,
) {
    let (code, message) = match recovery {
        HirStyleEnvironmentRecovery::MissingValue => (
            StyleDiagnosticCode::EnvironmentExpectedValue,
            "environment clause needs a value",
        ),
        HirStyleEnvironmentRecovery::UnsupportedValue(
            arcweft_lang_syntax::ast::style::StyleEnvironmentUnsupportedValueKind::FractionalPrecision,
        ) => (
            StyleDiagnosticCode::EnvironmentTextScalePrecision,
            "text-scale permits at most one fractional digit",
        ),
        HirStyleEnvironmentRecovery::TextScaleOutOfRange => (
            StyleDiagnosticCode::EnvironmentTextScaleRange,
            "text-scale is outside 50%..=400%",
        ),
        HirStyleEnvironmentRecovery::InvalidComparison => (
            StyleDiagnosticCode::EnvironmentInvalidComparison,
            "environment comparison is invalid",
        ),
        HirStyleEnvironmentRecovery::UnknownField { .. } => (
            StyleDiagnosticCode::EnvironmentExpectedField,
            "environment field is unknown",
        ),
        HirStyleEnvironmentRecovery::InvalidEnumValue { .. } => (
            StyleDiagnosticCode::EnvironmentInvalidValue,
            "environment enum value is invalid",
        ),
        HirStyleEnvironmentRecovery::DuplicateField => (
            StyleDiagnosticCode::EnvironmentDuplicateField,
            "environment condition repeats a field",
        ),
        HirStyleEnvironmentRecovery::DuplicateFieldOnEffectivePath => (
            StyleDiagnosticCode::EnvironmentDuplicateFieldOnPath,
            "environment path repeats a field",
        ),
        HirStyleEnvironmentRecovery::UnsupportedValue(_) => (
            StyleDiagnosticCode::EnvironmentUnsupportedValue,
            "environment value uses an unsupported lexical form",
        ),
    };
    diagnostics.push(StyleDiagnostic::new(code, message, clause.ranges().value()));
}

const fn checked_text_scale_comparison(
    comparison: HirStyleEnvironmentComparison,
) -> ViewTextScaleComparison {
    match comparison {
        HirStyleEnvironmentComparison::Equal | HirStyleEnvironmentComparison::Recovered => {
            ViewTextScaleComparison::Equal
        }
        HirStyleEnvironmentComparison::NotEqual => ViewTextScaleComparison::NotEqual,
        HirStyleEnvironmentComparison::Less => ViewTextScaleComparison::Less,
        HirStyleEnvironmentComparison::LessOrEqual => ViewTextScaleComparison::LessOrEqual,
        HirStyleEnvironmentComparison::Greater => ViewTextScaleComparison::Greater,
        HirStyleEnvironmentComparison::GreaterOrEqual => ViewTextScaleComparison::GreaterOrEqual,
    }
}

fn check_text_scale(
    percentage: &HirStyleEnvironmentPercentage,
    clause: &HirStyleEnvironmentClause,
    diagnostics: &mut Vec<StyleDiagnostic>,
) -> Option<TextScaleMilli> {
    let fractional_digits = percentage.fractional_digits().unwrap_or("");
    if fractional_digits.len() > 1 {
        diagnostics.push(StyleDiagnostic::new(
            StyleDiagnosticCode::EnvironmentTextScalePrecision,
            "text-scale permits at most one fractional digit",
            clause.ranges().value(),
        ));
        return None;
    }
    let normalized = percentage.integer_digits().trim_start_matches('0');
    let normalized = if normalized.is_empty() {
        "0"
    } else {
        normalized
    };
    let fractional = fractional_digits
        .bytes()
        .next()
        .map_or(0, |digit| digit.saturating_sub(b'0'));
    if compare_decimal_percentage(normalized, fractional, "50", 0) == Ordering::Less
        || compare_decimal_percentage(normalized, fractional, "400", 0) == Ordering::Greater
    {
        diagnostics.push(StyleDiagnostic::new(
            StyleDiagnosticCode::EnvironmentTextScaleRange,
            "text-scale is outside 50%..=400%",
            clause.ranges().value(),
        ));
        return None;
    }
    let integer = normalized
        .bytes()
        .fold(0u16, |value, digit| value * 10 + u16::from(digit - b'0'));
    TextScaleMilli::try_new(integer * 10 + u16::from(fractional)).ok()
}

fn compare_decimal_percentage(
    integer: &str,
    fractional: u8,
    bound_integer: &str,
    bound_fractional: u8,
) -> Ordering {
    integer
        .len()
        .cmp(&bound_integer.len())
        .then_with(|| integer.cmp(bound_integer))
        .then_with(|| fractional.cmp(&bound_fractional))
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
