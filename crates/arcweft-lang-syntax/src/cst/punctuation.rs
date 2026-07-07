//! Token-based punctuation scans and delimiter-aware source splits.

use super::lexer::{CstToken, lex_cst, token_text_is};
use super::{CstPunctuationDeltas, SyntaxKind};

/// Arcweft punctuation sequences that carry grammar meaning beyond a single
/// ASCII character.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ArcweftPunctuation {
    ThinArrow,
    LeftArrow,
    FatArrow,
    Pipe,
}

impl ArcweftPunctuation {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ThinArrow => "->",
            Self::LeftArrow => "<-",
            Self::FatArrow => "=>",
            Self::Pipe => "|>",
        }
    }

    const fn sequence(self) -> &'static [&'static str] {
        match self {
            Self::ThinArrow => &["-", ">"],
            Self::LeftArrow => &["<", "-"],
            Self::FatArrow => &["=", ">"],
            Self::Pipe => &["|", ">"],
        }
    }
}

/// Reusable punctuation token scan for source fragments that are not CST lines.
///
/// `CstLine` already stores punctuation summaries built during rowan line
/// projection. This type serves the remaining body-fragment paths where several
/// punctuation queries need to inspect the same string slice.
#[derive(Clone, Debug)]
pub(crate) struct CstPunctuationScan<'a> {
    tokens: Vec<CstToken<'a>>,
}

impl<'a> CstPunctuationScan<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self {
            tokens: lex_cst(source),
        }
    }

    pub(crate) fn find_matching_punctuation(
        &self,
        open_offset: usize,
        open: char,
        close: char,
    ) -> Option<usize> {
        let mut depth = 0usize;
        for token in self.punctuation_tokens() {
            if token.start() < open_offset {
                continue;
            }
            if token.text_starts_with(open) {
                depth += 1;
            } else if token.text_starts_with(close) {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(token.start());
                }
            }
        }
        None
    }

    pub(crate) fn find_top_level_punctuation(&self, punctuation: char) -> Option<usize> {
        let mut paren = 0usize;
        let mut square = 0usize;
        let mut brace = 0usize;
        let mut angle = 0usize;

        for token in self.punctuation_tokens() {
            if token.text_starts_with(punctuation)
                && paren == 0
                && square == 0
                && brace == 0
                && angle == 0
            {
                return Some(token.start());
            }

            update_depths(
                token.text(),
                &mut paren,
                &mut square,
                &mut brace,
                &mut angle,
            );
        }
        None
    }

    pub(crate) fn find_top_level_matching_punctuation(
        &self,
        open: char,
        close: char,
    ) -> Option<(usize, usize)> {
        let open_offset = self.find_top_level_punctuation(open)?;
        let close_offset = self.find_matching_punctuation(open_offset, open, close)?;
        Some((open_offset, close_offset))
    }

    pub(crate) fn deltas(&self) -> CstPunctuationDeltas {
        self.punctuation_tokens()
            .fold(CstPunctuationDeltas::default(), |mut deltas, token| {
                match token.text() {
                    "{" => deltas.brace += 1,
                    "}" => deltas.brace -= 1,
                    "(" => deltas.paren += 1,
                    ")" => deltas.paren -= 1,
                    "[" => deltas.bracket += 1,
                    "]" => deltas.bracket -= 1,
                    _ => {}
                }
                deltas
            })
    }

    pub(crate) fn line_deltas(&self, source: &str) -> Vec<CstPunctuationDeltas> {
        let mut deltas = source
            .lines()
            .map(|_| CstPunctuationDeltas::default())
            .collect::<Vec<_>>();
        let mut line_index = 0usize;
        let mut next_line_start = next_line_start_after(source, 0);

        for token in self.punctuation_tokens() {
            while let Some(start) = next_line_start {
                if token.start() < start {
                    break;
                }
                line_index += 1;
                next_line_start = next_line_start_after(source, start);
            }
            let Some(line_delta) = deltas.get_mut(line_index) else {
                break;
            };
            match token.text() {
                "{" => line_delta.brace += 1,
                "}" => line_delta.brace -= 1,
                "(" => line_delta.paren += 1,
                ")" => line_delta.paren -= 1,
                "[" => line_delta.bracket += 1,
                "]" => line_delta.bracket -= 1,
                _ => {}
            }
        }
        deltas
    }

    fn punctuation_tokens(&self) -> impl Iterator<Item = &CstToken<'a>> {
        self.tokens
            .iter()
            .filter(|token| token.kind() == SyntaxKind::Punctuation)
    }
}

fn next_line_start_after(source: &str, start: usize) -> Option<usize> {
    source[start..]
        .find('\n')
        .map(|relative| start + relative + '\n'.len_utf8())
}

fn update_depths(
    text: &str,
    paren: &mut usize,
    square: &mut usize,
    brace: &mut usize,
    angle: &mut usize,
) {
    match text {
        "(" => *paren += 1,
        ")" => *paren = paren.saturating_sub(1),
        "[" => *square += 1,
        "]" => *square = square.saturating_sub(1),
        "{" => *brace += 1,
        "}" => *brace = brace.saturating_sub(1),
        "<" => *angle += 1,
        ">" => *angle = angle.saturating_sub(1),
        _ => {}
    }
}
/// Finds the close punctuation matching an opening punctuation token.
///
/// The scan is token-based, so quoted strings and comments are never inspected
/// as nested syntax. This is the interim CST event utility used while the
/// grammar parser is being migrated away from local string splitters.
pub(crate) fn find_matching_punctuation(
    source: &str,
    open_offset: usize,
    open: char,
    close: char,
) -> Option<usize> {
    CstPunctuationScan::new(source).find_matching_punctuation(open_offset, open, close)
}

/// Finds the first top-level opening punctuation and its matching close with one token scan.
pub(crate) fn find_top_level_matching_punctuation(
    source: &str,
    open: char,
    close: char,
) -> Option<(usize, usize)> {
    CstPunctuationScan::new(source).find_top_level_matching_punctuation(open, close)
}

/// Finds a top-level punctuation token while ignoring strings and comments.
pub(crate) fn find_top_level_punctuation(source: &str, punctuation: char) -> Option<usize> {
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;

    for token in lex_cst(source) {
        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }

        if token.text_starts_with(punctuation)
            && paren == 0
            && square == 0
            && brace == 0
            && angle == 0
        {
            return Some(token.start());
        }

        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" => angle = angle.saturating_sub(1),
            _ => {}
        }
    }
    None
}

/// Finds the last top-level punctuation token while ignoring strings and comments.
pub(crate) fn find_last_top_level_punctuation(source: &str, punctuation: char) -> Option<usize> {
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;
    let mut found = None;

    for token in lex_cst(source) {
        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }

        if token.text_starts_with(punctuation)
            && paren == 0
            && square == 0
            && brace == 0
            && angle == 0
        {
            found = Some(token.start());
        }

        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" => angle = angle.saturating_sub(1),
            _ => {}
        }
    }
    found
}

/// Finds the last opening punctuation that starts while the matching delimiter depth is zero.
#[cfg(test)]
pub(crate) fn find_last_depth_zero_open_punctuation(
    source: &str,
    open: char,
    close: char,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut found = None;

    for token in lex_cst(source) {
        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }

        match token.text() {
            text if token_text_is(text, open) => {
                if depth == 0 {
                    found = Some(token.start());
                }
                depth += 1;
            }
            text if token_text_is(text, close) => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    found
}

/// Finds the closing `>` for an angle group that starts at `open_offset`.
pub(crate) fn find_matching_angle_group(source: &str, open_offset: usize) -> Option<usize> {
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;
    let mut previous_text = "";

    for token in lex_cst(source) {
        if token.kind() != SyntaxKind::Punctuation {
            previous_text = token.text();
            continue;
        }
        if token.start() < open_offset {
            previous_text = token.text();
            continue;
        }

        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" if previous_text != "-" => {
                angle = angle.checked_sub(1)?;
                if paren == 0 && square == 0 && brace == 0 && angle == 0 {
                    return Some(token.start());
                }
            }
            _ => {}
        }
        previous_text = token.text();
    }
    None
}

/// Splits once at a top-level punctuation token.
pub(crate) fn split_top_level_punctuation_once(
    source: &str,
    delimiter: char,
) -> Option<(&str, &str)> {
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;

    for token in lex_cst(source) {
        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }

        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" => angle = angle.saturating_sub(1),
            text if token_text_is(text, delimiter)
                && paren == 0
                && square == 0
                && brace == 0
                && angle == 0 =>
            {
                return Some((source[..token.start()].trim(), source[token.end()..].trim()));
            }
            _ => {}
        }
    }
    None
}

/// Splits at every top-level punctuation token.
pub(crate) fn split_top_level_punctuation(source: &str, delimiter: char) -> Vec<&str> {
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;
    let mut parts = Vec::new();
    let mut start = 0usize;

    for token in lex_cst(source) {
        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }

        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" => angle = angle.saturating_sub(1),
            text if token_text_is(text, delimiter)
                && paren == 0
                && square == 0
                && brace == 0
                && angle == 0 =>
            {
                parts.push(source[start..token.start()].trim());
                start = token.end();
            }
            _ => {}
        }
    }
    let tail = source[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

/// Returns the first complete string-literal body and the tail after the token.
pub(crate) fn split_first_string_literal(source: &str) -> Option<(&str, &str)> {
    lex_cst(source)
        .into_iter()
        .find(|token| token.kind() == SyntaxKind::String)
        .and_then(|token| {
            let text = token.text();
            (text.len() >= 2 && text.starts_with('"') && text.ends_with('"'))
                .then(|| (&text[1..text.len() - 1], &source[token.end()..]))
        })
}

/// Returns all `[[wiki link]]` marker ranges found in source text.
pub(crate) fn collect_wiki_link_ranges(source: &str) -> Vec<(&str, usize, usize)> {
    let mut links = Vec::new();
    let mut cursor = 0;
    while let Some(start_relative) = source[cursor..].find("[[") {
        let start = cursor + start_relative;
        let body_start = start + 2;
        let Some(end_relative) = source[body_start..].find("]]") else {
            break;
        };
        let end = body_start + end_relative;
        links.push((&source[body_start..end], start, end + 2));
        cursor = end + 2;
    }
    links
}

/// Splits once at a top-level contiguous punctuation token sequence.
///
/// Operators such as `=>`, `->`, and `<-` are lexed as individual punctuation
/// tokens. Keeping this sequence splitter in the CST layer prevents each parser
/// family from inventing its own string search for multi-token separators.
pub(crate) fn split_top_level_punctuation_sequence_once<'a>(
    source: &'a str,
    sequence: &[&str],
) -> Option<(&'a str, &'a str)> {
    let tokens = lex_cst(source);
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;

    for (index, token) in tokens.iter().enumerate() {
        if token.kind() == SyntaxKind::Punctuation
            && paren == 0
            && square == 0
            && brace == 0
            && angle == 0
            && punctuation_sequence_matches(&tokens, index, sequence)
        {
            let end = tokens[index + sequence.len() - 1].end();
            return Some((source[..token.start()].trim(), source[end..].trim()));
        }

        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }
        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" if !is_multi_token_punctuation_tail(&tokens, index) => {
                angle = angle.saturating_sub(1);
            }
            _ => {}
        }
    }
    None
}

/// Splits once at a top-level Arcweft punctuation token sequence.
pub(crate) fn split_top_level_arcweft_punctuation_once(
    source: &str,
    punctuation: ArcweftPunctuation,
) -> Option<(&str, &str)> {
    split_top_level_punctuation_sequence_once(source, punctuation.sequence())
}

/// Splits once at the last top-level contiguous punctuation token sequence.
pub(crate) fn split_last_top_level_punctuation_sequence_once<'a>(
    source: &'a str,
    sequence: &[&str],
) -> Option<(&'a str, &'a str)> {
    let tokens = lex_cst(source);
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;
    let mut found = None;

    for (index, token) in tokens.iter().enumerate() {
        if token.kind() == SyntaxKind::Punctuation
            && paren == 0
            && square == 0
            && brace == 0
            && angle == 0
            && punctuation_sequence_matches(&tokens, index, sequence)
        {
            let end = tokens[index + sequence.len() - 1].end();
            found = Some((token.start(), end));
        }

        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }
        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" if !is_multi_token_punctuation_tail(&tokens, index) => {
                angle = angle.saturating_sub(1);
            }
            _ => {}
        }
    }

    found.map(|(start, end)| (source[..start].trim(), source[end..].trim()))
}

/// Strips a leading Arcweft punctuation token sequence from already-trimmed source.
pub(crate) fn strip_prefix_arcweft_punctuation(
    source: &str,
    punctuation: ArcweftPunctuation,
) -> Option<&str> {
    let tokens = lex_cst(source);
    if punctuation_sequence_matches(&tokens, 0, punctuation.sequence()) {
        let end = tokens[punctuation.sequence().len() - 1].end();
        Some(&source[end..])
    } else {
        None
    }
}

/// Strips a trailing contiguous Arcweft punctuation spelling from source.
pub(crate) fn strip_suffix_arcweft_punctuation(
    source: &str,
    punctuation: ArcweftPunctuation,
) -> Option<&str> {
    source.strip_suffix(punctuation.as_str())
}

/// Returns whether the source contains the punctuation sequence outside token text.
pub(crate) fn contains_arcweft_punctuation(source: &str, punctuation: ArcweftPunctuation) -> bool {
    let tokens = lex_cst(source);
    tokens
        .iter()
        .enumerate()
        .any(|(index, _)| punctuation_sequence_matches(&tokens, index, punctuation.sequence()))
}

fn punctuation_sequence_matches(tokens: &[CstToken<'_>], index: usize, sequence: &[&str]) -> bool {
    if sequence.is_empty() || index + sequence.len() > tokens.len() {
        return false;
    }

    sequence.iter().enumerate().all(|(offset, expected)| {
        let token = &tokens[index + offset];
        token.kind() == SyntaxKind::Punctuation
            && token.text() == *expected
            && (offset == 0 || tokens[index + offset - 1].end() == token.start())
    })
}

fn is_multi_token_punctuation_tail(tokens: &[CstToken<'_>], index: usize) -> bool {
    let Some(token) = tokens.get(index) else {
        return false;
    };
    if token.kind() != SyntaxKind::Punctuation || token.text() != ">" {
        return false;
    }
    let Some(previous) = index.checked_sub(1).and_then(|index| tokens.get(index)) else {
        return false;
    };
    previous.kind() == SyntaxKind::Punctuation
        && previous.end() == token.start()
        && matches!(previous.text(), "-" | "=" | "|")
}

/// Splits once before a top-level identifier keyword.
pub(crate) fn split_top_level_keyword_once<'a>(
    source: &'a str,
    keyword: &str,
) -> (&'a str, Option<&'a str>) {
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;

    for token in lex_cst(source) {
        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" => angle = angle.saturating_sub(1),
            _ => {}
        }

        if token.kind() == SyntaxKind::Ident
            && token.text() == keyword
            && paren == 0
            && square == 0
            && brace == 0
            && angle == 0
        {
            return (&source[..token.start()], Some(source[token.end()..].trim()));
        }
    }
    (source, None)
}
