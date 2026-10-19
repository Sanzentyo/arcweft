use super::*;
use crate::awbc::schema::{
    AwbcEffectSet, AwbcEffectSetId, AwbcHostArgument, AwbcHostCall, AwbcProgram, AwbcRuntimeType,
    AwbcRuntimeTypeShape, AwbcSignature, AwbcSignatureId, AwbcStringId, AwbcTypeId,
};
use crate::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use crate::entry::RuntimeSchemaLimits;
use crate::plan::construction::source::{host_calls, selected_producers};
use crate::plan::task_source::RuntimeTaskSourceArtifactLimits;
use crate::plan::{FlowOp, RuntimePlan, RuntimePlanTypeProjection, RuntimeTaskPlanIndex};
use crate::task::{
    HostCallProducerDefinition, HostRestartPolicy, RuntimeHostArgumentTemplate, TaskClass,
    TaskPlanSemanticDigest, TaskPolicy,
};
use crate::value::{RuntimeRetainedValueAdmission, RuntimeValue};
use std::sync::Arc;

fn original(mode: RuntimeHostCallMode, empty: bool) -> RuntimePlan {
    host_calls::tests::fixture(mode, true, true, empty)
}

fn key(plan: &RuntimePlan) -> TaskPlanSemanticDigest {
    plan.task_plans()
        .by_index(RuntimeTaskPlanIndex(0))
        .unwrap()
        .key()
}

pub(in crate::plan::task_source) fn executable(
    mode: RuntimeHostCallMode,
    empty: bool,
) -> AwbcProgram {
    let plan = original(mode, empty);
    let FlowOp::HostCall { target, .. } = &plan
        .function_sites()
        .iter()
        .next()
        .unwrap()
        .body()
        .executable()
        .unwrap()
        .ops()[0]
    else {
        unreachable!()
    };
    let mut strings = vec![
        target.public_id.clone(),
        target.capability.clone(),
        target.operation.clone(),
        "別".to_owned(),
    ];
    strings.extend(
        target
            .args
            .iter()
            .filter_map(|argument| argument.name().map(str::to_owned)),
    );
    strings.sort();
    strings.dedup();
    let string = |value: &str| {
        AwbcStringId(
            u32::try_from(strings.iter().position(|actual| actual == value).unwrap()).unwrap(),
        )
    };
    let types = plan.type_table();
    let pair = types
        .declarations()
        .find(|row| matches!(row.projection(), RuntimePlanTypeProjection::Tuple(_)))
        .unwrap()
        .semantic_identity();
    let boolean = crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest();
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let row = AwbcHostCall {
        producer: HostCallProducerDefinition {
            contract: target.producer.contract,
            plan: key(&plan),
            site: target.producer.site,
        },
        public_id: string(&target.public_id),
        capability: string(&target.capability),
        operation: string(&target.operation),
        contract: target.contract,
        signature: AwbcSignatureId(0),
        mode: match mode {
            RuntimeHostCallMode::Immediate => AwbcHostCallMode::Immediate,
            RuntimeHostCallMode::Suspend => AwbcHostCallMode::Suspend,
        },
        deterministic: target.deterministic,
        arguments: target
            .args
            .iter()
            .map(|argument| AwbcHostArgument {
                name: argument.name().map(string),
                spread: argument.is_spread(),
            })
            .collect(),
    };
    let params = target
        .args
        .iter()
        .map(
            |argument| match types.get(argument.value().ty()).unwrap().projection() {
                RuntimePlanTypeProjection::Bool => AwbcTypeId(0),
                RuntimePlanTypeProjection::Tuple(_) => AwbcTypeId(1),
                _ => unreachable!(),
            },
        )
        .collect();
    AwbcProgram {
        original_source: Some(
            plan.task_source_artifact(RuntimeTaskSourceArtifactLimits::default())
                .unwrap(),
        ),
        strings,
        runtime_types: vec![
            AwbcRuntimeType::new(boolean, AwbcRuntimeTypeShape::Bool),
            AwbcRuntimeType::new(
                pair,
                AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0), AwbcTypeId(0)]),
            ),
            AwbcRuntimeType::new(unit, AwbcRuntimeTypeShape::Unit),
        ],
        effect_sets: vec![AwbcEffectSet::default()],
        signatures: vec![AwbcSignature {
            params,
            result: Some(AwbcTypeId(0)),
            effects: AwbcEffectSetId(0),
        }],
        host_calls: vec![row],
        ..AwbcProgram::default()
    }
}

fn admit(code: Arc<AwbcProgram>) -> RuntimeTaskSourceProgram {
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

fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(1_000_000, 0)
}

#[test]
fn awbc_host_assertions_resolve_the_original_row_after_canonical_decode() {
    for mode in [RuntimeHostCallMode::Immediate, RuntimeHostCallMode::Suspend] {
        for empty in [false, true] {
            let code = executable(mode, empty);
            let bytes = code.encode_canonical().unwrap();
            let decoded = AwbcProgram::decode_canonical(
                &bytes,
                crate::awbc::codec::AwbcDecodeBudget::default(),
            )
            .unwrap();
            let program = admit(Arc::new(decoded));
            let mut work = meter();
            let source = program
                .awbc_host_source(
                    AwbcHostCallId(0),
                    RuntimeTaskPlanSealLimits::default(),
                    &mut work,
                )
                .unwrap();
            assert_eq!(source.task_plan().key(), key(&original(mode, empty)));
            assert!(source.task_plan().belongs_to(program.original()));
            let Some(RuntimeBodyTaskSource::Host(host)) = source.request_endpoint().task_source()
            else {
                unreachable!()
            };
            assert_eq!(host.mode, mode);
            assert_eq!(host.args.is_empty(), empty);
            assert!(work.totals().0 > 0);
            assert_eq!(
                work.totals().1,
                0,
                "row assertion admission does not re-encode E/F/Q/C"
            );
        }
    }
}

#[test]
fn awbc_host_assertions_reject_unresolved_keys_without_site_or_metadata_fallback() {
    for claimed in [
        TaskPlanSemanticDigest::from_bytes([0; 32]),
        key(&original(RuntimeHostCallMode::Immediate, true)),
    ] {
        let mut code = executable(RuntimeHostCallMode::Suspend, false);
        code.host_calls[0].producer.plan = claimed;
        let program = admit(Arc::new(code));
        let mut work = meter();
        assert!(matches!(
            program.awbc_host_source(
                AwbcHostCallId(0),
                RuntimeTaskPlanSealLimits::default(),
                &mut work
            ),
            Err(RuntimeAwbcHostSourceError::Source {
                call: AwbcHostCallId(0),
                source: RuntimeTaskSourceError::MissingExpectedKey
            })
        ));
        assert_eq!(work.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    }
}

#[test]
fn awbc_host_assertions_reject_a_real_selected_row_before_host_metadata() {
    let selected = selected_producers::tests::fixture(
        2,
        TaskPolicy::JoinSameKey,
        HostRestartPolicy::MustBeQuiescent,
        TaskClass::Io,
    );
    let mut code = executable(RuntimeHostCallMode::Suspend, false);
    code.original_source = Some(
        selected
            .task_source_artifact(RuntimeTaskSourceArtifactLimits::default())
            .unwrap(),
    );
    code.host_calls[0].producer.plan = key(&selected);
    let program = admit(Arc::new(code));
    assert!(matches!(
        program.awbc_host_source(
            AwbcHostCallId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter()
        ),
        Err(RuntimeAwbcHostSourceError::Mismatch {
            field: RuntimeAwbcHostSourceField::RequestKind,
            ..
        })
    ));
}

#[test]
fn awbc_host_assertions_check_every_original_host_field_and_argument_abi() {
    use RuntimeAwbcHostSourceField as Field;
    let expected = [
        Field::ProducerContract,
        Field::ProducerSite,
        Field::PublicId,
        Field::Capability,
        Field::Operation,
        Field::ManifestContract,
        Field::ResultType,
        Field::Mode,
        Field::Deterministic,
        Field::ArgumentCount,
        Field::ArgumentName { index: 1 },
        Field::ArgumentSpread { index: 2 },
        Field::ArgumentType { index: 0 },
    ];
    for field in expected {
        let mut code = executable(RuntimeHostCallMode::Suspend, false);
        let wrong = AwbcStringId(
            u32::try_from(code.strings.iter().position(|value| value == "別").unwrap()).unwrap(),
        );
        let row = &mut code.host_calls[0];
        match field {
            Field::ProducerContract => {
                row.producer.contract =
                    crate::task::NeedProducerContractDigest::from_bytes([0xff; 32]);
            }
            Field::ProducerSite => {
                row.producer.site = crate::task::NeedProducerSiteDigest::from_bytes([0xff; 32]);
            }
            Field::PublicId => row.public_id = wrong,
            Field::Capability => row.capability = wrong,
            Field::Operation => row.operation = wrong,
            Field::ManifestContract => row.contract = None,
            Field::ResultType => code.signatures[0].result = Some(AwbcTypeId(2)),
            Field::Mode => row.mode = AwbcHostCallMode::Immediate,
            Field::Deterministic => row.deterministic = !row.deterministic,
            Field::ArgumentCount => {
                row.arguments.pop();
                code.signatures[0].params.pop();
            }
            Field::ArgumentName { index } => row.arguments[index].name = None,
            Field::ArgumentSpread { index } => {
                row.arguments[index].spread = !row.arguments[index].spread;
            }
            Field::ArgumentType { index } => code.signatures[0].params[index] = AwbcTypeId(2),
            Field::RequestKind => unreachable!(),
        }
        // Every mutation remains a verified AWBC program and carries a valid
        // original Source. Their correspondence is the additional obligation.
        let program = admit(Arc::new(code));
        let mut work = meter();
        assert!(
            matches!(program.awbc_host_source(AwbcHostCallId(0), RuntimeTaskPlanSealLimits::default(), &mut work), Err(RuntimeAwbcHostSourceError::Mismatch { call: AwbcHostCallId(0), field: actual }) if actual == field),
            "{field:?}"
        );
        assert_eq!(work.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    }
}

#[test]
fn awbc_host_assertions_share_exact_work_previous_usage_and_the_first_resource_failure() {
    let program = admit(Arc::new(executable(RuntimeHostCallMode::Suspend, false)));
    let limits = RuntimeTaskPlanSealLimits::default();
    let mut measured = meter();
    let expected = program
        .awbc_host_source(AwbcHostCallId(0), limits, &mut measured)
        .unwrap()
        .task_plan()
        .key();
    let (work, bytes) = measured.totals();
    assert!(work > 1);
    assert_eq!(bytes, 0);
    let mut exact = TaskSemanticMeter::new(work, 0);
    assert_eq!(
        program
            .awbc_host_source(AwbcHostCallId(0), limits, &mut exact)
            .unwrap()
            .task_plan()
            .key(),
        expected
    );
    assert_eq!(exact.totals(), (work, 0));
    let mut prefixed = TaskSemanticMeter::new(work + 1, 0);
    prefixed.charge_work(1).unwrap();
    assert_eq!(
        program
            .awbc_host_source(AwbcHostCallId(0), limits, &mut prefixed)
            .unwrap()
            .task_plan()
            .key(),
        expected
    );
    assert_eq!(prefixed.totals(), (work + 1, 0));
    let mut short = TaskSemanticMeter::new(work - 1, 0);
    for _ in 0..2 {
        assert!(matches!(
            program.awbc_host_source(AwbcHostCallId(0), limits, &mut short),
            Err(RuntimeAwbcHostSourceError::Encoding(
                TaskSemanticEncodingError::SemanticWork
            ))
        ));
        assert_eq!(short.status(), Err(TaskSemanticEncodingError::SemanticWork));
    }
    let native =
        RuntimeTaskSourceProgram::native(Arc::new(original(RuntimeHostCallMode::Suspend, false)));
    assert!(matches!(
        native.awbc_host_source(AwbcHostCallId(99), limits, &mut short),
        Err(RuntimeAwbcHostSourceError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    let mut tightened = meter();
    tightened.charge_work(1).unwrap();
    assert!(matches!(
        program.awbc_host_source(
            AwbcHostCallId(0),
            RuntimeTaskPlanSealLimits {
                max_semantic_work: 0,
                ..limits
            },
            &mut tightened
        ),
        Err(RuntimeAwbcHostSourceError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
}

#[test]
fn awbc_host_assertions_reject_wrong_backend_and_absent_rows_without_publication() {
    let native =
        RuntimeTaskSourceProgram::native(Arc::new(original(RuntimeHostCallMode::Suspend, false)));
    assert!(matches!(
        native.awbc_host_source(
            AwbcHostCallId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter()
        ),
        Err(RuntimeAwbcHostSourceError::Backend)
    ));
    let program = admit(Arc::new(executable(RuntimeHostCallMode::Suspend, false)));
    let mut work = meter();
    assert!(matches!(
        program.awbc_host_source(
            AwbcHostCallId(1),
            RuntimeTaskPlanSealLimits::default(),
            &mut work
        ),
        Err(RuntimeAwbcHostSourceError::MissingCall {
            call: AwbcHostCallId(1)
        })
    ));
    assert_eq!(work.status(), Err(TaskSemanticEncodingError::OwnerRejected));
}

#[test]
fn awbc_host_assertions_bound_argument_roles_before_matching_a_claimed_key() {
    for forged in [false, true] {
        let mut code = executable(RuntimeHostCallMode::Suspend, false);
        if forged {
            code.host_calls[0].producer.plan = TaskPlanSemanticDigest::from_bytes([0; 32]);
        }
        let program = admit(Arc::new(code));
        let mut work = meter();
        assert!(matches!(
            program.awbc_host_source(
                AwbcHostCallId(0),
                RuntimeTaskPlanSealLimits {
                    max_request_roles: 2,
                    ..RuntimeTaskPlanSealLimits::default()
                },
                &mut work
            ),
            Err(RuntimeAwbcHostSourceError::ArgumentRoles {
                call: AwbcHostCallId(0),
                actual: 3,
                maximum: 2,
            })
        ));
        assert_eq!(work.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    }
}

#[test]
fn awbc_host_assertions_reject_same_semantic_identity_with_a_different_shape() {
    let mut code = executable(RuntimeHostCallMode::Suspend, false);
    let claimed = code.runtime_types[0].semantic_identity();
    code.runtime_types[0] = AwbcRuntimeType::new(claimed, AwbcRuntimeTypeShape::String);
    let program = admit(Arc::new(code));
    assert!(
        program
            .awbc_host_source(
                AwbcHostCallId(0),
                RuntimeTaskPlanSealLimits::default(),
                &mut meter()
            )
            .is_err(),
        "an asserted semantic identity cannot substitute for the original executable type shape"
    );
}

#[test]
fn awbc_host_assertions_feed_the_source_bound_producer_without_copying_definitions() {
    let code = Arc::new(executable(RuntimeHostCallMode::Suspend, false));
    let weak = Arc::downgrade(&code);
    let program = admit(Arc::clone(&code));
    let source = program
        .awbc_host_source(
            AwbcHostCallId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter(),
        )
        .unwrap();
    let Some(RuntimeBodyTaskSource::Host(host)) = source.request_endpoint().task_source() else {
        unreachable!()
    };
    let values = [
        RuntimeValue::Bool(true),
        RuntimeValue::Bool(false),
        RuntimeValue::Tuple(vec![RuntimeValue::Bool(false), RuntimeValue::Bool(true)]),
    ];
    let operands: Vec<_> = host
        .args
        .iter()
        .zip(values)
        .map(|(original, value)| match original {
            RuntimeHostArgumentTemplate::Positional(role, _) => {
                RuntimeHostArgumentTemplate::positional(*role, value)
            }
            RuntimeHostArgumentTemplate::Named(role, named) => {
                RuntimeHostArgumentTemplate::named(*role, &named.name, value)
            }
            RuntimeHostArgumentTemplate::Spread(role, _) => {
                RuntimeHostArgumentTemplate::spread(*role, value)
            }
        })
        .collect();
    let mut bank = RuntimeRetainedValueAdmission::for_program(
        program.program_owner(),
        RuntimeSchemaLimits::engine_default(),
    );
    let spec = source
        .host_producer_spec(&program, &operands, None, &mut bank)
        .unwrap();
    assert_eq!(spec.source().task_plan().key(), key(program.original()));
    assert_eq!(
        spec.source().task_plan().key(),
        code.host_calls[0].producer.plan
    );
    drop(bank);
    drop(code);
    drop(program);
    assert!(weak.upgrade().is_some());
    drop(spec);
    assert!(weak.upgrade().is_none());
}

struct Observed<'a> {
    reads: &'a std::cell::Cell<usize>,
    value: &'a RuntimeValue,
}

impl crate::value::RuntimeStoredValue for Observed<'_> {
    fn borrowed_view(&self) -> crate::value::RuntimeValueView<'_> {
        self.reads.set(self.reads.get() + 1);
        self.value.view()
    }
    fn admission_origin() -> crate::entry::schema::value_encoding::ValueAdmissionOrigin {
        crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live
    }
}

#[test]
fn awbc_host_factory_rejects_unbound_source_before_value_views_or_lease_retention() {
    let code = Arc::new(executable(RuntimeHostCallMode::Suspend, false));
    let program = admit(Arc::clone(&code));
    let original = program.original();
    let raw = original
        .task_source_by_expected_key(
            &ExpectedTaskPlanKey::new(*key(original).as_bytes()),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter(),
        )
        .unwrap();
    let Some(RuntimeBodyTaskSource::Host(host)) = raw.request_endpoint().task_source() else {
        unreachable!()
    };
    let reads = std::cell::Cell::new(0);
    let values = [
        RuntimeValue::Bool(true),
        RuntimeValue::Bool(false),
        RuntimeValue::Tuple(vec![RuntimeValue::Bool(false), RuntimeValue::Bool(true)]),
    ];
    let arguments: Vec<_> = host
        .args
        .iter()
        .zip(&values)
        .map(|(argument, value)| {
            let value = Observed {
                reads: &reads,
                value,
            };
            match argument {
                RuntimeHostArgumentTemplate::Positional(role, _) => {
                    RuntimeHostArgumentTemplate::positional(*role, value)
                }
                RuntimeHostArgumentTemplate::Named(role, named) => {
                    RuntimeHostArgumentTemplate::named(*role, &named.name, value)
                }
                RuntimeHostArgumentTemplate::Spread(role, _) => {
                    RuntimeHostArgumentTemplate::spread(*role, value)
                }
            }
        })
        .collect();
    let mut bank = RuntimeRetainedValueAdmission::for_program(
        program.program_owner(),
        RuntimeSchemaLimits::engine_default(),
    );
    let leases = Arc::strong_count(&code);
    assert!(matches!(
        raw.host_producer_spec(&program, &arguments, None, &mut bank),
        Err(crate::task::RuntimeTaskProducerSourceError::OriginalProgram)
    ));
    assert!(
        crate::plan::task_source::RuntimeTaskSource::from_borrowed(
            &raw,
            &program,
            bank.program_owner().unwrap()
        )
        .is_none()
    );
    assert_eq!(reads.get(), 0);
    assert_eq!(Arc::strong_count(&code), leases);
    let bound = program
        .awbc_host_source(
            AwbcHostCallId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter(),
        )
        .unwrap();
    let spec = bound
        .host_producer_spec(&program, &arguments, None, &mut bank)
        .unwrap();
    assert!(reads.get() > 0);
    let borrowed = spec
        .source()
        .borrow(RuntimeTaskPlanSealLimits::default(), &mut meter())
        .unwrap();
    assert!(program.owns_source_for(&borrowed, bank.program_owner().unwrap()));
    assert_eq!(borrowed.task_plan().key(), bound.task_plan().key());
}

#[test]
fn awbc_host_owning_source_rechecks_row_limits_and_exact_work_on_reborrow() {
    let program = admit(Arc::new(executable(RuntimeHostCallMode::Suspend, false)));
    let limits = RuntimeTaskPlanSealLimits::default();
    let bound = program
        .awbc_host_source(AwbcHostCallId(0), limits, &mut meter())
        .unwrap();
    let owned = crate::plan::task_source::RuntimeTaskSource::from_borrowed(
        &bound,
        &program,
        &program.program_owner(),
    )
    .unwrap();
    let mut measured = meter();
    owned.borrow(limits, &mut measured).unwrap();
    let work = measured.totals().0;
    let mut exact = TaskSemanticMeter::new(work, 0);
    let borrowed = owned.borrow(limits, &mut exact).unwrap();
    assert!(program.owns_source_for(&borrowed, &program.program_owner()));
    assert_eq!(exact.totals(), (work, 0));
    let mut short = TaskSemanticMeter::new(work - 1, 0);
    for _ in 0..2 {
        assert!(owned.borrow(limits, &mut short).is_err());
        assert_eq!(short.status(), Err(TaskSemanticEncodingError::SemanticWork));
    }
    assert!(matches!(
        owned.borrow(
            RuntimeTaskPlanSealLimits {
                max_request_roles: 2,
                ..limits
            },
            &mut meter()
        ),
        Err(super::super::RuntimeTaskSourceBorrowError::Host(
            RuntimeAwbcHostSourceError::ArgumentRoles {
                actual: 3,
                maximum: 2,
                ..
            }
        ))
    ));
}
