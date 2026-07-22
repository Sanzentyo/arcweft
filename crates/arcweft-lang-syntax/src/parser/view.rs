use crate::ast::common::TextRange;
use crate::ast::flow::Stmt;
use crate::ast::ids::{EntityRef, EntityRefSyntax, IdRef};
use crate::ast::view::{
    ViewAction, ViewActionInvokeAction, ViewActionPayload, ViewArg, ViewAwait, ViewAwaitBranch,
    ViewAwaitBranchKind, ViewBody, ViewButton, ViewButtonLabel, ViewCall, ViewElement, ViewExpr,
    ViewForEach, ViewFxApplication, ViewFxApplicationOrdinal, ViewIf, ViewImage, ViewLet,
    ViewMatch, ViewMatchArm, ViewModifier, ViewNavigationDirection, ViewNavigationEdge,
    ViewNavigationModifier, ViewNavigationTarget, ViewStyleModifier, ViewText,
    ViewTextControlPayloadField, ViewTextField, ViewTextFieldMode,
};
use crate::cst::{
    ArcweftPunctuation, CstPunctuationScan, split_top_level_arcweft_punctuation_once,
    split_top_level_keyword_once, split_top_level_punctuation, split_top_level_punctuation_once,
};
use crate::expr::{CallArg, Expr, Literal};
use crate::pattern::parse_pattern_at;

use super::headers::{normalize_decl_id_ref, parse_required_id_ref, simple_error};
use super::recovery::{ParseError, ParseErrorKind, RecoverySuggestion};
use super::style::parse_inline_native_style;
use super::{parse_expr_lossy, split_top_level_binding};
use arcweft_source::SourceDocument;

mod part;

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
        enabled: Box<Option<Expr>>,
        focusable: bool,
    },
    ViewCall {
        view: Expr,
        args: Vec<ViewArg>,
    },
    Raw(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedViewChain {
    head: ViewHead,
    modifiers: Vec<ViewModifier>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ViewSourceLine {
    text: String,
    start: usize,
    end: usize,
}

struct ViewSourceMap<'a> {
    body: &'a str,
    document: &'a SourceDocument,
    base: usize,
    lines: Vec<(usize, usize, usize)>,
}

impl<'a> ViewSourceMap<'a> {
    fn new(
        body: &'a str,
        document: &'a SourceDocument,
        base: usize,
        lines: &[ViewSourceLine],
    ) -> Self {
        Self {
            body,
            document,
            base,
            lines: lines
                .iter()
                .map(|line| (line.text.as_ptr() as usize, line.start, line.end))
                .collect(),
        }
    }

    fn location(&self, line: &str) -> Option<TextRange> {
        let pointer = line.as_ptr() as usize;
        let pointer_end = pointer.checked_add(line.len())?;
        let body_pointer = self.body.as_ptr() as usize;
        let body_pointer_end = body_pointer.checked_add(self.body.len())?;
        if pointer >= body_pointer && pointer_end <= body_pointer_end {
            let offset = pointer.checked_sub(body_pointer)?;
            let range_start = self.base.checked_add(offset)?;
            let range_end = range_start.checked_add(line.len())?;
            return Some(TextRange::new(range_start, range_end));
        }
        self.lines.iter().find_map(|(candidate, start, end)| {
            let line_len = end.checked_sub(*start)?;
            let candidate_end = candidate.checked_add(line_len)?;
            if pointer < *candidate || pointer_end > candidate_end {
                return None;
            }
            let offset = pointer.checked_sub(*candidate)?;
            let range_start = start.checked_add(offset)?;
            let range_end = range_start.checked_add(line.len())?;
            Some(TextRange::new(range_start, range_end))
        })
    }

    fn mapped_location(&self, source: &str) -> TextRange {
        self.location(source)
            .expect("View parser fragments remain attached to their authored source line")
    }

    fn mapped_lines_range(&self, lines: &[&str]) -> TextRange {
        let first = self.mapped_location(
            lines
                .first()
                .expect("a parsed View node consumes at least one source line"),
        );
        let last = self.mapped_location(
            lines
                .last()
                .expect("a parsed View node consumes at least one source line"),
        );
        TextRange::new(first.start(), last.end())
    }

    fn source(&self, range: TextRange) -> Option<&'a str> {
        let start = range.start().checked_sub(self.base)?;
        let end = range.end().checked_sub(self.base)?;
        self.body.get(start..end)
    }

    fn parse_owned_expr(&self, source: &str, errors: &mut Vec<ParseError>) -> Expr {
        let Some(range) = self.location(source) else {
            errors.push(ParseError::new(
                TextRange::new(self.base, self.base),
                vec!["View expression with an authored source range".to_owned()],
                None,
                "View expression is not a checked subslice of its source owner".to_owned(),
                Vec::new(),
            ));
            return Expr::Raw(source.trim().to_owned());
        };
        super::helpers::parse_owned_expr_recovering(source, range.start(), None, errors)
    }

    fn lines_source(&self, lines: &[&str], consumed: usize) -> Option<(&'a str, TextRange)> {
        let first = self.location(lines.first()?)?;
        let last = self.location(lines.get(consumed.checked_sub(1)?)?)?;
        let range = TextRange::new(first.start(), last.end());
        self.source(range).map(|source| (source, range))
    }
}

pub(super) fn parse_view_body<'a>(
    body: &'a str,
    base: usize,
    module_path: Option<&str>,
    document: Option<&'a SourceDocument>,
    errors: &mut Vec<ParseError>,
) -> Option<ViewBody> {
    let Some(document) = document else {
        errors.push(simple_error(
            base,
            body.len().max(1),
            "View body cannot be parsed without a source identity",
            "revision-bound source document",
        ));
        return None;
    };
    let expanded_lines = mapped_view_lines(body, base);
    let source_map = ViewSourceMap::new(body, document, base, &expanded_lines);
    let lines = expanded_lines
        .iter()
        .map(|line| line.text.as_str())
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

    let export_count = lines
        .iter()
        .take_while(|line| part::is_export_candidate(line.trim()))
        .count();
    let exports = expanded_lines[..export_count]
        .iter()
        .filter_map(|line| part::parse_export(line, document, errors))
        .collect();
    let range = TextRange::new(base, base.saturating_add(body.len()));
    let value = parse_view_exprs(
        &lines[export_count..],
        base,
        module_path,
        &source_map,
        errors,
    );
    Some(ViewBody::new(Vec::new(), Vec::new(), exports, value, range))
}

fn mapped_view_lines(body: &str, base: usize) -> Vec<ViewSourceLine> {
    let mut offset = 0;
    body.split_inclusive('\n')
        .filter_map(|raw_line| {
            let content = raw_line.trim_end_matches(['\r', '\n']);
            let trimmed = content.trim();
            let leading = content.len() - content.trim_start().len();
            let line =
                (!trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("///"))
                    .then(|| ViewSourceLine {
                        text: trimmed.to_owned(),
                        start: base + offset + leading,
                        end: base + offset + leading + trimmed.len(),
                    });
            offset += raw_line.len();
            line
        })
        .flat_map(expand_view_line)
        .collect()
}

fn expand_view_line(line: ViewSourceLine) -> Vec<ViewSourceLine> {
    expand_else_line(line)
        .into_iter()
        .flat_map(expand_inline_view_chain_line)
        .collect()
}

fn expand_else_line(line: ViewSourceLine) -> Vec<ViewSourceLine> {
    let Some(rest) = line.text.strip_prefix("} else") else {
        return vec![line];
    };
    let else_start = line.text.len() - "else".len() - rest.len();
    vec![
        ViewSourceLine {
            text: "}".to_owned(),
            start: line.start,
            end: line.start + '}'.len_utf8(),
        },
        ViewSourceLine {
            text: line.text[else_start..].to_owned(),
            start: line.start + else_start,
            end: line.end,
        },
    ]
}

fn expand_inline_view_chain_line(line: ViewSourceLine) -> Vec<ViewSourceLine> {
    let boundaries =
        CstPunctuationScan::new(&line.text).parenthesized_postfix_separator_offsets('.');
    if boundaries.is_empty() {
        return vec![line];
    }

    let mut fragments = Vec::with_capacity(boundaries.len() + 1);
    let mut fragment_start = 0usize;
    for fragment_end in boundaries
        .into_iter()
        .chain(std::iter::once(line.text.len()))
    {
        fragments.push(ViewSourceLine {
            text: line.text[fragment_start..fragment_end].to_owned(),
            start: line.start + fragment_start,
            end: if fragment_end == line.text.len() {
                line.end
            } else {
                line.start + fragment_end
            },
        });
        fragment_start = fragment_end;
    }
    fragments
}

fn parse_view_exprs(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    source_map: &ViewSourceMap<'_>,
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
        if part::is_export_candidate(line) {
            let range = source_map
                .location(lines[index])
                .unwrap_or_else(|| TextRange::new(base, base.saturating_add(line.len())));
            let expected = "export part local as public before the View expression";
            errors.push(ParseError::new_with_kind(
                ParseErrorKind::ViewExportPartMisplaced,
                range,
                vec![expected.to_owned()],
                None,
                "View part exports must form the leading declaration block".to_owned(),
                vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
            ));
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
            items.push(parse_view_let_line(line, base, source_map, errors));
            index += 1;
            continue;
        }
        if let Some((nested, consumed)) =
            parse_view_control_expr(&lines[index..], base, module_path, source_map, errors)
        {
            items.push(nested);
            index += consumed.max(1);
            continue;
        }
        if line.ends_with('{') && !line.starts_with('.') {
            let (nested, consumed) =
                parse_view_block(&lines[index..], base, module_path, source_map, errors);
            items.push(nested);
            index += consumed.max(1);
            continue;
        }
        let consumed = collect_view_chain_lines(&lines[index..]);
        let chain = parse_view_chain(
            &lines[index..index + consumed],
            base,
            module_path,
            source_map,
            errors,
        );
        let range = source_map.mapped_lines_range(&lines[index..index + consumed]);
        items.push(build_view_expr(chain, range));
        index += consumed;
    }
    match items.as_slice() {
        [single] => single.clone(),
        _ => ViewExpr::Fragment(items),
    }
}

fn parse_view_control_expr(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    source_map: &ViewSourceMap<'_>,
    errors: &mut Vec<ParseError>,
) -> Option<(ViewExpr, usize)> {
    let line = lines.first()?.trim();
    let (kind, error, example) = if starts_view_control_keyword(line, "if") {
        (
            "if",
            "View `if` head must end before its braced body",
            "if condition {\n    Text(\"visible\")\n}",
        )
    } else if starts_view_control_keyword(line, "match") {
        (
            "match",
            "View `match` head must end before its braced body",
            "match value {\n    .Case => Text(\"value\")\n}",
        )
    } else if starts_view_control_keyword(line, "for") {
        (
            "for",
            "View `for` head must end before its braced body",
            "for item in items key = item.id {\n    Row(item)\n}",
        )
    } else if line == "AwaitView" || line.starts_with("AwaitView(") {
        (
            "await",
            "View await head must end before its braced branches",
            "AwaitView(load()) {\n    ready value => Text(value)\n}",
        )
    } else {
        return None;
    };
    if !line.ends_with('{') {
        let range = source_map.mapped_location(lines[0]);
        errors.push(simple_error(
            range.start(),
            range.end() - range.start(),
            error,
            example,
        ));
        return Some((ViewExpr::Raw(line.to_owned()), 1));
    }
    Some(match kind {
        "if" => parse_view_if_block(lines, base, module_path, source_map, errors),
        "match" => parse_view_match_block(lines, base, module_path, source_map, errors),
        "for" => parse_view_for_block(lines, base, module_path, source_map, errors),
        "await" => parse_view_await_block(lines, base, module_path, source_map, errors),
        _ => unreachable!("View control kind is selected above"),
    })
}

fn starts_view_control_keyword(line: &str, keyword: &str) -> bool {
    line == keyword
        || line
            .strip_prefix(keyword)
            .is_some_and(|rest| rest.chars().next().is_some_and(char::is_whitespace))
}

fn parse_view_let_line(
    line: &str,
    base: usize,
    source_map: &ViewSourceMap<'_>,
    errors: &mut Vec<ParseError>,
) -> ViewExpr {
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
        parse_pattern_at(
            pattern.trim(),
            source_map.mapped_location(line).start()
                + (pattern.trim().as_ptr() as usize - line.as_ptr() as usize),
        ),
        source_map.parse_owned_expr(value.trim(), errors),
        source_map.mapped_location(line),
    ))
}

fn parse_view_await_block(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    source_map: &ViewSourceMap<'_>,
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
    let mut branches = Vec::new();
    let mut branch_recovery = false;
    for line in &lines[1..end] {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(branch) = parse_view_await_branch(line, base, module_path, source_map, errors) {
            branches.push(branch);
        } else {
            branch_recovery = true;
        }
    }
    if branch_recovery {
        return (ViewExpr::Raw(head.to_owned()), end + 1);
    }
    (
        ViewExpr::Await(ViewAwait::new(
            source_map.parse_owned_expr(source.trim(), errors),
            branches,
            source_map.mapped_lines_range(&lines[..=end]),
        )),
        end + 1,
    )
}

fn parse_view_await_branch(
    line: &str,
    base: usize,
    module_path: Option<&str>,
    source_map: &ViewSourceMap<'_>,
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
        parse_pattern_at(
            pattern,
            source_map.mapped_location(line).start()
                + (pattern.as_ptr() as usize - line.as_ptr() as usize),
        ),
        parse_view_exprs(&[value.trim()], base, module_path, source_map, errors),
        source_map.mapped_location(line),
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
    source_map: &ViewSourceMap<'_>,
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
    let then_branch = parse_view_exprs(&lines[1..then_end], base, module_path, source_map, errors);
    let mut consumed = then_end + 1;
    let else_branch = lines.get(consumed).and_then(|line| {
        let line = line.trim();
        if line == "else {" {
            let else_end = find_view_block_end(&lines[consumed..])?;
            let branch = parse_view_exprs(
                &lines[consumed + 1..consumed + else_end],
                base,
                module_path,
                source_map,
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
            source_map.parse_owned_expr(condition, errors),
            Box::new(then_branch),
            else_branch,
            source_map.mapped_lines_range(&lines[..consumed]),
        )),
        consumed,
    )
}

fn parse_view_match_block(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    source_map: &ViewSourceMap<'_>,
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
    let mut arms = Vec::new();
    let mut arm_recovery = false;
    for line in &lines[1..end] {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(arm) = parse_view_match_arm(line, base, module_path, source_map, errors) {
            arms.push(arm);
        } else {
            arm_recovery = true;
        }
    }
    if arm_recovery {
        return (ViewExpr::Raw(head.to_owned()), end + 1);
    }
    (
        ViewExpr::Match(ViewMatch::new(
            source_map.parse_owned_expr(scrutinee, errors),
            arms,
            source_map.mapped_lines_range(&lines[..=end]),
        )),
        end + 1,
    )
}

fn parse_view_match_arm(
    line: &str,
    base: usize,
    module_path: Option<&str>,
    source_map: &ViewSourceMap<'_>,
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
        parse_pattern_at(
            pattern.trim(),
            source_map.mapped_location(line).start()
                + (pattern.trim().as_ptr() as usize - line.as_ptr() as usize),
        ),
        guard.map(|guard| source_map.parse_owned_expr(guard.trim(), errors)),
        parse_view_exprs(&[value.trim()], base, module_path, source_map, errors),
        source_map.mapped_location(line),
    ))
}

fn parse_view_for_block(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    source_map: &ViewSourceMap<'_>,
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
    let (source, key_source) = split_top_level_keyword_once(source_and_key, "key");
    let key = match key_source {
        None => None,
        Some(key_source) => {
            let key_source = key_source.trim();
            let Some((before_equals, value)) = split_top_level_punctuation_once(key_source, '=')
            else {
                let range = source_map.mapped_location(lines[0]);
                errors.push(simple_error(
                    range.start(),
                    range.end() - range.start(),
                    "View `for` key needs `=`",
                    "for item in items key = item.id { ... }",
                ));
                return (ViewExpr::Raw(head.to_owned()), end + 1);
            };
            if !before_equals.trim().is_empty() || value.trim().is_empty() {
                let range = source_map.mapped_location(lines[0]);
                errors.push(simple_error(
                    range.start(),
                    range.end() - range.start(),
                    "View `for` key needs exactly `key = expression`",
                    "for item in items key = item.id { ... }",
                ));
                return (ViewExpr::Raw(head.to_owned()), end + 1);
            }
            Some(source_map.parse_owned_expr(value.trim(), errors))
        }
    };
    let body = parse_view_exprs(&lines[1..end], base, module_path, source_map, errors);
    (
        ViewExpr::ForEach(ViewForEach::new(
            parse_pattern_at(
                pattern.trim(),
                source_map.mapped_location(lines[0]).start()
                    + (pattern.trim().as_ptr() as usize - head.as_ptr() as usize),
            ),
            source_map.parse_owned_expr(source.trim(), errors),
            key,
            Box::new(body),
            source_map.mapped_lines_range(&lines[..=end]),
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
    source_map: &ViewSourceMap<'_>,
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
            let children = parse_view_exprs(
                &lines[body_start..index],
                base,
                module_path,
                source_map,
                errors,
            );
            let child_list = match children {
                ViewExpr::Fragment(children) => children,
                child => vec![child],
            };
            let args = split_simple_call(head).map_or_else(Vec::new, |(_, args)| {
                parse_view_args_recovering(args, source_map, errors)
            });
            let callee = split_simple_call(head)
                .map_or(head, |(callee, _)| callee)
                .trim();
            if !is_view_container_element(callee) {
                let range = source_map.mapped_location(lines[0]);
                errors.push(simple_error(
                    range.start(),
                    range.end() - range.start(),
                    &format!("unsupported View element `{callee}`"),
                    "Panel(...) | Box(...) | Scroll(...) | Row(...) | Column(...) | Stack(...)",
                ));
                return (ViewExpr::Raw(head.to_owned()), index + 1);
            }
            let (modifiers, modifier_lines) =
                parse_view_modifiers(&lines[index + 1..], base, module_path, source_map, errors);
            let range = source_map.mapped_lines_range(&lines[..index + 1 + modifier_lines]);
            return (
                ViewExpr::Element(ViewElement::new(
                    callee.to_owned(),
                    args,
                    child_list,
                    modifiers,
                    range,
                )),
                index + 1 + modifier_lines,
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
    let mut delimiters = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for (index, line) in lines.iter().enumerate() {
        for character in line.chars() {
            if let Some(active_quote) = quote {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == active_quote {
                    quote = None;
                }
                continue;
            }
            match character {
                '"' => quote = Some(character),
                '(' | '[' | '{' => delimiters.push(character),
                ')' if delimiters.last() == Some(&'(') => {
                    delimiters.pop();
                }
                ']' if delimiters.last() == Some(&'[') => {
                    delimiters.pop();
                }
                '}' if delimiters.last() == Some(&'{') => {
                    delimiters.pop();
                }
                _ => (),
            }
        }
        if delimiters.is_empty() && quote.is_none() {
            return index + 1;
        }
    }
    lines.len()
}

fn parse_view_chain(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    source_map: &ViewSourceMap<'_>,
    errors: &mut Vec<ParseError>,
) -> ParsedViewChain {
    let head = parse_view_head(lines[0], module_path, source_map, errors);
    let (modifiers, _) = parse_view_modifiers(&lines[1..], base, module_path, source_map, errors);
    ParsedViewChain { head, modifiers }
}

fn parse_view_modifiers(
    lines: &[&str],
    base: usize,
    module_path: Option<&str>,
    source_map: &ViewSourceMap<'_>,
    errors: &mut Vec<ParseError>,
) -> (Vec<ViewModifier>, usize) {
    let mut modifiers = Vec::new();
    let mut part_rejected = false;
    let mut fx_ordinal = 0_u32;
    let mut index = 0;
    while index < lines.len() && is_view_modifier_line(lines[index]) {
        let line = lines[index];
        let error_count = errors.len();
        let rejected_consumed = collect_modifier_lines(&lines[index..]).max(1);
        if let Some((modifier, consumed)) = parse_view_modifier(
            &lines[index..],
            base,
            module_path,
            source_map,
            ViewFxApplicationOrdinal::new(fx_ordinal),
            errors,
        ) {
            if matches!(modifier, ViewModifier::Fx(_)) {
                fx_ordinal = fx_ordinal.saturating_add(1);
            }
            if let ViewModifier::Part(part) = &modifier
                && (part_rejected
                    || modifiers
                        .iter()
                        .any(|existing| matches!(existing, ViewModifier::Part(_))))
            {
                let range = part.modifier_span().range();
                let expected = "one .part(local_name) modifier";
                errors.push(ParseError::new_with_kind(
                    ParseErrorKind::ViewDuplicatePartModifier,
                    TextRange::new(range.start(), range.end()),
                    vec![expected.to_owned()],
                    None,
                    "View expression has more than one `.part(...)` modifier".to_owned(),
                    vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
                ));
                modifiers.retain(|existing| !matches!(existing, ViewModifier::Part(_)));
                part_rejected = true;
            } else {
                modifiers.push(modifier);
            }
            index += consumed.max(1);
        } else {
            if errors.len() == error_count {
                errors.push(simple_error(
                    base,
                    line.len(),
                    "unsupported View modifier",
                    ".label(\"Text\") | .on_click { action.invoke(@action:.name) } | .style(@style:.name)",
                ));
            }
            modifiers.push(ViewModifier::Raw(
                lines[index..index + rejected_consumed].join("\n"),
            ));
            index += rejected_consumed;
        }
    }
    (modifiers, index)
}

fn parse_view_head(
    line: &str,
    module_path: Option<&str>,
    source_map: &ViewSourceMap<'_>,
    errors: &mut Vec<ParseError>,
) -> ViewHead {
    let Some((callee, args_source)) = split_simple_call(line) else {
        return ViewHead::Raw(line.to_owned());
    };
    let args = parse_view_args_recovering(args_source, source_map, errors);
    match callee {
        "Button" => ViewHead::Button {
            label: button_label(&args),
            args: args.clone(),
            id: named_entity_arg(&args, "id").or_else(|| first_entity_arg(&args)),
            enabled: Box::new(named_arg(&args, "enabled").cloned()),
            focusable: named_arg_bool(&args, "focusable").unwrap_or(true),
        },
        other if is_view_container_element(other) => ViewHead::Element {
            callee: callee.to_owned(),
            args,
        },
        "Text" => ViewHead::Text {
            source: first_arg_expr(&args),
            rich: false,
        },
        "RichText" => ViewHead::Text {
            source: first_arg_expr(&args),
            rich: true,
        },
        "Image" => ViewHead::Image {
            source: first_arg_expr(&args),
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
        _ => ViewHead::ViewCall {
            view: parse_view_call_target(callee, module_path, source_map, errors),
            args,
        },
    }
}

fn parse_view_call_target(
    callee: &str,
    module_path: Option<&str>,
    source_map: &ViewSourceMap<'_>,
    errors: &mut Vec<ParseError>,
) -> Expr {
    let range = source_map.mapped_location(callee);
    if callee.starts_with('@') {
        let Some((id, trailing)) = parse_required_id_ref(callee, range.start(), errors) else {
            return Expr::Raw(callee.to_owned());
        };
        if !trailing.trim().is_empty() {
            errors.push(simple_error(
                range.start(),
                range.end() - range.start(),
                "unexpected tokens after nested View reference",
                "Child(...) | @view:.Child(...) | @view:package.Child(...)",
            ));
            return Expr::Raw(callee.to_owned());
        }
        let relative = matches!(id, IdRef::Relative(_) | IdRef::FamilyRelative(_));
        let Some(entity) = normalize_decl_id_ref(id, "view", errors) else {
            return Expr::Raw(callee.to_owned());
        };
        let entity = if relative {
            rebase_family_ref_entity(entity, "view", module_path)
        } else {
            entity
        };
        return Expr::EntityRef(EntityRefSyntax::absolute(entity));
    }
    Expr::EntityRef(EntityRefSyntax::absolute(
        EntityRef::module_scoped_declaration("view", callee, module_path, range),
    ))
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
    source_map: &ViewSourceMap<'_>,
    fx_ordinal: ViewFxApplicationOrdinal,
    errors: &mut Vec<ParseError>,
) -> Option<(ViewModifier, usize)> {
    let line = lines.first()?.trim();
    if line.starts_with(".fx") {
        let consumed = collect_modifier_lines(lines);
        let (source, range) = source_map.lines_source(lines, consumed)?;
        let arguments = call_arg(source.trim(), ".fx")?;
        return parse_view_fx_application(arguments, fx_ordinal, range, base, source_map, errors)
            .map(|application| (ViewModifier::Fx(Box::new(application)), consumed));
    }
    if let Some(value) = call_arg(line, ".style") {
        let value_range = source_map.mapped_location(value);
        let (reference, trailing) =
            parse_view_style_ref(value, value_range.start(), module_path, errors)?;
        if !trailing.trim().is_empty() {
            errors.push(simple_error(
                value_range.start(),
                value_range.end() - value_range.start(),
                "style reference modifier has trailing syntax",
                ".style(@style:.name)",
            ));
        }
        return Some((ViewModifier::Style(ViewStyleModifier::named(reference)), 1));
    }
    if line
        .split_once('{')
        .is_some_and(|(head, _)| head.trim() == ".style")
    {
        let (source, consumed, range) = collect_inline_style_block(lines, ".style", source_map)?;
        let patch = parse_inline_native_style(&source, range, errors);
        return Some((ViewModifier::style_inline(patch), consumed));
    }
    if let Some(part) = call_arg(line, ".part") {
        let range = source_map.mapped_location(lines[0]);
        return part::parse_label(part, line, range, source_map.document, errors)
            .map(|label| (ViewModifier::Part(label), 1));
    }
    if let Some(value) = call_arg(line, ".agent_target")
        && let Some(target) = entity_ref_expr(&source_map.parse_owned_expr(value, errors))
    {
        return Some((ViewModifier::AgentTarget(target), 1));
    }
    if let Some(value) = call_arg(line, ".nav") {
        let range = source_map.mapped_location(lines[0]);
        return Some((
            ViewModifier::Navigation(parse_navigation_modifier(value, range, source_map, errors)?),
            1,
        ));
    }
    if let Some(value) = call_arg(line, ".placeholder") {
        return Some((
            ViewModifier::Placeholder(source_map.parse_owned_expr(value, errors)),
            1,
        ));
    }
    if let Some(value) = call_arg(line, ".label") {
        return Some((
            ViewModifier::Label(source_map.parse_owned_expr(value, errors)),
            1,
        ));
    }
    if let Some(value) = call_arg(line, ".purpose") {
        return Some((
            ViewModifier::Purpose(source_map.parse_owned_expr(value, errors)),
            1,
        ));
    }
    if let Some(value) = call_arg(line, ".enter_key") {
        return Some((
            ViewModifier::EnterKey(source_map.parse_owned_expr(value, errors)),
            1,
        ));
    }
    if let Some(modifier) = view_event_modifier(lines, line, source_map, errors) {
        return Some(modifier);
    }
    if let Some(value) = call_arg(line, ".enabled") {
        return Some((
            ViewModifier::Enabled(source_map.parse_owned_expr(value, errors)),
            1,
        ));
    }
    if let Some(value) = call_arg(line, ".focusable") {
        return Some((parse_view_focusable(value, line, source_map, errors), 1));
    }
    if let Some((name, value)) = view_property_modifier(line) {
        return Some((
            ViewModifier::Property {
                name: name.to_owned(),
                value: source_map.parse_owned_expr(value, errors),
            },
            1,
        ));
    }
    None
}

fn parse_view_focusable(
    value: &str,
    line: &str,
    source_map: &ViewSourceMap<'_>,
    errors: &mut Vec<ParseError>,
) -> ViewModifier {
    if let Expr::Literal(Literal::Bool(value)) = source_map.parse_owned_expr(value, errors) {
        return ViewModifier::Focusable(value);
    }
    let range = source_map.mapped_location(line);
    errors.push(simple_error(
        range.start(),
        range.end() - range.start(),
        "View `.focusable` needs a literal boolean",
        ".focusable(true)",
    ));
    ViewModifier::Raw(line.to_owned())
}

fn parse_view_fx_application(
    source: &str,
    ordinal: ViewFxApplicationOrdinal,
    range: TextRange,
    base: usize,
    source_map: &ViewSourceMap<'_>,
    errors: &mut Vec<ParseError>,
) -> Option<ViewFxApplication> {
    let mut arguments = split_top_level_punctuation(source, ',')
        .into_iter()
        .map(str::trim)
        .filter(|argument| !argument.is_empty());
    let Some(call_source) = arguments.next() else {
        errors.push(simple_error(
            base,
            source.len().max(1),
            "View `.fx` needs an Fx function call",
            ".fx(wave(amplitude = 2px))",
        ));
        return None;
    };
    if split_top_level_binding(call_source).is_some() {
        errors.push(simple_error(
            base,
            call_source.len(),
            "View `.fx` needs its Fx function call before `key`",
            ".fx(wave(amplitude = 2px), key = enemy.id)",
        ));
        return None;
    }

    let call = source_map.parse_owned_expr(call_source, errors);
    let Expr::Call(parsed_call) = &call else {
        errors.push(simple_error(
            base,
            call_source.len(),
            "View `.fx` accepts only a reusable Fx function call",
            ".fx(wave(amplitude = 2px))",
        ));
        return None;
    };
    if parsed_call
        .args()
        .iter()
        .any(|argument| !matches!(argument, CallArg::Named { .. }))
    {
        errors.push(simple_error(
            base,
            call_source.len(),
            "Fx function arguments are named-only",
            "wave(amplitude = 2px, speed = 1.0)",
        ));
        return None;
    }

    let mut key = None;
    for argument in arguments {
        let Some((name, value)) = split_top_level_binding(argument) else {
            errors.push(simple_error(
                base,
                argument.len(),
                "View `.fx` accepts one Fx call and optional named `key`",
                ".fx(wave(), key = enemy.id)",
            ));
            return None;
        };
        if name.trim() != "key" {
            errors.push(simple_error(
                base,
                argument.len(),
                &format!("unknown View `.fx` option `{}`", name.trim()),
                ".fx(wave(), key = enemy.id)",
            ));
            return None;
        }
        if key.is_some() {
            errors.push(simple_error(
                base,
                argument.len(),
                "View `.fx` has more than one `key`",
                ".fx(wave(), key = enemy.id)",
            ));
            return None;
        }
        key = Some(source_map.parse_owned_expr(value.trim(), errors));
    }

    Some(ViewFxApplication::new(call, key, ordinal, range))
}

fn view_event_modifier(
    lines: &[&str],
    line: &str,
    source_map: &ViewSourceMap<'_>,
    errors: &mut Vec<ParseError>,
) -> Option<(ViewModifier, usize)> {
    let (head, name, tail) = view_event_head(line)?;
    if tail.starts_with('(') {
        let value = call_arg(line, head)?;
        return Some((
            view_on_event(name, source_map.parse_owned_expr(value, errors)),
            1,
        ));
    }
    if tail.starts_with('{') {
        let (source, consumed, range) = collect_inline_style_block(lines, head, source_map)?;
        let body = parse_view_callback_body(&source, range, errors);
        return Some((view_on_event(name, body), consumed));
    }
    None
}

fn parse_view_callback_body(source: &str, range: TextRange, errors: &mut Vec<ParseError>) -> Expr {
    match crate::parser::parse_callback_block_expr_body_recovering_at(source, range.start()) {
        Ok(parsed) => {
            for diagnostic in &parsed.diagnostics {
                super::helpers::retain_expr_recovery_diagnostic(diagnostic, errors);
            }
            parsed.expr
        }
        Err(error) => {
            errors.push(ParseError::from_expression(
                &error,
                vec!["View callback expression".to_owned()],
            ));
            Expr::Raw(source.to_owned())
        }
    }
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
        (".x", "x"),
        (".y", "y"),
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
    rebase_family_ref_entity(entity, "style", module_path)
}

fn rebase_family_ref_entity(
    entity: EntityRef,
    family: &str,
    module_path: Option<&str>,
) -> EntityRef {
    let Some(suffix) = entity.body().strip_prefix(&format!("{family}.")) else {
        return entity;
    };
    EntityRef::module_scoped_declaration(family, suffix, module_path, *entity.range())
}

fn parse_navigation_modifier(
    source: &str,
    range: TextRange,
    source_map: &ViewSourceMap<'_>,
    errors: &mut Vec<ParseError>,
) -> Option<ViewNavigationModifier> {
    let arguments = split_top_level_punctuation(source, ',')
        .into_iter()
        .map(str::trim)
        .filter(|argument| !argument.is_empty())
        .collect::<Vec<_>>();
    if arguments.is_empty() {
        errors.push(simple_error(
            range.start(),
            range.end() - range.start(),
            "View `.nav` needs at least one named direction",
            ".nav(right: @button:.apply)",
        ));
        return None;
    }

    let mut edges = Vec::with_capacity(arguments.len());
    let mut invalid = false;
    for argument in arguments {
        let argument_range = source_map.mapped_location(argument);
        let Some((name, value)) = split_top_level_binding(argument)
            .or_else(|| split_top_level_punctuation_once(argument, ':'))
        else {
            errors.push(simple_error(
                argument_range.start(),
                argument_range.end() - argument_range.start(),
                "View `.nav` arguments must name a direction",
                ".nav(right: @button:.apply)",
            ));
            invalid = true;
            continue;
        };
        let Some(direction) = parse_navigation_direction(name) else {
            errors.push(simple_error(
                argument_range.start(),
                argument_range.end() - argument_range.start(),
                &format!("unknown View navigation direction `{}`", name.trim()),
                "up | down | left | right | next | previous",
            ));
            invalid = true;
            continue;
        };
        if edges
            .iter()
            .any(|edge: &ViewNavigationEdge| edge.direction() == direction)
        {
            errors.push(simple_error(
                argument_range.start(),
                argument_range.end() - argument_range.start(),
                &format!("duplicate View navigation direction `{}`", name.trim()),
                "one target per navigation direction",
            ));
            invalid = true;
            continue;
        }
        let value = value.trim();
        if value.is_empty() {
            errors.push(simple_error(
                argument_range.start(),
                argument_range.end() - argument_range.start(),
                "View navigation direction needs a target",
                ".nav(right: @button:.apply)",
            ));
            invalid = true;
            continue;
        }
        let value = source_map.parse_owned_expr(value, errors);
        let Some(target) = parse_navigation_target(&value) else {
            errors.push(simple_error(
                argument_range.start(),
                argument_range.end() - argument_range.start(),
                "invalid View navigation target",
                "entity reference | auto | none | boundary",
            ));
            invalid = true;
            continue;
        };
        edges.push(ViewNavigationEdge::new(direction, target));
    }

    (!invalid).then(|| ViewNavigationModifier::new(edges, range))
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
        ViewHead::ViewCall { view, args } => {
            ViewExpr::ViewCall(ViewCall::new(view, args, chain.modifiers, range))
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

fn parse_view_args_recovering(
    source: &str,
    source_map: &ViewSourceMap<'_>,
    errors: &mut Vec<ParseError>,
) -> Vec<ViewArg> {
    split_top_level_punctuation(source, ',')
        .into_iter()
        .map(str::trim)
        .filter(|arg| !arg.is_empty())
        .map(|arg| {
            match split_top_level_binding(arg)
                .or_else(|| split_top_level_punctuation_once(arg, ':'))
            {
                Some((name, value)) => ViewArg::Named {
                    name: name.trim().to_owned(),
                    value: source_map.parse_owned_expr(value.trim(), errors),
                },
                None => ViewArg::Positional(source_map.parse_owned_expr(arg, errors)),
            }
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
        expr => ViewButtonLabel::Expr(Box::new(expr.clone())),
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
                .or_else(|| projected_action(&parsed))
                .or_else(|| action_invoke_action(&Expr::Raw(body.to_owned()), range))
                .or_else(|| noop_action(&Expr::Raw(body.to_owned())))
                .or_else(|| projected_action(&Expr::Raw(body.to_owned())))
        }
        _ => action_invoke_action(expr, range)
            .or_else(|| noop_action(expr))
            .or_else(|| projected_action(expr)),
    }
}

fn projected_action(expr: &Expr) -> Option<ViewAction> {
    expr.dotted_selector_label()
        .is_some_and(|label| label.split_once('.').is_some())
        .then(|| ViewAction::Projection(expr.clone()))
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
        Expr::Call(call) if is_action_invoke_callee(call.callee()) => {
            action_invoke_call_action(call.args(), range)
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
        Expr::Call(call) => Some(format!(
            "{}({})",
            expr_source(call.callee())?,
            call_args_source(call.args())?
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

fn first_arg_expr(args: &[ViewArg]) -> Expr {
    args.first().map_or_else(
        || parse_expr_lossy("\"\""),
        |arg| match arg {
            ViewArg::Positional(expr) | ViewArg::Named { value: expr, .. } => expr.clone(),
        },
    )
}

fn call_arg<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    source
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix('('))
        .and_then(|rest| rest.strip_suffix(')'))
        .map(str::trim)
}

fn collect_inline_style_block(
    lines: &[&str],
    head_prefix: &str,
    source_map: &ViewSourceMap<'_>,
) -> Option<(String, usize, TextRange)> {
    let consumed = collect_modifier_lines(lines);
    let first = source_map.location(lines.first()?)?;
    let last = source_map.location(lines.get(consumed.saturating_sub(1))?)?;
    let block_range = TextRange::new(first.start(), last.end());
    let block = source_map.source(block_range)?;
    let tail = block.strip_prefix(head_prefix)?;
    let open = head_prefix.len() + tail.find('{')?;
    let close = block.rfind('}')?;
    let body_start = open.saturating_add('{'.len_utf8()).min(close);
    let range = TextRange::new(first.start() + body_start, first.start() + close);
    Some((block[body_start..close].to_owned(), consumed, range))
}
