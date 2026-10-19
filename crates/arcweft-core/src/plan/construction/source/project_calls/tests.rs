use super::super::callable_states::tests as graph;
use super::super::expression::{
    RuntimeExpressionSourceHandles,
    tests::{decode, limits as expression_limits},
};
use super::super::pattern::tests::{meter, resources};
use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::*;
use crate::value::{RuntimeCallArgumentMode as Mode, RuntimeValue};

fn ty(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}
fn boolean(value: bool) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        ty(121),
        RuntimeExprSeedKind::Value(RuntimeValue::Bool(value)),
    )
}
fn operand(value: RuntimeExprSeed, mode: Mode) -> RuntimeProjectCallOperandSeed {
    RuntimeProjectCallOperandSeed { value, mode }
}
fn fixed(parameter: u32, source_index: u32) -> RuntimeProjectCallOrdinaryMaterializationSeed {
    RuntimeProjectCallOrdinaryMaterializationSeed::Fixed(
        RuntimeProjectCallFixedMaterializationSeed {
            parameter,
            abi_ty: ty(121),
            binding_ty: ty(121),
            source_index,
        },
    )
}
fn rest(sources: &[u32]) -> RuntimeProjectCallOrdinaryMaterializationSeed {
    RuntimeProjectCallOrdinaryMaterializationSeed::Rest(RuntimeProjectCallRestMaterializationSeed {
        parameter: 0,
        abi_ty: ty(121),
        binding_ty: ty(128),
        source_indices: sources.into(),
    })
}
fn call(
    state: &RuntimeCallableStateSeedId,
    function_type: u8,
    captures: &[bool],
    group: u32,
    operands: Vec<RuntimeProjectCallOperandSeed>,
    ordinary: Vec<RuntimeProjectCallOrdinaryMaterializationSeed>,
    attached: Option<RuntimeProjectCallAttachedMaterializationSeed>,
) -> RuntimeFlowOpSeed {
    RuntimeFlowOpSeed::ProjectCall {
        plan: RuntimeProjectCallPlanSeed {
            placement: crate::plan::RuntimeProjectCallSitePlacementSeed::Allocate,
            callee: RuntimeExprSeed::new(
                ty(function_type),
                RuntimeExprSeedKind::MakeCallable {
                    state: state.clone(),
                    captures: captures.iter().map(|&v| boolean(v)).collect(),
                },
            ),
            state: state.clone(),
            completed_group: group,
            operands: operands.into_boxed_slice(),
            ordinary: ordinary.into_boxed_slice(),
            attached,
        },
        result: RuntimePatternSeed::new(ty(121), RuntimePatternSeedKind::Discard),
    }
}
fn fixture() -> RuntimePlan {
    let (mut builder, functions, states) = graph::fixture_builder();
    graph::define_bodies(&mut builder, &functions, &states);
    let root = graph::reserve_executable_function(&mut builder, 231, &[], 0, ty(121));
    let mut ops = vec![
        call(
            &states[0],
            123,
            &[],
            0,
            vec![
                operand(boolean(true), Mode::Value),
                operand(boolean(false), Mode::Value),
            ],
            vec![fixed(0, 1), fixed(1, 0)],
            None,
        ),
        call(
            &states[3],
            122,
            &[true],
            1,
            vec![operand(boolean(false), Mode::Value)],
            vec![fixed(0, 0)],
            None,
        ),
        call(&states[9], 122, &[], 0, vec![], vec![rest(&[])], None),
        call(
            &states[9],
            122,
            &[],
            0,
            vec![
                operand(boolean(true), Mode::Value),
                operand(
                    RuntimeExprSeed::new(
                        ty(128),
                        RuntimeExprSeedKind::BracketSeq(vec![boolean(false)].into_boxed_slice()),
                    ),
                    Mode::Spread,
                ),
            ],
            vec![rest(&[0, 1])],
            None,
        ),
    ];
    use RuntimeProjectCallAttachedPresenceSeed as Presence;
    for (state, presence, present) in [
        (4, Presence::RequiredPresent, true),
        (5, Presence::OptionalPresent, true),
        (5, Presence::OptionalOmitted, false),
        (6, Presence::DefaultedPresent, true),
        (6, Presence::DefaultedOmitted, false),
        (7, Presence::DefaultedOmitted, false),
    ] {
        let abi_ty = if state == 4 { ty(121) } else { ty(127) };
        let binding_ty = if state == 5 { ty(127) } else { ty(121) };
        ops.push(call(
            &states[state],
            125,
            if state == 7 { &[false] } else { &[] },
            0,
            if present {
                vec![operand(boolean(true), Mode::Value)]
            } else {
                vec![]
            },
            vec![],
            Some(RuntimeProjectCallAttachedMaterializationSeed {
                abi_ty,
                binding_ty,
                source_index: present.then_some(0),
                presence,
            }),
        ));
    }
    ops.push(RuntimeFlowOpSeed::ReturnExpr(boolean(true)));
    builder
        .define_function_site_seed(
            &root,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: ops.into_boxed_slice(),
            }),
        )
        .unwrap();
    builder.finish().unwrap()
}

pub(in crate::plan::construction::source) fn differently_defined_body_fixture() -> RuntimePlan {
    let (mut builder, functions, states) = graph::fixture_builder();
    graph::define_bodies(&mut builder, &functions, &states);
    let first = graph::reserve_executable_function(&mut builder, 231, &[], 0, ty(121));
    let second = graph::reserve_executable_function(&mut builder, 232, &[], 0, ty(121));
    for (function, input) in [(second, false), (first, true)] {
        let op = call(
            &states[0],
            123,
            &[],
            0,
            vec![
                operand(boolean(input), Mode::Value),
                operand(boolean(!input), Mode::Value),
            ],
            vec![fixed(0, 0), fixed(1, 1)],
            None,
        );
        builder
            .define_function_site_seed(
                &function,
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: vec![op, RuntimeFlowOpSeed::ReturnExpr(boolean(input))].into_boxed_slice(),
                }),
            )
            .unwrap();
    }
    builder.finish().unwrap()
}

#[test]
fn project_site_reservations_reject_foreign_and_duplicate_definitions_without_publishing_a_plan() {
    for foreign in [true, false] {
        let (mut builder, functions, states) = graph::fixture_builder();
        graph::define_bodies(&mut builder, &functions, &states);
        let root = graph::reserve_executable_function(&mut builder, 233, &[], 0, ty(121));
        let mut other = RuntimePlanBuilder::new();
        let reservation = if foreign {
            other.reserve_project_call_site().unwrap()
        } else {
            builder.reserve_project_call_site().unwrap()
        };
        let mut operation = call(
            &states[0],
            123,
            &[],
            0,
            vec![
                operand(boolean(true), Mode::Value),
                operand(boolean(false), Mode::Value),
            ],
            vec![fixed(0, 0), fixed(1, 1)],
            None,
        );
        let RuntimeFlowOpSeed::ProjectCall { plan, .. } = &mut operation else {
            unreachable!()
        };
        plan.placement = RuntimeProjectCallSitePlacementSeed::Reserved(reservation.clone());
        let ops = if foreign {
            vec![operation]
        } else {
            vec![operation.clone(), operation]
        };
        let error = builder
            .define_function_site_seed(
                &root,
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: ops.into_boxed_slice(),
                }),
            )
            .unwrap_err();
        if foreign {
            assert!(matches!(
                error,
                RuntimePlanBuildError::ProjectCallSites(
                    RuntimeProjectCallSiteTableError::ForeignReservation
                )
            ));
        } else {
            assert!(
                matches!(error,RuntimePlanBuildError::ProjectCallSites(RuntimeProjectCallSiteTableError::DuplicateDefinition { site }) if site == reservation.id())
            );
        }
        assert_eq!(builder.finish(), Err(RuntimePlanBuildError::Poisoned));
    }
}

#[test]
fn project_site_reservations_must_be_defined_before_the_common_plan_can_be_published() {
    let (mut builder, functions, states) = graph::fixture_builder();
    graph::define_bodies(&mut builder, &functions, &states);
    let site = builder.reserve_project_call_site().unwrap();
    assert!(
        matches!(builder.finish(),Err(RuntimePlanBuildError::ProjectCallSites(RuntimeProjectCallSiteTableError::IncompleteDefinition { site:actual })) if actual == site.id())
    );
}
fn limits(plan: &RuntimePlan) -> RuntimeProjectCallSourceLimits {
    RuntimeProjectCallSourceLimits {
        nodes: 100_000,
        elements: 1_000_000,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        expression: expression_limits(plan),
    }
}
fn encode(row: &RuntimeProjectCallSite, plan: &RuntimePlan) -> Vec<u8> {
    row.encode_source(limits(plan), &resources(), &mut meter())
        .unwrap()
        .into_bytes()
}
fn source(
    plan: &RuntimePlan,
    bank: TaskSemanticMeter,
) -> (
    super::super::RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
    Box<[RuntimeCallableStateSeedId]>,
) {
    graph::source_with_meter(plan, &graph::encode(plan), bank)
}
fn handles(states: &[RuntimeCallableStateSeedId]) -> RuntimeExpressionSourceHandles<'_> {
    RuntimeExpressionSourceHandles {
        states,
        specializations: &[],
        helpers: &[],
        methods: &[],
        attempts: &[],
    }
}
fn replay(
    plan: &RuntimePlan,
    source: &mut super::super::RuntimePlanSourceConstruction,
    functions: &[RuntimeFunctionSiteSeedId],
    states: &[RuntimeCallableStateSeedId],
    bad_duplicate: bool,
) -> Result<(), RuntimePlanBuildError> {
    for (function, handle) in plan.function_sites().iter().zip(functions) {
        let body = match function.body() {
            RuntimeFunctionSiteBody::Expression(expr) => RuntimeFunctionSiteBodySeed::Expression(
                decode(expr, plan, source, &handles(states)),
            ),
            RuntimeFunctionSiteBody::Executable(body) => {
                let mut ops = Vec::new();
                for op in body.ops() {
                    ops.push(match op {
                        FlowOp::ProjectCall { site } => {
                            let row = plan.project_call_sites().get(*site).unwrap();
                            let bytes = encode(row, plan);
                            let prepared = PreparedRuntimeProjectCallSource::preflight(
                                &bytes,
                                limits(plan),
                                &resources(),
                                &mut source.meter,
                            )
                            .unwrap();
                            let mut decoded = prepared
                                .materialize(source, &handles(states), &mut resources())
                                .unwrap();
                            if bad_duplicate && site.index() == 1 {
                                let RuntimeFlowOpSeed::ProjectCall { plan, .. } = &mut decoded
                                else {
                                    unreachable!()
                                };
                                // The first real call is retained by the actual factory before
                                // this malformed later call fails its original ABI admission.
                                plan.operands = vec![
                                    operand(boolean(false), Mode::Value),
                                    operand(boolean(true), Mode::Value),
                                ]
                                .into_boxed_slice();
                            }
                            decoded
                        }
                        FlowOp::ReturnExpr(expr) => RuntimeFlowOpSeed::ReturnExpr(decode(
                            expr,
                            plan,
                            source,
                            &handles(states),
                        )),
                        _ => panic!("fixture contains only project calls and return"),
                    });
                }
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: body.effects().clone(),
                    ops: ops.into_boxed_slice(),
                })
            }
        };
        source.builder.define_function_site_seed(handle, body)?;
    }
    Ok(())
}
fn finish(
    source: super::super::RuntimePlanSourceConstruction,
) -> Result<RuntimePlan, crate::plan::body_semantic::task_image::RuntimeTaskPlanSealError> {
    let seal = RuntimeTaskPlanSealLimits::default();
    Ok(source.prepare_image(seal)?.seal(seal, None)?.into_plan())
}
#[test]
fn project_call_sources_restore_original_order_rest_all_attached_forms_defaults_captures_and_semantics()
 {
    let plan = fixture();
    assert_eq!(plan.project_call_sites().len(), 10);
    let (mut source, functions, states) = source(&plan, meter());
    replay(&plan, &mut source, &functions, &states, false).unwrap();
    let restored = finish(source).unwrap();
    assert_eq!(restored.project_call_sites(), plan.project_call_sites());
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(restored.callable_states(), plan.callable_states());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    for (original, restored) in plan
        .project_call_sites()
        .iter()
        .zip(restored.project_call_sites().iter())
    {
        assert_eq!(encode(original, &plan), encode(restored, &plan));
    }
}
#[test]
fn project_call_sources_share_exact_aggregate_limits_and_reject_one_less() {
    let plan = fixture();
    for row in plan.project_call_sites().iter() {
        let encoded = row
            .encode_source(limits(&plan), &resources(), &mut meter())
            .unwrap();
        let bytes = encoded.bytes();
        let prepared = PreparedRuntimeProjectCallSource::preflight(
            bytes,
            limits(&plan),
            &resources(),
            &mut meter(),
        )
        .unwrap();
        assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
        assert_eq!(prepared.elements(), encoded.elements());
        assert_eq!(prepared.node_count(), encoded.node_count());
        let exact = RuntimeProjectCallSourceLimits {
            nodes: prepared.node_count() as u32,
            elements: prepared.elements() as u32,
            encoded_bytes: bytes.len(),
            decoded_bytes: prepared.storage_bytes(),
            ..limits(&plan)
        };
        PreparedRuntimeProjectCallSource::preflight(bytes, exact, &resources(), &mut meter())
            .unwrap();
        row.encode_source(exact, &resources(), &mut meter())
            .unwrap();
        for short in [
            RuntimeProjectCallSourceLimits {
                nodes: exact.nodes - 1,
                ..exact
            },
            RuntimeProjectCallSourceLimits {
                elements: exact.elements - 1,
                ..exact
            },
            RuntimeProjectCallSourceLimits {
                encoded_bytes: exact.encoded_bytes - 1,
                ..exact
            },
            RuntimeProjectCallSourceLimits {
                decoded_bytes: exact.decoded_bytes - 1,
                ..exact
            },
        ] {
            let mut bank = meter();
            assert!(
                PreparedRuntimeProjectCallSource::preflight(bytes, short, &resources(), &mut bank)
                    .is_err()
            );
            assert!(bank.status().is_err());
            assert!(
                row.encode_source(short, &resources(), &mut meter())
                    .is_err()
            );
        }
    }
}
#[test]
fn project_call_sources_reject_every_truncated_prefix_bad_tags_refs_source_order_and_trailing_data()
{
    let plan = fixture();
    for row in plan.project_call_sites().iter() {
        let bytes = encode(row, &plan);
        for end in 0..bytes.len() {
            assert!(
                PreparedRuntimeProjectCallSource::preflight(
                    &bytes[..end],
                    limits(&plan),
                    &resources(),
                    &mut meter()
                )
                .is_err(),
                "prefix {end}"
            );
        }
        for (offset, value) in [(0, 0), (DOMAIN.len(), 2), (DOMAIN.len() + 1, 0)] {
            let mut bad = bytes.clone();
            bad[offset] = value;
            assert!(
                PreparedRuntimeProjectCallSource::preflight(
                    &bad,
                    limits(&plan),
                    &resources(),
                    &mut meter()
                )
                .is_err()
            );
        }
        let mut bad = bytes;
        bad.push(0);
        assert!(matches!(
            PreparedRuntimeProjectCallSource::preflight(
                &bad,
                limits(&plan),
                &resources(),
                &mut meter()
            ),
            Err(Error::Trailing { .. })
        ));
    }
    let row = plan.project_call_sites().iter().nth(3).unwrap();
    let bytes = encode(row, &plan);
    let mut cursor = cursor::Cursor::new(&bytes, limits(&plan));
    let row = cursor.row().unwrap();
    let operand = row.operands.as_ptr() as usize - bytes.as_ptr() as usize;
    let ordinary = row.ordinary.as_ptr() as usize - bytes.as_ptr() as usize;
    for (at, value) in [
        (operand, 2),
        (ordinary, 1),
        (ordinary + 4, 0),
        (ordinary + 12, 2),
        (ordinary + 17, 2),
        (ordinary + 21, 0),
    ] {
        let mut bad = bytes.clone();
        bad[at] = value;
        assert!(
            PreparedRuntimeProjectCallSource::preflight(
                &bad,
                limits(&plan),
                &resources(),
                &mut meter()
            )
            .is_err(),
            "offset {at}"
        );
    }
}
#[test]
fn project_call_sources_reject_invalid_attached_presence_flags_and_operand_references() {
    let plan = fixture();
    for row in plan.project_call_sites().iter().skip(4) {
        let bytes = encode(row, &plan);
        let mut cursor = cursor::Cursor::new(&bytes, limits(&plan));
        let view = cursor.row().unwrap();
        let at = view.ordinary.as_ptr() as usize - bytes.as_ptr() as usize + view.ordinary.len();
        for (offset, value) in [(at, 2), (at + 9, 255), (at + 10, 2)] {
            let mut bad = bytes.clone();
            bad[offset] = value;
            assert!(
                PreparedRuntimeProjectCallSource::preflight(
                    &bad,
                    limits(&plan),
                    &resources(),
                    &mut meter()
                )
                .is_err()
            );
        }
        let mut bad = bytes.clone();
        bad[at + 9] = if view.attached.unwrap().source.is_some() {
            4
        } else {
            0
        };
        assert!(
            PreparedRuntimeProjectCallSource::preflight(
                &bad,
                limits(&plan),
                &resources(),
                &mut meter()
            )
            .is_err()
        );
        if row.plan().attached().unwrap().source_index().is_some() {
            let mut bad = bytes;
            bad[at + 11..at + 15].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(matches!(
                PreparedRuntimeProjectCallSource::preflight(
                    &bad,
                    limits(&plan),
                    &resources(),
                    &mut meter()
                ),
                Err(Error::Reference {
                    kind: "physical operand",
                    ..
                })
            ));
        }
    }
}
#[test]
fn project_call_sources_authenticate_original_state_handles_and_missing_namespaces_before_retaining_sites()
 {
    let plan = fixture();
    let bytes = encode(plan.project_call_sites().iter().next().unwrap(), &plan);
    for kind in 0..3 {
        let (mut source, _, mut states) = source(&plan, meter());
        match kind {
            0 => states.swap(0, 1),
            1 => {
                let (_, _, other) = self::source(&plan, meter());
                states = other;
            }
            _ => states = Box::new([]),
        }
        let prepared = PreparedRuntimeProjectCallSource::preflight(
            &bytes,
            limits(&plan),
            &resources(),
            &mut source.meter,
        )
        .unwrap();
        assert!(
            prepared
                .materialize(&mut source, &handles(&states), &mut resources())
                .is_err()
        );
        assert_eq!(source.builder.project_call_sites.borrow().rows.len(), 0);
        assert!(source.meter.status().is_err());
        assert!(finish(source).is_err());
    }
}
#[test]
fn project_call_sources_late_actual_abi_failure_cannot_publish_the_prior_retained_site() {
    let plan = fixture();
    let (mut source, functions, states) = source(&plan, meter());
    assert!(replay(&plan, &mut source, &functions, &states, true).is_err());
    {
        let sites = source.builder.project_call_sites.borrow();
        assert_eq!(sites.rows.len(), 2);
        assert!(sites.rows[0].is_some());
        assert!(sites.rows[1].is_none());
    }
    assert!(finish(source).is_err());
}
#[test]
fn project_call_sources_preserve_the_original_finite_work_bank_and_reserved_output_prefix() {
    let plan = fixture();
    let row = plan.project_call_sites().iter().next().unwrap();
    let bytes = encode(row, &plan);
    let (trial, _, _) = source(&plan, meter());
    let used = trial.meter.totals().0;
    let (mut source, _, _) = source(&plan, TaskSemanticMeter::new(used + 1, u64::MAX));
    assert!(matches!(
        PreparedRuntimeProjectCallSource::preflight(
            &bytes,
            limits(&plan),
            &resources(),
            &mut source.meter
        ),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
    assert!(finish(source).is_err());
    let encoded = row
        .encode_source(limits(&plan), &resources(), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(encoded.bytes().len());
    output.push(83);
    assert!(matches!(
        encoded.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(output, [83]);
}
