use super::{
    BundleDigest, ProductSectionCodecKind, SectionCodecError, ViewResourceBudget,
    ViewResourceCompatibility, ViewThemeResource, check_budget, decode_view_section,
    encode_view_section, export_json_bytes, reject_duplicates, saturating_u32, unique_strings,
    validate_canonical_view_transcript,
};

impl ViewThemeResource {
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        section.validate(&ViewResourceBudget::default())?;
        encode_view_section(
            ProductSectionCodecKind::ViewTheme,
            "view_theme",
            &section,
            section.public_ids(),
            section.record_count(),
            &ViewResourceBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        Self::decode_canonical_section_with_budget(bytes, ViewResourceBudget::default())
    }

    pub fn decode_canonical_section_with_budget(
        bytes: &[u8],
        budget: ViewResourceBudget,
    ) -> Result<Self, SectionCodecError> {
        let (mut section, transcript): (Self, _) = decode_view_section(
            bytes,
            ProductSectionCodecKind::ViewTheme,
            "view_theme",
            &budget,
            Self::public_ids,
            Self::record_count,
        )?;
        section.canonicalize();
        section.validate(&budget)?;
        validate_canonical_view_transcript(&transcript, &section)?;
        Ok(section)
    }

    pub fn canonical_digest(&self) -> Result<BundleDigest, SectionCodecError> {
        self.encode_canonical_section()
            .map(|bytes| BundleDigest::of(&bytes))
    }

    pub fn export_json_bytes(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        let digest = section.canonical_digest()?;
        export_json_bytes(ProductSectionCodecKind::ViewTheme, &section, digest)
    }

    pub fn compatibility_with(&self, _next: &Self) -> ViewResourceCompatibility {
        ViewResourceCompatibility::ContentOnly
    }

    fn canonicalize(&mut self) {
        self.palette_overrides
            .sort_by(|left, right| left.color.source_name().cmp(right.color.source_name()));
        self.dark_mode_visual_golden_ids.sort();
    }

    fn validate(&self, budget: &ViewResourceBudget) -> Result<(), SectionCodecError> {
        check_budget(
            self.palette_overrides.len(),
            budget.palette_entries,
            "view_theme_palette_entries",
        )?;
        reject_duplicates(
            self.palette_overrides
                .iter()
                .map(|override_entry| format!("{:?}", override_entry.color)),
            "view_theme_palette_entries",
        )
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(self.dark_mode_visual_golden_ids.clone())
    }

    fn record_count(&self) -> u32 {
        saturating_u32(self.palette_overrides.len())
    }
}
