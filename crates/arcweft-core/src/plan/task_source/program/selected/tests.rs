use super::*;
use crate::awbc::schema::*;
use crate::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use crate::pattern::RuntimeCheckedType;
use crate::plan::construction::source::selected_producers;
use crate::plan::task_source::{RuntimeTaskSource, RuntimeTaskSourceArtifactLimits};
use crate::plan::{FlowOp, RuntimeTaskPlanIndex};
use crate::task::*;
use crate::value::{RuntimeRetainedValueAdmission, RuntimeStoredValue};
use std::cell::Cell;
use std::sync::Arc;

pub(in crate::plan::task_source) fn executable(request: u8) -> AwbcProgram {
    let original = selected_producers::tests::fixture(
        request,
        TaskPolicy::JoinSameKey,
        HostRestartPolicy::MustBeQuiescent,
        TaskClass::Io,
    );
    let FlowOp::StartNeedProducer { target, .. } = &original
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
    let metadata = target.plan();
    let mut strings = vec![metadata.cancel_scope().0.clone(), "別".to_owned()];
    match metadata.request() {
        NeedProducerRequestProjection::AssetLoad { argument_name, .. } => {
            strings.push(argument_name.clone())
        }
        NeedProducerRequestProjection::ExternCapability {
            capability,
            operation,
            argument_names,
            ..
        } => {
            strings.extend([capability.0.clone(), operation.clone()]);
            strings.extend(argument_names.iter().flatten().cloned());
        }
    }
    strings.sort();
    strings.dedup();
    let string = |text: &str| {
        AwbcStringId(
            u32::try_from(strings.iter().position(|actual| actual == text).unwrap()).unwrap(),
        )
    };
    let request = match metadata.request() {
        NeedProducerRequestProjection::AssetLoad {
            kind,
            argument_name,
        } => AwbcTaskRequestProjection::AssetLoad {
            kind: *kind,
            argument_name: string(argument_name),
        },
        NeedProducerRequestProjection::ExternCapability {
            capability,
            operation,
            contract,
            ..
        } => AwbcTaskRequestProjection::ExternCapability {
            capability: string(&capability.0),
            operation: string(operation),
            contract: *contract,
        },
    };
    let args = target.arguments();
    let boolean = RuntimeCheckedType::Bool.semantic_identity_digest();
    let params = metadata
        .argument_types()
        .iter()
        .map(|ty| {
            if *ty == boolean {
                AwbcTypeId(0)
            } else {
                AwbcTypeId(1)
            }
        })
        .collect();
    let row = AwbcTaskPlan {
        signature: AwbcSignatureId(0),
        request,
        class: AwbcTaskClass::Io,
        priority: metadata.priority().0,
        cancel_scope: string(&metadata.cancel_scope().0),
        policy: AwbcTaskPolicy::JoinSameKey,
        payload_type: AwbcTypeId(0),
        arguments: args
            .iter()
            .map(|arg| AwbcHostArgument {
                name: arg.name().map(string),
                spread: arg.is_spread(),
            })
            .collect(),
        kind: AwbcTaskPlanKind::NeedProducer {
            plan: original.task_plan(target.task_plan()).unwrap().key().into(),
            contract: metadata.contract(),
            site: metadata.site(),
            restart: AwbcTaskRestartPolicy::MustBeQuiescent,
        },
    };
    AwbcProgram {
        original_source: Some(
            original
                .task_source_artifact(RuntimeTaskSourceArtifactLimits::default())
                .unwrap(),
        ),
        strings,
        runtime_types: vec![
            AwbcRuntimeType::new(boolean, AwbcRuntimeTypeShape::Bool),
            AwbcRuntimeType::new(
                RuntimeCheckedType::EntityReference.semantic_identity_digest(),
                AwbcRuntimeTypeShape::EntityRef,
            ),
        ],
        effect_sets: vec![AwbcEffectSet::default()],
        signatures: vec![AwbcSignature {
            params,
            result: None,
            effects: AwbcEffectSetId(0),
        }],
        task_plans: vec![row],
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

pub(in crate::plan::task_source) fn source(
    program: &RuntimeTaskSourceProgram,
) -> RuntimeTaskSourceRef<'_> {
    let key = program
        .original()
        .task_plans()
        .by_index(RuntimeTaskPlanIndex(0))
        .unwrap()
        .key();
    program
        .original()
        .task_source_by_expected_key(
            &ExpectedTaskPlanKey::new(*key.as_bytes()),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter(),
        )
        .unwrap()
}

fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(1_000_000, 1_000_000)
}

#[test]
fn awbc_selected_construction_joins_all_original_selected_projections_after_decode() {
    for request in 0..3 {
        let encoded = executable(request).encode_canonical().unwrap();
        let program = admit(Arc::new(
            AwbcProgram::decode_canonical(
                &encoded,
                crate::awbc::codec::AwbcDecodeBudget::default(),
            )
            .unwrap(),
        ));
        let raw = source(&program);
        assert!(!program.owns_source_for(&raw, &program.program_owner()));
        let expected = raw.task_plan().key();
        let mut work = meter();
        let bound = program
            .awbc_selected_source(
                AwbcTaskPlanId(0),
                RuntimeTaskPlanSealLimits::default(),
                &mut work,
            )
            .unwrap();
        assert!(program.owns_source_for(&bound, &program.program_owner()));
        assert_eq!(bound.task_plan().key(), expected);
        assert_eq!(work.totals().1, 0);
        let owned =
            RuntimeTaskSource::from_borrowed(&bound, &program, &program.program_owner()).unwrap();
        let borrowed = owned
            .borrow(RuntimeTaskPlanSealLimits::default(), &mut meter())
            .unwrap();
        assert!(program.owns_source_for(&borrowed, &program.program_owner()));
        assert_eq!(borrowed.task_plan().key(), expected);
    }
}

#[test]
fn awbc_selected_assertion_rejects_missing_or_native_rows() {
    let program = admit(Arc::new(executable(2)));
    assert!(matches!(
        program.awbc_selected_source(
            AwbcTaskPlanId(1),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter()
        ),
        Err(RuntimeAwbcSelectedSourceError::MissingTask { .. })
    ));
    let native = RuntimeTaskSourceProgram::native(Arc::new(selected_producers::tests::fixture(
        2,
        TaskPolicy::JoinSameKey,
        HostRestartPolicy::MustBeQuiescent,
        TaskClass::Io,
    )));
    assert!(matches!(
        native.awbc_selected_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter()
        ),
        Err(RuntimeAwbcSelectedSourceError::Backend)
    ));
}

#[test]
fn decoded_selected_assertions_cannot_fall_back_to_matching_request_metadata() {
    let foreign = selected_producers::tests::fixture(
        0,
        TaskPolicy::JoinSameKey,
        HostRestartPolicy::MustBeQuiescent,
        TaskClass::Io,
    );
    let foreign = foreign
        .task_plans()
        .by_index(RuntimeTaskPlanIndex(0))
        .unwrap()
        .key();
    for assertion in [ExpectedTaskPlanKey::new([0; 32]), foreign.into()] {
        let mut code = executable(2);
        let AwbcTaskPlanKind::NeedProducer { plan, .. } = &mut code.task_plans[0].kind else {
            unreachable!()
        };
        *plan = assertion;
        let encoded = code.encode_canonical().unwrap();
        let code = Arc::new(
            AwbcProgram::decode_canonical(
                &encoded,
                crate::awbc::codec::AwbcDecodeBudget::default(),
            )
            .unwrap(),
        );
        let program = admit(Arc::clone(&code));
        let leases = Arc::strong_count(&code);
        let mut work = meter();
        assert!(matches!(
            program.awbc_selected_source(
                AwbcTaskPlanId(0),
                RuntimeTaskPlanSealLimits::default(),
                &mut work,
            ),
            Err(RuntimeAwbcSelectedSourceError::Source {
                source: crate::plan::task_source::RuntimeTaskSourceError::MissingExpectedKey,
                ..
            })
        ));
        assert_eq!(work.status(), Err(TaskSemanticEncodingError::OwnerRejected));
        assert_eq!(Arc::strong_count(&code), leases);
    }
}

#[test]
fn awbc_selected_construction_checks_verified_metadata_changes_without_reconstruction() {
    use RuntimeAwbcSelectedSourceField as Field;
    for field in [
        Field::Contract,
        Field::Site,
        Field::Capability,
        Field::Operation,
        Field::Class,
        Field::Priority,
        Field::CancelScope,
        Field::Policy,
        Field::Restart,
        Field::ArgumentName { index: 1 },
    ] {
        let mut code = executable(2);
        let wrong = AwbcStringId(
            u32::try_from(code.strings.iter().position(|text| text == "別").unwrap()).unwrap(),
        );
        let row = &mut code.task_plans[0];
        let AwbcTaskPlanKind::NeedProducer {
            contract,
            site,
            restart,
            ..
        } = &mut row.kind
        else {
            unreachable!()
        };
        match field {
            Field::Contract => {
                *contract = NeedProducerContractDigest::from_bytes([0x33; 32]);
                let AwbcTaskRequestProjection::ExternCapability { contract, .. } = &mut row.request
                else {
                    unreachable!()
                };
                *contract = crate::step::HostCallContractDigest::from_bytes([0x33; 32]);
            }
            Field::Site => *site = NeedProducerSiteDigest::from_bytes([0x33; 32]),
            Field::Capability => {
                let AwbcTaskRequestProjection::ExternCapability { capability, .. } =
                    &mut row.request
                else {
                    unreachable!()
                };
                *capability = wrong;
            }
            Field::Operation => {
                let AwbcTaskRequestProjection::ExternCapability { operation, .. } =
                    &mut row.request
                else {
                    unreachable!()
                };
                *operation = wrong;
            }
            Field::Class => row.class = AwbcTaskClass::Cpu,
            Field::Priority => row.priority += 1,
            Field::CancelScope => row.cancel_scope = wrong,
            Field::Policy => row.policy = AwbcTaskPolicy::AlwaysStart,
            Field::Restart => *restart = AwbcTaskRestartPolicy::Restartable,
            Field::ArgumentName { index } => row.arguments[index].name = None,
            _ => unreachable!(),
        }
        let program = admit(Arc::new(code));
        let mut work = meter();
        assert!(
            matches!(program.awbc_selected_source(AwbcTaskPlanId(0), RuntimeTaskPlanSealLimits::default(), &mut work), Err(RuntimeAwbcSelectedSourceError::Mismatch { field: actual, .. }) if actual == field),
            "{field:?}"
        );
        assert_eq!(work.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    }
}

#[test]
fn awbc_selected_construction_rejects_same_identity_with_another_executable_shape() {
    let mut code = executable(2);
    code.runtime_types[0] = AwbcRuntimeType::new(
        RuntimeCheckedType::Bool.semantic_identity_digest(),
        AwbcRuntimeTypeShape::String,
    );
    let program = admit(Arc::new(code));
    let mut work = meter();
    assert!(matches!(
        program.awbc_selected_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut work
        ),
        Err(RuntimeAwbcSelectedSourceError::Type {
            field: RuntimeAwbcSelectedSourceField::Payload,
            ..
        })
    ));
    assert_eq!(work.status(), Err(TaskSemanticEncodingError::OwnerRejected));
}

#[test]
fn awbc_selected_construction_and_reborrow_share_exact_and_sticky_work() {
    let program = admit(Arc::new(executable(2)));
    let limits = RuntimeTaskPlanSealLimits::default();
    let mut measured = meter();
    let bound = program
        .awbc_selected_source(AwbcTaskPlanId(0), limits, &mut measured)
        .unwrap();
    let (work, bytes) = measured.totals();
    assert!(work > 1);
    assert_eq!(bytes, 0);
    let mut exact = TaskSemanticMeter::new(work, 0);
    program
        .awbc_selected_source(AwbcTaskPlanId(0), limits, &mut exact)
        .unwrap();
    assert_eq!(exact.totals(), (work, 0));
    let mut short = TaskSemanticMeter::new(work - 1, 0);
    for _ in 0..2 {
        assert!(matches!(
            program.awbc_selected_source(AwbcTaskPlanId(0), limits, &mut short),
            Err(RuntimeAwbcSelectedSourceError::Encoding(
                TaskSemanticEncodingError::SemanticWork
            ))
        ));
    }
    let owned =
        RuntimeTaskSource::from_borrowed(&bound, &program, &program.program_owner()).unwrap();
    let mut measured = meter();
    owned.borrow(limits, &mut measured).unwrap();
    let total = measured.totals().0;
    assert!(
        total == work,
        "reborrow repeats the same assertion and original Source resolution"
    );
    let mut exact = TaskSemanticMeter::new(total, 0);
    owned.borrow(limits, &mut exact).unwrap();
    assert_eq!(exact.totals(), (total, 0));
    let mut short = TaskSemanticMeter::new(total - 1, 0);
    for _ in 0..2 {
        assert!(owned.borrow(limits, &mut short).is_err());
        assert_eq!(short.status(), Err(TaskSemanticEncodingError::SemanticWork));
    }
}

struct Observed<'a>(&'a Cell<usize>);
impl RuntimeStoredValue for Observed<'_> {
    fn borrowed_view(&self) -> crate::value::RuntimeValueView<'_> {
        self.0.set(self.0.get() + 1);
        crate::value::RuntimeValueView::Scalar(crate::value::RuntimeScalarView::Bool(true))
    }
    fn admission_origin() -> crate::entry::schema::value_encoding::ValueAdmissionOrigin {
        crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live
    }
}

#[test]
fn awbc_selected_factory_requires_a_checked_code_binding_before_value_views_or_leases() {
    let code = Arc::new(executable(2));
    let program = admit(Arc::clone(&code));
    let raw = source(&program);
    let reads = Cell::new(0);
    let args = [
        NeedProducerRuntimeArgument {
            name: None,
            value: Observed(&reads),
        },
        NeedProducerRuntimeArgument {
            name: Some("条件".into()),
            value: Observed(&reads),
        },
    ];
    let mut bank = RuntimeRetainedValueAdmission::for_program(
        program.program_owner(),
        crate::entry::RuntimeSchemaLimits::engine_default(),
    );
    let leases = Arc::strong_count(&code);
    assert!(matches!(
        raw.selected_producer_spec(&program, &args, None, &mut bank),
        Err(RuntimeTaskProducerSourceError::OriginalProgram)
    ));
    assert!(
        RuntimeTaskSource::from_borrowed(&raw, &program, bank.program_owner().unwrap()).is_none()
    );
    assert_eq!(reads.get(), 0);
    assert_eq!(Arc::strong_count(&code), leases);
    let bound = program
        .awbc_selected_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter(),
        )
        .unwrap();
    let spec = bound
        .selected_producer_spec(&program, &args, None, &mut bank)
        .unwrap();
    assert!(reads.get() > 0);
    assert_eq!(spec.source().task_plan().key(), bound.task_plan().key());
}

#[test]
fn saved_awbc_source_assertions_keep_the_exact_row_and_reject_forged_locators() {
    let bytes = executable(2).encode_canonical().unwrap();
    let code = Arc::new(AwbcProgram::decode_canonical(&bytes, Default::default()).unwrap());
    let program = admit(Arc::clone(&code));
    let limits = RuntimeTaskPlanSealLimits::default();
    let source = program
        .awbc_selected_source(AwbcTaskPlanId(0), limits, &mut meter())
        .unwrap();
    let retained =
        RuntimeTaskSource::from_borrowed(&source, &program, &program.program_owner()).unwrap();
    let leases = Arc::strong_count(&code);
    let encoded = serde_json::to_vec(&retained.assertion()).unwrap();
    let assertion: crate::plan::RuntimeTaskSourceAssertion =
        serde_json::from_slice(&encoded).unwrap();
    assert_eq!(Arc::strong_count(&code), leases);
    let restored = program
        .borrow_assertion(&assertion, limits, &mut meter())
        .unwrap();
    assert_eq!(restored.task_plan().key(), source.task_plan().key());
    assert!(program.owns_source_for(&restored, &program.program_owner()));
    let mut missing = assertion.clone();
    missing.expected = ExpectedTaskPlanKey::new([0; 32]);
    let mut row = assertion.clone();
    row.execution = crate::plan::task_source::owned::RuntimeTaskSourceExecutionAssertion::Awbc(
        crate::plan::task_source::program::RuntimeTaskExecutionRow::Selected(AwbcTaskPlanId(
            u32::MAX,
        )),
    );
    let mut backend = assertion.clone();
    backend.execution =
        crate::plan::task_source::owned::RuntimeTaskSourceExecutionAssertion::Native;
    for forged in [missing, row, backend] {
        let mut bank = meter();
        assert!(
            program
                .borrow_assertion(&forged, limits, &mut bank)
                .is_err()
        );
        assert_eq!(
            bank.status(),
            Err(crate::task::semantic::TaskSemanticEncodingError::OwnerRejected)
        );
        assert!(
            program
                .borrow_assertion(&assertion, limits, &mut bank)
                .is_err()
        );
        assert_eq!(Arc::strong_count(&code), leases);
    }
}
