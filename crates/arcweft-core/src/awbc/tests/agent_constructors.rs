use super::*;
use crate::awbc::vm::{VmExit, VmStepOptions, step};
use crate::value::{
    RuntimeAgentCompareOp, RuntimeAgentConstructor, RuntimeAgentPredicate, RuntimeAgentValue,
};

fn constructor_program(
    constructor: RuntimeAgentConstructor,
    shapes: Vec<AwbcRuntimeTypeShape>,
    operands: &[u32],
    result: u32,
) -> AwbcProgram {
    let mut program = minimal_program();
    program.runtime_types = shapes
        .into_iter()
        .enumerate()
        .map(|(index, shape)| runtime_type(u8::try_from(index + 1).unwrap(), shape))
        .collect();
    program.signatures[0].params = operands.iter().copied().map(AwbcTypeId).collect();
    program.signatures[0].result = Some(AwbcTypeId(result));
    program.frame_layouts[0].slots = operands
        .iter()
        .map(|ty| AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(*ty),
            role: AwbcFrameSlotRole::Parameter,
            scope_depth: 0,
        })
        .chain(std::iter::once(AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(result),
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        }))
        .collect();
    let dst = AwbcRegisterId(u32::try_from(operands.len()).unwrap());
    program.instructions = vec![AwbcInstruction::MakeAgent {
        dst,
        constructor,
        operands: (0..dst.0).map(AwbcRegisterId).collect(),
    }];
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);
    program.blocks[0].terminator = AwbcTerminator::Return { value: Some(dst) };
    program
}

fn round_trip(program: &AwbcProgram) -> AwbcProgram {
    let bytes = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(decoded.encode_canonical().unwrap(), bytes);
    decoded
}

fn execute(program: &AwbcProgram, operands: &[RuntimeValue]) -> RuntimeValue {
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let mut fiber =
        FiberState::for_function(program, AwbcEntryId(0), AwbcFunctionId(0), 1, 64).unwrap();
    fiber
        .bind_function_argument_values(program, operands)
        .unwrap();
    let output = step(program, &mut fiber, VmStepOptions::default()).unwrap();
    let VmExit::Returned(Some(value)) = output.exit else {
        panic!("constructor return: {:?}", output.exit)
    };
    value
}

fn probe() -> RuntimeValue {
    RuntimeValue::Agent(
        RuntimeAgentValue::try_construct(
            RuntimeAgentConstructor::ProbeSignal,
            vec![RuntimeValue::String("signal.ready".to_owned())],
        )
        .unwrap(),
    )
}

fn predicate() -> RuntimeValue {
    RuntimeValue::Agent(
        RuntimeAgentValue::try_construct(RuntimeAgentConstructor::PredicateExists, vec![probe()])
            .unwrap(),
    )
}

#[test]
fn comparison_contract_survives_codec_verification_and_execution() {
    use RuntimeAgentCompareOp as Op;
    use RuntimeAgentConstructor as Constructor;
    for (constructor, op) in [
        (Constructor::PredicateEq, Op::Eq),
        (Constructor::PredicateNotEq, Op::NotEq),
        (Constructor::PredicateGreater, Op::Greater),
        (Constructor::PredicateGreaterOrEqual, Op::GreaterOrEqual),
        (Constructor::PredicateLess, Op::Less),
        (Constructor::PredicateLessOrEqual, Op::LessOrEqual),
    ] {
        let shapes = vec![
            AwbcRuntimeTypeShape::Bool,
            AwbcRuntimeTypeShape::String,
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Probe(AwbcTypeId(0))),
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
                RuntimeAgentOperationalType::Predicate,
            )),
            AwbcRuntimeTypeShape::Dynamic,
        ];
        let program = round_trip(&constructor_program(
            constructor,
            shapes.clone(),
            &[2, 0],
            3,
        ));
        let value = execute(&program, &[probe(), RuntimeValue::Bool(true)]);
        assert!(
            matches!(value, RuntimeValue::Agent(RuntimeAgentValue::Predicate(RuntimeAgentPredicate::Compare { op: actual, value, .. })) if actual == op && *value == RuntimeValue::Bool(true))
        );
        for operands in [[2, 1], [2, 4], [4, 0]] {
            let program = round_trip(&constructor_program(
                constructor,
                shapes.clone(),
                &operands,
                3,
            ));
            assert!(
                matches!(program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()), Err(AwbcVerifyError::InvalidInvariant { message, .. }) if message.contains("rejects operand")),
                "{constructor:?} {operands:?}"
            );
        }
    }
}

#[test]
fn choice_action_materializes_the_retained_identity_in_the_vm() {
    let program = round_trip(&constructor_program(
        RuntimeAgentConstructor::ChoiceAction,
        vec![
            AwbcRuntimeTypeShape::String,
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
                RuntimeAgentOperationalType::ActionTarget,
            )),
        ],
        &[0],
        1,
    ));
    let value = execute(
        &program,
        &[RuntimeValue::String("choice.opening.listen".to_owned())],
    );
    let RuntimeValue::Agent(RuntimeAgentValue::ActionTarget(target)) = value else {
        panic!("ActionTarget")
    };
    assert_eq!(target.target().as_str(), "choice.opening.listen");
    assert_eq!(
        target.id().as_str(),
        "action.select_choice.choice.opening.listen"
    );
}

#[test]
fn collection_types_match_the_one_level_runtime_expansion() {
    let shapes = vec![
        AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
            RuntimeAgentOperationalType::Predicate,
        )),
        AwbcRuntimeTypeShape::Sequence(AwbcTypeId(0)),
        AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0)]),
        AwbcRuntimeTypeShape::Array {
            item: AwbcTypeId(0),
            length: 1,
        },
        AwbcRuntimeTypeShape::Tuple(Vec::new()),
        AwbcRuntimeTypeShape::Array {
            item: AwbcTypeId(0),
            length: 0,
        },
        AwbcRuntimeTypeShape::Sequence(AwbcTypeId(1)),
        AwbcRuntimeTypeShape::Sequence(AwbcTypeId(8)),
        AwbcRuntimeTypeShape::Dynamic,
    ];
    for constructor in [
        RuntimeAgentConstructor::PredicateAll,
        RuntimeAgentConstructor::PredicateAny,
    ] {
        for (ty, value) in [
            (0, predicate()),
            (1, runtime_sequence_values(vec![predicate()])),
            (2, RuntimeValue::Tuple(vec![predicate()])),
            (3, runtime_sequence_values(vec![predicate()])),
        ] {
            let program = round_trip(&constructor_program(constructor, shapes.clone(), &[ty], 0));
            let value = execute(&program, &[value]);
            let predicates = match value {
                RuntimeValue::Agent(RuntimeAgentValue::Predicate(RuntimeAgentPredicate::All {
                    predicates,
                })) if constructor == RuntimeAgentConstructor::PredicateAll => predicates,
                RuntimeValue::Agent(RuntimeAgentValue::Predicate(RuntimeAgentPredicate::Any {
                    predicates,
                })) if constructor == RuntimeAgentConstructor::PredicateAny => predicates,
                _ => panic!("collection constructor"),
            };
            assert_eq!(predicates.len(), 1);
        }
        for ty in [4, 5, 6, 7, 8] {
            let program = round_trip(&constructor_program(constructor, shapes.clone(), &[ty], 0));
            assert!(
                program
                    .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
                    .is_err(),
                "{constructor:?} type {ty}"
            );
        }
        for (ty, value) in [
            (4, RuntimeValue::Tuple(Vec::new())),
            (5, runtime_sequence_values(Vec::new())),
        ] {
            let program = round_trip(&constructor_program(
                constructor,
                shapes.clone(),
                &[ty, 0],
                0,
            ));
            execute(&program, &[value, predicate()]);
        }
    }
}

#[test]
fn dynamic_types_do_not_bypass_fixed_constructor_contracts() {
    let shapes = vec![
        AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::U32),
        AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
            RuntimeAgentOperationalType::ViewportPoint,
        )),
        AwbcRuntimeTypeShape::Dynamic,
    ];
    for (operands, result) in [([2, 0], 1), ([0, 2], 1), ([0, 0], 2)] {
        let program = round_trip(&constructor_program(
            RuntimeAgentConstructor::ViewportPoint,
            shapes.clone(),
            &operands,
            result,
        ));
        assert!(
            program
                .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
                .is_err()
        );
    }
    let program = round_trip(&constructor_program(
        RuntimeAgentConstructor::ViewportPoint,
        shapes,
        &[0, 0],
        1,
    ));
    execute(&program, &[RuntimeValue::u32(1), RuntimeValue::u32(2)]);
}

#[test]
fn an_unsized_empty_collection_rejects_when_its_cardinality_is_known() {
    for constructor in [
        RuntimeAgentConstructor::PredicateAll,
        RuntimeAgentConstructor::PredicateAny,
    ] {
        let program = round_trip(&constructor_program(
            constructor,
            vec![
                AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
                    RuntimeAgentOperationalType::Predicate,
                )),
                AwbcRuntimeTypeShape::Sequence(AwbcTypeId(0)),
            ],
            &[1],
            0,
        ));
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .unwrap();
        let mut fiber =
            FiberState::for_function(&program, AwbcEntryId(0), AwbcFunctionId(0), 1, 64).unwrap();
        fiber
            .bind_function_argument_values(&program, &[runtime_sequence_values(Vec::new())])
            .unwrap();
        let outcome = step(&program, &mut fiber, VmStepOptions::default()).unwrap();
        assert!(
            matches!(&outcome.exit, VmExit::Trapped(trap) if trap.code == AwbcTrapCode::InternalInvariant && trap.message.as_ref().is_some_and(|message| message.contains("does not accept 0 operand(s)"))),
            "{outcome:?}"
        );
        assert!(
            fiber
                .active_frame()
                .unwrap()
                .register(AwbcRegisterId(1))
                .is_err()
        );
    }
}
