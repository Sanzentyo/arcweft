//! AWFB v1 product container codec.

mod identity;
mod opaque;

pub use identity::ArtifactIdentity;
pub use opaque::{DecodedSectionKind, SectionKindCode};

use std::collections::BTreeSet;
use std::fmt::Write as _;
use thiserror::Error;

pub const MAGIC: [u8; 8] = *b"AWFB\r\n\x1a\n";
pub const CONTAINER_VERSION: u32 = 1;
pub const HEADER_SIZE: usize = 160;
pub const SECTION_INDEX_ENTRY_SIZE: usize = 160;
pub const PAYLOAD_ALIGNMENT: usize = 16;

const MANIFEST_DIGEST_OFFSET: usize = 96;
const INDEX_DIGEST_OFFSET: usize = 128;
const SIGNATURE_OFFSET_FIELD_OFFSET: usize = 56;
const SIGNATURE_LEN_FIELD_OFFSET: usize = 64;
const FILE_LEN_FIELD_OFFSET: usize = 72;
const REQUIRED_PROGRAM_SECTIONS: [BundleSectionKind; 5] = [
    BundleSectionKind::ProgramBytecode,
    BundleSectionKind::RuntimeTypes,
    BundleSectionKind::Entrypoints,
    BundleSectionKind::AdapterRequirements,
    BundleSectionKind::ContentCatalog,
];
const REQUIRED_PATCH_SECTIONS: [BundleSectionKind; 1] = [BundleSectionKind::PatchPlan];

#[derive(
    Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct BundleDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleKind {
    Program,
    AgentController,
    ContentPack,
    Patch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleSectionKind {
    ProgramBytecode,
    RuntimeTypes,
    Entrypoints,
    AdapterRequirements,
    ContentCatalog,
    DisplayCatalog,
    AudioGraph,
    AssetCatalog,
    AssetBlob,
    LocaleCatalog,
    SourceMap,
    DebugSymbols,
    NormalizedSource,
    HotSwapMap,
    PatchPlan,
    ViewProgram,
    ViewStyle,
    ViewText,
    ViewInput,
    ViewTheme,
    FxDefinitions,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentResidency {
    #[default]
    Startup,
    OnDemand,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentPlacement {
    #[default]
    Embedded,
    External,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Compression {
    #[default]
    None,
    Zstd,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("unsupported AWFB {kind} value `{value}`")]
pub struct ContainerEnumParseError {
    kind: &'static str,
    value: String,
}

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct SectionId([u8; 16]);

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SectionDescriptor {
    id: SectionId,
    kind: SectionKindCode,
    schema_version: u32,
    residency: ContentResidency,
    placement: ContentPlacement,
    compression: Compression,
    offset: u64,
    stored_size: u64,
    decoded_size: u64,
    stored_digest: BundleDigest,
    content_digest: BundleDigest,
    required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionInput {
    id: SectionId,
    kind: SectionKindCode,
    schema_version: u32,
    residency: ContentResidency,
    placement: ContentPlacement,
    compression: Compression,
    required: bool,
    stored_size: u64,
    decoded_size: u64,
    stored_digest: BundleDigest,
    content_digest: BundleDigest,
    stored_bytes: Vec<u8>,
}

/// Fetched bytes for one external AWFB section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalSectionPayload {
    id: SectionId,
    bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadBudget {
    file_size: u64,
    section_count: usize,
    embedded_bytes: u64,
    decoded_bytes: u64,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ContainerError {
    #[error("AWFB container is truncated")]
    Truncated,
    #[error("AWFB container magic does not match")]
    BadMagic,
    #[error("unsupported AWFB container version {0}")]
    UnsupportedVersion(u32),
    #[error("unsupported AWFB header size {0}")]
    UnsupportedHeaderSize(u32),
    #[error("unknown AWFB bundle kind {0}")]
    UnknownBundleKind(u32),
    #[error("unknown required AWFB section kind {0}")]
    UnknownRequiredSectionKind(u32),
    #[error("unknown AWFB section kind {0}")]
    UnknownSectionKind(u32),
    #[error("invalid AWFB content residency {0}")]
    InvalidResidency(u8),
    #[error("invalid AWFB content placement {0}")]
    InvalidPlacement(u8),
    #[error("invalid AWFB compression {0}")]
    InvalidCompression(u8),
    #[error("failed to compress AWFB zstd section {section}: {message}")]
    CompressZstd { section: SectionId, message: String },
    #[error("failed to decompress AWFB zstd section {section}: {message}")]
    DecompressZstd { section: SectionId, message: String },
    #[error("duplicate AWFB section id {0}")]
    DuplicateSection(SectionId),
    #[error("section {section:?} is not allowed in {bundle:?} AWFB bundles")]
    DisallowedSection {
        bundle: BundleKind,
        section: BundleSectionKind,
    },
    #[error("program AWFB bundle is missing required section {0:?}")]
    MissingRequiredSection(BundleSectionKind),
    #[error("external AWFB section {0} must not have an embedded payload range")]
    ExternalSectionHasPayload(SectionId),
    #[error("AWFB container range is out of bounds")]
    Bounds,
    #[error("AWFB section {0} overlaps an earlier range")]
    OverlappingSection(SectionId),
    #[error("AWFB signature block overlaps another range")]
    OverlappingSignature,
    #[error("AWFB container already has a signature block")]
    SignatureAlreadyPresent,
    #[error("AWFB signature block must not be empty")]
    EmptySignature,
    #[error("AWFB signature block must be trailing to compute signing digest")]
    NonTrailingSignature,
    #[error("AWFB stored digest mismatch for section {0}")]
    StoredDigestMismatch(SectionId),
    #[error("AWFB content digest mismatch for section {0}")]
    ContentDigestMismatch(SectionId),
    #[error("AWFB manifest digest mismatch")]
    ManifestDigestMismatch,
    #[error("AWFB section index digest mismatch")]
    IndexDigestMismatch,
    #[error("AWFB file length does not match header")]
    FileLengthMismatch,
    #[error("AWFB read budget exceeded")]
    BudgetExceeded,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ExternalSectionPayloadError {
    #[error("failed to decode AWFB section payload: {0}")]
    Container(#[from] ContainerError),
    #[error("external section {0} is missing its fetched payload")]
    MissingPayload(SectionId),
    #[error("duplicate external section payload {0}")]
    DuplicatePayload(SectionId),
    #[error("external section {id} decoded size mismatch: expected {expected}, actual {actual}")]
    SizeMismatch {
        id: SectionId,
        expected: u64,
        actual: u64,
    },
    #[error("external section {id} content digest mismatch: expected {expected}, actual {actual}")]
    ContentDigestMismatch {
        id: SectionId,
        expected: BundleDigest,
        actual: BundleDigest,
    },
}

#[derive(Clone, Debug)]
pub struct BundleView<'a> {
    kind: BundleKind,
    manifest: &'a [u8],
    signature: Option<&'a [u8]>,
    signature_range: Option<(usize, usize)>,
    sections: Vec<SectionDescriptor>,
    skipped_optional_sections: usize,
    bytes: &'a [u8],
    budget: ReadBudget,
}

#[derive(Clone, Copy, Debug)]
struct Header {
    kind: BundleKind,
    manifest_offset: u64,
    manifest_len: u64,
    index_offset: u64,
    index_len: u64,
    signature_offset: u64,
    signature_len: u64,
    file_len: u64,
    section_count: usize,
    manifest_digest: BundleDigest,
    index_digest: BundleDigest,
}

enum DecodedDescriptor {
    Descriptor(SectionDescriptor),
}

impl BundleDigest {
    pub const ZERO: Self = Self([0; 32]);

    pub fn of(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn to_hex(self) -> String {
        self.0
            .iter()
            .fold(String::with_capacity(64), |mut hex, byte| {
                write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
                hex
            })
    }
}

impl std::fmt::Debug for BundleDigest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl std::fmt::Display for BundleDigest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl BundleKind {
    pub const fn encoded(self) -> u32 {
        match self {
            Self::Program => 1,
            Self::AgentController => 2,
            Self::ContentPack => 3,
            Self::Patch => 4,
        }
    }

    pub const fn from_encoded(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::Program),
            2 => Some(Self::AgentController),
            3 => Some(Self::ContentPack),
            4 => Some(Self::Patch),
            _ => None,
        }
    }

    pub const fn allows_section(self, kind: BundleSectionKind) -> bool {
        match self {
            Self::ContentPack => !kind.is_executable(),
            Self::Patch => matches!(
                kind,
                BundleSectionKind::PatchPlan | BundleSectionKind::AssetBlob
            ),
            Self::Program | Self::AgentController => true,
        }
    }
}

impl BundleSectionKind {
    pub const fn encoded(self) -> u32 {
        match self {
            Self::ProgramBytecode => 1,
            Self::RuntimeTypes => 2,
            Self::Entrypoints => 3,
            Self::AdapterRequirements => 4,
            Self::ContentCatalog => 5,
            Self::DisplayCatalog => 6,
            Self::AudioGraph => 7,
            Self::AssetCatalog => 8,
            Self::AssetBlob => 9,
            Self::LocaleCatalog => 10,
            Self::SourceMap => 11,
            Self::DebugSymbols => 12,
            Self::NormalizedSource => 13,
            Self::HotSwapMap => 14,
            Self::PatchPlan => 15,
            Self::ViewProgram => 16,
            Self::ViewStyle => 17,
            Self::ViewText => 18,
            Self::ViewInput => 19,
            Self::ViewTheme => 20,
            Self::FxDefinitions => 21,
        }
    }

    pub const fn from_encoded(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::ProgramBytecode),
            2 => Some(Self::RuntimeTypes),
            3 => Some(Self::Entrypoints),
            4 => Some(Self::AdapterRequirements),
            5 => Some(Self::ContentCatalog),
            6 => Some(Self::DisplayCatalog),
            7 => Some(Self::AudioGraph),
            8 => Some(Self::AssetCatalog),
            9 => Some(Self::AssetBlob),
            10 => Some(Self::LocaleCatalog),
            11 => Some(Self::SourceMap),
            12 => Some(Self::DebugSymbols),
            13 => Some(Self::NormalizedSource),
            14 => Some(Self::HotSwapMap),
            15 => Some(Self::PatchPlan),
            16 => Some(Self::ViewProgram),
            17 => Some(Self::ViewStyle),
            18 => Some(Self::ViewText),
            19 => Some(Self::ViewInput),
            20 => Some(Self::ViewTheme),
            21 => Some(Self::FxDefinitions),
            _ => None,
        }
    }

    pub const fn is_executable(self) -> bool {
        matches!(
            self,
            Self::ProgramBytecode | Self::RuntimeTypes | Self::Entrypoints | Self::FxDefinitions
        )
    }

    pub const fn is_program_required(self) -> bool {
        matches!(
            self,
            Self::ProgramBytecode
                | Self::RuntimeTypes
                | Self::Entrypoints
                | Self::AdapterRequirements
                | Self::ContentCatalog
        )
    }

    pub const fn default_residency(self) -> ContentResidency {
        match self {
            Self::ProgramBytecode
            | Self::RuntimeTypes
            | Self::Entrypoints
            | Self::AdapterRequirements
            | Self::ContentCatalog
            | Self::DisplayCatalog
            | Self::AudioGraph
            | Self::AssetCatalog
            | Self::LocaleCatalog
            | Self::HotSwapMap
            | Self::PatchPlan
            | Self::ViewProgram
            | Self::ViewStyle
            | Self::ViewInput
            | Self::ViewTheme
            | Self::FxDefinitions => ContentResidency::Startup,
            Self::AssetBlob
            | Self::SourceMap
            | Self::DebugSymbols
            | Self::NormalizedSource
            | Self::ViewText => ContentResidency::OnDemand,
        }
    }

    /// Default patch compatibility for a section kind when no migrated codec can
    /// derive a more precise semantic fingerprint from section bytes.
    pub const fn patch_default_compatibility(self) -> crate::patch::PatchCompatibility {
        match self {
            Self::RuntimeTypes
            | Self::Entrypoints
            | Self::AdapterRequirements
            | Self::PatchPlan
            | Self::ViewInput => crate::patch::PatchCompatibility::RestartRequired,
            Self::ProgramBytecode => crate::patch::PatchCompatibility::CodeCompatible,
            Self::HotSwapMap | Self::FxDefinitions => {
                crate::patch::PatchCompatibility::CodeGenerational
            }
            Self::ContentCatalog
            | Self::DisplayCatalog
            | Self::AudioGraph
            | Self::AssetCatalog
            | Self::AssetBlob
            | Self::LocaleCatalog
            | Self::SourceMap
            | Self::DebugSymbols
            | Self::NormalizedSource
            | Self::ViewProgram
            | Self::ViewStyle
            | Self::ViewText
            | Self::ViewTheme => crate::patch::PatchCompatibility::ContentOnly,
        }
    }
}

impl ContentResidency {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::OnDemand => "on-demand",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ContainerEnumParseError> {
        match normalized_policy_value(value).as_str() {
            "startup" => Ok(Self::Startup),
            "on-demand" => Ok(Self::OnDemand),
            _ => Err(ContainerEnumParseError::new("content residency", value)),
        }
    }

    pub const fn encoded(self) -> u8 {
        match self {
            Self::Startup => 1,
            Self::OnDemand => 2,
        }
    }

    pub const fn from_encoded(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Startup),
            2 => Some(Self::OnDemand),
            _ => None,
        }
    }

    pub const fn must_be_ready_before_entry(self) -> bool {
        matches!(self, Self::Startup)
    }
}

impl std::fmt::Display for ContentResidency {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for ContentResidency {
    type Err = ContainerEnumParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl ContentPlacement {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::External => "external",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ContainerEnumParseError> {
        match normalized_policy_value(value).as_str() {
            "embedded" => Ok(Self::Embedded),
            "external" => Ok(Self::External),
            _ => Err(ContainerEnumParseError::new("content placement", value)),
        }
    }

    pub const fn encoded(self) -> u8 {
        match self {
            Self::Embedded => 1,
            Self::External => 2,
        }
    }

    pub const fn from_encoded(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Embedded),
            2 => Some(Self::External),
            _ => None,
        }
    }

    pub const fn is_embedded(self) -> bool {
        matches!(self, Self::Embedded)
    }
}

impl std::fmt::Display for ContentPlacement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for ContentPlacement {
    type Err = ContainerEnumParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Compression {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Zstd => "zstd",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ContainerEnumParseError> {
        match normalized_policy_value(value).as_str() {
            "none" => Ok(Self::None),
            "zstd" => Ok(Self::Zstd),
            _ => Err(ContainerEnumParseError::new("compression", value)),
        }
    }

    pub const fn encoded(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Zstd => 1,
        }
    }

    pub const fn from_encoded(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::Zstd),
            _ => None,
        }
    }
}

impl std::fmt::Display for Compression {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for Compression {
    type Err = ContainerEnumParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl ContainerEnumParseError {
    fn new(kind: &'static str, value: &str) -> Self {
        Self {
            kind,
            value: value.to_owned(),
        }
    }
}

fn normalized_policy_value(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('_', "-")
}

impl SectionId {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl std::fmt::Display for SectionId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl SectionDescriptor {
    pub const fn id(&self) -> SectionId {
        self.id
    }

    /// Returns the known section kind.
    ///
    /// # Panics
    ///
    /// Panics when this descriptor carries an unknown optional section kind.
    /// Use [`Self::known_kind`] or [`Self::kind_code`] when preserving opaque
    /// sections.
    pub const fn kind(&self) -> BundleSectionKind {
        self.known_kind()
            .expect("SectionDescriptor::kind called for unknown optional section")
    }

    pub const fn known_kind(&self) -> Option<BundleSectionKind> {
        self.kind.known()
    }

    pub const fn kind_code(&self) -> SectionKindCode {
        self.kind
    }

    pub const fn decoded_kind(&self) -> DecodedSectionKind {
        match self.known_kind() {
            Some(kind) => DecodedSectionKind::Known(kind),
            None => DecodedSectionKind::UnknownOptional(self.kind),
        }
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub const fn residency(&self) -> ContentResidency {
        self.residency
    }

    pub const fn placement(&self) -> ContentPlacement {
        self.placement
    }

    pub const fn compression(&self) -> Compression {
        self.compression
    }

    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub const fn stored_size(&self) -> u64 {
        self.stored_size
    }

    pub const fn decoded_size(&self) -> u64 {
        self.decoded_size
    }

    pub const fn stored_digest(&self) -> BundleDigest {
        self.stored_digest
    }

    pub const fn content_digest(&self) -> BundleDigest {
        self.content_digest
    }

    pub const fn required(&self) -> bool {
        self.required
    }
}

impl SectionInput {
    pub fn embedded(
        id: SectionId,
        kind: BundleSectionKind,
        schema_version: u32,
        residency: ContentResidency,
        required: bool,
        decoded_bytes: impl Into<Vec<u8>>,
    ) -> Self {
        let decoded_bytes = decoded_bytes.into();
        let size = u64::try_from(decoded_bytes.len()).unwrap_or(u64::MAX);
        let digest = BundleDigest::of(&decoded_bytes);
        Self {
            id,
            kind: kind.into(),
            schema_version,
            residency,
            placement: ContentPlacement::Embedded,
            compression: Compression::None,
            required,
            stored_size: size,
            decoded_size: size,
            stored_digest: digest,
            content_digest: digest,
            stored_bytes: decoded_bytes,
        }
    }

    pub fn embedded_zstd(
        id: SectionId,
        kind: BundleSectionKind,
        schema_version: u32,
        residency: ContentResidency,
        required: bool,
        decoded_bytes: impl AsRef<[u8]>,
    ) -> Result<Self, ContainerError> {
        let decoded_bytes = decoded_bytes.as_ref();
        let stored_bytes = zstd::bulk::compress(decoded_bytes, 0).map_err(|error| {
            ContainerError::CompressZstd {
                section: id,
                message: error.to_string(),
            }
        })?;
        let stored_size = u64::try_from(stored_bytes.len()).map_err(|_| ContainerError::Bounds)?;
        let decoded_size =
            u64::try_from(decoded_bytes.len()).map_err(|_| ContainerError::Bounds)?;
        Ok(Self {
            id,
            kind: kind.into(),
            schema_version,
            residency,
            placement: ContentPlacement::Embedded,
            compression: Compression::Zstd,
            required,
            stored_size,
            decoded_size,
            stored_digest: BundleDigest::of(&stored_bytes),
            content_digest: BundleDigest::of(decoded_bytes),
            stored_bytes,
        })
    }

    pub fn external(
        id: SectionId,
        kind: BundleSectionKind,
        schema_version: u32,
        residency: ContentResidency,
        required: bool,
    ) -> Self {
        Self::external_ref(
            id,
            kind,
            schema_version,
            residency,
            required,
            0,
            BundleDigest::ZERO,
        )
    }

    pub fn external_ref(
        id: SectionId,
        kind: BundleSectionKind,
        schema_version: u32,
        residency: ContentResidency,
        required: bool,
        decoded_size: u64,
        content_digest: BundleDigest,
    ) -> Self {
        Self {
            id,
            kind: kind.into(),
            schema_version,
            residency,
            placement: ContentPlacement::External,
            compression: Compression::None,
            required,
            stored_size: decoded_size,
            decoded_size,
            stored_digest: content_digest,
            content_digest,
            stored_bytes: Vec::new(),
        }
    }

    pub const fn id(&self) -> SectionId {
        self.id
    }

    /// Returns the known section kind.
    ///
    /// # Panics
    ///
    /// Panics when this input carries an unknown optional section kind. Use
    /// [`Self::known_kind`] or [`Self::kind_code`] when preserving opaque
    /// sections.
    pub const fn kind(&self) -> BundleSectionKind {
        self.known_kind()
            .expect("SectionInput::kind called for unknown optional section")
    }

    pub const fn known_kind(&self) -> Option<BundleSectionKind> {
        self.kind.known()
    }

    pub const fn kind_code(&self) -> SectionKindCode {
        self.kind
    }

    pub(crate) const fn content_digest(&self) -> BundleDigest {
        self.content_digest
    }

    pub fn embedded_unknown_optional(
        id: SectionId,
        kind: SectionKindCode,
        schema_version: u32,
        residency: ContentResidency,
        decoded_bytes: impl Into<Vec<u8>>,
    ) -> Self {
        let decoded_bytes = decoded_bytes.into();
        let size = u64::try_from(decoded_bytes.len()).unwrap_or(u64::MAX);
        let digest = BundleDigest::of(&decoded_bytes);
        Self {
            id,
            kind,
            schema_version,
            residency,
            placement: ContentPlacement::Embedded,
            compression: Compression::None,
            required: false,
            stored_size: size,
            decoded_size: size,
            stored_digest: digest,
            content_digest: digest,
            stored_bytes: decoded_bytes,
        }
    }

    pub fn embedded_raw_optional(
        id: SectionId,
        kind: SectionKindCode,
        schema_version: u32,
        residency: ContentResidency,
        required: bool,
        decoded_bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, ContainerError> {
        if required && kind.known().is_none() {
            return Err(ContainerError::UnknownRequiredSectionKind(kind.encoded()));
        }
        let decoded_bytes = decoded_bytes.into();
        let size = u64::try_from(decoded_bytes.len()).unwrap_or(u64::MAX);
        let digest = BundleDigest::of(&decoded_bytes);
        Ok(Self {
            id,
            kind,
            schema_version,
            residency,
            placement: ContentPlacement::Embedded,
            compression: Compression::None,
            required,
            stored_size: size,
            decoded_size: size,
            stored_digest: digest,
            content_digest: digest,
            stored_bytes: decoded_bytes,
        })
    }

    pub fn embedded_raw_optional_zstd(
        id: SectionId,
        kind: SectionKindCode,
        schema_version: u32,
        residency: ContentResidency,
        required: bool,
        decoded_bytes: impl AsRef<[u8]>,
    ) -> Result<Self, ContainerError> {
        if required && kind.known().is_none() {
            return Err(ContainerError::UnknownRequiredSectionKind(kind.encoded()));
        }
        let decoded_bytes = decoded_bytes.as_ref();
        let stored_bytes = zstd::bulk::compress(decoded_bytes, 0).map_err(|error| {
            ContainerError::CompressZstd {
                section: id,
                message: error.to_string(),
            }
        })?;
        let stored_size = u64::try_from(stored_bytes.len()).map_err(|_| ContainerError::Bounds)?;
        let decoded_size =
            u64::try_from(decoded_bytes.len()).map_err(|_| ContainerError::Bounds)?;
        Ok(Self {
            id,
            kind,
            schema_version,
            residency,
            placement: ContentPlacement::Embedded,
            compression: Compression::Zstd,
            required,
            stored_size,
            decoded_size,
            stored_digest: BundleDigest::of(&stored_bytes),
            content_digest: BundleDigest::of(decoded_bytes),
            stored_bytes,
        })
    }

    pub fn external_unknown_optional_ref(
        id: SectionId,
        kind: SectionKindCode,
        schema_version: u32,
        residency: ContentResidency,
        decoded_size: u64,
        content_digest: BundleDigest,
    ) -> Self {
        Self {
            id,
            kind,
            schema_version,
            residency,
            placement: ContentPlacement::External,
            compression: Compression::None,
            required: false,
            stored_size: decoded_size,
            decoded_size,
            stored_digest: content_digest,
            content_digest,
            stored_bytes: Vec::new(),
        }
    }

    pub fn external_raw_optional_ref(
        id: SectionId,
        kind: SectionKindCode,
        schema_version: u32,
        residency: ContentResidency,
        required: bool,
        decoded_size: u64,
        content_digest: BundleDigest,
    ) -> Result<Self, ContainerError> {
        if required && kind.known().is_none() {
            return Err(ContainerError::UnknownRequiredSectionKind(kind.encoded()));
        }
        Ok(Self {
            id,
            kind,
            schema_version,
            residency,
            placement: ContentPlacement::External,
            compression: Compression::None,
            required,
            stored_size: decoded_size,
            decoded_size,
            stored_digest: content_digest,
            content_digest,
            stored_bytes: Vec::new(),
        })
    }

    pub fn stored_bytes(&self) -> &[u8] {
        &self.stored_bytes
    }
}

impl ExternalSectionPayload {
    pub fn new(id: SectionId, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            id,
            bytes: bytes.into(),
        }
    }

    pub const fn id(&self) -> SectionId {
        self.id
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Default for ReadBudget {
    fn default() -> Self {
        Self {
            file_size: 256 * 1024 * 1024,
            section_count: 4096,
            embedded_bytes: 64 * 1024 * 1024,
            decoded_bytes: 64 * 1024 * 1024,
        }
    }
}

impl ReadBudget {
    pub const fn new(file_size: u64, section_count: usize, embedded_bytes: u64) -> Self {
        Self {
            file_size,
            section_count,
            embedded_bytes,
            decoded_bytes: embedded_bytes,
        }
    }

    #[must_use]
    pub const fn with_decoded_bytes(mut self, decoded_bytes: u64) -> Self {
        self.decoded_bytes = decoded_bytes;
        self
    }
}

impl<'a> BundleView<'a> {
    pub fn parse(bytes: &'a [u8], budget: ReadBudget) -> Result<Self, ContainerError> {
        let header = decode_header(bytes, budget)?;
        let manifest = checked_slice(bytes, header.manifest_offset, header.manifest_len)?;
        let index = checked_slice(bytes, header.index_offset, header.index_len)?;
        if BundleDigest::of(manifest) != header.manifest_digest {
            return Err(ContainerError::ManifestDigestMismatch);
        }
        if BundleDigest::of(index) != header.index_digest {
            return Err(ContainerError::IndexDigestMismatch);
        }
        if index.len()
            != header
                .section_count
                .checked_mul(SECTION_INDEX_ENTRY_SIZE)
                .ok_or(ContainerError::Bounds)?
        {
            return Err(ContainerError::Bounds);
        }

        let mut sections = Vec::with_capacity(header.section_count);
        let skipped_optional_sections = 0_usize;
        let mut seen = BTreeSet::new();
        let mut occupied_ranges = vec![
            (0_usize, HEADER_SIZE),
            checked_range(header.manifest_offset, header.manifest_len)?,
            checked_range(header.index_offset, header.index_len)?,
        ];
        let (signature, signature_range) = if header.signature_len == 0 {
            (None, None)
        } else {
            let range = checked_range(header.signature_offset, header.signature_len)?;
            if ranges_overlap_any(range, &occupied_ranges) {
                return Err(ContainerError::OverlappingSignature);
            }
            occupied_ranges.push(range);
            (
                Some(checked_slice(
                    bytes,
                    header.signature_offset,
                    header.signature_len,
                )?),
                Some(range),
            )
        };

        for entry in index.chunks_exact(SECTION_INDEX_ENTRY_SIZE) {
            match decode_descriptor(entry)? {
                DecodedDescriptor::Descriptor(descriptor) => {
                    if !seen.insert(descriptor.id) {
                        return Err(ContainerError::DuplicateSection(descriptor.id));
                    }
                    validate_descriptor(header.kind, &descriptor, budget)?;
                    if descriptor.placement.is_embedded() {
                        let range = checked_range(descriptor.offset, descriptor.stored_size)?;
                        if ranges_overlap_any(range, &occupied_ranges) {
                            return Err(ContainerError::OverlappingSection(descriptor.id));
                        }
                        let payload =
                            checked_slice(bytes, descriptor.offset, descriptor.stored_size)?;
                        if BundleDigest::of(payload) != descriptor.stored_digest {
                            return Err(ContainerError::StoredDigestMismatch(descriptor.id));
                        }
                        let decoded = decode_payload(&descriptor, payload, budget)?;
                        if BundleDigest::of(&decoded) != descriptor.content_digest {
                            return Err(ContainerError::ContentDigestMismatch(descriptor.id));
                        }
                        occupied_ranges.push(range);
                    }
                    sections.push(descriptor);
                }
            }
        }
        validate_required_sections(header.kind, &sections)?;

        Ok(Self {
            kind: header.kind,
            manifest,
            signature,
            signature_range,
            sections,
            skipped_optional_sections,
            bytes,
            budget,
        })
    }

    pub const fn kind(&self) -> BundleKind {
        self.kind
    }

    pub const fn manifest(&self) -> &'a [u8] {
        self.manifest
    }

    pub const fn signature(&self) -> Option<&'a [u8]> {
        self.signature
    }

    pub fn signing_digest(&self) -> Result<BundleDigest, ContainerError> {
        let Some((signature_start, signature_end)) = self.signature_range else {
            return Ok(BundleDigest::of(self.bytes));
        };
        if signature_end != self.bytes.len() {
            return Err(ContainerError::NonTrailingSignature);
        }
        let mut canonical = self.bytes[..signature_start].to_vec();
        write_u64(&mut canonical, SIGNATURE_OFFSET_FIELD_OFFSET, 0);
        write_u64(&mut canonical, SIGNATURE_LEN_FIELD_OFFSET, 0);
        write_u64(
            &mut canonical,
            FILE_LEN_FIELD_OFFSET,
            u64::try_from(signature_start).unwrap_or(u64::MAX),
        );
        Ok(BundleDigest::of(&canonical))
    }

    pub fn sections(&self) -> &[SectionDescriptor] {
        &self.sections
    }

    pub const fn skipped_optional_sections(&self) -> usize {
        self.skipped_optional_sections
    }

    pub fn embedded_section(&self, id: SectionId) -> Result<Option<&'a [u8]>, ContainerError> {
        let Some(section) = self.sections.iter().find(|section| section.id == id) else {
            return Ok(None);
        };
        if !section.placement.is_embedded() {
            return Ok(None);
        }
        checked_slice(self.bytes, section.offset, section.stored_size).map(Some)
    }

    pub fn decoded_section(&self, id: SectionId) -> Result<Option<Vec<u8>>, ContainerError> {
        let Some(section) = self.sections.iter().find(|section| section.id == id) else {
            return Ok(None);
        };
        if !section.placement.is_embedded() {
            return Ok(None);
        }
        let payload = checked_slice(self.bytes, section.offset, section.stored_size)?;
        decode_payload(section, payload, self.budget).map(Some)
    }

    pub fn decoded_section_with_external_payloads(
        &self,
        id: SectionId,
        payloads: &[ExternalSectionPayload],
    ) -> Result<Option<Vec<u8>>, ExternalSectionPayloadError> {
        let Some(section) = self.sections.iter().find(|section| section.id == id) else {
            return Ok(None);
        };
        if section.placement.is_embedded() {
            return self
                .decoded_section(id)
                .map_err(ExternalSectionPayloadError::Container);
        }
        let payload = external_payload_for_section(section, payloads)?;
        Ok(Some(payload.bytes().to_vec()))
    }

    pub fn content_root(&self) -> BundleDigest {
        let mut descriptors = self.sections.clone();
        descriptors.sort_by_key(SectionDescriptor::id);
        let mut bytes = Vec::new();
        for descriptor in descriptors {
            bytes.extend_from_slice(&descriptor.id.as_bytes());
            bytes.extend_from_slice(&descriptor.kind_code().encoded().to_le_bytes());
            bytes.extend_from_slice(&descriptor.schema_version.to_le_bytes());
            bytes.extend_from_slice(&descriptor.decoded_size.to_le_bytes());
            bytes.extend_from_slice(&descriptor.content_digest.as_bytes());
            bytes.push(u8::from(descriptor.required));
            bytes.push(descriptor.residency.encoded());
            bytes.push(descriptor.placement.encoded());
        }
        BundleDigest::of(&bytes)
    }

    pub fn artifact_identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::for_current_container(
            self.kind,
            self.content_root(),
            BundleDigest::of(self.manifest),
        )
    }
}

fn external_payload_for_section<'a>(
    section: &SectionDescriptor,
    payloads: &'a [ExternalSectionPayload],
) -> Result<&'a ExternalSectionPayload, ExternalSectionPayloadError> {
    let mut matches = payloads
        .iter()
        .filter(|payload| payload.id() == section.id());
    let payload = matches
        .next()
        .ok_or(ExternalSectionPayloadError::MissingPayload(section.id()))?;
    if matches.next().is_some() {
        return Err(ExternalSectionPayloadError::DuplicatePayload(section.id()));
    }
    validate_external_payload(section, payload.bytes())?;
    Ok(payload)
}

fn validate_external_payload(
    section: &SectionDescriptor,
    bytes: &[u8],
) -> Result<(), ExternalSectionPayloadError> {
    let actual_size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if actual_size != section.decoded_size() {
        return Err(ExternalSectionPayloadError::SizeMismatch {
            id: section.id(),
            expected: section.decoded_size(),
            actual: actual_size,
        });
    }
    let actual_digest = BundleDigest::of(bytes);
    if actual_digest != section.content_digest() {
        return Err(ExternalSectionPayloadError::ContentDigestMismatch {
            id: section.id(),
            expected: section.content_digest(),
            actual: actual_digest,
        });
    }
    Ok(())
}

pub fn encode_bundle(
    kind: BundleKind,
    manifest: &[u8],
    mut sections: Vec<SectionInput>,
) -> Result<Vec<u8>, ContainerError> {
    sections.sort_by_key(SectionInput::id);
    validate_inputs(kind, &sections)?;

    let manifest_offset = u64::try_from(HEADER_SIZE).map_err(|_| ContainerError::Bounds)?;
    let manifest_len = u64::try_from(manifest.len()).map_err(|_| ContainerError::Bounds)?;
    let index_offset = align_up(
        HEADER_SIZE
            .checked_add(manifest.len())
            .ok_or(ContainerError::Bounds)?,
    )?;
    let index_len = sections
        .len()
        .checked_mul(SECTION_INDEX_ENTRY_SIZE)
        .ok_or(ContainerError::Bounds)?;
    let mut payload_offset = align_up(
        index_offset
            .checked_add(index_len)
            .ok_or(ContainerError::Bounds)?,
    )?;

    let mut descriptors = Vec::with_capacity(sections.len());
    for section in &sections {
        let offset = if section.placement.is_embedded() {
            let current = u64::try_from(payload_offset).map_err(|_| ContainerError::Bounds)?;
            payload_offset = align_up(
                payload_offset
                    .checked_add(section.stored_bytes.len())
                    .ok_or(ContainerError::Bounds)?,
            )?;
            current
        } else {
            0
        };
        descriptors.push(SectionDescriptor {
            id: section.id,
            kind: section.kind,
            schema_version: section.schema_version,
            residency: section.residency,
            placement: section.placement,
            compression: section.compression,
            offset,
            stored_size: section.stored_size,
            decoded_size: section.decoded_size,
            stored_digest: section.stored_digest,
            content_digest: section.content_digest,
            required: section.required,
        });
    }

    let mut index = Vec::with_capacity(index_len);
    for descriptor in &descriptors {
        encode_descriptor(&mut index, descriptor);
    }

    let file_len = payload_offset;
    let mut out = vec![0_u8; file_len];
    out[HEADER_SIZE..HEADER_SIZE + manifest.len()].copy_from_slice(manifest);
    out[index_offset..index_offset + index.len()].copy_from_slice(&index);
    for (section, descriptor) in sections.iter().zip(&descriptors) {
        if descriptor.placement.is_embedded() {
            let start = usize::try_from(descriptor.offset).map_err(|_| ContainerError::Bounds)?;
            let end = start
                .checked_add(section.stored_bytes.len())
                .ok_or(ContainerError::Bounds)?;
            out[start..end].copy_from_slice(&section.stored_bytes);
        }
    }

    let header = Header {
        kind,
        manifest_offset,
        manifest_len,
        index_offset: u64::try_from(index_offset).map_err(|_| ContainerError::Bounds)?,
        index_len: u64::try_from(index.len()).map_err(|_| ContainerError::Bounds)?,
        signature_offset: 0,
        signature_len: 0,
        file_len: u64::try_from(file_len).map_err(|_| ContainerError::Bounds)?,
        section_count: descriptors.len(),
        manifest_digest: BundleDigest::of(manifest),
        index_digest: BundleDigest::of(&index),
    };
    encode_header(&mut out[..HEADER_SIZE], header)?;
    Ok(out)
}

pub fn append_signature_block(bytes: &[u8], signature: &[u8]) -> Result<Vec<u8>, ContainerError> {
    if signature.is_empty() {
        return Err(ContainerError::EmptySignature);
    }
    let view = BundleView::parse(bytes, ReadBudget::default())?;
    if view.signature().is_some() {
        return Err(ContainerError::SignatureAlreadyPresent);
    }
    let signature_offset = bytes.len();
    let signature_len = signature.len();
    let file_len = signature_offset
        .checked_add(signature_len)
        .ok_or(ContainerError::Bounds)?;
    let mut signed = bytes.to_vec();
    signed.extend_from_slice(signature);
    write_u64(
        &mut signed,
        SIGNATURE_OFFSET_FIELD_OFFSET,
        u64::try_from(signature_offset).map_err(|_| ContainerError::Bounds)?,
    );
    write_u64(
        &mut signed,
        SIGNATURE_LEN_FIELD_OFFSET,
        u64::try_from(signature_len).map_err(|_| ContainerError::Bounds)?,
    );
    write_u64(
        &mut signed,
        FILE_LEN_FIELD_OFFSET,
        u64::try_from(file_len).map_err(|_| ContainerError::Bounds)?,
    );
    Ok(signed)
}

fn validate_inputs(kind: BundleKind, sections: &[SectionInput]) -> Result<(), ContainerError> {
    let mut seen = BTreeSet::new();
    for section in sections {
        if !seen.insert(section.id) {
            return Err(ContainerError::DuplicateSection(section.id));
        }
        if section.required && section.known_kind().is_none() {
            return Err(ContainerError::UnknownRequiredSectionKind(
                section.kind_code().encoded(),
            ));
        }
        if let Some(section_kind) = section.known_kind()
            && !kind.allows_section(section_kind)
        {
            return Err(ContainerError::DisallowedSection {
                bundle: kind,
                section: section_kind,
            });
        }
        if section.placement == ContentPlacement::External && !section.stored_bytes.is_empty() {
            return Err(ContainerError::ExternalSectionHasPayload(section.id));
        }
    }
    validate_required_inputs(kind, sections)
}

fn validate_required_inputs(
    kind: BundleKind,
    sections: &[SectionInput],
) -> Result<(), ContainerError> {
    let required_sections = match kind {
        BundleKind::Program => REQUIRED_PROGRAM_SECTIONS.as_slice(),
        BundleKind::Patch => REQUIRED_PATCH_SECTIONS.as_slice(),
        BundleKind::AgentController | BundleKind::ContentPack => &[],
    };
    for required in required_sections {
        if !sections
            .iter()
            .any(|section| section.known_kind() == Some(*required))
        {
            return Err(ContainerError::MissingRequiredSection(*required));
        }
    }
    Ok(())
}

fn validate_required_sections(
    kind: BundleKind,
    sections: &[SectionDescriptor],
) -> Result<(), ContainerError> {
    let required_sections = match kind {
        BundleKind::Program => REQUIRED_PROGRAM_SECTIONS.as_slice(),
        BundleKind::Patch => REQUIRED_PATCH_SECTIONS.as_slice(),
        BundleKind::AgentController | BundleKind::ContentPack => &[],
    };
    for required in required_sections {
        if !sections
            .iter()
            .any(|section| section.known_kind() == Some(*required))
        {
            return Err(ContainerError::MissingRequiredSection(*required));
        }
    }
    Ok(())
}

fn validate_descriptor(
    bundle: BundleKind,
    descriptor: &SectionDescriptor,
    budget: ReadBudget,
) -> Result<(), ContainerError> {
    if descriptor.required && descriptor.known_kind().is_none() {
        return Err(ContainerError::UnknownRequiredSectionKind(
            descriptor.kind_code().encoded(),
        ));
    }
    if let Some(kind) = descriptor.known_kind()
        && !bundle.allows_section(kind)
    {
        return Err(ContainerError::DisallowedSection {
            bundle,
            section: kind,
        });
    }
    if descriptor.decoded_size > budget.decoded_bytes {
        return Err(ContainerError::BudgetExceeded);
    }
    match descriptor.placement {
        ContentPlacement::Embedded => {
            if descriptor.stored_size > budget.embedded_bytes {
                return Err(ContainerError::BudgetExceeded);
            }
            if descriptor.compression == Compression::None
                && descriptor.decoded_size != descriptor.stored_size
            {
                return Err(ContainerError::Bounds);
            }
        }
        ContentPlacement::External => {
            if descriptor.offset != 0 {
                return Err(ContainerError::ExternalSectionHasPayload(descriptor.id));
            }
            if descriptor.compression == Compression::None
                && descriptor.decoded_size != descriptor.stored_size
            {
                return Err(ContainerError::Bounds);
            }
        }
    }
    Ok(())
}

fn decode_payload(
    descriptor: &SectionDescriptor,
    stored: &[u8],
    budget: ReadBudget,
) -> Result<Vec<u8>, ContainerError> {
    let decoded = match descriptor.compression {
        Compression::None => stored.to_vec(),
        Compression::Zstd => decode_zstd_payload(descriptor, stored)?,
    };
    let decoded_len = u64::try_from(decoded.len()).map_err(|_| ContainerError::Bounds)?;
    if decoded_len != descriptor.decoded_size {
        return Err(ContainerError::Bounds);
    }
    if decoded_len > budget.decoded_bytes {
        return Err(ContainerError::BudgetExceeded);
    }
    Ok(decoded)
}

fn decode_zstd_payload(
    descriptor: &SectionDescriptor,
    stored: &[u8],
) -> Result<Vec<u8>, ContainerError> {
    let limit = usize::try_from(descriptor.decoded_size).map_err(|_| ContainerError::Bounds)?;
    zstd::bulk::decompress(stored, limit).map_err(|error| ContainerError::DecompressZstd {
        section: descriptor.id,
        message: error.to_string(),
    })
}

fn decode_header(bytes: &[u8], budget: ReadBudget) -> Result<Header, ContainerError> {
    if bytes.len() < HEADER_SIZE {
        return Err(ContainerError::Truncated);
    }
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > budget.file_size {
        return Err(ContainerError::BudgetExceeded);
    }
    if bytes[..8] != MAGIC {
        return Err(ContainerError::BadMagic);
    }
    let version = read_u32(bytes, 8)?;
    if version != CONTAINER_VERSION {
        return Err(ContainerError::UnsupportedVersion(version));
    }
    let header_size = read_u32(bytes, 12)?;
    if header_size != u32::try_from(HEADER_SIZE).expect("header size fits u32") {
        return Err(ContainerError::UnsupportedHeaderSize(header_size));
    }
    let kind_raw = read_u32(bytes, 20)?;
    let kind =
        BundleKind::from_encoded(kind_raw).ok_or(ContainerError::UnknownBundleKind(kind_raw))?;
    let file_len = read_u64(bytes, FILE_LEN_FIELD_OFFSET)?;
    if file_len != u64::try_from(bytes.len()).map_err(|_| ContainerError::Bounds)? {
        return Err(ContainerError::FileLengthMismatch);
    }
    let section_count =
        usize::try_from(read_u32(bytes, 80)?).map_err(|_| ContainerError::Bounds)?;
    if section_count > budget.section_count {
        return Err(ContainerError::BudgetExceeded);
    }
    Ok(Header {
        kind,
        manifest_offset: read_u64(bytes, 24)?,
        manifest_len: read_u64(bytes, 32)?,
        index_offset: read_u64(bytes, 40)?,
        index_len: read_u64(bytes, 48)?,
        signature_offset: read_u64(bytes, SIGNATURE_OFFSET_FIELD_OFFSET)?,
        signature_len: read_u64(bytes, SIGNATURE_LEN_FIELD_OFFSET)?,
        file_len,
        section_count,
        manifest_digest: read_digest(bytes, MANIFEST_DIGEST_OFFSET)?,
        index_digest: read_digest(bytes, INDEX_DIGEST_OFFSET)?,
    })
}

fn encode_header(out: &mut [u8], header: Header) -> Result<(), ContainerError> {
    out.fill(0);
    out[..8].copy_from_slice(&MAGIC);
    out[8..12].copy_from_slice(&CONTAINER_VERSION.to_le_bytes());
    out[12..16].copy_from_slice(
        &u32::try_from(HEADER_SIZE)
            .expect("header size fits u32")
            .to_le_bytes(),
    );
    out[16..20].copy_from_slice(&0_u32.to_le_bytes());
    out[20..24].copy_from_slice(&header.kind.encoded().to_le_bytes());
    out[24..32].copy_from_slice(&header.manifest_offset.to_le_bytes());
    out[32..40].copy_from_slice(&header.manifest_len.to_le_bytes());
    out[40..48].copy_from_slice(&header.index_offset.to_le_bytes());
    out[48..56].copy_from_slice(&header.index_len.to_le_bytes());
    out[SIGNATURE_OFFSET_FIELD_OFFSET..SIGNATURE_OFFSET_FIELD_OFFSET + 8]
        .copy_from_slice(&header.signature_offset.to_le_bytes());
    out[SIGNATURE_LEN_FIELD_OFFSET..SIGNATURE_LEN_FIELD_OFFSET + 8]
        .copy_from_slice(&header.signature_len.to_le_bytes());
    out[FILE_LEN_FIELD_OFFSET..FILE_LEN_FIELD_OFFSET + 8]
        .copy_from_slice(&header.file_len.to_le_bytes());
    out[80..84].copy_from_slice(
        &u32::try_from(header.section_count)
            .map_err(|_| ContainerError::Bounds)?
            .to_le_bytes(),
    );
    out[MANIFEST_DIGEST_OFFSET..MANIFEST_DIGEST_OFFSET + 32]
        .copy_from_slice(&header.manifest_digest.as_bytes());
    out[INDEX_DIGEST_OFFSET..INDEX_DIGEST_OFFSET + 32]
        .copy_from_slice(&header.index_digest.as_bytes());
    Ok(())
}

fn encode_descriptor(out: &mut Vec<u8>, value: &SectionDescriptor) {
    let start = out.len();
    out.extend_from_slice(&value.id.as_bytes());
    out.extend_from_slice(&value.kind_code().encoded().to_le_bytes());
    out.extend_from_slice(&value.schema_version.to_le_bytes());
    out.push(value.residency.encoded());
    out.push(value.placement.encoded());
    out.push(value.compression.encoded());
    out.push(u8::from(value.required));
    out.extend_from_slice(&0_u32.to_le_bytes());
    out.extend_from_slice(&value.offset.to_le_bytes());
    out.extend_from_slice(&value.stored_size.to_le_bytes());
    out.extend_from_slice(&value.decoded_size.to_le_bytes());
    out.extend_from_slice(&value.stored_digest.as_bytes());
    out.extend_from_slice(&value.content_digest.as_bytes());
    out.resize(start + SECTION_INDEX_ENTRY_SIZE, 0);
}

fn decode_descriptor(bytes: &[u8]) -> Result<DecodedDescriptor, ContainerError> {
    if bytes.len() != SECTION_INDEX_ENTRY_SIZE {
        return Err(ContainerError::Bounds);
    }
    let mut id = [0_u8; 16];
    id.copy_from_slice(&bytes[..16]);
    let id = SectionId::from_bytes(id);
    let kind_raw = read_u32(bytes, 16)?;
    let required = *bytes.get(27).ok_or(ContainerError::Bounds)? != 0;
    if required && BundleSectionKind::from_encoded(kind_raw).is_none() {
        return Err(ContainerError::UnknownRequiredSectionKind(kind_raw));
    }
    let residency_raw = *bytes.get(24).ok_or(ContainerError::Bounds)?;
    let placement_raw = *bytes.get(25).ok_or(ContainerError::Bounds)?;
    let compression_raw = *bytes.get(26).ok_or(ContainerError::Bounds)?;
    Ok(DecodedDescriptor::Descriptor(SectionDescriptor {
        id,
        kind: SectionKindCode::new(kind_raw),
        schema_version: read_u32(bytes, 20)?,
        residency: ContentResidency::from_encoded(residency_raw)
            .ok_or(ContainerError::InvalidResidency(residency_raw))?,
        placement: ContentPlacement::from_encoded(placement_raw)
            .ok_or(ContainerError::InvalidPlacement(placement_raw))?,
        compression: Compression::from_encoded(compression_raw)
            .ok_or(ContainerError::InvalidCompression(compression_raw))?,
        offset: read_u64(bytes, 32)?,
        stored_size: read_u64(bytes, 40)?,
        decoded_size: read_u64(bytes, 48)?,
        stored_digest: read_digest(bytes, 56)?,
        content_digest: read_digest(bytes, 88)?,
        required,
    }))
}

fn checked_slice(bytes: &[u8], offset: u64, len: u64) -> Result<&[u8], ContainerError> {
    let (start, end) = checked_range(offset, len)?;
    bytes.get(start..end).ok_or(ContainerError::Bounds)
}

fn checked_range(offset: u64, len: u64) -> Result<(usize, usize), ContainerError> {
    let start = usize::try_from(offset).map_err(|_| ContainerError::Bounds)?;
    let len = usize::try_from(len).map_err(|_| ContainerError::Bounds)?;
    let end = start.checked_add(len).ok_or(ContainerError::Bounds)?;
    Ok((start, end))
}

fn ranges_overlap_any(range: (usize, usize), ranges: &[(usize, usize)]) -> bool {
    ranges
        .iter()
        .any(|existing| range.0 < existing.1 && existing.0 < range.1)
}

fn align_up(value: usize) -> Result<usize, ContainerError> {
    let rem = value % PAYLOAD_ALIGNMENT;
    if rem == 0 {
        Ok(value)
    } else {
        value
            .checked_add(PAYLOAD_ALIGNMENT - rem)
            .ok_or(ContainerError::Bounds)
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ContainerError> {
    let chunk = bytes
        .get(offset..offset + 4)
        .ok_or(ContainerError::Bounds)?;
    Ok(u32::from_le_bytes(
        chunk.try_into().expect("slice length checked"),
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, ContainerError> {
    let chunk = bytes
        .get(offset..offset + 8)
        .ok_or(ContainerError::Bounds)?;
    Ok(u64::from_le_bytes(
        chunk.try_into().expect("slice length checked"),
    ))
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn read_digest(bytes: &[u8], offset: usize) -> Result<BundleDigest, ContainerError> {
    let chunk = bytes
        .get(offset..offset + 32)
        .ok_or(ContainerError::Bounds)?;
    Ok(BundleDigest::from_bytes(
        chunk.try_into().expect("slice length checked"),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        BundleDigest, BundleKind, BundleSectionKind, BundleView, Compression, ContainerError,
        ContentPlacement, ContentResidency, DecodedSectionKind, ExternalSectionPayload,
        ExternalSectionPayloadError, ReadBudget, SectionId, SectionInput, SectionKindCode,
        encode_bundle,
    };

    #[test]
    fn awfb_v1_header_index_and_embedded_sections_round_trip() {
        let bytecode = section_id(1);
        let bytes = encode_bundle(
            BundleKind::Program,
            br#"{"schema_version":1}"#,
            vec![
                required(bytecode, BundleSectionKind::ProgramBytecode, b"bytecode"),
                required(section_id(2), BundleSectionKind::RuntimeTypes, b"types"),
                required(
                    section_id(3),
                    BundleSectionKind::Entrypoints,
                    b"entrypoints",
                ),
                required(
                    section_id(4),
                    BundleSectionKind::AdapterRequirements,
                    b"adapters",
                ),
                required(section_id(5), BundleSectionKind::ContentCatalog, b"catalog"),
            ],
        )
        .expect("AWFB encodes");

        let view = BundleView::parse(&bytes, ReadBudget::default()).expect("AWFB parses");

        assert_eq!(view.kind(), BundleKind::Program);
        assert_eq!(view.manifest(), br#"{"schema_version":1}"#);
        assert_eq!(view.sections().len(), 5);
        assert_eq!(
            view.embedded_section(bytecode).expect("section resolves"),
            Some(b"bytecode".as_slice())
        );
        assert_ne!(view.content_root().as_bytes(), [0; 32]);
    }

    #[test]
    fn awfb_v1_rejects_duplicate_section_ids() {
        let id = section_id(1);
        let error = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![
                SectionInput::embedded(
                    id,
                    BundleSectionKind::AssetBlob,
                    1,
                    ContentResidency::OnDemand,
                    false,
                    b"a",
                ),
                SectionInput::embedded(
                    id,
                    BundleSectionKind::AssetCatalog,
                    1,
                    ContentResidency::OnDemand,
                    false,
                    b"b",
                ),
            ],
        )
        .expect_err("duplicate section id is rejected");

        assert!(matches!(error, ContainerError::DuplicateSection(duplicate) if duplicate == id));
    }

    #[test]
    fn awfb_v1_external_section_descriptors_round_trip_without_payload() {
        let id = section_id(1);
        let external_digest = super::BundleDigest::of(b"external asset bytes");
        let bytes = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![SectionInput::external_ref(
                id,
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                20,
                external_digest,
            )],
        )
        .expect("AWFB encodes");

        let view = BundleView::parse(&bytes, ReadBudget::default()).expect("AWFB parses");
        let section = view.sections().first().expect("section descriptor exists");

        assert_eq!(section.decoded_size(), 20);
        assert_eq!(section.content_digest(), external_digest);
        assert_eq!(view.embedded_section(id).expect("section lookup"), None);
        assert_ne!(view.content_root().as_bytes(), [0; 32]);
    }

    #[test]
    fn awfb_v1_decodes_external_section_from_verified_payload() {
        let id = section_id(1);
        let unrelated_id = section_id(2);
        let payload = b"external asset bytes".to_vec();
        let bytes = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![SectionInput::external_ref(
                id,
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                payload.len() as u64,
                super::BundleDigest::of(&payload),
            )],
        )
        .expect("AWFB encodes");
        let view = BundleView::parse(&bytes, ReadBudget::default()).expect("AWFB parses");

        assert_eq!(view.decoded_section(id).expect("inline lookup"), None);
        assert_eq!(
            view.decoded_section_with_external_payloads(
                id,
                &[
                    ExternalSectionPayload::new(unrelated_id, b"unused".to_vec()),
                    ExternalSectionPayload::new(id, payload.clone()),
                ],
            )
            .expect("external payload decodes"),
            Some(payload)
        );
    }

    #[test]
    fn awfb_v1_rejects_missing_external_section_payload() {
        let id = section_id(1);
        let payload = b"external asset bytes";
        let bytes = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![SectionInput::external_ref(
                id,
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                payload.len() as u64,
                super::BundleDigest::of(payload),
            )],
        )
        .expect("AWFB encodes");
        let view = BundleView::parse(&bytes, ReadBudget::default()).expect("AWFB parses");

        let error = view
            .decoded_section_with_external_payloads(id, &[])
            .expect_err("missing external payload rejects");

        assert_eq!(error, ExternalSectionPayloadError::MissingPayload(id));
    }

    #[test]
    fn awfb_v1_rejects_external_section_payload_digest_mismatch() {
        let id = section_id(1);
        let expected = b"external asset bytes";
        let actual = b"external asset bytez".to_vec();
        let bytes = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![SectionInput::external_ref(
                id,
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                expected.len() as u64,
                super::BundleDigest::of(expected),
            )],
        )
        .expect("AWFB encodes");
        let view = BundleView::parse(&bytes, ReadBudget::default()).expect("AWFB parses");

        let error = view
            .decoded_section_with_external_payloads(id, &[ExternalSectionPayload::new(id, actual)])
            .expect_err("digest mismatch rejects");

        assert!(matches!(
            error,
            ExternalSectionPayloadError::ContentDigestMismatch { id: failed, .. }
                if failed == id
        ));
    }

    #[test]
    fn awfb_v1_content_pack_rejects_executable_sections() {
        let error = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![SectionInput::embedded(
                section_id(1),
                BundleSectionKind::ProgramBytecode,
                1,
                ContentResidency::OnDemand,
                true,
                b"bytecode",
            )],
        )
        .expect_err("content pack must not contain executable code");

        assert!(matches!(
            error,
            ContainerError::DisallowedSection {
                bundle: BundleKind::ContentPack,
                section: BundleSectionKind::ProgramBytecode,
            }
        ));
    }

    #[test]
    fn awfb_v1_patch_rejects_direct_executable_sections() {
        let error = encode_bundle(
            BundleKind::Patch,
            b"{}",
            vec![
                SectionInput::embedded(
                    section_id(1),
                    BundleSectionKind::PatchPlan,
                    1,
                    ContentResidency::Startup,
                    true,
                    b"{}",
                ),
                SectionInput::embedded(
                    section_id(2),
                    BundleSectionKind::ProgramBytecode,
                    1,
                    ContentResidency::Startup,
                    true,
                    b"bytecode",
                ),
            ],
        )
        .expect_err("patch bundle must not carry executable sections directly");

        assert!(matches!(
            error,
            ContainerError::DisallowedSection {
                bundle: BundleKind::Patch,
                section: BundleSectionKind::ProgramBytecode,
            }
        ));
    }

    #[test]
    fn awfb_manifest_policy_enums_parse_display_and_default() {
        assert_eq!(ContentResidency::default(), ContentResidency::Startup);
        assert_eq!(
            "on_demand".parse::<ContentResidency>(),
            Ok(ContentResidency::OnDemand)
        );
        assert_eq!(ContentResidency::OnDemand.to_string(), "on-demand");

        assert_eq!(ContentPlacement::default(), ContentPlacement::Embedded);
        assert_eq!(
            "external".parse::<ContentPlacement>(),
            Ok(ContentPlacement::External)
        );
        assert_eq!(ContentPlacement::External.to_string(), "external");

        assert_eq!(Compression::default(), Compression::None);
        assert_eq!("zstd".parse::<Compression>(), Ok(Compression::Zstd));
        assert_eq!(Compression::Zstd.to_string(), "zstd");
        assert!("brotli".parse::<Compression>().is_err());
    }

    #[test]
    fn awfb_v1_program_requires_core_sections() {
        let error = encode_bundle(
            BundleKind::Program,
            b"{}",
            vec![required(
                section_id(1),
                BundleSectionKind::ProgramBytecode,
                b"bytecode",
            )],
        )
        .expect_err("program bundle must include every required section");

        assert!(matches!(
            error,
            ContainerError::MissingRequiredSection(BundleSectionKind::RuntimeTypes)
        ));
    }

    #[test]
    fn awfb_v1_rejects_digest_corruption() {
        let id = section_id(1);
        let mut bytes = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![SectionInput::embedded(
                id,
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                b"blob",
            )],
        )
        .expect("AWFB encodes");
        let payload = bytes
            .iter()
            .position(|byte| *byte == b'b')
            .expect("payload byte is present");
        bytes[payload] = b'X';

        let error =
            BundleView::parse(&bytes, ReadBudget::default()).expect_err("digest mismatch rejects");

        assert!(matches!(error, ContainerError::StoredDigestMismatch(section) if section == id));
    }

    #[test]
    fn awfb_v1_rejects_embedded_section_exceeding_decoded_budget() {
        let bytes = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![SectionInput::embedded(
                section_id(1),
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                b"blob",
            )],
        )
        .expect("AWFB encodes");
        let budget = ReadBudget::default().with_decoded_bytes(3);

        let error = BundleView::parse(&bytes, budget)
            .expect_err("decoded payload beyond budget is rejected");

        assert_eq!(error, ContainerError::BudgetExceeded);
    }

    #[test]
    fn awfb_v1_rejects_external_descriptor_exceeding_decoded_budget() {
        let bytes = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![SectionInput::external_ref(
                section_id(1),
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                4096,
                super::BundleDigest::of(b"external asset bytes"),
            )],
        )
        .expect("AWFB encodes");
        let budget = ReadBudget::default().with_decoded_bytes(1024);

        let error = BundleView::parse(&bytes, budget)
            .expect_err("external decoded size beyond budget is rejected");

        assert_eq!(error, ContainerError::BudgetExceeded);
    }

    #[test]
    fn awfb_v1_exposes_bounded_signature_block() {
        let bytes = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![SectionInput::embedded(
                section_id(1),
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                b"blob",
            )],
        )
        .expect("AWFB encodes");
        let signed = append_signature_block(bytes, b"signature");

        let view = BundleView::parse(&signed, ReadBudget::default()).expect("AWFB parses");

        assert_eq!(view.signature(), Some(b"signature".as_slice()));
    }

    #[test]
    fn awfb_v1_signing_digest_excludes_trailing_signature_block() {
        let bytes = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![SectionInput::embedded(
                section_id(1),
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                b"blob",
            )],
        )
        .expect("AWFB encodes");
        let signed_primary = append_signature_block(bytes.clone(), b"signature-a");
        let signed_alternate = append_signature_block(bytes.clone(), b"signature-b");
        let unsigned_view = BundleView::parse(&bytes, ReadBudget::default()).expect("AWFB parses");
        let signed_primary_view =
            BundleView::parse(&signed_primary, ReadBudget::default()).expect("signed AWFB parses");
        let signed_alternate_view = BundleView::parse(&signed_alternate, ReadBudget::default())
            .expect("signed AWFB parses");

        assert_eq!(
            signed_primary_view
                .signing_digest()
                .expect("digest computes"),
            unsigned_view.signing_digest().expect("digest computes")
        );
        assert_eq!(
            signed_alternate_view
                .signing_digest()
                .expect("digest computes"),
            unsigned_view.signing_digest().expect("digest computes")
        );
    }

    #[test]
    fn awfb_v1_rejects_signature_overlap_with_header_ranges() {
        let mut bytes = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![SectionInput::embedded(
                section_id(1),
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                b"blob",
            )],
        )
        .expect("AWFB encodes");
        write_u64(&mut bytes, 56, super::HEADER_SIZE as u64);
        write_u64(&mut bytes, 64, 1);

        let error = BundleView::parse(&bytes, ReadBudget::default())
            .expect_err("signature cannot overlap manifest/index/header ranges");

        assert_eq!(error, ContainerError::OverlappingSignature);
    }

    #[test]
    fn awfb_v1_decodes_zstd_embedded_sections_with_output_limit() {
        let id = section_id(1);
        let bytes = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![
                SectionInput::embedded_zstd(
                    id,
                    BundleSectionKind::AssetBlob,
                    1,
                    ContentResidency::OnDemand,
                    false,
                    b"blob blob blob blob",
                )
                .expect("zstd section encodes"),
            ],
        )
        .expect("AWFB encodes");

        let view = BundleView::parse(&bytes, ReadBudget::default()).expect("zstd AWFB parses");

        assert_eq!(
            view.decoded_section(id).expect("zstd section decodes"),
            Some(b"blob blob blob blob".to_vec())
        );
    }

    #[test]
    fn awfb_v1_rejects_zstd_section_exceeding_decoded_budget() {
        let bytes = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![
                SectionInput::embedded_zstd(
                    section_id(1),
                    BundleSectionKind::AssetBlob,
                    1,
                    ContentResidency::OnDemand,
                    false,
                    b"blob blob blob blob",
                )
                .expect("zstd section encodes"),
            ],
        )
        .expect("AWFB encodes");
        let budget = ReadBudget::default().with_decoded_bytes(4);

        let error = BundleView::parse(&bytes, budget)
            .expect_err("zstd decoded payload beyond budget is rejected");

        assert_eq!(error, ContainerError::BudgetExceeded);
    }

    #[test]
    fn awfb_v1_retains_unknown_optional_sections_and_rejects_required_unknown() {
        let mut bytes = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![SectionInput::embedded_unknown_optional(
                section_id(1),
                SectionKindCode::new(900),
                1,
                ContentResidency::OnDemand,
                b"blob",
            )],
        )
        .expect("AWFB encodes");

        let view = BundleView::parse(&bytes, ReadBudget::default())
            .expect("unknown optional section can be retained");
        assert_eq!(view.skipped_optional_sections(), 0);
        assert_eq!(view.sections().len(), 1);
        let descriptor = &view.sections()[0];
        assert_eq!(descriptor.known_kind(), None);
        assert_eq!(descriptor.kind_code(), SectionKindCode::new(900));
        assert_eq!(
            descriptor.decoded_kind(),
            DecodedSectionKind::UnknownOptional(SectionKindCode::new(900))
        );
        assert_eq!(
            view.decoded_section(descriptor.id())
                .expect("unknown optional section decodes"),
            Some(b"blob".to_vec())
        );

        let index_offset = 176_usize;
        bytes[index_offset + 27] = 1;
        refresh_index_digest(&mut bytes, index_offset, super::SECTION_INDEX_ENTRY_SIZE);
        let error = BundleView::parse(&bytes, ReadBudget::default())
            .expect_err("unknown required section is rejected");
        assert!(matches!(
            error,
            ContainerError::UnknownRequiredSectionKind(900)
        ));
    }

    #[test]
    fn awfb_v1_retains_unknown_optional_external_descriptors() {
        let payload = b"external opaque bytes";
        let payload_digest = BundleDigest::of(payload);
        let bytes = encode_bundle(
            BundleKind::ContentPack,
            b"{}",
            vec![SectionInput::external_unknown_optional_ref(
                section_id(2),
                SectionKindCode::new(901),
                1,
                ContentResidency::OnDemand,
                payload.len() as u64,
                payload_digest,
            )],
        )
        .expect("AWFB encodes");

        let view = BundleView::parse(&bytes, ReadBudget::default()).expect("AWFB parses");
        let descriptor = &view.sections()[0];

        assert_eq!(descriptor.known_kind(), None);
        assert_eq!(descriptor.kind_code(), SectionKindCode::new(901));
        assert_eq!(
            view.decoded_section_with_external_payloads(
                descriptor.id(),
                &[ExternalSectionPayload::new(
                    descriptor.id(),
                    payload.to_vec()
                )],
            )
            .expect("external unknown optional payload verifies"),
            Some(payload.to_vec())
        );
    }

    #[test]
    fn artifact_identity_changes_for_manifest_only_delta() {
        let section = SectionInput::embedded(
            section_id(3),
            BundleSectionKind::AssetBlob,
            1,
            ContentResidency::OnDemand,
            false,
            b"stable content",
        );
        let first = encode_bundle(
            BundleKind::ContentPack,
            br#"{"name":"first"}"#,
            vec![section],
        )
        .expect("first AWFB encodes");
        let section = SectionInput::embedded(
            section_id(3),
            BundleSectionKind::AssetBlob,
            1,
            ContentResidency::OnDemand,
            false,
            b"stable content",
        );
        let second = encode_bundle(
            BundleKind::ContentPack,
            br#"{"name":"second"}"#,
            vec![section],
        )
        .expect("second AWFB encodes");
        let first = BundleView::parse(&first, ReadBudget::default()).expect("first AWFB parses");
        let second = BundleView::parse(&second, ReadBudget::default()).expect("second AWFB parses");

        assert_eq!(first.content_root(), second.content_root());
        assert_ne!(first.artifact_identity(), second.artifact_identity());
        assert_ne!(
            first.artifact_identity().digest(),
            second.artifact_identity().digest()
        );
        assert_eq!(
            first.artifact_identity().manifest_digest,
            BundleDigest::of(br#"{"name":"first"}"#)
        );
    }

    fn required(id: SectionId, kind: BundleSectionKind, bytes: &'static [u8]) -> SectionInput {
        SectionInput::embedded(id, kind, 1, ContentResidency::Startup, true, bytes)
    }

    fn section_id(seed: u8) -> SectionId {
        SectionId::from_bytes([seed; 16])
    }

    fn refresh_index_digest(bytes: &mut [u8], index_offset: usize, index_len: usize) {
        let digest = super::BundleDigest::of(&bytes[index_offset..index_offset + index_len]);
        bytes[super::INDEX_DIGEST_OFFSET..super::INDEX_DIGEST_OFFSET + 32]
            .copy_from_slice(&digest.as_bytes());
    }

    fn append_signature_block(mut bytes: Vec<u8>, signature: &[u8]) -> Vec<u8> {
        let signature_offset = bytes.len();
        bytes.extend_from_slice(signature);
        write_u64(&mut bytes, 56, signature_offset as u64);
        write_u64(&mut bytes, 64, signature.len() as u64);
        let file_len = bytes.len() as u64;
        write_u64(&mut bytes, 72, file_len);
        bytes
    }

    fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
}
