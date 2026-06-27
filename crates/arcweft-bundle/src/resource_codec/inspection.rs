use super::error::SectionCodecError;
use super::field::{FieldRequirement, ResourceWireType};
use super::kind::ProductSectionCodecKind;
use super::table::EnumSymbol;
use super::wire::ProductResourceEnvelope;
use crate::container::BundleDigest;

/// JSON inspection view for humans and tools. It is intentionally produced from
/// compact bytes and must not be treated as an alternate product resource codec.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ResourceInspection {
    pub schema_version: u32,
    pub codec: ProductSectionCodecKind,
    pub codec_name: String,
    pub strings: Vec<String>,
    pub public_ids: Vec<String>,
    pub enum_symbols: Vec<EnumSymbolInspection>,
    pub fields: Vec<ResourceFieldInspection>,
    pub record_count: u32,
    pub canonical_digest: BundleDigest,
}

/// Human-readable enum symbol in an inspection view.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct EnumSymbolInspection {
    pub code: u32,
    pub name: String,
}

/// Human-readable field entry in an inspection view.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ResourceFieldInspection {
    pub id: u16,
    pub requirement: FieldRequirement,
    pub wire_type: ResourceWireType,
    pub nesting_depth: u16,
    pub reference_count: u16,
    pub payload_len: usize,
}

impl ProductResourceEnvelope {
    pub fn inspection(&self) -> Result<ResourceInspection, SectionCodecError> {
        let enum_symbols = self
            .enums
            .symbols()
            .iter()
            .copied()
            .map(|symbol| enum_symbol_inspection(self, symbol))
            .collect::<Result<Vec<_>, _>>()?;
        let fields = self
            .fields
            .iter()
            .map(|field| ResourceFieldInspection {
                id: field.id.0,
                requirement: field.requirement,
                wire_type: field.wire_type,
                nesting_depth: field.nesting_depth,
                reference_count: field.reference_count,
                payload_len: field.payload.len(),
            })
            .collect();
        Ok(ResourceInspection {
            schema_version: self.header.schema_version,
            codec: self.header.codec,
            codec_name: self.header.codec.as_str().to_owned(),
            strings: self.strings.values().to_vec(),
            public_ids: self.public_ids.values().to_vec(),
            enum_symbols,
            fields,
            record_count: self.header.record_count,
            canonical_digest: self.canonical_digest()?,
        })
    }

    pub fn inspection_json_bytes(&self) -> Result<Vec<u8>, SectionCodecError> {
        serde_json::to_vec_pretty(&self.inspection()?)
            .map_err(|_| SectionCodecError::NonCanonicalTable("inspection_json"))
    }
}

fn enum_symbol_inspection(
    envelope: &ProductResourceEnvelope,
    symbol: EnumSymbol,
) -> Result<EnumSymbolInspection, SectionCodecError> {
    envelope
        .strings
        .get(symbol.name)
        .map(|name| EnumSymbolInspection {
            code: symbol.code,
            name: name.to_owned(),
        })
}
