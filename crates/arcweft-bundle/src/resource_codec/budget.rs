use super::error::SectionCodecError;

/// Decoder and table validation budget for a compact product resource section.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SectionCodecBudget {
    /// Maximum decoded payload byte length.
    pub bytes: usize,
    /// Maximum common-envelope field/item count.
    pub items: usize,
    /// Maximum section-family record count declared in the header.
    pub records: usize,
    /// Maximum string table entries.
    pub strings: usize,
    /// Maximum aggregate UTF-8 string bytes across string and public-id tables.
    pub string_bytes: usize,
    /// Maximum public-id table entries.
    pub public_ids: usize,
    /// Maximum declared cross-reference count across common field headers.
    pub references: usize,
    /// Maximum declared nesting depth across common field headers.
    pub depth: usize,
    /// Maximum aggregate fan-out across shared string/public-id/enum/field tables.
    pub table_fan_out: usize,
}

impl Default for SectionCodecBudget {
    fn default() -> Self {
        Self {
            bytes: 128 * 1024 * 1024,
            items: 1_000_000,
            records: 1_000_000,
            strings: 1_000_000,
            string_bytes: 64 * 1024 * 1024,
            public_ids: 1_000_000,
            references: 4_000_000,
            depth: 128,
            table_fan_out: 4_000_000,
        }
    }
}

pub(crate) fn check_budget(
    actual: usize,
    budget: usize,
    name: &'static str,
) -> Result<(), SectionCodecError> {
    if actual > budget {
        Err(SectionCodecError::BudgetExceeded(name))
    } else {
        Ok(())
    }
}
