pub mod dialogue;
mod locale;

use core::fmt;
use core::str::FromStr;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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
    #[error("identifier value must not include a leading reference marker")]
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

/// Validated relative path within the authored asset virtual-file space.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetVirtualPath(String);

/// Stable catalog identity derived from an [`AssetVirtualPath`].
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetId(PublicId);

/// Global declaration-identity family owned by Arcweft.
///
/// `Asset` participates in retained reference validation even though packaged
/// assets are discovered by the asset catalog rather than an authored
/// top-level declaration. Callable declaration families share this authority
/// instead of maintaining parser-local string tables.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeclarationIdentityFamily {
    Asset,
    Character,
    View,
    Action,
    Activity,
    Signal,
    Metric,
    Layer,
    Flow,
    Proof,
    Style,
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

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AssetVirtualPathError {
    #[error("asset virtual path must not be empty")]
    Empty,
    #[error("asset virtual path must be relative: {value}")]
    Absolute { value: String },
    #[error("asset virtual path must use '/' separators: {value}")]
    Backslash { value: String },
    #[error("asset virtual path contains an empty component: {value}")]
    EmptyComponent { value: String },
    #[error("asset virtual path contains a relative traversal component: {value}")]
    RelativeComponent { value: String },
    #[error("asset virtual path contains a control character: {value}")]
    ContainsControl { value: String },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AssetIdError {
    #[error("asset virtual path has no effective identity components: {path}")]
    NoEffectiveComponents { path: AssetVirtualPath },
    #[error("asset virtual path component cannot form an asset identity: {component}")]
    InvalidComponent { component: String },
    #[error("derived asset public ID is invalid")]
    InvalidPublicId(#[from] IdError),
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

impl DeclarationIdentityFamily {
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
            Self::Flow => "flow",
            Self::Proof => "proof",
            Self::Style => "style",
        }
    }

    pub fn from_prefix(prefix: &str) -> Option<Self> {
        match prefix {
            "asset" => Some(Self::Asset),
            "character" => Some(Self::Character),
            "view" => Some(Self::View),
            "action" => Some(Self::Action),
            "activity" => Some(Self::Activity),
            "signal" => Some(Self::Signal),
            "metric" => Some(Self::Metric),
            "layer" => Some(Self::Layer),
            "flow" => Some(Self::Flow),
            "proof" => Some(Self::Proof),
            "style" => Some(Self::Style),
            _ => None,
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

impl AssetVirtualPath {
    pub fn try_new(value: impl Into<String>) -> Result<Self, AssetVirtualPathError> {
        let value = value.into();
        if value.is_empty() {
            return Err(AssetVirtualPathError::Empty);
        }
        if value.starts_with('/')
            || value
                .as_bytes()
                .get(1)
                .is_some_and(|separator| *separator == b':')
        {
            return Err(AssetVirtualPathError::Absolute { value });
        }
        if value.contains('\\') {
            return Err(AssetVirtualPathError::Backslash { value });
        }
        if value.chars().any(char::is_control) {
            return Err(AssetVirtualPathError::ContainsControl { value });
        }
        let mut components = value.split('/');
        if components.clone().any(str::is_empty) {
            return Err(AssetVirtualPathError::EmptyComponent { value });
        }
        if components.any(|component| matches!(component, "." | "..")) {
            return Err(AssetVirtualPathError::RelativeComponent { value });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AssetId {
    pub fn as_public_id(&self) -> &PublicId {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_public_id(self) -> PublicId {
        self.0
    }
}

impl TryFrom<&AssetVirtualPath> for AssetId {
    type Error = AssetIdError;

    fn try_from(path: &AssetVirtualPath) -> Result<Self, Self::Error> {
        let without_extension = path
            .as_str()
            .rsplit_once('.')
            .map_or(path.as_str(), |(stem, _)| stem);
        if without_extension.is_empty() {
            return Err(AssetIdError::NoEffectiveComponents { path: path.clone() });
        }

        let components = without_extension
            .split('/')
            .map(|component| {
                component
                    .chars()
                    .map(|character| {
                        if character.is_ascii_alphanumeric() {
                            Some(character.to_ascii_lowercase())
                        } else if matches!(character, '_' | '-') {
                            Some('_')
                        } else {
                            None
                        }
                    })
                    .collect::<Option<String>>()
                    .filter(|normalized| !normalized.is_empty())
                    .ok_or_else(|| AssetIdError::InvalidComponent {
                        component: component.to_owned(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        if components.is_empty() {
            return Err(AssetIdError::NoEffectiveComponents { path: path.clone() });
        }
        Ok(Self(PublicId::try_new(format!(
            "asset.{}",
            components.join(".")
        ))?))
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

impl Serialize for TextKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for TextKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for AssetVirtualPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for AssetId {
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

impl FromStr for AssetVirtualPath {
    type Err = AssetVirtualPathError;

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

    if reject_hash && value.starts_with(['#', '@']) {
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
        AssetId, AssetIdError, AssetVirtualPath, CharacterSurfaceAlias, DeclarationIdentityFamily,
        DeclarationName, IdErrorKind, PublicId, PublicIdFamilyError, TextKey,
    };

    #[test]
    fn public_id_rejects_reference_marker() {
        let err = PublicId::try_new("#flow.opening").expect_err("leading # is syntax, not id data");
        assert_eq!(err.kind(), IdErrorKind::StartsWithReferenceMarker);

        let err = PublicId::try_new("@flow.opening").expect_err("leading @ is syntax, not id data");
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
    fn declaration_family_validates_and_derives_public_identity() {
        let name = DeclarationName::try_new("MainDialogue").expect("declaration name");
        let derived = DeclarationIdentityFamily::View
            .derive_public_id(&name)
            .expect("derived View identity");
        assert_eq!(derived.as_str(), "view.MainDialogue");
        assert_eq!(
            DeclarationIdentityFamily::Character
                .validate_public_id(&derived)
                .expect_err("View identity is not a Character identity"),
            PublicIdFamilyError::WrongFamily {
                expected: "character",
                id: derived,
            }
        );
    }

    #[test]
    fn declaration_family_round_trips_its_owned_prefixes() {
        for family in [
            DeclarationIdentityFamily::Asset,
            DeclarationIdentityFamily::Character,
            DeclarationIdentityFamily::View,
            DeclarationIdentityFamily::Action,
            DeclarationIdentityFamily::Activity,
            DeclarationIdentityFamily::Signal,
            DeclarationIdentityFamily::Metric,
            DeclarationIdentityFamily::Layer,
            DeclarationIdentityFamily::Flow,
            DeclarationIdentityFamily::Proof,
            DeclarationIdentityFamily::Style,
        ] {
            assert_eq!(
                DeclarationIdentityFamily::from_prefix(family.prefix()),
                Some(family)
            );
        }
        assert_eq!(DeclarationIdentityFamily::from_prefix("image"), None);
    }

    #[test]
    fn asset_identity_is_derived_from_normalized_virtual_path() {
        let cases = [
            ("bg/Room.png", "asset.bg.room"),
            ("BG/Room.PNG", "asset.bg.room"),
            ("ui/main-menu.webp", "asset.ui.main_menu"),
            ("voice/alice/greeting.ogg", "asset.voice.alice.greeting"),
        ];
        for (path, expected) in cases {
            let path = AssetVirtualPath::try_new(path).expect("valid virtual path");
            assert_eq!(AssetId::try_from(&path).unwrap().as_str(), expected);
        }
    }

    #[test]
    fn asset_identity_rejects_invalid_components_and_empty_stems() {
        let spaced = AssetVirtualPath::try_new("ui/main menu.png").unwrap();
        assert_eq!(
            AssetId::try_from(&spaced),
            Err(AssetIdError::InvalidComponent {
                component: "main menu".to_owned(),
            })
        );

        let extension_only = AssetVirtualPath::try_new(".png").unwrap();
        assert!(matches!(
            AssetId::try_from(&extension_only),
            Err(AssetIdError::NoEffectiveComponents { .. })
        ));
    }

    #[test]
    fn asset_virtual_path_rejects_non_normalized_paths() {
        assert!(AssetVirtualPath::try_new("/ui/main.png").is_err());
        assert!(AssetVirtualPath::try_new("ui\\main.png").is_err());
        assert!(AssetVirtualPath::try_new("ui//main.png").is_err());
        assert!(AssetVirtualPath::try_new("ui/../main.png").is_err());
    }

    #[test]
    fn retained_local_names_share_the_language_identifier_rule() {
        assert!(DeclarationName::try_new("会話2").is_ok());
        assert!(CharacterSurfaceAlias::try_new("alice_2").is_ok());
        assert!(DeclarationName::try_new("dialogue.main").is_err());
        assert!(DeclarationName::try_new("2dialogue").is_err());
    }
}
