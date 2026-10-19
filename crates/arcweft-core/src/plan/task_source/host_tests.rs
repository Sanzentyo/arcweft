//! Original host operands, actual backend types and shared retained allowance.

use super::*;
use crate::awbc::schema::{AwbcProgram, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcTypeId};
use crate::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use crate::entry::RuntimeSchemaLimits;
use crate::plan::RuntimeTaskPlanIndex;
use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
use crate::plan::construction::source::host_calls;
use crate::step::RuntimeHostCallMode;
use crate::task::{
    NeedProducerSpec, RuntimeHostArgumentTemplate, RuntimeProgramOwner,
    RuntimeTaskProducerSourceError,
};
use crate::value::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission,
    RuntimeStoredValue, RuntimeValue,
};
use std::cell::Cell;
use std::sync::Arc;

fn native(empty: bool) -> RuntimeTaskSourceProgram {
    RuntimeTaskSourceProgram::native(Arc::new(host_calls::tests::fixture(
        RuntimeHostCallMode::Suspend,
        true,
        true,
        empty,
    )))
}

fn code(program: &RuntimeTaskSourceProgram, include_pair: bool) -> Arc<AwbcProgram> {
    let source = source(program);
    let Some(RuntimeBodyTaskSource::Host(host)) = source.request_endpoint().task_source() else {
        unreachable!()
    };
    let types = program.original().type_table();
    let mut runtime_types = vec![AwbcRuntimeType::new(
        types.get(host.result).unwrap().semantic_identity(),
        AwbcRuntimeTypeShape::Bool,
    )];
    if include_pair {
        runtime_types.push(AwbcRuntimeType::new(
            types
                .get(host.args[2].value().ty())
                .unwrap()
                .semantic_identity(),
            AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0), AwbcTypeId(0)]),
        ));
    }
    Arc::new(AwbcProgram {
        original_source: Some(
            program
                .original()
                .task_source_artifact(Default::default())
                .unwrap(),
        ),
        runtime_types,
        ..AwbcProgram::default()
    })
}

fn awbc(code: Arc<AwbcProgram>) -> RuntimeTaskSourceProgram {
    RuntimeTaskSourceProgram::admit_awbc(
        code,
        AwbcVerifyBudget::default(),
        AwbcVerifyContext {
            require_entrypoint: false,
            ..AwbcVerifyContext::default()
        },
        RuntimeTaskSourceArtifactLimits::default(),
        None,
    )
    .unwrap()
}

fn source(program: &RuntimeTaskSourceProgram) -> RuntimeTaskSourceRef<'_> {
    let key = program
        .original()
        .task_plans()
        .by_index(RuntimeTaskPlanIndex(0))
        .unwrap()
        .key();
    program
        .original()
        .task_source_by_expected_key(
            &crate::task::ExpectedTaskPlanKey::new(*key.as_bytes()),
            RuntimeTaskPlanSealLimits::default(),
            &mut TaskSemanticMeter::new(1_000_000, 0),
        )
        .unwrap()
}

fn values() -> [RuntimeValue; 3] {
    [
        RuntimeValue::Bool(true),
        RuntimeValue::Bool(false),
        RuntimeValue::Tuple(vec![RuntimeValue::Bool(false), RuntimeValue::Bool(true)]),
    ]
}

fn arguments<V>(
    program: &RuntimeTaskSourceProgram,
    values: impl IntoIterator<Item = V>,
) -> Vec<RuntimeHostArgumentTemplate<V>> {
    let source = source(program);
    let originals = source.request_endpoint().host_arguments().unwrap();
    originals
        .iter()
        .zip(values)
        .map(|(original, value)| match original {
            RuntimeHostArgumentTemplate::Positional(role, _) => {
                RuntimeHostArgumentTemplate::positional(*role, value)
            }
            RuntimeHostArgumentTemplate::Named(role, argument) => {
                RuntimeHostArgumentTemplate::named(*role, &argument.name, value)
            }
            RuntimeHostArgumentTemplate::Spread(role, _) => {
                RuntimeHostArgumentTemplate::spread(*role, value)
            }
        })
        .collect()
}

fn bank(
    program: &RuntimeTaskSourceProgram,
    limits: RuntimeSchemaLimits,
) -> RuntimeRetainedValueAdmission {
    RuntimeRetainedValueAdmission::for_program(program.program_owner(), limits)
}

#[test]
fn host_source_uses_the_completed_key_and_every_original_operand_before_spread() {
    for empty in [false, true] {
        let program = native(empty);
        let source = source(&program);
        let operands = arguments(&program, values());
        let spec = source
            .host_producer_spec(
                &program,
                &operands,
                None,
                &mut bank(&program, RuntimeSchemaLimits::engine_default()),
            )
            .unwrap();
        let Some(RuntimeBodyTaskSource::Host(host)) = source.request_endpoint().task_source()
        else {
            unreachable!()
        };
        assert_eq!(spec.source().task_plan().key(), source.task_plan().key());
        assert_eq!(spec.family(), source.task_plan().definition().family());
        assert_eq!(spec.contract(), host.producer.contract);
        assert_eq!(spec.producer_site(), host.producer.site);
        assert_eq!(
            spec.payload_type().as_bytes(),
            program
                .original()
                .type_table()
                .get(host.result)
                .unwrap()
                .semantic_identity()
                .as_bytes()
        );
        let views: Vec<_> = operands
            .iter()
            .map(|argument| argument.value().view())
            .collect();
        let digest = crate::entry::schema::canonical_runtime_value_view_digest(
            crate::value::RuntimeValueView::Tuple(crate::value::RuntimeTupleView::Views(&views)),
            16 * 1024 * 1024,
        )
        .unwrap();
        assert_eq!(spec.arguments(), digest);
        assert_eq!(
            spec.instance_key().unwrap(),
            NeedProducerSpec::new(
                spec.family(),
                spec.contract(),
                source.task_plan().key(),
                spec.producer_site(),
                spec.payload_type(),
                digest
            )
            .instance_key()
            .unwrap()
        );
        if !empty {
            let changed = arguments(
                &program,
                [
                    RuntimeValue::Bool(false),
                    RuntimeValue::Bool(false),
                    RuntimeValue::Tuple(vec![RuntimeValue::Bool(false), RuntimeValue::Bool(true)]),
                ],
            );
            let changed = source
                .host_producer_spec(
                    &program,
                    &changed,
                    None,
                    &mut bank(&program, RuntimeSchemaLimits::engine_default()),
                )
                .unwrap();
            assert_ne!(
                changed.instance_key().unwrap(),
                spec.instance_key().unwrap()
            );
        }
    }
}

#[test]
fn host_source_survives_canonical_awbc_source_admission_with_its_real_execution_owner() {
    let native = native(false);
    let code = code(&native, true);
    let encoded = code.encode_canonical().unwrap();
    let decoded = Arc::new(AwbcProgram::decode_canonical(&encoded, Default::default()).unwrap());
    let compiled = awbc(Arc::clone(&decoded));
    let produce = |program: &RuntimeTaskSourceProgram| {
        source(program)
            .host_producer_spec(
                program,
                &arguments(program, values()),
                None,
                &mut bank(program, RuntimeSchemaLimits::engine_default()),
            )
            .unwrap()
    };
    let original = produce(&native);
    let compiled_spec = produce(&compiled);
    assert_eq!(
        original.instance_key().unwrap(),
        compiled_spec.instance_key().unwrap()
    );
    assert_eq!(
        original.source().task_plan().key(),
        compiled_spec.source().task_plan().key()
    );
    assert_ne!(original.source(), compiled_spec.source());
    assert!(
        matches!(compiled_spec.source().program_owner(), RuntimeProgramOwner::Awbc(owner) if Arc::ptr_eq(&owner, &decoded))
    );
    let weak = Arc::downgrade(&decoded);
    drop(decoded);
    drop(compiled);
    assert!(weak.upgrade().is_some());
    assert_eq!(
        compiled_spec
            .source()
            .borrow(
                RuntimeTaskPlanSealLimits::default(),
                &mut TaskSemanticMeter::new(1_000_000, 0)
            )
            .unwrap()
            .task_plan()
            .key(),
        original.source().task_plan().key()
    );
    drop(compiled_spec);
    assert!(weak.upgrade().is_none());
}

struct Observed<'a> {
    reads: &'a Cell<usize>,
    value: &'a RuntimeValue,
}
impl RuntimeStoredValue for Observed<'_> {
    fn borrowed_view(&self) -> crate::value::RuntimeValueView<'_> {
        self.reads.set(self.reads.get() + 1);
        self.value.view()
    }
    fn admission_origin() -> crate::entry::schema::value_encoding::ValueAdmissionOrigin {
        crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live
    }
}

#[test]
fn host_source_checks_all_operand_roles_names_and_count_before_the_first_value_view() {
    let program = native(false);
    let source = source(&program);
    let values = values();
    let reads = Cell::new(0);
    for change in 0..4 {
        let mut operands = arguments(
            &program,
            values.iter().map(|value| Observed {
                reads: &reads,
                value,
            }),
        );
        match change {
            0 => {
                operands.pop();
            }
            1 => {
                let RuntimeHostArgumentTemplate::Named(_, named) = &mut operands[1] else {
                    unreachable!()
                };
                named.name = "別名".into();
            }
            2 => {
                let RuntimeHostArgumentTemplate::Spread(role, _) = &mut operands[2] else {
                    unreachable!()
                };
                *role = crate::task::RuntimeRequestRoleIdentity::from_accepted_identity([0xff; 32]);
            }
            3 => {
                let RuntimeHostArgumentTemplate::Spread(role, value) = operands.pop().unwrap()
                else {
                    unreachable!()
                };
                operands.push(RuntimeHostArgumentTemplate::positional(role, value));
            }
            _ => unreachable!(),
        }
        let error = source
            .host_producer_spec(
                &program,
                &operands,
                None,
                &mut bank(&program, RuntimeSchemaLimits::engine_default()),
            )
            .unwrap_err();
        match change {
            0 => assert!(matches!(
                error,
                RuntimeTaskProducerSourceError::ArgumentCount {
                    actual: 2,
                    expected: 3
                }
            )),
            1 => assert!(matches!(
                error,
                RuntimeTaskProducerSourceError::ArgumentName { index: 1 }
            )),
            2 | 3 => assert!(matches!(
                error,
                RuntimeTaskProducerSourceError::ArgumentRole { index: 2 }
            )),
            _ => unreachable!(),
        }
        assert_eq!(reads.get(), 0);
    }
}

#[test]
fn host_source_rejects_foreign_programs_before_operand_views() {
    let program = native(false);
    let compiled = awbc(code(&program, true));
    let values = values();
    let reads = Cell::new(0);
    let operands = arguments(
        &compiled,
        values.iter().map(|value| Observed {
            reads: &reads,
            value,
        }),
    );
    let actual_code = match compiled.program_owner() {
        RuntimeProgramOwner::Awbc(code) => code,
        _ => unreachable!(),
    };
    for owner in [
        program.program_owner(),
        native(false).program_owner(),
        RuntimeProgramOwner::Awbc(Arc::new((*actual_code).clone())),
    ] {
        let mut wrong_bank = RuntimeRetainedValueAdmission::for_program(
            owner,
            RuntimeSchemaLimits::engine_default(),
        );
        assert!(matches!(
            source(&compiled).host_producer_spec(&compiled, &operands, None, &mut wrong_bank),
            Err(RuntimeTaskProducerSourceError::OriginalProgram)
        ));
    }
    assert!(matches!(
        source(&compiled).host_producer_spec(
            &compiled,
            &operands,
            None,
            &mut RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimeTaskProducerSourceError::OriginalProgram)
    ));
    assert!(matches!(
        source(&program).host_producer_spec(
            &compiled,
            &operands,
            None,
            &mut bank(&compiled, RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimeTaskProducerSourceError::OriginalProgram)
    ));
    assert_eq!(reads.get(), 0);
}

#[test]
fn host_source_checks_original_types_against_the_actual_backend_not_a_native_surrogate() {
    let native = native(false);
    let compiled = awbc(code(&native, false));
    let error = source(&compiled)
        .host_producer_spec(
            &compiled,
            &arguments(&compiled, values()),
            None,
            &mut bank(&compiled, RuntimeSchemaLimits::engine_default()),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeTaskProducerSourceError::Admission(AwbcRuntimeValueSnapshotError::ProgramType(_))
    ));
    let wrong = arguments(
        &native,
        [
            RuntimeValue::String("wrong".into()),
            RuntimeValue::Bool(false),
            RuntimeValue::Tuple(vec![RuntimeValue::Bool(false), RuntimeValue::Bool(true)]),
        ],
    );
    assert!(matches!(
        source(&native).host_producer_spec(
            &native,
            &wrong,
            None,
            &mut bank(&native, RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimeTaskProducerSourceError::Admission(
            AwbcRuntimeValueSnapshotError::ProgramType(_)
        ))
    ));
}

#[test]
fn host_source_uses_one_value_bank_for_live_inert_exact_work_and_terminal_failure() {
    let native = native(false);
    for program in [native.clone(), awbc(code(&native, true))] {
        let limits = RuntimeSchemaLimits::engine_default();
        let operands = arguments(&program, values());
        let mut measured = bank(&program, limits);
        let expected = source(&program)
            .host_producer_spec(&program, &operands, None, &mut measured)
            .unwrap();
        let work = measured.work().consumed();
        assert!(work > 0);
        let inert = arguments(
            &program,
            values()
                .iter()
                .map(|value| {
                    AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
                        value,
                        &program.program_owner(),
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            source(&program)
                .host_producer_spec(&program, &inert, None, &mut measured)
                .unwrap(),
            expected
        );
        assert!(measured.work().consumed() > work);
        assert!(
            measured
                .program_owner()
                .unwrap()
                .same_program(&program.program_owner())
        );
        let mut exact = bank(
            &program,
            RuntimeSchemaLimits {
                max_validation_work: work,
                ..limits
            },
        );
        assert_eq!(
            source(&program)
                .host_producer_spec(&program, &operands, None, &mut exact)
                .unwrap(),
            expected
        );
        assert_eq!(exact.work().consumed(), work);
        let mut short = bank(
            &program,
            RuntimeSchemaLimits {
                max_validation_work: work - 1,
                ..limits
            },
        );
        let mut previous = None;
        for _ in 0..2 {
            let RuntimeTaskProducerSourceError::Admission(error) = source(&program)
                .host_producer_spec(&program, &operands, None, &mut short)
                .unwrap_err()
            else {
                panic!("the original bank must report the resource failure")
            };
            assert_eq!(short.status(), Err(error.clone()));
            assert!(short.work().failure().is_some());
            assert_eq!(short.work().consumed(), work);
            if let Some(previous) = &previous {
                assert_eq!(&error, previous);
            }
            previous = Some(error);
        }
        let mut prefixed = bank(
            &program,
            RuntimeSchemaLimits {
                max_validation_work: work + 1,
                ..limits
            },
        );
        prefixed.validate_metadata(&17_u64).unwrap();
        assert_eq!(
            source(&program)
                .host_producer_spec(&program, &operands, None, &mut prefixed)
                .unwrap(),
            expected
        );
        assert_eq!(prefixed.work().consumed(), work + 1);
    }
}

#[test]
fn host_source_rejects_collection_limits_and_a_selected_request_without_reading_values() {
    let program = native(false);
    let values = values();
    let reads = Cell::new(0);
    let operands = arguments(
        &program,
        values.iter().map(|value| Observed {
            reads: &reads,
            value,
        }),
    );
    for limits in [
        RuntimeSchemaLimits {
            max_sequence_items: 2,
            ..RuntimeSchemaLimits::engine_default()
        },
        RuntimeSchemaLimits {
            max_validation_work: 0,
            ..RuntimeSchemaLimits::engine_default()
        },
    ] {
        let mut bank = bank(&program, limits);
        for _ in 0..2 {
            assert!(matches!(
                source(&program).host_producer_spec(&program, &operands, None, &mut bank),
                Err(RuntimeTaskProducerSourceError::Admission(
                    AwbcRuntimeValueSnapshotError::Resource(_)
                ))
            ));
        }
    }
    let selected = RuntimeTaskSourceProgram::native(Arc::new(
        crate::plan::construction::source::selected_producers::tests::fixture(
            2,
            crate::task::TaskPolicy::JoinSameKey,
            crate::task::HostRestartPolicy::MustBeQuiescent,
            crate::task::TaskClass::Io,
        ),
    ));
    assert!(matches!(
        source(&selected).host_producer_spec(
            &selected,
            &operands,
            None,
            &mut bank(&selected, RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimeTaskProducerSourceError::RequestKind)
    ));
    assert_eq!(reads.get(), 0);
}
