use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_TYPE_PATH_SEGMENTS: usize = 256;

/// A validated Rust package identity used by the Arcweft ABI.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ArcweftRustPackageId(String);

/// One validated package-local Rust type path segment.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ArcweftRustTypePathSegment(String);

/// A validated, non-empty package-local Rust type path.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ArcweftRustTypePath {
    segments: Vec<ArcweftRustTypePathSegment>,
}

/// A declaration-local Rust type-parameter ordinal.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ArcweftRustTypeParameterIndex(u16);

/// A validated Rust type-parameter name.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ArcweftRustTypeParameterName(String);

/// A typed identity validation failure at the Rust ABI boundary.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ArcweftRustIdentityError {
    #[error("Rust package ID must not be empty")]
    EmptyPackageId,
    #[error("Rust package ID contains a control character at byte {byte}")]
    PackageControlCharacter { byte: usize },
    #[error("Rust type path must contain at least one segment")]
    EmptyTypePath,
    #[error("Rust type path has {observed} segments, exceeding {maximum}")]
    TypePathSegmentLimit { observed: usize, maximum: usize },
    #[error("Rust identifier must not be empty")]
    EmptyIdentifier,
    #[error("`{value}` is not a valid stored Rust identifier")]
    InvalidIdentifier { value: String },
    #[error("Rust type-parameter index {value} exceeds u16")]
    TypeParameterIndexOverflow { value: usize },
}

impl ArcweftRustPackageId {
    /// Validates and constructs one package identity.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ArcweftRustIdentityError> {
        let value = value.into();
        validate_package_id(&value)?;
        Ok(Self(value))
    }

    /// Returns the validated package ID text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn validate(&self) -> Result<(), ArcweftRustIdentityError> {
        validate_package_id(&self.0)
    }
}

impl ArcweftRustTypePathSegment {
    /// Validates and constructs one package-local path segment.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ArcweftRustIdentityError> {
        let value = value.into();
        validate_identifier(&value)?;
        Ok(Self(value))
    }

    /// Returns the validated segment text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn validate(&self) -> Result<(), ArcweftRustIdentityError> {
        validate_identifier(&self.0)
    }
}

impl ArcweftRustTypePath {
    /// Validates and constructs a non-empty package-local type path.
    pub fn try_new(
        segments: impl IntoIterator<Item = ArcweftRustTypePathSegment>,
    ) -> Result<Self, ArcweftRustIdentityError> {
        let segments = segments.into_iter().collect::<Vec<_>>();
        validate_path_segments(&segments)?;
        Ok(Self { segments })
    }

    /// Returns the exact path segments in semantic order.
    pub fn segments(&self) -> &[ArcweftRustTypePathSegment] {
        &self.segments
    }

    pub(crate) fn validate(&self) -> Result<(), ArcweftRustIdentityError> {
        validate_path_segments(&self.segments)
    }
}

impl ArcweftRustTypeParameterIndex {
    /// Converts a host index without truncation.
    pub fn try_from_usize(value: usize) -> Result<Self, ArcweftRustIdentityError> {
        let value = u16::try_from(value)
            .map_err(|_| ArcweftRustIdentityError::TypeParameterIndexOverflow { value })?;
        Ok(Self(value))
    }

    /// Returns the index as a platform-sized value.
    pub const fn get(self) -> usize {
        self.0 as usize
    }
}

impl ArcweftRustTypeParameterName {
    /// Validates and constructs a type-parameter name.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ArcweftRustIdentityError> {
        let value = value.into();
        validate_identifier(&value)?;
        Ok(Self(value))
    }

    /// Returns the validated parameter name.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn validate(&self) -> Result<(), ArcweftRustIdentityError> {
        validate_identifier(&self.0)
    }
}

fn validate_package_id(value: &str) -> Result<(), ArcweftRustIdentityError> {
    if value.is_empty() {
        return Err(ArcweftRustIdentityError::EmptyPackageId);
    }
    if let Some((byte, _)) = value
        .char_indices()
        .find(|(_, character)| character.is_control())
    {
        return Err(ArcweftRustIdentityError::PackageControlCharacter { byte });
    }
    Ok(())
}

fn validate_path_segments(
    segments: &[ArcweftRustTypePathSegment],
) -> Result<(), ArcweftRustIdentityError> {
    if segments.is_empty() {
        return Err(ArcweftRustIdentityError::EmptyTypePath);
    }
    if segments.len() > MAX_TYPE_PATH_SEGMENTS {
        return Err(ArcweftRustIdentityError::TypePathSegmentLimit {
            observed: segments.len(),
            maximum: MAX_TYPE_PATH_SEGMENTS,
        });
    }
    for segment in segments {
        segment.validate()?;
    }
    Ok(())
}

fn validate_identifier(value: &str) -> Result<(), ArcweftRustIdentityError> {
    if value.is_empty() {
        return Err(ArcweftRustIdentityError::EmptyIdentifier);
    }
    if value == "_" || value.starts_with("r#") || is_rust_keyword(value) {
        return Err(ArcweftRustIdentityError::InvalidIdentifier {
            value: value.to_owned(),
        });
    }
    let mut characters = value.chars();
    let first = characters.next().expect("non-empty identifier was checked");
    if first != '_' && !unicode_ident::is_xid_start(first) {
        return Err(ArcweftRustIdentityError::InvalidIdentifier {
            value: value.to_owned(),
        });
    }
    if !characters.all(unicode_ident::is_xid_continue) {
        return Err(ArcweftRustIdentityError::InvalidIdentifier {
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn is_rust_keyword(value: &str) -> bool {
    matches!(
        value,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "try"
            | "gen"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_validate_without_lossy_normalization() {
        let package = ArcweftRustPackageId::try_new("truck_game").expect("package ID");
        let segment = ArcweftRustTypePathSegment::try_new("モデル").expect("Unicode identifier");
        let path = ArcweftRustTypePath::try_new([segment]).expect("type path");

        assert_eq!(package.as_str(), "truck_game");
        assert_eq!(path.segments()[0].as_str(), "モデル");
        assert!(ArcweftRustTypePathSegment::try_new("r#type").is_err());
        assert!(ArcweftRustTypePathSegment::try_new("type").is_err());
        assert!(ArcweftRustPackageId::try_new("bad\npackage").is_err());
    }

    #[test]
    fn invalid_package_and_path_identities_fail_with_exact_kinds() {
        assert_eq!(
            ArcweftRustPackageId::try_new(""),
            Err(ArcweftRustIdentityError::EmptyPackageId)
        );
        assert_eq!(
            ArcweftRustPackageId::try_new("bad\npackage"),
            Err(ArcweftRustIdentityError::PackageControlCharacter { byte: 3 })
        );
        assert_eq!(
            ArcweftRustTypePath::try_new([]),
            Err(ArcweftRustIdentityError::EmptyTypePath)
        );
        assert_eq!(
            ArcweftRustTypePathSegment::try_new("type"),
            Err(ArcweftRustIdentityError::InvalidIdentifier {
                value: "type".to_owned(),
            })
        );
    }
}
