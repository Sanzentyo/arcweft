use super::*;
use crate::entry::{RuntimeSchemaError, RuntimeSchemaLimits};
use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
use crate::plan::construction::source::selected_producers;
use crate::plan::{FlowOp, RuntimePlan, RuntimeTaskPlanSealLimits};
use crate::task::semantic::TaskSemanticMeter;
use crate::task::{HostRestartPolicy, TaskClass, TaskPolicy};
use crate::task::{
    NeedProducerRuntimeArgument, NeedProducerSpec, RuntimeProgramOwner,
    RuntimeTaskProducerSourceError,
};
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission, RuntimeStoredValue,
};
use crate::value::{RuntimeEntityReference, RuntimeValue};
use std::cell::Cell;
use std::sync::Arc;

fn fixture(request: u8) -> Arc<RuntimePlan> {
    Arc::new(selected_producers::tests::fixture(
        request,
        TaskPolicy::JoinSameKey,
        HostRestartPolicy::MustBeQuiescent,
        TaskClass::Io,
    ))
}

fn borrow_source(plan: &RuntimePlan) -> RuntimeTaskSourceRef<'_> {
    let operation = &plan
        .function_sites()
        .iter()
        .next()
        .unwrap()
        .body()
        .executable()
        .unwrap()
        .ops()[0];
    let FlowOp::StartNeedProducer { target, .. } = operation else {
        unreachable!()
    };
    plan.task_source(
        target.task_plan(),
        RuntimeTaskPlanSealLimits::default(),
        &mut TaskSemanticMeter::new(1_000_000, 1_000_000),
    )
    .unwrap()
}

fn arguments(request: u8) -> Vec<NeedProducerRuntimeArgument> {
    if request < 2 {
        vec![NeedProducerRuntimeArgument {
            name: Some("素材".into()),
            value: RuntimeValue::EntityRef(
                RuntimeEntityReference::try_project(
                    arcweft_id::DeclarationIdentityFamily::Asset,
                    arcweft_id::PublicId::try_new("asset.source").unwrap(),
                )
                .unwrap(),
            ),
        }]
    } else {
        vec![
            NeedProducerRuntimeArgument {
                name: None,
                value: RuntimeValue::Bool(true),
            },
            NeedProducerRuntimeArgument {
                name: Some("条件".into()),
                value: RuntimeValue::Bool(false),
            },
        ]
    }
}

fn admission(
    plan: &Arc<RuntimePlan>,
    limits: RuntimeSchemaLimits,
) -> RuntimeRetainedValueAdmission {
    RuntimeRetainedValueAdmission::for_program(RuntimeProgramOwner::Plan(Arc::clone(plan)), limits)
}

struct ObservedValue<'a>(&'a Cell<usize>);

impl RuntimeStoredValue for ObservedValue<'_> {
    fn borrowed_view(&self) -> crate::value::RuntimeValueView<'_> {
        self.0.set(self.0.get() + 1);
        crate::value::RuntimeValueView::Scalar(crate::value::RuntimeScalarView::Bool(true))
    }

    fn admission_origin() -> crate::entry::schema::value_encoding::ValueAdmissionOrigin {
        crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live
    }
}

fn observed_arguments(
    counter: &Cell<usize>,
) -> [NeedProducerRuntimeArgument<ObservedValue<'_>>; 2] {
    [
        NeedProducerRuntimeArgument {
            name: None,
            value: ObservedValue(counter),
        },
        NeedProducerRuntimeArgument {
            name: Some("条件".into()),
            value: ObservedValue(counter),
        },
    ]
}

#[test]
fn source_bound_selected_specs_use_the_completed_key_and_canonical_request() {
    for request in 0..3 {
        let plan = fixture(request);
        let source = borrow_source(&plan);
        let values = arguments(request);
        let spec = source
            .selected_producer_spec(
                &values,
                None,
                &mut admission(&plan, RuntimeSchemaLimits::engine_default()),
            )
            .unwrap();
        let Some(RuntimeBodyTaskSource::Start(target)) = source.request_endpoint().task_source()
        else {
            unreachable!()
        };
        let metadata = target.plan();
        assert_eq!(spec.source().task_plan().key(), source.task_plan().key());
        assert_eq!(spec.family(), source.task_plan().definition().family());
        let expected = NeedProducerSpec::new(
            spec.family(),
            spec.contract(),
            source.task_plan().key(),
            spec.producer_site(),
            spec.payload_type(),
            spec.arguments(),
        );
        assert_eq!(
            spec.instance_key().unwrap(),
            expected.instance_key().unwrap()
        );
        assert_eq!(spec.contract(), metadata.contract());
        assert_eq!(spec.producer_site(), metadata.site());
        assert_eq!(
            spec.payload_type().as_bytes(),
            metadata.payload_type().as_bytes()
        );
        assert_eq!(
            spec.arguments(),
            metadata.producer_spec(&values).unwrap().arguments()
        );
        assert_ne!(
            spec.source().task_plan().key(),
            metadata.producer_spec(&values).unwrap().plan()
        );
    }
}

#[test]
fn source_bound_selected_specs_retain_the_original_program_after_the_caller_drops_it() {
    let (spec, weak, key) = {
        let plan = fixture(2);
        let source = borrow_source(&plan);
        let key = source.task_plan().key();
        let weak = Arc::downgrade(&plan);
        let spec = source
            .selected_producer_spec(
                &arguments(2),
                None,
                &mut admission(&plan, RuntimeSchemaLimits::engine_default()),
            )
            .unwrap();
        (spec, weak, key)
    };
    assert!(weak.upgrade().is_some());
    assert_eq!(spec.source().task_plan().key(), key);
    let borrowed = spec
        .source()
        .borrow(
            RuntimeTaskPlanSealLimits::default(),
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000),
        )
        .unwrap();
    assert_eq!(borrowed.task_plan().key(), key);
    drop(borrowed);
    let retained = spec.clone();
    drop(spec);
    assert!(weak.upgrade().is_some());
    assert_eq!(retained.source().task_plan().key(), key);
    drop(retained);
    assert!(weak.upgrade().is_none());
}

#[test]
fn source_bound_selected_specs_keep_program_identity_outside_the_semantic_transcript() {
    let plan = fixture(2);
    let other = fixture(2);
    let first = borrow_source(&plan)
        .selected_producer_spec(
            &arguments(2),
            None,
            &mut admission(&plan, RuntimeSchemaLimits::engine_default()),
        )
        .unwrap();
    let last = borrow_source(&other)
        .selected_producer_spec(
            &arguments(2),
            None,
            &mut admission(&other, RuntimeSchemaLimits::engine_default()),
        )
        .unwrap();
    assert_eq!(first.instance_key().unwrap(), last.instance_key().unwrap());
    assert_ne!(first, last);
    assert!(first.source().belongs_to(&plan));
    assert!(!first.source().belongs_to(&other));
}

#[test]
fn source_bound_selected_specs_keep_runtime_arguments_in_instance_identity() {
    let plan = fixture(2);
    let source = borrow_source(&plan);
    let values = arguments(2);
    let mut changed = arguments(2);
    changed[0].value = RuntimeValue::Bool(false);
    let first = source
        .selected_producer_spec(
            &values,
            None,
            &mut admission(&plan, RuntimeSchemaLimits::engine_default()),
        )
        .unwrap();
    let last = source
        .selected_producer_spec(
            &changed,
            None,
            &mut admission(&plan, RuntimeSchemaLimits::engine_default()),
        )
        .unwrap();
    assert_eq!(
        first.source().task_plan().key(),
        last.source().task_plan().key()
    );
    assert_ne!(first.arguments(), last.arguments());
    assert_ne!(first.instance_key().unwrap(), last.instance_key().unwrap());
}

#[test]
fn source_bound_selected_specs_require_the_actual_program_before_value_inspection() {
    let plan = fixture(2);
    let source = borrow_source(&plan);
    let equal = fixture(2);
    let copied = Arc::new((*plan).clone());
    assert_eq!(
        source.task_plan().key(),
        borrow_source(&equal).task_plan().key()
    );
    assert!(source.task_plan().belongs_to(&copied));
    let reads = Cell::new(0);
    let arguments = observed_arguments(&reads);
    let owners = [
        RuntimeProgramOwner::Plan(equal),
        RuntimeProgramOwner::Plan(copied),
        RuntimeProgramOwner::Awbc(Arc::new(crate::awbc::schema::AwbcProgram::default())),
    ];
    for owner in owners {
        let mut bank = RuntimeRetainedValueAdmission::for_program(
            owner,
            RuntimeSchemaLimits::engine_default(),
        );
        assert!(matches!(
            source.selected_producer_spec(&arguments, None, &mut bank),
            Err(RuntimeTaskProducerSourceError::OriginalProgram)
        ));
    }
    assert!(matches!(
        source.selected_producer_spec(
            &arguments,
            None,
            &mut RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimeTaskProducerSourceError::OriginalProgram)
    ));
    assert_eq!(reads.get(), 0);
}

#[test]
fn source_bound_selected_specs_reject_argument_shape_and_type_changes() {
    let plan = fixture(2);
    let source = borrow_source(&plan);
    assert!(matches!(
        source.selected_producer_spec::<RuntimeValue>(
            &[],
            None,
            &mut admission(&plan, RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimeTaskProducerSourceError::ArgumentCount {
            actual: 0,
            expected: 2
        })
    ));
    let mut named = arguments(2);
    named[0].name = Some("別名".into());
    assert!(matches!(
        source.selected_producer_spec(
            &named,
            None,
            &mut admission(&plan, RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimeTaskProducerSourceError::ArgumentName { index: 0 })
    ));
    let mut wrong_type = arguments(2);
    wrong_type[0].value = RuntimeValue::String("wrong".into());
    assert!(matches!(
        source.selected_producer_spec(
            &wrong_type,
            None,
            &mut admission(&plan, RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimeTaskProducerSourceError::Admission(
            AwbcRuntimeValueSnapshotError::ProgramType(_)
        ))
    ));
}

#[test]
fn source_bound_selected_specs_bound_the_complete_argument_list_before_value_views() {
    let plan = fixture(2);
    let source = borrow_source(&plan);
    let reads = Cell::new(0);
    let arguments = observed_arguments(&reads);
    for limits in [
        RuntimeSchemaLimits {
            max_sequence_items: 1,
            ..RuntimeSchemaLimits::engine_default()
        },
        RuntimeSchemaLimits {
            max_validation_work: 0,
            ..RuntimeSchemaLimits::engine_default()
        },
    ] {
        let mut bank = admission(&plan, limits);
        for _ in 0..2 {
            assert!(matches!(
                source.selected_producer_spec(&arguments, None, &mut bank),
                Err(RuntimeTaskProducerSourceError::Admission(
                    AwbcRuntimeValueSnapshotError::Resource(_)
                ))
            ));
        }
    }
    assert_eq!(reads.get(), 0);
}

#[test]
fn source_bound_selected_specs_reject_a_different_original_request_kind() {
    let plan = Arc::new(
        crate::plan::construction::source::host_calls::tests::fixture(
            crate::step::RuntimeHostCallMode::Suspend,
            true,
            true,
            false,
        ),
    );
    let operation = &plan
        .function_sites()
        .iter()
        .next()
        .unwrap()
        .body()
        .executable()
        .unwrap()
        .ops()[0];
    let FlowOp::HostCall { target, .. } = operation else {
        unreachable!()
    };
    let source = plan
        .task_source(
            &target.producer.plan,
            RuntimeTaskPlanSealLimits::default(),
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000),
        )
        .unwrap();
    assert!(matches!(
        source.selected_producer_spec::<RuntimeValue>(
            &[],
            None,
            &mut admission(&plan, RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimeTaskProducerSourceError::RequestKind)
    ));
}

#[test]
fn source_bound_selected_specs_reject_unresolved_invocation_effect_bindings() {
    let plan = fixture(2);
    let source = borrow_source(&plan);
    let binding = crate::program_types::RuntimeFunctionEffectInstantiation::from_source_rows(
        crate::pattern::RuntimeSemanticTypeId::from_bytes([0xfe; 32]),
        Box::new([]),
    );
    assert!(matches!(
        source.selected_producer_spec(
            &arguments(2),
            Some(&binding),
            &mut admission(&plan, RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimeTaskProducerSourceError::Admission(
            AwbcRuntimeValueSnapshotError::ProgramType(_)
        ))
    ));
}

#[test]
fn source_bound_selected_specs_share_exact_original_work_and_keep_first_failure() {
    let plan = fixture(2);
    let source = borrow_source(&plan);
    let values = arguments(2);
    let limits = RuntimeSchemaLimits::engine_default();
    let mut measured = admission(&plan, limits);
    let expected = source
        .selected_producer_spec(&values, None, &mut measured)
        .unwrap();
    let work = measured.work().consumed();
    assert!(work > 0);
    let mut exact = admission(
        &plan,
        RuntimeSchemaLimits {
            max_validation_work: work,
            ..limits
        },
    );
    assert_eq!(
        source
            .selected_producer_spec(&values, None, &mut exact)
            .unwrap(),
        expected
    );
    assert_eq!(exact.work().consumed(), work);
    let mut short = admission(
        &plan,
        RuntimeSchemaLimits {
            max_validation_work: work - 1,
            ..limits
        },
    );
    let Some(RuntimeBodyTaskSource::Start(target)) = source.request_endpoint().task_source() else {
        unreachable!()
    };
    let expected_type = plan
        .type_table()
        .id_for_semantic(target.plan().argument_types()[1])
        .unwrap();
    let mut previous = None;
    for _ in 0..2 {
        let error = source
            .selected_producer_spec(&values, None, &mut short)
            .unwrap_err();
        let RuntimeTaskProducerSourceError::Admission(error) = error else {
            panic!("{error:?}")
        };
        let AwbcRuntimeValueSnapshotError::ProgramType(program) = &error else {
            panic!("{error:?}")
        };
        let crate::program_types::RuntimeProgramTypeError::PlanValue(plan_error) = program.as_ref()
        else {
            panic!("{error:?}")
        };
        let crate::plan::RuntimePlanValueAdmissionError::Value {
            ty,
            source: resource,
        } = plan_error.as_ref()
        else {
            panic!("{error:?}")
        };
        assert_eq!(*ty, expected_type);
        assert!(
            matches!(resource, RuntimeSchemaError::ValidationWork { path, limit, consumed } if path == "$" && *limit == work - 1 && *consumed == work)
        );
        assert_eq!(short.work().failure().as_ref(), Some(resource));
        assert_eq!(short.status(), Err(error.clone()));
        assert_eq!(short.work().consumed(), work);
        if let Some(previous) = &previous {
            assert_eq!(&error, previous);
        }
        previous = Some(error);
    }
    let mut prefixed = admission(
        &plan,
        RuntimeSchemaLimits {
            max_validation_work: work + 1,
            ..limits
        },
    );
    prefixed.validate_metadata(&17_u64).unwrap();
    source
        .selected_producer_spec(&values, None, &mut prefixed)
        .unwrap();
    assert_eq!(prefixed.work().consumed(), work + 1);
}
