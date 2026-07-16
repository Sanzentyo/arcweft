use std::collections::BTreeMap;

use arcweft_source::{
    MAX_REGISTRATION_SOURCE_BYTES, SourceDocument, SourceRange, SourceSpan, SourceSpanError,
};
use thiserror::Error;

use crate::{
    LaunchProfileError, LaunchProfileManifest,
    source::{
        LaunchKeyPath, LaunchManifestSourceMap, LaunchToken, LaunchTokenPath,
        SourceBackedLaunchManifest,
    },
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TomlStructuralErrorKind {
    UnexpectedEnd,
    InvalidTableHeader,
    InvalidDottedKey,
    InvalidString,
    InvalidEscape,
    InvalidNumber,
    InvalidArray,
    TrailingData,
}

#[derive(Debug, Error)]
pub enum LaunchDocumentError {
    #[error("launch source exceeds the byte limit")]
    SourceBytesLimit { observed: u64, maximum: u64 },
    #[error("duplicate TOML key")]
    DuplicateKey {
        path: LaunchKeyPath,
        first: SourceSpan,
        duplicate: SourceSpan,
    },
    #[error("duplicate TOML table")]
    DuplicateTable {
        path: LaunchKeyPath,
        first: SourceSpan,
        duplicate: SourceSpan,
    },
    #[error("launch profiles cannot bind source role `{key}`")]
    ForbiddenProfileRole { key: String, span: SourceSpan },
    #[error("invalid launch TOML structure")]
    Syntax {
        kind: TomlStructuralErrorKind,
        span: SourceSpan,
    },
    #[error(transparent)]
    SourceSpan(#[from] SourceSpanError),
    #[error("typed launch decoding failed: {error}")]
    Typed {
        error: LaunchProfileError,
        span: SourceSpan,
    },
}

impl SourceBackedLaunchManifest {
    pub fn parse_document(document: &SourceDocument) -> Result<Self, LaunchDocumentError> {
        let observed = u64::try_from(document.text().len()).unwrap_or(u64::MAX);
        if observed > MAX_REGISTRATION_SOURCE_BYTES {
            return Err(LaunchDocumentError::SourceBytesLimit {
                observed,
                maximum: MAX_REGISTRATION_SOURCE_BYTES,
            });
        }
        let source_map = TomlScanner::new(document).scan()?;
        let document_span = document.span(SourceRange::new(0, document.text().len()))?;
        let manifest = LaunchProfileManifest::parse_toml(document.text()).map_err(|error| {
            LaunchDocumentError::Typed {
                error,
                span: document_span,
            }
        })?;
        Ok(Self {
            manifest,
            source_map,
        })
    }
}

struct TomlScanner<'a> {
    document: &'a SourceDocument,
    cursor: usize,
    current_table: LaunchKeyPath,
    table_spans: BTreeMap<LaunchKeyPath, SourceSpan>,
    key_spans: BTreeMap<LaunchKeyPath, SourceSpan>,
    tokens: BTreeMap<LaunchTokenPath, LaunchToken>,
}

impl<'a> TomlScanner<'a> {
    fn new(document: &'a SourceDocument) -> Self {
        Self {
            document,
            cursor: 0,
            current_table: LaunchKeyPath::new(Vec::new()),
            table_spans: BTreeMap::new(),
            key_spans: BTreeMap::new(),
            tokens: BTreeMap::new(),
        }
    }

    fn scan(mut self) -> Result<LaunchManifestSourceMap, LaunchDocumentError> {
        while self.cursor < self.document.text().len() {
            let line_start = self.cursor;
            let line_end = self.document.text()[line_start..]
                .find('\n')
                .map_or(self.document.text().len(), |offset| line_start + offset);
            self.cursor = (line_end + usize::from(line_end < self.document.text().len()))
                .min(self.document.text().len());
            self.scan_line(line_start, line_end)?;
        }
        Ok(LaunchManifestSourceMap::new(
            self.document.identity().clone(),
            self.tokens,
        ))
    }

    fn scan_line(&mut self, line_start: usize, line_end: usize) -> Result<(), LaunchDocumentError> {
        let source = self.document.text();
        let semantic_end = comment_start(source, line_start, line_end);
        let Some((start, end)) = trimmed_range(source, line_start, semantic_end) else {
            return Ok(());
        };
        if source.as_bytes()[start] == b'[' {
            self.scan_table(start, end)
        } else {
            self.scan_assignment(start, end)
        }
    }

    fn scan_table(&mut self, start: usize, end: usize) -> Result<(), LaunchDocumentError> {
        let source = self.document.text();
        if source[start..end].starts_with("[[")
            || !source[start..end].ends_with(']')
            || end <= start + 2
        {
            return Err(self.syntax(TomlStructuralErrorKind::InvalidTableHeader, start, end));
        }
        let inside_start = start + 1;
        let inside_end = end - 1;
        let Some((path_start, path_end)) = trimmed_range(source, inside_start, inside_end) else {
            return Err(self.syntax(TomlStructuralErrorKind::InvalidTableHeader, start, end));
        };
        let path = LaunchKeyPath::new(
            parse_dotted_key(&source[path_start..path_end])
                .map_err(|kind| self.syntax(kind, path_start, path_end))?,
        );
        let span = self.span(start, end);
        if let Some(first) = self.table_spans.insert(path.clone(), span.clone()) {
            return Err(LaunchDocumentError::DuplicateTable {
                path,
                first,
                duplicate: span,
            });
        }
        self.tokens.insert(
            LaunchTokenPath::Table {
                path: path.clone(),
                occurrence: 0,
            },
            LaunchToken::new(span, None, None),
        );
        self.current_table = path;
        Ok(())
    }

    fn scan_assignment(&mut self, start: usize, end: usize) -> Result<(), LaunchDocumentError> {
        let source = self.document.text();
        let equals = find_assignment_equals(source, start, end)
            .ok_or_else(|| self.syntax(TomlStructuralErrorKind::TrailingData, start, end))?;
        let Some((key_start, key_end)) = trimmed_range(source, start, equals) else {
            return Err(self.syntax(TomlStructuralErrorKind::InvalidDottedKey, start, equals));
        };
        let Some((value_start, value_end)) = trimmed_range(source, equals + 1, end) else {
            return Err(self.syntax(TomlStructuralErrorKind::UnexpectedEnd, equals, end));
        };
        validate_value(source, value_start, value_end)
            .map_err(|kind| self.syntax(kind, value_start, value_end))?;
        let key = parse_dotted_key(&source[key_start..key_end])
            .map_err(|kind| self.syntax(kind, key_start, key_end))?;
        let path = self.current_table.extended(key);
        let key_span = self.span(key_start, key_end);
        if let Some(key) = path.profile_field().filter(|key| is_source_role_key(key)) {
            return Err(LaunchDocumentError::ForbiddenProfileRole {
                key: key.to_owned(),
                span: key_span,
            });
        }
        if let Some(first) = self.key_spans.insert(path.clone(), key_span.clone()) {
            return Err(LaunchDocumentError::DuplicateKey {
                path,
                first,
                duplicate: key_span,
            });
        }
        let value_span = self.span(value_start, value_end);
        let string_content = string_content_range(source, value_start, value_end)
            .map(|range| self.span(range.start(), range.end()));
        self.tokens.insert(
            LaunchTokenPath::Key {
                path: path.clone(),
                occurrence: 0,
            },
            LaunchToken::new(key_span.clone(), Some(value_span), string_content),
        );
        if source.as_bytes()[value_start] == b'[' {
            for (index, range) in array_element_ranges(source, value_start, value_end)
                .into_iter()
                .enumerate()
            {
                self.tokens.insert(
                    LaunchTokenPath::ArrayElement {
                        path: path.clone(),
                        occurrence: 0,
                        index,
                    },
                    LaunchToken::new(
                        key_span.clone(),
                        Some(self.span(range.start(), range.end())),
                        string_content_range(source, range.start(), range.end())
                            .map(|content| self.span(content.start(), content.end())),
                    ),
                );
            }
        }
        Ok(())
    }

    fn syntax(
        &self,
        kind: TomlStructuralErrorKind,
        start: usize,
        end: usize,
    ) -> LaunchDocumentError {
        LaunchDocumentError::Syntax {
            kind,
            span: self.span(start, end.max(start)),
        }
    }

    fn span(&self, start: usize, end: usize) -> SourceSpan {
        self.document
            .span(SourceRange::new(start, end))
            .expect("TOML scanner offsets remain on UTF-8 boundaries")
    }
}

fn is_source_role_key(key: &str) -> bool {
    matches!(
        key,
        "state" | "initializer" | "event" | "reducer" | "controller"
    )
}

fn comment_start(source: &str, start: usize, end: usize) -> usize {
    let mut cursor = start;
    let mut quote = None;
    let mut escaped = false;
    while cursor < end {
        let character = source[cursor..end]
            .chars()
            .next()
            .expect("cursor precedes end");
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if character == '\\' => escaped = true,
            Some(active) if character == active => quote = None,
            None if matches!(character, '"' | '\'') => quote = Some(character),
            None if character == '#' => return cursor,
            Some(_) | None => {}
        }
        cursor += character.len_utf8();
    }
    end
}

fn trimmed_range(source: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let value = &source[start..end];
    let leading = value.len() - value.trim_start().len();
    let trailing = value.trim_end().len();
    (leading < trailing).then_some((start + leading, start + trailing))
}

fn find_assignment_equals(source: &str, start: usize, end: usize) -> Option<usize> {
    let mut cursor = start;
    let mut quote = None;
    let mut escaped = false;
    while cursor < end {
        let character = source[cursor..end].chars().next()?;
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if character == '\\' => escaped = true,
            Some(active) if character == active => quote = None,
            None if matches!(character, '"' | '\'') => quote = Some(character),
            None if character == '=' => return Some(cursor),
            Some(_) | None => {}
        }
        cursor += character.len_utf8();
    }
    None
}

fn parse_dotted_key(source: &str) -> Result<Vec<String>, TomlStructuralErrorKind> {
    let mut segments = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        while source
            .as_bytes()
            .get(cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            cursor += 1;
        }
        if cursor >= source.len() {
            return Err(TomlStructuralErrorKind::InvalidDottedKey);
        }
        let (segment, next) = if matches!(source.as_bytes()[cursor], b'"' | b'\'') {
            parse_quoted_key(source, cursor)?
        } else {
            let start = cursor;
            while source
                .as_bytes()
                .get(cursor)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            {
                cursor += 1;
            }
            if cursor == start {
                return Err(TomlStructuralErrorKind::InvalidDottedKey);
            }
            (source[start..cursor].to_owned(), cursor)
        };
        segments.push(segment);
        cursor = next;
        while source
            .as_bytes()
            .get(cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            cursor += 1;
        }
        if cursor == source.len() {
            break;
        }
        if source.as_bytes()[cursor] != b'.' {
            return Err(TomlStructuralErrorKind::InvalidDottedKey);
        }
        cursor += 1;
    }
    (!segments.is_empty())
        .then_some(segments)
        .ok_or(TomlStructuralErrorKind::InvalidDottedKey)
}

fn parse_quoted_key(
    source: &str,
    start: usize,
) -> Result<(String, usize), TomlStructuralErrorKind> {
    let quote = source.as_bytes()[start];
    let mut cursor = start + 1;
    let mut escaped = false;
    while cursor < source.len() {
        let byte = source.as_bytes()[cursor];
        if quote == b'"' && escaped {
            escaped = false;
            cursor += 1;
            continue;
        }
        if quote == b'"' && byte == b'\\' {
            escaped = true;
            cursor += 1;
            continue;
        }
        if byte == quote {
            cursor += 1;
            let token = &source[start..cursor];
            let wrapper = format!("value = {token}");
            let value: toml::Value =
                toml::from_str(&wrapper).map_err(|_| TomlStructuralErrorKind::InvalidString)?;
            return value
                .get("value")
                .and_then(toml::Value::as_str)
                .map(|value| (value.to_owned(), cursor))
                .ok_or(TomlStructuralErrorKind::InvalidString);
        }
        cursor += 1;
    }
    Err(TomlStructuralErrorKind::UnexpectedEnd)
}

fn validate_value(source: &str, start: usize, end: usize) -> Result<(), TomlStructuralErrorKind> {
    let wrapper = format!("value = {}", &source[start..end]);
    toml::from_str::<toml::Value>(&wrapper)
        .map(|_| ())
        .map_err(|error| {
            let message = error.to_string();
            if source.as_bytes()[start] == b'[' {
                TomlStructuralErrorKind::InvalidArray
            } else if matches!(source.as_bytes()[start], b'"' | b'\'') {
                if message.contains("escape") {
                    TomlStructuralErrorKind::InvalidEscape
                } else {
                    TomlStructuralErrorKind::InvalidString
                }
            } else if matches!(source.as_bytes()[start], b'+' | b'-' | b'0'..=b'9') {
                TomlStructuralErrorKind::InvalidNumber
            } else {
                TomlStructuralErrorKind::TrailingData
            }
        })
}

fn string_content_range(source: &str, start: usize, end: usize) -> Option<SourceRange> {
    let quote = *source.as_bytes().get(start)?;
    (matches!(quote, b'"' | b'\'')
        && end >= start + 2
        && source.as_bytes().get(end - 1) == Some(&quote))
    .then(|| SourceRange::new(start + 1, end - 1))
}

fn array_element_ranges(source: &str, start: usize, end: usize) -> Vec<SourceRange> {
    if end <= start + 1 || source.as_bytes()[end - 1] != b']' {
        return Vec::new();
    }
    let mut ranges = Vec::new();
    let mut cursor = start + 1;
    let mut element_start = cursor;
    let mut quote = None;
    let mut escaped = false;
    let mut nested = 0_usize;
    while cursor < end - 1 {
        let character = source[cursor..end].chars().next().expect("array cursor");
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if character == '\\' => escaped = true,
            Some(active) if character == active => quote = None,
            None if matches!(character, '"' | '\'') => quote = Some(character),
            None if matches!(character, '[' | '{' | '(') => nested += 1,
            None if matches!(character, ']' | '}' | ')') && nested > 0 => nested -= 1,
            None if character == ',' && nested == 0 => {
                if let Some((value_start, value_end)) = trimmed_range(source, element_start, cursor)
                {
                    ranges.push(SourceRange::new(value_start, value_end));
                }
                element_start = cursor + 1;
            }
            Some(_) | None => {}
        }
        cursor += character.len_utf8();
    }
    if let Some((value_start, value_end)) = trimmed_range(source, element_start, end - 1) {
        ranges.push(SourceRange::new(value_start, value_end));
    }
    ranges
}

#[cfg(test)]
mod tests {
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    use crate::{
        LaunchProfileManifest,
        parse::LaunchDocumentError,
        source::{LaunchKeyPath, LaunchTokenPath, SourceBackedLaunchManifest},
    };

    fn document(source: &str) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://game/arcw.toml").expect("id"),
            SourceName::path("arcw.toml"),
            source,
        )
        .expect("document")
    }

    #[test]
    fn duplicate_scalar_key_retains_spans() {
        let source = "[profiles.game]\nkind = \"game\"\nkind = \"server\"\nsource = \"main.arcw\"\nentry = \"entry.game\"\n";
        let error = SourceBackedLaunchManifest::parse_document(&document(source))
            .expect_err("duplicate key");
        let LaunchDocumentError::DuplicateKey {
            first, duplicate, ..
        } = error
        else {
            panic!("expected duplicate key");
        };
        assert_eq!(&source[first.range().as_range()], "kind");
        assert_eq!(&source[duplicate.range().as_range()], "kind");
    }

    #[test]
    fn duplicate_table_retains_spans() {
        let source = "[profiles.game]\nkind = \"game\"\nsource = \"main.arcw\"\nentry = \"entry.game\"\n[profiles.game]\n";
        let error = SourceBackedLaunchManifest::parse_document(&document(source))
            .expect_err("duplicate table");
        let LaunchDocumentError::DuplicateTable {
            path,
            first,
            duplicate,
        } = error
        else {
            panic!("expected duplicate table");
        };
        assert_eq!(
            path,
            LaunchKeyPath::new(vec!["profiles".to_owned(), "game".to_owned()])
        );
        assert_eq!(&source[first.range().as_range()], "[profiles.game]");
        assert_eq!(&source[duplicate.range().as_range()], "[profiles.game]");
        assert_ne!(first.range(), duplicate.range());
    }

    #[test]
    fn escaped_manifest_path_range_is_exact() {
        let source = "[profiles.game]\nkind = \"game\"\nsource = \"main.arcw\"\nentry = \"entry.game\"\ncharacter_manifests = [\"characters\\u002fakane.json\"]\n";
        let manifest = SourceBackedLaunchManifest::parse_document(&document(source))
            .expect("source-backed manifest");
        let token = manifest
            .source_map()
            .token(&LaunchTokenPath::ArrayElement {
                path: LaunchKeyPath::new(vec![
                    "profiles".to_owned(),
                    "game".to_owned(),
                    "character_manifests".to_owned(),
                ]),
                occurrence: 0,
                index: 0,
            })
            .expect("array token");
        assert_eq!(
            &source[token.value().expect("value").range().as_range()],
            "\"characters\\u002fakane.json\""
        );
        assert_eq!(
            &source[token
                .string_content()
                .expect("string content")
                .range()
                .as_range()],
            "characters\\u002fakane.json"
        );
    }

    #[test]
    fn launch_parse_document_is_the_only_registration_profile_decoder() {
        let source = "[package]\nname = \"game\"\n[profiles.game]\nkind = \"game\"\nsource = \"main.arcw\"\nentry = \"entry.game\"\ncharacter_manifests = [\"characters/akane.json\"]\n";
        let sourced = SourceBackedLaunchManifest::parse_document(&document(source))
            .expect("source-backed registration profile");
        let runtime = LaunchProfileManifest::parse_toml(source).expect("runtime profile decode");
        let token = sourced
            .source_map()
            .token(&LaunchTokenPath::ArrayElement {
                path: LaunchKeyPath::new(vec![
                    "profiles".to_owned(),
                    "game".to_owned(),
                    "character_manifests".to_owned(),
                ]),
                occurrence: 0,
                index: 0,
            })
            .and_then(|token| token.value())
            .expect("registration profile retains the declared path token");

        assert_eq!(sourced.manifest(), &runtime);
        assert_eq!(
            &source[token.range().as_range()],
            "\"characters/akane.json\""
        );
    }

    #[test]
    fn source_roles_are_rejected_at_the_key_span() {
        for role in ["state", "initializer", "event", "reducer", "controller"] {
            let source = format!(
                "[profiles.game]\nkind = \"game\"\nsource = \"main.arcw\"\nentry = \"entry.game.main\"\n{role} = \"forbidden\"\n"
            );
            let error = SourceBackedLaunchManifest::parse_document(&document(&source))
                .expect_err("launch profiles cannot bind source roles");
            let LaunchDocumentError::ForbiddenProfileRole { key, span } = error else {
                panic!("expected forbidden profile role");
            };
            assert_eq!(key, role);
            assert_eq!(&source[span.range().as_range()], role);
        }
    }
}
