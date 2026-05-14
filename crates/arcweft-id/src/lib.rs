use core::fmt;
use core::str::FromStr;
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{kind}")]
pub struct IdError {
    kind: IdErrorKind,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum IdErrorKind {
    #[error("identifier must not be empty")]
    Empty,
    #[error("identifier value must not include a leading # reference marker")]
    StartsWithReferenceMarker,
    #[error("identifier must not contain whitespace")]
    ContainsWhitespace,
    #[error("identifier must not contain control characters")]
    ContainsControl,
    #[error("identifier uses a reserved Arcweft prefix")]
    ReservedPrefix,
}

impl IdError {
    pub const fn kind(&self) -> IdErrorKind {
        self.kind
    }

    const fn new(kind: IdErrorKind) -> Self {
        Self { kind }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntityId(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PublicId(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextKey(String);

impl EntityId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdError> {
        validate_id_text(&value.into(), false, false).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PublicId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdError> {
        validate_id_text(&value.into(), true, true).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TextKey {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdError> {
        validate_id_text(&value.into(), true, false).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for PublicId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for TextKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for EntityId {
    type Err = IdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

impl FromStr for PublicId {
    type Err = IdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

impl FromStr for TextKey {
    type Err = IdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

fn validate_id_text(
    value: &str,
    reject_hash: bool,
    reject_reserved: bool,
) -> Result<String, IdError> {
    if value.is_empty() {
        return Err(IdError::new(IdErrorKind::Empty));
    }

    if reject_hash && value.starts_with('#') {
        return Err(IdError::new(IdErrorKind::StartsWithReferenceMarker));
    }

    if value.chars().any(char::is_whitespace) {
        return Err(IdError::new(IdErrorKind::ContainsWhitespace));
    }

    if value.chars().any(char::is_control) {
        return Err(IdError::new(IdErrorKind::ContainsControl));
    }

    if reject_reserved && is_reserved_prefix(value) {
        return Err(IdError::new(IdErrorKind::ReservedPrefix));
    }

    Ok(value.to_owned())
}

fn is_reserved_prefix(value: &str) -> bool {
    let first = value.split(['.', ':']).next().unwrap_or(value);
    matches!(first, "arcweft" | "__arcweft" | "builtin" | "core" | "std")
}

#[cfg(test)]
mod tests {
    use super::{IdErrorKind, PublicId, TextKey};

    #[test]
    fn public_id_rejects_reference_marker() {
        let err = PublicId::try_new("#flow.opening").expect_err("leading # is syntax, not id data");
        assert_eq!(err.kind(), IdErrorKind::StartsWithReferenceMarker);
    }

    #[test]
    fn public_id_rejects_reserved_prefix() {
        let err = PublicId::try_new("arcweft.internal").expect_err("reserved prefix must fail");
        assert_eq!(err.kind(), IdErrorKind::ReservedPrefix);
    }

    #[test]
    fn text_key_accepts_domain_key() {
        let key = TextKey::try_new("say.opening.dream_hint").expect("valid text key");
        assert_eq!(key.as_str(), "say.opening.dream_hint");
    }
}
