use super::{
    BundleDigest, ProductSectionCodecKind, SectionCodecError, ViewResourceBudget,
    ViewResourceCompatibility, ViewTextResource, ViewTextSourceKind, check_budget,
    decode_view_section, encode_view_section, export_json_bytes, reject_duplicates, saturating_u32,
    unique_strings, valid_identifier, validate_canonical_view_transcript,
};

impl ViewTextResource {
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        section.validate(&ViewResourceBudget::default())?;
        encode_view_section(
            ProductSectionCodecKind::ViewText,
            "view_text",
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
            ProductSectionCodecKind::ViewText,
            "view_text",
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
        export_json_bytes(ProductSectionCodecKind::ViewText, &section, digest)
    }

    pub fn compatibility_with(&self, next: &Self) -> ViewResourceCompatibility {
        if self == next {
            return ViewResourceCompatibility::ContentOnly;
        }
        if self.redactions != next.redactions {
            return ViewResourceCompatibility::RestartRequired;
        }
        ViewResourceCompatibility::ContentOnly
    }

    fn canonicalize(&mut self) {
        self.sources
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.localized
            .sort_by(|left, right| (&left.key, &left.locale).cmp(&(&right.key, &right.locale)));
        self.rich_text_documents
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.display_frames
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.reveal_policies
            .sort_by(|left, right| left.text_source.cmp(&right.text_source));
        self.cursor_policies
            .sort_by(|left, right| left.text_source.cmp(&right.text_source));
        self.redactions
            .sort_by(|left, right| left.text_source.cmp(&right.text_source));
    }

    fn validate(&self, budget: &ViewResourceBudget) -> Result<(), SectionCodecError> {
        let text_record_count = self
            .sources
            .len()
            .saturating_add(self.localized.len())
            .saturating_add(self.rich_text_documents.len())
            .saturating_add(self.display_frames.len());
        check_budget(text_record_count, budget.text_sources, "view_text_sources")?;
        check_budget(
            self.source_ranges.len(),
            budget.source_map_refs,
            "view_text_source_ranges",
        )?;
        reject_duplicates(
            self.sources.iter().map(|source| source.public_id.clone()),
            "view_text_sources",
        )?;
        reject_duplicates(
            self.localized.iter().map(|entry| {
                format!(
                    "{}\u{0}{}",
                    entry.key,
                    entry.locale.as_deref().unwrap_or_default()
                )
            }),
            "view_localized_text",
        )?;
        reject_duplicates(
            self.rich_text_documents
                .iter()
                .map(|document| document.public_id.clone()),
            "view_rich_text_documents",
        )?;
        reject_duplicates(
            self.display_frames
                .iter()
                .map(|frame| frame.public_id.clone()),
            "view_display_frames",
        )?;
        reject_duplicates(
            self.redactions
                .iter()
                .map(|redaction| redaction.text_source.clone()),
            "view_text_redactions",
        )?;
        let valid_sources = self.sources.iter().all(|source| match &source.kind {
            ViewTextSourceKind::Projection { path } => {
                !path.is_empty() && path.iter().all(|segment| valid_identifier(segment))
            }
            ViewTextSourceKind::Local { name } => valid_identifier(name),
            ViewTextSourceKind::RichTextDocument { document } => self
                .rich_text_documents
                .iter()
                .any(|entry| entry.public_id == *document),
            ViewTextSourceKind::DisplayFrame { frame } => self
                .display_frames
                .iter()
                .any(|entry| entry.public_id == *frame),
            ViewTextSourceKind::Dialogue { parameter, .. } => valid_identifier(parameter),
            ViewTextSourceKind::Literal { .. } | ViewTextSourceKind::Localized { .. } => true,
        });
        let valid_display_frames = self.display_frames.iter().all(|entry| {
            usize::try_from(entry.stage_index)
                .ok()
                .and_then(|index| entry.frame.stage(index))
                .is_some()
                && entry.frame.validate().is_ok()
        });
        if valid_sources && valid_display_frames {
            Ok(())
        } else {
            Err(SectionCodecError::NonCanonicalTable("view_text_projection"))
        }
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(
            self.sources
                .iter()
                .flat_map(|source| {
                    [Some(source.public_id.clone())]
                        .into_iter()
                        .chain(text_source_kind_public_ids(&source.kind).map(Some))
                        .flatten()
                })
                .chain(self.localized.iter().flat_map(|entry| {
                    [Some(entry.key.clone()), entry.locale.clone()]
                        .into_iter()
                        .flatten()
                }))
                .chain(
                    self.rich_text_documents
                        .iter()
                        .map(|document| document.public_id.clone()),
                )
                .chain(
                    self.display_frames
                        .iter()
                        .map(|frame| frame.public_id.clone()),
                )
                .chain(
                    self.reveal_policies
                        .iter()
                        .map(|policy| policy.text_source.clone()),
                )
                .chain(
                    self.cursor_policies
                        .iter()
                        .map(|policy| policy.text_source.clone()),
                )
                .chain(
                    self.redactions
                        .iter()
                        .map(|redaction| redaction.text_source.clone()),
                ),
        )
    }

    fn record_count(&self) -> u32 {
        saturating_u32(
            self.sources
                .len()
                .saturating_add(self.localized.len())
                .saturating_add(self.rich_text_documents.len())
                .saturating_add(self.display_frames.len()),
        )
    }
}

fn text_source_kind_public_ids(kind: &ViewTextSourceKind) -> impl Iterator<Item = String> + '_ {
    match kind {
        ViewTextSourceKind::Literal { .. }
        | ViewTextSourceKind::Projection { .. }
        | ViewTextSourceKind::Local { .. }
        | ViewTextSourceKind::Dialogue { .. } => Vec::new(),
        ViewTextSourceKind::RichTextDocument { document } => vec![document.clone()],
        ViewTextSourceKind::DisplayFrame { frame } => vec![frame.clone()],
        ViewTextSourceKind::Localized { key, locale } => [Some(key.clone()), locale.clone()]
            .into_iter()
            .flatten()
            .collect(),
    }
    .into_iter()
}
