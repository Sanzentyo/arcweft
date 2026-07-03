use crate::ast::common::TextRange;
use crate::ast::ids::{EntityRef, IdRef};
use crate::ast::view::{
    ComponentViewBody, ViewArg, ViewElement, ViewExpr, ViewImage, ViewModifier, ViewStyleModifier,
    ViewText, ViewTextField, ViewTextFieldMode,
};
use crate::cst::split_top_level_punctuation;
use crate::expr::Expr;

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
    let lines = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with("//") && !line.starts_with("///"))
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

    let chain = parse_view_chain(&lines, base, module_path, errors);
    let range = TextRange::new(base, base.saturating_add(body.len()));
    Some(ComponentViewBody::new(
        Vec::new(),
        Vec::new(),
        build_view_expr(chain, range),
        range,
    ))
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
        "Button" | "Surface" | "Row" | "Column" | "Stack" => ViewHead::Element {
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
            args,
        },
        "TextArea" => ViewHead::TextField {
            value: first_arg_expr(args_source),
            mode: ViewTextFieldMode::TextArea,
            args,
        },
        "SecureField" => ViewHead::TextField {
            value: first_arg_expr(args_source),
            mode: ViewTextFieldMode::SecureField,
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
    if line.starts_with(".on_") && line.contains('{') {
        let event = line
            .trim_start_matches('.')
            .split_once('{')
            .map_or(line.trim_start_matches('.'), |(head, _)| head.trim())
            .trim_start_matches("on_")
            .to_owned();
        let head = line.split('{').next().unwrap_or(line);
        let (source, consumed) = collect_inline_modifier_block(lines, head);
        return Some((
            ViewModifier::OnEvent {
                name: event,
                body: parse_expr_lossy(source.trim()),
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
    let Some(module) = module_path
        .map(style_module_path)
        .filter(|module| !module.is_empty())
    else {
        return entity;
    };
    EntityRef::new(format!("style.{module}.{suffix}"), false, *entity.range())
}

fn style_module_path(module_path: &str) -> String {
    module_path.replace("::", ".")
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
        ViewHead::TextField { value, mode, args } => ViewExpr::TextField(ViewTextField::new(
            value,
            mode,
            args,
            chain.modifiers,
            range,
        )),
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
            split_top_level_binding(arg).map_or_else(
                || ViewArg::Positional(parse_expr_lossy(arg)),
                |(name, value)| ViewArg::Named {
                    name: name.trim().to_owned(),
                    value: parse_expr_lossy(value.trim()),
                },
            )
        })
        .collect()
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
