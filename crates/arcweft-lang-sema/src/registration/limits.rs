use arcweft_source::MAX_REGISTRATION_SOURCE_BYTES;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterRegistrationLimitKind {
    SourceBytes,
    Catalogs,
    ManifestOccurrences,
    Owners,
    Parts,
    VariantsPerPart,
    VariantsPerManifest,
    Looks,
    Selections,
    Documents,
    Diagnostics,
    Work,
}

pub struct CharacterRegistrationLimits {
    catalogs: u64,
    manifest_occurrences: u64,
    owners: u64,
    documents: u64,
}

impl CharacterRegistrationLimits {
    pub const PRODUCTION: Self = Self {
        catalogs: 64,
        manifest_occurrences: 1_024,
        owners: 512,
        documents: 4_096,
    };

    pub const fn source_bytes(&self) -> u64 {
        MAX_REGISTRATION_SOURCE_BYTES
    }

    pub const fn catalogs(&self) -> u64 {
        self.catalogs
    }

    pub const fn manifest_occurrences(&self) -> u64 {
        self.manifest_occurrences
    }

    pub const fn owners(&self) -> u64 {
        self.owners
    }

    pub const fn parts(&self) -> u64 {
        arcweft_character::manifest::limits::CharacterManifestLimits::PRODUCTION.parts()
    }

    pub const fn variants_per_part(&self) -> u64 {
        arcweft_character::manifest::limits::CharacterManifestLimits::PRODUCTION.variants_per_part()
    }

    pub const fn variants_per_manifest(&self) -> u64 {
        arcweft_character::manifest::limits::CharacterManifestLimits::PRODUCTION
            .variants_per_manifest()
    }

    pub const fn looks(&self) -> u64 {
        arcweft_character::manifest::limits::CharacterManifestLimits::PRODUCTION.looks()
    }

    pub const fn selections(&self) -> u64 {
        arcweft_character::manifest::limits::CharacterManifestLimits::PRODUCTION.selections()
    }

    pub const fn documents(&self) -> u64 {
        self.documents
    }

    pub const fn diagnostics(&self) -> u64 {
        arcweft_lang_hir::symbol::ProjectSymbolLimits::PRODUCTION.diagnostics()
    }

    pub const fn work(&self) -> u64 {
        arcweft_lang_hir::symbol::ProjectSymbolLimits::PRODUCTION.work()
    }
}
