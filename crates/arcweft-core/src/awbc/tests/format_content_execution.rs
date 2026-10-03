use super::*;

#[derive(Clone, Copy)]
enum ValueThunk {
    Succeeds,
    RecoversFromDivisionByZero,
    Traps,
}

#[derive(Clone, Copy)]
enum FlowAttemptHelperOutcome {
    Succeeds,
    RecoversFromDivisionByZero,
    Traps,
}

fn format_program(value_thunk: ValueThunk) -> AwbcProgram {
    let mut program = super::minimal_program();
    let content_owner = crate::value::RuntimeDialogueOpaqueRole::Content.exact_owner();
    let producer = content_owner.producer().as_str().to_owned();
    program
        .strings
        .extend([producer.clone(), "number".to_owned()]);
    program.canonicalize_string_table();
    let producer = AwbcStringId(
        u32::try_from(
            program
                .strings
                .binary_search(&producer)
                .expect("Content producer string"),
        )
        .expect("Content producer string index"),
    );
    let style = AwbcStringId(
        u32::try_from(
            program
                .strings
                .binary_search(&"number".to_owned())
                .expect("Style string"),
        )
        .expect("Style string index"),
    );

    let content_type = AwbcTypeId(0);
    let int_type = AwbcTypeId(1);
    let string_type = AwbcTypeId(2);
    program.runtime_types = vec![
        AwbcRuntimeType::new(
            content_owner.semantic_identity(),
            AwbcRuntimeTypeShape::Opaque {
                producer,
                admission: content_owner.admission(),
                value_class: content_owner.value_class(),
                persistence: content_owner.persistence(),
                arguments: Vec::new(),
            },
        ),
        AwbcRuntimeType::new(
            RuntimeCheckedType::Signed(crate::value::RuntimeSignedIntWidth::I64)
                .semantic_identity_digest(),
            AwbcRuntimeTypeShape::Int(AwbcSignedIntKind::I64),
        ),
        AwbcRuntimeType::new(
            RuntimeCheckedType::String.semantic_identity_digest(),
            AwbcRuntimeTypeShape::String,
        ),
    ];

    program.signatures[0].result = Some(content_type);
    program.frame_layouts[0].slots = vec![
        AwbcFrameSlot {
            name: None,
            ty: content_type,
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        },
        AwbcFrameSlot {
            name: None,
            ty: int_type,
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        },
    ];
    program.content_templates = vec![AwbcDialogueContentTemplate {
        id: crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
            .expect("template ID"),
        digest: crate::entry::RuntimeDialogueContentTemplateDigest::from_bytes([0x19; 32]),
        slots: vec![AwbcDialogueContentSlot {
            slot: crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0)
                .expect("slot ID"),
            role: AwbcDialogueValueRole::Formatted,
            semantic_type: content_type,
        }],
        effects: Vec::new(),
    }];

    let int_constant = |value: i64| {
        let mut bits = [0; 16];
        bits[..8].copy_from_slice(&value.to_le_bytes());
        AwbcConstant::Int {
            kind: AwbcSignedIntKind::I64,
            bits,
        }
    };
    let denominator = match value_thunk {
        ValueThunk::RecoversFromDivisionByZero => 0,
        ValueThunk::Succeeds | ValueThunk::Traps => 1,
    };
    program.constants = vec![
        int_constant(12345),
        int_constant(denominator),
        AwbcConstant::String(style),
    ];
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::FormatContent {
            destination: AwbcRegisterId(0),
            template: crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
                .expect("template ID"),
            attempt: None,
            attempt_operands: Vec::new(),
            project_method: None,
            project_option: false,
            project_result: None,
            operands: vec![
                AwbcFormatOperand {
                    parameter: crate::value::RuntimeFmtParameterId::Value,
                    function: AwbcFunctionId(1),
                    captures: vec![AwbcRegisterId(1)],
                },
                AwbcFormatOperand {
                    parameter: crate::value::RuntimeFmtParameterId::Style,
                    function: AwbcFunctionId(2),
                    captures: Vec::new(),
                },
            ],
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(1),
        },
        AwbcInstruction::Binary {
            dst: AwbcRegisterId(2),
            op: AwbcBinaryOp::Div,
            lhs: AwbcRegisterId(0),
            rhs: AwbcRegisterId(1),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(2),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 2);
    program.blocks[0].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(0)),
    };
    program.signatures.extend([
        AwbcSignature {
            params: vec![int_type],
            result: Some(int_type),
            effects: AwbcEffectSetId(0),
        },
        AwbcSignature {
            params: Vec::new(),
            result: Some(string_type),
            effects: AwbcEffectSetId(0),
        },
    ]);
    program.frame_layouts.extend([
        AwbcFrameLayout {
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: int_type,
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: int_type,
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: int_type,
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
            ],
            scopes: Vec::new(),
            max_scope_depth: 0,
        },
        AwbcFrameLayout {
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: int_type,
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: string_type,
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
            ],
            scopes: Vec::new(),
            max_scope_depth: 0,
        },
    ]);
    program.functions.extend([
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Synthetic,
            signature: AwbcSignatureId(1),
            type_context: None,
            input_ownership: vec![AwbcFunctionInputOwnership::default()],
            frame_layout: AwbcFrameLayoutId(1),
            blocks: AwbcTableRange::new(1, 1),
            entry_block: AwbcBlockId(1),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Synthetic,
            signature: AwbcSignatureId(2),
            type_context: None,
            input_ownership: Vec::new(),
            frame_layout: AwbcFrameLayoutId(2),
            blocks: AwbcTableRange::new(2, 1),
            entry_block: AwbcBlockId(2),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        },
    ]);
    program.blocks.extend([
        AwbcBlock {
            owner: AwbcFunctionId(1),
            instructions: AwbcTableRange::new(2, 2),
            terminator: match value_thunk {
                ValueThunk::Traps => AwbcTerminator::Trap {
                    code: AwbcTrapCode::ExplicitPanic,
                    message: None,
                },
                ValueThunk::Succeeds | ValueThunk::RecoversFromDivisionByZero => {
                    AwbcTerminator::Return {
                        value: Some(AwbcRegisterId(2)),
                    }
                }
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
        AwbcBlock {
            owner: AwbcFunctionId(2),
            instructions: AwbcTableRange::new(4, 1),
            terminator: AwbcTerminator::Return {
                value: Some(AwbcRegisterId(1)),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
    ]);
    program
}

fn flow_format_attempt_program(outcome: FlowAttemptHelperOutcome) -> AwbcProgram {
    let mut program = format_program(ValueThunk::Succeeds);
    let int_type = AwbcTypeId(1);
    let string_type = AwbcTypeId(2);
    program.signatures[2].params = vec![int_type];
    program.functions[2].input_ownership = vec![AwbcFunctionInputOwnership::default()];
    program.frame_layouts[2].slots[0].role = AwbcFrameSlotRole::Parameter;
    let attempt = crate::runtime_id::RuntimeFormatAttemptId::from_zero_based(0)
        .expect("format attempt identity");
    let style = AwbcStringId(
        u32::try_from(program.strings.binary_search(&"number".to_owned()).unwrap())
            .expect("Style string index"),
    );
    let value_code = program.instructions[2..4].to_vec();

    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: string_type,
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.frame_layouts[2].slots.push(AwbcFrameSlot {
        name: None,
        ty: int_type,
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.functions[2].kind = AwbcFunctionKind::PureHelper;
    program.pure_helpers.push(AwbcPureHelper {
        public_id: style,
        signature: AwbcSignatureId(2),
        function: AwbcFunctionId(2),
        scalar_eval_supported: false,
        origin: AwbcPureHelperOrigin::EngineOwned,
    });

    let int_constant = |value: i64| {
        let mut bits = [0; 16];
        bits[..8].copy_from_slice(&value.to_le_bytes());
        AwbcConstant::Int {
            kind: AwbcSignedIntKind::I64,
            bits,
        }
    };
    if matches!(
        outcome,
        FlowAttemptHelperOutcome::RecoversFromDivisionByZero
    ) {
        program.constants[1] = int_constant(0);
    }

    let format_attempt = vec![
        AwbcInstruction::FormatOperandAttempt {
            attempt,
            parameter: crate::value::RuntimeFmtParameterId::Style,
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::CallPureHelper {
            dst: AwbcRegisterId(2),
            helper: AwbcPureHelperId(0),
            args: vec![AwbcRegisterId(1)],
        },
        AwbcInstruction::CompleteFormatOperand {
            attempt,
            parameter: crate::value::RuntimeFmtParameterId::Style,
            value: AwbcRegisterId(2),
        },
        AwbcInstruction::FormatOperandAttempt {
            attempt,
            parameter: crate::value::RuntimeFmtParameterId::Value,
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::CompleteFormatOperand {
            attempt,
            parameter: crate::value::RuntimeFmtParameterId::Value,
            value: AwbcRegisterId(1),
        },
        AwbcInstruction::FormatContent {
            destination: AwbcRegisterId(0),
            template: crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
                .expect("template ID"),
            attempt: Some(attempt),
            attempt_operands: vec![
                AwbcFormatAttemptOperand {
                    parameter: crate::value::RuntimeFmtParameterId::Style,
                    ty: string_type,
                },
                AwbcFormatAttemptOperand {
                    parameter: crate::value::RuntimeFmtParameterId::Value,
                    ty: int_type,
                },
            ],
            project_method: None,
            project_option: false,
            project_result: None,
            operands: Vec::new(),
        },
    ];
    let mut helper_body = vec![AwbcInstruction::LoadConst {
        dst: AwbcRegisterId(1),
        constant: AwbcConstantId(2),
    }];
    if matches!(
        outcome,
        FlowAttemptHelperOutcome::RecoversFromDivisionByZero
    ) {
        helper_body.extend([
            AwbcInstruction::LoadConst {
                dst: AwbcRegisterId(2),
                constant: AwbcConstantId(1),
            },
            AwbcInstruction::Binary {
                dst: AwbcRegisterId(2),
                op: AwbcBinaryOp::Div,
                lhs: AwbcRegisterId(0),
                rhs: AwbcRegisterId(2),
            },
        ]);
    }
    let helper_start = u32::try_from(format_attempt.len()).expect("main code length");
    let helper_len = u32::try_from(helper_body.len()).expect("helper code length");
    let value_start = helper_start + helper_len;
    program.instructions = format_attempt;
    program.instructions.extend(helper_body);
    program.instructions.extend(value_code);
    program.blocks[0].instructions = AwbcTableRange::new(0, helper_start);
    program.blocks[1].instructions = AwbcTableRange::new(value_start, 2);
    program.blocks[2].instructions = AwbcTableRange::new(helper_start, helper_len);
    program.blocks[2].terminator = match outcome {
        FlowAttemptHelperOutcome::Traps => AwbcTerminator::Trap {
            code: AwbcTrapCode::ExplicitPanic,
            message: None,
        },
        FlowAttemptHelperOutcome::Succeeds
        | FlowAttemptHelperOutcome::RecoversFromDivisionByZero => AwbcTerminator::Return {
            value: Some(AwbcRegisterId(1)),
        },
    };
    program
}

fn nested_flow_format_attempt_program() -> AwbcProgram {
    let mut program = format_program(ValueThunk::Succeeds);
    let outer = crate::runtime_id::RuntimeFormatAttemptId::from_zero_based(0)
        .expect("outer format attempt identity");
    let inner = crate::runtime_id::RuntimeFormatAttemptId::from_zero_based(1)
        .expect("inner format attempt identity");
    let content_type = AwbcTypeId(0);
    let int_type = AwbcTypeId(1);
    let template = crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
        .expect("template ID");
    let value_body = program.instructions[2..4].to_vec();
    let style_body = program.instructions[4..5].to_vec();
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: content_type,
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    let main = vec![
        AwbcInstruction::FormatOperandAttempt {
            attempt: outer,
            parameter: crate::value::RuntimeFmtParameterId::Value,
        },
        AwbcInstruction::FormatOperandAttempt {
            attempt: inner,
            parameter: crate::value::RuntimeFmtParameterId::Value,
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::CompleteFormatOperand {
            attempt: inner,
            parameter: crate::value::RuntimeFmtParameterId::Value,
            value: AwbcRegisterId(1),
        },
        AwbcInstruction::FormatContent {
            destination: AwbcRegisterId(2),
            template,
            attempt: Some(inner),
            attempt_operands: vec![AwbcFormatAttemptOperand {
                parameter: crate::value::RuntimeFmtParameterId::Value,
                ty: int_type,
            }],
            project_method: None,
            project_option: false,
            project_result: None,
            operands: Vec::new(),
        },
        AwbcInstruction::CompleteFormatOperand {
            attempt: outer,
            parameter: crate::value::RuntimeFmtParameterId::Value,
            value: AwbcRegisterId(2),
        },
        AwbcInstruction::FormatContent {
            destination: AwbcRegisterId(0),
            template,
            attempt: Some(outer),
            attempt_operands: vec![AwbcFormatAttemptOperand {
                parameter: crate::value::RuntimeFmtParameterId::Value,
                ty: content_type,
            }],
            project_method: None,
            project_option: false,
            project_result: None,
            operands: Vec::new(),
        },
    ];
    let value_start = u32::try_from(main.len()).expect("main instruction count");
    let style_start = value_start + u32::try_from(value_body.len()).expect("value body length");
    program.instructions = main;
    program.instructions.extend(value_body);
    program.instructions.extend(style_body);
    program.blocks[0].instructions = AwbcTableRange::new(0, value_start);
    program.blocks[1].instructions = AwbcTableRange::new(value_start, 2);
    program.blocks[2].instructions = AwbcTableRange::new(style_start, 1);
    program
}

fn project_display_program(project_success: bool) -> AwbcProgram {
    use crate::entry::{RuntimeNominalTypeId, TypeLayoutHash};
    use crate::pattern::RuntimeCheckedType;
    use crate::value::RuntimeRecordFieldId;

    let mut program = format_program(ValueThunk::Succeeds);
    program.strings.extend([
        "standard::DisplayContext".to_owned(),
        "standard::DisplayError".to_owned(),
        "locale".to_owned(),
        "style".to_owned(),
        "currency".to_owned(),
        "message".to_owned(),
        "Some".to_owned(),
        "None".to_owned(),
        "Ok".to_owned(),
        "Err".to_owned(),
        "declined by project formatter".to_owned(),
        "DisplayText::display_text".to_owned(),
    ]);
    program.canonicalize_string_table();
    let string = |value: &str| {
        AwbcStringId(
            u32::try_from(program.strings.binary_search(&value.to_owned()).unwrap())
                .expect("string table index"),
        )
    };

    let content_owner = crate::value::RuntimeDialogueOpaqueRole::Content.exact_owner();
    let string_type = AwbcTypeId(2);
    let context_type = AwbcTypeId(3);
    let error_type = AwbcTypeId(4);
    let result_type = AwbcTypeId(5);
    let option_string_type = AwbcTypeId(6);
    let tuple_string_type = AwbcTypeId(7);
    let tuple_content_type = AwbcTypeId(8);
    let tuple_error_type = AwbcTypeId(9);
    let context_layout = TypeLayoutHash::from_bytes([0x21; 32]);
    let error_layout = TypeLayoutHash::from_bytes([0x22; 32]);
    let context_checked = RuntimeCheckedType::Nominal {
        nominal: RuntimeNominalTypeId::try_new("standard::DisplayContext").unwrap(),
        semantic_identity: crate::pattern::RuntimeSemanticTypeId::from_bytes([0x23; 32]),
        layout: context_layout,
        arguments: Vec::new(),
    };
    let error_checked = RuntimeCheckedType::Nominal {
        nominal: RuntimeNominalTypeId::try_new("standard::DisplayError").unwrap(),
        semantic_identity: crate::pattern::RuntimeSemanticTypeId::from_bytes([0x24; 32]),
        layout: error_layout,
        arguments: Vec::new(),
    };
    let content_checked = RuntimeCheckedType::Opaque {
        owner: content_owner.clone(),
    };
    let result_checked = RuntimeCheckedType::Result {
        ok: Box::new(content_checked.clone()),
        error: Box::new(error_checked.clone()),
    };
    let option_string_checked = RuntimeCheckedType::Option(Box::new(RuntimeCheckedType::String));
    let tuple_string_checked = RuntimeCheckedType::Tuple(vec![RuntimeCheckedType::String]);
    let tuple_content_checked = RuntimeCheckedType::Tuple(vec![content_checked]);
    let tuple_error_checked = RuntimeCheckedType::Tuple(vec![error_checked.clone()]);

    program.runtime_types.extend([
        AwbcRuntimeType::new(
            context_checked.semantic_identity_digest(),
            AwbcRuntimeTypeShape::NominalRecord {
                public_id: string("standard::DisplayContext"),
                layout: *context_layout.as_bytes(),
                arguments: Vec::new(),
                shape: crate::entry::RuntimeNominalRecordShape::Record,
                fields: vec![
                    AwbcRecordField {
                        field: RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                        name: Some(string("locale")),
                        ty: string_type,
                    },
                    AwbcRecordField {
                        field: RuntimeRecordFieldId::try_from_zero_based_ordinal(1).unwrap(),
                        name: Some(string("style")),
                        ty: option_string_type,
                    },
                    AwbcRecordField {
                        field: RuntimeRecordFieldId::try_from_zero_based_ordinal(2).unwrap(),
                        name: Some(string("currency")),
                        ty: option_string_type,
                    },
                ],
            },
        ),
        AwbcRuntimeType::new(
            error_checked.semantic_identity_digest(),
            AwbcRuntimeTypeShape::NominalRecord {
                public_id: string("standard::DisplayError"),
                layout: *error_layout.as_bytes(),
                arguments: Vec::new(),
                shape: crate::entry::RuntimeNominalRecordShape::Record,
                fields: vec![AwbcRecordField {
                    field: RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                    name: Some(string("message")),
                    ty: string_type,
                }],
            },
        ),
        AwbcRuntimeType::new(
            result_checked.semantic_identity_digest(),
            AwbcRuntimeTypeShape::Variant {
                owner: AwbcVariantIdentity::Builtin(
                    crate::pattern::RuntimeBuiltinVariantIdentity::Result,
                ),
                arguments: Vec::new(),
                cases: vec![
                    AwbcVariantCase {
                        name: string("Ok"),
                        payload: Some(tuple_content_type),
                    },
                    AwbcVariantCase {
                        name: string("Err"),
                        payload: Some(tuple_error_type),
                    },
                ],
            },
        ),
        AwbcRuntimeType::new(
            option_string_checked.semantic_identity_digest(),
            AwbcRuntimeTypeShape::Variant {
                owner: AwbcVariantIdentity::Builtin(
                    crate::pattern::RuntimeBuiltinVariantIdentity::Option,
                ),
                arguments: Vec::new(),
                cases: vec![
                    AwbcVariantCase {
                        name: string("Some"),
                        payload: Some(tuple_string_type),
                    },
                    AwbcVariantCase {
                        name: string("None"),
                        payload: None,
                    },
                ],
            },
        ),
        AwbcRuntimeType::new(
            tuple_string_checked.semantic_identity_digest(),
            AwbcRuntimeTypeShape::Tuple(vec![string_type]),
        ),
        AwbcRuntimeType::new(
            tuple_content_checked.semantic_identity_digest(),
            AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0)]),
        ),
        AwbcRuntimeType::new(
            tuple_error_checked.semantic_identity_digest(),
            AwbcRuntimeTypeShape::Tuple(vec![error_type]),
        ),
    ]);

    let error_string =
        AwbcConstantId(u32::try_from(program.constants.len()).expect("constant identity"));
    program.constants.push(AwbcConstant::String(string(
        "declined by project formatter",
    )));
    let error_record =
        AwbcConstantId(u32::try_from(program.constants.len()).expect("constant identity"));
    program.constants.push(AwbcConstant::Record {
        ty: error_type,
        fields: vec![error_string],
    });
    let error_payload =
        AwbcConstantId(u32::try_from(program.constants.len()).expect("constant identity"));
    program
        .constants
        .push(AwbcConstant::Tuple(vec![error_record]));
    let result_constant =
        AwbcConstantId(u32::try_from(program.constants.len()).expect("constant identity"));
    program.constants.push(AwbcConstant::Variant {
        ty: result_type,
        case: 1,
        payload: Some(error_payload),
    });

    let result_register = AwbcRegisterId(2);
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: result_type,
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    let AwbcInstruction::FormatContent {
        project_method,
        project_option,
        project_result,
        ..
    } = &mut program.instructions[1]
    else {
        unreachable!("base fixture has FormatContent at the second instruction")
    };
    *project_method = Some(AwbcTraitMethodId(0));
    *project_option = false;
    *project_result = Some(result_register);

    let method_signature = AwbcSignature {
        params: vec![AwbcTypeId(1), context_type],
        result: Some(result_type),
        effects: AwbcEffectSetId(0),
    };
    let method_function = AwbcFunctionId(3);
    let method_block = AwbcBlockId(3);
    let method_entry =
        AwbcSignatureId(u32::try_from(program.signatures.len()).expect("signature identity"));
    let method_layout =
        AwbcFrameLayoutId(u32::try_from(program.frame_layouts.len()).expect("frame identity"));
    let method_instruction = u32::try_from(program.instructions.len()).expect("instruction index");
    program.signatures.push(method_signature);
    program.frame_layouts.push(AwbcFrameLayout {
        slots: vec![
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(1),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: context_type,
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: string_type,
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: tuple_content_type,
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: result_type,
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
        ],
        scopes: Vec::new(),
        max_scope_depth: 0,
    });
    if project_success {
        program.instructions.extend([
            AwbcInstruction::ProjectRecord {
                dst: AwbcRegisterId(2),
                target: AwbcRegisterId(1),
                ordinal: 0,
            },
            AwbcInstruction::FormatContent {
                destination: AwbcRegisterId(3),
                template: crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
                    .expect("template ID"),
                attempt: None,
                attempt_operands: Vec::new(),
                project_method: None,
                project_option: false,
                project_result: None,
                operands: vec![AwbcFormatOperand {
                    parameter: crate::value::RuntimeFmtParameterId::Value,
                    function: AwbcFunctionId(4),
                    captures: vec![AwbcRegisterId(2)],
                }],
            },
            AwbcInstruction::MakeTuple {
                dst: AwbcRegisterId(4),
                items: vec![AwbcRegisterId(3)],
            },
            AwbcInstruction::MakeVariant {
                dst: AwbcRegisterId(5),
                ty: result_type,
                case: 0,
                case_name: string("Ok"),
                payload: Some(AwbcRegisterId(4)),
            },
        ]);
    } else {
        program.instructions.push(AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(5),
            constant: result_constant,
        });
    }
    let method_instruction_count = if project_success { 4 } else { 1 };
    program.blocks.push(AwbcBlock {
        owner: method_function,
        instructions: AwbcTableRange::new(method_instruction, method_instruction_count),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(5)),
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.functions.push(AwbcFunction {
        public_id: None,
        kind: AwbcFunctionKind::TraitMethod,
        signature: method_entry,
        type_context: None,
        input_ownership: vec![AwbcFunctionInputOwnership::default(); 2],
        frame_layout: method_layout,
        blocks: AwbcTableRange::new(method_block.0, 1),
        entry_block: method_block,
        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
    });
    program.trait_methods.push(AwbcTraitMethod {
        public_id: string("DisplayText::display_text"),
        signature: method_entry,
        function: method_function,
        receiver: AwbcTraitReceiverMode::Owned,
        receiver_state_slot: None,
    });
    if project_success {
        let value_signature =
            AwbcSignatureId(u32::try_from(program.signatures.len()).expect("signature identity"));
        let value_layout =
            AwbcFrameLayoutId(u32::try_from(program.frame_layouts.len()).expect("frame identity"));
        let value_function = AwbcFunctionId(4);
        let value_block = AwbcBlockId(4);
        let value_instruction =
            u32::try_from(program.instructions.len()).expect("instruction index");
        program.signatures.push(AwbcSignature {
            params: vec![string_type],
            result: Some(string_type),
            effects: AwbcEffectSetId(0),
        });
        program.frame_layouts.push(AwbcFrameLayout {
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: string_type,
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: string_type,
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
            ],
            scopes: Vec::new(),
            max_scope_depth: 0,
        });
        program.instructions.push(AwbcInstruction::Move {
            dst: AwbcRegisterId(1),
            src: AwbcRegisterId(0),
        });
        program.blocks.push(AwbcBlock {
            owner: value_function,
            instructions: AwbcTableRange::new(value_instruction, 1),
            terminator: AwbcTerminator::Return {
                value: Some(AwbcRegisterId(1)),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        });
        program.functions.push(AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Synthetic,
            signature: value_signature,
            type_context: None,
            input_ownership: vec![AwbcFunctionInputOwnership::default()],
            frame_layout: value_layout,
            blocks: AwbcTableRange::new(value_block.0, 1),
            entry_block: value_block,
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        });
    }
    program
}

fn project_display_attempt_program() -> AwbcProgram {
    let mut program = project_display_program(true);
    let (destination, template, project_method, project_option, project_result) =
        match &program.instructions[1] {
            AwbcInstruction::FormatContent {
                destination,
                template,
                project_method,
                project_option,
                project_result,
                ..
            } => (
                *destination,
                *template,
                *project_method,
                *project_option,
                *project_result,
            ),
            _ => unreachable!("project fixture has FormatContent at instruction one"),
        };
    let attempt = crate::runtime_id::RuntimeFormatAttemptId::from_zero_based(0)
        .expect("format attempt identity");
    let string_type = AwbcTypeId(2);
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: string_type,
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    let main = vec![
        AwbcInstruction::FormatOperandAttempt {
            attempt,
            parameter: crate::value::RuntimeFmtParameterId::Value,
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::CompleteFormatOperand {
            attempt,
            parameter: crate::value::RuntimeFmtParameterId::Value,
            value: AwbcRegisterId(1),
        },
        AwbcInstruction::FormatOperandAttempt {
            attempt,
            parameter: crate::value::RuntimeFmtParameterId::Style,
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(3),
            constant: AwbcConstantId(2),
        },
        AwbcInstruction::CompleteFormatOperand {
            attempt,
            parameter: crate::value::RuntimeFmtParameterId::Style,
            value: AwbcRegisterId(3),
        },
        AwbcInstruction::FormatContent {
            destination,
            template,
            attempt: Some(attempt),
            attempt_operands: vec![
                AwbcFormatAttemptOperand {
                    parameter: crate::value::RuntimeFmtParameterId::Value,
                    ty: AwbcTypeId(1),
                },
                AwbcFormatAttemptOperand {
                    parameter: crate::value::RuntimeFmtParameterId::Style,
                    ty: string_type,
                },
            ],
            project_method,
            project_option,
            project_result,
            operands: Vec::new(),
        },
    ];
    program.instructions.splice(0..2, main);
    program.blocks[0].instructions = AwbcTableRange::new(0, 7);
    for block in program.blocks.iter_mut().skip(1) {
        block.instructions.start = block
            .instructions
            .start
            .checked_add(5)
            .expect("shifted fixture instruction index");
    }
    program
}

fn project_display_option_none_program() -> AwbcProgram {
    use crate::pattern::RuntimeCheckedType;

    let mut program = project_display_program(false);
    program
        .strings
        .extend(["not_a_locale".to_owned(), "none-text".to_owned()]);
    program.canonicalize_string_table();
    let string = |value: &str| {
        AwbcStringId(
            u32::try_from(program.strings.binary_search(&value.to_owned()).unwrap())
                .expect("string table index"),
        )
    };

    let option_int_type =
        AwbcTypeId(u32::try_from(program.runtime_types.len()).expect("runtime type identity"));
    let tuple_int_type = AwbcTypeId(option_int_type.0 + 1);
    let option_int_checked = RuntimeCheckedType::Option(Box::new(RuntimeCheckedType::Signed(
        crate::value::RuntimeSignedIntWidth::I64,
    )));
    let tuple_int_checked = RuntimeCheckedType::Tuple(vec![RuntimeCheckedType::Signed(
        crate::value::RuntimeSignedIntWidth::I64,
    )]);
    program.runtime_types.extend([
        AwbcRuntimeType::new(
            option_int_checked.semantic_identity_digest(),
            AwbcRuntimeTypeShape::Variant {
                owner: AwbcVariantIdentity::Builtin(
                    crate::pattern::RuntimeBuiltinVariantIdentity::Option,
                ),
                arguments: Vec::new(),
                cases: vec![
                    AwbcVariantCase {
                        name: string("Some"),
                        payload: Some(tuple_int_type),
                    },
                    AwbcVariantCase {
                        name: string("None"),
                        payload: None,
                    },
                ],
            },
        ),
        AwbcRuntimeType::new(
            tuple_int_checked.semantic_identity_digest(),
            AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(1)]),
        ),
    ]);
    let option_none =
        AwbcConstantId(u32::try_from(program.constants.len()).expect("constant identity"));
    program.constants.push(AwbcConstant::Variant {
        ty: option_int_type,
        case: 1,
        payload: None,
    });
    program.signatures[1].result = Some(option_int_type);
    program.frame_layouts[1].slots[2].ty = option_int_type;
    program.instructions[2] = AwbcInstruction::LoadConst {
        dst: AwbcRegisterId(2),
        constant: option_none,
    };
    program.instructions[3] = AwbcInstruction::Nop;
    let none_text_constant =
        AwbcConstantId(u32::try_from(program.constants.len()).expect("constant identity"));
    program
        .constants
        .push(AwbcConstant::String(string("none-text")));
    let none_function = AwbcFunctionId(4);
    let none_block = AwbcBlockId(4);
    let none_signature =
        AwbcSignatureId(u32::try_from(program.signatures.len()).expect("signature identity"));
    let none_layout =
        AwbcFrameLayoutId(u32::try_from(program.frame_layouts.len()).expect("frame identity"));
    let none_instruction = u32::try_from(program.instructions.len()).expect("instruction index");
    program.signatures.push(AwbcSignature {
        params: Vec::new(),
        result: Some(AwbcTypeId(2)),
        effects: AwbcEffectSetId(0),
    });
    program.frame_layouts.push(AwbcFrameLayout {
        slots: vec![
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(1),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(2),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
        ],
        scopes: Vec::new(),
        max_scope_depth: 0,
    });
    program.instructions.push(AwbcInstruction::LoadConst {
        dst: AwbcRegisterId(1),
        constant: none_text_constant,
    });
    program.blocks.push(AwbcBlock {
        owner: none_function,
        instructions: AwbcTableRange::new(none_instruction, 1),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(1)),
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.functions.push(AwbcFunction {
        public_id: None,
        kind: AwbcFunctionKind::Synthetic,
        signature: none_signature,
        type_context: None,
        input_ownership: Vec::new(),
        frame_layout: none_layout,
        blocks: AwbcTableRange::new(none_block.0, 1),
        entry_block: none_block,
        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
    });
    let AwbcInstruction::FormatContent {
        project_option,
        operands,
        ..
    } = &mut program.instructions[1]
    else {
        unreachable!("project DisplayText fixture has FormatContent at instruction one")
    };
    *project_option = true;
    operands[1].parameter = crate::value::RuntimeFmtParameterId::Locale;
    operands.push(AwbcFormatOperand {
        parameter: crate::value::RuntimeFmtParameterId::NoneValue,
        function: none_function,
        captures: Vec::new(),
    });
    program.constants[2] = AwbcConstant::String(string("not_a_locale"));
    program
}

#[derive(Clone, Copy)]
enum NestedValueCall {
    PureHelper,
    TraitMethod,
}

fn nested_format_program(call: NestedValueCall, value_thunk: ValueThunk) -> AwbcProgram {
    let mut program = format_program(value_thunk);
    let body = program.instructions[2..4].to_vec();
    let call_instruction = match call {
        NestedValueCall::PureHelper => AwbcInstruction::CallPureHelper {
            dst: AwbcRegisterId(2),
            helper: AwbcPureHelperId(0),
            args: vec![AwbcRegisterId(0)],
        },
        NestedValueCall::TraitMethod => AwbcInstruction::CallTraitMethod {
            dst: AwbcRegisterId(2),
            method: AwbcTraitMethodId(0),
            receiver: AwbcRegisterId(0),
            args: Vec::new(),
            receiver_out: None,
        },
    };
    program.instructions.splice(2..4, [call_instruction]);
    program.instructions.extend(body);
    program.blocks[1].instructions = AwbcTableRange::new(2, 1);
    program.blocks[2].instructions = AwbcTableRange::new(3, 1);
    let nested_terminator = std::mem::replace(
        &mut program.blocks[1].terminator,
        AwbcTerminator::Return {
            value: Some(AwbcRegisterId(2)),
        },
    );
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(3),
        instructions: AwbcTableRange::new(4, 2),
        terminator: nested_terminator,
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program.frame_layouts.push(program.frame_layouts[1].clone());
    program.functions.push(AwbcFunction {
        public_id: None,
        kind: match call {
            NestedValueCall::PureHelper => AwbcFunctionKind::PureHelper,
            NestedValueCall::TraitMethod => AwbcFunctionKind::TraitMethod,
        },
        signature: AwbcSignatureId(1),
        type_context: None,
        input_ownership: vec![AwbcFunctionInputOwnership::default()],
        frame_layout: AwbcFrameLayoutId(3),
        blocks: AwbcTableRange::new(3, 1),
        entry_block: AwbcBlockId(3),
        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
    });
    match call {
        NestedValueCall::PureHelper => program.pure_helpers.push(AwbcPureHelper {
            public_id: AwbcStringId(1),
            signature: AwbcSignatureId(1),
            function: AwbcFunctionId(3),
            scalar_eval_supported: false,
            origin: AwbcPureHelperOrigin::EngineOwned,
        }),
        NestedValueCall::TraitMethod => program.trait_methods.push(AwbcTraitMethod {
            public_id: AwbcStringId(1),
            signature: AwbcSignatureId(1),
            function: AwbcFunctionId(3),
            receiver: AwbcTraitReceiverMode::Owned,
            receiver_state_slot: None,
        }),
    }
    program
}

fn verify(program: AwbcProgram) -> std::sync::Arc<AwbcProgram> {
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("FormatContent execution fixture verifies");
    std::sync::Arc::new(program)
}

fn context(program: &std::sync::Arc<AwbcProgram>) -> crate::awbc::vm::VmExecutionContext {
    let artifact = crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x5a; 32])
        .expect("test artifact fingerprint is valid");
    crate::awbc::vm::VmExecutionContext::for_program(artifact, std::sync::Arc::clone(program))
}

fn step(
    program: &std::sync::Arc<AwbcProgram>,
    fiber: &mut FiberState,
    max_instructions: u64,
) -> crate::awbc::vm::VmStepOutput {
    step_with_format_context(
        program,
        fiber,
        max_instructions,
        crate::value::RuntimeFormatContext::default(),
    )
}

fn step_with_format_context(
    program: &std::sync::Arc<AwbcProgram>,
    fiber: &mut FiberState,
    max_instructions: u64,
    format_context: crate::value::RuntimeFormatContext,
) -> crate::awbc::vm::VmStepOutput {
    crate::awbc::vm::step_with_host_context(
        program,
        fiber,
        crate::awbc::vm::VmStepOptions { max_instructions },
        &context(program).with_format_context(format_context),
        &mut crate::awbc::vm::RejectingVmHost,
    )
    .expect("verified formatter program executes")
}

fn fiber(program: &AwbcProgram) -> FiberState {
    FiberState::for_entry(program, AwbcEntryId(0), 1, 64).expect("formatter fiber initializes")
}

fn returned_content(
    output: crate::awbc::vm::VmStepOutput,
) -> crate::value::RuntimeDialogueContentValue {
    let crate::awbc::vm::VmExit::Returned(Some(value)) = output.exit else {
        panic!("formatter returns a Content value: {:?}", output.exit);
    };
    crate::value::RuntimeDialogueContentValue::try_from_runtime_value(&value)
        .expect("formatter result has the exact Content owner")
}

fn run_to_completion(
    program: &std::sync::Arc<AwbcProgram>,
    fiber: &mut FiberState,
) -> crate::awbc::vm::VmStepOutput {
    for _ in 0..16 {
        let output = step(program, fiber, 64);
        if !matches!(output.exit, crate::awbc::vm::VmExit::Running) {
            return output;
        }
    }
    panic!("verified formatter did not finish within the instruction bound");
}

fn run_to_completion_with_format_context(
    program: &std::sync::Arc<AwbcProgram>,
    fiber: &mut FiberState,
    context: crate::value::RuntimeFormatContext,
) -> crate::awbc::vm::VmStepOutput {
    for _ in 0..16 {
        let output = step_with_format_context(program, fiber, 64, context.clone());
        if !matches!(output.exit, crate::awbc::vm::VmExit::Running) {
            return output;
        }
    }
    panic!("verified formatter did not finish within the instruction bound");
}

#[test]
fn format_content_vm_executes_value_then_style_and_returns_formatted_content() {
    let program = verify(format_program(ValueThunk::Succeeds));
    let mut fiber = fiber(&program);
    let context =
        crate::value::RuntimeFormatContext::new(arcweft_id::LocaleTag::try_new("de-DE").unwrap());
    let content = returned_content(run_to_completion_with_format_context(
        &program, &mut fiber, context,
    ));

    let slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).expect("slot ID");
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
        content.binding(slot)
    else {
        panic!("Content has the one Formatted slot");
    };
    assert_eq!(
        value.outcome(),
        &crate::value::RuntimeDialogueFormattedOutcome::Success {
            value: crate::value::RuntimeDialogueFormattedSuccess::Text("12.345".to_owned()),
            color: None,
        }
    );
}

#[test]
fn flow_format_attempt_catches_nested_recoverable_call_restores_and_runs_later_operand() {
    let program = verify(flow_format_attempt_program(
        FlowAttemptHelperOutcome::RecoversFromDivisionByZero,
    ));
    let mut fiber = fiber(&program);
    for _ in 0..24 {
        if fiber.cursor.function == AwbcFunctionId(2) {
            break;
        }
        assert!(matches!(
            step(&program, &mut fiber, 1).exit,
            crate::awbc::vm::VmExit::Running
        ));
    }
    assert_eq!(fiber.cursor.function, AwbcFunctionId(2));
    let attempt = fiber.frames[0]
        .format_attempts
        .first()
        .expect("attempt state stays on its owning Flow frame");
    assert_eq!(attempt.next_operand(), 0);
    assert_eq!(
        attempt.active_parameter(),
        Some(crate::value::RuntimeFmtParameterId::Style)
    );

    let snapshot = AwbcFiberStateSnapshot::from_live(&fiber)
        .expect("in-flight Flow operand attempt snapshots");
    let bytes = serde_json::to_vec(&snapshot).expect("attempt snapshot serializes");
    let mut forged: serde_json::Value =
        serde_json::from_slice(&bytes).expect("attempt snapshot JSON parses");
    forged["frames"][0]["format_attempts"][0]["active_parameter"] = serde_json::Value::from(255_u8);
    assert!(serde_json::from_value::<AwbcFiberStateSnapshot>(forged).is_err());
    let decoded: AwbcFiberStateSnapshot =
        serde_json::from_slice(&bytes).expect("attempt snapshot deserializes");
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program));
    let mut restored = decoded
        .into_live_for_program(&owner)
        .expect("in-flight Flow operand attempt restores");
    restored
        .validate_for_program(&program)
        .expect("restored nested attempt validates");

    let mut value_operand_starts = 0;
    let mut style_helper_instructions = 0;
    let content = loop {
        let output = step(&program, &mut restored, 1);
        for observation in &output.observations {
            if let crate::awbc::vm::VmObservation::Instruction {
                function,
                block,
                offset,
                ..
            } = observation
            {
                if *function == AwbcFunctionId(0) && *block == AwbcBlockId(0) && *offset == 4 {
                    value_operand_starts += 1;
                }
                if *function == AwbcFunctionId(2) {
                    style_helper_instructions += 1;
                }
            }
        }
        match output.exit {
            crate::awbc::vm::VmExit::Running => {}
            crate::awbc::vm::VmExit::Returned(_) => break returned_content(output),
            exit => panic!("recoverable attempt should finish after later Value: {exit:?}"),
        }
    };
    assert_eq!(value_operand_starts, 1);
    assert_eq!(style_helper_instructions, 3);
    let slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).expect("slot ID");
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
        content.binding(slot)
    else {
        panic!("Content has one Formatted slot");
    };
    let crate::value::RuntimeDialogueFormattedOutcome::Failure { reason, .. } = value.outcome()
    else {
        panic!("recoverable Flow source failure becomes formatted failure data");
    };
    assert!(reason.contains("division by zero"));
}

#[test]
fn flow_format_attempt_success_returns_formatted_content_and_fatal_trap_stops_later_operand() {
    let program = verify(flow_format_attempt_program(
        FlowAttemptHelperOutcome::Succeeds,
    ));
    let mut success_fiber = fiber(&program);
    let context = crate::value::RuntimeFormatContext::new(
        arcweft_id::LocaleTag::try_new("en-US").expect("locale"),
    );
    for _ in 0..32 {
        if success_fiber.frames[0]
            .format_attempts
            .first()
            .is_some_and(|attempt| attempt.next_operand() == 2)
        {
            break;
        }
        assert!(matches!(
            step_with_format_context(&program, &mut success_fiber, 1, context.clone()).exit,
            crate::awbc::vm::VmExit::Running
        ));
    }
    assert_eq!(success_fiber.frames[0].format_attempts.len(), 1);
    assert_eq!(success_fiber.frames[0].format_attempts[0].next_operand(), 2);
    let mut staged_values = Vec::new();
    success_fiber
        .visit_formatter_operand_values(|frame, site, ordinal, value| {
            staged_values.push((frame, site, ordinal, value.clone()));
            Ok::<_, ()>(())
        })
        .expect("formatter values are exposed with their dynamic site coordinates");
    assert_eq!(staged_values.len(), 2);
    assert_eq!(
        staged_values
            .iter()
            .map(|(_, _, ordinal, _)| *ordinal)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(matches!(&staged_values[0].3, RuntimeValue::String(_)));
    assert!(matches!(&staged_values[1].3, RuntimeValue::Int(_)));
    let content = returned_content(run_to_completion_with_format_context(
        &program,
        &mut success_fiber,
        context,
    ));
    let slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).expect("slot ID");
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
        content.binding(slot)
    else {
        panic!("Content has one Formatted slot");
    };
    assert_eq!(
        value.outcome(),
        &crate::value::RuntimeDialogueFormattedOutcome::Success {
            value: crate::value::RuntimeDialogueFormattedSuccess::Text("12,345".to_owned()),
            color: None,
        }
    );

    let program = verify(flow_format_attempt_program(FlowAttemptHelperOutcome::Traps));
    let mut fiber = fiber(&program);
    let output = run_to_completion(&program, &mut fiber);
    let crate::awbc::vm::VmExit::Trapped(trap) = output.exit else {
        panic!("fatal source helper trap propagates through the attempt");
    };
    assert_eq!(trap.code, AwbcTrapCode::ExplicitPanic);
    assert!(!output.observations.iter().any(|observation| matches!(
        observation,
        crate::awbc::vm::VmObservation::Instruction {
            function: AwbcFunctionId(0),
            block: AwbcBlockId(0),
            offset: 4,
            ..
        }
    )));
    assert!(fiber.frames[0].format_attempts.is_empty());
}

#[test]
fn awbc_verifier_rejects_forged_flow_format_attempt_manifests_and_sequences() {
    let verify_rejects = |program: AwbcProgram| {
        assert!(
            program
                .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
                .is_err()
        );
    };

    let mut missing_begin = flow_format_attempt_program(FlowAttemptHelperOutcome::Succeeds);
    missing_begin.instructions[0] = AwbcInstruction::Nop;
    verify_rejects(missing_begin);

    let mut duplicate_and_out_of_order =
        flow_format_attempt_program(FlowAttemptHelperOutcome::Succeeds);
    duplicate_and_out_of_order.instructions[4] = AwbcInstruction::FormatOperandAttempt {
        attempt: crate::runtime_id::RuntimeFormatAttemptId::from_zero_based(0).unwrap(),
        parameter: crate::value::RuntimeFmtParameterId::Style,
    };
    verify_rejects(duplicate_and_out_of_order);

    let mut missing_completion = flow_format_attempt_program(FlowAttemptHelperOutcome::Succeeds);
    missing_completion.instructions[3] = AwbcInstruction::Nop;
    verify_rejects(missing_completion);

    let mut wrong_value_type = flow_format_attempt_program(FlowAttemptHelperOutcome::Succeeds);
    wrong_value_type.instructions[3] = AwbcInstruction::CompleteFormatOperand {
        attempt: crate::runtime_id::RuntimeFormatAttemptId::from_zero_based(0).unwrap(),
        parameter: crate::value::RuntimeFmtParameterId::Style,
        value: AwbcRegisterId(1),
    };
    verify_rejects(wrong_value_type);
}

#[test]
fn nested_flow_format_attempts_keep_independent_frame_owned_state_across_restore() {
    let program = verify(nested_flow_format_attempt_program());
    let mut fiber = fiber(&program);
    assert!(matches!(
        step(&program, &mut fiber, 1).exit,
        crate::awbc::vm::VmExit::Running
    ));
    assert!(matches!(
        step(&program, &mut fiber, 1).exit,
        crate::awbc::vm::VmExit::Running
    ));
    assert_eq!(fiber.frames[0].format_attempts.len(), 2);
    assert_eq!(
        fiber.frames[0].format_attempts[0].attempt(),
        crate::runtime_id::RuntimeFormatAttemptId::from_zero_based(0).unwrap()
    );
    assert_eq!(
        fiber.frames[0].format_attempts[1].attempt(),
        crate::runtime_id::RuntimeFormatAttemptId::from_zero_based(1).unwrap()
    );

    let snapshot =
        AwbcFiberStateSnapshot::from_live(&fiber).expect("nested Flow format attempts snapshot");
    let encoded = serde_json::to_vec(&snapshot).expect("snapshot serializes");
    let decoded: AwbcFiberStateSnapshot =
        serde_json::from_slice(&encoded).expect("snapshot deserializes");
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program));
    let mut restored = decoded
        .into_live_for_program(&owner)
        .expect("nested Flow format attempts restore");
    restored
        .validate_for_program(&program)
        .expect("restored nested attempts validate independently");

    let content = returned_content(run_to_completion(&program, &mut restored));
    let slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).expect("slot ID");
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
        content.binding(slot)
    else {
        panic!("outer Content has the Formatted slot");
    };
    let crate::value::RuntimeDialogueFormattedOutcome::Success {
        value: crate::value::RuntimeDialogueFormattedSuccess::Content(inner),
        color: None,
    } = value.outcome()
    else {
        panic!("outer fmt preserves the nested Content value");
    };
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted {
        value: inner_value, ..
    }) = inner.binding(slot)
    else {
        panic!("inner Content has the Formatted slot");
    };
    assert_eq!(
        inner_value.outcome(),
        &crate::value::RuntimeDialogueFormattedOutcome::Success {
            value: crate::value::RuntimeDialogueFormattedSuccess::Text("12345".to_owned()),
            color: None,
        }
    );
}

#[test]
fn formatter_snapshot_keeps_start_locale_across_ambient_change() {
    let program = verify(format_program(ValueThunk::Succeeds));
    let mut fiber = fiber(&program);
    let en_us =
        crate::value::RuntimeFormatContext::new(arcweft_id::LocaleTag::try_new("en-US").unwrap());
    let de_de =
        crate::value::RuntimeFormatContext::new(arcweft_id::LocaleTag::try_new("de-DE").unwrap());
    for _ in 0..8 {
        if fiber.frames[0].format.is_some() {
            break;
        }
        assert!(matches!(
            step_with_format_context(&program, &mut fiber, 1, en_us.clone()).exit,
            crate::awbc::vm::VmExit::Running
        ));
    }
    assert_eq!(
        fiber.frames[0]
            .format
            .as_ref()
            .unwrap()
            .format_context()
            .active_locale(),
        en_us.active_locale()
    );
    let snapshot = AwbcFiberStateSnapshot::from_live(&fiber).unwrap();
    let bytes = serde_json::to_vec(&snapshot).unwrap();
    let mut corrupted: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    corrupted["frames"][0]["format"]["format_context"]["data_identity"] =
        serde_json::Value::String("unknown-locale-data".to_owned());
    assert!(serde_json::from_value::<AwbcFiberStateSnapshot>(corrupted).is_err());
    let decoded: AwbcFiberStateSnapshot = serde_json::from_slice(&bytes).unwrap();
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program));
    let mut restored = decoded.into_live_for_program(&owner).unwrap();
    let output = (0..16)
        .map(|_| step_with_format_context(&program, &mut restored, 64, de_de.clone()))
        .find(|output| !matches!(&output.exit, crate::awbc::vm::VmExit::Running))
        .expect("formatted value returns");
    let content = returned_content(output);
    let slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).unwrap();
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
        content.binding(slot)
    else {
        panic!("Content has the Formatted slot");
    };
    assert_eq!(
        value.outcome(),
        &crate::value::RuntimeDialogueFormattedOutcome::Success {
            value: crate::value::RuntimeDialogueFormattedSuccess::Text("12,345".to_owned()),
            color: None,
        }
    );
}

#[test]
fn format_content_vm_evaluates_style_after_recoverable_value_failure_and_resumes_snapshot() {
    let program = verify(format_program(ValueThunk::RecoversFromDivisionByZero));
    let mut fiber = fiber(&program);
    for _ in 0..8 {
        if fiber.frames[0]
            .format
            .as_ref()
            .is_some_and(|state| state.next_operand() == 1)
        {
            break;
        }
        let output = step(&program, &mut fiber, 1);
        assert!(matches!(output.exit, crate::awbc::vm::VmExit::Running));
    }

    let state = fiber.frames[0]
        .format
        .as_ref()
        .expect("formatter state remains at its instruction site");
    assert_eq!(state.next_operand(), 1);
    assert_eq!(state.values(), &[None, None]);
    assert!(
        state
            .first_recoverable()
            .is_some_and(|reason| reason.contains("division by zero"))
    );

    // Suspend at the exact formatter site after the failed Value thunk, then
    // restore its staged state before the later Style thunk runs.
    fiber.budget.remaining = 0;
    let output = step(&program, &mut fiber, 1);
    assert!(matches!(
        output.exit,
        crate::awbc::vm::VmExit::BudgetYield(_)
    ));
    let snapshot =
        AwbcFiberStateSnapshot::from_live(&fiber).expect("recoverable formatter state snapshots");
    let encoded = serde_json::to_vec(&snapshot).expect("snapshot serializes");
    let decoded: AwbcFiberStateSnapshot =
        serde_json::from_slice(&encoded).expect("snapshot deserializes");
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program));
    let mut restored = decoded
        .into_live_for_program(&owner)
        .expect("formatter snapshot restores for the exact program");
    restored
        .resume_budget_yield(&program)
        .expect("formatter resumes at its exact instruction");
    restored.replenish_budget();

    let style_start = step(&program, &mut restored, 1);
    assert!(matches!(style_start.exit, crate::awbc::vm::VmExit::Running));
    assert_eq!(restored.cursor.function, AwbcFunctionId(2));
    let _ = step(&program, &mut restored, 1);
    let _ = step(&program, &mut restored, 1);
    let resumed_state = restored.frames[0]
        .format
        .as_ref()
        .expect("formatter state waits for final construction");
    assert_eq!(resumed_state.next_operand(), 2);
    assert_eq!(
        resumed_state.values()[1],
        Some(crate::value::RuntimeValue::String("number".to_owned()))
    );

    let content = returned_content(run_to_completion(&program, &mut restored));
    let slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).expect("slot ID");
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
        content.binding(slot)
    else {
        panic!("Content has the one Formatted slot");
    };
    let crate::value::RuntimeDialogueFormattedOutcome::Failure { reason, .. } = value.outcome()
    else {
        panic!("recoverable Value evaluation produces formatted failure data");
    };
    assert!(reason.contains("division by zero"));
}

#[test]
fn format_content_vm_propagates_a_fatal_operand_trap_without_running_later_style() {
    let program = verify(format_program(ValueThunk::Traps));
    let mut fiber = fiber(&program);
    let output = run_to_completion(&program, &mut fiber);
    let crate::awbc::vm::VmExit::Trapped(trap) = output.exit else {
        panic!("fatal Value operand trap propagates from FormatContent");
    };
    assert_eq!(trap.code, AwbcTrapCode::ExplicitPanic);
    assert!(!output.observations.iter().any(|observation| matches!(
        observation,
        crate::awbc::vm::VmObservation::Instruction {
            function: AwbcFunctionId(2),
            ..
        }
    )));
}

#[test]
fn formatter_recovers_nested_helper_and_trait_errors_after_a_saved_budget_yield() {
    for call in [NestedValueCall::PureHelper, NestedValueCall::TraitMethod] {
        let program = verify(nested_format_program(
            call,
            ValueThunk::RecoversFromDivisionByZero,
        ));
        let mut fiber = fiber(&program);
        for _ in 0..8 {
            if fiber.cursor.function == AwbcFunctionId(3) {
                break;
            }
            assert!(matches!(
                step(&program, &mut fiber, 1).exit,
                crate::awbc::vm::VmExit::Running
            ));
        }
        assert_eq!(fiber.cursor.function, AwbcFunctionId(3));
        assert_eq!(fiber.frames.len(), 3);
        fiber.budget.remaining = 0;
        assert!(matches!(
            step(&program, &mut fiber, 1).exit,
            crate::awbc::vm::VmExit::BudgetYield(_)
        ));
        let snapshot = AwbcFiberStateSnapshot::from_live(&fiber).expect("nested call snapshots");
        let encoded = serde_json::to_vec(&snapshot).expect("nested call snapshot serializes");
        let decoded: AwbcFiberStateSnapshot =
            serde_json::from_slice(&encoded).expect("snapshot decodes");
        let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program));
        let mut forged = decoded.clone();
        let return_to = forged.frames[2]
            .return_to
            .as_mut()
            .expect("nested return point");
        let crate::awbc::fiber::AwbcFiberReturnContinuationSnapshot::InstructionCall { site } =
            &mut return_to.continuation
        else {
            panic!("nested call has a typed continuation");
        };
        site.instruction_offset += 1;
        assert!(
            forged
                .into_live_for_program(&owner)
                .expect("value projection succeeds")
                .validate_for_program(&program)
                .is_err()
        );

        let mut restored = decoded
            .into_live_for_program(&owner)
            .expect("exact nested call restores");
        restored
            .validate_for_program(&program)
            .expect("nested call validates");
        restored
            .resume_budget_yield(&program)
            .expect("nested budget yield resumes");
        restored.replenish_budget();
        let mut style_instructions = 0;
        let content = loop {
            let output = step(&program, &mut restored, 1);
            style_instructions += output
                .observations
                .iter()
                .filter(|observation| {
                    matches!(
                        observation,
                        crate::awbc::vm::VmObservation::Instruction {
                            function: AwbcFunctionId(2),
                            ..
                        }
                    )
                })
                .count();
            match output.exit {
                crate::awbc::vm::VmExit::Running => {}
                crate::awbc::vm::VmExit::Returned(_) => break returned_content(output),
                exit => panic!("nested formatter must finish: {exit:?}"),
            }
        };
        assert_eq!(style_instructions, 1);
        let slot =
            crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).expect("slot ID");
        let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
            content.binding(slot)
        else {
            panic!("Content has a Formatted slot");
        };
        assert!(
            matches!(value.outcome(), crate::value::RuntimeDialogueFormattedOutcome::Failure { reason, .. } if reason.contains("division by zero"))
        );
    }
}

#[test]
fn formatter_does_not_recover_nested_helper_or_trait_traps() {
    for call in [NestedValueCall::PureHelper, NestedValueCall::TraitMethod] {
        let program = verify(nested_format_program(call, ValueThunk::Traps));
        let mut fiber = fiber(&program);
        let output = run_to_completion(&program, &mut fiber);
        let crate::awbc::vm::VmExit::Trapped(trap) = output.exit else {
            panic!("nested fatal trap stays fatal");
        };
        assert_eq!(trap.code, AwbcTrapCode::ExplicitPanic);
        assert_eq!(
            fiber.frames[0]
                .format
                .as_ref()
                .expect("incomplete formatter")
                .next_operand(),
            0
        );
    }
}

#[test]
fn project_display_text_runs_in_the_caller_fiber_and_restores_its_result_continuation() {
    let program = verify(project_display_program(false));
    let mut fiber = fiber(&program);
    for _ in 0..32 {
        if fiber.cursor.function == AwbcFunctionId(3) {
            break;
        }
        assert!(matches!(
            step(&program, &mut fiber, 1).exit,
            crate::awbc::vm::VmExit::Running
        ));
    }
    assert_eq!(fiber.cursor.function, AwbcFunctionId(3));
    assert_eq!(fiber.frames.len(), 2);
    let caller = &fiber.frames[0];
    let format = caller
        .format
        .as_ref()
        .expect("caller retains formatter state while DisplayText runs");
    assert_eq!(format.next_operand(), 2);
    let return_to = fiber.frames[1]
        .return_to
        .as_ref()
        .expect("project formatter returns to the waiting FormatContent");
    assert_eq!(return_to.cursor, format.site());
    assert_eq!(return_to.destination, Some(AwbcRegisterId(2)));
    assert!(matches!(
        &return_to.continuation,
        crate::awbc::fiber::FiberReturnContinuation::FormatDisplay { site }
            if *site == format.site()
    ));
    let context = fiber.frames[1].registers[1]
        .as_ref()
        .and_then(RuntimeValue::as_nominal_record)
        .expect("DisplayContext is passed as a typed nominal record");
    assert_eq!(
        context.fields(),
        [
            RuntimeValue::String("ja-JP".to_owned()),
            RuntimeValue::option_some(RuntimeValue::String("number".to_owned())),
            RuntimeValue::option_none(),
        ]
    );

    let snapshot = AwbcFiberStateSnapshot::from_live(&fiber)
        .expect("in-flight project formatter state snapshots");
    let encoded = serde_json::to_vec(&snapshot).expect("snapshot serializes");
    let decoded: AwbcFiberStateSnapshot =
        serde_json::from_slice(&encoded).expect("snapshot deserializes");
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program));
    let mut restored = decoded
        .into_live_for_program(&owner)
        .expect("exact in-flight project formatter restores");
    let content = returned_content(run_to_completion(&program, &mut restored));
    let slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).unwrap();
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
        content.binding(slot)
    else {
        panic!("Content has the Formatted slot");
    };
    assert!(matches!(
        value.outcome(),
        crate::value::RuntimeDialogueFormattedOutcome::Failure { reason, .. }
            if reason == "declined by project formatter"
    ));
}

#[test]
fn flow_attempt_project_display_uses_staged_context_and_restores_method_call() {
    let program = verify(project_display_attempt_program());
    let mut fiber = fiber(&program);
    for _ in 0..32 {
        if fiber.cursor.function == AwbcFunctionId(3) {
            break;
        }
        assert!(matches!(
            step(&program, &mut fiber, 1).exit,
            crate::awbc::vm::VmExit::Running
        ));
    }
    assert_eq!(fiber.cursor.function, AwbcFunctionId(3));
    assert!(fiber.frames[0].format_attempts.is_empty());
    let format = fiber.frames[0]
        .format
        .as_ref()
        .expect("attempted project formatter has a caller continuation");
    assert_eq!(format.next_operand(), 2);
    assert_eq!(
        fiber.frames[1].registers[1]
            .as_ref()
            .and_then(RuntimeValue::as_nominal_record)
            .expect("DisplayContext is passed to the project method")
            .fields(),
        [
            RuntimeValue::String("ja-JP".to_owned()),
            RuntimeValue::option_some(RuntimeValue::String("number".to_owned())),
            RuntimeValue::option_none(),
        ]
    );

    let snapshot = AwbcFiberStateSnapshot::from_live(&fiber)
        .expect("project attempt method continuation snapshots");
    let encoded = serde_json::to_vec(&snapshot).expect("snapshot serializes");
    let decoded: AwbcFiberStateSnapshot =
        serde_json::from_slice(&encoded).expect("snapshot deserializes");
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program));
    let mut restored = decoded
        .into_live_for_program(&owner)
        .expect("project attempt method continuation restores");
    let content = returned_content(run_to_completion(&program, &mut restored));
    let slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).unwrap();
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
        content.binding(slot)
    else {
        panic!("Content has one Formatted slot");
    };
    let crate::value::RuntimeDialogueFormattedOutcome::Success {
        value: crate::value::RuntimeDialogueFormattedSuccess::Content(nested),
        color: None,
    } = value.outcome()
    else {
        panic!("project DisplayText returns nested Content successfully");
    };
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted {
        value: nested_value,
        ..
    }) = nested.binding(slot)
    else {
        panic!("nested Content has a Formatted slot");
    };
    assert_eq!(
        nested_value.outcome(),
        &crate::value::RuntimeDialogueFormattedOutcome::Success {
            value: crate::value::RuntimeDialogueFormattedSuccess::Text("ja-JP".to_owned()),
            color: None,
        }
    );
}

#[test]
fn project_display_text_success_content_flows_into_the_formatted_slot() {
    let program = verify(project_display_program(true));
    let mut fiber = fiber(&program);
    let content = returned_content(run_to_completion(&program, &mut fiber));
    let slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).unwrap();
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
        content.binding(slot)
    else {
        panic!("Content has the Formatted slot");
    };
    let crate::value::RuntimeDialogueFormattedOutcome::Success {
        value: crate::value::RuntimeDialogueFormattedSuccess::Content(nested),
        color: None,
    } = value.outcome()
    else {
        panic!("project DisplayText Content remains the typed outer success");
    };
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted {
        value: nested_value,
        ..
    }) = nested.binding(slot)
    else {
        panic!("project DisplayText returned Content with the Formatted slot");
    };
    assert_eq!(
        nested_value.outcome(),
        &crate::value::RuntimeDialogueFormattedOutcome::Success {
            value: crate::value::RuntimeDialogueFormattedSuccess::Text("ja-JP".to_owned()),
            color: None,
        }
    );
}

#[test]
fn option_project_none_skips_display_method_and_invalid_locale_context() {
    let program = verify(project_display_option_none_program());
    let mut fiber = fiber(&program);
    let content = returned_content(run_to_completion(&program, &mut fiber));
    let slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).unwrap();
    let Some(crate::value::RuntimeDialogueContentBinding::Formatted { value, .. }) =
        content.binding(slot)
    else {
        panic!("Content has the Formatted slot");
    };
    assert_eq!(
        value.outcome(),
        &crate::value::RuntimeDialogueFormattedOutcome::Success {
            value: crate::value::RuntimeDialogueFormattedSuccess::Text("none-text".to_owned()),
            color: None,
        }
    );
}
