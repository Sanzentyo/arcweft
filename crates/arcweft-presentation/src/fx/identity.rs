//! Stable identities and hashes used at every Fx boundary.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

/// Stable identity of one public `#[fx]` declaration.
///
/// Re-exports retain the original package and qualified declaration name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxId {
    package: FxPackageId,
    function: FxQualifiedName,
}

/// Validated package component of an [`FxId`].
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FxPackageId(String);

/// Validated original qualified function component of an [`FxId`].
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FxQualifiedName(String);

/// Stable identity of one applied Fx graph.
#[derive(Clone, Copy, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxInstanceId([u8; 32]);

/// Hash of an Fx function's public parameter and renderer-interface contract.
#[derive(Clone, Copy, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxAbiHash([u8; 32]);

/// Hash of an Fx function's complete typed graph and resource bindings.
#[derive(Clone, Copy, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxSemanticHash([u8; 32]);

/// Invalid stable Fx identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxIdError {
    /// Package identity was omitted.
    #[error("Fx package identity cannot be empty")]
    EmptyPackage,
    /// Package identity contains whitespace, separators, or unsupported punctuation.
    #[error("Fx package identity must contain only letters, digits, `_`, `-`, or `.`")]
    InvalidPackage,
    /// Qualified function identity was omitted or contains an invalid segment.
    #[error("Fx function identity must be a non-empty qualified name")]
    InvalidFunction,
}

impl FxId {
    /// Creates an identity from its canonical package and original declaration.
    pub fn try_new(
        package: impl Into<String>,
        function: impl Into<String>,
    ) -> Result<Self, FxIdError> {
        Ok(Self {
            package: FxPackageId::try_new(package)?,
            function: FxQualifiedName::try_new(function)?,
        })
    }

    /// Derives a collision-resistant identity for an Arcweft-owned typed
    /// builtin whose complete semantics are represented by `semantic_key`.
    pub fn derive_builtin(family: &str, semantic_key: &[u8]) -> Result<Self, FxIdError> {
        let family = FxQualifiedName::try_new(family.to_owned())?;
        let mut hasher = blake3::Hasher::new();
        hash_str(&mut hasher, "arcweft.fx-builtin.v1");
        hash_str(&mut hasher, family.as_str());
        hash_bytes(&mut hasher, semantic_key);
        let digest = hasher.finalize();
        let mut suffix = String::with_capacity(65);
        suffix.push('h');
        for byte in digest.as_bytes() {
            use fmt::Write as _;
            write!(&mut suffix, "{byte:02x}").expect("writing to a String cannot fail");
        }
        Self::try_new("arcweft.builtin", format!("{}.{suffix}", family.as_str()))
    }

    pub fn package(&self) -> &str {
        self.package.as_str()
    }

    pub fn function(&self) -> &str {
        self.function.as_str()
    }
}

impl FxPackageId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, FxIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(FxIdError::EmptyPackage);
        }
        if !value
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-' | '.'))
        {
            return Err(FxIdError::InvalidPackage);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FxQualifiedName {
    pub fn try_new(value: impl Into<String>) -> Result<Self, FxIdError> {
        let value = value.into();
        if value.is_empty() || !value.split('.').all(valid_identifier) {
            return Err(FxIdError::InvalidFunction);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize)]
struct FxIdWire {
    package: String,
    function: String,
}

impl<'de> Deserialize<'de> for FxId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxIdWire::deserialize(deserializer)?;
        Self::try_new(wire.package, wire.function).map_err(D::Error::custom)
    }
}

impl fmt::Display for FxId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}::{}", self.package(), self.function())
    }
}

impl FxInstanceId {
    /// Derives an application identity from its definition and stable owner path.
    pub fn derive<'a>(fx: &FxId, components: impl IntoIterator<Item = &'a str>) -> Self {
        let mut hasher = blake3::Hasher::new();
        hash_str(&mut hasher, "arcweft.fx-instance.v1");
        hash_str(&mut hasher, fx.package());
        hash_str(&mut hasher, fx.function());
        for component in components {
            hash_str(&mut hasher, component);
        }
        Self(*hasher.finalize().as_bytes())
    }

    pub const fn from_bytes(value: [u8; 32]) -> Self {
        Self(value)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl FxAbiHash {
    /// Derives a deterministic ABI hash from canonical external schema parts.
    pub fn derive<'a>(parts: impl IntoIterator<Item = &'a str>) -> Self {
        Self(derive_hash("arcweft.fx-abi.v1", parts))
    }

    pub const fn from_bytes(value: [u8; 32]) -> Self {
        Self(value)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl FxSemanticHash {
    /// Derives a deterministic semantic hash from canonical external parts.
    pub fn derive<'a>(parts: impl IntoIterator<Item = &'a str>) -> Self {
        Self(derive_hash("arcweft.fx-semantic.v1", parts))
    }

    pub const fn from_bytes(value: [u8; 32]) -> Self {
        Self(value)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for FxInstanceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hash(formatter, "FxInstanceId", &self.0)
    }
}

impl fmt::Debug for FxAbiHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hash(formatter, "FxAbiHash", &self.0)
    }
}

impl fmt::Debug for FxSemanticHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hash(formatter, "FxSemanticHash", &self.0)
    }
}

pub(crate) fn hash_bytes(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}

pub(crate) fn hash_str(hasher: &mut blake3::Hasher, value: &str) {
    hash_bytes(hasher, value.as_bytes());
}

pub(crate) fn hash_usize(hasher: &mut blake3::Hasher, value: usize) {
    hasher.update(&(value as u64).to_le_bytes());
}

fn valid_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

fn derive_hash<'a>(domain: &str, parts: impl IntoIterator<Item = &'a str>) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hash_str(&mut hasher, domain);
    for part in parts {
        hash_str(&mut hasher, part);
    }
    *hasher.finalize().as_bytes()
}

fn write_hash(formatter: &mut fmt::Formatter<'_>, label: &str, bytes: &[u8; 32]) -> fmt::Result {
    write!(formatter, "{label}(")?;
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    formatter.write_str(")")
}
