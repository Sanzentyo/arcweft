use super::*;
use crate::entry::RuntimeCommandTargetId;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::RuntimePlanTypeSeed;
use crate::value::RuntimeAgentCompareOp;

fn identity(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn fixture() -> RuntimePlanBuilder {
    use RuntimePlanTypeProjection as Type;
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                Type::Bool,
                Type::String,
                Type::Agent(RuntimeAgentTypeProjection::Probe(identity(1))),
                Type::Agent(RuntimeAgentTypeProjection::Predicate),
                Type::Agent(RuntimeAgentTypeProjection::ActionTarget),
                Type::Sequence {
                    kind: RuntimePlanSequenceKind::Vec,
                    item: identity(4),
                },
                Type::Tuple(Box::new([identity(4)])),
                Type::Array {
                    item: identity(4),
                    length: 1,
                },
                Type::Tuple(Box::new([])),
                Type::Array {
                    item: identity(4),
                    length: 0,
                },
                Type::Sequence {
                    kind: RuntimePlanSequenceKind::Vec,
                    item: identity(6),
                },
            ]
            .into_iter()
            .enumerate()
            .map(|(index, projection)| {
                RuntimePlanTypeSeed::new(identity(u8::try_from(index + 1).unwrap()), projection)
            }),
            [],
            [],
            [],
        )
        .unwrap();
    builder
}

fn agent(ty: u8, seed: RuntimeAgentExprSeed) -> RuntimeExprSeed {
    RuntimeExprSeed::new(identity(ty), RuntimeExprSeedKind::Agent(seed))
}

fn probe() -> RuntimeExprSeed {
    agent(
        3,
        RuntimeAgentExprSeed::ProbeSignal {
            target: Box::new(RuntimeExprSeed::new(
                identity(2),
                RuntimeExprSeedKind::Value(RuntimeValue::String("signal.ready".to_owned())),
            )),
        },
    )
}

fn predicate() -> RuntimeExprSeed {
    agent(
        4,
        RuntimeAgentExprSeed::PredicateExists {
            probe: Box::new(probe()),
        },
    )
}

#[test]
fn choice_identity_is_an_abi_operand_without_an_authored_expression() {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(
                    identity(5),
                    RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::ActionTarget),
                ),
                RuntimePlanTypeSeed::new(
                    identity(4),
                    RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::Predicate),
                ),
            ],
            [],
            [],
            [],
        )
        .unwrap();
    let choice = RuntimeCommandTargetId::try_new("choice.opening.listen").unwrap();
    let expression = builder
        .lower_expression(agent(
            5,
            RuntimeAgentExprSeed::ChoiceAction {
                choice: choice.clone(),
            },
        ))
        .unwrap();
    let RuntimeExprKind::Agent(expression) = expression.kind() else {
        panic!("Agent expression")
    };
    assert_eq!(expression.choice(), Some(&choice));
    assert!(expression.operands().is_empty());
    assert_eq!(
        expression.constructor(),
        RuntimeAgentConstructor::ChoiceAction
    );
    assert!(
        builder
            .lower_expression(agent(4, RuntimeAgentExprSeed::ChoiceAction { choice }))
            .is_err()
    );
}

#[test]
fn every_comparison_requires_the_probe_result_type() {
    let builder = fixture();
    for op in [
        RuntimeAgentCompareOp::Eq,
        RuntimeAgentCompareOp::NotEq,
        RuntimeAgentCompareOp::Greater,
        RuntimeAgentCompareOp::GreaterOrEqual,
        RuntimeAgentCompareOp::Less,
        RuntimeAgentCompareOp::LessOrEqual,
    ] {
        for (ty, value, accepted) in [
            (1, RuntimeValue::Bool(true), true),
            (2, RuntimeValue::String("true".to_owned()), false),
        ] {
            let result = builder.lower_expression(agent(
                4,
                RuntimeAgentExprSeed::PredicateCompare {
                    probe: Box::new(probe()),
                    op,
                    value: Box::new(RuntimeExprSeed::new(
                        identity(ty),
                        RuntimeExprSeedKind::Value(value),
                    )),
                },
            ));
            if accepted {
                result.unwrap();
            } else {
                assert!(
                    matches!(
                        result,
                        Err(RuntimePlanBuildError::InvalidAgentOperandType { operand: 1, .. })
                    ),
                    "{op:?}: {result:?}"
                );
            }
        }
    }
}

#[test]
fn predicate_collections_expand_one_level_and_require_a_possible_element() {
    let builder = fixture();
    let collections = [
        predicate(),
        RuntimeExprSeed::new(
            identity(6),
            RuntimeExprSeedKind::BracketSeq(Box::new([predicate()])),
        ),
        RuntimeExprSeed::new(
            identity(7),
            RuntimeExprSeedKind::Tuple(Box::new([predicate()])),
        ),
        RuntimeExprSeed::new(
            identity(8),
            RuntimeExprSeedKind::BracketSeq(Box::new([predicate()])),
        ),
    ];
    let empty = [
        RuntimeExprSeed::new(identity(9), RuntimeExprSeedKind::Tuple(Box::new([]))),
        RuntimeExprSeed::new(identity(10), RuntimeExprSeedKind::BracketSeq(Box::new([]))),
    ];
    for any in [false, true] {
        let collection = |predicates: Vec<RuntimeExprSeed>| {
            agent(
                4,
                if any {
                    RuntimeAgentExprSeed::PredicateAny {
                        predicates: predicates.into_boxed_slice(),
                    }
                } else {
                    RuntimeAgentExprSeed::PredicateAll {
                        predicates: predicates.into_boxed_slice(),
                    }
                },
            )
        };
        for operand in &collections {
            builder
                .lower_expression(collection(vec![operand.clone()]))
                .unwrap();
        }
        for operand in &empty {
            assert!(
                builder
                    .lower_expression(collection(vec![operand.clone()]))
                    .is_err()
            );
            builder
                .lower_expression(collection(vec![operand.clone(), predicate()]))
                .unwrap();
        }
        assert!(builder.lower_expression(collection(Vec::new())).is_err());
        let nested = RuntimeExprSeed::new(
            identity(11),
            RuntimeExprSeedKind::BracketSeq(Box::new([collections[1].clone()])),
        );
        assert!(matches!(
            builder.lower_expression(collection(vec![nested])),
            Err(RuntimePlanBuildError::InvalidAgentOperandType { operand: 0, .. })
        ));
    }
}
