use super::*;

#[test]
fn canonical_or_patterns_reject_bad_arity_cycles_and_binding_inventories() {
    let mut program = minimal_program();
    program.runtime_types = vec![runtime_type(1, AwbcRuntimeTypeShape::Bool)];
    program.signatures[0].params = vec![AwbcTypeId(0)];
    program.functions[0].input_ownership = vec![AwbcFunctionInputOwnership::parameter(
        manual_awbc_formal_identity(
            "arcweft-core.src.awbc.tests.or_patterns.canonical_or_patterns_reject_bad_arity_cycles_and_binding_inventories.input-a",
        ),
        0,
        crate::plan::RuntimeFunctionParameterPassing::Value,
    )];
    program.frame_layouts[0].slots = (0..3)
        .map(|index| AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(0),
            role: if index == 0 {
                AwbcFrameSlotRole::Parameter
            } else {
                AwbcFrameSlotRole::Local
            },
            scope_depth: 0,
        })
        .collect();
    program.patterns = vec![
        AwbcPattern::Bind {
            target: AwbcRegisterId(1),
            mutable: false,
            expected: Some(AwbcTypeId(0)),
        },
        AwbcPattern::Bind {
            target: AwbcRegisterId(1),
            mutable: false,
            expected: Some(AwbcTypeId(0)),
        },
        AwbcPattern::Or(vec![AwbcPatternId(0), AwbcPatternId(1)]),
    ];
    program.instructions = vec![AwbcInstruction::BindPattern {
        pattern: AwbcPatternId(2),
        value: AwbcRegisterId(0),
        mode: AwbcBindMode::Declare,
    }];
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);
    let bytes = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(decoded, program);
    assert_eq!(decoded.encode_canonical().unwrap(), bytes);
    decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();

    for alternatives in [
        vec![],
        vec![AwbcPatternId(0)],
        vec![AwbcPatternId(0), AwbcPatternId(2)],
        vec![AwbcPatternId(0), AwbcPatternId(99)],
    ] {
        let mut invalid = program.clone();
        invalid.patterns[2] = AwbcPattern::Or(alternatives);
        assert!(
            invalid
                .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
                .is_err()
        );
    }
    let mut invalid = program;
    invalid.patterns[1] = AwbcPattern::Bind {
        target: AwbcRegisterId(2),
        mutable: false,
        expected: Some(AwbcTypeId(0)),
    };
    assert!(matches!(
        invalid.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::InvalidInvariant { message, .. })
            if message == "alternative binding inventories differ"
    ));
}

fn manual_awbc_formal_identity(declaration: &str) -> crate::plan::RuntimeFunctionParameterIdentity {
    let mut hash = blake3::Hasher::new();
    hash.update(b"arcweft.manual-awbc-formal.v1\0");
    hash.update(declaration.as_bytes());
    crate::plan::RuntimeFunctionParameterIdentity::from_accepted_identity(
        *hash.finalize().as_bytes(),
    )
}
