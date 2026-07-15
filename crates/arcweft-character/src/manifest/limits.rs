//! Hard limits owned by the character manifest boundary.

/// Character-manifest collection constrained by a production hard limit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterManifestLimitKind {
    Parts,
    VariantsPerPart,
    VariantsPerManifest,
    Looks,
    Selections,
}

/// Single authority for production character-manifest limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CharacterManifestLimits {
    parts: u64,
    variants_per_part: u64,
    variants_per_manifest: u64,
    looks: u64,
    selections: u64,
}

impl CharacterManifestLimits {
    pub const PRODUCTION: Self = Self {
        parts: 256,
        variants_per_part: 512,
        variants_per_manifest: 16_384,
        looks: 4_096,
        selections: 65_536,
    };

    pub const fn parts(&self) -> u64 {
        self.parts
    }

    pub const fn variants_per_part(&self) -> u64 {
        self.variants_per_part
    }

    pub const fn variants_per_manifest(&self) -> u64 {
        self.variants_per_manifest
    }

    pub const fn looks(&self) -> u64 {
        self.looks
    }

    pub const fn selections(&self) -> u64 {
        self.selections
    }
}
