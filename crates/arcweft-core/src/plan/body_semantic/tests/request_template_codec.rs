use super::*;
use crate::plan::body_semantic::request::codec::RuntimeTaskRequestCodecLimits;
use crate::plan::body_semantic::request::*;
use crate::plan::{RuntimeExprSeed, RuntimeExprSeedKind, RuntimeHostArgumentSeed};

fn template(ty: RuntimePlanTypeId, endpoint: u32) -> RuntimeTaskRequestTemplate {
    RuntimeTaskRequestTemplate::new(
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
            &mut |_| Ok(owner.resolve(0).unwrap()),
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let endpoint = producer.endpoint(0).unwrap();
    let limits = RuntimeTaskRequestCodecLimits {
        roles: 1,
        path_steps: 1,
        encoded_bytes: 1000,
    };
    let definition = context
        .host_request_template(
            &mut meter,
            endpoint,
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
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
            expected: 0,
            actual: 1
        })
    ));
    assert_eq!(meter.totals(), (0, 0));
}
