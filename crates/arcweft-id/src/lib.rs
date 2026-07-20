pub mod dialogue;
mod locale;

use core::fmt;
use core::str::FromStr;
use thiserror::Error;

pub use locale::{LocaleTag, LocaleTagError};

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

/// Retained global-identity family owned by Arcweft.
///
/// `Asset` participates in retained reference validation even though packaged
/// assets are discovered by the asset catalog rather than an authored
/// top-level declaration.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RetainedIdentityFamily {
    Asset,
    Character,
    View,
    Action,
    Activity,
    Signal,
    Metric,
    Layer,
}

/// Validated module-local spelling of a retained declaration.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeclarationName(String);

/// Validated source-surface alias for a Character declaration.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterSurfaceAlias(String);

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DeclarationNameError {
    #[error("declaration name must not be empty")]
    Empty,
    #[error("declaration name is not one Arcweft identifier: {value}")]
    InvalidIdentifier { value: String },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PublicIdFamilyError {
    #[error("public ID {id} does not belong to the {expected} family")]
    WrongFamily {
        expected: &'static str,
        id: PublicId,
    },
    #[error("derived public ID is invalid")]
    InvalidDerivedId(#[from] IdError),
}

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

    /// Constructs an engine-owned public identity whose reserved prefix is
    /// intentionally unavailable to authored source.
    ///
    /// This remains checked for reference markers, whitespace, and control
    /// characters; only the reserved-prefix rule differs from [`Self::try_new`].
    pub fn try_new_engine_owned(value: impl Into<String>) -> Result<Self, IdError> {
        validate_id_text(&value.into(), true, false).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl RetainedIdentityFamily {
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Asset => "asset",
            Self::Character => "character",
            Self::View => "view",
            Self::Action => "action",
            Self::Activity => "activity",
            Self::Signal => "signal",
            Self::Metric => "metric",
            Self::Layer => "layer",
        }
    }

    pub fn validate_public_id(self, id: &PublicId) -> Result<(), PublicIdFamilyError> {
        let prefix = self.prefix();
        if id
            .as_str()
            .strip_prefix(prefix)
            .and_then(|tail| tail.strip_prefix('.'))
            .is_some_and(|tail| !tail.is_empty())
        {
            return Ok(());
        }
        Err(PublicIdFamilyError::WrongFamily {
            expected: prefix,
            id: id.clone(),
        })
    }

    pub fn derive_public_id(
        self,
        local: &DeclarationName,
    ) -> Result<PublicId, PublicIdFamilyError> {
        PublicId::try_new(format!("{}.{}", self.prefix(), local.as_str())).map_err(Into::into)
    }
}

impl DeclarationName {
    pub fn try_new(value: impl Into<String>) -> Result<Self, DeclarationNameError> {
        validate_declaration_identifier(value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CharacterSurfaceAlias {
    pub fn try_new(value: impl Into<String>) -> Result<Self, DeclarationNameError> {
        validate_declaration_identifier(value.into()).map(Self)
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

impl fmt::Display for DeclarationName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for CharacterSurfaceAlias {
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

impl FromStr for DeclarationName {
    type Err = DeclarationNameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

impl FromStr for CharacterSurfaceAlias {
    type Err = DeclarationNameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

fn validate_declaration_identifier(value: String) -> Result<String, DeclarationNameError> {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return Err(DeclarationNameError::Empty);
    };
    if (first != '_' && !first.is_alphabetic())
        || !characters.all(|character| {
            character == '_' || character.is_alphabetic() || character.is_ascii_digit()
        })
    {
        return Err(DeclarationNameError::InvalidIdentifier { value });
    }
    Ok(value)
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
    use super::{
        CharacterSurfaceAlias, DeclarationName, IdErrorKind, PublicId, PublicIdFamilyError,
        RetainedIdentityFamily, TextKey,
    };

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
    fn engine_owned_public_id_accepts_reserved_prefix_without_weakening_text_checks() {
        assert_eq!(
            PublicId::try_new_engine_owned("std.view.dialogue")
                .unwrap()
                .as_str(),
            "std.view.dialogue"
        );
        assert_eq!(
            PublicId::try_new_engine_owned("#std.view.dialogue")
                .unwrap_err()
                .kind(),
            IdErrorKind::StartsWithReferenceMarker
        );
    }

    #[test]
    fn text_key_accepts_domain_key() {
        let key = TextKey::try_new("say.opening.dream_hint").expect("valid text key");
        assert_eq!(key.as_str(), "say.opening.dream_hint");
    }

    #[test]
    fn retained_family_validates_and_derives_public_identity() {
        let name = DeclarationName::try_new("MainDialogue").expect("declaration name");
        let derived = RetainedIdentityFamily::View
            .derive_public_id(&name)
            .expect("derived View identity");
        assert_eq!(derived.as_str(), "view.MainDialogue");
        assert_eq!(
            RetainedIdentityFamily::Character
                .validate_public_id(&derived)
                .expect_err("View identity is not a Character identity"),
            PublicIdFamilyError::WrongFamily {
                expected: "character",
                id: derived,
            }
        );
    }

    #[test]
    fn retained_local_names_share_the_language_identifier_rule() {
        assert!(DeclarationName::try_new("会話2").is_ok());
        assert!(CharacterSurfaceAlias::try_new("alice_2").is_ok());
        assert!(DeclarationName::try_new("dialogue.main").is_err());
        assert!(DeclarationName::try_new("2dialogue").is_err());
    }
}
