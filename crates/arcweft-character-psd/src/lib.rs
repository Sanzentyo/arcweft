//! Pure PSD-to-Arcweft character package conversion.
//!
//! The crate accepts bytes and returns typed manifest data plus package-relative
//! file payloads. It performs no filesystem I/O, so CLI, editor, and build-system
//! adapters can share exactly the same conversion and diagnostics.

use arcweft_character::{
    id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId},
    manifest::{
        CharacterAssetPath, CharacterBlendMode, CharacterCanvas, CharacterLook, CharacterManifest,
        CharacterManifestError, CharacterPart, CharacterPartSelection, CharacterPoint,
        CharacterRect, CharacterSource, CharacterSourceLayer, CharacterVariant,
    },
};
use psd::{ColorMode, Psd, PsdDepth, PsdError, PsdLayer};
use thiserror::Error;

const PART_PREFIX: &str = "part:";
const LOOK_PREFIX: &str = "look:";
const IMPORTER_NAME: &str = concat!("arcweft-character-psd/", env!("CARGO_PKG_VERSION"));

/// Import policy supplied by a host adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PsdCharacterImportOptions {
    character: CharacterId,
    source_file_name: String,
    default_look: Option<CharacterLookId>,
    anchor: Option<CharacterPoint>,
    strict: bool,
}

/// One generated file inside an `.awchar` package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedCharacterFile {
    path: CharacterAssetPath,
    bytes: Vec<u8>,
}

/// Complete deterministic result of one PSD conversion.
#[derive(Clone, Debug)]
pub struct ImportedCharacter {
    manifest: CharacterManifest,
    files: Vec<ImportedCharacterFile>,
    warnings: Vec<String>,
}

/// PSD conversion failure.
#[derive(Debug, Error)]
pub enum PsdCharacterImportError {
    #[error("failed to parse PSD: {0}")]
    Psd(#[from] PsdError),
    #[error("PSD must use 8 bits per channel; found {0:?}")]
    UnsupportedDepth(PsdDepth),
    #[error("PSD must use RGB color mode; found {0:?}")]
    UnsupportedColorMode(ColorMode),
    #[error("PSD does not contain a top-level `part:<id>` group")]
    MissingPartGroups,
    #[error("part group `{0}` has no direct pixel layers")]
    EmptyPartGroup(String),
    #[error("look group `{0}` has no `part=variant` marker layers")]
    EmptyLookGroup(String),
    #[error("look marker `{layer}` in `{group}` must be named `part=variant`")]
    InvalidLookMarker { group: String, layer: String },
    #[error("layer `{layer}` in `{group}` has no pixels inside the PSD canvas")]
    LayerOutsideCanvas { group: String, layer: String },
    #[error("layer `{layer}` in `{group}` uses an unrecognized blend mode `{mode}`")]
    UnknownBlendMode {
        group: String,
        layer: String,
        mode: String,
    },
    #[error("cannot infer a look for part `{part}`; visible variants: {visible}")]
    AmbiguousInferredLook { part: String, visible: usize },
    #[error("requested default look `{0}` is not declared")]
    MissingDefaultLook(String),
    #[error("strict PSD import rejected warnings: {0}")]
    StrictWarnings(String),
    #[error("failed to encode `{path}` as PNG: {source}")]
    Png {
        path: String,
        #[source]
        source: png::EncodingError,
    },
    #[error(transparent)]
    Identifier(#[from] arcweft_character::id::CharacterIdError),
    #[error(transparent)]
    AssetPath(#[from] arcweft_character::manifest::CharacterAssetPathError),
    #[error(transparent)]
    Manifest(#[from] CharacterManifestError),
}

#[derive(Clone, Debug)]
struct ParsedPart {
    id: CharacterPartId,
    z: i32,
    variants: Vec<ParsedVariant>,
}

#[derive(Clone, Debug)]
struct ParsedVariant {
    variant: CharacterVariant,
    visible: bool,
    file: ImportedCharacterFile,
}

impl PsdCharacterImportOptions {
    /// Creates an import request with center-bottom anchoring and non-strict warnings.
    pub fn new(character: CharacterId, source_file_name: impl Into<String>) -> Self {
        Self {
            character,
            source_file_name: source_file_name.into(),
            default_look: None,
            anchor: None,
            strict: false,
        }
    }

    #[must_use]
    pub fn with_default_look(mut self, default_look: CharacterLookId) -> Self {
        self.default_look = Some(default_look);
        self
    }

    #[must_use]
    pub const fn with_anchor(mut self, anchor: CharacterPoint) -> Self {
        self.anchor = Some(anchor);
        self
    }

    #[must_use]
    pub const fn strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    pub const fn character(&self) -> &CharacterId {
        &self.character
    }
}

impl ImportedCharacterFile {
    pub fn new(path: CharacterAssetPath, bytes: Vec<u8>) -> Self {
        Self { path, bytes }
    }

    pub const fn path(&self) -> &CharacterAssetPath {
        &self.path
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl ImportedCharacter {
    pub const fn manifest(&self) -> &CharacterManifest {
        &self.manifest
    }

    pub fn files(&self) -> &[ImportedCharacterFile] {
        &self.files
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub fn into_parts(self) -> (CharacterManifest, Vec<ImportedCharacterFile>, Vec<String>) {
        (self.manifest, self.files, self.warnings)
    }
}

/// Converts PSD bytes into a typed Arcweft character package.
///
/// Contract:
/// - regular PSD version 1, not PSB;
/// - 8-bit RGB pixel layers;
/// - top-level `part:<id>` groups contain direct variant layers;
/// - top-level `look:<id>` groups contain direct `part=variant` marker layers.
///
/// The importer never calls `Psd::flatten_layers_rgba`. Each pixel layer is
/// exported separately, preserving part replacement semantics and blend metadata.
pub fn import_psd_character(
    bytes: &[u8],
    options: &PsdCharacterImportOptions,
) -> Result<ImportedCharacter, PsdCharacterImportError> {
    let psd = Psd::from_bytes(bytes)?;
    if psd.depth() != PsdDepth::Eight {
        return Err(PsdCharacterImportError::UnsupportedDepth(psd.depth()));
    }
    if psd.color_mode() != ColorMode::Rgb {
        return Err(PsdCharacterImportError::UnsupportedColorMode(
            psd.color_mode(),
        ));
    }

    let mut warnings = collect_group_shape_warnings(&psd);
    let parsed_parts = parse_parts(&psd, &mut warnings)?;
    if parsed_parts.is_empty() {
        return Err(PsdCharacterImportError::MissingPartGroups);
    }

    let mut looks = parse_looks(&psd)?;
    if looks.is_empty() {
        looks.push(infer_look(&parsed_parts, &mut warnings)?);
        warnings.push(
            "no `look:<id>` group was present; inferred one look from visible part variants"
                .to_owned(),
        );
    }

    let default_look = choose_default_look(&looks, options.default_look.as_ref())?;
    let anchor = options.anchor.unwrap_or_else(|| {
        CharacterPoint::new(
            i32::try_from(psd.width() / 2).unwrap_or(i32::MAX),
            i32::try_from(psd.height()).unwrap_or(i32::MAX),
        )
    });

    for part in &parsed_parts {
        for parsed in &part.variants {
            if !parsed.variant.blend().is_baseline_renderer_supported() {
                warnings.push(format!(
                    "{}.{} uses {:?}; metadata is preserved but the baseline renderer cannot reproduce it",
                    part.id.as_str(),
                    parsed.variant.id().as_str(),
                    parsed.variant.blend()
                ));
            }
            if parsed.variant.clipping() {
                warnings.push(format!(
                    "{}.{} is a clipping layer; metadata is preserved but clipping composition is not implemented",
                    part.id.as_str(),
                    parsed.variant.id().as_str()
                ));
            }
        }
    }
    warnings.sort();
    warnings.dedup();
    if options.strict && !warnings.is_empty() {
        return Err(PsdCharacterImportError::StrictWarnings(warnings.join("; ")));
    }

    let source_name = portable_file_name(&options.source_file_name);
    let source = CharacterSource::psd(
        source_name,
        blake3::hash(bytes).to_hex().to_string(),
        IMPORTER_NAME,
        warnings.clone(),
    );
    let parts = parsed_parts
        .iter()
        .map(|part| {
            CharacterPart::new(
                part.id.clone(),
                part.z,
                part.variants
                    .iter()
                    .map(|variant| variant.variant.clone())
                    .collect(),
            )
        })
        .collect();
    let mut files = parsed_parts
        .into_iter()
        .flat_map(|part| part.variants.into_iter().map(|variant| variant.file))
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.path().cmp(right.path()));

    let manifest = CharacterManifest::new(
        options.character.clone(),
        CharacterCanvas::new(psd.width(), psd.height()),
        anchor,
        default_look,
        parts,
        looks,
        Some(source),
    )?;
    Ok(ImportedCharacter {
        manifest,
        files,
        warnings,
    })
}

fn collect_group_shape_warnings(psd: &Psd) -> Vec<String> {
    let mut warnings = psd
        .group_ids_in_order()
        .iter()
        .filter_map(|id| psd.groups().get(id))
        .filter_map(|group| {
            let recognized = group.name().starts_with(PART_PREFIX)
                || group.name().starts_with(LOOK_PREFIX);
            match (group.parent_id(), recognized, group.name().starts_with('_')) {
                (None, false, false) => Some(format!(
                    "ignored top-level group `{}`; expected `part:` or `look:` prefix",
                    group.name()
                )),
                (Some(_), true, _) => Some(format!(
                    "ignored nested contract group `{}`; `part:` and `look:` groups must be top-level",
                    group.name()
                )),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    warnings.extend(
        psd.layers()
            .iter()
            .filter(|layer| layer.parent_id().is_none() && !layer.name().starts_with('_'))
            .map(|layer| {
                format!(
                    "ignored top-level pixel layer `{}`; variants must be inside a `part:` group",
                    layer.name()
                )
            }),
    );
    warnings
}

fn parse_parts(
    psd: &Psd,
    warnings: &mut Vec<String>,
) -> Result<Vec<ParsedPart>, PsdCharacterImportError> {
    let mut parts = Vec::new();
    for group_id in psd.group_ids_in_order() {
        let Some(group) = psd.groups().get(group_id) else {
            continue;
        };
        if group.parent_id().is_some() {
            continue;
        }
        let Some(raw_part) = group.name().strip_prefix(PART_PREFIX) else {
            continue;
        };
        let part_id = CharacterPartId::try_new(raw_part.trim())?;
        let mut variants = Vec::new();
        for (layer_index, layer) in direct_layers(psd, group.id()) {
            let variant_id = CharacterVariantId::try_new(layer.name().trim())?;
            variants.push(parse_variant(
                psd,
                group.name(),
                layer_index,
                layer,
                &part_id,
                &variant_id,
                warnings,
            )?);
        }
        if variants.is_empty() {
            return Err(PsdCharacterImportError::EmptyPartGroup(
                group.name().to_owned(),
            ));
        }
        let z = i32::try_from(parts.len()).unwrap_or(i32::MAX);
        parts.push(ParsedPart {
            id: part_id,
            z,
            variants,
        });
    }
    if parts.is_empty() {
        parse_loose_group_parts(psd, warnings)
    } else {
        Ok(parts)
    }
}

fn parse_loose_group_parts(
    psd: &Psd,
    warnings: &mut Vec<String>,
) -> Result<Vec<ParsedPart>, PsdCharacterImportError> {
    warnings.push(
        "no `part:<id>` groups were found; used loose PSD group names as character parts"
            .to_owned(),
    );

    let mut parts = Vec::new();
    for group_id in psd.group_ids_in_order() {
        let Some(group) = psd.groups().get(group_id) else {
            continue;
        };
        if !(group.name().starts_with('!') || group.name().starts_with('*')) {
            continue;
        }

        let part_id = CharacterPartId::try_new(stable_layer_id("part", group.id(), group.name()))?;
        let mut variants = Vec::new();
        for (layer_index, layer) in direct_layers(psd, group.id()) {
            let variant_id = CharacterVariantId::try_new(stable_layer_id(
                "variant",
                u32::try_from(layer_index).unwrap_or(u32::MAX),
                layer.name(),
            ))?;
            variants.push(parse_variant(
                psd,
                group.name(),
                layer_index,
                layer,
                &part_id,
                &variant_id,
                warnings,
            )?);
        }
        if variants.is_empty() {
            continue;
        }
        let z = i32::try_from(parts.len()).unwrap_or(i32::MAX);
        parts.push(ParsedPart {
            id: part_id,
            z,
            variants,
        });
    }

    let top_level_layers = psd
        .layers()
        .iter()
        .enumerate()
        .filter(|(_, layer)| layer.parent_id().is_none())
        .collect::<Vec<_>>();
    if !top_level_layers.is_empty() {
        let part_id = CharacterPartId::try_new("part_top_level")?;
        let mut variants = Vec::new();
        for (layer_index, layer) in top_level_layers {
            let variant_id = CharacterVariantId::try_new(stable_layer_id(
                "variant",
                u32::try_from(layer_index).unwrap_or(u32::MAX),
                layer.name(),
            ))?;
            variants.push(parse_variant(
                psd,
                "<top-level>",
                layer_index,
                layer,
                &part_id,
                &variant_id,
                warnings,
            )?);
        }
        parts.push(ParsedPart {
            id: part_id,
            z: i32::try_from(parts.len()).unwrap_or(i32::MAX),
            variants,
        });
    }

    Ok(parts)
}

fn parse_variant(
    psd: &Psd,
    group_name: &str,
    layer_index: usize,
    layer: &PsdLayer,
    part_id: &CharacterPartId,
    variant_id: &CharacterVariantId,
    warnings: &mut Vec<String>,
) -> Result<ParsedVariant, PsdCharacterImportError> {
    let rect = intersect_layer_rect(psd.width(), psd.height(), layer).ok_or_else(|| {
        PsdCharacterImportError::LayerOutsideCanvas {
            group: group_name.to_owned(),
            layer: layer.name().to_owned(),
        }
    })?;
    let rgba = crop_full_canvas_rgba(psd.width(), &layer.rgba(), rect);
    let asset_path = CharacterAssetPath::try_new(format!(
        "layers/{}--{}.png",
        part_id.as_str(),
        variant_id.as_str()
    ))?;
    let png = encode_png(asset_path.as_str(), rect.width(), rect.height(), &rgba)?;
    let blend_name = format!("{:?}", layer.blend_mode());
    let blend = CharacterBlendMode::from_photoshop_debug_name(&blend_name).ok_or_else(|| {
        PsdCharacterImportError::UnknownBlendMode {
            group: group_name.to_owned(),
            layer: layer.name().to_owned(),
            mode: blend_name,
        }
    })?;
    if rect.x() != layer.layer_left()
        || rect.y() != layer.layer_top()
        || rect.width() != inclusive_span(layer.layer_left(), layer.layer_right())
        || rect.height() != inclusive_span(layer.layer_top(), layer.layer_bottom())
    {
        warnings.push(format!(
            "{}.{} extended outside the PSD canvas and was clipped to the canvas",
            part_id.as_str(),
            variant_id.as_str()
        ));
    }
    let variant = CharacterVariant::new(
        variant_id.clone(),
        asset_path.clone(),
        rect,
        layer.opacity(),
        blend,
        layer.is_clipping_mask(),
    )
    .with_source_layer(CharacterSourceLayer::new(
        layer_index,
        group_name,
        layer.name(),
    ));
    Ok(ParsedVariant {
        variant,
        visible: layer.visible(),
        file: ImportedCharacterFile::new(asset_path, png),
    })
}

fn parse_looks(psd: &Psd) -> Result<Vec<CharacterLook>, PsdCharacterImportError> {
    let mut looks = Vec::new();
    for group_id in psd.group_ids_in_order() {
        let Some(group) = psd.groups().get(group_id) else {
            continue;
        };
        if group.parent_id().is_some() {
            continue;
        }
        let Some(raw_look) = group.name().strip_prefix(LOOK_PREFIX) else {
            continue;
        };
        let look_id = CharacterLookId::try_new(raw_look.trim())?;
        let mut selections = Vec::new();
        for (_, layer) in direct_layers(psd, group.id()) {
            let Some((part, variant)) = layer.name().split_once('=') else {
                return Err(PsdCharacterImportError::InvalidLookMarker {
                    group: group.name().to_owned(),
                    layer: layer.name().to_owned(),
                });
            };
            selections.push(CharacterPartSelection::new(
                CharacterPartId::try_new(part.trim())?,
                CharacterVariantId::try_new(variant.trim())?,
            ));
        }
        if selections.is_empty() {
            return Err(PsdCharacterImportError::EmptyLookGroup(
                group.name().to_owned(),
            ));
        }
        looks.push(CharacterLook::new(look_id, selections));
    }
    Ok(looks)
}

fn infer_look(
    parts: &[ParsedPart],
    warnings: &mut Vec<String>,
) -> Result<CharacterLook, PsdCharacterImportError> {
    let mut selections = Vec::with_capacity(parts.len());
    for part in parts {
        let visible = part
            .variants
            .iter()
            .filter(|variant| variant.visible)
            .collect::<Vec<_>>();
        let selected = match visible.as_slice() {
            [only] => *only,
            [] if part.variants.len() == 1 => &part.variants[0],
            [] => {
                warnings.push(format!(
                    "part `{}` has no visible variants; selected the first variant for the inferred look",
                    part.id
                ));
                &part.variants[0]
            }
            [first, ..] => {
                warnings.push(format!(
                    "part `{}` has {} visible variants; selected `{}` for the inferred look",
                    part.id,
                    visible.len(),
                    first.variant.id()
                ));
                *first
            }
        };
        selections.push(CharacterPartSelection::new(
            part.id.clone(),
            selected.variant.id().clone(),
        ));
    }
    Ok(CharacterLook::new(
        CharacterLookId::try_new("default")?,
        selections,
    ))
}

fn choose_default_look(
    looks: &[CharacterLook],
    requested: Option<&CharacterLookId>,
) -> Result<CharacterLookId, PsdCharacterImportError> {
    if let Some(requested) = requested {
        return looks
            .iter()
            .find(|look| look.id() == requested)
            .map(|look| look.id().clone())
            .ok_or_else(|| PsdCharacterImportError::MissingDefaultLook(requested.to_string()));
    }
    looks
        .iter()
        .find(|look| look.id().as_str() == "normal")
        .or_else(|| looks.first())
        .map(|look| look.id().clone())
        .ok_or_else(|| PsdCharacterImportError::MissingDefaultLook("<none>".to_owned()))
}

fn direct_layers(psd: &Psd, group_id: u32) -> impl Iterator<Item = (usize, &PsdLayer)> {
    psd.layers()
        .iter()
        .enumerate()
        .filter(move |(_, layer)| layer.parent_id() == Some(group_id))
}

fn intersect_layer_rect(width: u32, height: u32, layer: &PsdLayer) -> Option<CharacterRect> {
    let canvas_right = i32::try_from(width).ok()?.checked_sub(1)?;
    let canvas_bottom = i32::try_from(height).ok()?.checked_sub(1)?;
    let left = layer.layer_left().max(0);
    let top = layer.layer_top().max(0);
    let right = layer.layer_right().min(canvas_right);
    let bottom = layer.layer_bottom().min(canvas_bottom);
    (left <= right && top <= bottom).then(|| {
        CharacterRect::new(
            left,
            top,
            inclusive_span(left, right),
            inclusive_span(top, bottom),
        )
    })
}

fn inclusive_span(start: i32, end: i32) -> u32 {
    u32::try_from(i64::from(end) - i64::from(start) + 1).unwrap_or(0)
}

fn crop_full_canvas_rgba(canvas_width: u32, rgba: &[u8], rect: CharacterRect) -> Vec<u8> {
    let row_bytes = usize::try_from(rect.width()).unwrap_or(0) * 4;
    let mut cropped =
        Vec::with_capacity(row_bytes.saturating_mul(usize::try_from(rect.height()).unwrap_or(0)));
    for row in 0..rect.height() {
        let y = u32::try_from(rect.y()).unwrap_or(0).saturating_add(row);
        let x = u32::try_from(rect.x()).unwrap_or(0);
        let start_pixels = u64::from(y)
            .saturating_mul(u64::from(canvas_width))
            .saturating_add(u64::from(x));
        let start = usize::try_from(start_pixels.saturating_mul(4)).unwrap_or(usize::MAX);
        let end = start.saturating_add(row_bytes);
        if let Some(source_row) = rgba.get(start..end) {
            cropped.extend_from_slice(source_row);
        }
    }
    cropped
}

fn encode_png(
    path: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<Vec<u8>, PsdCharacterImportError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|source| PsdCharacterImportError::Png {
                path: path.to_owned(),
                source,
            })?;
        writer
            .write_image_data(rgba)
            .map_err(|source| PsdCharacterImportError::Png {
                path: path.to_owned(),
                source,
            })?;
    }
    Ok(bytes)
}

fn portable_file_name(source: &str) -> String {
    source
        .rsplit(['/', '\\'])
        .find(|value| !value.is_empty())
        .unwrap_or("source.psd")
        .to_owned()
}

fn stable_layer_id(prefix: &str, ordinal: u32, label: &str) -> String {
    let mut slug = label
        .trim_start_matches(['!', '*'])
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while slug.contains("__") {
        slug = slug.replace("__", "_");
    }
    let slug = slug.trim_matches('_');
    if slug.is_empty() {
        format!("{prefix}_{ordinal}")
    } else {
        format!("{prefix}_{ordinal}_{slug}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_intersection_uses_inclusive_psd_coordinates() {
        assert_eq!(inclusive_span(10, 10), 1);
        assert_eq!(inclusive_span(-2, 3), 6);
    }

    #[test]
    fn crop_reads_a_rect_from_full_canvas_rgba() {
        let rgba = (0_u8..64).collect::<Vec<_>>();
        let cropped = crop_full_canvas_rgba(4, &rgba, CharacterRect::new(1, 1, 2, 2));
        assert_eq!(cropped, [&rgba[20..28], &rgba[36..44]].concat());
    }

    #[test]
    fn source_provenance_keeps_only_the_file_name() {
        assert_eq!(portable_file_name("/secret/art/akane.psd"), "akane.psd");
        assert_eq!(portable_file_name(r"C:\\secret\\akane.psd"), "akane.psd");
    }

    #[test]
    fn loose_layer_ids_are_ascii_and_stable() {
        assert_eq!(stable_layer_id("part", 13, "!左腕"), "part_13");
        assert_eq!(
            stable_layer_id("variant", 4, "*Base 01"),
            "variant_4_base_01"
        );
    }
}
