use crate::ast::common::TextRange;
use crate::ast::ids::{EntityRef, EntityRefSyntax, IdRef};
use crate::ast::view::{
    ComponentViewBody, ViewAction, ViewArg, ViewButton, ViewButtonLabel, ViewElement, ViewExpr,
    ViewImage, ViewModifier, ViewStyleModifier, ViewText, ViewTextField, ViewTextFieldMode,
    ViewTextSubmitAction, ViewTextSubmitImePolicy,
};
use crate::cst::{split_top_level_punctuation, split_top_level_punctuation_once};
use crate::expr::{Expr, Literal};

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
        if line.starts_with('.') {
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
        if !line.starts_with('.') {
            break;
        }
        consumed += collect_modifier_lines(&lines[consumed..]).max(1);
    }
    consumed
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
                ".style(@style:.name) | .style { ... } | .style(.Css) { ... } | .part(name) | .on_click { ... }",
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
            id: named_entity_arg(&args, "id"),
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
            value: first_arg_expr(args_source),
            mode: ViewTextFieldMode::TextField,
            input: first_entity_arg(&args),
            args,
        },
        "TextArea" => ViewHead::TextField {
            value: first_arg_expr(args_source),
            mode: ViewTextFieldMode::TextArea,
            input: first_entity_arg(&args),
            args,
        },
        "SecureField" => ViewHead::TextField {
            value: first_arg_expr(args_source),
            mode: ViewTextFieldMode::SecureField,
            input: first_entity_arg(&args),
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
    if let Some(value) = call_arg(line, ".placeholder") {
        return Some((ViewModifier::Placeholder(parse_expr_lossy(value)), 1));
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
    if line.starts_with(".on_") && line.contains('{') {
        let event_head = line
            .trim_start_matches('.')
            .split_once('{')
            .map_or(line.trim_start_matches('.'), |(head, _)| head.trim())
            .trim_start_matches("on_");
        let event = event_head
            .split_once('(')
            .map_or(event_head, |(event, _)| event)
            .trim()
            .to_owned();
        let ime_policy = event_head
            .split_once('(')
            .and_then(|(_, args)| args.trim_end_matches(')').trim().strip_prefix("ime:"))
            .and_then(parse_ime_policy);
        let head = line.split('{').next().unwrap_or(line);
        let (source, consumed) = collect_inline_modifier_block(lines, head);
        return Some((
            ViewModifier::OnEvent {
                name: event,
                body: parse_expr_lossy(source.trim()),
                ime_policy,
            },
            consumed,
        ));
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
            id,
            enabled,
            focusable,
        } => {
            let activation = button_activation(&chain.modifiers, range);
            let enabled = enabled
                .or_else(|| modifier_enabled(&chain.modifiers))
                .or(Some(Expr::Literal(Literal::Bool(true))));
            let focusable = modifier_focusable(&chain.modifiers).unwrap_or(focusable);
            ViewExpr::Button(
                ViewButton::new(label, chain.modifiers, range)
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
        ViewArg::Positional(expr) => Some(expr),
        ViewArg::Named { .. } => None,
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

fn button_activation(modifiers: &[ViewModifier], range: TextRange) -> Option<ViewAction> {
    modifiers.iter().find_map(|modifier| {
        let ViewModifier::OnEvent {
            name,
            body,
            ime_policy,
        } = modifier
        else {
            return None;
        };
        (name == "click")
            .then(|| text_submit_action(body, ime_policy.unwrap_or_default(), range))?
    })
}

fn text_submit_action(
    expr: &Expr,
    ime_policy: ViewTextSubmitImePolicy,
    range: TextRange,
) -> Option<ViewAction> {
    let source = match expr {
        Expr::Raw(source) | Expr::Path(source) => source.trim(),
        _ => return None,
    };
    let input = source.strip_prefix("text_submit")?.trim();
    let input = parse_expr_lossy(input);
    entity_ref_expr(&input)
        .map(|input| ViewAction::TextSubmit(ViewTextSubmitAction::new(input, ime_policy, range)))
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
