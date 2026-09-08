//! Stable identities and hashes used at every Fx boundary.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use super::canonical::{
    CanonicalEncoder, CanonicalHashSink, CanonicalReader, FxCanonicalDecodeError,
};

pub const FX_MAX_PACKAGE_ID_BYTES: usize = 256;
pub const FX_MAX_PACKAGE_ID_SEGMENTS: usize = 32;
pub const FX_MAX_QUALIFIED_NAME_BYTES: usize = 1_024;
pub const FX_MAX_QUALIFIED_NAME_SEGMENTS: usize = 64;

/// Stable identity of one public `#[fx]` declaration.
///
/// Re-exports retain the original package and qualified declaration name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxId {
    package: FxPackageId,
    function: FxQualifiedName,
}

/// Opaque semantic identity of one validated [`FxId`].
///
/// The digest is issued by the Fx identity owner from its existing canonical
/// encoder.  Consumers therefore cannot accidentally re-encode package or
/// function names with a competing grammar.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FxIdentitySemanticDigest([u8; 32]);

impl FxIdentitySemanticDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Validated package component of an [`FxId`].
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FxPackageId {
    value: String,
    byte_len: u16,
    segment_count: u8,
}

/// Validated original qualified function component of an [`FxId`].
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FxQualifiedName {
    value: String,
    byte_len: u16,
    segment_count: u8,
}

/// Stable identity of one applied Fx graph.
#[derive(Clone, Copy, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxInstanceId([u8; 32]);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxInstanceIdentity {
    definition: FxId,
    owner: FxInstanceOwnerKey,
    authored_ordinal: u32,
    instance: FxInstanceId,
}

/// Owner-issued digest for one typed presentation occurrence.
///
/// The owner of an application (for example the dialogue or View runtime)
/// owns the canonical byte encoding of its typed identity.  Presentation only
/// seals those bytes into this opaque key; it never parses or reconstructs an
/// owner identity from a formatted string.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxInstanceOwnerKey([u8; 32]);

/// Hash of an Fx function's public parameter and renderer-interface contract.
#[derive(Clone, Copy, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxAbiHash([u8; 32]);

/// Hash of an Fx function's complete typed graph and resource bindings.
#[derive(Clone, Copy, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxSemanticHash([u8; 32]);

/// Invalid stable Fx identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxIdError {
    #[error(transparent)]
    Package(#[from] FxPackageIdError),
    #[error(transparent)]
    Function(#[from] FxQualifiedNameError),
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxPackageIdError {
    #[error("Fx package identity cannot be empty")]
    Empty,
    #[error("Fx package identity has {actual} UTF-8 bytes, exceeding the limit of {limit}")]
    TooLong { actual: usize, limit: usize },
    #[error("Fx package identity has {actual} segments, exceeding the limit of {limit}")]
    TooManySegments { actual: usize, limit: usize },
    #[error("Fx package identity contains an empty or invalid segment")]
    InvalidSegment,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxQualifiedNameError {
    #[error("Fx function identity cannot be empty")]
    Empty,
    #[error("Fx function identity has {actual} UTF-8 bytes, exceeding the limit of {limit}")]
    TooLong { actual: usize, limit: usize },
    #[error("Fx function identity has {actual} segments, exceeding the limit of {limit}")]
    TooManySegments { actual: usize, limit: usize },
    #[error("Fx function identity contains an invalid qualified-name segment")]
    InvalidSegment,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxIdCanonicalDecodeError {
    #[error(transparent)]
    Canonical(#[from] FxCanonicalDecodeError),
    #[error(transparent)]
    Identity(#[from] FxIdError),
    #[error("Fx identity {kind} segment count {encoded} does not match decoded count {actual}")]
    SegmentCountMismatch {
        kind: &'static str,
        encoded: u8,
        actual: u8,
    },
    #[error("Fx identity {kind} length {actual} exceeds the owner limit of {limit}")]
    OwnerLimit {
        kind: &'static str,
        actual: u64,
        limit: usize,
    },
    #[error("Fx identity allocation failed")]
    AllocationFailed,
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

    /// Constructs the canonical source identity for an Arcweft-owned builtin
    /// after its owning domain has streamed the complete structural
    /// specialization into `digest`.
    pub(super) fn from_builtin_structural_digest(
        family: &str,
        digest: &[u8; 32],
    ) -> Result<Self, FxIdError> {
        let family = FxQualifiedName::try_new(family.to_owned())?;
        let mut suffix = String::with_capacity(65);
        suffix.push('h');
        for byte in digest {
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

    pub(super) fn encode_canonical_v1<S: super::canonical::CanonicalSink>(
        &self,
        encoder: &mut CanonicalEncoder<S>,
    ) -> Result<(), S::Error> {
        self.package.encode_canonical_v1(encoder)?;
        self.function.encode_canonical_v1(encoder)
    }

    /// Issues the owner-defined semantic identity of this validated Fx
    /// declaration using the canonical v1 encoder and hash sink.
    #[must_use]
    pub fn semantic_digest(&self) -> FxIdentitySemanticDigest {
        let mut hasher = blake3::Hasher::new();
        {
            let mut encoder = CanonicalEncoder::new(CanonicalHashSink::new(&mut hasher));
            let encoding = (|| {
                encoder.domain_v1(b"arcweft.fx-id-semantic")?;
                self.encode_canonical_v1(&mut encoder)
            })();
            match encoding {
                Ok(()) => {}
                Err(error) => match error {},
            }
        }
        FxIdentitySemanticDigest(*hasher.finalize().as_bytes())
    }

    pub(super) fn decode_canonical_v1(
        reader: &mut CanonicalReader<'_>,
    ) -> Result<Self, FxIdCanonicalDecodeError> {
        let package_segments =
            u8::try_from(reader.unsigned()?).map_err(|_| FxCanonicalDecodeError::VarintOverflow)?;
        let package = decode_bounded_identity_string(reader, FX_MAX_PACKAGE_ID_BYTES, "package")?;
        let function_segments =
            u8::try_from(reader.unsigned()?).map_err(|_| FxCanonicalDecodeError::VarintOverflow)?;
        let function =
            decode_bounded_identity_string(reader, FX_MAX_QUALIFIED_NAME_BYTES, "function")?;
        let identity = Self::try_new(package, function)?;
        if identity.package.segment_count() != package_segments {
            return Err(FxIdCanonicalDecodeError::SegmentCountMismatch {
                kind: "package",
                encoded: package_segments,
                actual: identity.package.segment_count(),
            });
        }
        if identity.function.segment_count() != function_segments {
            return Err(FxIdCanonicalDecodeError::SegmentCountMismatch {
                kind: "function",
                encoded: function_segments,
                actual: identity.function.segment_count(),
            });
        }
        Ok(identity)
    }
}

impl FxPackageId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, FxPackageIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(FxPackageIdError::Empty);
        }
        if value.len() > FX_MAX_PACKAGE_ID_BYTES {
            return Err(FxPackageIdError::TooLong {
                actual: value.len(),
                limit: FX_MAX_PACKAGE_ID_BYTES,
            });
        }
        let segment_count = value.split('.').count();
        if segment_count > FX_MAX_PACKAGE_ID_SEGMENTS {
            return Err(FxPackageIdError::TooManySegments {
                actual: segment_count,
                limit: FX_MAX_PACKAGE_ID_SEGMENTS,
            });
        }
        if value.split('.').any(|segment| {
            segment.is_empty()
                || !segment
                    .chars()
                    .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-'))
        }) {
            return Err(FxPackageIdError::InvalidSegment);
        }
        let byte_len = u16::try_from(value.len()).map_err(|_| FxPackageIdError::TooLong {
            actual: value.len(),
            limit: FX_MAX_PACKAGE_ID_BYTES,
        })?;
        let segment_count =
            u8::try_from(segment_count).map_err(|_| FxPackageIdError::TooManySegments {
                actual: segment_count,
                limit: FX_MAX_PACKAGE_ID_SEGMENTS,
            })?;
        Ok(Self {
            value,
            byte_len,
            segment_count,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub const fn byte_len(&self) -> u16 {
        self.byte_len
    }

    pub const fn segment_count(&self) -> u8 {
        self.segment_count
    }

    fn encode_canonical_v1<S: super::canonical::CanonicalSink>(
        &self,
        encoder: &mut CanonicalEncoder<S>,
    ) -> Result<(), S::Error> {
        encoder.unsigned(u64::from(self.segment_count))?;
        encoder.unsigned(u64::from(self.byte_len))?;
        encoder.raw_bytes(self.value.as_bytes())
    }
}

impl FxQualifiedName {
    pub fn try_new(value: impl Into<String>) -> Result<Self, FxQualifiedNameError> {
        let value = value.into();
        if value.is_empty() {
            return Err(FxQualifiedNameError::Empty);
        }
        if value.len() > FX_MAX_QUALIFIED_NAME_BYTES {
            return Err(FxQualifiedNameError::TooLong {
                actual: value.len(),
                limit: FX_MAX_QUALIFIED_NAME_BYTES,
            });
        }
        let segment_count = value.split('.').count();
        if segment_count > FX_MAX_QUALIFIED_NAME_SEGMENTS {
            return Err(FxQualifiedNameError::TooManySegments {
                actual: segment_count,
                limit: FX_MAX_QUALIFIED_NAME_SEGMENTS,
            });
        }
        if value.split('.').any(|segment| !valid_identifier(segment)) {
            return Err(FxQualifiedNameError::InvalidSegment);
        }
        let byte_len = u16::try_from(value.len()).map_err(|_| FxQualifiedNameError::TooLong {
            actual: value.len(),
            limit: FX_MAX_QUALIFIED_NAME_BYTES,
        })?;
        let segment_count =
            u8::try_from(segment_count).map_err(|_| FxQualifiedNameError::TooManySegments {
                actual: segment_count,
                limit: FX_MAX_QUALIFIED_NAME_SEGMENTS,
            })?;
        Ok(Self {
            value,
            byte_len,
            segment_count,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub const fn byte_len(&self) -> u16 {
        self.byte_len
    }

    pub const fn segment_count(&self) -> u8 {
        self.segment_count
    }

    fn encode_canonical_v1<S: super::canonical::CanonicalSink>(
        &self,
        encoder: &mut CanonicalEncoder<S>,
    ) -> Result<(), S::Error> {
        encoder.unsigned(u64::from(self.segment_count))?;
        encoder.unsigned(u64::from(self.byte_len))?;
        encoder.raw_bytes(self.value.as_bytes())
    }
}

impl Serialize for FxPackageId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.value)
    }
}

impl<'de> Deserialize<'de> for FxPackageId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl Serialize for FxQualifiedName {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.value)
    }
}

impl<'de> Deserialize<'de> for FxQualifiedName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_new(String::deserialize(deserializer)?).map_err(D::Error::custom)
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
    /// Derives an application identity from its definition, typed owner key,
    /// and source-authored occurrence ordinal.
    pub fn derive(fx: &FxId, owner_key: FxInstanceOwnerKey, authored_ordinal: u32) -> Self {
        let mut hasher = blake3::Hasher::new();
        {
            let mut encoder = CanonicalEncoder::new(CanonicalHashSink::new(&mut hasher));
            let encoding = (|| {
                encoder.domain_v1(b"arcweft.fx-instance")?;
                fx.encode_canonical_v1(&mut encoder)?;
                encoder.digest32(owner_key.as_bytes())?;
                encoder.unsigned(u64::from(authored_ordinal))
            })();
            match encoding {
                Ok(()) => {}
                Err(error) => match error {},
            }
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

impl FxInstanceIdentity {
    pub fn new(definition: &FxId, owner: FxInstanceOwnerKey, authored_ordinal: u32) -> Self {
        Self {
            definition: definition.clone(),
            owner,
            authored_ordinal,
            instance: FxInstanceId::derive(definition, owner, authored_ordinal),
        }
    }

    pub const fn definition(&self) -> &FxId {
        &self.definition
    }

    pub const fn owner(&self) -> FxInstanceOwnerKey {
        self.owner
    }

    pub const fn authored_ordinal(&self) -> u32 {
        self.authored_ordinal
    }

    pub const fn instance(&self) -> FxInstanceId {
        self.instance
    }
}

#[derive(Deserialize)]
struct FxInstanceIdentityWire {
    definition: FxId,
    owner: FxInstanceOwnerKey,
    authored_ordinal: u32,
    instance: FxInstanceId,
}

impl<'de> Deserialize<'de> for FxInstanceIdentity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = FxInstanceIdentityWire::deserialize(deserializer)?;
        let identity = Self::new(&wire.definition, wire.owner, wire.authored_ordinal);
        if identity.instance != wire.instance {
            return Err(D::Error::custom(
                "Fx instance identity does not match its definition, owner, and authored ordinal",
            ));
        }
        Ok(identity)
    }
}

impl FxInstanceOwnerKey {
    /// Seals a dialogue-owned canonical encoding into an opaque key.
    ///
    /// The domain discriminator is part of the digest input, so a dialogue
    /// occurrence can never alias a View occurrence that happens to use the
    /// same canonical fields.
    pub fn from_dialogue_canonical_bytes(bytes: &[u8]) -> Self {
        Self::from_domain_canonical_bytes(0, bytes)
    }

    /// Seals a View-owned canonical encoding into an opaque key.
    ///
    /// The domain discriminator is part of the digest input, so a View
    /// occurrence can never alias a dialogue occurrence that happens to use
    /// the same canonical fields.
    pub fn from_view_canonical_bytes(bytes: &[u8]) -> Self {
        Self::from_domain_canonical_bytes(1, bytes)
    }

    fn from_domain_canonical_bytes(domain: u8, bytes: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hash_str(&mut hasher, "arcweft.fx-instance-owner.v1");
        hasher.update(&[domain]);
        hash_bytes(&mut hasher, bytes);
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
    pub const fn from_bytes(value: [u8; 32]) -> Self {
        Self(value)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl FxSemanticHash {
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

fn valid_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

fn decode_bounded_identity_string(
    reader: &mut CanonicalReader<'_>,
    limit: usize,
    kind: &'static str,
) -> Result<String, FxIdCanonicalDecodeError> {
    let encoded_len = reader.unsigned()?;
    if encoded_len > u64::try_from(limit).map_err(|_| FxCanonicalDecodeError::LengthOverflow)? {
        return Err(FxIdCanonicalDecodeError::OwnerLimit {
            kind,
            actual: encoded_len,
            limit,
        });
    }
    let length =
        usize::try_from(encoded_len).map_err(|_| FxCanonicalDecodeError::LengthOverflow)?;
    let bytes = reader.raw_bytes(length)?;
    let value = std::str::from_utf8(bytes).map_err(|_| FxCanonicalDecodeError::InvalidUtf8)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(length)
        .map_err(|_| FxIdCanonicalDecodeError::AllocationFailed)?;
    owned.push_str(value);
    Ok(owned)
}

fn write_hash(formatter: &mut fmt::Formatter<'_>, label: &str, bytes: &[u8; 32]) -> fmt::Result {
    write!(formatter, "{label}(")?;
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    formatter.write_str(")")
}
