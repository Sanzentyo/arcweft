//! One-pass lexer and root event stream for the staged document grammar.

#![allow(
    dead_code,
    reason = "the shadow document parser remains private until the atomic syntax switch"
)]

use arcweft_source::{SourceDocument, SourceRange};

use crate::grammar::build::{GrammarBuild, GrammarBuildError, build_grammar};
use crate::grammar::event::SyntaxEvent;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LexToken {
    kind: SyntaxKind,
    range: SourceRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BlockCommentKind {
    Ordinary,
    Documentation,
}

struct DocumentLexer<'a> {
    source: &'a str,
    cursor: usize,
    block_comment: Option<BlockCommentKind>,
}

impl<'a> DocumentLexer<'a> {
    const fn new(source: &'a str) -> Self {
        Self {
            source,
            cursor: 0,
            block_comment: None,
        }
    }

    fn lex(mut self) -> Box<[LexToken]> {
        let mut tokens = Vec::new();
        while let Some(token) = self.next_token() {
            tokens.push(token);
        }
        tokens.into_boxed_slice()
    }

    fn next_token(&mut self) -> Option<LexToken> {
        if self.cursor == self.source.len() {
            return None;
        }
        let start = self.cursor;
        let rest = &self.source[start..];
        let (kind, len) = if let Some(comment) = self.block_comment {
            self.block_comment_token(rest, comment)
        } else {
            self.regular_token(rest)?
        };
        self.cursor += len;
        Some(LexToken {
            kind,
            range: SourceRange::new(start, self.cursor),
        })
    }

    fn regular_token(&mut self, source: &str) -> Option<(SyntaxKind, usize)> {
        if let Some(len) = newline_len(source) {
            return Some((SyntaxKind::NewlineToken, len));
        }
        let first = source.chars().next()?;
        if first.is_whitespace() {
            return Some((
                SyntaxKind::WhitespaceToken,
                take_while(source, |character| {
                    character.is_whitespace() && !matches!(character, '\r' | '\n')
                }),
            ));
        }
        if source.starts_with("///") || source.starts_with("//!") {
            return Some((SyntaxKind::DocCommentToken, take_until_newline(source)));
        }
        if source.starts_with("//") {
            return Some((SyntaxKind::CommentToken, take_until_newline(source)));
        }
        if source.starts_with("/**") || source.starts_with("/*!") {
            self.block_comment = Some(BlockCommentKind::Documentation);
            return Some(self.block_comment_token(source, BlockCommentKind::Documentation));
        }
        if source.starts_with("/*") {
            self.block_comment = Some(BlockCommentKind::Ordinary);
            return Some(self.block_comment_token(source, BlockCommentKind::Ordinary));
        }
        if let Some(len) = raw_string_len(source) {
            return Some((SyntaxKind::RawStringToken, len));
        }
        if first == '"' {
            return Some((SyntaxKind::StringToken, quoted_token(source, '"').0));
        }
        if first == '\'' {
            return Some(character_or_lifetime(source));
        }
        if first == '@'
            && let Some(len) = entity_reference_len(source)
        {
            return Some((SyntaxKind::EntityReferenceToken, len));
        }
        if is_identifier_start(first) {
            let len = take_while(source, is_identifier_continue);
            let spelling = &source[..len];
            return Some((
                if is_keyword(spelling) {
                    SyntaxKind::KeywordToken
                } else {
                    SyntaxKind::IdentifierToken
                },
                len,
            ));
        }
        if first.is_ascii_digit() {
            return Some((SyntaxKind::NumberToken, number_len(source)));
        }
        if let Some(len) = punctuation_len(source) {
            return Some((SyntaxKind::PunctuationToken, len));
        }
        Some((SyntaxKind::TextToken, first.len_utf8()))
    }

    fn block_comment_token(
        &mut self,
        source: &str,
        comment: BlockCommentKind,
    ) -> (SyntaxKind, usize) {
        if let Some(len) = newline_len(source) {
            return (SyntaxKind::NewlineToken, len);
        }
        let close = source.find("*/").map(|index| index + 2);
        let newline = source.find(['\r', '\n']);
        let len = match (close, newline) {
            (Some(close), Some(newline)) if close <= newline => {
                self.block_comment = None;
                close
            }
            (Some(close), None) => {
                self.block_comment = None;
                close
            }
            (_, Some(newline)) => newline,
            (None, None) => source.len(),
        };
        (
            match comment {
                BlockCommentKind::Ordinary => SyntaxKind::CommentToken,
                BlockCommentKind::Documentation => SyntaxKind::DocCommentToken,
            },
            len,
        )
    }
}

/// Builds the private lossless root tree without allocating syntax identity.
fn parse_shadow_document(document: &SourceDocument) -> Result<GrammarBuild, GrammarBuildError> {
    let tokens = DocumentLexer::new(document.text()).lex();
    let mut events = Vec::with_capacity(tokens.len() + 8);
    events.push(SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root));
    events.push(SyntaxEvent::start(
        SyntaxKind::ItemList,
        SyntaxRole::Element(0),
    ));
    emit_logical_lines(document.text(), &tokens, &mut events)?;
    events.push(SyntaxEvent::token(
        SyntaxKind::EofToken,
        SourceRange::new(document.text().len(), document.text().len()),
    ));
    events.push(SyntaxEvent::FinishNode);
    events.push(SyntaxEvent::FinishNode);
    build_grammar(document, &events)
}

fn emit_logical_lines(
    source: &str,
    tokens: &[LexToken],
    events: &mut Vec<SyntaxEvent>,
) -> Result<(), GrammarBuildError> {
    let mut start = 0;
    let mut delimiter_depth = 0_usize;
    let mut ordinal = 0_u32;
    for (index, token) in tokens.iter().enumerate() {
        delimiter_depth = delimiter_depth_after(source, *token, delimiter_depth);
        if token.kind == SyntaxKind::NewlineToken && delimiter_depth == 0 {
            emit_logical_line(source, &tokens[start..=index], ordinal, events);
            start = index + 1;
            ordinal = ordinal
                .checked_add(1)
                .ok_or(GrammarBuildError::ChildIndexExhausted)?;
        }
    }
    if start < tokens.len() {
        emit_logical_line(source, &tokens[start..], ordinal, events);
    }
    Ok(())
}

fn emit_logical_line(
    source: &str,
    tokens: &[LexToken],
    ordinal: u32,
    events: &mut Vec<SyntaxEvent>,
) {
    events.push(SyntaxEvent::start(
        SyntaxKind::LogicalLine,
        SyntaxRole::Element(ordinal),
    ));
    let item = classify_top_level_item(source, tokens);
    if let Some(kind) = item {
        events.push(SyntaxEvent::start(kind, SyntaxRole::Element(ordinal)));
    }
    events.extend(
        tokens
            .iter()
            .map(|token| SyntaxEvent::token(token.kind, token.range)),
    );
    if item.is_some() {
        events.push(SyntaxEvent::FinishNode);
    }
    events.push(SyntaxEvent::FinishNode);
}

fn delimiter_depth_after(source: &str, token: LexToken, depth: usize) -> usize {
    if token.kind != SyntaxKind::PunctuationToken {
        return depth;
    }
    match &source[token.range.as_range()] {
        "(" | "[" | "{" => depth + 1,
        ")" | "]" | "}" => depth.saturating_sub(1),
        _ => depth,
    }
}

fn classify_top_level_item(source: &str, tokens: &[LexToken]) -> Option<SyntaxKind> {
    let significant = tokens.iter().filter(|token| {
        !matches!(
            token.kind,
            SyntaxKind::WhitespaceToken
                | SyntaxKind::NewlineToken
                | SyntaxKind::CommentToken
                | SyntaxKind::DocCommentToken
        )
    });
    let spellings = significant
        .clone()
        .filter(|token| token.kind == SyntaxKind::KeywordToken)
        .map(|token| &source[token.range.as_range()])
        .collect::<Vec<_>>();
    let first = significant.clone().next()?;
    let first_text = &source[first.range.as_range()];
    if first_text == "#" {
        return Some(
            significant
                .clone()
                .nth(1)
                .filter(|token| &source[token.range.as_range()] == "!")
                .map_or(SyntaxKind::OuterAttribute, |_| SyntaxKind::InnerAttribute),
        );
    }
    declaration_kind(&spellings).or_else(|| {
        (is_flow_statement_head(first_text)
            || matches!(
                first.kind,
                SyntaxKind::IdentifierToken | SyntaxKind::EntityReferenceToken
            ))
        .then_some(SyntaxKind::TopLevelFlowItem)
        .or(Some(SyntaxKind::ErrorItem))
    })
}

fn declaration_kind(keywords: &[&str]) -> Option<SyntaxKind> {
    let keyword = keywords
        .iter()
        .copied()
        .find(|keyword| !matches!(*keyword, "pub" | "crate" | "super"))?;
    Some(match keyword {
        "mod" => SyntaxKind::ModuleDeclaration,
        "use" => SyntaxKind::UseDeclaration,
        "flow" => SyntaxKind::FlowItem,
        "fn" => SyntaxKind::FunctionItem,
        "predicate" => SyntaxKind::PredicateItem,
        "proof" => SyntaxKind::ProofItem,
        "agent" => SyntaxKind::AgentItem,
        "callable" => SyntaxKind::CallableItem,
        "state" => SyntaxKind::StateItem,
        "trait" => SyntaxKind::TraitItem,
        "impl" => SyntaxKind::ImplItem,
        "enum" => SyntaxKind::EnumItem,
        "struct" => SyntaxKind::StructItem,
        "type" => SyntaxKind::TypeAliasItem,
        "entity" => SyntaxKind::EntityDeclarationItem,
        "entry" => SyntaxKind::EntryDeclarationItem,
        "extern" if keywords.contains(&"capability") => SyntaxKind::ExternCapabilityItem,
        "extern" if keywords.contains(&"mod") => SyntaxKind::ExternModuleItem,
        "hook" => SyntaxKind::HookItem,
        "dialogue" if keywords.contains(&"defaults") => SyntaxKind::DialogueDefaultsItem,
        "memo" if keywords.contains(&"fn") => SyntaxKind::MemoFunctionItem,
        "test" => SyntaxKind::TestItem,
        "bench" => SyntaxKind::BenchItem,
        "parser" => SyntaxKind::ParserItem,
        "source" => SyntaxKind::SourceItem,
        "style" => SyntaxKind::StyleItem,
        _ => return None,
    })
}

fn is_flow_statement_head(spelling: &str) -> bool {
    matches!(
        spelling,
        "assert"
            | "await"
            | "break"
            | "choice"
            | "close"
            | "continue"
            | "defer"
            | "for"
            | "goto"
            | "if"
            | "let"
            | "loop"
            | "match"
            | "on"
            | "out"
            | "return"
            | "select"
            | "signal"
            | "thread"
            | "unsafe"
            | "wait"
            | "while"
            | "yield"
    )
}

const fn newline_len(source: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    match bytes {
        [b'\r', b'\n', ..] => Some(2),
        [b'\r' | b'\n', ..] => Some(1),
        _ => None,
    }
}

fn take_until_newline(source: &str) -> usize {
    source.find(['\r', '\n']).unwrap_or(source.len())
}

fn take_while(source: &str, predicate: impl Fn(char) -> bool) -> usize {
    source
        .char_indices()
        .take_while(|(_, character)| predicate(*character))
        .map(|(index, character)| index + character.len_utf8())
        .last()
        .unwrap_or(0)
}

fn raw_string_len(source: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.first() != Some(&b'r') {
        return None;
    }
    let mut quote = 1;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let hashes = quote - 1;
    let body_start = quote + 1;
    let mut search = body_start;
    while let Some(relative) = source[search..].find('"') {
        let close_quote = search + relative;
        let suffix_end = close_quote + 1 + hashes;
        if suffix_end <= bytes.len()
            && bytes[close_quote + 1..suffix_end]
                .iter()
                .all(|byte| *byte == b'#')
        {
            return Some(suffix_end);
        }
        search = close_quote + 1;
    }
    Some(source.len())
}

fn quoted_token(source: &str, delimiter: char) -> (usize, bool) {
    let mut escaped = false;
    for (index, character) in source.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == delimiter {
            return (index + character.len_utf8(), true);
        } else if matches!(character, '\r' | '\n') {
            return (index, false);
        }
    }
    (source.len(), false)
}

fn character_or_lifetime(source: &str) -> (SyntaxKind, usize) {
    if let Some(len) = character_literal_len(source) {
        return (SyntaxKind::CharacterToken, len);
    }
    let rest = &source[1..];
    let Some(first) = rest.chars().next() else {
        return (SyntaxKind::PunctuationToken, 1);
    };
    if is_identifier_start(first) {
        return (
            SyntaxKind::LifetimeToken,
            1 + take_while(rest, is_identifier_continue),
        );
    }
    (SyntaxKind::PunctuationToken, 1)
}

fn character_literal_len(source: &str) -> Option<usize> {
    let rest = source.strip_prefix('\'')?;
    let first = rest.chars().next()?;
    let content_end = if first != '\\' {
        first.len_utf8()
    } else if let Some(body) = rest.strip_prefix("\\u{") {
        let close = body.find('}')?;
        let digits = &body[..close];
        if digits.is_empty()
            || !digits
                .chars()
                .all(|character| character == '_' || character.is_ascii_hexdigit())
        {
            return None;
        }
        "\\u{".len() + close + 1
    } else if let Some(body) = rest.strip_prefix("\\x") {
        let digits = body.get(..2)?;
        if !digits
            .chars()
            .all(|character| character.is_ascii_hexdigit())
        {
            return None;
        }
        4
    } else {
        '\\'.len_utf8() + rest['\\'.len_utf8()..].chars().next()?.len_utf8()
    };
    (rest.as_bytes().get(content_end) == Some(&b'\'')).then_some(content_end + 2)
}

fn entity_reference_len(source: &str) -> Option<usize> {
    let rest = source.strip_prefix('@')?;
    if let Some(delimited) = rest.strip_prefix('{') {
        return Some(delimited.find('}').map_or(source.len(), |close| close + 3));
    }
    let len = take_while(rest, |character| {
        is_identifier_continue(character) || matches!(character, '.' | ':' | '-' | '/')
    });
    (len > 0).then_some(len + 1)
}

fn number_len(source: &str) -> usize {
    let bytes = source.as_bytes();
    let mut cursor = 1;

    if bytes.first() == Some(&b'0')
        && matches!(bytes.get(1), Some(b'x' | b'X' | b'b' | b'B' | b'o' | b'O'))
    {
        cursor = 2;
        while bytes
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        {
            cursor += 1;
        }
        return cursor;
    }

    while bytes
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
    {
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'.') && bytes.get(cursor + 1) != Some(&b'.') {
        cursor += 1;
        while bytes
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
        {
            cursor += 1;
        }
    }
    if matches!(bytes.get(cursor), Some(b'e' | b'E')) {
        let exponent = cursor;
        cursor += 1;
        if matches!(bytes.get(cursor), Some(b'+' | b'-')) {
            cursor += 1;
        }
        let digits = cursor;
        while bytes
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
        {
            cursor += 1;
        }
        if !bytes[digits..cursor].iter().any(u8::is_ascii_digit) {
            cursor = exponent;
        }
    }
    while bytes
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'%') {
        cursor += 1;
    }
    cursor
}

fn punctuation_len(source: &str) -> Option<usize> {
    const MULTI: &[&str] = &[
        "..=", "===", "::", "->", "=>", "==", "!=", "<=", ">=", "&&", "||", "??", "?.", "..", "+=",
        "-=", "*=", "/=", "%=", "<<", ">>", "**", "|>", "<-",
    ];
    MULTI
        .iter()
        .find_map(|punctuation| source.starts_with(punctuation).then_some(punctuation.len()))
        .or_else(|| {
            source.chars().next().and_then(|character| {
                "(){}[]<>,.;:+-*/%=!?&|^~#@"
                    .contains(character)
                    .then_some(character.len_utf8())
            })
        })
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_alphabetic()
}

fn is_identifier_continue(character: char) -> bool {
    is_identifier_start(character) || character.is_ascii_digit()
}

fn is_keyword(spelling: &str) -> bool {
    matches!(
        spelling,
        "agent"
            | "as"
            | "assert"
            | "await"
            | "bench"
            | "break"
            | "callable"
            | "capability"
            | "choice"
            | "close"
            | "continue"
            | "crate"
            | "debug"
            | "defer"
            | "dialogue"
            | "defaults"
            | "else"
            | "ensures"
            | "entity"
            | "enum"
            | "entry"
            | "extern"
            | "false"
            | "flow"
            | "for"
            | "fn"
            | "goto"
            | "hook"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "lifetime"
            | "loop"
            | "match"
            | "memo"
            | "mod"
            | "move"
            | "mut"
            | "on"
            | "out"
            | "parser"
            | "predicate"
            | "proof"
            | "pub"
            | "requires"
            | "return"
            | "scope"
            | "select"
            | "self"
            | "signal"
            | "source"
            | "state"
            | "struct"
            | "style"
            | "super"
            | "test"
            | "thread"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "wait"
            | "where"
            | "while"
            | "yield"
    )
}

#[cfg(test)]
mod tests {
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    use super::{DocumentLexer, SyntaxKind, parse_shadow_document};

    fn document(text: &str) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/shadow-document").unwrap(),
            SourceName::path("shadow-document.arcw"),
            text,
        )
        .unwrap()
    }

    #[test]
    fn one_pass_lexer_classifies_current_token_families_losslessly() {
        let source = "proof π<'a>(c: Char = '界') = r##\"x\r\ny\"## // note\r\n@actor.hero";
        let document = document(source);
        let tokens = DocumentLexer::new(source).lex();
        let rebuilt = tokens
            .iter()
            .map(|token| &source[token.range.as_range()])
            .collect::<String>();
        assert_eq!(rebuilt, source);
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == SyntaxKind::KeywordToken)
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == SyntaxKind::LifetimeToken)
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == SyntaxKind::CharacterToken)
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == SyntaxKind::RawStringToken)
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == SyntaxKind::EntityReferenceToken)
        );
        assert_eq!(
            parse_shadow_document(&document)
                .unwrap()
                .green()
                .to_string(),
            source
        );
    }

    #[test]
    fn block_comments_split_newlines_without_losing_comment_state() {
        let source = "/** doc\r\nstill */\n/* ordinary */";
        let tokens = DocumentLexer::new(source).lex();
        assert_eq!(
            tokens
                .iter()
                .map(|token| &source[token.range.as_range()])
                .collect::<String>(),
            source
        );
        assert_eq!(tokens[0].kind, SyntaxKind::DocCommentToken);
        assert_eq!(tokens[1].kind, SyntaxKind::NewlineToken);
        assert_eq!(tokens[2].kind, SyntaxKind::DocCommentToken);
        assert_eq!(tokens[3].kind, SyntaxKind::NewlineToken);
        assert_eq!(tokens[4].kind, SyntaxKind::CommentToken);
    }

    #[test]
    fn numeric_ranges_raw_strings_and_character_escapes_keep_exact_boundaries() {
        let source = "1..2 3.14 6.02e-23 0xff_u8 r###\"x\"##y\"### '界' '\\u{754c}' 'life";
        let tokens = DocumentLexer::new(source).lex();
        let significant = tokens
            .iter()
            .filter(|token| token.kind != SyntaxKind::WhitespaceToken)
            .map(|token| (token.kind, &source[token.range.as_range()]))
            .collect::<Vec<_>>();
        assert_eq!(
            significant,
            [
                (SyntaxKind::NumberToken, "1"),
                (SyntaxKind::PunctuationToken, ".."),
                (SyntaxKind::NumberToken, "2"),
                (SyntaxKind::NumberToken, "3.14"),
                (SyntaxKind::NumberToken, "6.02e-23"),
                (SyntaxKind::NumberToken, "0xff_u8"),
                (SyntaxKind::RawStringToken, "r###\"x\"##y\"###"),
                (SyntaxKind::CharacterToken, "'界'"),
                (SyntaxKind::CharacterToken, "'\\u{754c}'"),
                (SyntaxKind::LifetimeToken, "'life"),
            ]
        );
    }

    #[test]
    fn shadow_root_assigns_current_item_families_without_public_identity() {
        let source = concat!(
            "pub predicate positive(x: Int) = x > 0\n",
            "proof unit() {}\n",
            "pub(crate) fn value() -> Int { 1 }\n",
            "let shown = true\n",
            "???\n",
        );
        let built = parse_shadow_document(&document(source)).unwrap();
        let kinds = built
            .index()
            .entries()
            .iter()
            .map(crate::grammar::build::UnattachedGrammarEntry::kind)
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                SyntaxKind::SourceFile,
                SyntaxKind::PredicateItem,
                SyntaxKind::ProofItem,
                SyntaxKind::FunctionItem,
                SyntaxKind::TopLevelFlowItem,
                SyntaxKind::ErrorItem,
            ]
        );
        assert_eq!(built.green().to_string(), source);
    }
}
