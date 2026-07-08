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
    LocaleText,
    AudioGraph,
    Shader,
    View,
    Entity,
    DebugSymbols,
    Contracts,
    GraphIndex,
    ViewProgram,
    ViewStyle,
    ViewText,
    ViewInput,
    ViewTheme,
}

/// Migration status for compact product resource section families.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductResourceMigrationStatus {
    /// The next section-family slice should implement this as compact-first.
    CompactFirst,
    /// The current product AWFB path may keep typed JSON until its owning slice.
    JsonTemporary,
    /// The data is only exposed through human-facing inspection/export in this slice.
    InspectionOnly,
    /// The family has no stable product section yet and must not define a private codec.
    Future,
}

impl ProductSectionCodecKind {
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
            Self::LocaleText => 8,
            Self::AudioGraph => 9,
            Self::Shader => 10,
            Self::View => 11,
            Self::Entity => 12,
            Self::DebugSymbols => 13,
            Self::Contracts => 14,
            Self::GraphIndex => 15,
            Self::ViewProgram => 16,
            Self::ViewStyle => 17,
            Self::ViewText => 18,
            Self::ViewInput => 19,
            Self::ViewTheme => 20,
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
            8 => Some(Self::LocaleText),
            9 => Some(Self::AudioGraph),
            10 => Some(Self::Shader),
            11 => Some(Self::View),
            12 => Some(Self::Entity),
            13 => Some(Self::DebugSymbols),
            14 => Some(Self::Contracts),
            15 => Some(Self::GraphIndex),
            16 => Some(Self::ViewProgram),
            17 => Some(Self::ViewStyle),
            18 => Some(Self::ViewText),
            19 => Some(Self::ViewInput),
            20 => Some(Self::ViewTheme),
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
            Self::LocaleText => "locale_text",
            Self::AudioGraph => "audio_graph",
            Self::Shader => "shader",
            Self::View => "view",
            Self::Entity => "entity",
            Self::DebugSymbols => "debug_symbols",
            Self::Contracts => "contracts",
            Self::GraphIndex => "graph_index",
            Self::ViewProgram => "view_program",
            Self::ViewStyle => "view_style",
            Self::ViewText => "view_text",
            Self::ViewInput => "view_input",
            Self::ViewTheme => "view_theme",
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
            Self::LocaleText => *b"AWLT\r\n\x1a\n",
            Self::AudioGraph => *b"AWAG\r\n\x1a\n",
            Self::Shader => *b"AWSH\r\n\x1a\n",
            Self::View => *b"AWVW\r\n\x1a\n",
            Self::Entity => *b"AWEN\r\n\x1a\n",
            Self::DebugSymbols => *b"AWDS\r\n\x1a\n",
            Self::Contracts => *b"AWCT\r\n\x1a\n",
            Self::GraphIndex => *b"AWGI\r\n\x1a\n",
            Self::ViewProgram => *b"AWVP\r\n\x1a\n",
            Self::ViewStyle => *b"AWVS\r\n\x1a\n",
            Self::ViewText => *b"AWVT\r\n\x1a\n",
            Self::ViewInput => *b"AWVI\r\n\x1a\n",
            Self::ViewTheme => *b"AWVH\r\n\x1a\n",
        }
    }

    /// Existing AWFB container section kind for this codec family, when one has
    /// already been introduced.
    pub const fn section_kind(self) -> Option<BundleSectionKind> {
        match self {
            Self::RuntimeTypes => Some(BundleSectionKind::RuntimeTypes),
            Self::Entrypoints => Some(BundleSectionKind::Entrypoints),
            Self::AdapterRequirements => Some(BundleSectionKind::AdapterRequirements),
            Self::ContentCatalog => Some(BundleSectionKind::ContentCatalog),
            Self::AssetCatalog => Some(BundleSectionKind::AssetCatalog),
            Self::DisplayCatalog => Some(BundleSectionKind::DisplayCatalog),
            Self::SourceMap => Some(BundleSectionKind::SourceMap),
            Self::LocaleText => Some(BundleSectionKind::LocaleCatalog),
            Self::AudioGraph => Some(BundleSectionKind::AudioGraph),
            Self::DebugSymbols => Some(BundleSectionKind::DebugSymbols),
            Self::ViewProgram => Some(BundleSectionKind::ViewProgram),
            Self::ViewStyle => Some(BundleSectionKind::ViewStyle),
            Self::ViewText => Some(BundleSectionKind::ViewText),
            Self::ViewInput => Some(BundleSectionKind::ViewInput),
            Self::ViewTheme => Some(BundleSectionKind::ViewTheme),
            Self::Shader | Self::View | Self::Entity | Self::Contracts | Self::GraphIndex => None,
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
            BundleSectionKind::LocaleCatalog => Some(Self::LocaleText),
            BundleSectionKind::AudioGraph => Some(Self::AudioGraph),
            BundleSectionKind::DebugSymbols => Some(Self::DebugSymbols),
            BundleSectionKind::ViewProgram => Some(Self::ViewProgram),
            BundleSectionKind::ViewStyle => Some(Self::ViewStyle),
            BundleSectionKind::ViewText => Some(Self::ViewText),
            BundleSectionKind::ViewInput => Some(Self::ViewInput),
            BundleSectionKind::ViewTheme => Some(Self::ViewTheme),
            BundleSectionKind::ProgramBytecode
            | BundleSectionKind::AssetBlob
            | BundleSectionKind::NormalizedSource
            | BundleSectionKind::HotSwapMap
            | BundleSectionKind::PatchPlan => None,
        }
    }

    pub const fn migration_status(self) -> ProductResourceMigrationStatus {
        match self {
            Self::RuntimeTypes | Self::Entrypoints | Self::AdapterRequirements => {
                ProductResourceMigrationStatus::CompactFirst
            }
            Self::ContentCatalog
            | Self::AssetCatalog
            | Self::DisplayCatalog
            | Self::SourceMap
            | Self::AudioGraph
            | Self::ViewProgram
            | Self::ViewStyle
            | Self::ViewText
            | Self::ViewInput
            | Self::ViewTheme => ProductResourceMigrationStatus::CompactFirst,
            Self::LocaleText | Self::DebugSymbols => ProductResourceMigrationStatus::JsonTemporary,
            Self::Shader | Self::View | Self::Entity | Self::Contracts | Self::GraphIndex => {
                ProductResourceMigrationStatus::Future
            }
        }
    }

    pub fn default_budget(self) -> SectionCodecBudget {
        SectionCodecBudget::default()
    }

    pub const fn affects_code_compatibility(self) -> bool {
        matches!(
            self,
            Self::RuntimeTypes
                | Self::Entrypoints
                | Self::AdapterRequirements
                | Self::Contracts
                | Self::ViewInput
                | Self::GraphIndex
        )
    }

    pub const fn patch_compatibility(self) -> PatchCompatibility {
        match self {
            Self::RuntimeTypes
            | Self::Entrypoints
            | Self::AdapterRequirements
            | Self::Contracts
            | Self::ViewInput => PatchCompatibility::RestartRequired,
            Self::GraphIndex => PatchCompatibility::CodeCompatible,
            Self::ContentCatalog
            | Self::AssetCatalog
            | Self::DisplayCatalog
            | Self::SourceMap
            | Self::LocaleText
            | Self::AudioGraph
            | Self::Shader
            | Self::View
            | Self::Entity
            | Self::DebugSymbols
            | Self::ViewProgram
            | Self::ViewStyle
            | Self::ViewText
            | Self::ViewTheme => PatchCompatibility::ContentOnly,
        }
    }
}
