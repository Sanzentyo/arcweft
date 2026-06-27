use super::budget::{SectionCodecBudget, check_budget};
use super::codec_io::{read_array, read_u32, read_u64};
use super::error::SectionCodecError;
use super::kind::ProductSectionCodecKind;

/// Default schema version for compact product resource sections.
pub const PRODUCT_SECTION_SCHEMA_VERSION: u32 = 1;

/// Fixed byte length of the compact resource wire header.
pub const PRODUCT_SECTION_HEADER_LEN: usize = 48;

/// Compact resource payloads use fixed little-endian lengths and no interior
/// padding. The surrounding AWFB container still aligns section payload starts.
pub const PRODUCT_SECTION_WIRE_ALIGNMENT: usize = 1;

/// Common compact-section header after container-level AWFB validation.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ProductSectionHeader {
    pub magic: [u8; 8],
    pub schema_version: u32,
    pub codec: ProductSectionCodecKind,
    pub string_table_len: u32,
    pub public_id_table_len: u32,
    pub enum_registry_len: u32,
    pub field_count: u32,
    pub record_count: u32,
    pub body_len: u64,
}

impl ProductSectionHeader {
    pub fn new(
        codec: ProductSectionCodecKind,
        string_table_len: u32,
        public_id_table_len: u32,
        record_count: u32,
    ) -> Self {
        Self::with_tables_and_body(
            codec,
            string_table_len,
            public_id_table_len,
            0,
            0,
            record_count,
            0,
        )
    }

    pub fn with_tables_and_body(
        codec: ProductSectionCodecKind,
        string_table_len: u32,
        public_id_table_len: u32,
        enum_registry_len: u32,
        field_count: u32,
        record_count: u32,
        body_len: u64,
    ) -> Self {
        Self {
            magic: codec.magic(),
            schema_version: PRODUCT_SECTION_SCHEMA_VERSION,
            codec,
            string_table_len,
            public_id_table_len,
            enum_registry_len,
            field_count,
            record_count,
            body_len,
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
        check_budget(self.field_count as usize, budget.items, "items")?;
        check_budget(self.record_count as usize, budget.records, "records")?;
        let fan_out = self
            .string_table_len
            .saturating_add(self.public_id_table_len)
            .saturating_add(self.enum_registry_len)
            .saturating_add(self.field_count) as usize;
        check_budget(fan_out, budget.table_fan_out, "table_fan_out")
    }

    pub fn validate_for(
        &self,
        expected: ProductSectionCodecKind,
        bytes: usize,
        budget: SectionCodecBudget,
    ) -> Result<(), SectionCodecError> {
        if self.codec != expected {
            return Err(SectionCodecError::UnexpectedCodec {
                expected,
                actual: self.codec,
            });
        }
        self.validate(bytes, budget)
    }

    pub(crate) fn encode_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.magic);
        out.extend_from_slice(&self.schema_version.to_le_bytes());
        out.extend_from_slice(&self.codec.encoded().to_le_bytes());
        out.extend_from_slice(&self.string_table_len.to_le_bytes());
        out.extend_from_slice(&self.public_id_table_len.to_le_bytes());
        out.extend_from_slice(&self.enum_registry_len.to_le_bytes());
        out.extend_from_slice(&self.field_count.to_le_bytes());
        out.extend_from_slice(&self.record_count.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&self.body_len.to_le_bytes());
    }

    pub(crate) fn decode_from(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        if bytes.len() < PRODUCT_SECTION_HEADER_LEN {
            return Err(SectionCodecError::Truncated);
        }
        let magic = read_array::<8>(bytes, 0)?;
        let schema_version = read_u32(bytes, 8)?;
        let codec_tag = read_u32(bytes, 12)?;
        let codec = ProductSectionCodecKind::from_encoded(codec_tag)
            .ok_or(SectionCodecError::UnsupportedCodecTag(codec_tag))?;
        let string_table_len = read_u32(bytes, 16)?;
        let public_id_table_len = read_u32(bytes, 20)?;
        let enum_registry_len = read_u32(bytes, 24)?;
        let field_count = read_u32(bytes, 28)?;
        let record_count = read_u32(bytes, 32)?;
        let reserved = read_u32(bytes, 36)?;
        if reserved != 0 {
            return Err(SectionCodecError::NonCanonicalTable("header_reserved"));
        }
        let body_len = read_u64(bytes, 40)?;
        Ok(Self {
            magic,
            schema_version,
            codec,
            string_table_len,
            public_id_table_len,
            enum_registry_len,
            field_count,
            record_count,
            body_len,
        })
    }
}
