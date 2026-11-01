use crate::{
    entry::RuntimeSchemaLimits,
    pattern::{RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId},
    plan::{RuntimePlanBuilder, RuntimePlanTypeProjection},
    presentation::RuntimePresentationFactoryCall,
};
use arcweft_interaction_model::presentation::{
    PresentationArgumentKey, PresentationArgumentRole, PresentationCallableId,
    PresentationResourceKind,
};
use std::sync::Arc;
pub(crate) const BOOL: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([91; 32]);
pub(crate) const ALIAS: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([92; 32]);
pub(crate) const RESOURCE: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([93; 32]);
pub(crate) const HANDLE: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([94; 32]);

pub(crate) fn fixture() -> (RuntimePlanBuilder, Arc<RuntimePresentationFactoryCall>) {
    let inputs: Box<[_]> = Box::new([PresentationArgumentKey::try_new(
        PresentationArgumentRole::Parameter,
        Some("channel".into()),
    )
    .unwrap()]);
    let body = RuntimePresentationFactoryCall::retained_body_identity(
        PresentationCallableId::Image,
        &inputs,
        RESOURCE,
        &crate::plan::RuntimeTypeScope::root(),
        1,
        |_| Some(BOOL),
    )
    .unwrap();
    let owner =
        RuntimeOpaqueTypeOwner::presentation_handle(PresentationResourceKind::Image, HANDLE);
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                crate::plan::RuntimePlanTypeSeed::new(BOOL, RuntimePlanTypeProjection::Bool),
                crate::plan::RuntimePlanTypeSeed::new(ALIAS, RuntimePlanTypeProjection::Bool),
                crate::plan::RuntimePlanTypeSeed::new(
                    RESOURCE,
                    RuntimePlanTypeProjection::PresentationResource(
                        PresentationResourceKind::Image,
                    ),
                ),
                crate::plan::RuntimePlanTypeSeed::new(
                    body,
                    RuntimePlanTypeProjection::Tuple(Box::new([BOOL])),
                ),
                crate::plan::RuntimePlanTypeSeed::new(
                    HANDLE,
                    RuntimePlanTypeProjection::Opaque {
                        producer: owner.producer().clone(),
                        admission: owner.admission(),
                        value_class: owner.value_class(),
                        persistence: owner.persistence(),
                        arguments: Box::new([]),
                    },
                ),
            ],
            [],
        )
        .unwrap();
    let call = Arc::new(
        RuntimePresentationFactoryCall::try_new(
            PresentationCallableId::Image,
            inputs,
            RESOURCE,
            body,
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap(),
    );
    (builder, call)
}
