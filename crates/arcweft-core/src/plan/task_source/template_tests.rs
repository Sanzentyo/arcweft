use super::*;
use crate::awbc::schema::{AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcTaskPlanId};
use crate::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use crate::entry::RuntimeSchemaLimits;
use crate::plan::*;
use crate::task::*;
use crate::value::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission,
    RuntimeStoredValue, RuntimeValue, RuntimeValueSource,
};
use std::cell::Cell;
use std::sync::Arc;

fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(1_000_000, 1_000_000)
}
fn native() -> RuntimeTaskSourceProgram {
    RuntimeTaskSourceProgram::native(Arc::new(program::template_original_fixture()))
}
fn compiled() -> RuntimeTaskSourceProgram {
    RuntimeTaskSourceProgram::admit_awbc(
        Arc::new(program::template_executable_fixture()),
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
fn source(program: &RuntimeTaskSourceProgram, child: bool) -> RuntimeTaskSourceRef<'_> {
    let id = u32::from(child);
    if matches!(program.program_owner(), RuntimeProgramOwner::Awbc(_)) {
        program
            .awbc_template_source(
                AwbcTaskPlanId(id),
                RuntimeTaskPlanSealLimits::default(),
                &mut meter(),
            )
            .unwrap()
    } else {
        let key = program
            .original()
            .task_plans()
            .by_index(RuntimeTaskPlanIndex(id))
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
}
fn bank(
    program: &RuntimeTaskSourceProgram,
    limits: RuntimeSchemaLimits,
) -> RuntimeRetainedValueAdmission {
    RuntimeRetainedValueAdmission::for_program(program.program_owner(), limits)
}
fn values() -> [RuntimeValue; 2] {
    [RuntimeValue::Bool(true), RuntimeValue::Bool(false)]
}
fn input() -> RuntimeValue {
    RuntimeValue::Seq(crate::value::RuntimeSeq::values(vec![
        RuntimeValue::Bool(false),
        RuntimeValue::Bool(true),
    ]))
}

#[test]
fn template_materialization_keeps_original_metadata_and_rejects_foreign_or_failed_banks() {
    for program in [native(), compiled()] {
        let base = source(&program, false);
        let child = source(&program, true);
        let captures = values();
        let source_value = input();
        let mut admission = bank(&program, RuntimeSchemaLimits::engine_default());
        let checked = base
            .template_producer_spec(
                &program,
                RuntimeTaskTemplateArguments::AwaitManyBase {
                    captures: &captures,
                    source: RuntimeValueSource::Live(&source_value),
                },
                None,
                &mut admission,
            )
            .unwrap();
        let request = || {
            HostTaskRequest::custom_with_named_args(
                "source",
                "request",
                [crate::value::RuntimePayload(captures[0].clone())],
                [(
                    "条件".into(),
                    crate::value::RuntimePayload(captures[1].clone()),
                )],
            )
        };
        let generation = GenerationId::new(13);
        let spec = base
            .template_task_spec(&checked, generation, request(), &mut admission)
            .unwrap();
        assert_eq!(spec.generation, generation);
        assert_eq!(
            spec.producer,
            NeedProducerInstance::try_from(&checked).unwrap()
        );
        assert_eq!(spec.class, TaskClass::Cpu);
        assert_eq!(spec.priority, TaskPriority(-3));
        assert_eq!(spec.cancel_scope, CancelScopeId("元scope".into()));
        assert_eq!(spec.policy, TaskPolicy::AlwaysStart);
        assert_eq!(spec.debug_label, "template source fixture");
        assert_eq!(
            spec.outcome.payload_semantic_identity().as_bytes(),
            checked.payload_type().as_bytes()
        );
        assert!(matches!(
            child.template_task_spec(&checked, generation, request(), &mut admission),
            Err(RuntimeTaskProducerSourceError::OriginalProgram)
        ));

        let foreign = match program.program_owner() {
            RuntimeProgramOwner::Plan(_) => native(),
            RuntimeProgramOwner::Awbc(_) => compiled(),
        };
        let foreign_base = source(&foreign, false);
        assert_eq!(base.task_plan().key(), foreign_base.task_plan().key());
        assert!(matches!(
            foreign_base.template_task_spec(&checked, generation, request(), &mut admission),
            Err(RuntimeTaskProducerSourceError::OriginalProgram)
        ));
        assert!(matches!(
            base.template_task_spec(
                &checked,
                generation,
                request(),
                &mut bank(&foreign, RuntimeSchemaLimits::engine_default())
            ),
            Err(RuntimeTaskProducerSourceError::OriginalProgram)
        ));

        let failure = admission
            .validate_metadata_collection(usize::MAX)
            .unwrap_err();
        let Err(RuntimeTaskProducerSourceError::Admission(retained)) =
            base.template_task_spec(&checked, generation, request(), &mut admission)
        else {
            panic!("a failed shared bank cannot construct a task specification")
        };
        assert_eq!(retained, failure);
    }
}

#[test]
fn host_template_sources_cannot_be_relabelled_as_program_work() {
    for program in [native(), compiled()] {
        let original = source(&program, false);
        let captures = values();
        let input = input();
        let mut admission = bank(&program, RuntimeSchemaLimits::engine_default());
        let checked = original
            .template_producer_spec(
                &program,
                RuntimeTaskTemplateArguments::AwaitManyBase {
                    captures: &captures,
                    source: RuntimeValueSource::Live(&input),
                },
                None,
                &mut admission,
            )
            .unwrap();
        let request = HostTaskRequest::custom_with_named_args(
            "source",
            "request",
            [crate::value::RuntimePayload(captures[0].clone())],
            [(
                "条件".into(),
                crate::value::RuntimePayload(captures[1].clone()),
            )],
        );
        let mut spec = original
            .template_task_spec(&checked, GenerationId::new(13), request, &mut admission)
            .unwrap();
        assert!(matches!(
            spec.producer.handle_type(),
            TaskHandleType::Need { .. }
        ));
        spec.request = TaskRequest::Program(checked.source().clone());
        let mut registry = NeedProducerRegistry::default();
        let before = registry.launch_frontiers();
        assert_eq!(
            registry
                .ensure_program_task_with_binding::<(), TaskEnsureError>(spec, |_| {
                    panic!("a Host template grants no original program body")
                })
                .unwrap_err(),
            TaskEnsureError::ProgramSourceMismatch,
        );
        assert_eq!(registry.launch_frontiers(), before);
        assert!(registry.task_admissions().next().is_none());
    }
}

#[test]
fn template_sources_keep_original_keys_and_exact_nested_argument_transcripts() {
    let native = native();
    let code = compiled();
    let captures = values();
    let source_value = input();
    let items = values();
    let borrowed_captures: Vec<_> = captures.iter().collect();
    let mut keys = Vec::new();
    for program in [&native, &code] {
        let base = source(program, false);
        let child = source(program, true);
        let mut admission = bank(program, RuntimeSchemaLimits::engine_default());
        let base_spec = base
            .template_producer_spec(
                program,
                RuntimeTaskTemplateArguments::AwaitManyBase {
                    captures: &borrowed_captures,
                    source: RuntimeValueSource::Live(&source_value),
                },
                None,
                &mut admission,
            )
            .unwrap();
        let child_spec = child
            .template_producer_spec(
                program,
                RuntimeTaskTemplateArguments::AwaitManyChild {
                    captures: &captures,
                    index: 7,
                    item: RuntimeValueSource::Live(&items[0]),
                },
                None,
                &mut admission,
            )
            .unwrap();
        let expected_base = RuntimeValue::Tuple(vec![
            RuntimeValue::Tuple(captures.to_vec()),
            RuntimeValue::Tuple(vec![RuntimeValue::Bool(false), RuntimeValue::Bool(true)]),
        ]);
        let expected_child = RuntimeValue::Tuple(vec![
            RuntimeValue::Tuple(captures.to_vec()),
            RuntimeValue::u32(7),
            items[0].clone(),
        ]);
        let base_arguments = crate::entry::schema::canonical_runtime_value_view_digest(
            expected_base.view(),
            1 << 20,
        )
        .unwrap();
        let child_arguments = crate::entry::schema::canonical_runtime_value_view_digest(
            expected_child.view(),
            1 << 20,
        )
        .unwrap();
        let Some(crate::plan::body_semantic::flow::RuntimeBodyTaskSource::Producer(base_metadata)) =
            base.request_endpoint().task_source()
        else {
            unreachable!()
        };
        let expected = NeedProducerSpec::new(
            base.task_plan().definition().family(),
            base_metadata.definition.contract,
            base.task_plan().key(),
            base_metadata.definition.producer_site,
            base_metadata.definition.handle_type,
            base_arguments,
        );
        assert_eq!(
            base_spec.instance_key().unwrap(),
            expected.instance_key().unwrap()
        );
        let Some(crate::plan::body_semantic::flow::RuntimeBodyTaskSource::Producer(child_metadata)) =
            child.request_endpoint().task_source()
        else {
            unreachable!()
        };
        let expected = NeedProducerSpec::new(
            child.task_plan().definition().family(),
            child_metadata.definition.contract,
            child.task_plan().key(),
            child_metadata.definition.producer_site,
            child_metadata.definition.handle_type,
            child_arguments,
        );
        assert_eq!(
            child_spec.instance_key().unwrap(),
            expected.instance_key().unwrap()
        );
        assert_eq!(base_spec.source().task_plan().key(), base.task_plan().key());
        assert_eq!(
            child_spec.source().task_plan().key(),
            child.task_plan().key()
        );
        assert_ne!(
            base_spec.instance_key().unwrap(),
            child_spec.instance_key().unwrap()
        );
        keys.push((
            base_spec.instance_key().unwrap(),
            child_spec.instance_key().unwrap(),
        ));
    }
    assert_eq!(keys[0], keys[1]);
}

struct Observed<'a> {
    reads: &'a Cell<usize>,
    value: &'a RuntimeValue,
}

#[test]
fn template_sources_bound_debug_metadata_before_borrowing_captures() {
    for program in [native(), compiled()] {
        let base = source(&program, false);
        let reads = Cell::new(0);
        let values = values();
        let captures = values
            .iter()
            .map(|value| Observed {
                reads: &reads,
                value,
            })
            .collect::<Vec<_>>();
        let source_value = input();
        let mut admission = bank(
            &program,
            RuntimeSchemaLimits {
                max_string_bytes: 10,
                ..RuntimeSchemaLimits::engine_default()
            },
        );
        let Err(RuntimeTaskProducerSourceError::Admission(error)) = base.template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::AwaitManyBase {
                captures: &captures,
                source: RuntimeValueSource::Live(&source_value),
            },
            None,
            &mut admission,
        ) else {
            panic!("original debug metadata must be bounded before issuing the producer");
        };
        assert_eq!(
            error,
            AwbcRuntimeValueSnapshotError::Resource(
                crate::entry::RuntimeSchemaError::BudgetExceeded {
                    budget: "string_bytes"
                },
            )
        );
        assert_eq!(reads.get(), 0);
        assert_eq!(admission.status(), Err(error));
    }
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
fn template_sources_preflight_owner_kind_and_complete_capture_count_before_value_views() {
    let program = compiled();
    let reads = Cell::new(0);
    let values = values();
    let captures = [Observed {
        reads: &reads,
        value: &values[0],
    }];
    let item = Observed {
        reads: &reads,
        value: &values[1],
    };
    let child = source(&program, true);
    assert!(matches!(
        child.template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::AwaitManyChild {
                captures: &captures,
                index: 0,
                item: RuntimeValueSource::Live(item.value)
            },
            None,
            &mut bank(&program, RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimeTaskProducerSourceError::CaptureCount {
            actual: 1,
            expected: 2
        })
    ));
    assert!(matches!(
        child.template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::ProgramTask {
                captures: &captures
            },
            None,
            &mut bank(&program, RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimeTaskProducerSourceError::RequestKind)
    ));
    let key = child.task_plan().key();
    let raw = program
        .original()
        .task_source_by_expected_key(
            &ExpectedTaskPlanKey::new(*key.as_bytes()),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter(),
        )
        .unwrap();
    assert!(matches!(
        raw.template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::AwaitManyChild {
                captures: &captures,
                index: 0,
                item: RuntimeValueSource::Live(item.value)
            },
            None,
            &mut bank(&program, RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimeTaskProducerSourceError::OriginalProgram)
    ));
    assert_eq!(reads.get(), 0);
}

struct SnapshotView<'a> {
    reads: &'a Cell<usize>,
    value: &'a AwbcRuntimeValueSnapshot,
}
impl RuntimeStoredValue for SnapshotView<'_> {
    fn borrowed_view(&self) -> crate::value::RuntimeValueView<'_> {
        self.reads.set(self.reads.get() + 1);
        self.value.view()
    }
    fn admission_origin() -> crate::entry::schema::value_encoding::ValueAdmissionOrigin {
        crate::entry::schema::value_encoding::ValueAdmissionOrigin::InertSnapshot
    }
}

#[test]
fn template_source_count_preflight_rejects_virtual_fanout_before_capture_or_child_views() {
    let program = compiled();
    let source = source(&program, false);
    let reads = Cell::new(0);
    let captured = AwbcRuntimeValueSnapshot::Bool(true);
    let count = u64::from(u32::MAX) + 1;
    let oversized: AwbcRuntimeValueSnapshot = serde_json::from_value(
        serde_json::json!({ "Seq": { "TupleColumns": { "len": count, "columns": [] } } }),
    )
    .unwrap();
    let captures = [
        SnapshotView {
            reads: &reads,
            value: &captured,
        },
        SnapshotView {
            reads: &reads,
            value: &captured,
        },
    ];
    let mut admission = bank(&program, RuntimeSchemaLimits::engine_default());
    let error = source
        .template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::AwaitManyBase {
                captures: &captures,
                source: RuntimeValueSource::Snapshot(&oversized),
            },
            None,
            &mut admission,
        )
        .unwrap_err();
    match usize::try_from(count) {
        Ok(expected) => assert!(
            matches!(error, RuntimeTaskProducerSourceError::SourceCount { actual } if actual == expected)
        ),
        Err(_) => {
            let RuntimeTaskProducerSourceError::Admission(error) = error else {
                panic!("{error:?}")
            };
            assert_eq!(admission.status(), Err(error));
        }
    }
    assert_eq!(
        reads.get(),
        0,
        "no capture or source child is projected; no oversized backing graph is allocated"
    );
}

#[test]
fn template_sources_check_capture_source_and_item_types_against_the_actual_program() {
    for program in [native(), compiled()] {
        let wrong = RuntimeValue::String("wrong".into());
        let captures = values();
        let base = source(&program, false);
        let child = source(&program, true);
        assert!(matches!(
            child.template_producer_spec(
                &program,
                RuntimeTaskTemplateArguments::AwaitManyChild {
                    captures: &[wrong.clone(), captures[1].clone()],
                    index: 0,
                    item: RuntimeValueSource::Live(&captures[0])
                },
                None,
                &mut bank(&program, RuntimeSchemaLimits::engine_default())
            ),
            Err(RuntimeTaskProducerSourceError::Admission(
                AwbcRuntimeValueSnapshotError::ProgramType(_)
            ))
        ));
        assert!(matches!(
            child.template_producer_spec(
                &program,
                RuntimeTaskTemplateArguments::AwaitManyChild {
                    captures: &captures,
                    index: 0,
                    item: RuntimeValueSource::Live(&wrong)
                },
                None,
                &mut bank(&program, RuntimeSchemaLimits::engine_default())
            ),
            Err(RuntimeTaskProducerSourceError::Admission(
                AwbcRuntimeValueSnapshotError::ProgramType(_)
            ))
        ));
        let invalid_source = RuntimeValue::Seq(crate::value::RuntimeSeq::values(vec![wrong]));
        assert!(matches!(
            base.template_producer_spec(
                &program,
                RuntimeTaskTemplateArguments::AwaitManyBase {
                    captures: &captures,
                    source: RuntimeValueSource::Live(&invalid_source)
                },
                None,
                &mut bank(&program, RuntimeSchemaLimits::engine_default())
            ),
            Err(RuntimeTaskProducerSourceError::Admission(
                AwbcRuntimeValueSnapshotError::ProgramType(_)
            ))
        ));
    }
}

#[test]
fn template_sources_preserve_inert_storage_and_one_exact_sticky_bank() {
    let program = compiled();
    let owner = program.program_owner();
    let captures = values();
    let item = RuntimeValue::Bool(true);
    let inert_captures: Vec<_> = captures
        .iter()
        .map(|value| {
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(value, &owner).unwrap()
        })
        .collect();
    let inert_item =
        AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&item, &owner).unwrap();
    let source = source(&program, true);
    let limits = RuntimeSchemaLimits::engine_default();
    let mut admission = bank(&program, limits);
    let expected = source
        .template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::AwaitManyChild {
                captures: &captures,
                index: 1,
                item: RuntimeValueSource::Live(&item),
            },
            None,
            &mut admission,
        )
        .unwrap();
    let work = admission.work().consumed();
    let actual = source
        .template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::AwaitManyChild {
                captures: &inert_captures,
                index: 1,
                item: RuntimeValueSource::Snapshot(&inert_item),
            },
            None,
            &mut admission,
        )
        .unwrap();
    assert_eq!(expected, actual);
    assert!(admission.work().consumed() > work);
    let base = self::source(&program, false);
    let source_value = input();
    let inert_source =
        AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&source_value, &owner).unwrap();
    let expected_base = base
        .template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::AwaitManyBase {
                captures: &captures,
                source: RuntimeValueSource::Live(&source_value),
            },
            None,
            &mut admission,
        )
        .unwrap();
    let actual_base = base
        .template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::AwaitManyBase {
                captures: &inert_captures,
                source: RuntimeValueSource::Snapshot(&inert_source),
            },
            None,
            &mut admission,
        )
        .unwrap();
    assert_eq!(expected_base, actual_base);
    let mixed_base = base
        .template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::AwaitManyBase {
                captures: &captures,
                source: RuntimeValueSource::Snapshot(&inert_source),
            },
            None,
            &mut admission,
        )
        .unwrap();
    assert_eq!(mixed_base, expected_base);
    let mixed_child = source
        .template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::AwaitManyChild {
                captures: &inert_captures,
                index: 1,
                item: RuntimeValueSource::Live(&item),
            },
            None,
            &mut admission,
        )
        .unwrap();
    assert_eq!(mixed_child, expected);
    let mut exact = bank(
        &program,
        RuntimeSchemaLimits {
            max_validation_work: work,
            ..limits
        },
    );
    assert_eq!(
        source
            .template_producer_spec(
                &program,
                RuntimeTaskTemplateArguments::AwaitManyChild {
                    captures: &captures,
                    index: 1,
                    item: RuntimeValueSource::Live(&item)
                },
                None,
                &mut exact
            )
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
    let mut prior = None;
    for _ in 0..2 {
        let error = source
            .template_producer_spec(
                &program,
                RuntimeTaskTemplateArguments::AwaitManyChild {
                    captures: &captures,
                    index: 1,
                    item: RuntimeValueSource::Live(&item),
                },
                None,
                &mut short,
            )
            .unwrap_err();
        let RuntimeTaskProducerSourceError::Admission(error) = error else {
            panic!("{error:?}")
        };
        assert_eq!(short.status(), Err(error.clone()));
        if let Some(prior) = &prior {
            assert_eq!(prior, &error);
        }
        prior = Some(error);
    }
}

#[test]
fn template_source_leases_keep_the_actual_original_and_code_alive_until_the_last_spec() {
    let code = Arc::new(program::template_executable_fixture());
    let weak = Arc::downgrade(&code);
    let program = RuntimeTaskSourceProgram::admit_awbc(
        Arc::clone(&code),
        AwbcVerifyBudget::default(),
        AwbcVerifyContext {
            require_entrypoint: false,
            ..AwbcVerifyContext::default()
        },
        RuntimeTaskSourceArtifactLimits::default(),
        None,
    )
    .unwrap();
    let source = source(&program, true);
    let captures = values();
    let spec = source
        .template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::AwaitManyChild {
                captures: &captures,
                index: 0,
                item: RuntimeValueSource::Live(&captures[0]),
            },
            None,
            &mut bank(&program, RuntimeSchemaLimits::engine_default()),
        )
        .unwrap();
    drop(source);
    drop(program);
    drop(code);
    assert!(weak.upgrade().is_some());
    let clone = spec.clone();
    drop(spec);
    let borrowed = clone
        .source()
        .borrow(RuntimeTaskPlanSealLimits::default(), &mut meter())
        .unwrap();
    assert_eq!(borrowed.task_plan().key(), clone.source().task_plan().key());
    drop(borrowed);
    drop(clone);
    assert!(weak.upgrade().is_none());
}

#[test]
fn template_source_container_graphs_reject_same_identity_with_another_shape_before_values() {
    let mut code = program::template_executable_fixture();
    // The source type and aggregate payload share this row in the fixture.
    // A different homogeneous container with the same ID remains verified.
    let identity = code.runtime_types[1].semantic_identity();
    code.runtime_types[1] = AwbcRuntimeType::new(
        identity,
        AwbcRuntimeTypeShape::Sequence {
            kind: RuntimePlanSequenceKind::Slice,
            item: crate::awbc::schema::AwbcTypeId(0),
        },
    );
    let program = RuntimeTaskSourceProgram::admit_awbc(
        Arc::new(code),
        AwbcVerifyBudget::default(),
        AwbcVerifyContext {
            require_entrypoint: false,
            ..AwbcVerifyContext::default()
        },
        RuntimeTaskSourceArtifactLimits::default(),
        None,
    )
    .unwrap();
    assert!(matches!(
        program.awbc_template_source(
            AwbcTaskPlanId(1),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter()
        ),
        Err(RuntimeAwbcTemplateSourceError::Type {
            field: RuntimeAwbcTemplateSourceField::SourceType,
            ..
        })
    ));
}

pub(in crate::plan::task_source) fn thread() -> RuntimeTaskSourceProgram {
    program_context(false, 1, false)
}

pub(in crate::plan::task_source) fn program_need() -> RuntimeTaskSourceProgram {
    program_context(false, 1, true)
}

pub(in crate::plan::task_source) fn thread_with_lexical_context() -> RuntimeTaskSourceProgram {
    program_context(true, 1, false)
}

pub(in crate::plan::task_source) fn thread_with_captures(count: usize) -> RuntimeTaskSourceProgram {
    program_context(false, count, false)
}

fn program_context(
    lexical_context: bool,
    capture_count: usize,
    ordinary_need: bool,
) -> RuntimeTaskSourceProgram {
    let mut builder = RuntimePlanBuilder::new();
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let boolean = crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest();
    let handle = RuntimeSemanticTypeId::from_bytes([20; 32]);
    let function_type = RuntimeSemanticTypeId::from_bytes([40; 32]);
    let locals = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    handle,
                    if ordinary_need {
                        RuntimePlanTypeProjection::Need(unit)
                    } else {
                        RuntimePlanTypeProjection::ThreadHandle(unit)
                    },
                ),
                RuntimePlanTypeSeed::new(
                    function_type,
                    RuntimePlanTypeProjection::Function {
                        contract: crate::plan::RuntimeFunctionTypeContract::default(),
                        parameters: Box::new([]),
                        result: unit,
                    },
                ),
            ],
            (0..capture_count).map(|index| {
                RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalDeclarationSource::Binding {
                        identity: {
                            let mut identity = [31; 32];
                            if index != 0 {
                                identity[..8].copy_from_slice(&(index as u64).to_le_bytes());
                            }
                            identity
                        },
                        declaration: RuntimeLocalBindingDeclaration::new(
                            RuntimeLocalBindingKind::PatternBinding,
                            false,
                            RuntimeLocalBindingStorage::Derived,
                        ),
                    },
                    boolean,
                )
            }),
        )
        .unwrap();
    let mut functions = Vec::new();
    for marker in [1, 2] {
        functions.push(
            builder
                .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                    definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [marker; 32],
                    ),
                    role: RuntimeFunctionSemanticRole::Ordinary,
                    function_type: (lexical_context && marker == 1).then_some(function_type),
                    inputs: Box::new([]),
                    result: unit,
                    body_kind: RuntimeFunctionSiteBodyKind::Executable,
                    effects: RuntimeEffectSet::empty(),
                })
                .unwrap(),
        );
    }
    let unit_value = RuntimeExprSeed::new(unit, RuntimeExprSeedKind::Value(RuntimeValue::Unit));
    builder
        .define_function_site_seed(
            &functions[1],
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(unit_value.clone())]),
            }),
        )
        .unwrap();
    let captures = locals.local_ids().to_vec();
    let coordinate = builder
        .task_plan_construction()
        .reserve_original_source()
        .unwrap();
    let mut ops = captures
        .iter()
        .map(|local| RuntimeFlowOpSeed::Let {
            pattern: RuntimePatternSeed::new(
                boolean,
                RuntimePatternSeedKind::Bind {
                    local: local.clone(),
                    mutable: false,
                },
            ),
            expr: RuntimeExprSeed::new(
                boolean,
                RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
            ),
        })
        .collect::<Vec<_>>();
    ops.push(RuntimeFlowOpSeed::ProgramTask {
        name: None,
        binding: RuntimePatternSeed::new(handle, RuntimePatternSeedKind::Discard),
        captures,
        producer: crate::task::NeedProducerDefinition {
            function: RuntimeFunctionDefinitionIdentity::from_accepted_identity([2; 32]),
            family: NeedProducerFamily::StructuredTaskPlan,
            contract: NeedProducerContractDigest::from_bytes([21; 32]),
            plan: coordinate,
            producer_site: NeedProducerSiteDigest::from_bytes([22; 32]),
            handle_type: if ordinary_need {
                crate::task::TaskHandleType::Need {
                    payload: RuntimeTypeSemanticDigest::from_bytes(*unit.as_bytes()),
                }
            } else {
                crate::task::TaskHandleType::ThreadHandle {
                    payload: RuntimeTypeSemanticDigest::from_bytes(*unit.as_bytes()),
                }
            },
            class: TaskClass::Cpu,
            priority: TaskPriority(0),
            cancel_scope: CancelScopeId("thread".into()),
            policy: TaskPolicy::AlwaysStart,
            debug_label: "thread source".into(),
        },
        body: vec![RuntimeFlowOpSeed::ReturnExpr(unit_value.clone())],
    });
    ops.push(RuntimeFlowOpSeed::ReturnExpr(unit_value));
    builder
        .define_function_site_seed(
            &functions[0],
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: ops.into_boxed_slice(),
            }),
        )
        .unwrap();
    RuntimeTaskSourceProgram::native(Arc::new(builder.finish().unwrap()))
}

#[test]
fn thread_source_codec_retains_program_definition_and_capture_roles() {
    let program = thread();
    let limits = RuntimeTaskSourceArtifactLimits::default();
    let artifact = program.original().task_source_artifact(limits).unwrap();
    let decoded = artifact.admit(limits, None).unwrap();
    assert_eq!(
        decoded.executable_semantic_digest(),
        program.original().executable_semantic_digest(),
    );
    assert_eq!(decoded.task_source_artifact(limits).unwrap(), artifact);
    let row = decoded
        .task_plans()
        .by_index(RuntimeTaskPlanIndex(0))
        .unwrap();
    let source = decoded
        .task_source_by_expected_key(
            &ExpectedTaskPlanKey::new(*row.key().as_bytes()),
            limits.seal,
            &mut meter(),
        )
        .unwrap();
    assert!(matches!(
        source.request_endpoint().task_source(),
        Some(crate::plan::body_semantic::flow::RuntimeBodyTaskSource::ProgramProducer(_))
    ));
    assert!(source.request_endpoint().host_arguments().is_none());
    let request = source
        .request_endpoint()
        .definition(&mut meter(), limits.seal)
        .unwrap();
    let [capture] = request.arguments() else {
        panic!("the original Thread has one capture role");
    };
    assert_eq!(capture.role, RuntimeRequestArgumentRole::Capture);
    assert_eq!(capture.identity, None);
    assert_eq!(
        decoded
            .type_table()
            .get(capture.ty)
            .unwrap()
            .semantic_identity(),
        crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest(),
    );
    assert_eq!(capture.source, RuntimeRequestValueSource::Local);
    assert_eq!(&*capture.path, &[RuntimeRequestPathStep::Capture(0)]);
    let mut no_roles = limits.seal;
    no_roles.max_request_roles = 0;
    assert!(matches!(
        source.request_endpoint().definition(&mut meter(), no_roles),
        Err(RuntimeBodySemanticError::RequestRoles {
            actual: 1,
            maximum: 0
        })
    ));
    let Some(FlowOp::ProgramTask {
        captures, producer, ..
    }) = source.request_endpoint().operation()
    else {
        panic!("round trip must retain the actual Thread operation");
    };
    assert_eq!(captures.len(), 1);
    assert_eq!(
        producer.function,
        RuntimeFunctionDefinitionIdentity::from_accepted_identity([2; 32])
    );
    assert_eq!(producer.family, NeedProducerFamily::StructuredTaskPlan);
    assert_eq!(
        producer.handle_type,
        TaskHandleType::ThreadHandle {
            payload: RuntimeTypeSemanticDigest::from_bytes(
                *crate::pattern::RuntimeCheckedType::Unit
                    .semantic_identity_digest()
                    .as_bytes()
            ),
        }
    );
    // Corrupt the actual encoded producer field. Borrowed admission must reject
    // its carrier before materializing any definition or issuing a Source lease.
    let mut prefix = producer.function.as_bytes().to_vec();
    prefix.push(producer.family.semantic_tag());
    prefix.extend_from_slice(producer.contract.as_bytes());
    prefix.extend_from_slice(&producer.plan.ordinal().to_le_bytes());
    prefix.extend_from_slice(producer.producer_site.as_bytes());
    let offsets = artifact
        .bytes()
        .windows(prefix.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == prefix).then_some(offset + prefix.len()))
        .collect::<Vec<_>>();
    let [offset] = offsets.as_slice() else {
        panic!("the original producer has exactly one encoded definition");
    };
    for tag in [2, u8::MAX] {
        let mut bytes = artifact.bytes().to_vec();
        bytes[*offset] = tag;
        let error = RuntimeTaskSourceArtifact::preflight_encoded(&bytes, limits).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&format!("task handle type tag {tag}")),
            "{error}"
        );
    }
}

#[test]
fn awbc_host_request_kernel_cannot_claim_an_original_thread_source() {
    use crate::awbc::schema::{AwbcHostTaskPlanKind, AwbcTypeId};
    let native = thread();
    let original = native.original();
    let row = original
        .task_plans()
        .by_index(RuntimeTaskPlanIndex(0))
        .unwrap();
    let source = original
        .task_source_by_expected_key(
            &ExpectedTaskPlanKey::new(*row.key().as_bytes()),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter(),
        )
        .unwrap();
    let Some(crate::plan::body_semantic::flow::RuntimeBodyTaskSource::ProgramProducer(definition)) =
        source.request_endpoint().task_source()
    else {
        panic!("Thread has an actual program producer");
    };
    let mut code = program::template_executable_fixture();
    code.original_source = Some(
        original
            .task_source_artifact(RuntimeTaskSourceArtifactLimits::default())
            .unwrap(),
    );
    let AwbcHostTaskPlanKind::AwaitMany {
        request_function, ..
    } = code.task_plans[0].host_mut().expect("Host fixture").kind
    else {
        panic!("fixture owns a real Host request kernel");
    };
    code.task_plans[0].host_mut().expect("Host fixture").kind = AwbcHostTaskPlanKind::Template {
        family: definition.family,
        contract: definition.contract,
        site: definition.producer_site,
        plan: row.key(),
        request_function,
    };
    code.task_plans[0].handle_type = code.task_plans[0]
        .handle_type
        .map_payload(|_| AwbcTypeId(u32::try_from(code.runtime_types.len()).unwrap()));
    code.runtime_types.push(AwbcRuntimeType::new(
        crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest(),
        AwbcRuntimeTypeShape::Unit,
    ));
    let program = RuntimeTaskSourceProgram::admit_awbc(
        Arc::new(code),
        AwbcVerifyBudget::default(),
        AwbcVerifyContext {
            require_entrypoint: false,
            ..AwbcVerifyContext::default()
        },
        RuntimeTaskSourceArtifactLimits::default(),
        None,
    )
    .unwrap();
    assert!(matches!(
        program.awbc_template_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter()
        ),
        Err(program::RuntimeAwbcTemplateSourceError::Mismatch {
            field: program::RuntimeAwbcTemplateSourceField::RequestKind,
            ..
        }),
    ));
}

#[test]
fn thread_source_uses_original_capture_order_and_retains_only_an_admitted_source() {
    let program = thread();
    let key = program
        .original()
        .task_plans()
        .by_index(RuntimeTaskPlanIndex(0))
        .unwrap()
        .key();
    let source = program
        .original()
        .task_source_by_expected_key(
            &ExpectedTaskPlanKey::new(*key.as_bytes()),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter(),
        )
        .unwrap();
    let captures = [RuntimeValue::Bool(true)];
    let spec = source
        .template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::ProgramTask {
                captures: &captures,
            },
            None,
            &mut bank(&program, RuntimeSchemaLimits::engine_default()),
        )
        .unwrap();
    let Some(crate::plan::body_semantic::flow::RuntimeBodyTaskSource::ProgramProducer(original)) =
        source.request_endpoint().task_source()
    else {
        unreachable!()
    };
    assert_eq!(
        source.request_endpoint().kind(),
        crate::plan::body_semantic::function::EndpointKind::ProgramTask
    );
    assert!(source.request_endpoint().host_arguments().is_none());
    let digest = crate::entry::schema::canonical_runtime_value_view_digest(
        crate::value::RuntimeValueView::Tuple(crate::value::RuntimeTupleView::Values(&captures)),
        1 << 20,
    )
    .unwrap();
    let expected = NeedProducerSpec::new(
        NeedProducerFamily::StructuredTaskPlan,
        original.contract,
        key,
        original.producer_site,
        original.handle_type,
        digest,
    );
    assert_eq!(
        spec.instance_key().unwrap(),
        expected.instance_key().unwrap()
    );
    assert_eq!(spec.source().task_plan().key(), key);
    let task = TaskSpec {
        generation: GenerationId::new(0),
        producer: NeedProducerInstance::try_from(&spec).unwrap(),
        class: original.class.clone(),
        priority: original.priority,
        cancel_scope: original.cancel_scope.clone(),
        policy: original.policy,
        outcome: original.outcome(),
        request: TaskRequest::Program(spec.source().clone()),
        debug_label: original.debug_label.clone(),
    };
    let mut registry = NeedProducerRegistry::default();
    let before = registry.launch_frontiers();
    let mut wrong_carrier = task.clone();
    let changed = NeedProducerSpec::new(
        spec.family(),
        spec.contract(),
        key,
        spec.producer_site(),
        crate::task::TaskHandleType::Need {
            payload: spec.payload_type(),
        },
        spec.arguments(),
    );
    wrong_carrier.producer = NeedProducerInstance::try_from(&changed).unwrap();
    assert_eq!(
        registry
            .ensure_program_task_with_binding::<(), TaskEnsureError>(wrong_carrier, |_| {
                panic!("an original Thread cannot be relabelled as an ordinary Need")
            })
            .unwrap_err(),
        TaskEnsureError::OutcomeContractMismatch
    );
    assert_eq!(registry.launch_frontiers(), before);
    assert!(registry.task_admissions().next().is_none());
    let mut standalone = task.clone();
    standalone.outcome = TaskOutcomeContract::new(crate::pattern::RuntimeCheckedType::Unit);
    assert_eq!(
        standalone.outcome.payload_semantic_identity(),
        task.outcome.payload_semantic_identity(),
        "equal payload digests do not replace the original program type authority"
    );
    assert_eq!(
        registry
            .ensure_program_task_with_binding::<(), TaskEnsureError>(standalone, |_| {
                panic!("a standalone predicate cannot publish original program work")
            })
            .unwrap_err(),
        TaskEnsureError::OutcomeContractMismatch
    );
    assert_eq!(registry.launch_frontiers(), before);
    assert!(registry.task_admissions().next().is_none());

    #[derive(Debug, PartialEq)]
    enum BindingFailure {
        Admission(TaskEnsureError),
        Rejected,
    }
    impl From<TaskEnsureError> for BindingFailure {
        fn from(error: TaskEnsureError) -> Self {
            Self::Admission(error)
        }
    }
    let mut observed_actual_receipt = false;
    let rejected = registry.ensure_program_task_with_binding(task.clone(), |receipt| {
        let handle = crate::task::RuntimeNeedHandle::try_from(receipt.clone()).unwrap();
        assert_eq!(handle.correlation(), receipt.handle().correlation);
        assert!(handle.requires_producer_custody());
        observed_actual_receipt = true;
        Err::<(), _>(BindingFailure::Rejected)
    });
    assert_eq!(rejected.unwrap_err(), BindingFailure::Rejected);
    assert!(observed_actual_receipt);
    assert_eq!(registry.launch_frontiers(), before);
    assert!(registry.task_admissions().next().is_none());
    assert!(registry.launches().next().is_none());

    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        registry.ensure_program_task_with_binding::<(), TaskEnsureError>(task.clone(), |_| {
            panic!("injected binding unwind before child or launch publication")
        })
    }));
    assert!(unwound.is_err());
    assert_eq!(registry.launch_frontiers(), before);
    assert!(registry.task_admissions().next().is_none());
    assert!(registry.launches().next().is_none());

    let (accepted, launch, ()) = registry
        .ensure_program_task_with_binding(task.clone(), |_| Ok::<(), TaskEnsureError>(()))
        .unwrap();
    assert_eq!(launch.handle(), accepted.handle());
    assert_eq!(launch.handle().correlation.launch_ordinal.get(), 1);
    assert_eq!(launch.source(), spec.source());
    assert!(launch.matches_spec(&task));
    let mut changed = task;
    changed.priority = TaskPriority(changed.priority.0 + 1);
    changed.validate_handle(accepted.handle()).unwrap();
    assert!(
        !launch.matches_spec(&changed),
        "the accepted correlation alone does not authenticate scheduling metadata"
    );
    assert!(matches!(
        source.template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::ProgramTask {
                captures: &[] as &[RuntimeValue]
            },
            None,
            &mut bank(&program, RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimeTaskProducerSourceError::CaptureCount {
            actual: 0,
            expected: 1
        })
    ));
}
