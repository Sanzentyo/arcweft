use crate::container::BundleSectionKind;
use crate::patch::PatchCompatibility;

use super::budget::SectionCodecBudget;

/// Product resource section codec families.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ProductSectionCodecKind {
    RuntimeTypes,
    Entrypoints,
    AdapterRequirements,
    ContentCatalog,
    AssetCatalog,
    DisplayCatalog,
    SourceMap,
    AudioGraph,
    ViewProgram,
    ViewStyle,
    ViewText,
    ViewInput,
    ViewTheme,
    LocaleCatalog,
}

impl ProductSectionCodecKind {
    /// Complete inventory of implemented compact product resource codecs.
    pub const ALL: [Self; 14] = [
        Self::RuntimeTypes,
        Self::Entrypoints,
        Self::AdapterRequirements,
        Self::ContentCatalog,
        Self::AssetCatalog,
        Self::DisplayCatalog,
        Self::SourceMap,
        Self::AudioGraph,
        Self::ViewProgram,
        Self::ViewStyle,
        Self::ViewText,
        Self::ViewInput,
        Self::ViewTheme,
        Self::LocaleCatalog,
    ];

    /// Stable numeric tag written into compact resource section headers.
    pub const fn encoded(self) -> u32 {
        match self {
            Self::RuntimeTypes => 1,
            Self::Entrypoints => 2,
            Self::AdapterRequirements => 3,
            Self::ContentCatalog => 4,
            Self::AssetCatalog => 5,
            Self::DisplayCatalog => 6,
            Self::SourceMap => 7,
            Self::AudioGraph => 8,
            Self::ViewProgram => 9,
            Self::ViewStyle => 10,
            Self::ViewText => 11,
            Self::ViewInput => 12,
            Self::ViewTheme => 13,
            Self::LocaleCatalog => 14,
        }
    }

    /// Parses a stable compact resource codec tag.
    pub const fn from_encoded(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::RuntimeTypes),
            2 => Some(Self::Entrypoints),
            3 => Some(Self::AdapterRequirements),
            4 => Some(Self::ContentCatalog),
            5 => Some(Self::AssetCatalog),
            6 => Some(Self::DisplayCatalog),
            7 => Some(Self::SourceMap),
            8 => Some(Self::AudioGraph),
            9 => Some(Self::ViewProgram),
            10 => Some(Self::ViewStyle),
            11 => Some(Self::ViewText),
            12 => Some(Self::ViewInput),
            13 => Some(Self::ViewTheme),
            14 => Some(Self::LocaleCatalog),
            _ => None,
        }
    }

    /// Stable snake-case codec label for manifests, inspection, and docs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeTypes => "runtime_types",
            Self::Entrypoints => "entrypoints",
            Self::AdapterRequirements => "adapter_requirements",
            Self::ContentCatalog => "content_catalog",
            Self::AssetCatalog => "asset_catalog",
            Self::DisplayCatalog => "display_catalog",
            Self::SourceMap => "source_map",
            Self::AudioGraph => "audio_graph",
            Self::ViewProgram => "view_program",
            Self::ViewStyle => "view_style",
            Self::ViewText => "view_text",
            Self::ViewInput => "view_input",
            Self::ViewTheme => "view_theme",
            Self::LocaleCatalog => "locale_catalog",
        }
    }

    /// Magic bytes for a compact resource section payload.
    pub const fn magic(self) -> [u8; 8] {
        match self {
            Self::RuntimeTypes => *b"AWRT\r\n\x1a\n",
            Self::Entrypoints => *b"AWEP\r\n\x1a\n",
            Self::AdapterRequirements => *b"AWAR\r\n\x1a\n",
            Self::ContentCatalog => *b"AWCC\r\n\x1a\n",
            Self::AssetCatalog => *b"AWAC\r\n\x1a\n",
            Self::DisplayCatalog => *b"AWDC\r\n\x1a\n",
            Self::SourceMap => *b"AWSM\r\n\x1a\n",
            Self::AudioGraph => *b"AWAG\r\n\x1a\n",
            Self::ViewProgram => *b"AWVP\r\n\x1a\n",
            Self::ViewStyle => *b"AWVS\r\n\x1a\n",
            Self::ViewText => *b"AWVT\r\n\x1a\n",
            Self::ViewInput => *b"AWVI\r\n\x1a\n",
            Self::ViewTheme => *b"AWVH\r\n\x1a\n",
            Self::LocaleCatalog => *b"AWLC\r\n\x1a\n",
        }
    }

    /// AWFB container section owned by this implemented codec family.
    pub const fn section_kind(self) -> BundleSectionKind {
        match self {
            Self::RuntimeTypes => BundleSectionKind::RuntimeTypes,
            Self::Entrypoints => BundleSectionKind::Entrypoints,
            Self::AdapterRequirements => BundleSectionKind::AdapterRequirements,
            Self::ContentCatalog => BundleSectionKind::ContentCatalog,
            Self::AssetCatalog => BundleSectionKind::AssetCatalog,
            Self::DisplayCatalog => BundleSectionKind::DisplayCatalog,
            Self::SourceMap => BundleSectionKind::SourceMap,
            Self::AudioGraph => BundleSectionKind::AudioGraph,
            Self::ViewProgram => BundleSectionKind::ViewProgram,
            Self::ViewStyle => BundleSectionKind::ViewStyle,
            Self::ViewText => BundleSectionKind::ViewText,
            Self::ViewInput => BundleSectionKind::ViewInput,
            Self::ViewTheme => BundleSectionKind::ViewTheme,
            Self::LocaleCatalog => BundleSectionKind::LocaleCatalog,
        }
    }

    /// Inverse mapping from an AWFB section kind to the compact product resource
    /// family that owns the section payload, when one exists.
    pub const fn from_section_kind(kind: BundleSectionKind) -> Option<Self> {
        match kind {
            BundleSectionKind::RuntimeTypes => Some(Self::RuntimeTypes),
            BundleSectionKind::Entrypoints => Some(Self::Entrypoints),
            BundleSectionKind::AdapterRequirements => Some(Self::AdapterRequirements),
            BundleSectionKind::ContentCatalog => Some(Self::ContentCatalog),
            BundleSectionKind::AssetCatalog => Some(Self::AssetCatalog),
            BundleSectionKind::DisplayCatalog => Some(Self::DisplayCatalog),
            BundleSectionKind::SourceMap => Some(Self::SourceMap),
            BundleSectionKind::AudioGraph => Some(Self::AudioGraph),
            BundleSectionKind::ViewProgram => Some(Self::ViewProgram),
            BundleSectionKind::ViewStyle => Some(Self::ViewStyle),
            BundleSectionKind::ViewText => Some(Self::ViewText),
            BundleSectionKind::ViewInput => Some(Self::ViewInput),
            BundleSectionKind::ViewTheme => Some(Self::ViewTheme),
            BundleSectionKind::LocaleCatalog => Some(Self::LocaleCatalog),
            BundleSectionKind::ProgramBytecode
            | BundleSectionKind::AssetBlob
            | BundleSectionKind::DebugSymbols
            | BundleSectionKind::NormalizedSource
            | BundleSectionKind::HotSwapMap
            | BundleSectionKind::PatchPlan
            | BundleSectionKind::FxDefinitions
            | BundleSectionKind::ResourceTypeManifests => None,
        }
    }

    pub fn default_budget(self) -> SectionCodecBudget {
        SectionCodecBudget::default()
    }

    pub const fn affects_code_compatibility(self) -> bool {
        matches!(
            self,
            Self::RuntimeTypes | Self::Entrypoints | Self::AdapterRequirements | Self::ViewInput
        )
    }

    pub const fn patch_compatibility(self) -> PatchCompatibility {
        match self {
            Self::RuntimeTypes
            | Self::Entrypoints
            | Self::AdapterRequirements
            | Self::ViewInput => PatchCompatibility::RestartRequired,
            Self::ContentCatalog
            | Self::AssetCatalog
            | Self::DisplayCatalog
            | Self::SourceMap
            | Self::AudioGraph
            | Self::ViewProgram
            | Self::ViewStyle
            | Self::ViewText
            | Self::ViewTheme
            | Self::LocaleCatalog => PatchCompatibility::ContentOnly,
        }
    }
}
