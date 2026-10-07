use super::*;
use crate::{
    awbc::schema::AwbcRuntimeType,
    effect_row::EffectFormula,
    pattern::RuntimeSemanticTypeId,
    plan::{RuntimeFunctionTypeContract, RuntimeTypeBinder, RuntimeTypeScope},
};

#[test]
fn nominal_arguments_preserve_declaration_identity_and_invariant_effects() {
    let row = |marker, shape| {
        AwbcRuntimeType::new(RuntimeSemanticTypeId::from_bytes([marker; 32]), shape)
    };
    let declaration = crate::entry::RuntimeNominalDeclarationId::from_bytes([0x81; 32]);
    let callback = |effects| AwbcRuntimeTypeShape::Function {
        contract: RuntimeFunctionTypeContract::new(
            RuntimeTypeBinder::EMPTY,
            EffectPredicate::unconstrained(),
            effects,
        ),
        parameters: vec![],
        result: AwbcTypeId(0),
    };
    let nominal = |argument| AwbcRuntimeTypeShape::NominalRecord {
        public_id: crate::awbc::schema::AwbcStringId(0),
        layout: [0x82; 32],
        arguments: vec![argument],
        shape: crate::entry::RuntimeNominalRecordShape::Unit,
        fields: vec![],
    };
    let mut program = AwbcProgram::default();
    program.runtime_types = vec![
        row(1, AwbcRuntimeTypeShape::Unit),
        row(2, callback(EffectFormula::empty())),
        row(
            3,
            callback(EffectFormula::literal(
                crate::effect_row::EffectSet::from_labels(["io.read"]).unwrap(),
                None,
            )),
        ),
        row(4, nominal(AwbcTypeId(1))).with_nominal_declaration(declaration),
        row(5, nominal(AwbcTypeId(2))).with_nominal_declaration(declaration),
        row(6, nominal(AwbcTypeId(1))).with_nominal_declaration(
            crate::entry::RuntimeNominalDeclarationId::from_bytes([0x83; 32]),
        ),
        row(
            7,
            AwbcRuntimeTypeShape::Function {
                contract: RuntimeFunctionTypeContract::new(
                    RuntimeTypeBinder::new(0, 0, 1),
                    EffectPredicate::unconstrained(),
                    EffectFormula::empty(),
                ),
                parameters: vec![AwbcTypeId(4)],
                result: AwbcTypeId(0),
            },
        ),
    ];
    assert!(program.parameter_contract_accepts_types(AwbcTypeId(6), [(0, AwbcTypeId(4))]));
    assert!(!program.parameter_contract_accepts_types(AwbcTypeId(6), [(0, AwbcTypeId(3))]));
    program.runtime_types[6] = row(
        8,
        AwbcRuntimeTypeShape::Function {
            contract: RuntimeFunctionTypeContract::new(
                RuntimeTypeBinder::new(0, 0, 1),
                EffectPredicate::unconstrained(),
                EffectFormula::empty(),
            ),
            parameters: vec![AwbcTypeId(3)],
            result: AwbcTypeId(0),
        },
    );
    assert!(!program.parameter_contract_accepts_types(AwbcTypeId(6), [(0, AwbcTypeId(5))]));
}

#[test]
fn nominal_origin_is_independent_of_destination_effect_bindings() {
    let id = |marker| RuntimeSemanticTypeId::from_bytes([marker; 32]);
    let binder = RuntimeTypeBinder::new(0, 0, 1);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let callback = |effects| AwbcRuntimeTypeShape::Function {
        contract: RuntimeFunctionTypeContract::new(
            RuntimeTypeBinder::EMPTY,
            EffectPredicate::unconstrained(),
            effects,
        ),
        parameters: vec![],
        result: AwbcTypeId(0),
    };
    let nominal = |argument| AwbcRuntimeTypeShape::NominalRecord {
        public_id: crate::awbc::schema::AwbcStringId(0),
        layout: [0x82; 32],
        arguments: vec![argument],
        shape: crate::entry::RuntimeNominalRecordShape::Unit,
        fields: vec![],
    };
    let declaration = crate::entry::RuntimeNominalDeclarationId::from_bytes([0x81; 32]);
    let mut program = AwbcProgram::default();
    program.runtime_types = vec![
        AwbcRuntimeType::new(id(1), AwbcRuntimeTypeShape::Unit),
        AwbcRuntimeType::new(id(2), callback(EffectFormula::empty())),
        AwbcRuntimeType::new(
            id(3),
            callback(EffectFormula::literal(
                Default::default(),
                Some(scope.bound_effect(0, 0).unwrap()),
            )),
        )
        .with_scope(scope.clone()),
        AwbcRuntimeType::new(id(4), nominal(AwbcTypeId(1))).with_nominal_declaration(declaration),
        AwbcRuntimeType::new(id(5), nominal(AwbcTypeId(2)))
            .with_scope(scope)
            .with_nominal_declaration(declaration),
        AwbcRuntimeType::new(
            id(6),
            AwbcRuntimeTypeShape::Function {
                contract: RuntimeFunctionTypeContract::new(
                    binder,
                    EffectPredicate::unconstrained(),
                    EffectFormula::empty(),
                ),
                parameters: vec![AwbcTypeId(2)],
                result: AwbcTypeId(0),
            },
        ),
    ];
    let empty = crate::effect_row::EffectSet::new();
    let io = crate::effect_row::EffectSet::from_labels(["io.read"]).unwrap();
    let binding = |effects| RuntimeFunctionEffectInstantiation {
        context: id(6),
        effects: Box::new([effects]),
    };
    let destination = binding(empty.clone());
    let source = binding(io);
    let matches = |destination: Option<&RuntimeFunctionEffectInstantiation>, origin| {
        RuntimeFunctionEffectInstantiation::with_value_relation(&program, destination, |relation| {
            relation.nominal(
                if destination.is_some() {
                    AwbcTypeId(4)
                } else {
                    AwbcTypeId(3)
                },
                AwbcTypeId(4),
                origin,
            )
        }) == Some(true)
    };
    assert!(!matches(Some(&destination), None));
    assert!(!matches(Some(&destination), Some(&source)));
    assert!(matches(Some(&destination), Some(&destination)));
    assert!(!matches(None, Some(&source)));
    assert!(matches(None, Some(&destination)));
    let malformed = RuntimeFunctionEffectInstantiation {
        context: id(6),
        effects: Box::new([]),
    };
    assert!(!matches(Some(&destination), Some(&malformed)));
}

#[test]
fn callback_local_type_and_length_binders_preserve_rigid_identity() {
    use crate::plan::{
        RuntimeArrayLength, RuntimePlanBuilder, RuntimePlanTypeProjection as T, RuntimePlanTypeSeed,
    };
    let id = |marker| RuntimeSemanticTypeId::from_bytes([marker; 32]);
    let outer = RuntimeTypeBinder::new(0, 0, 1);
    let inner = RuntimeTypeBinder::new(2, 2, 0);
    let outer_scope = RuntimeTypeScope::root().enter(outer).unwrap();
    let expected_scope = outer_scope.enter(inner).unwrap();
    let actual_scope = RuntimeTypeScope::root().enter(inner).unwrap();
    let io = crate::effect_row::EffectSet::from_labels(["io.read"]).unwrap();
    let expected_effects = EffectFormula::literal(
        Default::default(),
        Some(expected_scope.bound_effect(1, 0).unwrap()),
    );
    let actual_effects = EffectFormula::literal(io.clone(), None);
    let contract = |effects| {
        RuntimeFunctionTypeContract::new(inner, EffectPredicate::unconstrained(), effects)
    };
    let outer_contract = RuntimeFunctionTypeContract::new(
        outer,
        EffectPredicate::unconstrained(),
        EffectFormula::empty(),
    );
    let expected_type = expected_scope.bound_type(0, 0).unwrap();
    let actual_type = actual_scope.bound_type(0, 0).unwrap();
    let other_type = actual_scope.bound_type(0, 1).unwrap();
    let expected_length = RuntimeArrayLength::Bound(expected_scope.bound_const(0, 0).unwrap());
    let actual_length = RuntimeArrayLength::Bound(actual_scope.bound_const(0, 0).unwrap());
    let other_length = RuntimeArrayLength::Bound(actual_scope.bound_const(0, 1).unwrap());
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(id(1), T::Unit),
                RuntimePlanTypeSeed::new(id(2), T::BoundType(expected_type))
                    .with_scope(expected_scope.clone()),
                RuntimePlanTypeSeed::new(
                    id(3),
                    T::Array {
                        item: id(2),
                        length: expected_length,
                    },
                )
                .with_scope(expected_scope),
                RuntimePlanTypeSeed::new(
                    id(4),
                    T::Function {
                        contract: contract(expected_effects.clone()),
                        parameters: Box::new([id(3)]),
                        result: id(2),
                    },
                )
                .with_scope(outer_scope.clone()),
                RuntimePlanTypeSeed::new(id(5), T::BoundType(actual_type))
                    .with_scope(actual_scope.clone()),
                RuntimePlanTypeSeed::new(
                    id(6),
                    T::Array {
                        item: id(5),
                        length: actual_length,
                    },
                )
                .with_scope(actual_scope.clone()),
                RuntimePlanTypeSeed::new(
                    id(7),
                    T::Function {
                        contract: contract(actual_effects.clone()),
                        parameters: Box::new([id(6)]),
                        result: id(5),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    id(8),
                    T::Function {
                        contract: outer_contract.clone(),
                        parameters: Box::new([id(4)]),
                        result: id(1),
                    },
                ),
                RuntimePlanTypeSeed::new(id(9), T::BoundType(other_type))
                    .with_scope(actual_scope.clone()),
                RuntimePlanTypeSeed::new(
                    id(10),
                    T::Array {
                        item: id(9),
                        length: actual_length,
                    },
                )
                .with_scope(actual_scope.clone()),
                RuntimePlanTypeSeed::new(
                    id(11),
                    T::Function {
                        contract: contract(actual_effects.clone()),
                        parameters: Box::new([id(10)]),
                        result: id(9),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    id(12),
                    T::Array {
                        item: id(5),
                        length: other_length,
                    },
                )
                .with_scope(actual_scope.clone()),
                RuntimePlanTypeSeed::new(
                    id(13),
                    T::Function {
                        contract: contract(actual_effects.clone()),
                        parameters: Box::new([id(12)]),
                        result: id(5),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    id(14),
                    T::Array {
                        item: id(5),
                        length: RuntimeArrayLength::Constant(3),
                    },
                )
                .with_scope(actual_scope.clone()),
                RuntimePlanTypeSeed::new(
                    id(15),
                    T::Function {
                        contract: contract(actual_effects.clone()),
                        parameters: Box::new([id(14)]),
                        result: id(5),
                    },
                ),
            ],
            [],
        )
        .unwrap();
    let plan = builder.finish().unwrap();
    let row = |marker, shape| AwbcRuntimeType::new(id(marker), shape);
    let function = |contract, parameter, result| AwbcRuntimeTypeShape::Function {
        contract,
        parameters: vec![AwbcTypeId(parameter)],
        result: AwbcTypeId(result),
    };
    let mut program = AwbcProgram::default();
    program.runtime_types = vec![
        row(1, AwbcRuntimeTypeShape::Unit),
        row(2, AwbcRuntimeTypeShape::BoundType(expected_type))
            .with_scope(outer_scope.enter(inner).unwrap()),
        row(
            3,
            AwbcRuntimeTypeShape::Array {
                item: AwbcTypeId(1),
                length: expected_length,
            },
        )
        .with_scope(outer_scope.enter(inner).unwrap()),
        row(4, function(contract(expected_effects), 2, 1)).with_scope(outer_scope),
        row(5, AwbcRuntimeTypeShape::BoundType(actual_type)).with_scope(actual_scope.clone()),
        row(
            6,
            AwbcRuntimeTypeShape::Array {
                item: AwbcTypeId(4),
                length: actual_length,
            },
        )
        .with_scope(actual_scope.clone()),
        row(7, function(contract(actual_effects.clone()), 5, 4)),
        row(8, function(outer_contract, 3, 0)),
        row(9, AwbcRuntimeTypeShape::BoundType(other_type)).with_scope(actual_scope.clone()),
        row(
            10,
            AwbcRuntimeTypeShape::Array {
                item: AwbcTypeId(8),
                length: actual_length,
            },
        )
        .with_scope(actual_scope.clone()),
        row(11, function(contract(actual_effects.clone()), 9, 8)),
        row(
            12,
            AwbcRuntimeTypeShape::Array {
                item: AwbcTypeId(4),
                length: other_length,
            },
        )
        .with_scope(actual_scope.clone()),
        row(13, function(contract(actual_effects.clone()), 11, 4)),
        row(
            14,
            AwbcRuntimeTypeShape::Array {
                item: AwbcTypeId(4),
                length: RuntimeArrayLength::Constant(3),
            },
        )
        .with_scope(actual_scope.clone()),
        row(15, function(contract(actual_effects), 13, 4)),
    ];
    fn relates<A: FunctionTypeAuthority>(
        program: &A,
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
        context: RuntimeSemanticTypeId,
        effects: crate::effect_row::EffectSet,
    ) -> bool {
        let binding = RuntimeFunctionEffectInstantiation {
            context,
            effects: Box::new([effects]),
        };
        let mut matcher = binding.matcher(program).unwrap();
        let environment = matcher.parameters.clone();
        matcher
            .types(
                program.by_semantic(expected).unwrap(),
                program.by_semantic(actual).unwrap(),
                0,
                &environment,
                &ContractEnvironment::root(),
            )
            .is_ok()
            && matcher.accepted()
    }
    // The local T/N binder is alpha-equivalent despite a different outer
    // effect scope. Its slots are rigid, never inferred from runtime values.
    for actual in [7, 11, 13, 15] {
        for effects in [Default::default(), io.clone()] {
            let accepted = actual == 7 && !effects.is_empty();
            assert_eq!(
                relates(&plan, id(4), id(actual), id(8), effects.clone()),
                accepted,
                "Plan actual={actual}"
            );
            assert_eq!(
                relates(&program, id(4), id(actual), id(8), effects),
                accepted,
                "AWBC actual={actual}"
            );
        }
    }
    // Inert AWBC coordinates must still belong to their declared namespace.
    program.runtime_types[4] = row(
        5,
        AwbcRuntimeTypeShape::BoundType(RuntimeBoundTypeReference::from_coordinates(0, 2)),
    )
    .with_scope(actual_scope.clone());
    assert!(!relates(&program, id(4), id(7), id(8), io.clone()));
    program.runtime_types[4] =
        row(5, AwbcRuntimeTypeShape::BoundType(actual_type)).with_scope(actual_scope.clone());
    program.runtime_types[5] = row(
        6,
        AwbcRuntimeTypeShape::Array {
            item: AwbcTypeId(4),
            length: RuntimeArrayLength::Bound(RuntimeBoundConstReference::from_coordinates(0, 2)),
        },
    )
    .with_scope(actual_scope);
    assert!(!relates(&program, id(4), id(7), id(8), io));
}

#[test]
fn failed_value_choice_does_not_leak_callback_effect_constraints() {
    use crate::plan::{
        RuntimeEffectSet, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed,
        RuntimeFlowSchema, RuntimeFlowSeed, RuntimeFunctionSiteBodyKind,
        RuntimeFunctionSiteDeclarationSeed, RuntimePatternSeed, RuntimePatternSeedKind,
        RuntimePlanBuilder, RuntimePlanTypeProjection as T, RuntimePlanTypeSeed,
    };
    use crate::value::{RuntimeCallableValue, RuntimeExprKind};
    let id = |value| RuntimeSemanticTypeId::from_bytes([value; 32]);
    let binder = RuntimeTypeBinder::new(0, 0, 1);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let io = crate::effect_row::EffectSet::from_labels(["io.read"]).unwrap();
    let function = |effects| T::Function {
        contract: RuntimeFunctionTypeContract::new(
            RuntimeTypeBinder::EMPTY,
            EffectPredicate::unconstrained(),
            effects,
        ),
        parameters: Box::new([]),
        result: id(1),
    };
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(id(1), T::Unit),
                RuntimePlanTypeSeed::new(id(2), T::String),
                RuntimePlanTypeSeed::new(id(3), function(EffectFormula::literal(io.clone(), None))),
                RuntimePlanTypeSeed::new(
                    id(4),
                    function(EffectFormula::literal(
                        Default::default(),
                        Some(scope.bound_effect(0, 0).unwrap()),
                    )),
                )
                .with_scope(scope.clone()),
                RuntimePlanTypeSeed::new(id(5), T::Tuple(Box::new([id(4), id(2)])))
                    .with_scope(scope.clone()),
                RuntimePlanTypeSeed::new(id(6), T::Tuple(Box::new([id(3), id(1)]))),
                RuntimePlanTypeSeed::new(id(7), T::Choice(Box::new([id(5), id(6)])))
                    .with_scope(scope),
                RuntimePlanTypeSeed::new(
                    id(8),
                    T::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            binder,
                            EffectPredicate::unconstrained(),
                            EffectFormula::empty(),
                        ),
                        parameters: Box::new([id(7)]),
                        result: id(1),
                    },
                ),
            ],
            [],
        )
        .unwrap();
    let site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                [41; 32],
            ),
            role: crate::plan::RuntimeFunctionSemanticRole::Closure,
            function_type: None,
            inputs: Box::new([]),
            result: id(1),
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeEffectSet::try_from_effects(io.iter().cloned()).unwrap(),
        })
        .unwrap();
    builder
        .define_function_site_seed(
            &site,
            RuntimeExprSeed::new(id(1), RuntimeExprSeedKind::Value(RuntimeValue::Unit)),
        )
        .unwrap();
    let flow = crate::plan::FlowRuntimeId::canonical("choice_relation").unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: vec![],
        })
        .unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow,
            crate::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([61; 32]),
                None,
                Box::new([]),
                id(1),
                RuntimeEffectSet::empty(),
            ),
            crate::plan::RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: (vec![RuntimeFlowOpSeed::Let {
                    pattern: RuntimePatternSeed::new(id(3), RuntimePatternSeedKind::Discard),
                    expr: RuntimeExprSeed::new(
                        id(3),
                        RuntimeExprSeedKind::Function {
                            site,
                            captures: Box::new([]),
                        },
                    ),
                }])
                .into_boxed_slice(),
            },
        ))
        .unwrap();
    let plan = std::sync::Arc::new(builder.finish().unwrap());
    let crate::plan::FlowOp::Let { expr, .. } = &plan.flows()[0].body().ops()[0] else {
        panic!("typed callback fixture")
    };
    let RuntimeExprKind::MakeCallable { state, .. } = expr.kind() else {
        panic!("admitted callback state")
    };
    let callback = RuntimeCallableValue::try_new(
        crate::task::RuntimeProgramOwner::Plan(plan.clone()),
        *state,
        [],
    )
    .unwrap();
    let value = RuntimeValue::Tuple(vec![RuntimeValue::Callable(callback), RuntimeValue::Unit]);
    let binding = super::super::RuntimeProgramTypes::Plan(&plan)
        .instantiate_function_effects(id(8), &[&value])
        .unwrap();
    assert_eq!(
        binding.effects.as_ref(),
        &[crate::effect_row::EffectSet::new()]
    );
    let expected = plan.type_table().id_for_semantic(id(7)).unwrap();
    assert!(binding.value_matches(plan.as_ref(), expected, &value));
    assert!(
        plan.validate_live_value(
            expected,
            &value,
            crate::entry::RuntimeSchemaLimits::engine_default()
        )
        .is_err()
    );
}

#[test]
fn scoped_default_preserves_shared_source_rows_across_callback_variance() {
    let binder = RuntimeTypeBinder::new(0, 0, 2);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let row = |marker, shape| {
        AwbcRuntimeType::new(RuntimeSemanticTypeId::from_bytes([marker; 32]), shape)
    };
    let function = |parameters, result, effects| AwbcRuntimeTypeShape::Function {
        contract: RuntimeFunctionTypeContract::new(
            RuntimeTypeBinder::EMPTY,
            EffectPredicate::unconstrained(),
            effects,
        ),
        parameters,
        result,
    };
    let header = |parameters, result| AwbcRuntimeTypeShape::Function {
        contract: RuntimeFunctionTypeContract::new(
            binder,
            EffectPredicate::unconstrained(),
            EffectFormula::empty(),
        ),
        parameters,
        result,
    };
    let mut program = AwbcProgram::default();
    program.runtime_types = vec![
        row(1, AwbcRuntimeTypeShape::Unit),
        row(
            2,
            function(
                vec![],
                AwbcTypeId(0),
                EffectFormula::literal(Default::default(), Some(scope.bound_effect(0, 0).unwrap())),
            ),
        )
        .with_scope(scope.clone()),
        row(
            3,
            function(
                vec![],
                AwbcTypeId(0),
                EffectFormula::literal(Default::default(), Some(scope.bound_effect(0, 1).unwrap())),
            ),
        )
        .with_scope(scope.clone()),
        row(
            4,
            function(vec![AwbcTypeId(1)], AwbcTypeId(0), EffectFormula::empty()),
        )
        .with_scope(scope.clone()),
        row(
            5,
            function(vec![AwbcTypeId(2)], AwbcTypeId(0), EffectFormula::empty()),
        )
        .with_scope(scope.clone()),
        row(
            6,
            AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(1), AwbcTypeId(3)]),
        )
        .with_scope(scope.clone()),
        row(
            7,
            AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(2), AwbcTypeId(4)]),
        )
        .with_scope(scope),
        row(8, header(vec![AwbcTypeId(1), AwbcTypeId(6)], AwbcTypeId(0))),
        row(9, header(vec![AwbcTypeId(1)], AwbcTypeId(5))),
        row(
            10,
            function(
                vec![],
                AwbcTypeId(0),
                EffectFormula::literal(
                    crate::effect_row::EffectSet::from_labels(["io.read"]).unwrap(),
                    None,
                ),
            ),
        ),
        row(
            11,
            header(vec![AwbcTypeId(1), AwbcTypeId(1)], AwbcTypeId(0)),
        ),
    ];
    // Both tuple members refer to the same source row. The destination
    // can choose that row after the source is fixed, in both variances.
    assert!(program.parameter_contract_accepts_default(
        AwbcTypeId(7),
        1,
        Some(AwbcTypeId(8)),
        AwbcTypeId(5)
    ));
    assert!(!program.parameter_contract_accepts_default(AwbcTypeId(7), 1, None, AwbcTypeId(5)));
    // A row already owned by an earlier input is rigid. An IO callback
    // cannot be a default for every possible earlier row, including empty.
    assert!(!program.parameter_contract_accepts_default(AwbcTypeId(10), 1, None, AwbcTypeId(9)));
}

#[test]
fn shared_effect_bindings_reject_jointly_incompatible_callback_variance() {
    let binder = RuntimeTypeBinder::new(0, 0, 1);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let reference = scope.bound_effect(0, 0).unwrap();
    let function = |parameters, effects| AwbcRuntimeTypeShape::Function {
        contract: RuntimeFunctionTypeContract::new(
            RuntimeTypeBinder::EMPTY,
            EffectPredicate::unconstrained(),
            effects,
        ),
        parameters,
        result: AwbcTypeId(0),
    };
    let row = |marker, shape| {
        AwbcRuntimeType::new(RuntimeSemanticTypeId::from_bytes([marker; 32]), shape)
    };
    let mut program = AwbcProgram::default();
    program.runtime_types = vec![
        row(1, AwbcRuntimeTypeShape::Unit),
        row(
            2,
            function(
                vec![],
                EffectFormula::literal(Default::default(), Some(reference)),
            ),
        )
        .with_scope(scope.clone()),
        row(3, function(vec![], EffectFormula::empty())),
        row(4, function(vec![AwbcTypeId(1)], EffectFormula::empty())).with_scope(scope),
        row(5, function(vec![AwbcTypeId(2)], EffectFormula::empty())),
        row(
            6,
            function(
                vec![],
                EffectFormula::literal(
                    crate::effect_row::EffectSet::from_labels(["io.read"]).unwrap(),
                    None,
                ),
            ),
        ),
        row(
            7,
            AwbcRuntimeTypeShape::Function {
                contract: RuntimeFunctionTypeContract::new(
                    binder,
                    EffectPredicate::unconstrained(),
                    EffectFormula::empty(),
                ),
                parameters: vec![AwbcTypeId(3), AwbcTypeId(1)],
                result: AwbcTypeId(0),
            },
        ),
    ];
    assert!(program.parameter_contract_accepts_types(AwbcTypeId(6), [(0, AwbcTypeId(4))]));
    assert!(program.parameter_contract_accepts_types(AwbcTypeId(6), [(1, AwbcTypeId(5))]));
    assert!(
        !program.parameter_contract_accepts_types(
            AwbcTypeId(6),
            [(0, AwbcTypeId(4)), (1, AwbcTypeId(5))]
        )
    );
    assert!(
        program.parameter_contract_accepts_types(
            AwbcTypeId(6),
            [(0, AwbcTypeId(4)), (1, AwbcTypeId(2))]
        )
    );
    assert!(
        !program.parameter_contract_accepts_types(
            AwbcTypeId(6),
            [(0, AwbcTypeId(4)), (0, AwbcTypeId(4))]
        )
    );
    // An input is an instantiated value, never a declaration-scoped type.
    assert!(!program.parameter_contract_accepts_types(AwbcTypeId(6), [(1, AwbcTypeId(1))]));
    assert!(!program.parameter_contract_accepts_types(AwbcTypeId(2), []));

    let fixed = |effects| RuntimeFunctionEffectInstantiation {
        context: program.runtime_types[6].semantic_identity(),
        effects: vec![effects].into_boxed_slice(),
    };
    let relates = |binding: &RuntimeFunctionEffectInstantiation, expected, actual| {
        let mut matcher = binding.matcher(&program).unwrap();
        let environment = matcher.parameters.clone();
        matcher
            .types(
                expected,
                actual,
                0,
                &environment,
                &ContractEnvironment::root(),
            )
            .is_ok()
            && matcher.accepted()
    };
    let empty = fixed(Default::default());
    let io = fixed(crate::effect_row::EffectSet::from_labels(["io.read"]).unwrap());
    assert!(relates(&empty, AwbcTypeId(1), AwbcTypeId(2)));
    assert!(!relates(&empty, AwbcTypeId(1), AwbcTypeId(5)));
    assert!(relates(&io, AwbcTypeId(1), AwbcTypeId(5)));
    assert!(relates(&empty, AwbcTypeId(3), AwbcTypeId(4)));
    assert!(!relates(&io, AwbcTypeId(3), AwbcTypeId(4)));
    let encoded = serde_json::to_value(&io).unwrap();
    assert_eq!(
        serde_json::from_value::<RuntimeFunctionEffectInstantiation>(encoded.clone()).unwrap(),
        io
    );
    let mut duplicate = encoded;
    duplicate["effects"][0] = serde_json::json!(["io.read", "io.read"]);
    assert!(serde_json::from_value::<RuntimeFunctionEffectInstantiation>(duplicate).is_err());
    assert!(
        !RuntimeFunctionEffectInstantiation {
            context: program.runtime_types[6].semantic_identity(),
            effects: Box::new([]),
        }
        .is_valid(&program)
    );
}
