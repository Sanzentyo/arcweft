use super::*;
use crate::plan::body_semantic::request::codec::RuntimeTaskRequestCodecLimits;
use crate::plan::body_semantic::request::*;
use crate::plan::{
    RuntimeExprSeed, RuntimeExprSeedKind, RuntimeHostArgumentSeed, RuntimeTaskPlanSealLimits,
};

fn template(ty: RuntimePlanTypeId, endpoint: u32) -> RuntimeTaskRequestTemplate {
    RuntimeTaskRequestTemplate::at_operation(
        endpoint,
        Box::new([RuntimeRequestArgument {
            role: RuntimeRequestArgumentRole::Positional,
            identity: None,
            ty,
            source: RuntimeRequestValueSource::Literal,
            path: Box::new([RuntimeRequestPathStep::Operand(0)]),
        }]),
        Box::new([]),
    )
}

#[test]
fn decoded_request_definition_recomputes_q_under_the_actual_f_endpoint() {
    let (plan, owner, function) = producer_host_plan_with_arguments(
        true,
        vec![RuntimeHostArgumentSeed::Positional(
            RuntimeRequestRoleIdentity::from_accepted_identity([42; 32]),
            RuntimeExprSeed::new(
                RuntimeSemanticTypeId::from_bytes([61; 32]),
                RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true)),
            ),
        )],
    );
    let context = RuntimeBodySemanticContext::new(&plan);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let producer = context
        .producer_function(
            &mut meter,
            function,
            &owner,
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let endpoint = producer.endpoint(0).unwrap();
    let limits = RuntimeTaskRequestCodecLimits {
        roles: 1,
        path_steps: 1,
        encoded_bytes: 1000,
        decoded_bytes: 16_384,
        type_count: plan.type_table().len(),
    };
    let definition = context
        .static_request_template(
            &mut meter,
            endpoint,
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    assert_known_q_bytes(&context, endpoint, &definition);
    let bytes = definition.encode(limits).unwrap();
    let decoded = RuntimeTaskRequestTemplate::decode(&bytes, limits).unwrap();
    assert_eq!(
        context
            .request_template_digest(
                &mut meter,
                endpoint,
                &definition,
                crate::plan::RuntimeTaskPlanSealLimits::default()
            )
            .unwrap(),
        context
            .request_template_digest(
                &mut meter,
                endpoint,
                &decoded,
                crate::plan::RuntimeTaskPlanSealLimits::default()
            )
            .unwrap()
    );
    let changed = template(
        RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::new(99).unwrap()),
        endpoint.ordinal(),
    );
    assert!(matches!(
        changed.encode(limits),
        Err(
            crate::plan::body_semantic::request::codec::RuntimeTaskRequestCodecError::Limit {
                kind: "type coordinate",
                actual: 99,
                ..
            }
        )
    ));
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        context.request_template_digest(
            &mut meter,
            endpoint,
            &changed,
            crate::plan::RuntimeTaskPlanSealLimits::default()
        ),
        Err(RuntimeBodySemanticError::UnknownType { .. })
    ));
    let changed = template(
        plan.type_table().declarations_with_ids().next().unwrap().0,
        1,
    );
    let decoded =
        RuntimeTaskRequestTemplate::decode(&changed.encode(limits).unwrap(), limits).unwrap();
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        context.request_template_digest(
            &mut meter,
            endpoint,
            &decoded,
            crate::plan::RuntimeTaskPlanSealLimits::default()
        ),
        Err(RuntimeBodySemanticError::InvalidRequestEndpoint {
            expected: RuntimeTaskRequestSource::Operation(0),
            actual: RuntimeTaskRequestSource::Operation(1)
        })
    ));
    assert_eq!(meter.totals(), (0, 0));
}

fn assert_known_q_bytes(
    context: &RuntimeBodySemanticContext<'_>,
    endpoint: crate::plan::body_semantic::function::ProducerEndpoint<'_>,
    definition: &RuntimeTaskRequestTemplate,
) {
    let mut exact_meter = TaskSemanticMeter::new(100_000, 1_000_000);
    context
        .request_template_digest(
            &mut exact_meter,
            endpoint,
            definition,
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    assert_eq!(
        definition.known_transcript_bytes().unwrap(),
        exact_meter.totals().1
    );
}

#[test]
fn all_host_argument_transports_round_trip_and_reissue_only_the_original_q() {
    use crate::plan::{RuntimePlanSequenceKind, RuntimePlanTypeProjection, RuntimePlanTypeSeed};
    let (plan, owner, function) = producer_host_plan_with_setup(true, |builder| {
        let boolean = RuntimeSemanticTypeId::from_bytes([61; 32]);
        let sequence = RuntimeSemanticTypeId::from_bytes([64; 32]);
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    sequence,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: boolean,
                    },
                )],
                [],
            )
            .unwrap();
        let scalar = RuntimeExprSeed::new(
            boolean,
            RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true)),
        );
        let role = |byte| RuntimeRequestRoleIdentity::from_accepted_identity([byte; 32]);
        (
            vec![
                RuntimeHostArgumentSeed::Positional(role(42), scalar.clone()),
                RuntimeHostArgumentSeed::Named(
                    role(43),
                    crate::task::NamedHostArg {
                        name: "named".to_owned(),
                        value: scalar.clone(),
                    },
                ),
                RuntimeHostArgumentSeed::Spread(
                    role(44),
                    RuntimeExprSeed::new(
                        sequence,
                        RuntimeExprSeedKind::BracketSeq(Box::new([scalar])),
                    ),
                ),
            ],
            Box::new([]),
        )
    });
    let context = RuntimeBodySemanticContext::new(&plan);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let producer = context
        .producer_function(
            &mut meter,
            function,
            &owner,
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let endpoint = producer.endpoint(0).unwrap();
    let definition = context
        .static_request_template(&mut meter, endpoint, RuntimeTaskPlanSealLimits::default())
        .unwrap();
    let limits = RuntimeTaskRequestCodecLimits {
        roles: 3,
        path_steps: 5,
        encoded_bytes: 1000,
        decoded_bytes: 16_384,
        type_count: plan.type_table().len(),
    };
    let decoded =
        RuntimeTaskRequestTemplate::decode(&definition.encode(limits).unwrap(), limits).unwrap();
    assert_eq!(definition, decoded);
    assert_eq!(
        context
            .static_request_template_digest(
                &mut meter,
                endpoint,
                RuntimeTaskPlanSealLimits::default()
            )
            .unwrap(),
        context
            .request_template_digest(
                &mut meter,
                endpoint,
                &decoded,
                RuntimeTaskPlanSealLimits::default()
            )
            .unwrap()
    );
    assert_known_q_bytes(&context, endpoint, &decoded);
}

#[test]
fn decoded_request_cannot_forge_any_original_argument_or_field_component() {
    let (plan, owner, function) = producer_host_plan_with_arguments(
        true,
        vec![RuntimeHostArgumentSeed::Positional(
            RuntimeRequestRoleIdentity::from_accepted_identity([42; 32]),
            RuntimeExprSeed::new(
                RuntimeSemanticTypeId::from_bytes([61; 32]),
                RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true)),
            ),
        )],
    );
    let context = RuntimeBodySemanticContext::new(&plan);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let producer = context
        .producer_function(
            &mut meter,
            function,
            &owner,
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let endpoint = producer.endpoint(0).unwrap();
    let other_type = plan
        .type_table()
        .declarations_with_ids()
        .find(|(_, row)| {
            matches!(
                row.projection(),
                crate::plan::RuntimePlanTypeProjection::Unit
            )
        })
        .unwrap()
        .0;
    let limits = RuntimeTaskRequestCodecLimits {
        roles: 2,
        path_steps: 3,
        encoded_bytes: 1000,
        decoded_bytes: 16_384,
        type_count: plan.type_table().len(),
    };
    for mutation in 0..9 {
        let mut arguments = vec![RuntimeRequestArgument {
            role: RuntimeRequestArgumentRole::Positional,
            identity: None,
            ty: endpoint.host_arguments().unwrap()[0].value().ty(),
            source: RuntimeRequestValueSource::Literal,
            path: Box::new([RuntimeRequestPathStep::Operand(0)]),
        }];
        let mut fields = Vec::new();
        match mutation {
            0 => arguments[0].role = RuntimeRequestArgumentRole::Named,
            1 => {
                arguments[0].identity =
                    Some(RuntimeRequestRoleIdentity::from_accepted_identity([99; 32]));
            }
            2 => arguments[0].ty = other_type,
            3 => arguments[0].source = RuntimeRequestValueSource::NeedHandle,
            4 => arguments[0].path = Box::new([]),
            5 => arguments[0].path = Box::new([RuntimeRequestPathStep::Operand(1)]),
            6 => arguments[0].path = Box::new([RuntimeRequestPathStep::Capture(0)]),
            7 => arguments.clear(),
            8 => {
                fields.push(RuntimeRequestField {
                    identity: RuntimeRequestRoleIdentity::from_accepted_identity([99; 32]),
                    role: RuntimeRequestFieldRole::Required,
                    ty: other_type,
                    path: Box::new([]),
                });
            }
            _ => unreachable!(),
        }
        let definition = RuntimeTaskRequestTemplate::at_operation(
            endpoint.ordinal(),
            arguments.into_boxed_slice(),
            fields.into_boxed_slice(),
        );
        let inert = RuntimeTaskRequestTemplate::decode(&definition.encode(limits).unwrap(), limits)
            .unwrap();
        let mut admission = TaskSemanticMeter::new(100_000, 1_000_000);
        let rejected = context.request_template_digest(
            &mut admission,
            endpoint,
            &inert,
            RuntimeTaskPlanSealLimits::default(),
        );
        assert!(
            matches!(
                rejected,
                Err(RuntimeBodySemanticError::RequestTemplateMismatch { endpoint: 0, .. })
            ),
            "mutation {mutation}: {rejected:?}"
        );
        assert_eq!(admission.totals().1, 0, "unadmitted input emits no Q bytes");
        assert_eq!(
            TaskSemanticEncoder::new(b"later", &mut admission).finish(),
            Err(TaskSemanticEncodingError::OwnerRejected)
        );
    }
}

#[test]
fn decoded_request_has_exact_shared_work_and_bytes_without_refreshing_allowance() {
    let (plan, owner, function) = producer_host_plan_with_arguments(
        true,
        vec![RuntimeHostArgumentSeed::Positional(
            RuntimeRequestRoleIdentity::from_accepted_identity([42; 32]),
            RuntimeExprSeed::new(
                RuntimeSemanticTypeId::from_bytes([61; 32]),
                RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true)),
            ),
        )],
    );
    let context = RuntimeBodySemanticContext::new(&plan);
    let mut preparation = TaskSemanticMeter::new(100_000, 1_000_000);
    let producer = context
        .producer_function(
            &mut preparation,
            function,
            &owner,
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let endpoint = producer.endpoint(0).unwrap();
    let definition = context
        .static_request_template(
            &mut preparation,
            endpoint,
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let mut generous = TaskSemanticMeter::new(100_000, 1_000_000);
    let digest = context
        .request_template_digest(
            &mut generous,
            endpoint,
            &definition,
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let (work, bytes) = generous.totals();
    let mut exact = TaskSemanticMeter::new(work, bytes);
    assert_eq!(
        context
            .request_template_digest(
                &mut exact,
                endpoint,
                &definition,
                RuntimeTaskPlanSealLimits::default()
            )
            .unwrap(),
        digest
    );
    for (work, bytes, expected) in [
        (work - 1, bytes, TaskSemanticEncodingError::SemanticWork),
        (work, bytes - 1, TaskSemanticEncodingError::TranscriptBytes),
    ] {
        let mut limited = TaskSemanticMeter::new(work, bytes);
        assert!(
            matches!(context.request_template_digest(&mut limited, endpoint, &definition, RuntimeTaskPlanSealLimits::default()), Err(RuntimeBodySemanticError::Encoding(actual)) if actual == expected)
        );
        assert_eq!(
            TaskSemanticEncoder::new(b"later", &mut limited).finish(),
            Err(expected)
        );
    }
}
