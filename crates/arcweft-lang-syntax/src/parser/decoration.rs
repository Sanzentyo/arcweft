use crate::{
    ast::{
        common::TextRange,
        decoration::{DecorationItem, DecorationLayer, DecorationParam},
    },
    cst::{find_matching_punctuation, split_leading_ident},
    expr::{Expr, parse_expr},
};

use super::{
    Parser,
    headers::{parse_visibility_prefix, slice_offset},
};

impl Parser<'_> {
    pub(super) fn parse_decoration(&mut self) -> Option<DecorationItem> {
        let doc = self.take_pending_doc();
        let attrs = self.take_pending_attrs();
        let start_line = self.current().clone();
        let event = self.take_brace_block_event();
        if !event.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing decoration declaration",
                ["}"],
                Some(start_line.text().trim()),
                ["insert a closing `}` for the decoration body"],
            );
            return None;
        }

        let original_body_range = event.body_range.clone();
        let original_head = original_body_range
            .as_ref()
            .and_then(|range| range.start.checked_sub('{'.len_utf8()))
            .and_then(|open| self.source.get(start_line.start..open))
            .map(str::to_owned);
        let original_body = original_body_range
            .as_ref()
            .and_then(|range| self.source.get(range.clone()))
            .map(str::to_owned);
        let head_source = original_head.unwrap_or_else(|| event.head.to_string());
        let body_source = original_body.unwrap_or_else(|| event.body.to_string());
        let head = head_source.trim();
        let head_leading = head_source.len() - head_source.trim_start().len();
        let head_base = start_line.start + head_leading;
        let (name, params) = self.parse_decoration_header(head, head_base)?;
        let body_base = original_body_range
            .as_ref()
            .map_or(start_line.end, |range| range.start);
        let layers = self.parse_decoration_layers(&body_source, body_base);

        Some(DecorationItem::new(
            doc,
            attrs,
            name,
            params,
            layers,
            TextRange::new(start_line.start, event.end),
        ))
    }

    fn parse_decoration_header(
        &mut self,
        head: &str,
        head_base: usize,
    ) -> Option<(String, Vec<DecorationParam>)> {
        let (visibility, rest) = parse_visibility_prefix(head);
        if visibility.is_some() {
            let prefix_end = head.find("decoration").unwrap_or(head.len());
            self.push_error(
                TextRange::new(head_base, head_base + prefix_end),
                "decoration declarations are module-local and cannot use `pub`",
                ["decoration"],
                Some(&head[..prefix_end]),
                ["remove `pub`; importable decoration namespaces are not part of this declaration form"],
            );
        }
        let rest = rest.trim_start().strip_prefix("decoration")?;
        let rest = rest.trim_start();
        let Some((name, tail)) = split_leading_ident(rest) else {
            self.push_error(
                TextRange::new(head_base, head_base + head.len()),
                "decoration declaration requires a name",
                ["decoration name(...) { ... }"],
                Some(head),
                ["add a simple identifier before the parameter list"],
            );
            return None;
        };
        let name = name.to_owned();
        let tail = tail.trim_start();
        if !tail.starts_with('(') {
            self.push_error(
                TextRange::new(
                    head_base + slice_offset(head, tail),
                    head_base + slice_offset(head, tail) + tail.len(),
                ),
                "decoration declaration requires a parameter list",
                ["(...)"],
                (!tail.is_empty()).then_some(tail),
                ["add `()` after the decoration name"],
            );
            return None;
        }
        let Some(close) = find_matching_punctuation(tail, 0, '(', ')') else {
            let start = head_base + slice_offset(head, tail);
            self.push_error(
                TextRange::new(start, start + tail.len()),
                "unclosed decoration parameter list",
                [")"],
                Some(tail),
                ["insert a closing `)` before the decoration body"],
            );
            return None;
        };
        let trailing = tail[close + 1..].trim();
        if !trailing.is_empty() {
            let start = head_base + slice_offset(head, &tail[close + 1..]);
            self.push_error(
                TextRange::new(start, start + tail[close + 1..].len()),
                "unexpected text after decoration parameter list",
                ["{"],
                Some(trailing),
                ["move decoration layers into the declaration body"],
            );
        }

        let param_source = &tail[1..close];
        let param_base = head_base + slice_offset(head, param_source);
        let params = self.parse_decoration_params(param_source, param_base);
        Some((name, params))
    }

    fn parse_decoration_params(&mut self, source: &str, base: usize) -> Vec<DecorationParam> {
        let segments = split_top_level_segments(source, ',');
        let mut params = Vec::new();

        for (index, segment) in segments.iter().enumerate() {
            let Some(segment) = segment.trimmed() else {
                if index + 1 != segments.len() {
                    self.push_error(
                        TextRange::new(base + segment.start, base + segment.end),
                        "empty decoration parameter",
                        ["parameter name"],
                        None,
                        ["remove the extra comma or add a parameter"],
                    );
                }
                continue;
            };
            let param_range = TextRange::new(base + segment.start, base + segment.end);
            let (rest, param_source) = segment
                .source
                .strip_prefix("...")
                .map_or((false, segment.source), |tail| (true, tail.trim_start()));
            let assignment = top_level_separator(param_source, '=');
            let name_source = assignment.map_or(param_source, |offset| &param_source[..offset]);
            let name_source = name_source.trim();
            let valid_name = split_leading_ident(name_source)
                .is_some_and(|(name, tail)| name == name_source && tail.is_empty());
            if !valid_name {
                self.push_error(
                    param_range,
                    "decoration parameter must be a simple identifier",
                    ["name", "name = expression", "...custom"],
                    Some(name_source),
                    ["replace the parameter with a simple identifier"],
                );
                continue;
            }
            let default_tail = assignment.map(|offset| &param_source[offset + '='.len_utf8()..]);
            let default_source = default_tail
                .map(str::trim)
                .filter(|source| !source.is_empty());
            if assignment.is_some() && default_source.is_none() {
                self.push_error(
                    param_range,
                    "decoration parameter default requires an expression",
                    ["expression"],
                    Some(segment.source),
                    ["add a default expression after `=`"],
                );
            }
            let default_range = default_source.map(|default_source| {
                let param_offset = slice_offset(segment.source, param_source);
                let default_tail = default_tail.unwrap_or_default();
                let default_offset = slice_offset(param_source, default_tail);
                let leading = default_tail.len() - default_tail.trim_start().len();
                let relative = param_offset + default_offset + leading;
                TextRange::new(
                    base + segment.start + relative,
                    base + segment.start + relative + default_source.len(),
                )
            });
            let default = default_source.map(|default_source| {
                parse_expr(default_source).unwrap_or_else(|error| {
                    let range = default_range.unwrap_or(param_range);
                    self.push_error(
                        range,
                        &format!("invalid decoration parameter default: {error}"),
                        ["expression"],
                        Some(default_source),
                        ["replace the default with a valid expression"],
                    );
                    Expr::Raw(default_source.to_owned())
                })
            });

            params.push(DecorationParam::new(
                name_source.to_owned(),
                default,
                default_source.map(str::to_owned),
                rest,
                param_range,
                default_range,
            ));
        }
        params
    }

    fn parse_decoration_layers(&mut self, source: &str, base: usize) -> Vec<DecorationLayer> {
        decoration_layer_segments(source)
            .into_iter()
            .filter_map(|segment| {
                let segment = segment.trimmed()?;
                let range = TextRange::new(base + segment.start, base + segment.end);
                let expr = parse_expr(segment.source).unwrap_or_else(|error| {
                    self.push_error(
                        range,
                        &format!("invalid decoration layer: {error}"),
                        ["builder call"],
                        Some(segment.source),
                        ["use a builder call such as `strong()` or `effect(.wave, amp=2px)`"],
                    );
                    Expr::Raw(segment.source.to_owned())
                });
                Some(DecorationLayer::new(expr, segment.source.to_owned(), range))
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
struct SourceSegment<'a> {
    source: &'a str,
    start: usize,
    end: usize,
}

impl SourceSegment<'_> {
    fn trimmed(self) -> Option<Self> {
        let leading = self.source.len() - self.source.trim_start().len();
        let source = self.source.trim();
        if source.is_empty() {
            return None;
        }
        let start = self.start + leading;
        Some(Self {
            source,
            start,
            end: start + source.len(),
        })
    }
}

fn split_top_level_segments(source: &str, separator: char) -> Vec<SourceSegment<'_>> {
    let mut segments = Vec::new();
    let mut start = 0;
    let mut punctuation = PunctuationState::default();
    for (index, ch) in source.char_indices() {
        if punctuation.consume(ch) {
            continue;
        }
        if ch == separator && punctuation.at_top_level() {
            segments.push(SourceSegment {
                source: &source[start..index],
                start,
                end: index,
            });
            start = index + ch.len_utf8();
        }
    }
    segments.push(SourceSegment {
        source: &source[start..],
        start,
        end: source.len(),
    });
    segments
}

fn top_level_separator(source: &str, separator: char) -> Option<usize> {
    let mut punctuation = PunctuationState::default();
    source.char_indices().find_map(|(index, ch)| {
        if punctuation.consume(ch) {
            return None;
        }
        (ch == separator && punctuation.at_top_level()).then_some(index)
    })
}

fn decoration_layer_segments(source: &str) -> Vec<SourceSegment<'_>> {
    let mut segments = Vec::new();
    let mut start = 0;
    let mut punctuation = PunctuationState::default();
    let mut chars = source.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if punctuation.quote.is_none()
            && ch == '/'
            && chars.peek().is_some_and(|(_, next)| *next == '/')
        {
            push_layer_segment(source, start, index, &mut segments);
            let mut ended_at_newline = false;
            for (comment_index, comment_char) in chars.by_ref() {
                if comment_char == '\n' {
                    start = comment_index + comment_char.len_utf8();
                    ended_at_newline = true;
                    break;
                }
            }
            if !ended_at_newline {
                start = source.len();
            }
            continue;
        }
        if punctuation.consume(ch) {
            continue;
        }
        if punctuation.at_top_level() && matches!(ch, ';' | '\n' | '\r') {
            push_layer_segment(source, start, index, &mut segments);
            start = index + ch.len_utf8();
        }
    }
    push_layer_segment(source, start, source.len(), &mut segments);
    segments
}

fn push_layer_segment<'a>(
    source: &'a str,
    start: usize,
    end: usize,
    segments: &mut Vec<SourceSegment<'a>>,
) {
    if start <= end {
        segments.push(SourceSegment {
            source: &source[start..end],
            start,
            end,
        });
    }
}

#[derive(Default)]
struct PunctuationState {
    paren: u32,
    bracket: u32,
    brace: u32,
    quote: Option<char>,
    escaped: bool,
}

impl PunctuationState {
    fn consume(&mut self, ch: char) -> bool {
        if let Some(quote) = self.quote {
            if self.escaped {
                self.escaped = false;
            } else if ch == '\\' {
                self.escaped = true;
            } else if ch == quote {
                self.quote = None;
            }
            return true;
        }
        if matches!(ch, '"' | '\'') {
            self.quote = Some(ch);
            return true;
        }
        match ch {
            '(' => self.paren += 1,
            ')' => self.paren = self.paren.saturating_sub(1),
            '[' => self.bracket += 1,
            ']' => self.bracket = self.bracket.saturating_sub(1),
            '{' => self.brace += 1,
            '}' => self.brace = self.brace.saturating_sub(1),
            _ => return false,
        }
        true
    }

    const fn at_top_level(&self) -> bool {
        self.quote.is_none() && self.paren == 0 && self.bracket == 0 && self.brace == 0
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::items::Item,
        expr::{CallArg, Expr, Literal},
        parser::parse_source,
    };

    #[test]
    fn parses_parameterized_decoration_declaration() {
        let parsed = parse_source(
            r##"
#[ui]
decoration warning(
    accent = "#ff4050",
    amplitude = 2px,
    required,
    ...custom,
) {
    strong()
    color(value=accent);
    // renderer-specific parameters remain explicit
    effect(.wave, amp=amplitude, custom...)
}
"##,
        );

        assert_eq!(parsed.errors(), &[]);
        let [Item::Decoration(item)] = parsed.typed_tree().items() else {
            panic!("expected one decoration item");
        };
        assert_eq!(item.name(), "warning");
        assert_eq!(item.attrs().len(), 1);
        assert_eq!(item.params().len(), 4);
        assert!(matches!(
            item.params()[0].default(),
            Some(Expr::Literal(Literal::String(value))) if value == "#ff4050"
        ));
        assert_eq!(item.params()[1].default_source(), Some("2px"));
        assert!(item.params()[2].default().is_none());
        assert!(item.params()[3].is_rest());
        assert_eq!(item.layers().len(), 3);
        assert!(matches!(item.layers()[0].expr(), Expr::Call { .. }));
        let Expr::Call { args, .. } = item.layers()[2].expr() else {
            panic!("effect layer is a call");
        };
        assert!(matches!(args.last(), Some(CallArg::Spread { .. })));
        assert!(item.params()[0].default_range().is_some());
        assert!(
            item.layers()
                .iter()
                .all(|layer| layer.range().start() < layer.range().end())
        );
    }

    #[test]
    fn parses_semicolon_separated_layers_on_one_line() {
        let parsed = parse_source(
            "decoration warning(accent = \"#ff4050\") { strong(); color(value=accent); effect(.wave) }",
        );

        assert_eq!(parsed.errors(), &[]);
        let [Item::Decoration(item)] = parsed.typed_tree().items() else {
            panic!("expected one decoration item");
        };
        assert_eq!(item.layers().len(), 3);
    }

    #[test]
    fn default_range_uses_the_expression_after_the_assignment() {
        let source = "decoration same(value = value) { strong() }";
        let parsed = parse_source(source);

        assert_eq!(parsed.errors(), &[]);
        let [Item::Decoration(item)] = parsed.typed_tree().items() else {
            panic!("expected one decoration item");
        };
        let range = item.params()[0]
            .default_range()
            .expect("default expression range");
        assert_eq!(&source[range.as_range()], "value");
    }

    #[test]
    fn multiline_crlf_ranges_slice_the_original_source() {
        let source = "decoration warning(\r\n    accent = \"#ff4050\",\r\n    amount = 2px,\r\n) {\r\n    strong()\r\n    effect(.wave, amp=amount)\r\n}\r\n";
        let parsed = parse_source(source);

        assert_eq!(parsed.errors(), &[]);
        let [Item::Decoration(item)] = parsed.typed_tree().items() else {
            panic!("expected decoration item");
        };
        let default = item.params()[0].default_range().expect("default range");
        assert_eq!(&source[default.as_range()], "\"#ff4050\"");
        assert_eq!(&source[item.layers()[0].range().as_range()], "strong()");
        assert_eq!(
            &source[item.layers()[1].range().as_range()],
            "effect(.wave, amp=amount)"
        );
        assert_eq!(&source[item.range().as_range()], source.trim_end());
    }

    #[test]
    fn reports_malformed_decoration_parameter_syntax() {
        let parsed = parse_source(
            r"
decoration warning(first =, invalid-name) {
    strong()
}
",
        );

        assert!(
            parsed
                .errors()
                .iter()
                .any(|error| error.message().contains("default requires"))
        );
        assert!(
            parsed
                .errors()
                .iter()
                .any(|error| error.message().contains("simple identifier"))
        );
    }

    #[test]
    fn rejects_public_visibility_for_module_local_decorations() {
        let parsed = parse_source("pub decoration warning() { strong() }");

        assert!(parsed.errors().iter().any(|error| {
            error
                .message()
                .contains("module-local and cannot use `pub`")
        }));
        assert!(matches!(
            parsed.typed_tree().items(),
            [Item::Decoration(item)] if item.name() == "warning"
        ));
    }
}
