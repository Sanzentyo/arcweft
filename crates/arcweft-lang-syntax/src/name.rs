//! Validated source-level identifier names shared by typed syntax families.

/// One validated identifier spelling.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxName(Box<str>);

/// Why an attempted syntax name was not admitted.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxNameIssue {
    Missing,
    InvalidStart { spelling: Box<str> },
    InvalidContinuation { spelling: Box<str> },
}

impl SyntaxName {
    pub(crate) fn try_new(spelling: &str) -> Result<Self, SyntaxNameIssue> {
        let mut characters = spelling.chars();
        let Some(first) = characters.next() else {
            return Err(SyntaxNameIssue::Missing);
        };
        if !is_identifier_start(first) {
            return Err(SyntaxNameIssue::InvalidStart {
                spelling: spelling.into(),
            });
        }
        if !characters.all(is_identifier_continue) {
            return Err(SyntaxNameIssue::InvalidContinuation {
                spelling: spelling.into(),
            });
        }
        Ok(Self(spelling.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub(crate) fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_alphabetic()
}

pub(crate) fn is_identifier_continue(character: char) -> bool {
    is_identifier_start(character) || character.is_ascii_digit()
}
