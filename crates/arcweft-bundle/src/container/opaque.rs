use super::BundleSectionKind;

/// Raw AWFB section kind code preserved even when this runtime does not know it.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(transparent)]
pub struct SectionKindCode(pub u32);

impl SectionKindCode {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn encoded(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn known(self) -> Option<BundleSectionKind> {
        BundleSectionKind::from_encoded(self.0)
    }
}

impl From<BundleSectionKind> for SectionKindCode {
    fn from(value: BundleSectionKind) -> Self {
        Self(value.encoded())
    }
}

/// Known/unknown view of a section descriptor's raw kind code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedSectionKind {
    Known(BundleSectionKind),
    UnknownOptional(SectionKindCode),
}

impl DecodedSectionKind {
    #[must_use]
    pub const fn known(self) -> Option<BundleSectionKind> {
        match self {
            Self::Known(kind) => Some(kind),
            Self::UnknownOptional(_) => None,
        }
    }

    #[must_use]
    pub const fn code(self) -> SectionKindCode {
        match self {
            Self::Known(kind) => SectionKindCode(kind.encoded()),
            Self::UnknownOptional(code) => code,
        }
    }
}
