use super::*;
use crate::presentation::RuntimePresentationFactoryCall;
use crate::{
    awbc::{
        codec::AwbcDecodeBudget,
        schema::{AwbcProgram, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcTypeId},
        verify::{AwbcVerifyBudget, AwbcVerifyContext},
    },
    entry::RuntimeSchemaLimits,
    plan::{
        RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
        RuntimeTaskSourceArtifactLimits,
    },
    task::RuntimeProgramOwner,
    value::{
        AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission, RuntimeStoredValue,
        ownership::RuntimeValueOwnership,
    },
};
use arcweft_interaction_model::presentation::{
    PresentationArgumentKey, PresentationArgumentRole, PresentationCallableId,
};

const BOOL: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([81; 32]);
const BOOL_ALIAS: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([84; 32]);
const RESOURCE: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([83; 32]);

fn keys() -> Box<[PresentationArgumentKey]> {
    Box::new([PresentationArgumentKey::try_new(
        PresentationArgumentRole::Parameter,
        Some("channel".into()),
    )
    .unwrap()])
}

fn body_type() -> RuntimeSemanticTypeId {
    RuntimePresentationFactoryCall::retained_body_identity(
        PresentationCallableId::Image,
        &keys(),
        RESOURCE,
        &crate::plan::RuntimeTypeScope::root(),
        1,
        |_| Some(BOOL),
    )
    .unwrap()
}

fn sources() -> [RuntimeTaskSourceProgram; 2] {
    sources_for(PresentationResourceKind::Image, body_type())
}

fn sources_for(
    resource: PresentationResourceKind,
    body_type: RuntimeSemanticTypeId,
) -> [RuntimeTaskSourceProgram; 2] {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(BOOL, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(BOOL_ALIAS, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    body_type,
                    RuntimePlanTypeProjection::Tuple(Box::new([BOOL])),
                ),
                RuntimePlanTypeSeed::new(
                    RESOURCE,
                    RuntimePlanTypeProjection::PresentationResource(resource),
                ),
            ],
            std::iter::empty::<crate::plan::RuntimeLocalDeclarationSeed>(),
        )
        .unwrap();
    let plan = Arc::new(builder.finish().unwrap());
    let original_source = plan
        .task_source_artifact(RuntimeTaskSourceArtifactLimits::default())
        .unwrap();
    // Decode the actual wire representation before selecting its execution lease.
    let program = AwbcProgram {
        runtime_types: vec![
            AwbcRuntimeType::new(BOOL, AwbcRuntimeTypeShape::Bool),
            AwbcRuntimeType::new(body_type, AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0)])),
            AwbcRuntimeType::new(
                RESOURCE,
                AwbcRuntimeTypeShape::PresentationResource(resource),
            ),
            AwbcRuntimeType::new(BOOL_ALIAS, AwbcRuntimeTypeShape::Bool),
        ],
        original_source: Some(original_source),
        ..AwbcProgram::default()
    };
    let program = AwbcProgram::decode_canonical(
        &program.encode_canonical().unwrap(),
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    let awbc = RuntimeTaskSourceProgram::admit_awbc(
        Arc::new(program),
        AwbcVerifyBudget::default(),
        AwbcVerifyContext {
            require_entrypoint: false,
            ..AwbcVerifyContext::default()
        },
        RuntimeTaskSourceArtifactLimits::default(),
        None,
    )
    .unwrap();
    [RuntimeTaskSourceProgram::native(plan), awbc]
}

fn layout() -> Arc<RuntimePresentationFactoryCall> {
    Arc::new(
        RuntimePresentationFactoryCall::try_new(
            PresentationCallableId::Image,
            keys(),
            RESOURCE,
            body_type(),
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap(),
    )
}

fn view_layout() -> Arc<crate::presentation::RuntimePresentationViewCall> {
    use crate::presentation::{RuntimePresentationViewCall, RuntimePresentationViewIdentity};
    let view = arcweft_id::view::ViewId::try_new("view.card").unwrap();
    let definition = RuntimePresentationViewIdentity::from_checked_declaration([77; 32]).unwrap();
    let body = RuntimePresentationViewCall::retained_body_identity(
        &view,
        definition,
        3,
        &[2],
        RESOURCE,
        &crate::plan::RuntimeTypeScope::root(),
        1,
        |_| Some(BOOL),
    )
    .unwrap();
    Arc::new(
        RuntimePresentationViewCall::try_new(
            view,
            definition,
            3,
            Box::new([2]),
            RESOURCE,
            body,
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap(),
    )
}

#[test]
fn unmounted_view_retains_supplied_capture_and_omission_across_native_and_decoded_awbc_save() {
    let call = view_layout();
    for source in sources_for(PresentationResourceKind::View, call.body_type()) {
        let owner = source.program_owner();
        let value = RuntimeRetainedValueAdmission::for_task_source(
            &source,
            RuntimeSchemaLimits::engine_default(),
        )
        .construct_presentation_resource(
            source.clone(),
            crate::presentation::RuntimePresentationResourceLayout::View(Arc::clone(&call)),
            None,
            RuntimePayload(RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)])),
        )
        .unwrap();
        assert_eq!(value.ownership(), RuntimeValueOwnership::Affine);
        assert!(value.live_view().copy_unrestricted().is_none());
        let snapshot =
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &owner).unwrap();
        let decoded: AwbcRuntimeValueSnapshot =
            serde_json::from_slice(&serde_json::to_vec(&snapshot).unwrap()).unwrap();
        let mut admission = RuntimeRetainedValueAdmission::for_task_source(
            &source,
            RuntimeSchemaLimits::engine_default(),
        );
        admission
            .validate_owned_borrowed(decoded.borrowed_value())
            .unwrap();
        let context = crate::value::RuntimeValueRestoreContext::new(
            &owner,
            Some(crate::task::TaskSourceRestore {
                program: &source,
                admission: &admission,
            }),
        );
        let restored = decoded.into_admitted_runtime_value(context).unwrap();
        assert_eq!(restored, value);
        let RuntimeValue::PresentationResource(resource) = restored else {
            panic!("retained View")
        };
        let view = resource.layout().view().unwrap();
        assert_eq!(view.parameters(), 3);
        assert_eq!(view.supplied(), &[2]);
        assert_eq!(
            resource.body(),
            &RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)])
        );
        assert!(resource.source.program_owner().same_program(&owner));
    }
}

#[test]
fn unmounted_view_rejects_changed_definition_destinations_and_resource_under_original_type_graph() {
    use crate::presentation::{
        RuntimePresentationResourceLayout, RuntimePresentationViewCall,
        RuntimePresentationViewIdentity,
    };
    let call = view_layout();
    for source in sources_for(PresentationResourceKind::View, call.body_type()) {
        let owner = source.program_owner();
        let value = RuntimeRetainedValueAdmission::for_task_source(
            &source,
            RuntimeSchemaLimits::engine_default(),
        )
        .construct_presentation_resource(
            source.clone(),
            RuntimePresentationResourceLayout::View(Arc::clone(&call)),
            None,
            RuntimePayload(RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)])),
        )
        .unwrap();
        let snapshot =
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &owner).unwrap();
        for mutation in 0..6 {
            let mut forged = snapshot.clone();
            let AwbcRuntimeValueSnapshot::PresentationResource(resource) = &mut forged else {
                panic!("snapshot View")
            };
            resource.layout = RuntimePresentationResourceLayout::View(Arc::new(
                RuntimePresentationViewCall::try_new(
                    if mutation == 0 {
                        arcweft_id::view::ViewId::try_new("view.other").unwrap()
                    } else {
                        call.view().clone()
                    },
                    if mutation == 1 {
                        RuntimePresentationViewIdentity::from_checked_declaration([78; 32]).unwrap()
                    } else {
                        call.definition()
                    },
                    if mutation == 2 { 4 } else { 3 },
                    if mutation == 3 {
                        Box::new([1])
                    } else {
                        Box::new([2])
                    },
                    if mutation == 4 { BOOL } else { RESOURCE },
                    call.body_type(),
                    RuntimeSchemaLimits::engine_default(),
                )
                .unwrap(),
            ));
            if mutation == 5 {
                resource.body = Box::new(AwbcRuntimeValueSnapshot::Tuple(vec![
                    AwbcRuntimeValueSnapshot::Bool(false),
                    AwbcRuntimeValueSnapshot::Bool(true),
                ]));
            }
            let mut admission = RuntimeRetainedValueAdmission::for_task_source(
                &source,
                RuntimeSchemaLimits::engine_default(),
            );
            assert!(
                admission
                    .validate_owned_borrowed(forged.borrowed_value())
                    .is_err(),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn presentation_factory_input_abi_rejects_a_distinct_identity_with_the_same_physical_shape() {
    for source in sources() {
        let owner = source.program_owner();
        let call = layout();
        call.validate_input_types(owner.types(), 1, |_| Some(BOOL))
            .unwrap();
        let error = call
            .validate_input_types(owner.types(), 1, |_| Some(BOOL_ALIAS))
            .unwrap_err();
        assert!(matches!(
            error,
            crate::presentation::RuntimePresentationFactoryCallError::InputType {
                index: 0,
                expected: BOOL,
                actual: Some(BOOL_ALIAS)
            }
        ));
        assert!(matches!(
            call.validate_input_types(owner.types(), 0, |_| None)
                .unwrap_err(),
            crate::presentation::RuntimePresentationFactoryCallError::Arguments {
                expected: 1,
                actual: 0
            }
        ));
        assert!(matches!(
            call.validate_input_types(owner.types(), 1, |_| None)
                .unwrap_err(),
            crate::presentation::RuntimePresentationFactoryCallError::InputType {
                index: 0,
                expected: BOOL,
                actual: None
            }
        ));
    }
}

fn construct(
    source: &RuntimeTaskSourceProgram,
    body: RuntimeValue,
) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
    RuntimeRetainedValueAdmission::for_task_source(source, RuntimeSchemaLimits::engine_default())
        .construct_presentation_resource(
            source.clone(),
            crate::presentation::RuntimePresentationResourceLayout::Factory(layout()),
            None,
            RuntimePayload(body),
        )
}

#[test]
fn presentation_resource_is_affine_and_roundtrips_under_its_original_native_and_decoded_awbc_source()
 {
    for source in sources() {
        let owner = source.program_owner();
        let body = RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)]);
        let value = construct(&source, body.clone()).unwrap();
        assert_eq!(value.ownership(), RuntimeValueOwnership::Affine);
        assert!(value.live_view().copy_unrestricted().is_none());
        assert!(serde_json::to_vec(&value).is_err());
        assert!(matches!(
            AwbcRuntimeValueSnapshot::from_unrestricted_runtime_value_for_program(&value, &owner),
            Err(AwbcRuntimeValueSnapshotError::AffineValue)
        ));
        let snapshot =
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &owner).unwrap();
        let wire = serde_json::to_vec(&snapshot).unwrap();
        let decoded: AwbcRuntimeValueSnapshot = serde_json::from_slice(&wire).unwrap();
        let mut admission = RuntimeRetainedValueAdmission::for_task_source(
            &source,
            RuntimeSchemaLimits::engine_default(),
        );
        admission
            .validate_owned_borrowed(decoded.borrowed_value())
            .unwrap();
        let context = crate::value::RuntimeValueRestoreContext::new(
            &owner,
            Some(crate::task::TaskSourceRestore {
                program: &source,
                admission: &admission,
            }),
        );
        let restored = decoded.into_admitted_runtime_value(context).unwrap();
        assert_eq!(restored, value);
        let RuntimeValue::PresentationResource(resource) = restored else {
            panic!("resource carrier")
        };
        assert_eq!(resource.body(), &body);
        assert!(resource.source.program_owner().same_program(&owner));
        assert_eq!(
            resource.layout().factory().unwrap().inputs()[0].parameter(),
            Some("channel")
        );
    }
}

#[test]
fn presentation_resource_rejects_foreign_lease_and_forged_source_body_and_resource_type() {
    for source in sources() {
        let owner = source.program_owner();
        let value =
            construct(&source, RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)])).unwrap();
        let foreign = match &owner {
            RuntimeProgramOwner::Plan(plan) => {
                RuntimeProgramOwner::Plan(Arc::new(plan.as_ref().clone()))
            }
            RuntimeProgramOwner::Awbc(program) => {
                RuntimeProgramOwner::Awbc(Arc::new(program.as_ref().clone()))
            }
        };
        assert!(
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &foreign).is_err()
        );
        let snapshot =
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &owner).unwrap();
        for mutation in 0..4 {
            let mut forged = snapshot.clone();
            let AwbcRuntimeValueSnapshot::PresentationResource(resource) = &mut forged else {
                panic!("snapshot carrier")
            };
            match mutation {
                0 => resource.original_executable[0] ^= 1,
                1 => {
                    resource.body = Box::new(AwbcRuntimeValueSnapshot::Tuple(vec![
                        AwbcRuntimeValueSnapshot::String("wrong type".into()),
                    ]))
                }
                2 => {
                    resource.layout =
                        crate::presentation::RuntimePresentationResourceLayout::Factory(Arc::new(
                            RuntimePresentationFactoryCall::try_new(
                                PresentationCallableId::Image,
                                layout().inputs().into(),
                                BOOL,
                                body_type(),
                                RuntimeSchemaLimits::engine_default(),
                            )
                            .unwrap(),
                        ))
                }
                3 => {
                    let changed = Box::new([PresentationArgumentKey::try_new(
                        PresentationArgumentRole::Parameter,
                        Some("other".into()),
                    )
                    .unwrap()]);
                    resource.layout =
                        crate::presentation::RuntimePresentationResourceLayout::Factory(Arc::new(
                            RuntimePresentationFactoryCall::try_new(
                                PresentationCallableId::Image,
                                changed,
                                RESOURCE,
                                body_type(),
                                RuntimeSchemaLimits::engine_default(),
                            )
                            .unwrap(),
                        ));
                }
                _ => unreachable!(),
            }
            let mut admission = RuntimeRetainedValueAdmission::for_task_source(
                &source,
                RuntimeSchemaLimits::engine_default(),
            );
            let error = admission
                .validate_owned_borrowed(forged.borrowed_value())
                .unwrap_err();
            match mutation {
                0 => assert!(
                    matches!(
                        error,
                        AwbcRuntimeValueSnapshotError::ProgramOwnerMismatch { .. }
                    ),
                    "{error}"
                ),
                1 => assert!(
                    matches!(error, AwbcRuntimeValueSnapshotError::ProgramType(_)),
                    "{error}"
                ),
                2 => assert!(
                    error.to_string().contains("requires resource type"),
                    "{error}"
                ),
                3 => assert!(
                    error.to_string().contains("disagrees with its layout"),
                    "{error}"
                ),
                _ => unreachable!(),
            }
        }
        assert!(
            construct(
                &source,
                RuntimeValue::Tuple(vec![RuntimeValue::String("wrong".into())])
            )
            .is_err()
        );
    }
}

#[test]
fn presentation_resource_preflights_complete_capture_and_keeps_resource_failure_sticky() {
    for source in sources() {
        for limits in [
            RuntimeSchemaLimits {
                max_nodes: 2,
                ..RuntimeSchemaLimits::engine_default()
            },
            RuntimeSchemaLimits {
                max_string_bytes: 3,
                ..RuntimeSchemaLimits::engine_default()
            },
        ] {
            let mut admission = RuntimeRetainedValueAdmission::for_task_source(&source, limits);
            let error = admission
                .construct_presentation_resource(
                    source.clone(),
                    crate::presentation::RuntimePresentationResourceLayout::Factory(layout()),
                    None,
                    RuntimePayload(RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)])),
                )
                .unwrap_err();
            assert!(matches!(error, AwbcRuntimeValueSnapshotError::Resource(_)));
            assert_eq!(admission.validate_metadata(&0_u8), Err(error));
        }
    }
}

#[test]
fn presentation_resource_rejects_an_awbc_capture_type_that_disagrees_with_original_source() {
    let [_, source] = sources();
    let RuntimeProgramOwner::Awbc(program) = source.program_owner() else {
        unreachable!()
    };
    let mut program = program.as_ref().clone();
    program.runtime_types[0] = AwbcRuntimeType::new(BOOL, AwbcRuntimeTypeShape::String);
    let source = RuntimeTaskSourceProgram::admit_awbc(
        Arc::new(program),
        AwbcVerifyBudget::default(),
        AwbcVerifyContext {
            require_entrypoint: false,
            ..AwbcVerifyContext::default()
        },
        RuntimeTaskSourceArtifactLimits::default(),
        None,
    )
    .unwrap();
    let error = construct(
        &source,
        RuntimeValue::Tuple(vec![RuntimeValue::String(
            "executable accepts this, original does not".into(),
        )]),
    )
    .unwrap_err();
    assert!(error.to_string().contains("Shape"), "{error}");
}

#[test]
fn presentation_resource_types_keep_all_families_distinct_in_original_source_and_awbc_codecs() {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            PresentationResourceKind::ALL.into_iter().map(|kind| {
                RuntimePlanTypeSeed::new(
                    RuntimeSemanticTypeId::from_bytes([100 + kind.semantic_tag(); 32]),
                    RuntimePlanTypeProjection::PresentationResource(kind),
                )
            }),
            std::iter::empty::<crate::plan::RuntimeLocalDeclarationSeed>(),
        )
        .unwrap();
    let plan = builder.finish().unwrap();
    let source = plan
        .task_source_artifact(RuntimeTaskSourceArtifactLimits::default())
        .unwrap();
    let restored = source
        .admit(RuntimeTaskSourceArtifactLimits::default(), None)
        .unwrap();
    for kind in PresentationResourceKind::ALL {
        let semantic = RuntimeSemanticTypeId::from_bytes([100 + kind.semantic_tag(); 32]);
        let ty = restored.type_table().id_for_semantic(semantic).unwrap();
        assert_eq!(
            restored.type_table().get(ty).unwrap().projection(),
            &RuntimePlanTypeProjection::PresentationResource(kind)
        );
    }
    let program = AwbcProgram {
        runtime_types: PresentationResourceKind::ALL
            .into_iter()
            .map(|kind| {
                AwbcRuntimeType::new(
                    RuntimeSemanticTypeId::from_bytes([100 + kind.semantic_tag(); 32]),
                    AwbcRuntimeTypeShape::PresentationResource(kind),
                )
            })
            .collect(),
        original_source: Some(source),
        ..AwbcProgram::default()
    };
    let bytes = program.encode_canonical().unwrap();
    let restored = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(restored.runtime_types, program.runtime_types);
    assert_eq!(restored.encode_canonical().unwrap(), bytes);
}
