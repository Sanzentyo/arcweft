#!/usr/bin/env cargo +nightly -Zscript
---cargo
[dependencies]
arcweft-bundle = { path = "../crates/arcweft-bundle" }
arcweft-core = { path = "../crates/arcweft-core" }
arcweft-render-text = { path = "../crates/arcweft-render-text" }
arcweft-source = { path = "../crates/arcweft-source" }
# Current nightly rejects zune-core's disabled-log statement macro in an
# expression position. Enable the upstream logging macro through feature
# unification; the generator itself still emits no log output.
zune-jpeg = { version = "0.5.15", features = ["log"] }
---

use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_bundle::resource_codec::view::{
    CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, ViewInputKind,
    ViewInputOptions, ViewInputPurpose, ViewInputResource, ViewLayoutBoundsResource,
    ViewLogicalRect, ViewProgramResource, ViewSecureInputPolicy, ViewSemanticTarget,
    ViewTextResource, ViewTextSelectionPolicy, ViewTextShortcutPolicy, ViewTextSourceKind,
    ViewTextSourceRecord, ViewTextTabPolicy, ViewTextVerticalNavigationPolicy,
};
use arcweft_bundle::{ArcweftBundle, BundleFormat, BundleManifest, BundleRuntimeSummary};
use arcweft_core::awbc::schema::{
    AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
    AwbcFlowBinding, AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags,
    AwbcFunctionId, AwbcFunctionKind, AwbcProgram, AwbcSafePointKind, AwbcSignature,
    AwbcSignatureId, AwbcStringId, AwbcTableRange, AwbcTerminator,
};
use arcweft_render_text::LineDisplayCatalog;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::env;
use std::fs;
use std::path::PathBuf;

fn fixture_runtime_artifact_fingerprint() -> arcweft_core::effect::RuntimeArtifactFingerprint {
    arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([0x6a; 32])
        .expect("fixture runtime artifact fingerprint is non-zero")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = output_path()?;
    let bundle = web_ime_player_rendered_bundle();
    let bytes = bundle.to_format_bytes(BundleFormat::Awfb)?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&out, bytes)?;
    println!("wrote {}", out.display());
    Ok(())
}

fn output_path() -> Result<PathBuf, String> {
    let mut args = env::args().skip(1);
    let mut out = PathBuf::from("web/ime-player-rendered.awfb");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" | "-o" => {
                let Some(value) = args.next() else {
                    return Err("--out requires a path".to_owned());
                };
                out = PathBuf::from(value);
            }
            "--help" | "-h" => {
                println!(
                    "usage: cargo +nightly -Zscript tools/build-web-ime-player-rendered-fixture.rs --out web/ime-player-rendered.awfb"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(out)
}

fn web_ime_player_rendered_bundle() -> ArcweftBundle {
    minimal_bundle()
        .with_view_text(view_text())
        .with_view_input(view_input())
        .with_view_program(view_program())
}

fn minimal_bundle() -> ArcweftBundle {
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("web/ime-player-rendered.arcw").expect("source ID"),
        SourceName::path("web/ime-player-rendered.arcw"),
        include_str!("../web/ime-player-rendered.arcw"),
    )
    .expect("source document");
    let source_map = SourceMapSection::try_from_documents(&[&source]).expect("source map");

    ArcweftBundle::try_new(
        BundleManifest {
            profile_id: Some("sample.web_ime_player_rendered".to_owned()),
            profile_kind: None,
            entry: Some("entry.main".to_owned()),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                artifact_fingerprint: fixture_runtime_artifact_fingerprint(),
                entry_flow: Some("entry.main".to_owned()),
                flows: 1,
                bytecode_instructions: 0,
                line_task_groups: 0,
                stream_plans: 0,
            },
        },
        source_map,
        minimal_awbc_program(),
        LineDisplayCatalog::default(),
    )
    .expect("standard dialogue source joins source map")
}

fn view_program() -> ViewProgramResource {
    ViewProgramResource {
        program_id: "view.program.web_ime_player_rendered".to_owned(),
        root_view: "view.root.web_ime_player_rendered".to_owned(),
        instructions: Vec::new(),
        child_spans: Vec::new(),
        handlers: Vec::new(),
        state_schema_hashes: Vec::new(),
        exported_parts: Vec::new(),
        semantic_targets: vec![
            semantic(
                "target.jp_text_field",
                "input.jp_text_field",
                "text.label.jp_text_field",
            ),
            semantic(
                "target.long_latin_area",
                "input.long_latin_area",
                "text.label.long_latin_area",
            ),
            semantic(
                "target.secret_secure_field",
                "input.secret_secure_field",
                "text.label.secret_secure_field",
            ),
        ],
        layout_bounds: vec![
            text_control_layout("input.jp_text_field", 48, 48, 420, 48),
            semantic_layout("target.jp_text_field", 48, 48, 420, 48),
            text_control_layout("input.long_latin_area", 48, 112, 420, 136),
            semantic_layout("target.long_latin_area", 48, 112, 420, 136),
            text_control_layout("input.secret_secure_field", 48, 264, 420, 48),
            semantic_layout("target.secret_secure_field", 48, 264, 420, 48),
        ],
        scroll_regions: Vec::new(),
        surfaces: Vec::new(),
        text_blocks: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        adapter_requirements: Vec::new(),
    }
}

fn text_control_layout(
    public_id: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> ViewLayoutBoundsResource {
    ViewLayoutBoundsResource::text_control(public_id, ViewLogicalRect::from_px(x, y, width, height))
}

fn semantic_layout(
    public_id: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> ViewLayoutBoundsResource {
    ViewLayoutBoundsResource::semantic_target(
        public_id,
        ViewLogicalRect::from_px(x, y, width, height),
    )
}

fn semantic(public_id: &str, target: &str, label_text_source: &str) -> ViewSemanticTarget {
    ViewSemanticTarget {
        public_id: public_id.to_owned(),
        target: target.to_owned(),
        view: None,
        label_text_source: Some(label_text_source.to_owned()),
        source: None,
    }
}

fn view_text() -> ViewTextResource {
    ViewTextResource {
        sources: vec![
            literal("text.value.jp_text_field", "かな入力 sample"),
            literal("text.placeholder.jp_text_field", "ここに日本語 IME で入力"),
            literal("text.label.jp_text_field", "Japanese TextField"),
            literal(
                "text.value.long_latin_area",
                "Long Latin text wraps through the renderer; 日本語の語句も同じ Arcweft frameで表示する。",
            ),
            literal(
                "text.placeholder.long_latin_area",
                "Long text and Japanese text",
            ),
            literal("text.label.long_latin_area", "Long TextArea"),
            literal("text.value.secret_secure_field", "arcweft-secret-1234"),
            literal("text.placeholder.secret_secure_field", "secret"),
            literal("text.label.secret_secure_field", "SecureField"),
        ],
        display_frame_refs: Vec::new(),
        source_ranges: Vec::new(),
        reveal_policies: Vec::new(),
        cursor_policies: Vec::new(),
        redactions: Vec::new(),
    }
}

fn literal(public_id: &str, value: &str) -> ViewTextSourceRecord {
    ViewTextSourceRecord {
        public_id: public_id.to_owned(),
        kind: ViewTextSourceKind::Literal {
            value: value.to_owned(),
        },
        source: None,
    }
}

fn view_input() -> ViewInputResource {
    ViewInputResource {
        options: vec![
            input_option(
                "input.jp_text_field",
                ViewInputKind::TextField,
                "text.value.jp_text_field",
                Some("text.placeholder.jp_text_field"),
                ViewInputPurpose::Text,
                ViewSecureInputPolicy::Plain,
                Some("handler.jp_text_field.change"),
                Some("handler.jp_text_field.submit"),
            ),
            input_option(
                "input.long_latin_area",
                ViewInputKind::TextArea,
                "text.value.long_latin_area",
                Some("text.placeholder.long_latin_area"),
                ViewInputPurpose::Text,
                ViewSecureInputPolicy::Plain,
                Some("handler.long_latin_area.change"),
                None,
            ),
            input_option(
                "input.secret_secure_field",
                ViewInputKind::SecureField,
                "text.value.secret_secure_field",
                Some("text.placeholder.secret_secure_field"),
                ViewInputPurpose::Password,
                ViewSecureInputPolicy::Password,
                Some("handler.secret_secure_field.change"),
                Some("handler.secret_secure_field.submit"),
            ),
        ],
        adapter_requirements: Vec::new(),
    }
}

fn input_option(
    public_id: &str,
    kind: ViewInputKind,
    value_text_source: &str,
    placeholder_text_source: Option<&str>,
    purpose: ViewInputPurpose,
    secure_policy: ViewSecureInputPolicy,
    change_handler: Option<&str>,
    submit_handler: Option<&str>,
) -> ViewInputOptions {
    ViewInputOptions {
        public_id: public_id.to_owned(),
        view: None,
        containing_scroll_region: None,
        kind,
        value_text_source: value_text_source.to_owned(),
        placeholder_text_source: placeholder_text_source.map(ToOwned::to_owned),
        purpose,
        autocorrect: TextAssistPolicy::PlatformDefault,
        spellcheck: TextAssistPolicy::PlatformDefault,
        capitalization: TextCapitalization::None,
        enter_key: if kind.is_multiline() {
            EnterKeyHint::Enter
        } else {
            EnterKeyHint::Done
        },
        multiline: kind.is_multiline(),
        selection_policy: ViewTextSelectionPolicy::Enabled,
        shortcut_policy: ViewTextShortcutPolicy::Enabled,
        tab_policy: ViewTextTabPolicy::FocusNavigation,
        vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
        secure_policy,
        composition_on_blur: CompositionOnBlurPolicy::Commit,
        submit_handler: submit_handler.map(ToOwned::to_owned),
        change_handler: change_handler.map(ToOwned::to_owned),
        adapter_requirements: Vec::new(),
    }
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
        flow_bindings: vec![AwbcFlowBinding {
            flow: arcweft_core::plan::FlowRuntimeId::from_checked_declaration_digest(
                [0x31; 32],
                "flow.main",
            )
            .expect("fixture checked Flow identity"),
            function: AwbcFunctionId(0),
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
