use crate::ast::choice::{
    ChoiceAction, ChoiceItem, ChoiceMatchArm, ChoiceOption, ChoicePlanItem, ChoiceUiField,
};
use crate::ast::common::TextRange;
use crate::ast::flow::Stmt;
use crate::ast::items::RawSyntax;
use crate::cst::{
    find_matching_punctuation, find_top_level_punctuation, split_first_string_literal,
    split_top_level_keyword_once, split_top_level_punctuation_sequence_once,
};
use crate::expr::parse_expr;
use crate::pattern::parse_pattern;

use super::{
    ParseError, collect_logical_block_items, parse_expr_lossy, parse_optional_id_ref,
    parse_required_entity_ref_syntax, parse_required_id_ref, parse_stmt, parse_stmt_lines,
    parse_trigger_pattern, split_brace_item, split_pattern_guard, split_top_level_binding,
};

pub(super) fn parse_choice_items(
    body: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Vec<ChoiceItem> {
    collect_logical_block_items(body)
        .into_iter()
        .map(|line| {
            parse_choice_item(line.trim(), base, errors).unwrap_or_else(|| {
                ChoiceItem::Raw(RawSyntax::choice_item(
                    line.trim(),
                    Some(TextRange::new(base, base + line.len())),
                ))
            })
        })
        .collect()
}

fn parse_choice_item(
    trimmed: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<ChoiceItem> {
    if trimmed.starts_with("let ") {
        return Some(match parse_stmt(trimmed) {
            Stmt::Let { pattern, expr, .. } => ChoiceItem::Let { pattern, expr },
            _ => ChoiceItem::Raw(RawSyntax::choice_item(
                trimmed,
                Some(TextRange::new(base, base + trimmed.len())),
            )),
        });
    }
    if let Some((head, body)) = split_brace_item(trimmed) {
        if let Some(rest) = head.strip_prefix("option ") {
            if let (pattern, Some(source)) = split_top_level_keyword_once(rest, "in") {
                let option_head = format!("option {}", pattern.trim());
                let Some(option) = parse_choice_option_block(&option_head, body, base, errors)
                else {
                    return Some(ChoiceItem::Raw(RawSyntax::choice_item(
                        trimmed,
                        Some(TextRange::new(base, base + trimmed.len())),
                    )));
                };
                return Some(ChoiceItem::For {
                    pattern: parse_pattern(pattern.trim()),
                    source: parse_expr_lossy(source.trim()),
                    items: vec![ChoiceItem::Option(Box::new(option))],
                });
            }
        }
        if let Some(condition) = head.strip_prefix("if ") {
            return Some(ChoiceItem::If {
                condition: parse_expr_lossy(condition.trim()),
                items: parse_choice_items(body, base, errors),
            });
        }
        if let Some(expr) = head.strip_prefix("match ") {
            return Some(ChoiceItem::Match {
                expr: parse_expr_lossy(expr.trim()),
                arms: parse_choice_match_arms(body, base, errors),
            });
        }
        if let Some(rest) = head.strip_prefix("for ") {
            if let (pattern, Some(source)) = split_top_level_keyword_once(rest, "in") {
                return Some(ChoiceItem::For {
                    pattern: parse_pattern(pattern.trim()),
                    source: parse_expr_lossy(source.trim()),
                    items: parse_choice_items(body, base, errors),
                });
            }
        }
        if head.starts_with("option ") {
            return parse_choice_option_block(head, body, base, errors)
                .map(Box::new)
                .map(ChoiceItem::Option);
        }
    }
    parse_choice_arm_sugar(trimmed, base, errors)
        .map(Box::new)
        .map(ChoiceItem::Option)
}

fn parse_choice_match_arms(
    body: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Vec<ChoiceMatchArm> {
    collect_logical_block_items(body)
        .into_iter()
        .filter_map(|line| {
            let (head, value) =
                split_top_level_punctuation_sequence_once(line.trim(), &["=", ">"])?;
            let (pattern, guard) = split_pattern_guard(head.trim());
            let value = value.trim();
            let items = if let Some(block) = value
                .strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'))
            {
                parse_choice_items(block.trim(), base, errors)
            } else {
                parse_choice_item(value, base, errors).map_or_else(
                    || {
                        vec![ChoiceItem::Raw(RawSyntax::choice_item(
                            value,
                            Some(TextRange::new(base, base + value.len())),
                        ))]
                    },
                    |item| vec![item],
                )
            };
            Some(ChoiceMatchArm::new(
                parse_pattern(pattern.trim()),
                guard.map(|guard| parse_expr_lossy(guard.trim())),
                items,
            ))
        })
        .collect()
}

pub(super) fn parse_choice_plan_items(body: &str) -> Vec<ChoicePlanItem> {
    collect_logical_block_items(body)
        .into_iter()
        .map(|line| {
            let trimmed = line.trim();
            if let Some((head, block_body)) = split_brace_item(trimmed) {
                if let Some(duration) = head.strip_prefix("timeout ") {
                    return ChoicePlanItem::Timeout {
                        duration: parse_expr_lossy(duration.trim()),
                        body: parse_stmt_lines(block_body.trim()),
                    };
                }
                if let Some(trigger) = head.strip_prefix("cancel on ") {
                    return ChoicePlanItem::Cancel {
                        trigger: parse_trigger_pattern(trigger.trim()),
                        body: parse_stmt_lines(block_body.trim()),
                    };
                }
                if let Some(pattern) = head.strip_prefix("on select ") {
                    return ChoicePlanItem::OnSelect {
                        pattern: parse_pattern(pattern.trim()),
                        body: parse_stmt_lines(block_body.trim()),
                    };
                }
            }
            split_top_level_binding(trimmed).map_or_else(
                || {
                    ChoicePlanItem::Raw(RawSyntax::choice_plan_item(
                        trimmed,
                        Some(TextRange::new(0, trimmed.len())),
                    ))
                },
                |(name, value)| ChoicePlanItem::Option {
                    name: name.trim().to_owned(),
                    value: parse_expr_lossy(value.trim()),
                },
            )
        })
        .collect()
}

fn parse_choice_arm_sugar(
    trimmed: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<ChoiceOption> {
    if trimmed.is_empty() {
        return None;
    }
    let (id, rest) = parse_optional_id_ref(trimmed, base, errors);
    // Compact arms are registry-visible localization entries, so their leading
    // option ID must be static. Dynamic option IDs use full `option ... { }`
    // blocks or `option pattern in expr { id = ... }` sugar.
    let id = id?;
    let rest = rest.trim();
    let (label, after_label) = split_first_string_literal(rest)?;
    let label = label.to_owned();
    let after_label = after_label.trim();
    let (enabled, action) = if let Some(condition_body) = after_label.strip_prefix("if ") {
        let (condition, action) = split_choice_condition_action(condition_body, base, errors)?;
        (
            Some(
                parse_expr(condition.trim())
                    .unwrap_or_else(|_| crate::expr::Expr::Raw(condition.trim().to_owned())),
            ),
            action,
        )
    } else {
        let action = after_label
            .strip_prefix("->")
            .map(|target| format!("->{}", target.trim()))
            .or_else(|| {
                after_label
                    .strip_prefix("=>")
                    .map(|expr| format!("=>{}", expr.trim()))
            })?;
        (None, parse_choice_action(&action, base, errors)?)
    };
    let mut option = ChoiceOption::new(
        Some(id),
        label,
        action,
        TextRange::new(base, base + trimmed.len()),
    );
    if let Some(enabled) = enabled {
        option = option.with_enabled(enabled);
    }
    Some(option)
}

fn split_choice_condition_action<'a>(
    source: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(&'a str, ChoiceAction)> {
    if let Some((condition, target)) =
        split_top_level_punctuation_sequence_once(source, &["-", ">"])
    {
        let target = parse_required_entity_ref_syntax(target.trim(), base, errors)?.0;
        return Some((condition, ChoiceAction::Goto(target)));
    }
    split_top_level_punctuation_sequence_once(source, &["=", ">"])
        .map(|(condition, expr)| (condition, ChoiceAction::Out(parse_expr_lossy(expr.trim()))))
}

fn parse_choice_action(
    source: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<ChoiceAction> {
    if let Some(target) = source.strip_prefix("->") {
        return parse_required_entity_ref_syntax(target.trim(), base, errors)
            .map(|(entity, _)| ChoiceAction::Goto(entity));
    }
    source
        .strip_prefix("=>")
        .map(|expr| ChoiceAction::Out(parse_expr_lossy(expr.trim())))
}

fn parse_choice_option_block(
    head: &str,
    body: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<ChoiceOption> {
    let option_id = head.strip_prefix("option")?.trim();
    let (id, rest) = parse_optional_id_ref(option_id, base, errors);
    let mut id_expr =
        (id.is_none() && !rest.trim().is_empty()).then(|| parse_expr_lossy(rest.trim()));
    let mut label = String::new();
    let mut label_text_key = None;
    let mut value = None;
    let mut enabled = None;
    let mut visible = None;
    let mut order = None;
    let mut hotkey = None;
    let mut ui_fields = Vec::new();
    let mut action = ChoiceAction::None;

    for line in collect_logical_block_items(body) {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("label =") {
            label = trim_string_literal(value.trim()).unwrap_or_else(|| value.trim().to_owned());
        } else if let Some(value_expr) = trimmed.strip_prefix("id =") {
            id_expr = Some(parse_expr_lossy(value_expr.trim()));
        } else if trimmed.starts_with("label(") {
            if let Some(open) = find_top_level_punctuation(trimmed, '(')
                && let Some(close) = find_matching_punctuation(trimmed, open, '(', ')')
            {
                let key_part = &trimmed[open + '('.len_utf8()..close];
                if let Some((key, text_key)) = split_top_level_binding(key_part)
                    && key.trim() == "id"
                {
                    label_text_key = parse_required_id_ref(text_key.trim(), base, errors)
                        .map(|(entity, _)| entity);
                }
                let expr_part = trimmed[close + ')'.len_utf8()..]
                    .trim()
                    .strip_prefix('=')
                    .unwrap_or(&trimmed[close + ')'.len_utf8()..])
                    .trim();
                label = trim_string_literal(expr_part).unwrap_or_else(|| expr_part.to_owned());
            }
        } else if let Some(value_expr) = trimmed.strip_prefix("value =") {
            value = Some(parse_expr_lossy(value_expr.trim()));
        } else if let Some(value) = trimmed.strip_prefix("enabled =") {
            enabled = Some(parse_expr_lossy(value.trim()));
        } else if let Some(value) = trimmed.strip_prefix("visible =") {
            visible = Some(parse_expr_lossy(value.trim()));
        } else if let Some(value) = trimmed.strip_prefix("order =") {
            order = Some(parse_expr_lossy(value.trim()));
        } else if let Some(value) = trimmed.strip_prefix("hotkey =") {
            hotkey = Some(parse_expr_lossy(value.trim()));
        } else if let Some((head, ui_body)) = split_brace_item(trimmed) {
            if head == "ui" {
                ui_fields = parse_choice_ui_fields(ui_body);
            } else if head == "select" {
                action = parse_choice_select_action(ui_body);
            }
        }
    }

    let mut option = ChoiceOption::new(id, label, action, TextRange::new(base, base + head.len()));
    if let Some(id_expr) = id_expr {
        option = option.with_id_expr(id_expr);
    }
    if let Some(label_text_key) = label_text_key {
        option = option.with_label_text_key(label_text_key);
    }
    if let Some(value) = value {
        option = option.with_value(value);
    }
    if let Some(enabled) = enabled {
        option = option.with_enabled(enabled);
    }
    if let Some(visible) = visible {
        option = option.with_visible(visible);
    }
    if let Some(order) = order {
        option = option.with_order(order);
    }
    if let Some(hotkey) = hotkey {
        option = option.with_hotkey(hotkey);
    }
    Some(option.with_ui_fields(ui_fields))
}

fn parse_choice_ui_fields(body: &str) -> Vec<ChoiceUiField> {
    body.lines()
        .map(str::trim)
        .filter_map(|line| {
            let (name, value) = split_top_level_binding(line)?;
            Some(ChoiceUiField::new(
                name.trim().to_owned(),
                parse_expr_lossy(value.trim()),
            ))
        })
        .collect()
}

fn parse_choice_select_action(body: &str) -> ChoiceAction {
    let statements = parse_stmt_lines(body);
    match statements.as_slice() {
        [Stmt::Goto(crate::expr::Expr::EntityRef(target))] => ChoiceAction::Goto(target.clone()),
        [Stmt::Out { expr, .. }] => ChoiceAction::Out(expr.clone()),
        [] => ChoiceAction::None,
        _ => ChoiceAction::SelectBlock(statements),
    }
}

fn trim_string_literal(source: &str) -> Option<String> {
    source
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::to_owned)
}
