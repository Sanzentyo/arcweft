use super::budget::{SectionCodecBudget, check_budget};
use super::codec_io::{Cursor, u32_from_usize, usize_from_u32};
use super::error::SectionCodecError;
use super::types::StableId;

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

/// Common enum registry entry. Section-family codecs map their own enum domains
/// to stable numeric codes, and this registry keeps inspection/export readable.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct EnumSymbol {
    pub code: u32,
    pub name: StringId,
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

/// Deduplicated enum symbol registry.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct EnumRegistry {
    symbols: Vec<EnumSymbol>,
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
        Self::from_sorted_unique(values, budget)
    }

    pub fn from_sorted_unique(
        values: Vec<String>,
        budget: SectionCodecBudget,
    ) -> Result<Self, SectionCodecError> {
        reject_duplicate_strings(&values, "strings")?;
        if !values.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(SectionCodecError::NonCanonicalTable("strings"));
        }
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

    pub fn id_for(&self, value: &str) -> Option<StringId> {
        self.values
            .binary_search_by(|candidate| candidate.as_str().cmp(value))
            .ok()
            .and_then(|index| u32::try_from(index).ok())
            .map(StringId)
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

    pub(crate) fn string_bytes(&self) -> usize {
        self.values.iter().map(String::len).sum()
    }

    pub(crate) fn encode_entries(&self, out: &mut Vec<u8>) -> Result<(), SectionCodecError> {
        encode_string_entries(out, &self.values)
    }

    pub(crate) fn decode_entries(
        cursor: &mut Cursor<'_>,
        count: u32,
        budget: SectionCodecBudget,
    ) -> Result<Self, SectionCodecError> {
        let values = decode_string_entries(cursor, count, "strings")?;
        Self::from_sorted_unique(values, budget)
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
        Self::from_sorted_unique(values, budget)
    }

    pub fn from_sorted_unique(
        values: Vec<String>,
        budget: SectionCodecBudget,
    ) -> Result<Self, SectionCodecError> {
        reject_duplicate_strings(&values, "public_ids")?;
        if !values.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(SectionCodecError::NonCanonicalTable("public_ids"));
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

    pub fn id_for(&self, value: &str) -> Option<PublicIdRef> {
        self.values
            .binary_search_by(|candidate| candidate.as_str().cmp(value))
            .ok()
            .and_then(|index| u32::try_from(index).ok())
            .map(PublicIdRef)
    }

    pub fn stable_id(&self, id: PublicIdRef) -> Result<StableId, SectionCodecError> {
        self.get(id).map(StableId::for_key)
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

    pub(crate) fn string_bytes(&self) -> usize {
        self.values.iter().map(String::len).sum()
    }

    pub(crate) fn encode_entries(&self, out: &mut Vec<u8>) -> Result<(), SectionCodecError> {
        encode_string_entries(out, &self.values)
    }

    pub(crate) fn decode_entries(
        cursor: &mut Cursor<'_>,
        count: u32,
        budget: SectionCodecBudget,
    ) -> Result<Self, SectionCodecError> {
        let values = decode_string_entries(cursor, count, "public_ids")?;
        Self::from_sorted_unique(values, budget)
    }
}

impl EnumRegistry {
    pub fn new(symbols: impl IntoIterator<Item = EnumSymbol>) -> Result<Self, SectionCodecError> {
        let mut symbols = symbols.into_iter().collect::<Vec<_>>();
        symbols.sort();
        if let Some(duplicate) = duplicate_enum_code(&symbols) {
            return Err(SectionCodecError::DuplicateEnumCode(duplicate));
        }
        Ok(Self { symbols })
    }

    pub fn with_budget(
        symbols: impl IntoIterator<Item = EnumSymbol>,
        strings: &StringTable,
        budget: SectionCodecBudget,
    ) -> Result<Self, SectionCodecError> {
        let registry = Self::new(symbols)?;
        check_budget(registry.symbols.len(), budget.items, "items")?;
        registry.symbols.iter().try_for_each(|symbol| {
            strings
                .get(symbol.name)
                .map(|_| ())
                .map_err(|_| SectionCodecError::EnumNameOutOfBounds(symbol.name))
        })?;
        Ok(registry)
    }

    pub fn get(&self, code: u32) -> Option<EnumSymbol> {
        self.symbols
            .binary_search_by_key(&code, |symbol| symbol.code)
            .ok()
            .map(|index| self.symbols[index])
    }

    pub fn symbols(&self) -> &[EnumSymbol] {
        &self.symbols
    }

    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    pub(crate) fn encode_entries(&self, out: &mut Vec<u8>) {
        self.symbols.iter().for_each(|symbol| {
            out.extend_from_slice(&symbol.code.to_le_bytes());
            out.extend_from_slice(&symbol.name.0.to_le_bytes());
        });
    }

    pub(crate) fn decode_entries(
        cursor: &mut Cursor<'_>,
        count: u32,
        strings: &StringTable,
        budget: SectionCodecBudget,
    ) -> Result<Self, SectionCodecError> {
        let mut symbols = Vec::with_capacity(usize_from_u32(count)?);
        for _ in 0..count {
            let code = cursor.read_u32()?;
            let name = StringId(cursor.read_u32()?);
            symbols.push(EnumSymbol { code, name });
        }
        Self::with_budget(symbols, strings, budget)
    }
}

pub(crate) fn encoded_string_entries_len(values: &[String]) -> Result<usize, SectionCodecError> {
    values.iter().try_fold(0_usize, |len, value| {
        len.checked_add(4)
            .and_then(|len| len.checked_add(value.len()))
            .ok_or(SectionCodecError::LengthOverflow)
    })
}

fn encode_string_entries(out: &mut Vec<u8>, values: &[String]) -> Result<(), SectionCodecError> {
    values.iter().try_for_each(|value| {
        out.extend_from_slice(&u32_from_usize(value.len())?.to_le_bytes());
        out.extend_from_slice(value.as_bytes());
        Ok(())
    })
}

fn decode_string_entries(
    cursor: &mut Cursor<'_>,
    count: u32,
    table: &'static str,
) -> Result<Vec<String>, SectionCodecError> {
    (0..count)
        .map(|_| {
            let len = usize_from_u32(cursor.read_u32()?)?;
            let bytes = cursor.read_bytes(len)?;
            std::str::from_utf8(bytes)
                .map(str::to_owned)
                .map_err(|_| SectionCodecError::InvalidUtf8(table))
        })
        .collect()
}

fn reject_duplicate_strings(
    values: &[String],
    table: &'static str,
) -> Result<(), SectionCodecError> {
    match table {
        "strings" => duplicate_sorted_string(values)
            .map(SectionCodecError::DuplicateString)
            .map_or(Ok(()), Err),
        "public_ids" => duplicate_sorted_string(values)
            .map(SectionCodecError::DuplicatePublicId)
            .map_or(Ok(()), Err),
        _ => Ok(()),
    }
}

fn duplicate_sorted_string(values: &[String]) -> Option<String> {
    values
        .windows(2)
        .find(|pair| pair[0] == pair[1])
        .map(|pair| pair[0].clone())
}

fn duplicate_enum_code(symbols: &[EnumSymbol]) -> Option<u32> {
    symbols
        .windows(2)
        .find(|pair| pair[0].code == pair[1].code)
        .map(|pair| pair[0].code)
}
