use arcweft_audio_core::graph::{
    AudioAsset, AudioBusDef, AudioDecodeStrategy, AudioFormat, AudioGraph,
};
use arcweft_bundle::container::{BundleSectionKind, BundleView, ReadBudget};
use arcweft_bundle::resource_codec::{
    CompactAssetCatalogSection, CompactAudioGraphSection, CompactDisplayCatalogSection,
    CompactSourceMapSection, FieldId, ProductResourceEnvelope, ProductSectionCodecKind,
    ResourceField, ResourceWireType, SectionCodecBudget,
};
use arcweft_bundle::{
    ArcweftBundle, BundleFormat, BundleImageAnimation, BundleImageAsset, BundleImageDimensions,
    BundleImageObject, BundleImageObjectAlignment, BundleImageObjectBounds, BundleImageObjectFit,
    BundleImageObjectPlayback, BundleImageObjectTransform, BundleManifest, BundleRuntimeSummary,
    BundleSource, BundleVirtualFile, BundleVirtualFileRef, BundleVirtualFileSpace,
};
use arcweft_core::awbc::schema::{
    AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
    AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId,
    AwbcFunctionKind, AwbcProgram, AwbcSafePointKind, AwbcSignature, AwbcSignatureId, AwbcStringId,
    AwbcTableRange, AwbcTerminator,
};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_interaction_model::audio::{AudioBusId, AudioLoopMode, AudioResourceId, GainDbMilli};
use arcweft_render_text::LineDisplayCatalog;

#[test]
fn product_catalog_compact_codecs_round_trip_current_bundle_resources() {
    let bundle = fixture_bundle();

    let asset = CompactAssetCatalogSection::from_bundle(&bundle);
    let asset_bytes = asset
        .encode_canonical_section()
        .expect("asset catalog encodes");
    assert_eq!(
        asset_bytes[..8],
        ProductSectionCodecKind::AssetCatalog.magic()
    );
    assert_eq!(
        CompactAssetCatalogSection::decode_canonical_section(&asset_bytes)
            .expect("asset catalog decodes"),
        asset
    );

    let display = CompactDisplayCatalogSection::from_bundle(&bundle);
    let display_bytes = display
        .encode_canonical_section()
        .expect("display catalog encodes");
    assert_eq!(
        display_bytes[..8],
        ProductSectionCodecKind::DisplayCatalog.magic()
    );
    assert_eq!(
        CompactDisplayCatalogSection::decode_canonical_section(&display_bytes)
            .expect("display catalog decodes"),
        display
    );

    let source = CompactSourceMapSection::from_bundle(&bundle);
    let source_bytes = source.encode_canonical_section().expect("source encodes");
    assert_eq!(
        source_bytes[..8],
        ProductSectionCodecKind::SourceMap.magic()
    );
    assert_eq!(
        CompactSourceMapSection::decode_canonical_section(&source_bytes).expect("source decodes"),
        source
    );

    let audio = CompactAudioGraphSection::from_graph(bundle.audio.clone().expect("audio graph"));
    let audio_bytes = audio.encode_canonical_section().expect("audio encodes");
    assert_eq!(
        audio_bytes[..8],
        ProductSectionCodecKind::AudioGraph.magic()
    );
    assert_eq!(
        CompactAudioGraphSection::decode_canonical_section(&audio_bytes).expect("audio decodes"),
        audio
    );
}

#[test]
fn product_awfb_uses_compact_sections_for_migrated_catalog_families() {
    let bundle = fixture_bundle();
    let bytes = bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("AWFB encodes");
    let view = BundleView::parse(&bytes, ReadBudget::default()).expect("AWFB parses");

    assert_section_magic(
        &view,
        BundleSectionKind::ContentCatalog,
        ProductSectionCodecKind::ContentCatalog.magic(),
    );
    assert_section_magic(
        &view,
        BundleSectionKind::AssetCatalog,
        ProductSectionCodecKind::AssetCatalog.magic(),
    );
    assert_section_magic(
        &view,
        BundleSectionKind::DisplayCatalog,
        ProductSectionCodecKind::DisplayCatalog.magic(),
    );
    assert_section_magic(
        &view,
        BundleSectionKind::SourceMap,
        ProductSectionCodecKind::SourceMap.magic(),
    );
    assert_section_magic(
        &view,
        BundleSectionKind::AudioGraph,
        ProductSectionCodecKind::AudioGraph.magic(),
    );
    assert!(
        view.sections()
            .iter()
            .all(|section| section.known_kind() != Some(BundleSectionKind::NormalizedSource)),
        "product AWFB must not keep legacy NormalizedSource JSON after SourceMap migration"
    );

    let decoded = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &bytes)
        .expect("compact product AWFB decodes");
    assert_eq!(decoded.virtual_files, bundle.virtual_files);
    assert_eq!(decoded.image_assets, bundle.image_assets);
    assert_eq!(decoded.image_objects, bundle.image_objects);
    assert_eq!(decoded.source, bundle.source);
    assert_eq!(decoded.audio, bundle.audio);
}

#[test]
fn product_catalog_unknown_optional_fields_skip_and_unknown_required_reject() {
    let source = CompactSourceMapSection::from_bundle(&fixture_bundle());
    let bytes = source.encode_canonical_section().expect("source encodes");
    let envelope = ProductResourceEnvelope::decode_all_fields(
        &bytes,
        ProductSectionCodecKind::SourceMap,
        SectionCodecBudget::default(),
    )
    .expect("source envelope decodes");

    let optional_bytes = envelope_with_extra_field(
        &envelope,
        ResourceField::optional(FieldId(30_000), ResourceWireType::Bytes, b"future"),
    );
    assert_eq!(
        CompactSourceMapSection::decode_canonical_section(&optional_bytes)
            .expect("unknown optional field skips"),
        source
    );

    let required_bytes = envelope_with_extra_field(
        &envelope,
        ResourceField::required(FieldId(30_001), ResourceWireType::Bytes, b"future"),
    );
    assert!(
        CompactSourceMapSection::decode_canonical_section(&required_bytes).is_err(),
        "unknown required field must reject"
    );
}

#[test]
fn product_catalog_common_budget_failures_are_reported() {
    let source = CompactSourceMapSection::from_bundle(&fixture_bundle());
    let bytes = source.encode_canonical_section().expect("source encodes");
    let tiny_budget = SectionCodecBudget {
        bytes: 1,
        ..SectionCodecBudget::default()
    };

    assert!(
        ProductResourceEnvelope::decode_all_fields(
            &bytes,
            ProductSectionCodecKind::SourceMap,
            tiny_budget,
        )
        .is_err(),
        "common section budget must be enforced for product catalog sections"
    );
}

fn assert_section_magic(view: &BundleView<'_>, kind: BundleSectionKind, magic: [u8; 8]) {
    let descriptor = view
        .sections()
        .iter()
        .find(|section| section.known_kind() == Some(kind))
        .unwrap_or_else(|| panic!("{kind:?} section exists"));
    let bytes = view
        .decoded_section(descriptor.id())
        .expect("section decodes")
        .expect("section is embedded");
    assert_ne!(bytes.first(), Some(&b'{'), "{kind:?} must not be JSON");
    assert_eq!(bytes[..8], magic, "{kind:?} compact magic");
}

fn envelope_with_extra_field(envelope: &ProductResourceEnvelope, field: ResourceField) -> Vec<u8> {
    let mut fields = envelope.fields.clone();
    fields.push(field);
    ProductResourceEnvelope::new(
        envelope.header.codec,
        envelope.strings.clone(),
        envelope.public_ids.clone(),
        envelope.enums.clone(),
        fields,
        envelope.header.record_count,
    )
    .expect("envelope rebuilds")
    .encode_canonical()
    .expect("envelope re-encodes")
}

fn fixture_bundle() -> ArcweftBundle {
    let image_file = BundleVirtualFile {
        space: BundleVirtualFileSpace::Asset,
        path: "images/logo.webp".to_owned(),
        bytes: b"webp-bytes".to_vec(),
    };
    let audio_file = BundleVirtualFile {
        space: BundleVirtualFileSpace::Asset,
        path: "audio/opening.wav".to_owned(),
        bytes: b"wav-bytes".to_vec(),
    };
    let master_bus = AudioBusId::new("bus.master").expect("bus id");
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
    .with_virtual_files([image_file, audio_file])
    .with_image_assets([BundleImageAsset {
        id: "asset.ui.logo".to_owned(),
        file: BundleVirtualFileRef {
            space: BundleVirtualFileSpace::Asset,
            path: "images/logo.webp".to_owned(),
        },
        format: arcweft_bundle::BundleImageFormat::WebP,
        animation: BundleImageAnimation::Animated,
        dimensions: Some(BundleImageDimensions::new(320, 180)),
    }])
    .with_image_objects([BundleImageObject {
        id: "image.hero.logo".to_owned(),
        asset: "asset.ui.logo".to_owned(),
        target: Some("target.hero.logo".to_owned()),
        layer: Some("layer.foreground".to_owned()),
        bounds: BundleImageObjectBounds::from_px(10, 20, 320, 180),
        fit: BundleImageObjectFit::Cover,
        alignment: BundleImageObjectAlignment {
            x_milli: 250,
            y_milli: 750,
        },
        playback: BundleImageObjectPlayback {
            start_time_millis: 40,
            rate_milli: 500,
            paused_at_millis: None,
            pinned_local_time_millis: Some(160),
        },
        transform: BundleImageObjectTransform {
            m11_milli: 1_000,
            m12_milli: 0,
            m21_milli: 0,
            m22_milli: 1_000,
            tx_milli: 12_000,
            ty_milli: -3_000,
        },
        depth_milli: 2_400,
        opacity_milli: 900,
        visible: true,
    }])
    .with_audio_graph(AudioGraph {
        master_bus: master_bus.clone(),
        assets: vec![AudioAsset {
            id: AudioResourceId::new("asset.voice.opening").expect("audio asset id"),
            path: "audio/opening.wav".to_owned(),
            format: AudioFormat::Wav,
            strategy: AudioDecodeStrategy::Preload,
            default_loop: AudioLoopMode::None,
        }],
        buses: vec![AudioBusDef {
            id: master_bus,
            parent: None,
            gain: GainDbMilli::UNITY,
            muted: false,
            effects: Vec::new(),
        }],
        snapshots: Vec::new(),
    })
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
