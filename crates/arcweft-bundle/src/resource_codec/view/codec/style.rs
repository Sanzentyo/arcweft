use super::{
    BTreeSet, BundleDigest, ProductSectionCodecKind, PublicIdTable, SectionCodecError,
    SourceMapIndex, SourceMapSourceId, ViewResourceBudget, ViewResourceCompatibility,
    ViewSpecifiedValue, ViewStyleDeclaration, ViewStylePatch, ViewStyleProgram, ViewStyleResource,
    ViewStyleRule, ViewStyleSelector, ViewStyleSheet, ViewStyleSourceId, ViewStyleToken,
    check_budget, decode_view_section, encode_view_section, export_json_bytes, reject_duplicates,
    saturating_u32, style_environment, unique_strings, valid_resource_identity,
    validate_canonical_view_transcript,
};

impl ViewStyleResource {
    /// Validates guarded-rule provenance against the decoded product source map.
    pub fn validate_environment_sources(
        &self,
        sources: &SourceMapIndex,
        source_id: &SourceMapSourceId,
    ) -> Result<(), SectionCodecError> {
        style_environment::validate_source_extents(self, sources, source_id).map_err(Into::into)
    }

    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize()?;
        section.validate(&ViewResourceBudget::default())?;
        encode_view_section(
            ProductSectionCodecKind::ViewStyle,
            "view_style",
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
        let (section, transcript): (Self, _) = decode_view_section(
            bytes,
            ProductSectionCodecKind::ViewStyle,
            "view_style",
            &budget,
            Self::public_ids,
            Self::record_count,
        )?;
        if !section.is_canonical_order() {
            return Err(SectionCodecError::NonCanonicalTable(
                "view_style_inventory_order",
            ));
        }
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
        section.canonicalize()?;
        let digest = section.canonical_digest()?;
        export_json_bytes(ProductSectionCodecKind::ViewStyle, &section, digest)
    }

    pub fn compatibility_with(&self, next: &Self) -> ViewResourceCompatibility {
        if self == next {
            return ViewResourceCompatibility::ContentOnly;
        }
        if self.adapter_requirements != next.adapter_requirements {
            return ViewResourceCompatibility::RestartRequired;
        }
        ViewResourceCompatibility::ContentOnly
    }

    /// Canonical public-ID table used by compiler source-map lowering and
    /// section encoding. Callers must not reproduce this inventory manually.
    pub fn public_id_table(&self) -> Result<PublicIdTable, SectionCodecError> {
        PublicIdTable::new(self.public_ids())
    }

    pub(in crate::resource_codec::view) fn canonicalize(
        &mut self,
    ) -> Result<(), SectionCodecError> {
        let mut source_order = (0..self.source_map_refs.len()).collect::<Vec<_>>();
        source_order.sort_by_key(|index| {
            let range = self.source_map_refs[*index];
            (range.source, range.start_byte, range.end_byte)
        });

        if source_order
            .iter()
            .enumerate()
            .any(|(new_index, old_index)| new_index != *old_index)
        {
            let mut source_rebase = vec![ViewStyleSourceId::new(0); source_order.len()];
            let mut canonical_ranges = Vec::with_capacity(source_order.len());
            for (new_index, old_index) in source_order.into_iter().enumerate() {
                let new_index =
                    u32::try_from(new_index).map_err(|_| SectionCodecError::LengthOverflow)?;
                source_rebase[old_index] = ViewStyleSourceId::new(new_index);
                canonical_ranges.push(self.source_map_refs[old_index]);
            }
            self.rebase_source_ids(&source_rebase)?;
            self.source_map_refs = canonical_ranges;
        }

        self.adapter_requirements.sort_by_key(|reference| {
            (
                reference.section_kind,
                reference.section_id,
                reference.content_digest,
                reference.public_id,
            )
        });
        Ok(())
    }

    fn rebase_source_ids(
        &mut self,
        source_rebase: &[ViewStyleSourceId],
    ) -> Result<(), SectionCodecError> {
        let source = |id: ViewStyleSourceId| {
            source_rebase.get(id.value() as usize).copied().ok_or(
                SectionCodecError::NonCanonicalTable("view_style_source_ids"),
            )
        };
        let sheets = self
            .program
            .sheets()
            .iter()
            .map(|sheet| {
                let tokens = sheet
                    .tokens()
                    .iter()
                    .map(|token| {
                        ViewStyleToken::new(
                            token.id().clone(),
                            token.value_kind(),
                            token.value().clone(),
                            source(token.source())?,
                        )
                        .map_err(|_| SectionCodecError::NonCanonicalTable("view_style_program"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let rules = sheet
                    .rules()
                    .iter()
                    .map(|rule| {
                        let declarations = rule
                            .declarations()
                            .iter()
                            .map(|declaration| {
                                ViewStyleDeclaration::new(
                                    declaration.property(),
                                    declaration.value().clone(),
                                    declaration.op(),
                                    source(declaration.source())?,
                                )
                                .map_err(|_| {
                                    SectionCodecError::NonCanonicalTable("view_style_program")
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        ViewStyleRule::new(
                            rule.selector().clone(),
                            rule.environment()
                                .map(|condition| condition.try_map_sources(&source))
                                .transpose()?,
                            declarations,
                            rule.source_order(),
                            source(rule.source())?,
                        )
                        .map_err(|_| SectionCodecError::NonCanonicalTable("view_style_program"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                ViewStyleSheet::new(sheet.id().clone(), tokens, rules)
                    .map_err(|_| SectionCodecError::NonCanonicalTable("view_style_program"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let patches = self
            .program
            .patches()
            .iter()
            .map(|patch| {
                patch
                    .declarations()
                    .iter()
                    .map(|declaration| {
                        ViewStyleDeclaration::new(
                            declaration.property(),
                            declaration.value().clone(),
                            declaration.op(),
                            source(declaration.source())?,
                        )
                        .map_err(|_| SectionCodecError::NonCanonicalTable("view_style_program"))
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(|declarations| ViewStylePatch::new(patch.id(), declarations))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.program = ViewStyleProgram::try_new(sheets, patches)
            .map_err(|_| SectionCodecError::NonCanonicalTable("view_style_program"))?;
        Ok(())
    }

    fn is_canonical_order(&self) -> bool {
        self.program
            .sheets()
            .windows(2)
            .all(|pair| pair[0].id() < pair[1].id())
            && self
                .program
                .patches()
                .windows(2)
                .all(|pair| pair[0].id() < pair[1].id())
            && self.source_map_refs.windows(2).all(|pair| {
                (pair[0].source, pair[0].start_byte, pair[0].end_byte)
                    <= (pair[1].source, pair[1].start_byte, pair[1].end_byte)
            })
            && self.adapter_requirements.windows(2).all(|pair| {
                (
                    pair[0].section_kind,
                    pair[0].section_id,
                    pair[0].content_digest,
                    pair[0].public_id,
                ) <= (
                    pair[1].section_kind,
                    pair[1].section_id,
                    pair[1].content_digest,
                    pair[1].public_id,
                )
            })
    }

    fn validate(&self, budget: &ViewResourceBudget) -> Result<(), SectionCodecError> {
        self.validate_budgets(budget)?;
        self.validate_identity_contracts()?;
        self.validate_source_maps()?;
        self.program.sheets().iter().try_for_each(|sheet| {
            sheet.tokens().iter().try_for_each(|token| {
                check_budget(
                    style_token_depth(sheet, token),
                    budget.style_token_depth,
                    "view_style_token_depth",
                )
            })
        })
    }

    fn validate_budgets(&self, budget: &ViewResourceBudget) -> Result<(), SectionCodecError> {
        check_budget(
            self.program.sheets().len(),
            budget.style_sheets,
            "view_style_sheets",
        )?;
        check_budget(
            self.program.patches().len(),
            budget.style_patches,
            "view_style_inline_patches",
        )?;
        let token_count = self
            .program
            .sheets()
            .iter()
            .map(|sheet| sheet.tokens().len())
            .sum();
        let rule_count = self
            .program
            .sheets()
            .iter()
            .map(|sheet| sheet.rules().len())
            .sum();
        let declaration_count = self
            .program
            .sheets()
            .iter()
            .flat_map(ViewStyleSheet::rules)
            .map(|rule| rule.declarations().len())
            .chain(
                self.program
                    .patches()
                    .iter()
                    .map(|patch| patch.declarations().len()),
            )
            .sum();
        check_budget(token_count, budget.style_tokens, "view_style_tokens")?;
        check_budget(rule_count, budget.style_rules, "view_style_rules")?;
        check_budget(
            declaration_count,
            budget.style_declarations,
            "view_style_declarations",
        )?;
        check_budget(
            self.source_map_refs.len(),
            budget.source_map_refs,
            "view_style_source_map_refs",
        )?;
        self.program
            .sheets()
            .iter()
            .flat_map(ViewStyleSheet::rules)
            .map(arcweft_view::style::ViewStyleRule::selector)
            .try_for_each(|selector| {
                check_budget(
                    selector.max_depth(),
                    budget.selector_depth,
                    "view_selector_depth",
                )
            })?;
        let environment_conditions = self
            .program
            .sheets()
            .iter()
            .flat_map(ViewStyleSheet::rules)
            .filter(|rule| rule.environment().is_some())
            .count();
        check_budget(
            environment_conditions,
            budget.environment_conditions,
            "view_style_environment_conditions",
        )?;
        let environment_clauses = self
            .program
            .sheets()
            .iter()
            .flat_map(ViewStyleSheet::rules)
            .filter_map(arcweft_view::style::ViewStyleRule::environment)
            .map(|condition| condition.clauses().len())
            .sum();
        check_budget(
            environment_clauses,
            budget.environment_clauses,
            "view_style_environment_clauses",
        )?;
        let part_count = self
            .program
            .sheets()
            .iter()
            .flat_map(ViewStyleSheet::rules)
            .flat_map(|rule| rule.selector().sequences())
            .filter_map(|sequence| sequence.part())
            .collect::<BTreeSet<_>>()
            .len();
        check_budget(part_count, budget.part_count, "view_style_part_count")
    }

    fn validate_identity_contracts(&self) -> Result<(), SectionCodecError> {
        if !valid_resource_identity(&self.style_program_id) {
            return Err(SectionCodecError::NonCanonicalTable(
                "view_style_program_identity",
            ));
        }
        reject_duplicates(
            std::iter::once(self.style_program_id.clone()).chain(
                self.program
                    .sheets()
                    .iter()
                    .map(|sheet| sheet.id().public_id().as_str().to_owned()),
            ),
            "view_style_product_identities",
        )
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(
            std::iter::once(self.style_program_id.clone())
                .chain(self.program.sheets().iter().flat_map(|sheet| {
                    std::iter::once(sheet.id().public_id().as_str().to_owned())
                        .chain(sheet.tokens().iter().flat_map(|token| {
                            std::iter::once(token.id().public_id().as_str().to_owned())
                                .chain(style_value_public_ids(token.value()))
                        }))
                        .chain(sheet.rules().iter().flat_map(style_rule_public_ids))
                }))
                .chain(self.program.patches().iter().flat_map(|patch| {
                    patch
                        .declarations()
                        .iter()
                        .flat_map(|declaration| style_value_public_ids(declaration.value()))
                })),
        )
    }

    fn record_count(&self) -> u32 {
        let records = self
            .program
            .sheets()
            .len()
            .saturating_add(self.program.patches().len())
            .saturating_add(
                self.program
                    .sheets()
                    .iter()
                    .map(|sheet| sheet.rules().len())
                    .sum(),
            );
        saturating_u32(records)
    }

    fn validate_source_maps(&self) -> Result<(), SectionCodecError> {
        style_environment::validate_structure(self)?;
        let public_ids = self.public_id_table()?;
        let valid_owners = std::iter::once(self.style_program_id.as_str())
            .chain(
                self.program
                    .sheets()
                    .iter()
                    .map(|sheet| sheet.id().public_id().as_str()),
            )
            .collect::<BTreeSet<_>>();

        for range in &self.source_map_refs {
            if range.start_byte > range.end_byte {
                return Err(SectionCodecError::NonCanonicalTable(
                    "view_style_source_range_order",
                ));
            }
            if !valid_owners.contains(public_ids.get(range.source)?) {
                return Err(SectionCodecError::NonCanonicalTable(
                    "view_style_source_range_owners",
                ));
            }
        }

        for sheet in self.program.sheets() {
            let owner = sheet.id().public_id().as_str();
            let sources = sheet.tokens().iter().map(ViewStyleToken::source).chain(
                sheet.rules().iter().flat_map(|rule| {
                    std::iter::once(rule.source()).chain(
                        rule.declarations()
                            .iter()
                            .map(arcweft_view::style::ViewStyleDeclaration::source),
                    )
                }),
            );
            for source in sources {
                if self.source_owner(&public_ids, source)? != owner {
                    return Err(SectionCodecError::NonCanonicalTable(
                        "view_style_sheet_source_map_owner",
                    ));
                }
            }
        }

        for patch in self.program.patches() {
            for source in patch
                .declarations()
                .iter()
                .map(arcweft_view::style::ViewStyleDeclaration::source)
            {
                if self.source_owner(&public_ids, source)? != self.style_program_id {
                    return Err(SectionCodecError::NonCanonicalTable(
                        "view_style_patch_source_map_owner",
                    ));
                }
            }
        }
        Ok(())
    }

    fn source_owner<'a>(
        &self,
        public_ids: &'a PublicIdTable,
        source: ViewStyleSourceId,
    ) -> Result<&'a str, SectionCodecError> {
        let range = self.source_map_refs.get(source.value() as usize).ok_or(
            SectionCodecError::NonCanonicalTable("view_style_source_ids"),
        )?;
        public_ids.get(range.source)
    }
}

fn style_rule_public_ids(rule: &arcweft_view::style::ViewStyleRule) -> Vec<String> {
    style_selector_public_ids(rule.selector())
        .chain(
            rule.declarations()
                .iter()
                .flat_map(|declaration| style_value_public_ids(declaration.value())),
        )
        .collect()
}

fn style_selector_public_ids(selector: &ViewStyleSelector) -> impl Iterator<Item = String> + '_ {
    selector.sequences().iter().filter_map(|sequence| {
        sequence
            .part()
            .map(|part| part.as_public_id().as_str().to_owned())
    })
}

fn style_value_public_ids(value: &ViewSpecifiedValue) -> Vec<String> {
    match value {
        ViewSpecifiedValue::Token { token, .. } => {
            vec![token.public_id().as_str().to_owned()]
        }
        ViewSpecifiedValue::Resource { value } => vec![value.as_str().to_owned()],
        ViewSpecifiedValue::BoxAxes { .. }
        | ViewSpecifiedValue::Bool { .. }
        | ViewSpecifiedValue::Integer { .. }
        | ViewSpecifiedValue::Ratio { .. }
        | ViewSpecifiedValue::Scalar { .. }
        | ViewSpecifiedValue::Length { .. }
        | ViewSpecifiedValue::Angle { .. }
        | ViewSpecifiedValue::Color { .. }
        | ViewSpecifiedValue::FontFamilyList { .. }
        | ViewSpecifiedValue::FontWeight { .. }
        | ViewSpecifiedValue::FontStyle { .. }
        | ViewSpecifiedValue::Display { .. }
        | ViewSpecifiedValue::Position { .. }
        | ViewSpecifiedValue::Overflow { .. }
        | ViewSpecifiedValue::FlexDirection { .. }
        | ViewSpecifiedValue::FlexWrap { .. }
        | ViewSpecifiedValue::Alignment { .. }
        | ViewSpecifiedValue::BorderRadii { .. }
        | ViewSpecifiedValue::ShadowList { .. }
        | ViewSpecifiedValue::FilterList { .. }
        | ViewSpecifiedValue::Clip { .. }
        | ViewSpecifiedValue::Mask { .. }
        | ViewSpecifiedValue::BlendMode { .. }
        | ViewSpecifiedValue::Transition { .. } => Vec::new(),
    }
}

fn style_token_depth(sheet: &ViewStyleSheet, token: &ViewStyleToken) -> usize {
    let mut depth = 1_usize;
    let mut current = token;
    while let Some((referenced, _)) = current.value().token_reference() {
        let Some(next) = sheet.token(referenced) else {
            break;
        };
        depth = depth.saturating_add(1);
        current = next;
    }
    depth
}
