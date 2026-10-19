use super::*;
use crate::entry::RuntimeSchemaLimits;
use crate::pattern::RuntimeCheckedType;
use crate::plan::construction::source::selected_producers;
use crate::plan::{RuntimeTaskPlanIndex, RuntimeTaskPlanSealError, RuntimeTaskPlanSealLimits};
use crate::task::semantic::TaskSemanticMeter;
use crate::task::{
    HostRestartPolicy, NeedProducerRuntimeArgument, RuntimeTaskProducerSourceError, TaskClass,
    TaskPolicy,
};
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission, RuntimeStoredValue, RuntimeValue,
};
use std::cell::Cell;

fn original() -> RuntimePlan {
    selected_producers::tests::fixture(
        2,
        TaskPolicy::JoinSameKey,
        HostRestartPolicy::MustBeQuiescent,
        TaskClass::Io,
    )
}

fn executable() -> AwbcProgram {
    super::selected::tests::executable(2)
}

fn context() -> AwbcVerifyContext<'static> {
    AwbcVerifyContext {
        require_entrypoint: false,
        ..AwbcVerifyContext::default()
    }
}

fn admit(
    program: Arc<AwbcProgram>,
) -> Result<RuntimeTaskSourceProgram, RuntimeTaskSourceProgramError> {
    RuntimeTaskSourceProgram::admit_awbc(
        program,
        AwbcVerifyBudget::default(),
        context(),
        RuntimeTaskSourceArtifactLimits::default(),
        None,
    )
}

fn source(program: &RuntimeTaskSourceProgram) -> RuntimeTaskSourceRef<'_> {
    let expected = crate::task::ExpectedTaskPlanKey::new(
        *program
            .original()
            .task_plans()
            .by_index(RuntimeTaskPlanIndex(0))
            .unwrap()
            .key()
            .as_bytes(),
    );
    let source = program
        .original()
        .task_source_by_expected_key(
            &expected,
            RuntimeTaskPlanSealLimits::default(),
            &mut TaskSemanticMeter::new(1_000_000, 0),
        )
        .unwrap();
    match program.data.as_ref() {
        RuntimeTaskSourceProgramData::Native(_) => source,
        RuntimeTaskSourceProgramData::Awbc { .. } => program
            .awbc_selected_source(
                crate::awbc::schema::AwbcTaskPlanId(0),
                source,
                RuntimeTaskPlanSealLimits::default(),
                &mut TaskSemanticMeter::new(1_000_000, 0),
            )
            .unwrap(),
    }
}

fn arguments() -> [NeedProducerRuntimeArgument; 2] {
    [
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

#[test]
fn awbc_source_program_keeps_completed_source_and_the_actual_backend_value_owner() {
    let executable = Arc::new(executable());
    let program = admit(Arc::clone(&executable)).unwrap();
    assert!(
        matches!(program.program_owner(), RuntimeProgramOwner::Awbc(owner) if Arc::ptr_eq(&owner, &executable))
    );
    let limits = RuntimeSchemaLimits::engine_default();
    let mut bank = RuntimeRetainedValueAdmission::for_program(program.program_owner(), limits);
    let spec = source(&program)
        .selected_producer_spec(&program, &arguments(), None, &mut bank)
        .unwrap();
    assert!(
        spec.source()
            .program_owner()
            .same_program(bank.program_owner().unwrap())
    );
    let native = RuntimeTaskSourceProgram::native(Arc::new(original()));
    let mut native_bank =
        RuntimeRetainedValueAdmission::for_program(native.program_owner(), limits);
    let native_spec = source(&native)
        .selected_producer_spec(&native, &arguments(), None, &mut native_bank)
        .unwrap();
    assert_eq!(
        spec.instance_key().unwrap(),
        native_spec.instance_key().unwrap()
    );
    assert_eq!(
        spec.source().task_plan().key(),
        native_spec.source().task_plan().key()
    );
    assert_ne!(spec, native_spec);
    assert!(spec.source().belongs_to(program.original()));
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
fn awbc_source_program_rejects_native_surrogates_foreign_code_and_source_before_value_views() {
    let executable = Arc::new(executable());
    let program = admit(Arc::clone(&executable)).unwrap();
    let other = admit(Arc::clone(&executable)).unwrap();
    let reads = Cell::new(0);
    let arguments = [
        NeedProducerRuntimeArgument {
            name: None,
            value: Observed(&reads),
        },
        NeedProducerRuntimeArgument {
            name: Some("条件".into()),
            value: Observed(&reads),
        },
    ];
    for owner in [
        RuntimeProgramOwner::Plan(Arc::new(program.original().clone())),
        RuntimeProgramOwner::Awbc(Arc::new((*executable).clone())),
    ] {
        let mut bank = RuntimeRetainedValueAdmission::for_program(
            owner,
            RuntimeSchemaLimits::engine_default(),
        );
        assert!(matches!(
            source(&program).selected_producer_spec(&program, &arguments, None, &mut bank),
            Err(RuntimeTaskProducerSourceError::OriginalProgram)
        ));
    }
    let mut bank = RuntimeRetainedValueAdmission::for_program(
        program.program_owner(),
        RuntimeSchemaLimits::engine_default(),
    );
    assert!(matches!(
        source(&other).selected_producer_spec(&program, &arguments, None, &mut bank),
        Err(RuntimeTaskProducerSourceError::OriginalProgram)
    ));
    assert_eq!(reads.get(), 0);
}

#[test]
fn awbc_source_program_uses_awbc_type_rows_instead_of_its_original_native_types() {
    let mut executable = executable();
    executable.runtime_types.clear();
    assert!(
        original()
            .type_table()
            .id_for_semantic(RuntimeCheckedType::Bool.semantic_identity_digest())
            .is_some()
    );
    assert!(matches!(
        admit(Arc::new(executable)),
        Err(RuntimeTaskSourceProgramError::Awbc(
            AwbcVerifyError::IndexOutOfBounds {
                table: "runtime_types",
                index: 0,
                ..
            }
        ))
    ));
}

#[test]
fn awbc_source_program_retains_both_leases_until_the_final_source_drops() {
    let (spec, code, original) = {
        let executable = Arc::new(executable());
        let code = Arc::downgrade(&executable);
        let program = admit(executable).unwrap();
        let original = Arc::downgrade(&program.data);
        let mut bank = RuntimeRetainedValueAdmission::for_program(
            program.program_owner(),
            RuntimeSchemaLimits::engine_default(),
        );
        let spec = source(&program)
            .selected_producer_spec(&program, &arguments(), None, &mut bank)
            .unwrap();
        (spec, code, original)
    };
    assert!(code.upgrade().is_some());
    assert!(original.upgrade().is_some());
    let cloned = spec.clone();
    drop(spec);
    assert!(code.upgrade().is_some());
    assert!(original.upgrade().is_some());
    let key = cloned.source().task_plan().key();
    assert_eq!(
        cloned
            .source()
            .borrow(
                RuntimeTaskPlanSealLimits::default(),
                &mut TaskSemanticMeter::new(1_000_000, 0)
            )
            .unwrap()
            .task_plan()
            .key(),
        key
    );
    drop(cloned);
    assert!(code.upgrade().is_none());
    assert!(original.upgrade().is_none());
}

#[test]
fn awbc_source_program_rejects_missing_source_code_policy_and_forged_e_before_publication() {
    assert!(matches!(
        admit(Arc::new(AwbcProgram::default())),
        Err(RuntimeTaskSourceProgramError::MissingOriginalSource)
    ));
    let mut invalid = executable();
    invalid.header.abi_version += 1;
    assert!(matches!(
        admit(Arc::new(invalid)),
        Err(RuntimeTaskSourceProgramError::Awbc(
            AwbcVerifyError::UnsupportedAbi { .. }
        ))
    ));
    let mut forged = executable();
    let original = forged.original_source.as_ref().unwrap();
    let mut expected = *original.expected_executable();
    expected[0] ^= 1;
    forged.original_source = Some(crate::plan::RuntimeTaskSourceArtifact::from_encoded_source(
        expected,
        original.bytes().to_vec(),
    ));
    let executable = Arc::new(forged);
    let error = admit(Arc::clone(&executable)).unwrap_err();
    assert!(
        matches!(error, RuntimeTaskSourceProgramError::Source(source) if matches!(source.seal_error(), Some(RuntimeTaskPlanSealError::ExpectedExecutableKeyMismatch)))
    );
    assert_eq!(Arc::strong_count(&executable), 1);
}

#[test]
fn awbc_source_program_honors_the_code_policy_and_original_source_limits() {
    let executable = Arc::new(executable());
    assert!(matches!(
        RuntimeTaskSourceProgram::admit_awbc(
            Arc::clone(&executable),
            AwbcVerifyBudget::default(),
            AwbcVerifyContext {
                runtime_abi_version: 0,
                ..context()
            },
            RuntimeTaskSourceArtifactLimits::default(),
            None,
        ),
        Err(RuntimeTaskSourceProgramError::Awbc(
            AwbcVerifyError::RuntimeAbiTooOld { .. }
        ))
    ));
    let defaults = RuntimeTaskSourceArtifactLimits::default();
    let mut storage = defaults;
    storage.codec.decoded_bytes = 0;
    let mut work = defaults;
    work.seal.max_semantic_work = 0;
    for limits in [storage, work] {
        assert!(matches!(
            RuntimeTaskSourceProgram::admit_awbc(
                Arc::clone(&executable),
                AwbcVerifyBudget::default(),
                context(),
                limits,
                None,
            ),
            Err(RuntimeTaskSourceProgramError::Source(_))
        ));
        assert_eq!(Arc::strong_count(&executable), 1);
    }
}

#[test]
fn awbc_source_program_preserves_inert_arguments_in_the_original_value_bank() {
    let program = admit(Arc::new(executable())).unwrap();
    let owner = program.program_owner();
    let values = arguments();
    let inert: Vec<_> = values
        .iter()
        .map(|argument| NeedProducerRuntimeArgument {
            name: argument.name.clone(),
            value: crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
                &argument.value,
                &owner,
            )
            .unwrap(),
        })
        .collect();
    let mut bank = RuntimeRetainedValueAdmission::for_program(
        owner.clone(),
        RuntimeSchemaLimits::engine_default(),
    );
    let expected = source(&program)
        .selected_producer_spec(&program, &values, None, &mut bank)
        .unwrap();
    let previous_work = bank.work().consumed();
    let restored = source(&program)
        .selected_producer_spec(&program, &inert, None, &mut bank)
        .unwrap();
    assert_eq!(restored, expected);
    assert!(bank.work().consumed() > previous_work);
    assert!(bank.program_owner().unwrap().same_program(&owner));
}

#[test]
fn awbc_source_program_enforces_exact_original_value_work_and_keeps_its_failure() {
    let program = admit(Arc::new(executable())).unwrap();
    let limits = RuntimeSchemaLimits::engine_default();
    let mut measured = RuntimeRetainedValueAdmission::for_program(program.program_owner(), limits);
    let expected = source(&program)
        .selected_producer_spec(&program, &arguments(), None, &mut measured)
        .unwrap();
    let work = measured.work().consumed();
    assert!(work > 0);
    let mut exact = RuntimeRetainedValueAdmission::for_program(
        program.program_owner(),
        RuntimeSchemaLimits {
            max_validation_work: work,
            ..limits
        },
    );
    assert_eq!(
        source(&program)
            .selected_producer_spec(&program, &arguments(), None, &mut exact)
            .unwrap(),
        expected
    );
    assert_eq!(exact.work().consumed(), work);
    let mut short = RuntimeRetainedValueAdmission::for_program(
        program.program_owner(),
        RuntimeSchemaLimits {
            max_validation_work: work - 1,
            ..limits
        },
    );
    let mut previous = None;
    for _ in 0..2 {
        let error = source(&program)
            .selected_producer_spec(&program, &arguments(), None, &mut short)
            .unwrap_err();
        let RuntimeTaskProducerSourceError::Admission(error) = error else {
            panic!("{error:?}")
        };
        assert_eq!(short.status(), Err(error.clone()));
        assert!(short.work().failure().is_some());
        assert_eq!(short.work().consumed(), work);
        if let Some(previous) = &previous {
            assert_eq!(&error, previous);
        }
        previous = Some(error);
    }
}
