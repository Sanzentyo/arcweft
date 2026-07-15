/// Stable diagnostic families shared by manifest, registration, and tooling boundaries.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDiagnosticCode {
    InvalidCharacterId,
    InvalidPartId,
    InvalidLookId,
    InvalidVariantId,
    ManifestValidation,
    UnknownOwner,
    UnknownPart,
    UnknownLook,
    UnknownVariant,
    DuplicateCatalogOwner,
    ConflictingManifest,
    AliasCollision,
    CanonicalPathCollision,
    ProjectLink,
    EnvironmentBindingProvenance,
    EnvironmentBindingCollision,
    NominalFamilyMismatch,
    NominalOwnerMismatch,
    NominalVariantPartMismatch,
    MissingProvenance,
    StaleProvenance,
    LimitExceeded,
    DiagnosticLimitExceeded,
    RevisionExhausted,
    WorldMismatch,
}

impl CharacterDiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidCharacterId => "character.id.invalid_character",
            Self::InvalidPartId => "character.id.invalid_part",
            Self::InvalidLookId => "character.id.invalid_look",
            Self::InvalidVariantId => "character.id.invalid_variant",
            Self::ManifestValidation => "character.manifest.validation",
            Self::UnknownOwner => "character.registration.unknown_owner",
            Self::UnknownPart => "character.lookup.unknown_part",
            Self::UnknownLook => "character.lookup.unknown_look",
            Self::UnknownVariant => "character.lookup.unknown_variant",
            Self::DuplicateCatalogOwner => "character.catalog.duplicate_owner",
            Self::ConflictingManifest => "character.registration.conflicting_manifest",
            Self::AliasCollision => "character.registration.alias_collision",
            Self::CanonicalPathCollision => "character.registration.canonical_path_collision",
            Self::ProjectLink => "character.registration.project_link",
            Self::EnvironmentBindingProvenance => {
                "character.registration.environment_binding_provenance"
            }
            Self::EnvironmentBindingCollision => {
                "character.registration.environment_binding_collision"
            }
            Self::NominalFamilyMismatch => "character.nominal.family_mismatch",
            Self::NominalOwnerMismatch => "character.nominal.owner_mismatch",
            Self::NominalVariantPartMismatch => "character.nominal.variant_part_mismatch",
            Self::MissingProvenance => "character.registration.missing_provenance",
            Self::StaleProvenance => "character.registration.stale_provenance",
            Self::LimitExceeded => "character.registration.limit_exceeded",
            Self::DiagnosticLimitExceeded => "character.registration.diagnostic_limit",
            Self::RevisionExhausted => "character.registration.revision_exhausted",
            Self::WorldMismatch => "character.registration.world_mismatch",
        }
    }
}
