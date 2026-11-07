use super::*;

fn termination_program(terminator: AwbcTerminator, depth: u32) -> AwbcProgram {
    let mut program = minimal_program();
    if matches!(terminator, AwbcTerminator::Complete) {
        program.runtime_types = vec![runtime_type(1, AwbcRuntimeTypeShape::Unit)];
        program.signatures[0].result = Some(AwbcTypeId(0));
        program.functions[0].semantic_role = crate::plan::RuntimeFunctionSemanticRole::Flow;
    }
    program.frame_layouts[0].scopes = (0..depth)
        .map(|index| AwbcScopeDefinition {
            kind: crate::scope::RuntimeScopeFrameKind::EmittedLexical,
            parent: index.checked_sub(1).map(AwbcScopeId),
            identity: crate::scope::RuntimeScopeIdentity::Anonymous,
        })
        .collect();
    program.frame_layouts[0].max_scope_depth = depth;
    program.instructions = (0..depth)
        .map(|index| AwbcInstruction::EnterScope {
            scope: AwbcScopeId(index),
        })
        .collect();
    program.blocks[0].instructions = AwbcTableRange::new(0, depth);
    program.blocks[0].terminator = terminator;
    program
}

fn assert_scope_exit_is_required(terminator: AwbcTerminator) {
    for depth in [1, 2] {
        let mut program = termination_program(terminator.clone(), depth);
        assert!(matches!(
            program.verify(Default::default(), Default::default()),
            Err(AwbcVerifyError::ScopeDiscipline {
                function: 0,
                block: 0,
                ..
            })
        ));
        assert!(matches!(
            super::super::product_step::AwbcProductProgram::admit(
                std::sync::Arc::new(program.clone()),
                Default::default(),
                Default::default(),
                Default::default(),
                None,
            ),
            Err(super::super::product_step::AwbcProductStepBuildError::InvalidProgram { .. })
        ));
        for index in (0..depth).rev() {
            program.instructions.push(AwbcInstruction::ExitScope {
                scope: AwbcScopeId(index),
            });
            program.blocks[0].instructions.len += 1;
            if index > 0 {
                assert!(matches!(
                    program.verify(Default::default(), Default::default()),
                    Err(AwbcVerifyError::ScopeDiscipline { .. })
                ));
            }
        }
        program
            .verify(Default::default(), Default::default())
            .expect("termination follows every lexical scope exit");
        let encoded = program.encode_canonical().unwrap();
        let decoded = AwbcProgram::decode_canonical(&encoded, Default::default()).unwrap();
        decoded
            .verify(Default::default(), Default::default())
            .expect("canonical code preserves explicit scope exits");
        super::super::product_step::AwbcProductProgram::admit(
            std::sync::Arc::new(decoded),
            Default::default(),
            Default::default(),
            Default::default(),
            None,
        )
        .expect("the fully exited code can activate as a Product program");
    }
}

#[test]
fn raw_return_requires_all_lexical_scopes_to_exit_before_admission() {
    assert_scope_exit_is_required(AwbcTerminator::Return { value: None });
}

#[test]
fn raw_completion_requires_all_lexical_scopes_to_exit_before_admission() {
    assert_scope_exit_is_required(AwbcTerminator::Complete);
}
