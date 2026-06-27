use crate::container::{
    BundleDigest, BundleKind as ContainerBundleKind, BundleSectionKind, BundleView,
    ContentResidency, ExternalSectionPayload, ReadBudget, SectionId, SectionInput, encode_bundle,
};
use crate::{
    ARCWEFT_BUNDLE_SCHEMA_VERSION, ArcweftBundle, BundleAdapterManifest, BundleAwbcProgram,
    BundleBytecodeEncoding, BundleBytecodeProgram, BundleCodecError, BundleImageAsset,
    BundleImageObject, BundleKind, BundleManifest, BundleSource, BundleVirtualFile,
};
use arcweft_agent_protocol::artifact::AgentArtifactManifest;
use arcweft_audio_core::graph::AudioGraph;
use arcweft_core::bytecode::{BytecodeProgram, BytecodeRuntimeLayout};
use arcweft_render_text::LineDisplayCatalog;
use serde::{Deserialize, Serialize};

const AWFB_SECTION_SCHEMA_VERSION: u32 = 1;
const LEGACY_BYTECODE_SECTION_MAGIC: [u8; 8] = *b"AWBC\r\n\x1a\n";
const LEGACY_PRODUCT_BYTECODE_MESSAGEPACK_TAG: u32 = 1;
const LEGACY_PRODUCT_BYTECODE_COMPACT_SIDECAR_TAG: u32 = 2;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProductManifest {
    schema_version: u32,
    bundle_kind: BundleKind,
    #[serde(default = "default_executable_payload")]
    executable_payload: String,
    manifest: BundleManifest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent: Option<AgentArtifactManifest>,
}

fn default_executable_payload() -> String {
    crate::product_awbc::PRODUCT_EXECUTABLE_PAYLOAD_AWBC_V1.to_owned()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RuntimeTypesSection {
    schema_version: u32,
    #[serde(default)]
    runtime_layout: BytecodeRuntimeLayout,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EntrypointsSection {
    entry: Option<String>,
    entry_flow: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AdapterRequirementsSection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    adapter_manifest_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    required_host_calls: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    adapter_manifests: Vec<BundleAdapterManifest>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ContentCatalogSection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    virtual_files: Vec<BundleVirtualFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    image_assets: Vec<BundleImageAsset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    audio: Option<AudioGraph>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    image_objects: Vec<BundleImageObject>,
}

pub(crate) fn to_awfb_bytes(bundle: &ArcweftBundle) -> Result<Vec<u8>, BundleCodecError> {
    let product_manifest = ProductManifest {
        schema_version: bundle.schema_version,
        bundle_kind: bundle.bundle_kind,
        executable_payload: crate::product_awbc::PRODUCT_EXECUTABLE_PAYLOAD_AWBC_V1.to_owned(),
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
            encode_json(&RuntimeTypesSection {
                schema_version: AWFB_SECTION_SCHEMA_VERSION,
                runtime_layout: bundle.bytecode.program.runtime_layout.clone(),
            })?,
        ),
        required_section(
            BundleSectionKind::Entrypoints,
            encode_json(&EntrypointsSection {
                entry: bundle.manifest.entry.clone(),
                entry_flow: bundle.manifest.runtime.entry_flow.clone(),
            })?,
        ),
        required_section(
            BundleSectionKind::AdapterRequirements,
            encode_json(&AdapterRequirementsSection {
                adapter_manifest_ids: bundle.manifest.adapter_manifest_ids.clone(),
                required_host_calls: bundle.manifest.required_host_calls.clone(),
                adapter_manifests: bundle.adapter_manifests.clone(),
            })?,
        ),
        required_section(
            BundleSectionKind::ContentCatalog,
            encode_json(&ContentCatalogSection {
                virtual_files: bundle.virtual_files.clone(),
                image_assets: bundle.image_assets.clone(),
                audio: bundle.audio.clone(),
                image_objects: bundle.image_objects.clone(),
            })?,
        ),
        optional_section(
            BundleSectionKind::DisplayCatalog,
            encode_json(&bundle.display)?,
        ),
        optional_section(
            BundleSectionKind::NormalizedSource,
            encode_json(&bundle.source)?,
        ),
    ];
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
    let product_manifest = decode_json::<ProductManifest>(view.manifest())?;
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
    if product_manifest.executable_payload
        != crate::product_awbc::PRODUCT_EXECUTABLE_PAYLOAD_AWBC_V1
    {
        return Err(BundleCodecError::UnsupportedProductExecutablePayload {
            actual: product_manifest.executable_payload,
        });
    }

    let runtime_types = required_payload::<RuntimeTypesSection>(
        &view,
        external_sections,
        BundleSectionKind::RuntimeTypes,
    )?;
    let _entrypoints = required_payload::<EntrypointsSection>(
        &view,
        external_sections,
        BundleSectionKind::Entrypoints,
    )?;
    let adapters = required_payload::<AdapterRequirementsSection>(
        &view,
        external_sections,
        BundleSectionKind::AdapterRequirements,
    )?;
    let content = required_payload::<ContentCatalogSection>(
        &view,
        external_sections,
        BundleSectionKind::ContentCatalog,
    )?;
    let product_awbc = required_product_awbc_bytes(&view, external_sections)
        .and_then(|bytes| reject_structured_or_decode_awbc(&bytes))?;
    let display = optional_payload::<LineDisplayCatalog>(
        &view,
        external_sections,
        BundleSectionKind::DisplayCatalog,
    )?
    .unwrap_or_default();
    let source = optional_payload::<BundleSource>(
        &view,
        external_sections,
        BundleSectionKind::NormalizedSource,
    )?
    .unwrap_or_else(|| BundleSource {
        label: product_manifest.manifest.source_label.clone(),
        text: String::new(),
    });

    Ok(ArcweftBundle {
        schema_version: product_manifest.schema_version,
        bundle_kind: product_manifest.bundle_kind,
        manifest: product_manifest.manifest,
        agent: product_manifest.agent,
        source,
        bytecode: BundleBytecodeProgram {
            encoding: BundleBytecodeEncoding::StructuredJson,
            program: BytecodeProgram {
                runtime_layout: runtime_types.runtime_layout,
                ..BytecodeProgram::default()
            },
        },
        product_awbc: Some(product_awbc),
        display,
        adapter_manifests: adapters.adapter_manifests,
        virtual_files: content.virtual_files,
        image_assets: content.image_assets,
        audio: content.audio,
        image_objects: content.image_objects,
    })
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

fn required_payload<T>(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
    kind: BundleSectionKind,
) -> Result<T, BundleCodecError>
where
    T: for<'de> Deserialize<'de>,
{
    optional_payload(view, external_sections, kind)?.ok_or_else(|| BundleCodecError::DecodeAwfb {
        message: format!("AWFB bundle is missing required {kind:?} section"),
    })
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

fn optional_payload<T>(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
    kind: BundleSectionKind,
) -> Result<Option<T>, BundleCodecError>
where
    T: for<'de> Deserialize<'de>,
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
    decode_json(&bytes).map(Some)
}

fn reject_structured_or_decode_awbc(bytes: &[u8]) -> Result<BundleAwbcProgram, BundleCodecError> {
    if let Some(tag) = legacy_structured_product_tag(bytes) {
        return Err(BundleCodecError::StructuredProductBytecodeUnsupported { encoding_tag: tag });
    }
    BundleAwbcProgram::decode_product_section(bytes)
}

fn legacy_structured_product_tag(bytes: &[u8]) -> Option<u32> {
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
        AWFB_SECTION_SCHEMA_VERSION, AdapterRequirementsSection, ContentCatalogSection,
        EntrypointsSection, ProductManifest, RuntimeTypesSection, container_kind, encode_json,
        optional_section, required_section, section_id,
    };
    use crate::container::{
        BundleDigest, BundleSectionKind, BundleView, ExternalSectionPayload, ReadBudget,
        SectionInput, encode_bundle,
    };
    use crate::{
        ArcweftBundle, BundleCodecError, BundleFormat, BundleManifest, BundleRuntimeSummary,
        BundleSource,
    };
    use arcweft_core::awbc::schema::{
        AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
        AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId,
        AwbcFunctionKind, AwbcProgram, AwbcSafePointKind, AwbcSignature, AwbcSignatureId,
        AwbcStringId, AwbcTableRange, AwbcTerminator,
    };
    use arcweft_core::bytecode::BytecodeProgram;
    use arcweft_render_text::LineDisplayCatalog;
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

    fn awfb_with_external_program_bytecode(
        bundle: &ArcweftBundle,
        bytecode_bytes: Vec<u8>,
    ) -> (Vec<u8>, ExternalSectionPayload) {
        let product_manifest = ProductManifest {
            schema_version: bundle.schema_version,
            bundle_kind: bundle.bundle_kind,
            executable_payload: crate::product_awbc::PRODUCT_EXECUTABLE_PAYLOAD_AWBC_V1.to_owned(),
            manifest: bundle.manifest.clone(),
            agent: bundle.agent.clone(),
        };
        let manifest = encode_json(&product_manifest).expect("manifest encodes");
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
                encode_json(&RuntimeTypesSection {
                    schema_version: AWFB_SECTION_SCHEMA_VERSION,
                    runtime_layout: bundle.bytecode.program.runtime_layout.clone(),
                })
                .expect("runtime types encode"),
            ),
            required_section(
                BundleSectionKind::Entrypoints,
                encode_json(&EntrypointsSection {
                    entry: bundle.manifest.entry.clone(),
                    entry_flow: bundle.manifest.runtime.entry_flow.clone(),
                })
                .expect("entrypoints encode"),
            ),
            required_section(
                BundleSectionKind::AdapterRequirements,
                encode_json(&AdapterRequirementsSection {
                    adapter_manifest_ids: bundle.manifest.adapter_manifest_ids.clone(),
                    required_host_calls: bundle.manifest.required_host_calls.clone(),
                    adapter_manifests: bundle.adapter_manifests.clone(),
                })
                .expect("adapter requirements encode"),
            ),
            required_section(
                BundleSectionKind::ContentCatalog,
                encode_json(&ContentCatalogSection {
                    virtual_files: bundle.virtual_files.clone(),
                    image_assets: bundle.image_assets.clone(),
                    audio: bundle.audio.clone(),
                    image_objects: bundle.image_objects.clone(),
                })
                .expect("content catalog encode"),
            ),
            optional_section(
                BundleSectionKind::DisplayCatalog,
                encode_json(&bundle.display).expect("display catalog encode"),
            ),
            optional_section(
                BundleSectionKind::NormalizedSource,
                encode_json(&bundle.source).expect("source encode"),
            ),
        ];
        let bytes = encode_bundle(container_kind(bundle.bundle_kind), &manifest, sections)
            .expect("AWFB encodes");
        (
            bytes,
            ExternalSectionPayload::new(bytecode_id, bytecode_bytes),
        )
    }

    fn empty_bundle() -> ArcweftBundle {
        ArcweftBundle::new(
            BundleManifest {
                source_label: "main.arcw".to_owned(),
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
            BundleSource {
                label: "main.arcw".to_owned(),
                text: "flow @flow.main main { return \"ok\" }".to_owned(),
            },
            BytecodeProgram::default(),
            LineDisplayCatalog::default(),
        )
        .with_product_awbc(minimal_awbc_program())
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
