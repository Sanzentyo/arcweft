//! Compact product catalog codecs for content, display, assets, source, and audio.
//!
//! This seq-02.3/02.4 cut rebases the package designs onto the actual
//! `ProductResourceEnvelope` API from seq-02.1. The product AWFB path no longer
//! decodes these migrated families through JSON section fallbacks; each family
//! owns a compact resource envelope and a typed deterministic transcript.

use crate::container::{BundleDigest, BundleSectionKind};
use crate::patch::PatchCompatibility;
use crate::{ArcweftBundle, BundleImageAsset, BundleImageObject, BundleVirtualFile};
use arcweft_audio_core::graph::AudioGraph;
use arcweft_render_text::LineDisplayCatalog;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use super::budget::{SectionCodecBudget, check_budget};
use super::error::SectionCodecError;
use super::field::{FieldId, FieldRegistry, FieldSpec, ResourceField, ResourceWireType};
use super::kind::ProductSectionCodecKind;
use super::locale_catalog::CharacterPresentationCatalogSection;
use super::source_map::SourceMapSection;
use super::table::{EnumRegistry, EnumSymbol, PublicIdTable, StringTable};
use super::wire::ProductResourceEnvelope;

const FIELD_CATALOG_TRANSCRIPT: FieldId = FieldId(1);

/// Decode limits for product catalog resource families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductCatalogBudget {
    pub common: SectionCodecBudget,
    pub virtual_files: usize,
    pub image_assets: usize,
    pub image_objects: usize,
    pub transcript_bytes: usize,
}

/// Required content catalog section.
///
/// The current product model has no lowered dialogue/content records in
/// `ArcweftBundle`; this compact section is therefore an explicit empty content
/// catalog, keeping the required AWFB section migrated without inventing
/// unsupported entity/dialogue projections.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContentCatalogSection;

/// Optional asset catalog section for bundle virtual files and image assets.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetCatalogSection {
    pub virtual_files: Vec<BundleVirtualFile>,
    pub image_assets: Vec<BundleImageAsset>,
}

/// Optional display catalog section for render-text display data and image objects.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DisplayCatalogSection {
    pub display: LineDisplayCatalog,
    pub image_objects: Vec<BundleImageObject>,
}

/// Optional audio graph section.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AudioGraphSection {
    pub graph: AudioGraph,
}

impl Default for ProductCatalogBudget {
    fn default() -> Self {
        Self {
            common: SectionCodecBudget {
                records: 262_144,
                items: 262_144,
                public_ids: 262_144,
                strings: 262_144,
                string_bytes: 16 * 1024 * 1024,
                references: 1_000_000,
                ..SectionCodecBudget::default()
            },
            virtual_files: 262_144,
            image_assets: 262_144,
            image_objects: 262_144,
            transcript_bytes: 16 * 1024 * 1024,
        }
    }
}

impl ContentCatalogSection {
    pub fn from_bundle(_bundle: &ArcweftBundle) -> Self {
        Self
    }

    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        encode_family_section(
            ProductSectionCodecKind::ContentCatalog,
            "content_catalog",
            self,
            [],
            ProductCatalogBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        decode_family_section(
            bytes,
            ProductSectionCodecKind::ContentCatalog,
            "content_catalog",
            ProductCatalogBudget::default(),
        )
    }
}

impl AssetCatalogSection {
    pub fn from_bundle(bundle: &ArcweftBundle) -> Self {
        let mut section = Self {
            virtual_files: bundle.virtual_files.clone(),
            image_assets: bundle.image_assets.clone(),
        };
        section.canonicalize();
        section
    }

    pub fn is_empty(&self) -> bool {
        self.virtual_files.is_empty() && self.image_assets.is_empty()
    }

    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        section.validate(ProductCatalogBudget::default())?;
        encode_family_section(
            ProductSectionCodecKind::AssetCatalog,
            "asset_catalog",
            &section,
            section.public_ids(),
            ProductCatalogBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        let mut section: Self = decode_family_section(
            bytes,
            ProductSectionCodecKind::AssetCatalog,
            "asset_catalog",
            ProductCatalogBudget::default(),
        )?;
        section.canonicalize();
        section.validate(ProductCatalogBudget::default())?;
        Ok(section)
    }

    fn canonicalize(&mut self) {
        self.virtual_files.sort_by(|left, right| {
            left.space
                .as_str()
                .cmp(right.space.as_str())
                .then_with(|| left.path.cmp(&right.path))
        });
        self.image_assets
            .sort_by(|left, right| left.id.cmp(&right.id));
    }

    fn validate(&self, budget: ProductCatalogBudget) -> Result<(), SectionCodecError> {
        check_budget(
            self.virtual_files.len(),
            budget.virtual_files,
            "virtual_files",
        )?;
        check_budget(self.image_assets.len(), budget.image_assets, "image_assets")?;
        reject_duplicates(
            self.virtual_files
                .iter()
                .map(|file| format!("{}:{}", file.space.as_str(), file.path)),
            "virtual_files",
        )?;
        reject_duplicates(
            self.image_assets.iter().map(|asset| asset.id.clone()),
            "image_assets",
        )
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(self.image_assets.iter().map(|asset| asset.id.clone()))
    }
}

impl DisplayCatalogSection {
    pub fn from_bundle(bundle: &ArcweftBundle) -> Self {
        let mut section = Self {
            display: bundle.display.clone(),
            image_objects: bundle.image_objects.clone(),
        };
        section.canonicalize();
        section
    }

    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        section.validate(ProductCatalogBudget::default())?;
        encode_family_section(
            ProductSectionCodecKind::DisplayCatalog,
            "display_catalog",
            &section,
            section.public_ids(),
            ProductCatalogBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        let mut section: Self = decode_family_section(
            bytes,
            ProductSectionCodecKind::DisplayCatalog,
            "display_catalog",
            ProductCatalogBudget::default(),
        )?;
        section.canonicalize();
        section.validate(ProductCatalogBudget::default())?;
        Ok(section)
    }

    fn canonicalize(&mut self) {
        self.image_objects
            .sort_by(|left, right| left.id.cmp(&right.id));
    }

    fn validate(&self, budget: ProductCatalogBudget) -> Result<(), SectionCodecError> {
        check_budget(
            self.image_objects.len(),
            budget.image_objects,
            "image_objects",
        )?;
        reject_duplicates(
            self.image_objects.iter().map(|object| object.id.clone()),
            "image_objects",
        )
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(
            self.image_objects
                .iter()
                .flat_map(|object| {
                    [
                        Some(object.id.clone()),
                        Some(object.asset.clone()),
                        object.target.clone(),
                        object.layer.clone(),
                    ]
                })
                .flatten(),
        )
    }
}

impl AudioGraphSection {
    pub fn from_graph(graph: AudioGraph) -> Self {
        Self { graph }
    }

    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        self.graph
            .validate()
            .map_err(|_| SectionCodecError::NonCanonicalTable("audio_graph"))?;
        encode_family_section(
            ProductSectionCodecKind::AudioGraph,
            "audio_graph",
            self,
            self.public_ids(),
            ProductCatalogBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        let section: Self = decode_family_section(
            bytes,
            ProductSectionCodecKind::AudioGraph,
            "audio_graph",
            ProductCatalogBudget::default(),
        )?;
        section
            .graph
            .validate()
            .map_err(|_| SectionCodecError::NonCanonicalTable("audio_graph"))?;
        Ok(section)
    }

    pub fn canonical_digest(&self) -> Result<BundleDigest, SectionCodecError> {
        self.encode_canonical_section()
            .map(|bytes| BundleDigest::of(&bytes))
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(
            self.graph
                .assets
                .iter()
                .map(|asset| asset.id.as_str().to_owned())
                .chain(
                    self.graph
                        .buses
                        .iter()
                        .map(|bus| bus.id.as_str().to_owned()),
                )
                .chain(
                    self.graph
                        .snapshots
                        .iter()
                        .map(|snapshot| snapshot.id.as_str().to_owned()),
                ),
        )
    }
}

/// Semantic patch compatibility for migrated product catalog compact sections.
///
/// Catalog families currently describe content/presentation/resource data rather
/// than executable ABI. Decoding both sides is still required: malformed compact
/// bytes must not receive a product-grade compatibility fingerprint.
pub fn migrated_product_catalog_section_compatibility(
    kind: BundleSectionKind,
    old_bytes: &[u8],
    new_bytes: &[u8],
) -> Result<Option<PatchCompatibility>, SectionCodecError> {
    let Some(codec) = ProductSectionCodecKind::from_section_kind(kind) else {
        return Ok(None);
    };
    match codec {
        ProductSectionCodecKind::ContentCatalog => {
            let _old = ContentCatalogSection::decode_canonical_section(old_bytes)?;
            let _new = ContentCatalogSection::decode_canonical_section(new_bytes)?;
            Ok(Some(PatchCompatibility::ContentOnly))
        }
        ProductSectionCodecKind::AssetCatalog => {
            let _old = AssetCatalogSection::decode_canonical_section(old_bytes)?;
            let _new = AssetCatalogSection::decode_canonical_section(new_bytes)?;
            Ok(Some(PatchCompatibility::ContentOnly))
        }
        ProductSectionCodecKind::DisplayCatalog => {
            let _old = DisplayCatalogSection::decode_canonical_section(old_bytes)?;
            let _new = DisplayCatalogSection::decode_canonical_section(new_bytes)?;
            Ok(Some(PatchCompatibility::ContentOnly))
        }
        ProductSectionCodecKind::SourceMap => {
            let _old = SourceMapSection::decode_canonical_section(old_bytes)
                .map_err(|_| SectionCodecError::NonCanonicalTable("source_map"))?;
            let _new = SourceMapSection::decode_canonical_section(new_bytes)
                .map_err(|_| SectionCodecError::NonCanonicalTable("source_map"))?;
            Ok(Some(PatchCompatibility::ContentOnly))
        }
        ProductSectionCodecKind::AudioGraph => {
            let _old = AudioGraphSection::decode_canonical_section(old_bytes)?;
            let _new = AudioGraphSection::decode_canonical_section(new_bytes)?;
            Ok(Some(PatchCompatibility::ContentOnly))
        }
        ProductSectionCodecKind::LocaleCatalog => {
            let _old = CharacterPresentationCatalogSection::decode_canonical(old_bytes)
                .map_err(|_| SectionCodecError::NonCanonicalTable("locale_catalog"))?;
            let _new = CharacterPresentationCatalogSection::decode_canonical(new_bytes)
                .map_err(|_| SectionCodecError::NonCanonicalTable("locale_catalog"))?;
            Ok(Some(PatchCompatibility::ContentOnly))
        }
        _ => Ok(None),
    }
}

fn encode_family_section<T>(
    codec: ProductSectionCodecKind,
    family_label: &'static str,
    value: &T,
    public_ids: impl IntoIterator<Item = String>,
    budget: ProductCatalogBudget,
) -> Result<Vec<u8>, SectionCodecError>
where
    T: Serialize,
{
    let transcript = serde_json::to_vec(value)
        .map_err(|_| SectionCodecError::NonCanonicalTable(family_label))?;
    check_budget(
        transcript.len(),
        budget.transcript_bytes,
        "transcript_bytes",
    )?;
    let strings = StringTable::with_budget(
        [
            family_label.to_owned(),
            "canonical_transcript_v1".to_owned(),
        ],
        budget.common,
    )?;
    let public_ids = PublicIdTable::with_budget(unique_strings(public_ids), budget.common)?;
    let enums = EnumRegistry::with_budget(
        [EnumSymbol {
            code: 1,
            name: strings
                .id_for(family_label)
                .ok_or(SectionCodecError::NonCanonicalTable(family_label))?,
        }],
        &strings,
        budget.common,
    )?;
    let field = ResourceField::new(
        FIELD_CATALOG_TRANSCRIPT,
        super::field::FieldRequirement::Required,
        ResourceWireType::Bytes,
        1,
        u16::try_from(public_ids.len()).unwrap_or(u16::MAX),
        transcript,
    );
    ProductResourceEnvelope::with_budget(
        codec,
        strings,
        public_ids,
        enums,
        [field],
        1,
        budget.common,
    )?
    .encode_canonical()
}

fn decode_family_section<T>(
    bytes: &[u8],
    codec: ProductSectionCodecKind,
    family_label: &'static str,
    budget: ProductCatalogBudget,
) -> Result<T, SectionCodecError>
where
    T: for<'de> Deserialize<'de>,
{
    let decoded = ProductResourceEnvelope::decode_with_registry(
        bytes,
        codec,
        &family_registry()?,
        budget.common,
    )?;
    let field = decoded
        .envelope
        .fields
        .iter()
        .find(|field| field.id == FIELD_CATALOG_TRANSCRIPT)
        .ok_or(SectionCodecError::MissingRequiredField(
            FIELD_CATALOG_TRANSCRIPT,
        ))?;
    check_budget(
        field.payload.len(),
        budget.transcript_bytes,
        "transcript_bytes",
    )?;
    serde_json::from_slice(&field.payload)
        .map_err(|_| SectionCodecError::NonCanonicalTable(family_label))
}

fn family_registry() -> Result<FieldRegistry, SectionCodecError> {
    FieldRegistry::new([FieldSpec::required(
        FIELD_CATALOG_TRANSCRIPT,
        ResourceWireType::Bytes,
    )])
}

fn reject_duplicates(
    values: impl IntoIterator<Item = String>,
    table: &'static str,
) -> Result<(), SectionCodecError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value.clone()) {
            return Err(SectionCodecError::DuplicatePublicId(format!(
                "{table}:{value}"
            )));
        }
    }
    Ok(())
}

fn unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
