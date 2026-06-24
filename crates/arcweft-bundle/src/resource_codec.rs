//! Shared product resource section codec contracts.
//!
//! Product resource sections migrate away from JSON one section family at a
//! time. This module owns the Sans I/O common table/budget model used by those
//! compact binary sections; it does not perform filesystem, network, signing,
//! or platform capability checks.

use crate::container::BundleSectionKind;
use crate::patch::PatchCompatibility;
use thiserror::Error;

/// Default schema version for compact product resource sections.
pub const PRODUCT_SECTION_SCHEMA_VERSION: u32 = 1;

/// Decoder and table validation budget for a compact product resource section.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SectionCodecBudget {
    pub bytes: usize,
    pub records: usize,
    pub strings: usize,
    pub string_bytes: usize,
    pub public_ids: usize,
    pub references: usize,
    pub depth: usize,
}

/// Product resource section codec families.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
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
    ShaderUi,
    DebugSymbols,
    Contracts,
    EntityGraph,
}

/// Common compact-section header after container-level AWFB validation.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ProductSectionHeader {
    pub magic: [u8; 8],
    pub schema_version: u32,
    pub codec: ProductSectionCodecKind,
    pub string_table_len: u32,
    pub public_id_table_len: u32,
    pub record_count: u32,
}

/// Deduplicated string table.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct StringTable {
    values: Vec<String>,
}

/// Deduplicated public-id table. Duplicate IDs are rejected rather than
/// silently collapsed so product resource references stay auditably stable.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PublicIdTable {
    values: Vec<String>,
}

/// String table index.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct StringId(pub u32);

/// Public-id table index.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct PublicIdRef(pub u32);

/// Compact resource section codec validation error.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SectionCodecError {
    #[error("section codec budget exceeded: {0}")]
    BudgetExceeded(&'static str),
    #[error("section codec magic {actual:?} does not match {expected:?}")]
    BadMagic { expected: [u8; 8], actual: [u8; 8] },
    #[error("unsupported section codec schema version {actual}; expected {expected}")]
    UnsupportedSchema { actual: u32, expected: u32 },
    #[error("section codec string index {0:?} is out of bounds")]
    StringOutOfBounds(StringId),
    #[error("section codec public id index {0:?} is out of bounds")]
    PublicIdOutOfBounds(PublicIdRef),
    #[error("section codec duplicate public id `{0}`")]
    DuplicatePublicId(String),
}

impl Default for SectionCodecBudget {
    fn default() -> Self {
        Self {
            bytes: 128 * 1024 * 1024,
            records: 1_000_000,
            strings: 1_000_000,
            string_bytes: 64 * 1024 * 1024,
            public_ids: 1_000_000,
            references: 4_000_000,
            depth: 128,
        }
    }
}

impl ProductSectionCodecKind {
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
            Self::ShaderUi => *b"AWSU\r\n\x1a\n",
            Self::DebugSymbols => *b"AWDS\r\n\x1a\n",
            Self::Contracts => *b"AWCT\r\n\x1a\n",
            Self::EntityGraph => *b"AWEG\r\n\x1a\n",
        }
    }

    /// Existing AWFB container section kind for this codec family, when one
    /// has already been introduced.
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
            Self::ShaderUi | Self::Contracts | Self::EntityGraph => None,
        }
    }

    pub fn default_budget(self) -> SectionCodecBudget {
        SectionCodecBudget::default()
    }

    pub const fn affects_code_compatibility(self) -> bool {
        matches!(
            self,
            Self::RuntimeTypes | Self::Entrypoints | Self::AdapterRequirements | Self::Contracts
        )
    }

    pub const fn patch_compatibility(self) -> PatchCompatibility {
        match self {
            Self::RuntimeTypes
            | Self::Entrypoints
            | Self::AdapterRequirements
            | Self::Contracts => PatchCompatibility::RestartRequired,
            Self::ContentCatalog
            | Self::AssetCatalog
            | Self::DisplayCatalog
            | Self::SourceMap
            | Self::LocaleText
            | Self::AudioGraph
            | Self::ShaderUi
            | Self::DebugSymbols
            | Self::EntityGraph => PatchCompatibility::ContentOnly,
        }
    }
}

impl ProductSectionHeader {
    pub fn new(
        codec: ProductSectionCodecKind,
        string_table_len: u32,
        public_id_table_len: u32,
        record_count: u32,
    ) -> Self {
        Self {
            magic: codec.magic(),
            schema_version: PRODUCT_SECTION_SCHEMA_VERSION,
            codec,
            string_table_len,
            public_id_table_len,
            record_count,
        }
    }

    pub fn validate(
        &self,
        bytes: usize,
        budget: SectionCodecBudget,
    ) -> Result<(), SectionCodecError> {
        if self.magic != self.codec.magic() {
            return Err(SectionCodecError::BadMagic {
                expected: self.codec.magic(),
                actual: self.magic,
            });
        }
        if self.schema_version != PRODUCT_SECTION_SCHEMA_VERSION {
            return Err(SectionCodecError::UnsupportedSchema {
                actual: self.schema_version,
                expected: PRODUCT_SECTION_SCHEMA_VERSION,
            });
        }
        check_budget(bytes, budget.bytes, "bytes")?;
        check_budget(self.string_table_len as usize, budget.strings, "strings")?;
        check_budget(
            self.public_id_table_len as usize,
            budget.public_ids,
            "public_ids",
        )?;
        check_budget(self.record_count as usize, budget.records, "records")
    }
}

impl StringTable {
    pub fn new(values: impl IntoIterator<Item = String>) -> Result<Self, SectionCodecError> {
        Self::with_budget(values, SectionCodecBudget::default())
    }

    pub fn with_budget(
        values: impl IntoIterator<Item = String>,
        budget: SectionCodecBudget,
    ) -> Result<Self, SectionCodecError> {
        let mut values = values.into_iter().collect::<Vec<_>>();
        values.sort();
        values.dedup();
        check_budget(values.len(), budget.strings, "strings")?;
        let string_bytes = values.iter().map(String::len).sum::<usize>();
        check_budget(string_bytes, budget.string_bytes, "string_bytes")?;
        Ok(Self { values })
    }

    pub fn get(&self, id: StringId) -> Result<&str, SectionCodecError> {
        self.values
            .get(id.0 as usize)
            .map(String::as_str)
            .ok_or(SectionCodecError::StringOutOfBounds(id))
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn values(&self) -> &[String] {
        &self.values
    }
}

impl PublicIdTable {
    pub fn new(values: impl IntoIterator<Item = String>) -> Result<Self, SectionCodecError> {
        Self::with_budget(values, SectionCodecBudget::default())
    }

    pub fn with_budget(
        values: impl IntoIterator<Item = String>,
        budget: SectionCodecBudget,
    ) -> Result<Self, SectionCodecError> {
        let mut values = values.into_iter().collect::<Vec<_>>();
        values.sort();
        if let Some(duplicate) = values
            .windows(2)
            .find(|pair| pair[0] == pair[1])
            .map(|pair| pair[0].clone())
        {
            return Err(SectionCodecError::DuplicatePublicId(duplicate));
        }
        check_budget(values.len(), budget.public_ids, "public_ids")?;
        let string_bytes = values.iter().map(String::len).sum::<usize>();
        check_budget(string_bytes, budget.string_bytes, "string_bytes")?;
        Ok(Self { values })
    }

    pub fn get(&self, id: PublicIdRef) -> Result<&str, SectionCodecError> {
        self.values
            .get(id.0 as usize)
            .map(String::as_str)
            .ok_or(SectionCodecError::PublicIdOutOfBounds(id))
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn values(&self) -> &[String] {
        &self.values
    }
}

fn check_budget(actual: usize, budget: usize, name: &'static str) -> Result<(), SectionCodecError> {
    if actual > budget {
        Err(SectionCodecError::BudgetExceeded(name))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_codec_kinds_map_to_magic_and_existing_sections() {
        assert_eq!(
            ProductSectionCodecKind::RuntimeTypes.magic(),
            *b"AWRT\r\n\x1a\n"
        );
        assert_eq!(
            ProductSectionCodecKind::RuntimeTypes.section_kind(),
            Some(BundleSectionKind::RuntimeTypes)
        );
        assert_eq!(ProductSectionCodecKind::Contracts.section_kind(), None);
    }

    #[test]
    fn resource_codec_header_validates_magic_schema_and_budgets() {
        let header = ProductSectionHeader::new(ProductSectionCodecKind::Entrypoints, 1, 1, 1);
        header
            .validate(8, SectionCodecBudget::default())
            .expect("header validates");

        let mut bad_magic = header.clone();
        bad_magic.magic = ProductSectionCodecKind::RuntimeTypes.magic();
        assert!(matches!(
            bad_magic.validate(8, SectionCodecBudget::default()),
            Err(SectionCodecError::BadMagic { .. })
        ));

        let mut bad_schema = header;
        bad_schema.schema_version = PRODUCT_SECTION_SCHEMA_VERSION + 1;
        assert_eq!(
            bad_schema.validate(8, SectionCodecBudget::default()),
            Err(SectionCodecError::UnsupportedSchema {
                actual: PRODUCT_SECTION_SCHEMA_VERSION + 1,
                expected: PRODUCT_SECTION_SCHEMA_VERSION,
            })
        );

        assert_eq!(
            ProductSectionHeader::new(ProductSectionCodecKind::Entrypoints, 2, 0, 0).validate(
                8,
                SectionCodecBudget {
                    strings: 1,
                    ..SectionCodecBudget::default()
                }
            ),
            Err(SectionCodecError::BudgetExceeded("strings"))
        );
    }

    #[test]
    fn string_table_sorts_and_deduplicates_values() {
        let table = StringTable::new(["zeta".to_owned(), "alpha".to_owned(), "zeta".to_owned()])
            .expect("strings build");

        assert_eq!(table.values(), &["alpha".to_owned(), "zeta".to_owned()]);
        assert_eq!(table.get(StringId(1)), Ok("zeta"));
        assert_eq!(
            table.get(StringId(2)),
            Err(SectionCodecError::StringOutOfBounds(StringId(2)))
        );
    }

    #[test]
    fn public_id_table_rejects_duplicates_without_deduplicating() {
        let error = PublicIdTable::new([
            "flow.main".to_owned(),
            "flow.other".to_owned(),
            "flow.main".to_owned(),
        ])
        .expect_err("duplicate public ids reject");

        assert_eq!(
            error,
            SectionCodecError::DuplicatePublicId("flow.main".to_owned())
        );
    }

    #[test]
    fn public_id_table_enforces_count_and_byte_budgets() {
        assert_eq!(
            PublicIdTable::with_budget(
                ["flow.main".to_owned()],
                SectionCodecBudget {
                    public_ids: 0,
                    ..SectionCodecBudget::default()
                }
            ),
            Err(SectionCodecError::BudgetExceeded("public_ids"))
        );

        assert_eq!(
            PublicIdTable::with_budget(
                ["flow.main".to_owned()],
                SectionCodecBudget {
                    string_bytes: 4,
                    ..SectionCodecBudget::default()
                }
            ),
            Err(SectionCodecError::BudgetExceeded("string_bytes"))
        );
    }

    #[test]
    fn resource_codec_kind_reports_patch_compatibility_class() {
        assert_eq!(
            ProductSectionCodecKind::AdapterRequirements.patch_compatibility(),
            PatchCompatibility::RestartRequired
        );
        assert_eq!(
            ProductSectionCodecKind::DisplayCatalog.patch_compatibility(),
            PatchCompatibility::ContentOnly
        );
    }
}
