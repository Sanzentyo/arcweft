use arcweft_bundle::resource_codec::view::{
    ViewObserveClassification, ViewProgramInstruction, ViewSecureRedactionMetadata,
    ViewTextSourceKind, ViewTextSourceRecord,
};
use arcweft_bundle::resource_codec::{
    ViewCallArgumentBindingRef, ViewDefinitionResource, ViewInstructionSpan, ViewParameterResource,
    ViewProgramResource, ViewTextResource, ViewValueInputNamespace, ViewValueInputResource,
    ViewValueInputSource,
};
use arcweft_core::value::{RuntimeBinding, RuntimeInt, RuntimeValue};
use arcweft_presentation::fx::{
    FxContextSlot, FxRuntimeType, FxRuntimeValue, ValueInstruction, ValueProgramSchema,
};
use arcweft_runtime_driver::presentation_handles::{
    PresentationHandleId, PresentationHandleKind, PresentationHandleRecord,
    PresentationResourceState,
};
use arcweft_runtime_driver::view_runtime::{
    BundleViewDiagnosticCode, BundleViewRuntime, BundleViewTextValue,
};
use arcweft_view::{ViewValueProgram, ViewValueProgramId};

fn handle(id: &str, view: &str) -> PresentationHandleRecord {
    PresentationHandleRecord::new(
        PresentationHandleId::try_new(id).unwrap(),
        PresentationHandleKind::View,
        view.to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    )
}

fn value_program(
    id: u32,
    parameter_types: Vec<FxRuntimeType>,
    state_types: Vec<FxRuntimeType>,
    return_type: FxRuntimeType,
    instructions: Vec<ValueInstruction>,
) -> ViewValueProgram {
    ViewValueProgram::validate(
        ViewValueProgramId(id),
        ValueProgramSchema::new(parameter_types, state_types, return_type),
        instructions,
    )
    .unwrap()
}

fn text_resource(
    records: impl IntoIterator<Item = (&'static str, ViewTextSourceKind)>,
) -> ViewTextResource {
    ViewTextResource {
        sources: records
            .into_iter()
            .map(|(public_id, kind)| ViewTextSourceRecord {
                public_id: public_id.to_owned(),
                kind,
                source: None,
            })
            .collect(),
        ..ViewTextResource::default()
    }
}

fn plain_text(frame: &arcweft_runtime_driver::view_runtime::BundleViewFrame) -> &str {
    let BundleViewTextValue::Plain { value } = &frame.mounts[0].text[0].value else {
        panic!("expected plain text")
    };
    value
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the complete typed IR fixture is kept beside all three frame assertions"
)]
fn branch_reacts_per_mount_and_missing_input_never_uses_placeholder() {
    let program = ViewProgramResource {
        program_id: "view.program.branch".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.Root".to_owned(),
            body: ViewInstructionSpan::new(0, 3),
            parameters: vec![ViewParameterResource {
                ordinal: 0,
                name: "active".to_owned(),
                value_type: Some(FxRuntimeType::Bool),
                value_slot: Some(0),
                default_program: None,
            }],
            state_schema_hash: 11,
        }],
        value_programs: vec![value_program(
            0,
            vec![FxRuntimeType::Bool],
            Vec::new(),
            FxRuntimeType::Bool,
            vec![
                ValueInstruction::LoadParameter {
                    slot: 0,
                    ty: FxRuntimeType::Bool,
                },
                ValueInstruction::Return,
            ],
        )],
        value_inputs: vec![ViewValueInputResource {
            namespace: ViewValueInputNamespace::Parameter,
            slot: 0,
            value_type: FxRuntimeType::Bool,
            source: ViewValueInputSource::DefinitionParameter {
                view: "view.Root".to_owned(),
                name: "active".to_owned(),
            },
        }],
        instructions: vec![
            ViewProgramInstruction::Branch {
                condition_program: ViewValueProgramId(0),
                then_span: 1,
                else_span: Some(1),
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.yes".to_owned(),
                style: None,
                part: None,
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.no".to_owned(),
                style: None,
                part: None,
                source: None,
            },
        ],
        ..ViewProgramResource::default()
    };
    let mut text = text_resource([
        (
            "text.yes",
            ViewTextSourceKind::Literal {
                value: "yes".to_owned(),
            },
        ),
        (
            "text.no",
            ViewTextSourceKind::Literal {
                value: "no".to_owned(),
            },
        ),
    ]);
    text.redactions.push(ViewSecureRedactionMetadata {
        text_source: "text.yes".to_owned(),
        classification: ViewObserveClassification::AgentMasked,
        replacement: Some("masked".to_owned()),
    });
    let mounted = handle("handle.root", "view.Root");

    let mut missing =
        BundleViewRuntime::try_new(Some(program.clone()), Some(text.clone())).unwrap();
    let frame = missing.evaluate(std::slice::from_ref(&mounted), &[], false);
    assert!(frame.mounts.is_empty());
    assert_eq!(
        frame.diagnostics[0].code,
        BundleViewDiagnosticCode::MissingInput
    );

    let mut runtime = BundleViewRuntime::try_new(Some(program), Some(text)).unwrap();
    let active = runtime.evaluate(
        std::slice::from_ref(&mounted),
        &[RuntimeBinding {
            name: "active".to_owned(),
            value: RuntimeValue::Bool(true),
        }],
        false,
    );
    assert!(active.diagnostics.is_empty());
    assert_eq!(plain_text(&active), "yes");
    assert_eq!(plain_text(&active.redacted_for_observation()), "masked");
    let mount_id = active.mounts[0].mount;

    let inactive = runtime.evaluate(
        std::slice::from_ref(&mounted),
        &[RuntimeBinding {
            name: "active".to_owned(),
            value: RuntimeValue::Bool(false),
        }],
        false,
    );
    assert!(inactive.diagnostics.is_empty());
    assert_eq!(plain_text(&inactive), "no");
    assert_eq!(inactive.mounts[0].mount, mount_id);

    let retained = runtime.evaluate(std::slice::from_ref(&mounted), &[], false);
    assert_eq!(plain_text(&retained), "no");
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the parent/child IR fixture and exact restore assertions describe one persistence scenario"
)]
fn nested_mounts_round_trip_exactly_and_allocator_stays_fresh() {
    let common_parameters = vec![FxRuntimeType::I32];
    let constant = value_program(
        0,
        common_parameters.clone(),
        Vec::new(),
        FxRuntimeType::I32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::I32(5),
            },
            ValueInstruction::Return,
        ],
    );
    let program = ViewProgramResource {
        program_id: "view.program.nested-runtime".to_owned(),
        definitions: vec![
            ViewDefinitionResource {
                public_id: "view.Parent".to_owned(),
                body: ViewInstructionSpan::new(0, 1),
                parameters: Vec::new(),
                state_schema_hash: 21,
            },
            ViewDefinitionResource {
                public_id: "view.Child".to_owned(),
                body: ViewInstructionSpan::new(1, 2),
                parameters: vec![ViewParameterResource {
                    ordinal: 0,
                    name: "count".to_owned(),
                    value_type: Some(FxRuntimeType::I32),
                    value_slot: Some(0),
                    default_program: None,
                }],
                state_schema_hash: 22,
            },
        ],
        value_programs: vec![constant],
        value_inputs: vec![ViewValueInputResource {
            namespace: ViewValueInputNamespace::Parameter,
            slot: 0,
            value_type: FxRuntimeType::I32,
            source: ViewValueInputSource::DefinitionParameter {
                view: "view.Child".to_owned(),
                name: "count".to_owned(),
            },
        }],
        instructions: vec![
            ViewProgramInstruction::CallView {
                view: "view.Child".to_owned(),
                arguments: vec![ViewCallArgumentBindingRef {
                    ordinal: 0,
                    name: Some("count".to_owned()),
                    value_program: ViewValueProgramId(0),
                }],
                style: None,
                part: None,
                key: None,
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.child.count".to_owned(),
                style: None,
                part: None,
                source: None,
            },
        ],
        ..ViewProgramResource::default()
    };
    let text = text_resource([(
        "text.child.count",
        ViewTextSourceKind::Projection {
            path: vec!["count".to_owned()],
        },
    )]);
    let first_handle = handle("handle.first", "view.Parent");
    let mut runtime =
        BundleViewRuntime::try_new(Some(program.clone()), Some(text.clone())).unwrap();
    runtime.advance_millis(1_250).unwrap();
    let first = runtime.evaluate(std::slice::from_ref(&first_handle), &[], false);
    assert!(first.diagnostics.is_empty());
    assert_eq!(first.mounts.len(), 2);
    assert_eq!(first.mounts[1].view, "view.Child");
    assert_eq!(
        first.mounts[1].text[0].value,
        BundleViewTextValue::Plain {
            value: "5".to_owned()
        }
    );

    let snapshot = runtime.snapshot();
    let mut restored = BundleViewRuntime::try_new(Some(program), Some(text)).unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    let after_restore = restored.evaluate(std::slice::from_ref(&first_handle), &[], false);
    assert_eq!(
        after_restore
            .mounts
            .iter()
            .map(|mount| mount.mount)
            .collect::<Vec<_>>(),
        first
            .mounts
            .iter()
            .map(|mount| mount.mount)
            .collect::<Vec<_>>()
    );

    let second_handle = handle("handle.second", "view.Parent");
    let both = restored.evaluate(&[first_handle, second_handle], &[], false);
    assert!(both.diagnostics.is_empty());
    assert_eq!(both.mounts.len(), 4);
    let unique = both
        .mounts
        .iter()
        .map(|mount| mount.mount)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), 4);
    assert!(
        both.mounts
            .iter()
            .map(|mount| mount.mount.get())
            .max()
            .unwrap()
            >= 3
    );
}

#[test]
fn duplicate_repeat_keys_fail_structurally_instead_of_reusing_one_child() {
    let source = value_program(
        0,
        Vec::new(),
        Vec::new(),
        FxRuntimeType::I32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::I32(2),
            },
            ValueInstruction::Return,
        ],
    );
    let duplicate_key = value_program(
        1,
        Vec::new(),
        Vec::new(),
        FxRuntimeType::I32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::I32(7),
            },
            ValueInstruction::Return,
        ],
    );
    let program = ViewProgramResource {
        program_id: "view.program.repeat".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.Repeat".to_owned(),
            body: ViewInstructionSpan::new(0, 2),
            parameters: Vec::new(),
            state_schema_hash: 31,
        }],
        value_programs: vec![source, duplicate_key],
        instructions: vec![
            ViewProgramInstruction::RepeatKeyed {
                source_program: ViewValueProgramId(0),
                key_program: ViewValueProgramId(1),
                body_span: 1,
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.item".to_owned(),
                style: None,
                part: None,
                source: None,
            },
        ],
        ..ViewProgramResource::default()
    };
    let text = text_resource([(
        "text.item",
        ViewTextSourceKind::Literal {
            value: "item".to_owned(),
        },
    )]);
    let mut runtime = BundleViewRuntime::try_new(Some(program), Some(text)).unwrap();
    let frame = runtime.evaluate(&[handle("handle.repeat", "view.Repeat")], &[], false);
    assert!(frame.mounts.is_empty());
    assert_eq!(
        frame.diagnostics[0].code,
        BundleViewDiagnosticCode::DuplicateRepeatKey
    );
}

#[test]
fn logical_time_updates_context_cache_and_reduce_motion_freezes_it() {
    let program = ViewProgramResource {
        program_id: "view.program.time".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.Time".to_owned(),
            body: ViewInstructionSpan::new(0, 2),
            parameters: Vec::new(),
            state_schema_hash: 41,
        }],
        value_programs: vec![value_program(
            0,
            Vec::new(),
            vec![FxRuntimeType::F32],
            FxRuntimeType::F32,
            vec![
                ValueInstruction::LoadContext {
                    slot: FxContextSlot::Time,
                },
                ValueInstruction::Return,
            ],
        )],
        value_inputs: vec![ViewValueInputResource {
            namespace: ViewValueInputNamespace::State,
            slot: 0,
            value_type: FxRuntimeType::F32,
            source: ViewValueInputSource::Local {
                view: "view.Time".to_owned(),
                name: "elapsed".to_owned(),
            },
        }],
        instructions: vec![
            ViewProgramInstruction::BindLocal {
                binding: "elapsed".to_owned(),
                value_program: ViewValueProgramId(0),
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.elapsed".to_owned(),
                style: None,
                part: None,
                source: None,
            },
        ],
        ..ViewProgramResource::default()
    };
    let text = text_resource([(
        "text.elapsed",
        ViewTextSourceKind::Local {
            name: "elapsed".to_owned(),
        },
    )]);
    let mounted = handle("handle.time", "view.Time");
    let mut runtime = BundleViewRuntime::try_new(Some(program), Some(text)).unwrap();
    let initial = runtime.evaluate(std::slice::from_ref(&mounted), &[], false);
    assert_eq!(plain_text(&initial), "0");
    runtime.advance_millis(1_000).unwrap();
    let advanced = runtime.evaluate(std::slice::from_ref(&mounted), &[], false);
    assert_eq!(plain_text(&advanced), "1");
    let reduced = runtime.evaluate(std::slice::from_ref(&mounted), &[], true);
    assert_eq!(plain_text(&reduced), "0");
}

#[test]
fn exact_i32_width_is_enforced_at_the_runtime_boundary() {
    let program = ViewProgramResource {
        program_id: "view.program.i32".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.Exact".to_owned(),
            body: ViewInstructionSpan::new(0, 0),
            parameters: vec![ViewParameterResource {
                ordinal: 0,
                name: "count".to_owned(),
                value_type: Some(FxRuntimeType::I32),
                value_slot: Some(0),
                default_program: None,
            }],
            state_schema_hash: 51,
        }],
        value_inputs: vec![ViewValueInputResource {
            namespace: ViewValueInputNamespace::Parameter,
            slot: 0,
            value_type: FxRuntimeType::I32,
            source: ViewValueInputSource::DefinitionParameter {
                view: "view.Exact".to_owned(),
                name: "count".to_owned(),
            },
        }],
        value_programs: vec![value_program(
            0,
            vec![FxRuntimeType::I32],
            Vec::new(),
            FxRuntimeType::I32,
            vec![
                ValueInstruction::LoadParameter {
                    slot: 0,
                    ty: FxRuntimeType::I32,
                },
                ValueInstruction::Return,
            ],
        )],
        ..ViewProgramResource::default()
    };
    let mut runtime = BundleViewRuntime::try_new(Some(program), None).unwrap();
    let frame = runtime.evaluate(
        &[handle("handle.exact", "view.Exact")],
        &[RuntimeBinding {
            name: "count".to_owned(),
            value: RuntimeValue::Int(RuntimeInt::i64(4)),
        }],
        false,
    );
    assert!(frame.mounts.is_empty());
    assert_eq!(
        frame.diagnostics[0].code,
        BundleViewDiagnosticCode::InputType
    );
}
