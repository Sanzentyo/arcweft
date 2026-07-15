use crate::ast::common::TextRange;
use crate::ast::items::{HookInit, HookItem};
use crate::cst::is_identifier;
use crate::expr::parse_expr;
use crate::types::parse_type_ref;

use super::headers::{
    parse_required_decl_entity_ref_without_name_marker, parse_required_entity_ref,
    parse_visibility_prefix,
};
use super::helpers::trimmed_nonempty_lines_with_offsets;
use super::{Parser, parse_stmt_lines, split_comma_args};

#[derive(Default)]
struct ParsedHookHeaders {
    target: Option<String>,
    phase: Option<String>,
    when: Option<crate::expr::Expr>,
    priority: Option<i64>,
    once: bool,
    effects: Option<Vec<crate::expr::Expr>>,
}

impl Parser<'_> {
    pub(super) fn parse_hook(&mut self) -> Option<HookItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing hook",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the hook body"],
            );
            return None;
        }
        let header_lines = trimmed_nonempty_lines_with_offsets(&head);
        let (first, _) = *header_lines.first()?;
        let (visibility, after_visibility) = parse_visibility_prefix(first);
        let after_hook = after_visibility
            .trim_start()
            .strip_prefix("hook")?
            .trim_start();
        let (id, _) = parse_required_decl_entity_ref_without_name_marker(
            after_hook,
            "hook",
            "hook declaration marker needs an explicit hook name suffix",
            start_line.start,
            &mut self.errors,
        )?;
        let mut headers = ParsedHookHeaders::default();
        for &(line, offset) in header_lines.iter().skip(1) {
            let base = start_line.start + offset;
            self.parse_hook_header_line(line, base, &mut headers);
        }
        self.require_hook_target_and_phase(&headers, first, &start_line);
        let body_statements = parse_stmt_lines(&body);

        Some(HookItem::new(HookInit {
            visibility,
            id,
            target: headers.target.unwrap_or_default(),
            phase: headers.phase.unwrap_or_default(),
            when: headers.when,
            priority: headers.priority,
            once: headers.once,
            effects: headers.effects.unwrap_or_default(),
            body: body.into_owned(),
            body_statements,
            range: TextRange::new(start_line.start, end),
        }))
    }

    fn parse_hook_header_line(&mut self, line: &str, base: usize, headers: &mut ParsedHookHeaders) {
        let keyword_end = line.find(char::is_whitespace).unwrap_or(line.len());
        let keyword = &line[..keyword_end];
        let value = line[keyword_end..].trim_start();
        match keyword {
            "on" if is_current_hook_target(value) => {
                if headers.target.replace(value.to_owned()).is_some() {
                    self.push_duplicate_hook_header_error("on", base, line);
                }
            }
            "phase" if is_identifier(value) => {
                if headers.phase.replace(value.to_owned()).is_some() {
                    self.push_duplicate_hook_header_error("phase", base, line);
                }
            }
            "when" => match parse_expr(value) {
                Ok(expr) if headers.when.is_none() => headers.when = Some(expr),
                Ok(_) => self.push_duplicate_hook_header_error("when", base, line),
                Err(_) => self.push_invalid_hook_header_error(base, line),
            },
            "priority" => match value.parse::<i32>() {
                Ok(value) if headers.priority.replace(i64::from(value)).is_none() => {}
                Ok(_) => self.push_duplicate_hook_header_error("priority", base, line),
                Err(_) => self.push_invalid_hook_header_error(base, line),
            },
            "once" if value.is_empty() => {
                if headers.once {
                    self.push_duplicate_hook_header_error("once", base, line);
                } else {
                    headers.once = true;
                }
            }
            "effects" if headers.effects.is_none() => {
                match split_comma_args(value)
                    .into_iter()
                    .map(parse_expr)
                    .collect::<Result<Vec<_>, _>>()
                {
                    Ok(parsed) if !parsed.is_empty() => headers.effects = Some(parsed),
                    Ok(_) | Err(_) => self.push_invalid_hook_header_error(base, line),
                }
            }
            "effects" => self.push_duplicate_hook_header_error("effects", base, line),
            "on" | "phase" | "once" => self.push_invalid_hook_header_error(base, line),
            _ => self.push_unknown_hook_header_error(base, line),
        }
    }

    fn require_hook_target_and_phase(
        &mut self,
        headers: &ParsedHookHeaders,
        declaration: &str,
        line: &super::CstLine<'_>,
    ) {
        if headers.target.is_none() {
            self.push_error(
                TextRange::new(line.start, line.end),
                "hook declaration requires a target header",
                ["on HookTargetExpr"],
                Some(declaration),
                ["add one current `on` header"],
            );
        }
        if headers.phase.is_none() {
            self.push_error(
                TextRange::new(line.start, line.end),
                "hook declaration requires a phase header",
                ["phase HookPhase"],
                Some(declaration),
                ["add one current `phase` header"],
            );
        }
    }

    fn push_duplicate_hook_header_error(&mut self, name: &str, base: usize, line: &str) {
        self.push_error(
            TextRange::new(base, base + line.len()),
            &format!("duplicate hook `{name}` header"),
            [name],
            Some(line),
            ["keep only one header with this name"],
        );
    }

    fn push_invalid_hook_header_error(&mut self, base: usize, line: &str) {
        self.push_error(
            TextRange::new(base, base + line.len()),
            "invalid hook header",
            CURRENT_HOOK_HEADERS,
            Some(line),
            ["use a valid current hook header"],
        );
    }

    fn push_unknown_hook_header_error(&mut self, base: usize, line: &str) {
        self.push_error(
            TextRange::new(base, base + line.len()),
            "unknown hook header",
            CURRENT_HOOK_HEADERS,
            Some(line),
            ["remove the unknown header or use a current hook header"],
        );
    }
}

const CURRENT_HOOK_HEADERS: [&str; 6] = [
    "on HookTargetExpr",
    "phase HookPhase",
    "when expr",
    "priority i32",
    "once",
    "effects expr, ...",
];

fn is_current_hook_target(source: &str) -> bool {
    let source = source.trim();
    if let Some(entity) = source.strip_prefix("signal ").map(str::trim) {
        return is_complete_entity_ref(entity);
    }
    if let Some(path) = source.strip_prefix("state ").map(str::trim) {
        let Some(path) = path.strip_prefix('.') else {
            return false;
        };
        return !path.is_empty() && parse_expr(&format!("state.{path}")).is_ok();
    }
    if let Some(query) = source.strip_prefix("query ").map(str::trim) {
        let (ty, predicate) = query
            .split_once(" where ")
            .map_or((query, None), |(ty, predicate)| (ty, Some(predicate)));
        return parse_type_ref(ty.trim()).is_ok()
            && predicate.is_none_or(|predicate| parse_expr(predicate.trim()).is_ok());
    }
    is_complete_entity_ref(source)
}

fn is_complete_entity_ref(source: &str) -> bool {
    let mut errors = Vec::new();
    parse_required_entity_ref(source, 0, &mut errors)
        .is_some_and(|(_, rest)| rest.trim().is_empty())
        && errors.is_empty()
}
