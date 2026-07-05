use crate::ast::common::TextRange;
use crate::ast::ids::{EntityRef, EntityRefSyntax, IdRef};
use crate::ast::view::{
    ComponentViewBody, ViewAction, ViewArg, ViewButton, ViewButtonLabel, ViewElement, ViewExpr,
    ViewImage, ViewModifier, ViewNavigationDirection, ViewNavigationEdge, ViewNavigationModifier,
    ViewNavigationTarget, ViewStyleModifier, ViewText, ViewTextField, ViewTextFieldMode,
    ViewTextSubmitAction, ViewTextSubmitImePolicy,
};
use crate::cst::{split_top_level_punctuation, split_top_level_punctuation_once};
use crate::expr::{CallArg, Expr, Literal};

use super::headers::{normalize_decl_id_ref, parse_required_id_ref, simple_error};
use super::recovery::ParseError;
use super::{parse_expr_lossy, split_top_level_binding};

#[derive(Clone, Debug, Eq, PartialEq)]
enum ViewHead {
    Element {
        callee: String,
        args: Vec<ViewArg>,
    },
    Text {
        source: Expr,
        rich: bool,
    },
    Image {
        source: Expr,
    },
    TextField {
        value: Expr,
        mode: ViewTextFieldMode,
        args: Vec<ViewArg>,
        input: Option<EntityRefSyntax>,
    },
    Button {
        label: ViewButtonLabel,
        args: Vec<ViewArg>,
        id: Option<EntityRefSyntax>,
        enabled: Option<Expr>,
        focusable: bool,
    },
    Raw(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedViewChain {
    head: ViewHead,
    modifiers: Vec<ViewModifier>,
}

pub(super) fn parse_component_view_body(
    body: &str,
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> Option<ComponentViewBody> {
    let expanded_lines = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with("//") && !line.starts_with("///"))
        .flat_map(expand_inline_view_chain_line)
        .collect::<Vec<_>>();
    let lines = expanded_lines
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();

    if lines.is_empty() {
        errors.push(simple_error(
            base,
            body.len().max(1),
            "component returning View needs a View expression body",
            "Button(\"Label\")",
        ));
        return None;
    }

    let range = TextRange::new(base, base.saturating_add(body.len()));
    let value = parse_view_exprs(&lines, base, module_path, errors);
    Some(ComponentViewBody::new(Vec::new(), Vec::new(), value, range))
}

fn expand_inline_view_chain_line(line: &str) -> Vec<String> {
    let Some(index) = line.find(").") else {
        return vec![line.to_owned()];
    };
    let (head, tail) = line.split_at(index + 1);
    vec![head.trim().to_owned(), tail.trim().to_owned()]
}

fn parse_view_exprs(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> ViewExpr {
    let mut items = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if line == "}" {
            index += 1;
            continue;
        }
        if is_view_modifier_line(line) {
            errors.push(simple_error(
                base,
                line.len(),
                &format!("View modifier `{line}` needs a preceding View expression"),
                "Button(\"Label\").style(@style:.name)",
            ));
            index += 1;
            continue;
        }
        if line.ends_with('{') && !line.starts_with('.') {
            let (nested, consumed) = parse_view_block(&lines[index..], base, module_path, errors);
            items.push(nested);
            index += consumed.max(1);
            continue;
        }
        let consumed = collect_view_chain_lines(&lines[index..]);
        let chain = parse_view_chain(&lines[index..index + consumed], base, module_path, errors);
        let range = TextRange::new(base, base.saturating_add(line.len()));
        items.push(build_view_expr(chain, range));
        index += consumed;
    }
    match items.as_slice() {
        [single] => single.clone(),
        _ => ViewExpr::Fragment(items),
    }
}

fn parse_view_block(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> (ViewExpr, usize) {
    let head = lines[0].trim().trim_end_matches('{').trim();
    let mut depth = 0_i32;
    let mut body_start = 1;
    for (index, line) in lines.iter().enumerate() {
        for character in line.chars() {
            match character {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if index == 0 {
            body_start = 1;
        } else if depth <= 0 {
            let children = parse_view_exprs(&lines[body_start..index], base, module_path, errors);
            let child_list = match children {
                ViewExpr::Fragment(children) => children,
                child => vec![child],
            };
            let args =
                split_simple_call(head).map_or_else(Vec::new, |(_, args)| parse_view_args(args));
            let callee = split_simple_call(head)
                .map_or(head, |(callee, _)| callee)
                .trim();
            let range = TextRange::new(base, base.saturating_add(head.len()));
            return (
                ViewExpr::Element(ViewElement::new(
                    callee.to_owned(),
                    args,
                    child_list,
                    Vec::new(),
                    range,
                )),
                index + 1,
            );
        }
    }
    errors.push(simple_error(
        base,
        head.len(),
        "unclosed View element block",
        "VStack { ... }",
    ));
    (ViewExpr::Raw(head.to_owned()), lines.len())
}

fn collect_view_chain_lines(lines: &[&str]) -> usize {
    let mut consumed = 1;
    while consumed < lines.len() {
        let line = lines[consumed].trim();
        if !is_view_modifier_line(line) {
            break;
        }
        consumed += collect_modifier_lines(&lines[consumed..]).max(1);
    }
    consumed
}

fn is_view_modifier_line(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with('.')
}

fn collect_modifier_lines(lines: &[&str]) -> usize {
    let first = lines.first().map_or("", |line| line.trim());
    if !first.contains('{') {
        return 1;
    }
    let mut depth = 0_i32;
    for (index, line) in lines.iter().enumerate() {
        for character in line.chars() {
            match character {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if depth <= 0 {
            return index + 1;
        }
    }
    lines.len()
}

fn parse_view_chain(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> ParsedViewChain {
    let head = parse_view_head(lines[0], base, errors);
    let mut modifiers = Vec::new();
    let mut index = 1;
    while index < lines.len() {
        let line = lines[index];
        if let Some((modifier, consumed)) =
            parse_view_modifier(&lines[index..], base, module_path, errors)
        {
            modifiers.push(modifier);
            index += consumed.max(1);
        } else {
            errors.push(simple_error(
                base,
                line.len(),
                &format!("unsupported View modifier `{line}`"),
                ".label(\"Text\") | .on_click(|| text_submit @input:.name) | .style(@style:.name)",
            ));
            index += 1;
        }
    }
    ParsedViewChain { head, modifiers }
}

fn parse_view_head(line: &str, base: usize, errors: &mut Vec<ParseError>) -> ViewHead {
    let Some((callee, args_source)) = split_simple_call(line) else {
        return ViewHead::Raw(line.to_owned());
    };
    let args = parse_view_args(args_source);
    match callee {
        "Button" => ViewHead::Button {
            label: button_label(&args),
            args: args.clone(),
            id: named_entity_arg(&args, "id").or_else(|| first_entity_arg(&args)),
            enabled: named_arg(&args, "enabled").cloned(),
            focusable: named_arg_bool(&args, "focusable").unwrap_or(true),
        },
        "Surface" | "Row" | "Column" | "Stack" | "VStack" | "HStack" => ViewHead::Element {
            callee: callee.to_owned(),
            args,
        },
        "Text" => ViewHead::Text {
            source: first_arg_expr(args_source),
            rich: false,
        },
        "RichText" => ViewHead::Text {
            source: first_arg_expr(args_source),
            rich: true,
        },
        "Image" => ViewHead::Image {
            source: first_arg_expr(args_source),
        },
        "TextField" => ViewHead::TextField {
            value: text_field_value_expr(&args),
            mode: ViewTextFieldMode::TextField,
            input: text_field_input_arg(&args),
            args,
        },
        "TextArea" => ViewHead::TextField {
            value: text_field_value_expr(&args),
            mode: ViewTextFieldMode::TextArea,
            input: text_field_input_arg(&args),
            args,
        },
        "SecureField" => ViewHead::TextField {
            value: text_field_value_expr(&args),
            mode: ViewTextFieldMode::SecureField,
            input: text_field_input_arg(&args),
            args,
        },
        other if other.chars().next().is_some_and(char::is_uppercase) => ViewHead::Element {
            callee: other.to_owned(),
            args,
        },
        _ => {
            errors.push(simple_error(
                base,
                line.len(),
                &format!("unsupported View expression head `{callee}`"),
                "Button(...) | Text(...) | RichText(...) | TextField(...) | TextArea(...) | SecureField(...)",
            ));
            ViewHead::Raw(line.to_owned())
        }
    }
}

fn parse_view_modifier(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> Option<(ViewModifier, usize)> {
    let line = lines.first()?.trim();
    if let Some(value) = call_arg(line, ".style") {
        let (reference, trailing) = parse_view_style_ref(value, base, module_path, errors)?;
        if !trailing.trim().is_empty() {
            errors.push(simple_error(
                base,
                value.len(),
                "style reference modifier has trailing syntax",
                ".style(@style:.name)",
            ));
        }
        return Some((ViewModifier::Style(ViewStyleModifier::named(reference)), 1));
    }
    if line.starts_with(".style(.Css)") {
        let (source, consumed) = collect_inline_modifier_block(lines, ".style(.Css)");
        return Some((ViewModifier::style_css(source), consumed));
    }
    if line.starts_with(".style") && line.contains('{') {
        let (source, consumed) = collect_inline_modifier_block(lines, ".style");
        return Some((ViewModifier::style_arcweft(source), consumed));
    }
    if let Some(part) = call_arg(line, ".part") {
        return Some((ViewModifier::Part(part.trim().to_owned()), 1));
    }
    if let Some(value) = call_arg(line, ".agent_target")
        && let Some(target) = entity_ref_expr(&parse_expr_lossy(value))
    {
        return Some((ViewModifier::AgentTarget(target), 1));
    }
    if let Some(value) = call_arg(line, ".nav") {
        let range = TextRange::new(base, base.saturating_add(line.len()));
        return Some((
            ViewModifier::Navigation(parse_navigation_modifier(value, range)?),
            1,
        ));
    }
    if let Some(value) = call_arg(line, ".placeholder") {
        return Some((ViewModifier::Placeholder(parse_expr_lossy(value)), 1));
    }
    if let Some(value) = call_arg(line, ".label") {
        return Some((ViewModifier::Label(parse_expr_lossy(value)), 1));
    }
    if let Some(value) = call_arg(line, ".on_click") {
        return Some((
            ViewModifier::OnEvent {
                name: "click".to_owned(),
                body: parse_expr_lossy(value),
                ime_policy: None,
            },
            1,
        ));
    }
    if let Some(value) = call_arg(line, ".submit_action") {
        return Some((ViewModifier::SubmitAction(parse_expr_lossy(value)), 1));
    }
    if let Some(value) = call_arg(line, ".enabled") {
        return Some((ViewModifier::Enabled(parse_expr_lossy(value)), 1));
    }
    if let Some(value) = call_arg(line, ".focusable") {
        let focusable = matches!(parse_expr_lossy(value), Expr::Literal(Literal::Bool(true)));
        return Some((ViewModifier::Focusable(focusable), 1));
    }
    None
}

fn parse_view_style_ref<'a>(
    source: &'a str,
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> Option<(crate::ast::ids::EntityRefSyntax, &'a str)> {
    let (id, trailing) = parse_required_id_ref(source, base, errors)?;
    let relative = matches!(id, IdRef::Relative(_) | IdRef::FamilyRelative(_));
    let entity = normalize_decl_id_ref(id, "style", errors)?;
    let entity = if relative {
        rebase_style_ref_entity(entity, module_path)
    } else {
        entity
    };
    Some((crate::ast::ids::EntityRefSyntax::absolute(entity), trailing))
}

fn rebase_style_ref_entity(entity: EntityRef, module_path: Option<&str>) -> EntityRef {
    let Some(suffix) = entity.body().strip_prefix("style.") else {
        return entity;
    };
    EntityRef::module_scoped_declaration("style", suffix, module_path, *entity.range())
}

fn parse_navigation_modifier(source: &str, range: TextRange) -> Option<ViewNavigationModifier> {
    let edges = parse_view_args(source)
        .into_iter()
        .filter_map(|arg| {
            let ViewArg::Named { name, value } = arg else {
                return None;
            };
            Some(ViewNavigationEdge::new(
                parse_navigation_direction(&name)?,
                parse_navigation_target(&value)?,
            ))
        })
        .collect::<Vec<_>>();
    (!edges.is_empty()).then(|| ViewNavigationModifier::new(edges, range))
}

fn parse_navigation_direction(value: &str) -> Option<ViewNavigationDirection> {
    match value.trim().trim_start_matches('.') {
        "up" => Some(ViewNavigationDirection::Up),
        "down" => Some(ViewNavigationDirection::Down),
        "left" => Some(ViewNavigationDirection::Left),
        "right" => Some(ViewNavigationDirection::Right),
        "next" => Some(ViewNavigationDirection::Next),
        "previous" => Some(ViewNavigationDirection::Previous),
        _ => None,
    }
}

fn parse_navigation_target(value: &Expr) -> Option<ViewNavigationTarget> {
    match value {
        Expr::EntityRef(reference) => Some(ViewNavigationTarget::Explicit(reference.clone())),
        Expr::Raw(value) | Expr::Path(value) => match value.trim().trim_start_matches('.') {
            "auto" => Some(ViewNavigationTarget::Auto),
            "none" => Some(ViewNavigationTarget::None),
            "boundary" | "group_boundary" => Some(ViewNavigationTarget::GroupBoundary),
            _ => None,
        },
        _ => None,
    }
}

fn build_view_expr(chain: ParsedViewChain, range: TextRange) -> ViewExpr {
    match chain.head {
        ViewHead::Element { callee, args } => ViewExpr::Element(ViewElement::new(
            callee,
            args,
            Vec::new(),
            chain.modifiers,
            range,
        )),
        ViewHead::Text { source, rich } => {
            let text = ViewText::new(source, chain.modifiers, range);
            if rich {
                ViewExpr::Text(text.with_rich_surface("RichText"))
            } else {
                ViewExpr::Text(text)
            }
        }
        ViewHead::Image { source } => {
            ViewExpr::Image(ViewImage::new(source, chain.modifiers, range))
        }
        ViewHead::TextField {
            value,
            mode,
            args,
            input,
        } => {
            let field = ViewTextField::new(value, mode, args, chain.modifiers, range);
            ViewExpr::TextField(if let Some(input) = input {
                field.with_input(input)
            } else {
                field
            })
        }
        ViewHead::Button {
            label,
            args,
            id,
            enabled,
            focusable,
        } => {
            let activation = button_activation_modifier(&chain.modifiers, range);
            let enabled = enabled
                .or_else(|| modifier_enabled(&chain.modifiers))
                .or(Some(Expr::Literal(Literal::Bool(true))));
            let focusable = modifier_focusable(&chain.modifiers).unwrap_or(focusable);
            ViewExpr::Button(
                ViewButton::new(label, args, chain.modifiers, range)
                    .with_id(id)
                    .with_enabled(enabled)
                    .with_focusable(focusable)
                    .with_activation(activation),
            )
        }
        ViewHead::Raw(source) => ViewExpr::Raw(source),
    }
}

fn split_simple_call(line: &str) -> Option<(&str, &str)> {
    let open = line.find('(')?;
    let close = line.rfind(')')?;
    if close < open {
        return None;
    }
    let callee = line[..open].trim();
    (!callee.is_empty()).then_some((callee, &line[open + 1..close]))
}

fn parse_view_args(source: &str) -> Vec<ViewArg> {
    split_top_level_punctuation(source, ',')
        .into_iter()
        .map(str::trim)
        .filter(|arg| !arg.is_empty())
        .map(|arg| {
            split_top_level_binding(arg)
                .or_else(|| split_top_level_punctuation_once(arg, ':'))
                .map_or_else(
                    || ViewArg::Positional(parse_expr_lossy(arg)),
                    |(name, value)| ViewArg::Named {
                        name: name.trim().to_owned(),
                        value: parse_expr_lossy(value.trim()),
                    },
                )
        })
        .collect()
}

fn button_label(args: &[ViewArg]) -> ViewButtonLabel {
    let Some(expr) = args.iter().find_map(|arg| match arg {
        ViewArg::Positional(expr) if entity_ref_expr(expr).is_none() => Some(expr),
        ViewArg::Named { name, value } if name == "label" => Some(value),
        ViewArg::Positional(_) | ViewArg::Named { .. } => None,
    }) else {
        return ViewButtonLabel::Empty;
    };
    match expr {
        Expr::Literal(Literal::String(value)) => ViewButtonLabel::Literal(value.clone()),
        expr => ViewButtonLabel::Expr(expr.clone()),
    }
}

fn first_entity_arg(args: &[ViewArg]) -> Option<EntityRefSyntax> {
    args.iter().find_map(|arg| match arg {
        ViewArg::Positional(expr) => entity_ref_expr(expr),
        ViewArg::Named { .. } => None,
    })
}

fn text_field_input_arg(args: &[ViewArg]) -> Option<EntityRefSyntax> {
    named_entity_arg(args, "id")
        .or_else(|| named_entity_arg(args, "input"))
        .or_else(|| first_entity_arg(args))
}

fn text_field_value_expr(args: &[ViewArg]) -> Expr {
    named_arg(args, "value")
        .or_else(|| named_arg(args, "initial"))
        .cloned()
        .or_else(|| {
            args.iter().find_map(|arg| match arg {
                ViewArg::Positional(expr) if entity_ref_expr(expr).is_none() => Some(expr.clone()),
                ViewArg::Positional(_) | ViewArg::Named { .. } => None,
            })
        })
        .unwrap_or_else(|| parse_expr_lossy("\"\""))
}

fn named_entity_arg(args: &[ViewArg], name: &str) -> Option<EntityRefSyntax> {
    named_arg(args, name).and_then(entity_ref_expr)
}

fn named_arg<'a>(args: &'a [ViewArg], name: &str) -> Option<&'a Expr> {
    args.iter().find_map(|arg| match arg {
        ViewArg::Named {
            name: actual,
            value,
        } if actual == name => Some(value),
        _ => None,
    })
}

fn named_arg_bool(args: &[ViewArg], name: &str) -> Option<bool> {
    match named_arg(args, name) {
        Some(Expr::Literal(Literal::Bool(value))) => Some(*value),
        _ => None,
    }
}

fn modifier_enabled(modifiers: &[ViewModifier]) -> Option<Expr> {
    modifiers.iter().find_map(|modifier| match modifier {
        ViewModifier::Enabled(expr) => Some(expr.clone()),
        _ => None,
    })
}

fn modifier_focusable(modifiers: &[ViewModifier]) -> Option<bool> {
    modifiers.iter().find_map(|modifier| match modifier {
        ViewModifier::Focusable(value) => Some(*value),
        _ => None,
    })
}

fn entity_ref_expr(expr: &Expr) -> Option<EntityRefSyntax> {
    match expr {
        Expr::EntityRef(reference) => Some(reference.clone()),
        _ => None,
    }
}

fn button_activation_modifier(modifiers: &[ViewModifier], range: TextRange) -> Option<ViewAction> {
    modifiers.iter().find_map(|modifier| match modifier {
        ViewModifier::OnEvent { name, body, .. } if name == "click" => click_action(body, range),
        _ => None,
    })
}

fn click_action(expr: &Expr, range: TextRange) -> Option<ViewAction> {
    match expr {
        Expr::Closure { body, .. } => click_action(body, range),
        Expr::Raw(source) => {
            let body = source
                .trim()
                .strip_prefix("||")
                .map(str::trim)
                .or_else(|| strip_parameterized_closure_body(source.trim()))
                .unwrap_or(source.trim());
            let parsed = parse_expr_lossy(body);
            text_submit_action(&parsed, range)
                .or_else(|| noop_action(&parsed))
                .or_else(|| text_submit_action(&Expr::Raw(body.to_owned()), range))
                .or_else(|| noop_action(&Expr::Raw(body.to_owned())))
        }
        _ => text_submit_action(expr, range).or_else(|| noop_action(expr)),
    }
}

fn noop_action(expr: &Expr) -> Option<ViewAction> {
    let source = match expr {
        Expr::Raw(source) | Expr::Path(source) => source.trim(),
        Expr::Closure { body, .. } => return noop_action(body),
        _ => return None,
    };
    let source = source
        .strip_prefix("||")
        .map(str::trim)
        .or_else(|| strip_parameterized_closure_body(source))
        .unwrap_or(source);
    (source == "noop").then_some(ViewAction::Noop)
}

fn text_submit_action(expr: &Expr, range: TextRange) -> Option<ViewAction> {
    if let Expr::Closure { body, .. } = expr {
        return text_submit_action(body, range);
    }
    if let Expr::Call { callee, args } = expr {
        return text_submit_call_action(callee, args, range);
    }
    let source = match expr {
        Expr::Raw(source) | Expr::Path(source) => source.trim(),
        _ => return None,
    };
    let source = source
        .strip_prefix("||")
        .map(str::trim)
        .or_else(|| strip_parameterized_closure_body(source))
        .unwrap_or(source);
    let input = source.strip_prefix("text_submit")?.trim();
    if let Some(call_args) = input
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    {
        return text_submit_source_call_action(call_args, range);
    }
    let input = parse_expr_lossy(input);
    entity_ref_expr(&input).map(|input| {
        ViewAction::TextSubmit(ViewTextSubmitAction::new(
            input,
            ViewTextSubmitImePolicy::Commit,
            range,
        ))
    })
}

fn text_submit_call_action(
    callee: &Expr,
    args: &[CallArg],
    range: TextRange,
) -> Option<ViewAction> {
    let (Expr::Path(callee) | Expr::Raw(callee)) = callee else {
        return None;
    };
    if callee != "text_submit" {
        return None;
    }
    let input = args.iter().find_map(|arg| match arg {
        CallArg::Positional(expr) => entity_ref_expr(expr),
        CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })?;
    let ime_policy = args
        .iter()
        .find_map(|arg| match arg {
            CallArg::Named { name, value } if name == "ime" || name == "ime_policy" => {
                ime_policy_expr(value)
            }
            CallArg::Positional(_) | CallArg::Named { .. } | CallArg::Spread { .. } => None,
        })
        .unwrap_or_default();
    Some(ViewAction::TextSubmit(ViewTextSubmitAction::new(
        input, ime_policy, range,
    )))
}

fn text_submit_source_call_action(call_args: &str, range: TextRange) -> Option<ViewAction> {
    let args = parse_view_args(call_args);
    let input = args.iter().find_map(|arg| match arg {
        ViewArg::Positional(expr) => entity_ref_expr(expr),
        ViewArg::Named { .. } => None,
    })?;
    let ime_policy = args
        .iter()
        .find_map(|arg| match arg {
            ViewArg::Named { name, value } if name == "ime" || name == "ime_policy" => {
                ime_policy_expr(value)
            }
            ViewArg::Positional(_) | ViewArg::Named { .. } => None,
        })
        .unwrap_or_default();
    Some(ViewAction::TextSubmit(ViewTextSubmitAction::new(
        input, ime_policy, range,
    )))
}

fn strip_parameterized_closure_body(source: &str) -> Option<&str> {
    let rest = source.strip_prefix('|')?;
    let (_, body) = rest.split_once('|')?;
    Some(body.trim())
}

fn ime_policy_expr(expr: &Expr) -> Option<ViewTextSubmitImePolicy> {
    match expr {
        Expr::Path(value) | Expr::Raw(value) | Expr::Literal(Literal::String(value)) => {
            parse_ime_policy(value)
        }
        _ => None,
    }
}

fn parse_ime_policy(source: &str) -> Option<ViewTextSubmitImePolicy> {
    match source.trim() {
        ".commit" | "commit" => Some(ViewTextSubmitImePolicy::Commit),
        ".cancel" | "cancel" => Some(ViewTextSubmitImePolicy::Cancel),
        ".reject" | "reject" => Some(ViewTextSubmitImePolicy::Reject),
        _ => None,
    }
}

fn first_arg_expr(source: &str) -> Expr {
    split_top_level_punctuation(source, ',')
        .into_iter()
        .next()
        .map(str::trim)
        .filter(|arg| !arg.is_empty())
        .map_or_else(|| parse_expr_lossy("\"\""), parse_expr_lossy)
}

fn call_arg<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    source
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix('('))
        .and_then(|rest| rest.strip_suffix(')'))
        .map(str::trim)
}

fn collect_inline_modifier_block(lines: &[&str], head_prefix: &str) -> (String, usize) {
    let mut depth = 0_i32;
    let mut body = Vec::new();
    let mut consumed = 0;
    for line in lines {
        consumed += 1;
        for ch in line.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        body.push(*line);
        if consumed > 0 && depth <= 0 {
            break;
        }
    }
    let joined = body.join("\n");
    let without_head = joined
        .trim_start()
        .strip_prefix(head_prefix)
        .unwrap_or(joined.trim_start())
        .trim_start();
    let without_open = without_head.strip_prefix('{').unwrap_or(without_head);
    (
        without_open
            .trim_end()
            .trim_end_matches('}')
            .trim()
            .to_owned(),
        consumed,
    )
}
