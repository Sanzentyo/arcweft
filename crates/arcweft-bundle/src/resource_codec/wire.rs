use super::budget::{SectionCodecBudget, check_budget};
use super::codec_io::{Cursor, u32_from_usize, u64_from_usize, usize_from_u32, usize_from_u64};
use super::error::SectionCodecError;
use super::field::{FIELD_HEADER_LEN, validate_field_budgets, validate_strict_field_order};
use super::field::{FieldRegistry, ResourceField};
use super::header::PRODUCT_SECTION_HEADER_LEN;
use super::header::ProductSectionHeader;
use super::kind::ProductSectionCodecKind;
use super::table::encoded_string_entries_len;
use super::table::{EnumRegistry, PublicIdTable, StringTable};
use crate::container::BundleDigest;

/// Decoded compact resource section plus forward-compatibility accounting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedResourceSection {
    pub envelope: ProductResourceEnvelope,
    pub skipped_unknown_optional_fields: usize,
}

/// Compact resource section envelope shared by all migrated product families.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ProductResourceEnvelope {
    pub header: ProductSectionHeader,
    pub strings: StringTable,
    pub public_ids: PublicIdTable,
    pub enums: EnumRegistry,
    pub fields: Vec<ResourceField>,
}

impl ProductResourceEnvelope {
    pub fn new(
        codec: ProductSectionCodecKind,
        strings: StringTable,
        public_ids: PublicIdTable,
        enums: EnumRegistry,
        fields: impl IntoIterator<Item = ResourceField>,
        record_count: u32,
    ) -> Result<Self, SectionCodecError> {
        Self::with_budget(
            codec,
            strings,
            public_ids,
            enums,
            fields,
            record_count,
            SectionCodecBudget::default(),
        )
    }

    pub fn with_budget(
        codec: ProductSectionCodecKind,
        strings: StringTable,
        public_ids: PublicIdTable,
        enums: EnumRegistry,
        fields: impl IntoIterator<Item = ResourceField>,
        record_count: u32,
        budget: SectionCodecBudget,
    ) -> Result<Self, SectionCodecError> {
        let mut fields = fields.into_iter().collect::<Vec<_>>();
        fields.sort_by(|left, right| left.canonical_key().cmp(&right.canonical_key()));
        validate_table_budgets(&strings, &public_ids, &enums, budget)?;
        validate_field_budgets(&fields, budget)?;
        let body_len = encoded_body_len(&strings, &public_ids, &enums, &fields)?;
        let header = ProductSectionHeader::with_tables_and_body(
            codec,
            u32_from_usize(strings.len())?,
            u32_from_usize(public_ids.len())?,
            u32_from_usize(enums.len())?,
            u32_from_usize(fields.len())?,
            record_count,
            body_len,
        );
        header.validate(
            PRODUCT_SECTION_HEADER_LEN.saturating_add(usize_from_u64(body_len)?),
            budget,
        )?;
        Ok(Self {
            header,
            strings,
            public_ids,
            enums,
            fields,
        })
    }

    pub fn encode_canonical(&self) -> Result<Vec<u8>, SectionCodecError> {
        self.header.validate(
            PRODUCT_SECTION_HEADER_LEN.saturating_add(usize_from_u64(self.header.body_len)?),
            SectionCodecBudget::default(),
        )?;
        let mut body = Vec::with_capacity(usize_from_u64(self.header.body_len)?);
        self.strings.encode_entries(&mut body)?;
        self.public_ids.encode_entries(&mut body)?;
        self.enums.encode_entries(&mut body);
        self.fields
            .iter()
            .try_for_each(|field| field.encode_into(&mut body))?;
        if body.len() as u64 != self.header.body_len {
            return Err(SectionCodecError::NonCanonicalTable("body_len"));
        }
        let mut out = Vec::with_capacity(PRODUCT_SECTION_HEADER_LEN + body.len());
        self.header.encode_into(&mut out);
        out.extend_from_slice(&body);
        Ok(out)
    }

    pub fn decode_with_registry(
        bytes: &[u8],
        expected: ProductSectionCodecKind,
        registry: &FieldRegistry,
        budget: SectionCodecBudget,
    ) -> Result<DecodedResourceSection, SectionCodecError> {
        check_budget(bytes.len(), budget.bytes, "bytes")?;
        let header = ProductSectionHeader::decode_from(bytes)?;
        header.validate_for(expected, bytes.len(), budget)?;
        let body_len = usize_from_u64(header.body_len)?;
        let end = PRODUCT_SECTION_HEADER_LEN
            .checked_add(body_len)
            .ok_or(SectionCodecError::LengthOverflow)?;
        if bytes.len() < end {
            return Err(SectionCodecError::Truncated);
        }
        if bytes.len() != end {
            return Err(SectionCodecError::TrailingBytes);
        }
        let mut cursor = Cursor::new(&bytes[PRODUCT_SECTION_HEADER_LEN..end]);
        let strings = StringTable::decode_entries(&mut cursor, header.string_table_len, budget)?;
        let public_ids =
            PublicIdTable::decode_entries(&mut cursor, header.public_id_table_len, budget)?;
        let enums =
            EnumRegistry::decode_entries(&mut cursor, header.enum_registry_len, &strings, budget)?;
        let mut raw_fields = Vec::with_capacity(usize_from_u32(header.field_count)?);
        for _ in 0..header.field_count {
            let field = ResourceField::decode_from(&mut cursor)?;
            if field.nesting_depth as usize > budget.depth {
                return Err(SectionCodecError::BudgetExceeded("depth"));
            }
            if field.reference_count as usize > budget.references {
                return Err(SectionCodecError::BudgetExceeded("references"));
            }
            raw_fields.push(field);
        }
        if cursor.remaining() != 0 {
            return Err(SectionCodecError::TrailingBytes);
        }
        validate_strict_field_order(&raw_fields)?;
        validate_field_budgets(&raw_fields, budget)?;

        let mut fields = Vec::with_capacity(raw_fields.len());
        let mut skipped_unknown_optional_fields = 0_usize;
        for field in raw_fields {
            if registry.validate_known_field(&field)? {
                fields.push(field);
            } else if field.is_required() {
                return Err(SectionCodecError::UnknownRequiredField(field.id));
            } else {
                skipped_unknown_optional_fields += 1;
            }
        }
        registry.validate_required_presence(&fields)?;
        let envelope = ProductResourceEnvelope::with_budget(
            header.codec,
            strings,
            public_ids,
            enums,
            fields,
            header.record_count,
            budget,
        )?;
        Ok(DecodedResourceSection {
            envelope,
            skipped_unknown_optional_fields,
        })
    }

    pub fn decode_all_fields(
        bytes: &[u8],
        expected: ProductSectionCodecKind,
        budget: SectionCodecBudget,
    ) -> Result<Self, SectionCodecError> {
        check_budget(bytes.len(), budget.bytes, "bytes")?;
        let header = ProductSectionHeader::decode_from(bytes)?;
        header.validate_for(expected, bytes.len(), budget)?;
        let body_len = usize_from_u64(header.body_len)?;
        let end = PRODUCT_SECTION_HEADER_LEN
            .checked_add(body_len)
            .ok_or(SectionCodecError::LengthOverflow)?;
        if bytes.len() < end {
            return Err(SectionCodecError::Truncated);
        }
        if bytes.len() != end {
            return Err(SectionCodecError::TrailingBytes);
        }
        let mut cursor = Cursor::new(&bytes[PRODUCT_SECTION_HEADER_LEN..end]);
        let strings = StringTable::decode_entries(&mut cursor, header.string_table_len, budget)?;
        let public_ids =
            PublicIdTable::decode_entries(&mut cursor, header.public_id_table_len, budget)?;
        let enums =
            EnumRegistry::decode_entries(&mut cursor, header.enum_registry_len, &strings, budget)?;
        let mut fields = Vec::with_capacity(usize_from_u32(header.field_count)?);
        for _ in 0..header.field_count {
            fields.push(ResourceField::decode_from(&mut cursor)?);
        }
        if cursor.remaining() != 0 {
            return Err(SectionCodecError::TrailingBytes);
        }
        validate_strict_field_order(&fields)?;
        validate_field_budgets(&fields, budget)?;
        ProductResourceEnvelope::with_budget(
            header.codec,
            strings,
            public_ids,
            enums,
            fields,
            header.record_count,
            budget,
        )
    }

    pub fn canonical_digest(&self) -> Result<BundleDigest, SectionCodecError> {
        self.encode_canonical()
            .map(|bytes| BundleDigest::of(&bytes))
    }
}

fn validate_table_budgets(
    strings: &StringTable,
    public_ids: &PublicIdTable,
    enums: &EnumRegistry,
    budget: SectionCodecBudget,
) -> Result<(), SectionCodecError> {
    check_budget(strings.len(), budget.strings, "strings")?;
    check_budget(public_ids.len(), budget.public_ids, "public_ids")?;
    check_budget(enums.len(), budget.items, "items")?;
    check_budget(
        strings
            .string_bytes()
            .checked_add(public_ids.string_bytes())
            .ok_or(SectionCodecError::LengthOverflow)?,
        budget.string_bytes,
        "string_bytes",
    )
}

fn encoded_body_len(
    strings: &StringTable,
    public_ids: &PublicIdTable,
    enums: &EnumRegistry,
    fields: &[ResourceField],
) -> Result<u64, SectionCodecError> {
    let string_len = encoded_string_entries_len(strings.values())?;
    let public_id_len = encoded_string_entries_len(public_ids.values())?;
    let enum_len = enums
        .len()
        .checked_mul(8)
        .ok_or(SectionCodecError::LengthOverflow)?;
    let field_len = fields.iter().try_fold(0_usize, |len, field| {
        len.checked_add(FIELD_HEADER_LEN)
            .and_then(|len| len.checked_add(field.payload.len()))
            .ok_or(SectionCodecError::LengthOverflow)
    })?;
    u64_from_usize(
        string_len
            .checked_add(public_id_len)
            .and_then(|len| len.checked_add(enum_len))
            .and_then(|len| len.checked_add(field_len))
            .ok_or(SectionCodecError::LengthOverflow)?,
    )
}
