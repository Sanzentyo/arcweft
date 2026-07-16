use crate::container::{
    BundleDigest, BundleKind as ContainerBundleKind, BundleSectionKind, BundleView,
    ContentResidency, ExternalSectionPayload, ReadBudget, SectionId, SectionInput, encode_bundle,
};
use crate::fx_definitions::FxDefinitions;
use crate::product_awbc::ProductExecutablePayload;
use crate::resource_codec::runtime::{
    AdapterRequirementsSection as CompactAdapterRequirementsSection,
    EntrypointsSection as CompactEntrypointsSection,
    RuntimeTypesSection as CompactRuntimeTypesSection,
};
use crate::resource_codec::{
    CompactAssetCatalogSection, CompactAudioGraphSection, CompactContentCatalogSection,
    CompactDisplayCatalogSection, SourceMapSection,
};
use crate::resource_codec::{
    ViewInputResource, ViewProgramResource, ViewStyleResource, ViewTextResource, ViewThemeResource,
};
use crate::{
    ARCWEFT_BUNDLE_SCHEMA_VERSION, ArcweftBundle, BundleAwbcProgram, BundleBytecodeEncoding,
    BundleBytecodeProgram, BundleCodecError, BundleKind, BundleManifest,
};
use arcweft_agent_protocol::artifact::AgentArtifactManifest;
use arcweft_core::bytecode::BytecodeProgram;
use serde::{Deserialize, Deserializer, Serialize};

mod source_projection;
mod style_cross_section;
use source_projection::validate_view_sources;
use style_cross_section::{validate_style_bundle_view, validate_style_section_inputs};

const AWFB_SECTION_SCHEMA_VERSION: u32 = 1;
const LEGACY_BYTECODE_SECTION_MAGIC: [u8; 8] = *b"AWBC\r\n\x1a\n";
const LEGACY_PRODUCT_BYTECODE_MESSAGEPACK_TAG: u32 = 1;
const LEGACY_PRODUCT_BYTECODE_COMPACT_SIDECAR_TAG: u32 = 2;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProductManifest {
    schema_version: u32,
    bundle_kind: BundleKind,
    executable_payload: ProductExecutablePayload,
    manifest: BundleManifest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent: Option<AgentArtifactManifest>,
}

#[derive(Debug, Deserialize)]
struct ProductExecutablePayloadProbe {
    #[serde(default)]
    executable_payload: ProductExecutablePayloadField,
}

#[derive(Debug, Default)]
enum ProductExecutablePayloadField {
    #[default]
    Missing,
    Present(serde_json::Value),
}

impl<'de> Deserialize<'de> for ProductExecutablePayloadField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        serde_json::Value::deserialize(deserializer).map(Self::Present)
    }
}

pub(crate) fn to_awfb_bytes(bundle: &ArcweftBundle) -> Result<Vec<u8>, BundleCodecError> {
    let product_manifest = ProductManifest {
        schema_version: bundle.schema_version,
        bundle_kind: bundle.bundle_kind,
        executable_payload: ProductExecutablePayload::AwbcV1,
        manifest: bundle.manifest.clone(),
        agent: bundle.agent.clone(),
    };
    let manifest = encode_json(&product_manifest)?;
    let sections = vec![
        required_section(
            BundleSectionKind::ProgramBytecode,
            bundle
                .product_awbc
                .as_ref()
                .ok_or(BundleCodecError::MissingProductAwbcExecutable)?
                .encode_product_section()?,
        ),
        required_section(
            BundleSectionKind::RuntimeTypes,
            CompactRuntimeTypesSection::from_bundle(bundle)
                .and_then(|section| section.encode_canonical_section())
                .map_err(|error| compact_encode_error(&error))?,
        ),
        required_section(
            BundleSectionKind::Entrypoints,
            CompactEntrypointsSection::from_bundle(bundle)
                .and_then(|section| section.encode_canonical_section())
                .map_err(|error| compact_encode_error(&error))?,
        ),
        required_section(
            BundleSectionKind::AdapterRequirements,
            CompactAdapterRequirementsSection::from_bundle(bundle)
                .and_then(|section| section.encode_canonical_section())
                .map_err(|error| compact_encode_error(&error))?,
        ),
        required_section(
            BundleSectionKind::ContentCatalog,
            CompactContentCatalogSection::from_bundle(bundle)
                .encode_canonical_section()
                .map_err(|error| compact_encode_error(&error))?,
        ),
        optional_section(
            BundleSectionKind::DisplayCatalog,
            CompactDisplayCatalogSection::from_bundle(bundle)
                .encode_canonical_section()
                .map_err(|error| compact_encode_error(&error))?,
        ),
        optional_section(
            BundleSectionKind::SourceMap,
            bundle
                .source_map
                .encode_canonical_section()
                .map_err(|error| BundleCodecError::EncodeAwfb {
                    message: error.to_string(),
                })?,
        ),
    ]
    .into_iter()
    .chain(optional_asset_catalog_section(bundle)?)
    .chain(optional_audio_graph_section(bundle)?)
    .chain(optional_view_program_section(bundle)?)
    .chain(optional_view_style_section(bundle)?)
    .chain(optional_view_text_section(bundle)?)
    .chain(optional_view_input_section(bundle)?)
    .chain(optional_view_theme_section(bundle)?)
    .chain(optional_fx_definitions_section(bundle)?)
    .collect::<Vec<_>>();
    validate_style_section_inputs(bundle.view_style.as_ref(), &sections).map_err(|error| {
        BundleCodecError::EncodeAwfb {
            message: error.to_string(),
        }
    })?;
    encode_bundle(container_kind(bundle.bundle_kind), &manifest, sections).map_err(|error| {
        BundleCodecError::EncodeAwfb {
            message: error.to_string(),
        }
    })
}

pub(crate) fn from_awfb_slice(bytes: &[u8]) -> Result<ArcweftBundle, BundleCodecError> {
    from_awfb_slice_with_external_sections(bytes, &[])
}

pub(crate) fn from_awfb_slice_with_external_sections(
    bytes: &[u8],
    external_sections: &[ExternalSectionPayload],
) -> Result<ArcweftBundle, BundleCodecError> {
    let view = BundleView::parse(bytes, ReadBudget::default()).map_err(|error| {
        BundleCodecError::DecodeAwfb {
            message: error.to_string(),
        }
    })?;
    let product_manifest = decode_product_manifest(view.manifest())?;
    if product_manifest.schema_version != ARCWEFT_BUNDLE_SCHEMA_VERSION {
        return Err(BundleCodecError::UnsupportedSchema {
            actual: product_manifest.schema_version,
            expected: ARCWEFT_BUNDLE_SCHEMA_VERSION,
        });
    }
    if view.kind() != container_kind(product_manifest.bundle_kind) {
        return Err(BundleCodecError::DecodeAwfb {
            message: "container kind does not match product manifest bundle kind".to_owned(),
        });
    }
    let product_awbc = required_product_awbc_bytes(&view, external_sections)
        .and_then(|bytes| reject_structured_or_decode_awbc(&bytes))?;
    let runtime_types = required_runtime_types(&view, external_sections)?;
    runtime_types
        .validate_awbc(product_awbc.program())
        .map_err(|error| compact_decode_error(&error))?;
    let entrypoints = required_entrypoints(&view, external_sections)?;
    entrypoints
        .validate_manifest(&product_manifest.manifest)
        .and_then(|()| entrypoints.validate_awbc(product_awbc.program()))
        .map_err(|error| compact_decode_error(&error))?;
    let adapters = required_adapter_requirements(&view, external_sections)?;
    let _content = required_content_catalog(&view, external_sections)?;
    let assets = optional_asset_catalog(&view, external_sections)?.unwrap_or_default();
    let display = optional_display_catalog(&view, external_sections)?.unwrap_or_default();
    let source_map = optional_source_map(&view, external_sections)?.ok_or_else(|| {
        BundleCodecError::DecodeAwfb {
            message: "AWFB bundle is missing its canonical SourceMap section".to_owned(),
        }
    })?;
    let view_program = optional_view_program(&view, external_sections)?;
    let view_style = optional_view_style(&view, external_sections)?;
    validate_view_sources(
        view_program.as_ref(),
        view_style.as_ref(),
        Some(&source_map),
    )?;
    let audio = optional_audio_graph(&view, external_sections)?.map(|section| section.graph);
    let view_text = optional_view_text(&view, external_sections)?;
    let view_input = optional_view_input(&view, external_sections)?;
    let view_theme = optional_view_theme(&view, external_sections)?;
    let fx_definitions = optional_fx_definitions(&view, external_sections)?.unwrap_or_default();
    validate_style_bundle_view(view_style.as_ref(), &view, external_sections).map_err(|error| {
        BundleCodecError::DecodeAwfb {
            message: error.to_string(),
        }
    })?;

    let bundle = ArcweftBundle {
        schema_version: product_manifest.schema_version,
        bundle_kind: product_manifest.bundle_kind,
        manifest: product_manifest.manifest,
        agent: product_manifest.agent,
        source_map,
        bytecode: BundleBytecodeProgram {
            encoding: BundleBytecodeEncoding::StructuredJson,
            program: BytecodeProgram {
                runtime_layout: runtime_types.runtime_layout,
                ..BytecodeProgram::default()
            },
        },
        product_awbc: Some(product_awbc),
        display: display.display,
        fx_definitions,
        adapter_manifests: adapters.adapter_manifests,
        virtual_files: assets.virtual_files,
        image_assets: assets.image_assets,
        character_packages: Vec::new(),
        audio,
        image_objects: display.image_objects,
        view_program,
        view_style,
        view_text,
        view_input,
        view_theme,
    };
    bundle.validate_schema_and_kind()?;
    Ok(bundle)
}

fn required_runtime_types(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<CompactRuntimeTypesSection, BundleCodecError> {
    required_compact_payload(
        view,
        external_sections,
        BundleSectionKind::RuntimeTypes,
        CompactRuntimeTypesSection::decode_canonical_section,
    )
}

fn required_entrypoints(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<CompactEntrypointsSection, BundleCodecError> {
    required_compact_payload(
        view,
        external_sections,
        BundleSectionKind::Entrypoints,
        CompactEntrypointsSection::decode_canonical_section,
    )
}

fn required_adapter_requirements(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<CompactAdapterRequirementsSection, BundleCodecError> {
    required_compact_payload(
        view,
        external_sections,
        BundleSectionKind::AdapterRequirements,
        CompactAdapterRequirementsSection::decode_canonical_section,
    )
}

fn optional_asset_catalog_section(
    bundle: &ArcweftBundle,
) -> Result<Option<SectionInput>, BundleCodecError> {
    let section = CompactAssetCatalogSection::from_bundle(bundle);
    if section.is_empty() {
        return Ok(None);
    }
    section
        .encode_canonical_section()
        .map_err(|error| compact_encode_error(&error))
        .map(|bytes| Some(optional_section(BundleSectionKind::AssetCatalog, bytes)))
}

fn optional_audio_graph_section(
    bundle: &ArcweftBundle,
) -> Result<Option<SectionInput>, BundleCodecError> {
    bundle
        .audio
        .clone()
        .map(CompactAudioGraphSection::from_graph)
        .map(|section| {
            section
                .encode_canonical_section()
                .map_err(|error| compact_encode_error(&error))
                .map(|bytes| optional_section(BundleSectionKind::AudioGraph, bytes))
        })
        .transpose()
}

fn optional_view_program_section(
    bundle: &ArcweftBundle,
) -> Result<Option<SectionInput>, BundleCodecError> {
    optional_view_section(
        BundleSectionKind::ViewProgram,
        bundle
            .view_program
            .as_ref()
            .map(ViewProgramResource::encode_canonical_section),
    )
}

fn optional_view_style_section(
    bundle: &ArcweftBundle,
) -> Result<Option<SectionInput>, BundleCodecError> {
    optional_view_section(
        BundleSectionKind::ViewStyle,
        bundle
            .view_style
            .as_ref()
            .map(ViewStyleResource::encode_canonical_section),
    )
}

fn optional_view_text_section(
    bundle: &ArcweftBundle,
) -> Result<Option<SectionInput>, BundleCodecError> {
    optional_view_section(
        BundleSectionKind::ViewText,
        bundle
            .view_text
            .as_ref()
            .map(ViewTextResource::encode_canonical_section),
    )
}

fn optional_view_input_section(
    bundle: &ArcweftBundle,
) -> Result<Option<SectionInput>, BundleCodecError> {
    optional_view_section(
        BundleSectionKind::ViewInput,
        bundle
            .view_input
            .as_ref()
            .map(ViewInputResource::encode_canonical_section),
    )
}

fn optional_view_theme_section(
    bundle: &ArcweftBundle,
) -> Result<Option<SectionInput>, BundleCodecError> {
    optional_view_section(
        BundleSectionKind::ViewTheme,
        bundle
            .view_theme
            .as_ref()
            .map(ViewThemeResource::encode_canonical_section),
    )
}

fn optional_fx_definitions_section(
    bundle: &ArcweftBundle,
) -> Result<Option<SectionInput>, BundleCodecError> {
    if bundle.fx_definitions.is_empty() {
        return Ok(None);
    }
    bundle
        .fx_definitions
        .encode_canonical_section()
        .map(|bytes| optional_section(BundleSectionKind::FxDefinitions, bytes))
        .map(Some)
        .map_err(|source| BundleCodecError::EncodeAwfb {
            message: source.to_string(),
        })
}

fn optional_view_section(
    kind: BundleSectionKind,
    encode: Option<Result<Vec<u8>, crate::resource_codec::SectionCodecError>>,
) -> Result<Option<SectionInput>, BundleCodecError> {
    encode
        .transpose()
        .map_err(|error| compact_encode_error(&error))
        .map(|bytes| bytes.map(|bytes| optional_section(kind, bytes)))
}

fn required_content_catalog(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<CompactContentCatalogSection, BundleCodecError> {
    required_compact_payload(
        view,
        external_sections,
        BundleSectionKind::ContentCatalog,
        CompactContentCatalogSection::decode_canonical_section,
    )
}

fn optional_asset_catalog(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<Option<CompactAssetCatalogSection>, BundleCodecError> {
    optional_compact_payload(
        view,
        external_sections,
        BundleSectionKind::AssetCatalog,
        CompactAssetCatalogSection::decode_canonical_section,
    )
}

fn optional_display_catalog(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<Option<CompactDisplayCatalogSection>, BundleCodecError> {
    optional_compact_payload(
        view,
        external_sections,
        BundleSectionKind::DisplayCatalog,
        CompactDisplayCatalogSection::decode_canonical_section,
    )
}

fn optional_source_map(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<Option<SourceMapSection>, BundleCodecError> {
    optional_compact_payload(
        view,
        external_sections,
        BundleSectionKind::SourceMap,
        SourceMapSection::decode_canonical_section,
    )
}

fn optional_audio_graph(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<Option<CompactAudioGraphSection>, BundleCodecError> {
    optional_compact_payload(
        view,
        external_sections,
        BundleSectionKind::AudioGraph,
        CompactAudioGraphSection::decode_canonical_section,
    )
}

fn optional_view_program(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<Option<ViewProgramResource>, BundleCodecError> {
    optional_compact_payload(
        view,
        external_sections,
        BundleSectionKind::ViewProgram,
        ViewProgramResource::decode_canonical_section,
    )
}

fn optional_view_style(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<Option<ViewStyleResource>, BundleCodecError> {
    optional_compact_payload(
        view,
        external_sections,
        BundleSectionKind::ViewStyle,
        ViewStyleResource::decode_canonical_section,
    )
}

fn optional_view_text(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<Option<ViewTextResource>, BundleCodecError> {
    optional_compact_payload(
        view,
        external_sections,
        BundleSectionKind::ViewText,
        ViewTextResource::decode_canonical_section,
    )
}

fn optional_view_input(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<Option<ViewInputResource>, BundleCodecError> {
    optional_compact_payload(
        view,
        external_sections,
        BundleSectionKind::ViewInput,
        ViewInputResource::decode_canonical_section,
    )
}

fn optional_view_theme(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<Option<ViewThemeResource>, BundleCodecError> {
    optional_compact_payload(
        view,
        external_sections,
        BundleSectionKind::ViewTheme,
        ViewThemeResource::decode_canonical_section,
    )
}

fn optional_fx_definitions(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<Option<FxDefinitions>, BundleCodecError> {
    let mut matches = view
        .sections()
        .iter()
        .filter(|descriptor| descriptor.known_kind() == Some(BundleSectionKind::FxDefinitions));
    let Some(descriptor) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        return Err(BundleCodecError::DecodeAwfb {
            message: "AWFB bundle contains multiple FxDefinitions sections".to_owned(),
        });
    }
    let bytes = view
        .decoded_section_with_external_payloads(descriptor.id(), external_sections)
        .map_err(|source| BundleCodecError::DecodeAwfb {
            message: source.to_string(),
        })?
        .ok_or_else(|| BundleCodecError::DecodeAwfb {
            message: "AWFB FxDefinitions section is external and cannot be decoded inline"
                .to_owned(),
        })?;
    FxDefinitions::decode_canonical_section(&bytes)
        .map(Some)
        .map_err(|source| BundleCodecError::DecodeAwfb {
            message: source.to_string(),
        })
}

fn required_compact_payload<T>(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
    kind: BundleSectionKind,
    decode: fn(&[u8]) -> Result<T, crate::resource_codec::SectionCodecError>,
) -> Result<T, BundleCodecError> {
    let descriptor = required_descriptor(view, kind)?;
    let bytes = view
        .decoded_section_with_external_payloads(descriptor.id(), external_sections)
        .map_err(|error| BundleCodecError::DecodeAwfb {
            message: error.to_string(),
        })?
        .ok_or_else(|| BundleCodecError::DecodeAwfb {
            message: format!("AWFB {kind:?} section is external and cannot be decoded inline"),
        })?;
    decode(&bytes).map_err(|error| compact_decode_error(&error))
}

fn optional_compact_payload<T, E>(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
    kind: BundleSectionKind,
    decode: fn(&[u8]) -> Result<T, E>,
) -> Result<Option<T>, BundleCodecError>
where
    E: std::fmt::Display,
{
    let mut matches = view
        .sections()
        .iter()
        .filter(|descriptor| descriptor.known_kind() == Some(kind));
    let Some(descriptor) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        return Err(BundleCodecError::DecodeAwfb {
            message: format!("AWFB bundle contains multiple {kind:?} sections"),
        });
    }
    let Some(bytes) = view
        .decoded_section_with_external_payloads(descriptor.id(), external_sections)
        .map_err(|error| BundleCodecError::DecodeAwfb {
            message: error.to_string(),
        })?
    else {
        return Err(BundleCodecError::DecodeAwfb {
            message: format!("AWFB {kind:?} section is external and cannot be decoded inline"),
        });
    };
    decode(&bytes)
        .map(Some)
        .map_err(|error| BundleCodecError::DecodeAwfb {
            message: error.to_string(),
        })
}

fn compact_encode_error(error: &crate::resource_codec::SectionCodecError) -> BundleCodecError {
    BundleCodecError::EncodeAwfb {
        message: error.to_string(),
    }
}

fn compact_decode_error(error: &crate::resource_codec::SectionCodecError) -> BundleCodecError {
    BundleCodecError::DecodeAwfb {
        message: error.to_string(),
    }
}

fn required_section(kind: BundleSectionKind, bytes: Vec<u8>) -> SectionInput {
    SectionInput::embedded(
        section_id(kind),
        kind,
        AWFB_SECTION_SCHEMA_VERSION,
        ContentResidency::Startup,
        true,
        bytes,
    )
}

fn optional_section(kind: BundleSectionKind, bytes: Vec<u8>) -> SectionInput {
    SectionInput::embedded(
        section_id(kind),
        kind,
        AWFB_SECTION_SCHEMA_VERSION,
        kind.default_residency(),
        false,
        bytes,
    )
}

fn required_product_awbc_bytes(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<Vec<u8>, BundleCodecError> {
    let descriptor = required_descriptor(view, BundleSectionKind::ProgramBytecode)
        .map_err(|_| BundleCodecError::MissingProductAwbcExecutable)?;
    view.decoded_section_with_external_payloads(descriptor.id(), external_sections)
        .map_err(|error| BundleCodecError::DecodeAwfb {
            message: error.to_string(),
        })?
        .ok_or_else(|| BundleCodecError::DecodeAwfb {
            message: "AWFB ProgramBytecode section is external and cannot be decoded inline"
                .to_owned(),
        })
}

fn required_descriptor<'a>(
    view: &'a BundleView<'_>,
    kind: BundleSectionKind,
) -> Result<&'a crate::container::SectionDescriptor, BundleCodecError> {
    let mut matches = view
        .sections()
        .iter()
        .filter(|descriptor| descriptor.known_kind() == Some(kind));
    let Some(descriptor) = matches.next() else {
        return Err(BundleCodecError::DecodeAwfb {
            message: format!("AWFB bundle is missing required {kind:?} section"),
        });
    };
    if matches.next().is_some() {
        return Err(BundleCodecError::DecodeAwfb {
            message: format!("AWFB bundle contains multiple {kind:?} sections"),
        });
    }
    Ok(descriptor)
}

fn reject_structured_or_decode_awbc(bytes: &[u8]) -> Result<BundleAwbcProgram, BundleCodecError> {
    if let Some(tag) = structured_product_container_tag(bytes) {
        return Err(BundleCodecError::StructuredProductBytecodeUnsupported { encoding_tag: tag });
    }
    BundleAwbcProgram::decode_product_section(bytes)
}

fn structured_product_container_tag(bytes: &[u8]) -> Option<u32> {
    if bytes.len() < 16 || !bytes.starts_with(&LEGACY_BYTECODE_SECTION_MAGIC) {
        return None;
    }
    let tag = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
    matches!(
        tag,
        LEGACY_PRODUCT_BYTECODE_MESSAGEPACK_TAG | LEGACY_PRODUCT_BYTECODE_COMPACT_SIDECAR_TAG
    )
    .then_some(tag)
}

fn encode_json<T>(value: &T) -> Result<Vec<u8>, BundleCodecError>
where
    T: Serialize,
{
    serde_json::to_vec(value).map_err(|error| BundleCodecError::EncodeAwfb {
        message: error.to_string(),
    })
}

fn decode_json<T>(bytes: &[u8]) -> Result<T, BundleCodecError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_slice(bytes).map_err(|error| BundleCodecError::DecodeAwfb {
        message: error.to_string(),
    })
}

fn decode_product_manifest(bytes: &[u8]) -> Result<ProductManifest, BundleCodecError> {
    let probe = decode_json::<ProductExecutablePayloadProbe>(bytes)?;
    match probe.executable_payload {
        ProductExecutablePayloadField::Missing => {
            return Err(BundleCodecError::MissingProductExecutablePayload);
        }
        ProductExecutablePayloadField::Present(serde_json::Value::String(wire_name))
            if ProductExecutablePayload::from_wire_name(&wire_name).is_none() =>
        {
            return Err(BundleCodecError::UnsupportedProductExecutablePayload {
                actual: wire_name,
            });
        }
        ProductExecutablePayloadField::Present(_) => {}
    }
    decode_json(bytes)
}

fn container_kind(kind: BundleKind) -> ContainerBundleKind {
    match kind {
        BundleKind::Game => ContainerBundleKind::Program,
        BundleKind::AgentController => ContainerBundleKind::AgentController,
    }
}

fn section_id(kind: BundleSectionKind) -> SectionId {
    let mut id = [0_u8; 16];
    id[..4].copy_from_slice(&kind.encoded().to_le_bytes());
    id[4..].copy_from_slice(&BundleDigest::of(b"arcweft-awfb-v1-section").as_bytes()[..12]);
    SectionId::from_bytes(id)
}

#[cfg(test)]
mod tests {
    use super::{
        AWFB_SECTION_SCHEMA_VERSION, CompactAdapterRequirementsSection, CompactEntrypointsSection,
        CompactRuntimeTypesSection, ProductExecutablePayload, ProductManifest, container_kind,
        encode_json, optional_asset_catalog_section, optional_audio_graph_section,
        optional_section, required_section, section_id,
    };
    use crate::container::{
        BundleDigest, BundleSectionKind, BundleView, ExternalSectionPayload, ReadBudget,
        SectionInput, encode_bundle,
    };
    use crate::fx_definitions::FxDefinitions;
    use crate::resource_codec::{
        CompactContentCatalogSection, CompactDisplayCatalogSection, SourceMapSection,
    };
    use crate::{
        ArcweftBundle, BundleCodecError, BundleFormat, BundleManifest, BundleRuntimeSummary,
    };
    use arcweft_core::awbc::schema::{
        AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
        AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId,
        AwbcFunctionKind, AwbcProgram, AwbcSafePointKind, AwbcSignature, AwbcSignatureId,
        AwbcStringId, AwbcTableRange, AwbcTerminator,
    };
    use arcweft_core::bytecode::BytecodeProgram;
    use arcweft_presentation::fx::{FxDefinition, FxGraph, FxId, FxNode};
    use arcweft_render_text::LineDisplayCatalog;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use std::path::Path;

    #[test]
    fn awfb_product_decodes_verified_external_program_bytecode_section() {
        let bundle = empty_bundle();
        let (bytes, payload) = awfb_with_external_bytecode(&bundle);

        let inline_error =
            super::from_awfb_slice(&bytes).expect_err("external bytecode requires payload");
        assert!(matches!(inline_error, BundleCodecError::DecodeAwfb { .. }));

        let decoded = ArcweftBundle::from_awfb_slice_with_external_sections(&bytes, &[payload])
            .expect("external bytecode payload decodes");

        assert_eq!(decoded.product_awbc(), bundle.product_awbc());
        assert_eq!(decoded.manifest, bundle.manifest);
    }

    #[test]
    fn awfb_product_path_decode_accepts_external_sections() {
        let bundle = empty_bundle();
        let (bytes, payload) = awfb_with_external_bytecode(&bundle);

        let decoded = ArcweftBundle::from_product_path_slice_with_external_sections(
            Path::new("game.awfb"),
            &bytes,
            &[payload],
        )
        .expect("external product payload decodes");

        assert_eq!(decoded.product_awbc(), bundle.product_awbc());
        assert_eq!(decoded.manifest, bundle.manifest);
    }

    #[test]
    fn awfb_product_encodes_program_bytecode_as_canonical_awbc() {
        let bundle = empty_bundle();
        let bytes = bundle
            .to_format_bytes(BundleFormat::Awfb)
            .expect("AWFB encodes");
        let view = BundleView::parse(&bytes, ReadBudget::default()).expect("AWFB parses");
        let descriptor = view
            .sections()
            .iter()
            .find(|section| section.known_kind() == Some(BundleSectionKind::ProgramBytecode))
            .expect("program bytecode section exists");
        let bytecode_bytes = view
            .decoded_section(descriptor.id())
            .expect("bytecode section decodes")
            .expect("bytecode section is embedded");

        assert_ne!(bytecode_bytes.first(), Some(&b'{'));
        assert_eq!(
            bytecode_bytes,
            bundle
                .product_awbc_program()
                .expect("product AWBC exists")
                .encode_canonical()
                .expect("AWBC encodes")
        );
        assert_eq!(
            super::from_awfb_slice(&bytes)
                .expect("AWBC bytecode AWFB decodes")
                .product_awbc(),
            bundle.product_awbc()
        );
    }

    #[test]
    fn awfb_product_manifest_discriminator_round_trips_canonically() {
        let bundle = empty_bundle();
        let bytes = bundle
            .to_format_bytes(BundleFormat::Awfb)
            .expect("AWFB encodes");
        let view = BundleView::parse(&bytes, ReadBudget::default()).expect("AWFB parses");
        let manifest = serde_json::from_slice::<serde_json::Value>(view.manifest())
            .expect("product manifest is JSON");

        assert_eq!(
            manifest
                .get("executable_payload")
                .and_then(|value| value.as_str()),
            Some(ProductExecutablePayload::AwbcV1.wire_name())
        );

        let decoded = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &bytes)
            .expect("canonical product manifest decodes");
        assert_eq!(decoded, bundle);
        assert_eq!(
            decoded
                .to_format_bytes(BundleFormat::Awfb)
                .expect("decoded AWFB re-encodes"),
            bytes,
            "canonical product bytes must be deterministic after round-trip"
        );
    }

    #[test]
    fn awfb_product_rejects_missing_executable_payload_discriminator() {
        let bundle = empty_bundle();
        let mut manifest = product_manifest_json(&bundle);
        manifest
            .as_object_mut()
            .expect("product manifest is an object")
            .remove("executable_payload");
        let (bytes, payload) = awfb_with_external_bytecode_and_manifest(&bundle, &manifest);

        let error = ArcweftBundle::from_awfb_slice_with_external_sections(&bytes, &[payload])
            .expect_err("missing executable payload discriminator is rejected");

        assert!(matches!(
            error,
            BundleCodecError::MissingProductExecutablePayload
        ));
    }

    #[test]
    fn awfb_product_rejects_null_executable_payload_as_malformed() {
        let bundle = empty_bundle();
        let mut manifest = product_manifest_json(&bundle);
        manifest["executable_payload"] = serde_json::Value::Null;
        let (bytes, payload) = awfb_with_external_bytecode_and_manifest(&bundle, &manifest);

        let error = ArcweftBundle::from_awfb_slice_with_external_sections(&bytes, &[payload])
            .expect_err("null executable payload discriminator is malformed");

        assert!(matches!(error, BundleCodecError::DecodeAwfb { .. }));
    }

    #[test]
    fn awfb_product_rejects_duplicate_executable_payload_as_malformed() {
        let bundle = empty_bundle();
        let manifest = String::from_utf8(
            encode_json(&product_manifest(&bundle)).expect("canonical manifest encodes"),
        )
        .expect("canonical manifest is UTF-8");
        let manifest_body = manifest
            .strip_prefix('{')
            .expect("canonical manifest is a JSON object");
        let manifest = format!(
            "{{\"executable_payload\":\"{}\",{}",
            ProductExecutablePayload::AwbcV1.wire_name(),
            manifest_body
        );
        let (bytes, payload) =
            awfb_with_external_bytecode_and_manifest_bytes(&bundle, manifest.as_bytes());

        let error = ArcweftBundle::from_awfb_slice_with_external_sections(&bytes, &[payload])
            .expect_err("duplicate executable payload discriminator is malformed");

        assert!(matches!(error, BundleCodecError::DecodeAwfb { .. }));
    }

    #[test]
    fn awfb_product_rejects_wrong_executable_payload_discriminator() {
        let bundle = empty_bundle();
        let mut manifest = product_manifest_json(&bundle);
        manifest["executable_payload"] = serde_json::Value::String("structured_json_v0".to_owned());
        let (bytes, payload) = awfb_with_external_bytecode_and_manifest(&bundle, &manifest);

        let error = ArcweftBundle::from_awfb_slice_with_external_sections(&bytes, &[payload])
            .expect_err("wrong executable payload discriminator is rejected");

        assert!(matches!(
            error,
            BundleCodecError::UnsupportedProductExecutablePayload { actual }
                if actual == "structured_json_v0"
        ));
    }

    #[test]
    fn awfb_product_round_trips_first_class_fx_definitions_section() {
        let definition = FxDefinition::new(
            FxId::try_new("test", "fx.pulse").expect("valid Fx ID"),
            Vec::new(),
            FxGraph::new(vec![FxNode::Text {
                properties: Vec::new(),
            }]),
        )
        .expect("valid Fx definition");
        let bundle = empty_bundle()
            .with_fx_definitions(FxDefinitions::try_new([definition]).expect("valid Fx inventory"));
        let bytes = bundle
            .to_format_bytes(BundleFormat::Awfb)
            .expect("AWFB encodes");
        let view = BundleView::parse(&bytes, ReadBudget::default()).expect("AWFB parses");
        assert!(
            view.sections()
                .iter()
                .any(|section| { section.known_kind() == Some(BundleSectionKind::FxDefinitions) })
        );

        let decoded = super::from_awfb_slice(&bytes).expect("AWFB decodes");
        assert_eq!(decoded.fx_definitions, bundle.fx_definitions);
    }

    #[test]
    fn awfb_product_rejects_old_structured_product_bytecode_tag() {
        let bundle = empty_bundle();
        let mut old_payload = Vec::new();
        old_payload.extend_from_slice(b"AWBC\r\n\x1a\n");
        old_payload.extend_from_slice(&1_u32.to_le_bytes());
        old_payload.extend_from_slice(&2_u32.to_le_bytes());
        old_payload.extend_from_slice(&0_u32.to_le_bytes());
        old_payload.extend_from_slice(&0_u32.to_le_bytes());
        let (bytes, payload) = awfb_with_external_program_bytecode(&bundle, old_payload);

        let error = ArcweftBundle::from_awfb_slice_with_external_sections(&bytes, &[payload])
            .expect_err("old structured product bytecode is rejected");

        assert!(matches!(
            error,
            BundleCodecError::StructuredProductBytecodeUnsupported { encoding_tag: 2 }
        ));
    }

    fn awfb_with_external_bytecode(bundle: &ArcweftBundle) -> (Vec<u8>, ExternalSectionPayload) {
        let bytecode_bytes = bundle
            .product_awbc_program()
            .expect("product AWBC exists")
            .encode_canonical()
            .expect("AWBC encodes");
        awfb_with_external_program_bytecode(bundle, bytecode_bytes)
    }

    fn awfb_with_external_bytecode_and_manifest(
        bundle: &ArcweftBundle,
        manifest: &serde_json::Value,
    ) -> (Vec<u8>, ExternalSectionPayload) {
        let manifest = serde_json::to_vec(manifest).expect("manifest encodes");
        awfb_with_external_bytecode_and_manifest_bytes(bundle, &manifest)
    }

    fn awfb_with_external_bytecode_and_manifest_bytes(
        bundle: &ArcweftBundle,
        manifest: &[u8],
    ) -> (Vec<u8>, ExternalSectionPayload) {
        let bytecode_bytes = bundle
            .product_awbc_program()
            .expect("product AWBC exists")
            .encode_canonical()
            .expect("AWBC encodes");
        awfb_with_external_program_bytecode_and_manifest(bundle, bytecode_bytes, manifest)
    }

    fn awfb_with_external_program_bytecode(
        bundle: &ArcweftBundle,
        bytecode_bytes: Vec<u8>,
    ) -> (Vec<u8>, ExternalSectionPayload) {
        let manifest = encode_json(&product_manifest(bundle)).expect("manifest encodes");
        awfb_with_external_program_bytecode_and_manifest(bundle, bytecode_bytes, &manifest)
    }

    fn awfb_with_external_program_bytecode_and_manifest(
        bundle: &ArcweftBundle,
        bytecode_bytes: Vec<u8>,
        manifest: &[u8],
    ) -> (Vec<u8>, ExternalSectionPayload) {
        let bytecode_id = section_id(BundleSectionKind::ProgramBytecode);
        let sections = vec![
            SectionInput::external_ref(
                bytecode_id,
                BundleSectionKind::ProgramBytecode,
                AWFB_SECTION_SCHEMA_VERSION,
                BundleSectionKind::ProgramBytecode.default_residency(),
                true,
                bytecode_bytes.len() as u64,
                BundleDigest::of(&bytecode_bytes),
            ),
            required_section(
                BundleSectionKind::RuntimeTypes,
                CompactRuntimeTypesSection::from_bundle(bundle)
                    .and_then(|section| section.encode_canonical_section())
                    .expect("runtime types encode"),
            ),
            required_section(
                BundleSectionKind::Entrypoints,
                CompactEntrypointsSection::from_bundle(bundle)
                    .and_then(|section| section.encode_canonical_section())
                    .expect("entrypoints encode"),
            ),
            required_section(
                BundleSectionKind::AdapterRequirements,
                CompactAdapterRequirementsSection::from_bundle(bundle)
                    .and_then(|section| section.encode_canonical_section())
                    .expect("adapter requirements encode"),
            ),
            required_section(
                BundleSectionKind::ContentCatalog,
                CompactContentCatalogSection::from_bundle(bundle)
                    .encode_canonical_section()
                    .expect("content catalog encode"),
            ),
            optional_section(
                BundleSectionKind::DisplayCatalog,
                CompactDisplayCatalogSection::from_bundle(bundle)
                    .encode_canonical_section()
                    .expect("display catalog encode"),
            ),
            optional_section(
                BundleSectionKind::SourceMap,
                bundle
                    .source_map
                    .encode_canonical_section()
                    .expect("source encode"),
            ),
        ]
        .into_iter()
        .chain(optional_asset_catalog_section(bundle).expect("asset catalog encodes"))
        .chain(optional_audio_graph_section(bundle).expect("audio graph encodes"))
        .collect::<Vec<_>>();
        let bytes = encode_bundle(container_kind(bundle.bundle_kind), manifest, sections)
            .expect("AWFB encodes");
        (
            bytes,
            ExternalSectionPayload::new(bytecode_id, bytecode_bytes),
        )
    }

    fn product_manifest(bundle: &ArcweftBundle) -> ProductManifest {
        ProductManifest {
            schema_version: bundle.schema_version,
            bundle_kind: bundle.bundle_kind,
            executable_payload: ProductExecutablePayload::AwbcV1,
            manifest: bundle.manifest.clone(),
            agent: bundle.agent.clone(),
        }
    }

    fn product_manifest_json(bundle: &ArcweftBundle) -> serde_json::Value {
        serde_json::to_value(product_manifest(bundle)).expect("manifest is JSON")
    }

    fn empty_bundle() -> ArcweftBundle {
        ArcweftBundle::new(
            BundleManifest {
                profile_id: None,
                profile_kind: None,
                entry: Some("main".to_owned()),
                adapter: None,
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: 0,
                    line_task_groups: 0,
                    stream_plans: 0,
                    source_plans: 0,
                },
            },
            source_map("main.arcw", "flow @flow.main main { return \"ok\" }"),
            BytecodeProgram::default(),
            LineDisplayCatalog::default(),
        )
        .with_product_awbc(minimal_awbc_program())
    }

    fn source_map(label: &str, text: &str) -> SourceMapSection {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new(label).expect("source ID"),
            SourceName::path(label),
            text,
        )
        .expect("source document");
        SourceMapSection::try_from_documents(&[&document]).expect("source map")
    }

    fn minimal_awbc_program() -> AwbcProgram {
        AwbcProgram {
            strings: vec!["entry.main".to_owned()],
            signatures: vec![AwbcSignature {
                params: Vec::new(),
                result: None,
                effects: AwbcEffectSetId(0),
            }],
            frame_layouts: vec![AwbcFrameLayout {
                slots: Vec::new(),
                max_scope_depth: 0,
            }],
            functions: vec![AwbcFunction {
                public_id: Some(AwbcStringId(0)),
                kind: AwbcFunctionKind::Flow,
                signature: AwbcSignatureId(0),
                frame_layout: AwbcFrameLayoutId(0),
                blocks: AwbcTableRange::new(0, 1),
                entry_block: AwbcBlockId(0),
                flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
            }],
            blocks: vec![AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Return { value: None },
                safe_point: AwbcSafePointKind::FlowEntry,
                source_map: None,
            }],
            entries: vec![AwbcEntry {
                public_id: AwbcStringId(0),
                kind: AwbcEntryKind::Game,
                signature: AwbcSignatureId(0),
                target: AwbcEntryTarget::Function(AwbcFunctionId(0)),
            }],
            ..AwbcProgram::default()
        }
    }
}
