use super::*;
use crate::entry::schema::value_encoding::ValueAdmissionOrigin;
use crate::entry::{RuntimeSchemaError, RuntimeSchemaLimits};
use crate::pattern::RuntimeCheckedType as Checked;
use crate::plan::{RuntimePlanBuilder, RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed};
use crate::value::RuntimeValue;

fn source_plan() -> crate::plan::RuntimePlan {
    let unit = Checked::Unit.semantic_identity_digest();
    let string = Checked::String.semantic_identity_digest();
    let unit_payload = Checked::Tuple(vec![Checked::Unit]).semantic_identity_digest();
    let string_payload = Checked::Tuple(vec![Checked::String]).semantic_identity_digest();
    let result = Checked::Result {
        ok: Box::new(Checked::Unit),
        error: Box::new(Checked::String),
    }
    .semantic_identity_digest();
    let option = Checked::Option(Box::new(Checked::Unit)).semantic_identity_digest();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, Type::Unit),
                RuntimePlanTypeSeed::new(string, Type::String),
                RuntimePlanTypeSeed::new(unit_payload, Type::Tuple(Box::new([unit]))),
                RuntimePlanTypeSeed::new(string_payload, Type::Tuple(Box::new([string]))),
                RuntimePlanTypeSeed::new(
                    result,
                    Type::Result {
                        value: unit,
                        error: string,
                        value_payload: unit_payload,
                        error_payload: string_payload,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    option,
                    Type::Option {
                        item: unit,
                        some_payload: unit_payload,
                    },
                ),
            ],
            [],
        )
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn live_and_inert_context_causes_share_exact_resources_without_a_synthetic_receiver() {
    let plan = source_plan();
    let source = crate::program_types::RuntimeProgramTypes::Plan(&plan)
        .value_type(
            Checked::Result {
                ok: Box::new(Checked::Unit),
                error: Box::new(Checked::String),
            }
            .semantic_identity_digest(),
        )
        .unwrap();
    let live = RuntimeArcErrorContextPending::ResultErr(RuntimeValue::String("cause".into()));
    let saved =
        RuntimeArcErrorContextPending::ResultErr(AwbcRuntimeValueSnapshot::String("cause".into()));
    let exact = RuntimeSchemaLimits {
        max_nodes: 1,
        max_encoded_bytes: 7,
        max_string_bytes: 5,
        max_validation_work: 5,
        ..RuntimeSchemaLimits::engine_default()
    };
    for origin in [
        ValueAdmissionOrigin::Live,
        ValueAdmissionOrigin::InertSnapshot,
    ] {
        let validate = |limits| {
            let mut resources = ValueResourceAdmission::snapshot(limits, origin);
            match origin {
                ValueAdmissionOrigin::Live => {
                    live.validate_source(source, None, RuntimeValue::view, &mut resources)
                }
                ValueAdmissionOrigin::InertSnapshot => saved.validate_source(
                    source,
                    None,
                    AwbcRuntimeValueSnapshot::view,
                    &mut resources,
                ),
            }
        };
        validate(exact).unwrap();
        for short in [
            RuntimeSchemaLimits {
                max_nodes: 0,
                ..exact
            },
            RuntimeSchemaLimits {
                max_encoded_bytes: 6,
                ..exact
            },
            RuntimeSchemaLimits {
                max_string_bytes: 4,
                ..exact
            },
            RuntimeSchemaLimits {
                max_validation_work: 4,
                ..exact
            },
        ] {
            assert!(
                validate(short).is_err(),
                "selected one-over policy: {short:?}"
            );
        }
        let mut resources = ValueResourceAdmission::snapshot(exact, origin);
        live.validate_source(source, None, RuntimeValue::view, &mut resources)
            .unwrap();
        let error = saved
            .validate_source(source, None, AwbcRuntimeValueSnapshot::view, &mut resources)
            .unwrap_err();
        assert!(matches!(error, RuntimeProgramTypeError::Resource(error)
            if matches!(*error, RuntimeSchemaError::ValidationWork { limit: 5, consumed: 6, .. })));
    }
}

#[test]
fn option_absence_admits_zero_value_nodes_and_bytes_but_still_charges_header_work() {
    let plan = source_plan();
    let source = crate::program_types::RuntimeProgramTypes::Plan(&plan)
        .value_type(Checked::Option(Box::new(Checked::Unit)).semantic_identity_digest())
        .unwrap();
    let pending: RuntimeArcErrorContextPending<AwbcRuntimeValueSnapshot> =
        RuntimeArcErrorContextPending::OptionNone;
    let exact = RuntimeSchemaLimits {
        max_nodes: 0,
        max_encoded_bytes: 0,
        max_validation_work: 2,
        ..RuntimeSchemaLimits::engine_default()
    };
    pending
        .validate_source(
            source,
            None,
            AwbcRuntimeValueSnapshot::view,
            &mut ValueResourceAdmission::snapshot(exact, ValueAdmissionOrigin::InertSnapshot),
        )
        .unwrap();
    assert!(
        pending
            .validate_source(
                source,
                None,
                AwbcRuntimeValueSnapshot::view,
                &mut ValueResourceAdmission::snapshot(
                    RuntimeSchemaLimits {
                        max_validation_work: 1,
                        ..exact
                    },
                    ValueAdmissionOrigin::InertSnapshot
                )
            )
            .is_err()
    );
}

#[test]
fn inert_context_codec_round_trips_and_rejects_unknown_or_malformed_cases() {
    for saved in [
        RuntimeArcErrorContextPending::ResultErr(AwbcRuntimeValueSnapshot::String("cause".into())),
        RuntimeArcErrorContextPending::OptionNone,
    ] {
        let bytes = serde_json::to_vec(&saved).unwrap();
        assert_eq!(
            serde_json::from_slice::<RuntimeArcErrorContextPending<AwbcRuntimeValueSnapshot>>(
                &bytes
            )
            .unwrap(),
            saved
        );
    }
    for invalid in [
        r#""Unknown""#,
        r#"{"ResultErr":null}"#,
        r#"{"OptionNone":{"unexpected":true}}"#,
    ] {
        assert!(
            serde_json::from_str::<RuntimeArcErrorContextPending<AwbcRuntimeValueSnapshot>>(
                invalid
            )
            .is_err()
        );
    }
}
