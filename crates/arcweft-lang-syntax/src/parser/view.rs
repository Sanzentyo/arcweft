use crate::ast::common::TextRange;
use crate::ast::flow::Stmt;
use crate::ast::ids::{EntityRef, EntityRefSyntax, IdRef};
use crate::ast::view::{
    ViewAction, ViewActionInvokeAction, ViewActionPayload, ViewArg, ViewAwait, ViewAwaitBranch,
    ViewAwaitBranchKind, ViewBody, ViewButton, ViewButtonLabel, ViewElement, ViewExpr, ViewForEach,
    ViewIf, ViewImage, ViewLet, ViewMatch, ViewMatchArm, ViewModifier, ViewNavigationDirection,
    ViewNavigationEdge, ViewNavigationModifier, ViewNavigationTarget, ViewStyleModifier, ViewText,
    ViewTextControlPayloadField, ViewTextField, ViewTextFieldMode,
};
use crate::cst::{
    ArcweftPunctuation, split_top_level_arcweft_punctuation_once, split_top_level_keyword_once,
    split_top_level_punctuation, split_top_level_punctuation_once,
};
use crate::expr::{CallArg, Expr, Literal};
use crate::pattern::parse_pattern;

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

pub(super) fn parse_view_body(
    body: &str,
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> Option<ViewBody> {
    let expanded_lines = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with("//") && !line.starts_with("///"))
        .flat_map(expand_view_line)
        .collect::<Vec<_>>();
    let lines = expanded_lines
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();

    if lines.is_empty() {
        errors.push(simple_error(
            base,
            body.len().max(1),
            "view needs a retained View expression body",
            "Panel { Button(\"Label\") }",
        ));
        return None;
    }

    let range = TextRange::new(base, base.saturating_add(body.len()));
    let value = parse_view_exprs(&lines, base, module_path, errors);
    Some(ViewBody::new(Vec::new(), Vec::new(), value, range))
}

fn expand_view_line(line: &str) -> Vec<String> {
    expand_else_line(line)
        .into_iter()
        .flat_map(|line| expand_inline_view_chain_line(&line))
        .collect()
}

fn expand_else_line(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let Some(rest) = trimmed.strip_prefix("} else") else {
        return vec![line.to_owned()];
    };
    vec!["}".to_owned(), format!("else{rest}").trim().to_owned()]
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
        if line.starts_with("else") {
            errors.push(simple_error(
                base,
                line.len(),
                "View `else` branch needs a preceding `if` block",
                "if condition { ... } else { ... }",
            ));
            index += 1;
            continue;
        }
        if line.starts_with("let ") {
            items.push(parse_view_let_line(line, base, errors));
            index += 1;
            continue;
        }
        if line.starts_with("if ") && line.ends_with('{') {
            let (nested, consumed) =
                parse_view_if_block(&lines[index..], base, module_path, errors);
            items.push(nested);
            index += consumed.max(1);
            continue;
        }
        if line.starts_with("match ") && line.ends_with('{') {
            let (nested, consumed) =
                parse_view_match_block(&lines[index..], base, module_path, errors);
            items.push(nested);
            index += consumed.max(1);
            continue;
        }
        if line.starts_with("for ") && line.ends_with('{') {
            let (nested, consumed) =
                parse_view_for_block(&lines[index..], base, module_path, errors);
            items.push(nested);
            index += consumed.max(1);
            continue;
        }
        if line.starts_with("AwaitView(") && line.ends_with('{') {
            let (nested, consumed) =
                parse_view_await_block(&lines[index..], base, module_path, errors);
            items.push(nested);
            index += consumed.max(1);
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

fn parse_view_let_line(line: &str, base: usize, errors: &mut Vec<ParseError>) -> ViewExpr {
    let rest = line.strip_prefix("let").map(str::trim).unwrap_or_default();
    let Some((pattern, value)) = split_top_level_binding(rest) else {
        errors.push(simple_error(
            base,
            line.len(),
            "View `let` binding needs `=`",
            "let visitor_name = input.text(@input:.visitor_name, initial = \"\")",
        ));
        return ViewExpr::Raw(line.to_owned());
    };
    ViewExpr::Let(ViewLet::new(
        parse_pattern(pattern.trim()),
        parse_expr_lossy(value.trim()),
        TextRange::new(base, base.saturating_add(line.len())),
    ))
}

fn parse_view_await_block(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> (ViewExpr, usize) {
    let head = lines[0].trim().trim_end_matches('{').trim();
    let Some((callee, source)) = split_simple_call(head) else {
        errors.push(simple_error(
            base,
            head.len(),
            "View await needs `AwaitView(expr) { ... }`",
            "AwaitView(load_avatar(user)) { pending _ => Text(\"Loading\") }",
        ));
        return (ViewExpr::Raw(head.to_owned()), 1);
    };
    if callee != "AwaitView" {
        errors.push(simple_error(
            base,
            head.len(),
            &format!("unsupported View await head `{callee}`"),
            "AwaitView(expr) { ... }",
        ));
        return (ViewExpr::Raw(head.to_owned()), 1);
    }
    let Some(end) = find_view_block_end(lines) else {
        errors.push(simple_error(
            base,
            head.len(),
            "unclosed View await block",
            "AwaitView(expr) { pending _ => Text(\"Loading\") }",
        ));
        return (ViewExpr::Raw(head.to_owned()), lines.len());
    };
    let branches = lines[1..end]
        .iter()
        .filter_map(|line| parse_view_await_branch(line.trim(), base, module_path, errors))
        .collect::<Vec<_>>();
    (
        ViewExpr::Await(ViewAwait::new(
            parse_expr_lossy(source.trim()),
            branches,
            TextRange::new(base, base.saturating_add(head.len())),
        )),
        end + 1,
    )
}

fn parse_view_await_branch(
    line: &str,
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> Option<ViewAwaitBranch> {
    if line.is_empty() {
        return None;
    }
    let Some((head, value)) =
        split_top_level_arcweft_punctuation_once(line, ArcweftPunctuation::FatArrow)
    else {
        errors.push(simple_error(
            base,
            line.len(),
            "View await branch needs `=>`",
            "pending _ => Text(\"Loading\")",
        ));
        return None;
    };
    let mut parts = head.trim().splitn(2, char::is_whitespace);
    let kind = parts.next().and_then(view_await_branch_kind);
    let Some(kind) = kind else {
        errors.push(simple_error(
            base,
            head.len(),
            "View await branch needs `pending`, `ready`, `error`, or `denied`",
            "ready value => Image(value)",
        ));
        return None;
    };
    let pattern = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(pattern) = pattern else {
        errors.push(simple_error(
            base,
            head.len(),
            "View await branch needs a binding pattern",
            "pending _ => Text(\"Loading\")",
        ));
        return None;
    };
    Some(ViewAwaitBranch::new(
        kind,
        parse_pattern(pattern),
        parse_view_exprs(&[value.trim()], base, module_path, errors),
    ))
}

fn view_await_branch_kind(value: &str) -> Option<ViewAwaitBranchKind> {
    match value {
        "pending" => Some(ViewAwaitBranchKind::Pending),
        "ready" => Some(ViewAwaitBranchKind::Ready),
        "error" => Some(ViewAwaitBranchKind::Error),
        "denied" => Some(ViewAwaitBranchKind::Denied),
        _ => None,
    }
}

fn parse_view_if_block(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> (ViewExpr, usize) {
    let head = lines[0].trim().trim_end_matches('{').trim();
    let condition = head.strip_prefix("if").map(str::trim).unwrap_or_default();
    let Some(then_end) = find_view_block_end(lines) else {
        errors.push(simple_error(
            base,
            head.len(),
            "unclosed View `if` block",
            "if condition { ... }",
        ));
        return (ViewExpr::Raw(head.to_owned()), lines.len());
    };
    let then_branch = parse_view_exprs(&lines[1..then_end], base, module_path, errors);
    let mut consumed = then_end + 1;
    let else_branch = lines.get(consumed).and_then(|line| {
        let line = line.trim();
        if line == "else {" {
            let else_end = find_view_block_end(&lines[consumed..])?;
            let branch = parse_view_exprs(
                &lines[consumed + 1..consumed + else_end],
                base,
                module_path,
                errors,
            );
            consumed += else_end + 1;
            Some(Box::new(branch))
        } else {
            None
        }
    });
    (
        ViewExpr::If(ViewIf::new(
            parse_expr_lossy(condition),
            Box::new(then_branch),
            else_branch,
            TextRange::new(base, base.saturating_add(head.len())),
        )),
        consumed,
    )
}

fn parse_view_match_block(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> (ViewExpr, usize) {
    let head = lines[0].trim().trim_end_matches('{').trim();
    let scrutinee = head
        .strip_prefix("match")
        .map(str::trim)
        .unwrap_or_default();
    let Some(end) = find_view_block_end(lines) else {
        errors.push(simple_error(
            base,
            head.len(),
            "unclosed View `match` block",
            "match value { .Case => Text(\"...\") }",
        ));
        return (ViewExpr::Raw(head.to_owned()), lines.len());
    };
    let arms = lines[1..end]
        .iter()
        .filter_map(|line| parse_view_match_arm(line.trim(), base, module_path, errors))
        .collect::<Vec<_>>();
    (
        ViewExpr::Match(ViewMatch::new(
            parse_expr_lossy(scrutinee),
            arms,
            TextRange::new(base, base.saturating_add(head.len())),
        )),
        end + 1,
    )
}

fn parse_view_match_arm(
    line: &str,
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> Option<ViewMatchArm> {
    if line.is_empty() {
        return None;
    }
    let Some((head, value)) =
        split_top_level_arcweft_punctuation_once(line, ArcweftPunctuation::FatArrow)
    else {
        errors.push(simple_error(
            base,
            line.len(),
            "View `match` arm needs `=>`",
            ".Case => Text(\"...\")",
        ));
        return None;
    };
    let (pattern, guard) = split_top_level_keyword_once(head, "when");
    Some(ViewMatchArm::new(
        parse_pattern(pattern.trim()),
        guard.map(|guard| parse_expr_lossy(guard.trim())),
        parse_view_exprs(&[value.trim()], base, module_path, errors),
    ))
}

fn parse_view_for_block(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> (ViewExpr, usize) {
    let head = lines[0].trim().trim_end_matches('{').trim();
    let rest = head.strip_prefix("for").map(str::trim).unwrap_or_default();
    let Some(end) = find_view_block_end(lines) else {
        errors.push(simple_error(
            base,
            head.len(),
            "unclosed View `for` block",
            "for item in items key = item.id { ... }",
        ));
        return (ViewExpr::Raw(head.to_owned()), lines.len());
    };
    let (pattern, Some(source_and_key)) = split_top_level_keyword_once(rest, "in") else {
        errors.push(simple_error(
            base,
            head.len(),
            "View `for` block needs `in`",
            "for item in items key = item.id { ... }",
        ));
        return (ViewExpr::Raw(head.to_owned()), end + 1);
    };
    let (source, key) = split_top_level_keyword_once(source_and_key, "key");
    let key = key.and_then(|key| {
        split_top_level_punctuation_once(key.trim(), '=')
            .map(|(_, value)| parse_expr_lossy(value.trim()))
    });
    let body = parse_view_exprs(&lines[1..end], base, module_path, errors);
    (
        ViewExpr::ForEach(ViewForEach::new(
            parse_pattern(pattern.trim()),
            parse_expr_lossy(source.trim()),
            key,
            Box::new(body),
            TextRange::new(base, base.saturating_add(head.len())),
        )),
        end + 1,
    )
}

fn find_view_block_end(lines: &[&str]) -> Option<usize> {
    let mut depth = 0_i32;
    for (index, line) in lines.iter().enumerate() {
        for character in line.chars() {
            match character {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if index > 0 && depth <= 0 {
            return Some(index);
        }
    }
    None
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
            if !is_view_container_element(callee) {
                errors.push(simple_error(
                    base,
                    head.len(),
                    &format!("unsupported View element `{callee}`"),
                    "Panel(...) | Box(...) | Scroll(...) | Row(...) | Column(...) | Stack(...)",
                ));
                return (ViewExpr::Raw(head.to_owned()), index + 1);
            }
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
        "Column { ... }",
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
                ".label(\"Text\") | .on_click { action.invoke(@action:.name) } | .style(@style:.name)",
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
        other if is_view_container_element(other) => ViewHead::Element {
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
        _ => {
            errors.push(simple_error(
                base,
                line.len(),
                &format!("unsupported View expression head `{callee}`"),
                "Panel(...) | Box(...) | Scroll(...) | Row(...) | Column(...) | Stack(...) | Button(...) | Text(...) | RichText(...) | TextField(...) | TextArea(...) | SecureField(...)",
            ));
            ViewHead::Raw(line.to_owned())
        }
    }
}

fn is_view_container_element(callee: &str) -> bool {
    matches!(
        callee,
        "Panel" | "Box" | "Scroll" | "Row" | "Column" | "Stack"
    )
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
    if let Some(value) = call_arg(line, ".purpose") {
        return Some((ViewModifier::Purpose(parse_expr_lossy(value)), 1));
    }
    if let Some(value) = call_arg(line, ".enter_key") {
        return Some((ViewModifier::EnterKey(parse_expr_lossy(value)), 1));
    }
    if let Some(modifier) = view_event_modifier(lines, line) {
        return Some(modifier);
    }
    if let Some(value) = call_arg(line, ".enabled") {
        return Some((ViewModifier::Enabled(parse_expr_lossy(value)), 1));
    }
    if let Some(value) = call_arg(line, ".focusable") {
        let focusable = matches!(parse_expr_lossy(value), Expr::Literal(Literal::Bool(true)));
        return Some((ViewModifier::Focusable(focusable), 1));
    }
    if let Some((name, value)) = view_property_modifier(line) {
        return Some((
            ViewModifier::Property {
                name: name.to_owned(),
                value: parse_expr_lossy(value),
            },
            1,
        ));
    }
    None
}

fn view_event_modifier(lines: &[&str], line: &str) -> Option<(ViewModifier, usize)> {
    let (head, name, tail) = view_event_head(line)?;
    if tail.starts_with('(') {
        let value = call_arg(line, head)?;
        return Some((view_on_event(name, parse_expr_lossy(value)), 1));
    }
    if tail.starts_with('{') {
        let (source, consumed) = collect_inline_modifier_block(lines, head);
        return Some((
            view_on_event(name, crate::parser::parse_callback_block_expr_body(&source)),
            consumed,
        ));
    }
    None
}

fn view_event_head(line: &str) -> Option<(&str, &str, &str)> {
    let rest = line.strip_prefix(".on_")?;
    let name_len = rest
        .char_indices()
        .find_map(|(index, ch)| (!is_view_event_name_char(ch)).then_some(index))
        .unwrap_or(rest.len());
    if name_len == 0 {
        return None;
    }
    let name = &rest[..name_len];
    let head = &line[..".on_".len() + name_len];
    let tail = rest[name_len..].trim_start();
    (tail.starts_with('(') || tail.starts_with('{')).then_some((head, name, tail))
}

fn is_view_event_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn view_on_event(name: &str, body: Expr) -> ViewModifier {
    ViewModifier::OnEvent {
        name: name.to_owned(),
        body,
    }
}

fn view_property_modifier(line: &str) -> Option<(&'static str, &str)> {
    [
        (".width", "width"),
        (".height", "height"),
        (".w", "w"),
        (".h", "h"),
        (".overflow", "overflow"),
        (".overflow_y", "overflow_y"),
        (".clip", "clip"),
        (".axis", "axis"),
        (".overscroll", "overscroll"),
        (".indicators", "indicators"),
    ]
    .into_iter()
    .find_map(|(modifier, name)| call_arg(line, modifier).map(|value| (name, value)))
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
        Expr::Raw(value) => match value.trim().trim_start_matches('.') {
            "auto" => Some(ViewNavigationTarget::Auto),
            "none" => Some(ViewNavigationTarget::None),
            "boundary" | "group_boundary" => Some(ViewNavigationTarget::GroupBoundary),
            _ => None,
        },
        Expr::Path(value) => match value.as_label().trim().trim_start_matches('.') {
            "auto" => Some(ViewNavigationTarget::Auto),
            "none" => Some(ViewNavigationTarget::None),
            "boundary" | "group_boundary" => Some(ViewNavigationTarget::GroupBoundary),
            _ => None,
        },
        Expr::ShortVariant(value) => match value.as_str() {
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
            let submit_action = submit_action_modifier(&chain.modifiers, range);
            let field = ViewTextField::new(value, mode, args, chain.modifiers, range)
                .with_submit_action(submit_action);
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

fn submit_action_modifier(modifiers: &[ViewModifier], range: TextRange) -> Option<ViewAction> {
    modifiers.iter().find_map(|modifier| match modifier {
        ViewModifier::OnEvent { name, body, .. } if name == "submit" => click_action(body, range),
        _ => None,
    })
}

fn click_action(expr: &Expr, range: TextRange) -> Option<ViewAction> {
    match expr {
        Expr::Closure { body, .. } => click_action(body, range),
        Expr::Block {
            value: Some(value), ..
        } => click_action(value, range),
        Expr::Raw(source) => {
            let body = source
                .trim()
                .strip_prefix("||")
                .map(str::trim)
                .or_else(|| strip_parameterized_closure_body(source.trim()))
                .unwrap_or(source.trim());
            let parsed = parse_expr_lossy(body);
            action_invoke_action(&parsed, range)
                .or_else(|| noop_action(&parsed))
                .or_else(|| action_invoke_action(&Expr::Raw(body.to_owned()), range))
                .or_else(|| noop_action(&Expr::Raw(body.to_owned())))
        }
        _ => action_invoke_action(expr, range).or_else(|| noop_action(expr)),
    }
}

fn noop_action(expr: &Expr) -> Option<ViewAction> {
    let source = match expr {
        Expr::Raw(source) => source.trim(),
        Expr::Path(source) => source.as_label().trim(),
        Expr::Closure { body, .. } => return noop_action(body),
        Expr::Block {
            value: Some(value), ..
        } => return noop_action(value),
        _ => return None,
    };
    let source = source
        .strip_prefix("||")
        .map(str::trim)
        .or_else(|| strip_parameterized_closure_body(source))
        .unwrap_or(source);
    (source == "noop").then_some(ViewAction::Noop)
}

fn action_invoke_action(expr: &Expr, range: TextRange) -> Option<ViewAction> {
    match expr {
        Expr::Closure { body, .. } => action_invoke_action(body, range),
        Expr::Block { statements, value } => value
            .as_deref()
            .and_then(|value| action_invoke_action(value, range))
            .or_else(|| {
                statements.iter().find_map(|statement| match statement {
                    Stmt::Expr { expr, .. } => action_invoke_action(expr, range),
                    _ => None,
                })
            }),
        Expr::Call { callee, args } if is_action_invoke_callee(callee) => {
            action_invoke_call_action(args, range)
        }
        Expr::Raw(source) => {
            let source = source
                .trim()
                .strip_prefix("||")
                .map(str::trim)
                .or_else(|| strip_parameterized_closure_body(source.trim()))
                .unwrap_or(source.trim());
            let source = source
                .trim()
                .strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'))
                .map_or(source.trim(), str::trim);
            let parsed = parse_expr_lossy(source);
            match parsed {
                Expr::Raw(_) => action_invoke_source_call_action(source, range),
                _ => action_invoke_action(&parsed, range)
                    .or_else(|| action_invoke_source_call_action(source, range)),
            }
        }
        _ => None,
    }
}

fn is_action_invoke_callee(callee: &Expr) -> bool {
    match callee {
        Expr::Path(path) => path.matches_segments(&["action", "invoke"]),
        Expr::Raw(source) => source.trim() == "action.invoke",
        Expr::Select(select) => {
            select.member() == "invoke" && expr_source(select.target()).as_deref() == Some("action")
        }
        _ => false,
    }
}

fn action_invoke_call_action(args: &[CallArg], range: TextRange) -> Option<ViewAction> {
    let action = args.iter().find_map(|arg| match arg {
        CallArg::Positional(expr) => entity_ref_expr(expr),
        CallArg::Named { name, value } if name == "action" => entity_ref_expr(value),
        CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })?;
    let payload = args.iter().find_map(|arg| match arg {
        CallArg::Named { name, value } if name != "action" => {
            action_payload(value).map(|payload| (name.clone(), payload))
        }
        CallArg::Positional(_) | CallArg::Named { .. } | CallArg::Spread { .. } => None,
    });
    Some(ViewAction::ActionInvoke(ViewActionInvokeAction::new(
        action,
        payload.as_ref().map(|(name, _)| name.clone()),
        payload.map(|(_, payload)| payload),
        range,
    )))
}

fn action_invoke_source_call_action(source: &str, range: TextRange) -> Option<ViewAction> {
    let args = source
        .trim()
        .strip_prefix("action.invoke")?
        .trim_start()
        .strip_prefix('(')?
        .trim_end()
        .strip_suffix(')')?;
    let args = parse_view_args(args);
    let action = args.iter().find_map(|arg| match arg {
        ViewArg::Positional(expr) => entity_ref_expr(expr),
        ViewArg::Named { name, value } if name == "action" => entity_ref_expr(value),
        ViewArg::Named { .. } => None,
    })?;
    let payload = args.iter().find_map(|arg| match arg {
        ViewArg::Named { name, value } if name != "action" => {
            action_payload(value).map(|payload| (name.clone(), payload))
        }
        ViewArg::Positional(_) | ViewArg::Named { .. } => None,
    });
    Some(ViewAction::ActionInvoke(ViewActionInvokeAction::new(
        action,
        payload.as_ref().map(|(name, _)| name.clone()),
        payload.map(|(_, payload)| payload),
        range,
    )))
}

fn action_payload(expr: &Expr) -> Option<ViewActionPayload> {
    match expr {
        Expr::Literal(Literal::String(value)) => {
            Some(ViewActionPayload::LiteralString(value.clone()))
        }
        Expr::Select(select) => text_control_payload_target(select.target())
            .zip(text_control_payload_field(select.member().as_str()))
            .map(|(input, field)| ViewActionPayload::TextControlProjection { input, field }),
        _ => None,
    }
}

fn text_control_payload_field(field: &str) -> Option<ViewTextControlPayloadField> {
    match field {
        "text" => Some(ViewTextControlPayloadField::Text),
        "value" => Some(ViewTextControlPayloadField::Value),
        _ => None,
    }
}

fn text_control_payload_target(expr: &Expr) -> Option<String> {
    match expr {
        Expr::EntityRef(reference) => Some(reference.canonical_body()),
        Expr::Path(path) => Some(path.as_label().to_owned()),
        Expr::Raw(source) => Some(source.trim().to_owned()),
        _ => None,
    }
}

fn expr_source(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(Literal::String(value)) => Some(format!("{value:?}")),
        Expr::Literal(Literal::Bool(value)) => Some(value.to_string()),
        Expr::EntityRef(reference) => Some(reference.canonical_body()),
        Expr::Path(path) => Some(path.as_label().to_owned()),
        Expr::ShortVariant(value) => Some(format!(".{}", value.as_str())),
        Expr::Raw(source) => Some(source.trim().to_owned()),
        Expr::Select(select) => Some(format!(
            "{}.{}",
            expr_source(select.target())?,
            select.member().as_str()
        )),
        Expr::Call { callee, args } => Some(format!(
            "{}({})",
            expr_source(callee)?,
            call_args_source(args)?
        )),
        _ => None,
    }
}

fn call_args_source(args: &[CallArg]) -> Option<String> {
    args.iter()
        .map(|arg| match arg {
            CallArg::Positional(expr) => expr_source(expr),
            CallArg::Named { name, value } => Some(format!("{name} = {}", expr_source(value)?)),
            CallArg::Spread { value } => Some(format!("..{}", expr_source(value)?)),
        })
        .collect::<Option<Vec<_>>>()
        .map(|args| args.join(", "))
}

fn strip_parameterized_closure_body(source: &str) -> Option<&str> {
    let rest = source.strip_prefix('|')?;
    let (_, body) = rest.split_once('|')?;
    Some(body.trim())
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
