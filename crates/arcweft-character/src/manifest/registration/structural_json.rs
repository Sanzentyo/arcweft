//! Structural JSON scanner used by both registration and runtime decoders.

use std::collections::BTreeMap;

use arcweft_source::{SourceDocument, SourceRange};

use super::{JsonObjectPath, bound_span};
use crate::manifest::diagnostic::{
    CharacterRegistrationDecodeError, CharacterRuntimeDecodeError, JsonStructuralErrorKind,
};

#[derive(Clone, Debug)]
pub(super) struct RawJsonNode {
    pub(super) range: SourceRange,
    kind: RawJsonKind,
}

impl RawJsonNode {
    pub(super) fn object(&self) -> Option<&[RawJsonMember]> {
        match &self.kind {
            RawJsonKind::Object(value) => Some(value),
            _ => None,
        }
    }

    pub(super) fn array(&self) -> Option<&[RawJsonNode]> {
        match &self.kind {
            RawJsonKind::Array(value) => Some(value),
            _ => None,
        }
    }

    pub(super) fn string(&self) -> Option<&str> {
        match &self.kind {
            RawJsonKind::String(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
enum RawJsonKind {
    Object(Vec<RawJsonMember>),
    Array(Vec<RawJsonNode>),
    String(String),
    Scalar,
}

#[derive(Clone, Debug)]
pub(super) struct RawJsonMember {
    pub(super) key: String,
    pub(super) key_range: SourceRange,
    pub(super) value: RawJsonNode,
}

pub(super) enum RawJsonError {
    Syntax {
        kind: JsonStructuralErrorKind,
        range: SourceRange,
    },
    Duplicate {
        object: JsonObjectPath,
        key: String,
        first: SourceRange,
        duplicate: SourceRange,
    },
}

impl RawJsonError {
    pub(super) fn bind(self, document: &SourceDocument) -> CharacterRegistrationDecodeError {
        match self {
            Self::Syntax { kind, range } => CharacterRegistrationDecodeError::Syntax {
                kind,
                span: bound_span(document, range),
            },
            Self::Duplicate {
                object,
                key,
                first,
                duplicate,
            } => CharacterRegistrationDecodeError::DuplicateKey {
                object,
                key,
                first: bound_span(document, first),
                duplicate: bound_span(document, duplicate),
            },
        }
    }

    pub(super) fn into_runtime(self) -> CharacterRuntimeDecodeError {
        match self {
            Self::Syntax { kind, range } => CharacterRuntimeDecodeError::Syntax { kind, range },
            Self::Duplicate {
                object,
                key,
                first,
                duplicate,
            } => CharacterRuntimeDecodeError::DuplicateKey {
                object,
                key,
                first,
                duplicate,
            },
        }
    }
}

pub(super) struct RawJsonParser<'a> {
    source: &'a str,
    cursor: usize,
}

impl<'a> RawJsonParser<'a> {
    pub(super) fn new(source: &'a str) -> Self {
        Self { source, cursor: 0 }
    }

    pub(super) fn parse(mut self) -> Result<RawJsonNode, RawJsonError> {
        self.whitespace();
        let node = self.value(&JsonObjectPath::default())?;
        self.whitespace();
        if self.cursor != self.source.len() {
            return Err(self.syntax(JsonStructuralErrorKind::TrailingData));
        }
        Ok(node)
    }

    fn value(&mut self, path: &JsonObjectPath) -> Result<RawJsonNode, RawJsonError> {
        self.whitespace();
        match self.byte() {
            Some(b'{') => self.object(path),
            Some(b'[') => self.array(path),
            Some(b'"') => {
                let (value, range) = self.string_token()?;
                Ok(RawJsonNode {
                    range,
                    kind: RawJsonKind::String(value),
                })
            }
            Some(b't') => self.literal("true"),
            Some(b'f') => self.literal("false"),
            Some(b'n') => self.literal("null"),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.syntax(JsonStructuralErrorKind::UnexpectedToken)),
            None => Err(self.syntax(JsonStructuralErrorKind::UnexpectedEnd)),
        }
    }

    fn object(&mut self, path: &JsonObjectPath) -> Result<RawJsonNode, RawJsonError> {
        let start = self.cursor;
        self.cursor += 1;
        self.whitespace();
        let mut members = Vec::new();
        let mut keys = BTreeMap::<String, SourceRange>::new();
        if self.take(b'}') {
            return Ok(RawJsonNode {
                range: SourceRange::new(start, self.cursor),
                kind: RawJsonKind::Object(members),
            });
        }
        loop {
            self.whitespace();
            let (key, key_range) = self.string_token()?;
            if let Some(first) = keys.get(&key) {
                return Err(RawJsonError::Duplicate {
                    object: path.clone(),
                    key,
                    first: *first,
                    duplicate: key_range,
                });
            }
            keys.insert(key.clone(), key_range);
            self.whitespace();
            if !self.take(b':') {
                return Err(self.syntax(JsonStructuralErrorKind::UnexpectedToken));
            }
            let value = self.value(&path.with_key(key.clone()))?;
            members.push(RawJsonMember {
                key,
                key_range,
                value,
            });
            self.whitespace();
            if self.take(b'}') {
                break;
            }
            if !self.take(b',') {
                return Err(self.syntax(JsonStructuralErrorKind::UnexpectedToken));
            }
        }
        Ok(RawJsonNode {
            range: SourceRange::new(start, self.cursor),
            kind: RawJsonKind::Object(members),
        })
    }

    fn array(&mut self, path: &JsonObjectPath) -> Result<RawJsonNode, RawJsonError> {
        let start = self.cursor;
        self.cursor += 1;
        self.whitespace();
        let mut values = Vec::new();
        if self.take(b']') {
            return Ok(RawJsonNode {
                range: SourceRange::new(start, self.cursor),
                kind: RawJsonKind::Array(values),
            });
        }
        loop {
            values.push(self.value(&path.with_index(values.len()))?);
            self.whitespace();
            if self.take(b']') {
                break;
            }
            if !self.take(b',') {
                return Err(self.syntax(JsonStructuralErrorKind::UnexpectedToken));
            }
        }
        Ok(RawJsonNode {
            range: SourceRange::new(start, self.cursor),
            kind: RawJsonKind::Array(values),
        })
    }

    fn string_token(&mut self) -> Result<(String, SourceRange), RawJsonError> {
        let start = self.cursor;
        if !self.take(b'"') {
            return Err(self.syntax(JsonStructuralErrorKind::UnexpectedToken));
        }
        while let Some(byte) = self.byte() {
            match byte {
                b'"' => {
                    self.cursor += 1;
                    let range = SourceRange::new(start, self.cursor);
                    let token = &self.source[range.as_range()];
                    return serde_json::from_str::<String>(token)
                        .map(|value| (value, range))
                        .map_err(|_| RawJsonError::Syntax {
                            kind: JsonStructuralErrorKind::InvalidUnicodeEscape,
                            range,
                        });
                }
                b'\\' => {
                    self.cursor += 1;
                    match self.byte() {
                        Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
                            self.cursor += 1;
                        }
                        Some(b'u') => {
                            self.cursor += 1;
                            let unicode_end = self.cursor.saturating_add(4);
                            if unicode_end > self.source.len()
                                || !self.source.as_bytes()[self.cursor..unicode_end]
                                    .iter()
                                    .all(u8::is_ascii_hexdigit)
                            {
                                return Err(RawJsonError::Syntax {
                                    kind: JsonStructuralErrorKind::InvalidUnicodeEscape,
                                    range: SourceRange::new(
                                        start,
                                        unicode_end.min(self.source.len()),
                                    ),
                                });
                            }
                            self.cursor = unicode_end;
                        }
                        Some(_) => {
                            return Err(RawJsonError::Syntax {
                                kind: JsonStructuralErrorKind::InvalidEscape,
                                range: SourceRange::new(start, self.cursor + 1),
                            });
                        }
                        None => return Err(self.syntax(JsonStructuralErrorKind::UnexpectedEnd)),
                    }
                }
                0..=0x1f => return Err(self.syntax(JsonStructuralErrorKind::UnexpectedToken)),
                _ => {
                    let character = self.source[self.cursor..]
                        .chars()
                        .next()
                        .expect("cursor is on a UTF-8 boundary");
                    self.cursor += character.len_utf8();
                }
            }
        }
        Err(RawJsonError::Syntax {
            kind: JsonStructuralErrorKind::UnexpectedEnd,
            range: SourceRange::new(start, self.source.len()),
        })
    }

    fn literal(&mut self, literal: &str) -> Result<RawJsonNode, RawJsonError> {
        let start = self.cursor;
        if !self.source[self.cursor..].starts_with(literal) {
            return Err(self.syntax(JsonStructuralErrorKind::UnexpectedToken));
        }
        self.cursor += literal.len();
        Ok(RawJsonNode {
            range: SourceRange::new(start, self.cursor),
            kind: RawJsonKind::Scalar,
        })
    }

    fn number(&mut self) -> Result<RawJsonNode, RawJsonError> {
        let start = self.cursor;
        while self
            .byte()
            .is_some_and(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b',' | b']' | b'}'))
        {
            self.cursor += 1;
        }
        let range = SourceRange::new(start, self.cursor);
        let token = &self.source[range.as_range()];
        if serde_json::from_str::<serde_json::Number>(token).is_err() {
            return Err(RawJsonError::Syntax {
                kind: JsonStructuralErrorKind::InvalidNumber,
                range,
            });
        }
        Ok(RawJsonNode {
            range,
            kind: RawJsonKind::Scalar,
        })
    }

    fn whitespace(&mut self) {
        while self
            .byte()
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.cursor += 1;
        }
    }

    fn take(&mut self, expected: u8) -> bool {
        if self.byte() == Some(expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn byte(&self) -> Option<u8> {
        self.source.as_bytes().get(self.cursor).copied()
    }

    fn syntax(&self, kind: JsonStructuralErrorKind) -> RawJsonError {
        let end = self.source[self.cursor..]
            .chars()
            .next()
            .map_or(self.cursor, |character| self.cursor + character.len_utf8());
        RawJsonError::Syntax {
            kind,
            range: SourceRange::new(self.cursor, end),
        }
    }
}
