use crate::ast::{DialogueTag, DialogueToken, LineMark};
use crate::expr::{Expr, Literal, parse_expr};

/// Parses dialogue-text mode into tokens.
///
/// This tokenizer is deliberately permissive: malformed tags are kept as text
/// so the higher-level parser can continue and attach diagnostics to the
/// surrounding line.
pub fn parse_dialogue_tokens(source: &str) -> Vec<DialogueToken> {
    let mut tokens = Vec::new();
    let mut text = String::new();
    let mut chars = source.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        match ch {
            '\\' => {
                if let Some((_, escaped)) = chars.next() {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(DialogueToken::Escape(escaped));
                } else {
                    text.push(ch);
                }
            }
            '｜' => {
                if let Some((ruby_token, consumed_to)) = parse_natural_ruby(source, index) {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(ruby_token);
                    while chars
                        .peek()
                        .is_some_and(|(offset, _)| *offset < consumed_to)
                    {
                        let _ = chars.next();
                    }
                } else {
                    text.push(ch);
                }
            }
            '#' if chars.peek().is_some_and(|(_, next)| *next == '[') => {
                let _ = chars.next();
                if let Some((expr, consumed_to)) = take_balanced_bracket(source, index + 2) {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(parse_dialogue_expr_token(&expr));
                    while chars
                        .peek()
                        .is_some_and(|(offset, _)| *offset < consumed_to)
                    {
                        let _ = chars.next();
                    }
                } else {
                    text.push_str("#[");
                }
            }
            '[' => {
                if let Some((ruby, consumed_to)) = parse_bracket_ruby(source, index) {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(ruby);
                    while chars
                        .peek()
                        .is_some_and(|(offset, _)| *offset < consumed_to)
                    {
                        let _ = chars.next();
                    }
                    continue;
                }
                if let Some((raw, consumed_to)) = parse_raw_span(source, index) {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(DialogueToken::Raw(raw));
                    while chars
                        .peek()
                        .is_some_and(|(offset, _)| *offset < consumed_to)
                    {
                        let _ = chars.next();
                    }
                    continue;
                }
                if let Some((tag, consumed_to)) = parse_tag(source, index + 1) {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(tag);
                    while chars
                        .peek()
                        .is_some_and(|(offset, _)| *offset < consumed_to)
                    {
                        let _ = chars.next();
                    }
                } else {
                    text.push(ch);
                }
            }
            _ => text.push(ch),
        }
    }

    flush_text(&mut text, &mut tokens);
    tokens
}

fn flush_text(text: &mut String, tokens: &mut Vec<DialogueToken>) {
    if !text.is_empty() {
        tokens.push(DialogueToken::Text(core::mem::take(text)));
    }
}

fn parse_natural_ruby(source: &str, start: usize) -> Option<(DialogueToken, usize)> {
    let after_marker = start + '｜'.len_utf8();
    let tail = source.get(after_marker..)?;
    let open_relative = tail.find('《')?;
    let base = &tail[..open_relative];
    if base.is_empty() {
        return None;
    }
    let ruby_start = after_marker + open_relative + '《'.len_utf8();
    let ruby_tail = source.get(ruby_start..)?;
    let close_relative = ruby_tail.find('》')?;
    let ruby = &ruby_tail[..close_relative];
    if ruby.is_empty() {
        return None;
    }
    let consumed_to = ruby_start + close_relative + '》'.len_utf8();
    Some((
        DialogueToken::Ruby {
            base: base.to_owned(),
            ruby: ruby.to_owned(),
        },
        consumed_to,
    ))
}

fn parse_bracket_ruby(source: &str, start: usize) -> Option<(DialogueToken, usize)> {
    let after_open = source
        .get(start..)?
        .find(']')
        .map(|close| start + close + 1)?;
    let inside = source.get(start + 1..after_open - 1)?.trim();
    let attrs = inside.strip_prefix("ruby")?.trim();
    let ruby = parse_ruby_attr(attrs)?;
    let tail = source.get(after_open..)?;
    let close_relative = tail.find("[/ruby]")?;
    let base_end = after_open + close_relative;
    let base = source.get(after_open..base_end)?.trim();
    if base.is_empty() {
        return None;
    }
    Some((
        DialogueToken::Ruby {
            base: base.to_owned(),
            ruby,
        },
        base_end + "[/ruby]".len(),
    ))
}

fn parse_ruby_attr(attrs: &str) -> Option<String> {
    let value = attrs.trim().strip_prefix("rt")?.trim_start();
    let value = value.strip_prefix('=')?.trim_start();
    let quoted = value.strip_prefix('"')?;
    let end = quoted.find('"')?;
    Some(quoted[..end].to_owned())
}

fn parse_raw_span(source: &str, start: usize) -> Option<(String, usize)> {
    let raw_body_start = start + "[raw]".len();
    if !source.get(start..)?.starts_with("[raw]") {
        return None;
    }
    let tail = source.get(raw_body_start..)?;
    let close_relative = tail.find("[/raw]")?;
    let raw_body_end = raw_body_start + close_relative;
    let consumed_to = raw_body_end + "[/raw]".len();
    Some((source[raw_body_start..raw_body_end].to_owned(), consumed_to))
}

fn take_balanced_bracket(source: &str, start: usize) -> Option<(String, usize)> {
    let mut depth = 1_u32;
    for (relative, ch) in source.get(start..)?.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    let cursor = start + relative;
                    return Some((source[start..cursor].to_owned(), cursor + 1));
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_tag(source: &str, start: usize) -> Option<(DialogueToken, usize)> {
    let close_relative = source.get(start..)?.find(']')?;
    let inside = &source[start..start + close_relative];
    let consumed_to = start + close_relative + 1;
    if let Some(name) = inside.strip_prefix('/') {
        return Some((DialogueToken::EndTag(name.trim().to_owned()), consumed_to));
    }

    let trimmed = inside.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or_default().to_owned();
    let attrs = parts.next().unwrap_or_default().trim().to_owned();
    if name == "mark" && !attrs.is_empty() {
        return Some((DialogueToken::Mark(LineMark::new(attrs)), consumed_to));
    }
    Some((
        DialogueToken::Tag(DialogueTag::new(name, attrs)),
        consumed_to,
    ))
}

fn parse_dialogue_expr_lossy(source: &str) -> Expr {
    parse_expr(source).unwrap_or_else(|_| Expr::Raw(source.to_owned()))
}

fn parse_dialogue_expr_token(source: &str) -> DialogueToken {
    let expr = parse_dialogue_expr_lossy(source);
    function_ruby_token(&expr).unwrap_or(DialogueToken::Expr(expr))
}

fn function_ruby_token(expr: &Expr) -> Option<DialogueToken> {
    let Expr::Call { callee, args } = expr else {
        return None;
    };
    if !matches!(callee.as_ref(), Expr::Path(path) if path == "ruby") {
        return None;
    }
    let [
        Expr::Literal(Literal::String(base)),
        Expr::Literal(Literal::String(ruby)),
    ] = args.as_slice()
    else {
        return None;
    };
    Some(DialogueToken::Ruby {
        base: base.to_owned(),
        ruby: ruby.to_owned(),
    })
}
