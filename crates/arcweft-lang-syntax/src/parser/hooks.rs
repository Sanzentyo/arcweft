use crate::ast::common::TextRange;
use crate::ast::items::{HookInit, HookItem};

use super::{
    Parser, find_header_value, parse_expr_lossy,
    parse_required_decl_entity_ref_without_name_marker, parse_stmt_lines, parse_visibility_prefix,
    split_comma_args,
};

impl Parser {
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
        let header_lines: Vec<_> = head
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        self.reject_old_hook_header_syntax(&header_lines, start_line.start);
        let first = header_lines.first()?;
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
        let target = find_header_value(&header_lines, "on ");
        let phase = find_header_value(&header_lines, "phase ");
        let mut when = None;
        let mut priority = None;
        let mut once = false;
        let mut effects = Vec::new();
        for line in header_lines.iter().skip(1) {
            if let Some(expr) = line.strip_prefix("when ").map(str::trim) {
                if when.replace(parse_expr_lossy(expr)).is_some() {
                    self.push_duplicate_hook_header_error("when", start_line.start, line);
                }
            } else if let Some(value) = line.strip_prefix("priority ").map(str::trim) {
                match value.parse::<i64>() {
                    Ok(value) if priority.replace(value).is_none() => {}
                    Ok(_) => {
                        self.push_duplicate_hook_header_error("priority", start_line.start, line);
                    }
                    Err(_) => self.push_error(
                        TextRange::new(start_line.start, start_line.start + line.len()),
                        "hook priority must be an integer",
                        ["priority 0"],
                        Some(line),
                        ["write priority as a signed integer"],
                    ),
                }
            } else if let Some(value) = line.strip_prefix("once").map(str::trim) {
                if once {
                    self.push_duplicate_hook_header_error("once", start_line.start, line);
                }
                once = true;
                if !value.is_empty() && value != "true" {
                    self.push_error(
                        TextRange::new(start_line.start, start_line.start + line.len()),
                        "hook `once` does not take a value in this grammar",
                        ["once"],
                        Some(line),
                        ["remove the value after `once`"],
                    );
                }
            } else if let Some(values) = line.strip_prefix("effects ").map(str::trim) {
                effects.extend(split_comma_args(values).into_iter().map(parse_expr_lossy));
            } else if line.starts_with("check ") {
                self.push_error(
                    TextRange::new(start_line.start, start_line.start + line.len()),
                    "`check` is not valid hook condition syntax",
                    ["when expr"],
                    Some(line),
                    ["write hook conditions with `when`"],
                );
            }
        }
        let body_statements = parse_stmt_lines(&body);

        Some(HookItem::new(HookInit {
            visibility,
            id,
            target,
            phase,
            when,
            priority,
            once,
            effects,
            body,
            body_statements,
            range: TextRange::new(start_line.start, end),
        }))
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

    fn reject_old_hook_header_syntax(&mut self, header_lines: &[&str], base: usize) {
        for line in header_lines {
            if line.starts_with("for ") {
                self.push_error(
                    TextRange::new(base, base + line.len()),
                    "`for` is not valid hook target syntax",
                    ["on #target"],
                    Some(line),
                    ["write the hook target as `on #target`"],
                );
            } else if line.starts_with("phase =") {
                self.push_error(
                    TextRange::new(base, base + line.len()),
                    "`phase =` is not valid hook phase syntax",
                    ["phase PhaseName"],
                    Some(line),
                    ["write the hook phase without `=`"],
                );
            } else if line.starts_with("on input target") {
                self.push_error(
                    TextRange::new(base, base + line.len()),
                    "`on input target` is not valid hook input syntax",
                    ["phase InputTarget", "check on input EventKind"],
                    Some(line),
                    ["split input hooks into `phase InputTarget` and `check on input EventKind`"],
                );
            }
        }
    }
}
