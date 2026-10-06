#!/usr/bin/env cargo +nightly -Zscript
---cargo
[dependencies]
arcweft-bundle = { path = "../crates/arcweft-bundle" }
arcweft-core = { path = "../crates/arcweft-core" }
arcweft-runtime-plan = { path = "../crates/arcweft-runtime-plan" }
arcweft-id = { path = "../crates/arcweft-id" }
arcweft-text-model = { path = "../crates/arcweft-text-model" }
arcweft-view = { path = "../crates/arcweft-view" }
arcweft-source = { path = "../crates/arcweft-source" }
# Current nightly rejects zune-core's disabled-log statement macro in an
# expression position. Enable the upstream logging macro through feature
# unification; the generator itself still emits no log output.
zune-jpeg = { version = "0.5.15", features = ["log"] }
---
use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_bundle::resource_codec::view::{
    CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, ViewElementKind,
    ViewInputKind, ViewInputOptions, ViewInputPurpose, ViewInputResource, ViewLayoutBoundsResource,
    ViewLogicalRect, ViewProgramInstruction, ViewProgramResource, ViewSecureInputPolicy,
    ViewSemanticTarget, ViewTextResource, ViewTextSelectionPolicy, ViewTextShortcutPolicy,
    ViewTextSourceKind, ViewTextSourceRecord, ViewTextTabPolicy, ViewTextVerticalNavigationPolicy,
};
use arcweft_bundle::resource_codec::view::{
    ViewDefinitionRef, ViewDefinitionResource, ViewHandlerRef, ViewInstructionSpan, ViewProgramId,
};
use arcweft_bundle::{ArcweftBundle, BundleFormat, BundleManifest, BundleRuntimeSummary};
use arcweft_core::awbc::schema::AwbcProgram;
use arcweft_core::effect::{LineEffectRequest, RuntimeCall};
use arcweft_core::entry::{
    EntryBindingIdentity, FlowContractHash, RuntimeEntryRoles, RuntimeFlowExecutable,
};
use arcweft_core::pattern::RuntimeCheckedType;
use arcweft_core::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeEffectSet, RuntimeEntryKind, RuntimeEntrySpec,
    RuntimeEntryTarget, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimeFlowSchema,
    RuntimeFlowSeed, RuntimeFunctionSemanticRole, RuntimeLineEffectSeed, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimePureProgramBindingSeed,
};
use arcweft_core::value::RuntimeValue;
use arcweft_id::runtime_program::RuntimePureProgramId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_text_model::DialogueContentCatalog;
use arcweft_view::{
    ViewHandlerResult, ViewHandlerResultRole, ViewHandlerTransitionValueRole, ViewId,
};
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
    let mut bundle = minimal_bundle();
    let mut text = bundle
        .view_text
        .take()
        .expect("standard View text inventory");
    text.sources.extend(view_text().sources);
    bundle
        .with_view_text(text)
        .with_view_input(view_input())
        .with_view_resources(Some(view_program()), None)
        .expect("authored View links with the standard View library")
}

fn minimal_bundle() -> ArcweftBundle {
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("web/ime-player-rendered.arcw").expect("source ID"),
        SourceName::path("web/ime-player-rendered.arcw"),
        include_str!("../web/ime-player-rendered.arcw"),
    )
    .expect("source document");
    let source_map = SourceMapSection::try_from_documents(&[&source]).expect("source map");

    let program = minimal_awbc_program();
    let instruction_count = program.instructions.len();
    ArcweftBundle::try_new(
        BundleManifest {
            profile_id: Some("sample.web_ime_player_rendered".to_owned()),
            profile_kind: None,
            entry: Some("entry.main".to_owned()),
            adapter: None,
            locale: Default::default(),
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                artifact_fingerprint: fixture_runtime_artifact_fingerprint(),
                entry_flow: Some("flow.web_ime_player_rendered".to_owned()),
                flows: 1,
                bytecode_instructions: instruction_count,
                line_task_groups: 0,
                stream_plans: 0,
            },
        },
        source_map,
        program,
        DialogueContentCatalog::new(),
    )
    .expect("standard dialogue source joins source map")
}

fn view_program() -> ViewProgramResource {
    let mut instructions = Vec::new();
    for (element, target, semantic, label) in [
        (
            ViewElementKind::TextField,
            "input.jp_text_field",
            "target.jp_text_field",
            "text.label.jp_text_field",
        ),
        (
            ViewElementKind::TextArea,
            "input.long_latin_area",
            "target.long_latin_area",
            "text.label.long_latin_area",
        ),
        (
            ViewElementKind::SecureField,
            "input.secret_secure_field",
            "target.secret_secure_field",
            "text.label.secret_secure_field",
        ),
    ] {
        instructions.extend([
            ViewProgramInstruction::OpenElement {
                element,
                target: Some(target.to_owned()),
                styles: Vec::new(),
                part: None,
                key: None,
                source: None,
            },
            ViewProgramInstruction::AttachSemantic {
                target: semantic.to_owned(),
                label_text_source: Some(label.to_owned()),
                source: None,
            },
            ViewProgramInstruction::CloseElement,
        ]);
    }
    let body = ViewInstructionSpan::new(
        0,
        u32::try_from(instructions.len()).expect("fixture body range"),
    );
    ViewProgramResource {
        program_id: ViewProgramId::try_new("view.program.web_ime_player_rendered")
            .expect("fixture program identity"),
        source_refs: Vec::new(),
        definitions: vec![ViewDefinitionResource {
            public_id: ViewDefinitionRef::new(
                ViewId::try_new("view.root.web_ime_player_rendered")
                    .expect("fixture View identity"),
            ),
            body,
            styles: Vec::new(),
            parameters: Vec::new(),
            parameter_contract: None,
            state_schema_hash: 1,
        }],
        value_programs: Vec::new(),
        value_inputs: Vec::new(),
        instructions,
        handlers: FixtureHandler::ALL
            .iter()
            .map(|handler| ViewHandlerRef {
                program: handler.program(),
                captures: Vec::new(),
                result: ViewHandlerResult::new(
                    ViewHandlerResultRole::StateTransition {
                        value: ViewHandlerTransitionValueRole::Unit,
                        writes: Box::new([]),
                    },
                    handler_result_type(),
                ),
            })
            .collect(),
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
        view: Some("view.root.web_ime_player_rendered".to_owned()),
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
        localized: Vec::new(),
        rich_text_documents: Vec::new(),
        display_frames: Vec::new(),
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
                Some(FixtureHandler::JapaneseChange),
                Some(FixtureHandler::JapaneseSubmit),
            ),
            input_option(
                "input.long_latin_area",
                ViewInputKind::TextArea,
                "text.value.long_latin_area",
                Some("text.placeholder.long_latin_area"),
                ViewInputPurpose::Text,
                ViewSecureInputPolicy::Plain,
                Some(FixtureHandler::MultilineChange),
                None,
            ),
            input_option(
                "input.secret_secure_field",
                ViewInputKind::SecureField,
                "text.value.secret_secure_field",
                Some("text.placeholder.secret_secure_field"),
                ViewInputPurpose::Password,
                ViewSecureInputPolicy::Password,
                Some(FixtureHandler::SecureChange),
                Some(FixtureHandler::SecureSubmit),
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
    change_handler: Option<FixtureHandler>,
    submit_handler: Option<FixtureHandler>,
) -> ViewInputOptions {
    ViewInputOptions {
        public_id: public_id.to_owned(),
        view: Some("view.root.web_ime_player_rendered".to_owned()),
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
        submit_handler: submit_handler.map(FixtureHandler::program),
        change_handler: change_handler.map(FixtureHandler::program),
        adapter_requirements: Vec::new(),
    }
}

// These domain-owned fixture programs acknowledge callbacks after the player
// writes the typed edit into its runtime overlay. They do not capture secrets.
#[derive(Clone, Copy)]
enum FixtureHandler {
    JapaneseChange,
    JapaneseSubmit,
    MultilineChange,
    SecureChange,
    SecureSubmit,
}
impl FixtureHandler {
    const ALL: &[Self] = &[
        Self::JapaneseChange,
        Self::JapaneseSubmit,
        Self::MultilineChange,
        Self::SecureChange,
        Self::SecureSubmit,
    ];
    const fn program(self) -> RuntimePureProgramId {
        let tag = match self {
            Self::JapaneseChange => 0x61,
            Self::JapaneseSubmit => 0x62,
            Self::MultilineChange => 0x63,
            Self::SecureChange => 0x64,
            Self::SecureSubmit => 0x65,
        };
        RuntimePureProgramId::from_checked_digest([tag; 32])
    }
}
fn handler_result_type() -> arcweft_id::RuntimeSemanticTypeId {
    RuntimeCheckedType::Tuple(vec![RuntimeCheckedType::Unit]).semantic_identity_digest()
}
fn minimal_awbc_program() -> AwbcProgram {
    let unit = RuntimeCheckedType::Unit.semantic_identity_digest();
    let result = handler_result_type();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    result,
                    RuntimePlanTypeProjection::Tuple(Box::new([unit])),
                ),
            ],
            [],
        )
        .expect("fixture callback types admit");
    for &handler in FixtureHandler::ALL {
        let body = RuntimeExprSeed::new(
            result,
            RuntimeExprSeedKind::Tuple(Box::new([RuntimeExprSeed::new(
                unit,
                RuntimeExprSeedKind::Value(RuntimeValue::Unit),
            )])),
        );
        let site = builder
            .push_function_site_seed(
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    handler.program().as_bytes(),
                ),
                RuntimeFunctionSemanticRole::Closure,
                [],
                body,
            )
            .expect("fixture callback admits");
        builder
            .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
                program: handler.program(),
                site,
            })
            .expect("fixture callback identity binds to its exact body");
    }
    let flow =
        FlowRuntimeId::from_checked_declaration_digest([0x31; 32], "flow.web_ime_player_rendered")
            .expect("fixture Flow identity");
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: Vec::new(),
        })
        .expect("fixture Flow schema");
    builder
        .push_flow_executable(RuntimeFlowExecutable {
            flow: flow.clone(),
            contract: FlowContractHash::from_bytes([0x32; 32]),
            controller: None,
        })
        .expect("fixture Flow metadata");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([61; 32]),
            flow.clone(),
            [],
            RuntimeEffectSet::try_from_effects([arcweft_id::EffectId::parse(
                "presentation.handle.create",
            )
            .expect("fixture mount capability")])
            .expect("fixture mount effect row"),
            vec![
                RuntimeFlowOpSeed::Effect(RuntimeLineEffectSeed::Static(LineEffectRequest::Call(
                    RuntimeCall {
                        callee: "presentation.handle.create".to_owned(),
                        args: vec![
                            "handle = @handle.web_ime_player_rendered".to_owned(),
                            "kind = \"view\"".to_owned(),
                            "resource = @view.root.web_ime_player_rendered".to_owned(),
                        ],
                    },
                ))),
                RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                    unit,
                    RuntimeExprSeedKind::Value(RuntimeValue::Unit),
                )),
            ],
        ))
        .expect("fixture Flow body");
    builder
        .push_entry(RuntimeEntrySpec {
            id: EntryRuntimeId::from_source_entity_body("entry.main").expect("fixture Entry"),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([0x33; 32]),
            target: RuntimeEntryTarget::Flow(flow),
            roles: RuntimeEntryRoles::None,
        })
        .expect("fixture CLI Entry");
    let plan = builder.finish().expect("fixture complete plan seals");
    arcweft_runtime_plan::awbc_lower::AwbcLowerer::new(
        &plan,
        &DialogueContentCatalog::new(),
        "web/ime-player-rendered.arcw",
    )
    .lower()
    .expect("fixture typed plan lowers to verified AWBC")
    .program
}
