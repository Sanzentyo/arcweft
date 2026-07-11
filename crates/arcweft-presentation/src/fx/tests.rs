use std::collections::BTreeSet;

use serde::Deserialize;

use super::{
    Angle, FX_GOLDEN_ANGLE_RAD, FiniteF32, FxCapabilitySet, FxDefinition, FxDefinitionError,
    FxDiagnosticCode, FxDiagnosticContext, FxEvaluationBudget, FxEvaluationError, FxGraph,
    FxGraphChildPath, FxId, FxInstanceId, FxInstanceSnapshot, FxLogicalTime, FxNode, FxNodeKind,
    FxParameter, FxParameterSlot, FxPhase, FxProviderError, FxProviderLimits, FxProviderOutput,
    FxRendererInterface, FxRuntimeType, FxRuntimeValue, FxSampleContext, FxSamplerProgram,
    FxSemanticHash, FxStaticType, FxStaticValue, FxTarget, Length, ResolvedFxOperation,
    ResolvedFxPlan, ResolvedTransform2D, Seconds, Transform2D, ValueInstruction,
    ValueProgramInputs, ValueProgramSchema, ValueProgramValidationError, derive_deterministic_seed,
};

// Keep local constructors terse so tests emphasize the typed contracts.
fn finite(value: f32) -> FiniteF32 {
    FiniteF32::try_new(value).expect("finite fixture")
}

fn length(value: f32) -> Length {
    Length::try_pixels(value).expect("finite length fixture")
}

fn angle(value: f32) -> Angle {
    Angle::try_radians(value).expect("finite angle fixture")
}

fn seconds(value: f32) -> Seconds {
    Seconds::try_seconds(value).expect("finite seconds fixture")
}

fn sampler(return_type: FxRuntimeType, instructions: Vec<ValueInstruction>) -> FxSamplerProgram {
    FxSamplerProgram::validate(
        ValueProgramSchema::new(Vec::new(), Vec::new(), return_type),
        instructions,
    )
    .expect("valid sampler fixture")
}

fn context(time: f32, ordinal: u32) -> FxSampleContext {
    FxSampleContext::from_elapsed(seconds(time), ordinal, 0x55aa, false)
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 0.000_01,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn finite_values_reject_nonfinite_and_canonicalize_negative_zero() {
    let negative_zero = FiniteF32::try_new(-0.0).expect("negative zero is finite");
    assert_eq!(negative_zero, FiniteF32::ZERO);
    assert_eq!(negative_zero.to_bits(), 0);
    assert!(FiniteF32::try_new(f32::INFINITY).is_err());
    assert!(FiniteF32::try_new(f32::NEG_INFINITY).is_err());
    assert!(FiniteF32::try_new(f32::NAN).is_err());

    let deserializer = serde::de::value::F32Deserializer::<serde::de::value::Error>::new(f32::NAN);
    assert!(FiniteF32::deserialize(deserializer).is_err());
    assert!(serde_json::from_str::<FiniteF32>("1e100").is_err());
    assert!(serde_json::from_str::<FiniteF32>("1e-100").is_err());
}

#[test]
fn units_convert_once_to_canonical_runtime_values() {
    assert_close(
        Angle::try_degrees(180.0).expect("degrees").radians(),
        std::f32::consts::PI,
    );
    assert_close(
        Angle::try_turns(0.5).expect("turns").radians(),
        std::f32::consts::PI,
    );
    assert_close(
        Seconds::try_milliseconds(125.0)
            .expect("milliseconds")
            .seconds(),
        0.125,
    );
}

#[test]
fn transform_uses_documented_origin_scale_rotation_translation_order() {
    let transform = Transform2D {
        translate_x: length(5.0),
        translate_y: length(7.0),
        scale_x: finite(2.0),
        scale_y: finite(3.0),
        skew_x: Angle::ZERO,
        skew_y: Angle::ZERO,
        rotation: angle(std::f32::consts::FRAC_PI_2),
        origin_x: length(1.0),
        origin_y: length(1.0),
        opacity: FiniteF32::ONE,
    };
    let point = transform
        .resolve()
        .expect("transform resolves")
        .apply_point(length(2.0), length(1.0))
        .expect("point remains finite");
    assert_close(point[0].pixels(), 6.0);
    assert_close(point[1].pixels(), 10.0);
}

#[test]
fn authored_transform_stack_applies_each_next_transform_after_previous() {
    let translate = Transform2D {
        translate_x: length(10.0),
        ..Transform2D::default()
    };
    let scale = Transform2D {
        scale_x: finite(2.0),
        scale_y: finite(2.0),
        ..Transform2D::default()
    };
    let resolved = ResolvedTransform2D::compose_authored([translate, scale])
        .expect("authored transforms compose");
    let point = resolved
        .apply_point(length(1.0), Length::ZERO)
        .expect("point remains finite");
    assert_close(point[0].pixels(), 22.0);
}

#[test]
fn transform_deserialization_rejects_invalid_opacity() {
    let json = r#"{
        "translate_x": 0.0, "translate_y": 0.0,
        "scale_x": 1.0, "scale_y": 1.0,
        "skew_x": 0.0, "skew_y": 0.0, "rotation": 0.0,
        "origin_x": 0.0, "origin_y": 0.0, "opacity": 1.01
    }"#;
    assert!(serde_json::from_str::<Transform2D>(json).is_err());
}

#[test]
fn program_validation_rejects_mixed_units_before_execution() {
    let error = FxSamplerProgram::validate(
        ValueProgramSchema::new(Vec::new(), Vec::new(), FxRuntimeType::Length),
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::Length(length(1.0)),
            },
            ValueInstruction::Constant {
                value: FxRuntimeValue::Angle(angle(1.0)),
            },
            ValueInstruction::Add,
            ValueInstruction::Return,
        ],
    )
    .expect_err("mixed units are invalid");
    assert!(matches!(
        error,
        ValueProgramValidationError::InvalidOperands {
            operation: "add",
            ..
        }
    ));
}

#[test]
fn sampler_deserialization_revalidates_stack_and_return_contract() {
    let json = r#"{
        "program": {
            "schema": {"parameter_types": [], "state_types": [], "return_type": "f32"},
            "instructions": [{"op": "return"}]
        }
    }"#;
    assert!(serde_json::from_str::<FxSamplerProgram>(json).is_err());
}

#[test]
fn validated_sampler_round_trips_through_serde() {
    let program = sampler(
        FxRuntimeType::F32,
        vec![
            ValueInstruction::LoadContext {
                slot: super::FxContextSlot::Time,
            },
            ValueInstruction::Return,
        ],
    );
    let bytes = serde_json::to_vec(&program).expect("sampler serializes");
    assert_eq!(
        serde_json::from_slice::<FxSamplerProgram>(&bytes).expect("sampler revalidates"),
        program
    );
}

#[test]
fn definition_round_trip_revalidates_typed_hashes() {
    let graph = FxGraph::try_new(vec![FxNode::Style {
        properties: vec![super::FxProperty::new(
            "opacity",
            FxRuntimeValue::F32(finite(0.75)).into(),
        )],
    }])
    .expect("valid graph");
    let definition = FxDefinition::new(
        FxId::try_new("game", "ui.effects.fade").expect("Fx ID"),
        vec![
            FxParameter::try_new(
                "strength",
                FxRuntimeType::F32,
                Some(FxRuntimeValue::F32(finite(0.75))),
            )
            .expect("parameter"),
        ],
        graph,
    )
    .expect("definition");
    let bytes = serde_json::to_vec(&definition).expect("definition serializes");
    assert_eq!(
        serde_json::from_slice::<FxDefinition>(&bytes).expect("hashes revalidate"),
        definition
    );
}

#[test]
fn constructor_property_expectations_are_the_validator_source_of_truth() {
    assert_eq!(
        FxNodeKind::Transform.property_type("target"),
        Some(FxStaticType::Target)
    );
    assert_eq!(
        FxNodeKind::Transform.property_type("phase"),
        Some(FxStaticType::Phase)
    );
    assert_eq!(
        FxNodeKind::Transform.property_type("sampler"),
        Some(FxStaticType::Runtime(FxRuntimeType::Transform2D))
    );
    assert_eq!(FxNodeKind::Transform.property_type("amplitude"), None);
    assert!(
        FxStaticType::Runtime(FxRuntimeType::F32).accepts(&FxStaticValue::Parameter(
            FxParameterSlot {
                index: 0,
                ty: FxRuntimeType::F32,
            }
        ))
    );
}

#[test]
fn definition_validation_rejects_out_of_bounds_parameter_slots() {
    let graph = FxGraph::try_new(vec![FxNode::Style {
        properties: vec![super::FxProperty::new(
            "opacity",
            FxStaticValue::Parameter(FxParameterSlot {
                index: 1,
                ty: FxRuntimeType::F32,
            }),
        )],
    }])
    .expect("property is locally typed");
    let error = FxDefinition::new(
        FxId::try_new("game", "ui.effects.fade").expect("Fx ID"),
        vec![FxParameter::try_new("strength", FxRuntimeType::F32, None).expect("parameter")],
        graph,
    )
    .expect_err("definition slot inventory is authoritative");
    assert_eq!(
        error,
        FxDefinitionError::ParameterSlotOutOfBounds {
            slot: 1,
            available: 1
        }
    );
}

#[test]
fn instance_snapshot_round_trip_preserves_logical_state() {
    let definition = FxId::try_new("game", "ui.effects.fade").expect("Fx ID");
    let snapshot = FxInstanceSnapshot {
        instance: FxInstanceId::derive(&definition, ["view.hud", "node.1"]),
        definition,
        abi_hash: super::FxAbiHash::derive(["fade"]),
        activation_logical_time: FxLogicalTime::try_new(seconds(4.5)).expect("logical time"),
        deterministic_seed: 42,
        parameters: vec![FxRuntimeValue::F32(finite(0.75))],
        child_path: FxGraphChildPath::try_new(vec![2, 1]).expect("child path"),
        provider_state: Vec::new(),
    }
    .validate()
    .expect("snapshot bounds");
    let bytes = serde_json::to_vec(&snapshot).expect("snapshot serializes");
    assert_eq!(
        serde_json::from_slice::<FxInstanceSnapshot>(&bytes).expect("snapshot revalidates"),
        snapshot
    );
}

#[test]
fn division_by_zero_is_a_structured_evaluation_error() {
    let program = sampler(
        FxRuntimeType::F32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(finite(1.0)),
            },
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(FiniteF32::ZERO),
            },
            ValueInstruction::Div,
            ValueInstruction::Return,
        ],
    );
    let error = program
        .evaluate(
            ValueProgramInputs {
                parameters: &[],
                state: &[],
            },
            context(0.0, 0),
            &mut FxEvaluationBudget::default(),
        )
        .expect_err("zero division fails");
    assert_eq!(error, FxEvaluationError::DivisionByZero { instruction: 2 });
}

#[test]
fn evaluator_rejects_nonfinite_arithmetic_without_clamping() {
    let program = sampler(
        FxRuntimeType::F32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(finite(f32::MAX)),
            },
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(finite(f32::MAX)),
            },
            ValueInstruction::Add,
            ValueInstruction::Return,
        ],
    );
    assert!(matches!(
        program.evaluate(
            ValueProgramInputs {
                parameters: &[],
                state: &[],
            },
            context(0.0, 0),
            &mut FxEvaluationBudget::default(),
        ),
        Err(FxEvaluationError::NonFiniteResult {
            instruction: 2,
            operation: "add"
        })
    ));
}

#[test]
fn sin_sampler_uses_logical_time_and_golden_ordinal_phase() {
    let program = sampler(
        FxRuntimeType::F32,
        vec![
            ValueInstruction::LoadContext {
                slot: super::FxContextSlot::Time,
            },
            ValueInstruction::LoadContext {
                slot: super::FxContextSlot::OrdinalPhase,
            },
            ValueInstruction::Add,
            ValueInstruction::Sin,
            ValueInstruction::Return,
        ],
    );
    let value = program
        .evaluate(
            ValueProgramInputs {
                parameters: &[],
                state: &[],
            },
            context(0.5, 1),
            &mut FxEvaluationBudget::default(),
        )
        .expect("sampler evaluates");
    let FxRuntimeValue::F32(value) = value else {
        panic!("declared F32 result");
    };
    assert_close(value.get(), (0.5 + FX_GOLDEN_ANGLE_RAD).sin());
}

#[test]
fn ordinal_phase_uses_fixed_golden_angle_bits() {
    assert_eq!(FX_GOLDEN_ANGLE_RAD.to_bits(), 0x4019_98ff);
    assert_eq!(
        context(9.0, 1)
            .ordinal_phase()
            .expect("phase remains finite")
            .to_bits(),
        0x4019_98ff
    );
    let expected = (7.0 * FX_GOLDEN_ANGLE_RAD).rem_euclid(std::f32::consts::TAU);
    assert_eq!(
        context(0.0, 7)
            .ordinal_phase()
            .expect("phase remains finite"),
        finite(expected)
    );
}

#[test]
fn reduce_motion_freezes_sampler_time_without_changing_ordinal() {
    let context = FxSampleContext::from_elapsed(seconds(12.0), 7, 123, true);
    assert_eq!(context.time(), FiniteF32::ZERO);
    assert_eq!(context.ordinal(), 7);
}

#[test]
fn evaluation_budget_is_shared_and_fails_before_partial_return() {
    let program = sampler(
        FxRuntimeType::F32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::F32(finite(2.0)),
            },
            ValueInstruction::Return,
        ],
    );
    let error = program
        .evaluate(
            ValueProgramInputs {
                parameters: &[],
                state: &[],
            },
            context(0.0, 0),
            &mut FxEvaluationBudget::new(1),
        )
        .expect_err("return requires a second operation");
    assert_eq!(
        error,
        FxEvaluationError::BudgetExceeded {
            instruction: 1,
            limit: 1
        }
    );
}

#[test]
fn seed_derivation_is_stable_and_includes_nested_child_path() {
    let id = FxId::try_new("game", "ui.effects.wave").expect("Fx id");
    let instance = FxInstanceId::derive(&id, ["view.hud", "node.7", "fx.0"]);
    let semantic = FxSemanticHash::derive(["typed-wave"]);
    let first_path = FxGraphChildPath::try_new(vec![1, 2]).expect("path");
    let second_path = FxGraphChildPath::try_new(vec![1, 3]).expect("path");
    let first = derive_deterministic_seed(instance, semantic, Some(b"authored"), &first_path);
    assert_eq!(
        first,
        derive_deterministic_seed(instance, semantic, Some(b"authored"), &first_path)
    );
    assert_ne!(
        first,
        derive_deterministic_seed(instance, semantic, Some(b"authored"), &second_path)
    );
    assert_ne!(
        first,
        derive_deterministic_seed(instance, semantic, None, &first_path)
    );
}

#[test]
fn unsupported_target_interface_fails_transactionally_with_typed_context() {
    let definition = FxId::try_new("game", "ui.effects.blur").expect("Fx id");
    let instance = FxInstanceId::derive(&definition, ["view.hud", "glyph.0"]);
    let context = FxDiagnosticContext {
        definition: Some(definition),
        instance: Some(instance),
        ..FxDiagnosticContext::default()
    };
    let operation = ResolvedFxOperation::Values(super::ResolvedValueOperation::new(
        FxRendererInterface::Filter,
        FxPhase::GlyphColor,
        FxTarget::Glyph,
        Vec::new(),
    ));
    let plan = ResolvedFxPlan::resolve_application(
        &context,
        &FxCapabilitySet::canonical(),
        vec![operation],
    );
    assert!(plan.glyph().is_empty());
    assert_eq!(plan.diagnostics().len(), 1);
    assert_eq!(
        plan.diagnostics()[0].code,
        FxDiagnosticCode::UnsupportedCapability
    );
    assert_eq!(plan.diagnostics()[0].context.instance, Some(instance));
}

#[test]
fn interactive_noninvertible_transform_is_not_committed() {
    let transform = Transform2D {
        scale_x: FiniteF32::ZERO,
        ..Transform2D::default()
    }
    .resolve()
    .expect("zero scale is a finite visual transform");
    let operation = ResolvedFxOperation::Transform(super::ResolvedTransformOperation::new(
        FxPhase::LayoutTransform,
        FxTarget::Node,
        transform,
        true,
    ));
    let plan = ResolvedFxPlan::resolve_application(
        &FxDiagnosticContext::default(),
        &FxCapabilitySet::canonical(),
        vec![operation],
    );
    assert!(plan.layout().is_empty());
    assert_eq!(
        plan.diagnostics()[0].code,
        FxDiagnosticCode::NonInvertibleTransform
    );
}

#[test]
fn provider_output_enforces_typed_operation_budget() {
    let mut output = FxProviderOutput::new(FxProviderLimits {
        max_operations: 1,
        max_values_per_operation: 2,
        max_state_values: 0,
    });
    let operation = ResolvedFxOperation::Values(super::ResolvedValueOperation::new(
        FxRendererInterface::Color,
        FxPhase::GlyphColor,
        FxTarget::Glyph,
        Vec::new(),
    ));
    output
        .try_push(operation.clone())
        .expect("first output fits");
    assert_eq!(
        output.try_push(operation),
        Err(FxProviderError::OutputBudgetExceeded { limit: 1 })
    );
}

#[test]
fn semantic_hash_canonicalizes_named_property_order() {
    let first = FxGraph::try_new(vec![FxNode::Style {
        properties: vec![
            super::FxProperty::new(
                "opacity",
                FxStaticValue::Runtime(FxRuntimeValue::F32(finite(0.5))),
            ),
            super::FxProperty::new("size", FxRuntimeValue::Length(length(18.0)).into()),
        ],
    }])
    .expect("valid style graph");
    let reordered = FxGraph::try_new(vec![FxNode::Style {
        properties: vec![
            super::FxProperty::new("size", FxRuntimeValue::Length(length(18.0)).into()),
            super::FxProperty::new(
                "opacity",
                FxStaticValue::Runtime(FxRuntimeValue::F32(finite(0.5))),
            ),
        ],
    }])
    .expect("valid style graph");
    assert_eq!(
        FxSemanticHash::for_graph(&first),
        FxSemanticHash::for_graph(&reordered)
    );
}

#[test]
fn provider_kind_inventory_is_typed() {
    let kinds = BTreeSet::from([super::FxProviderKind::Builtin, super::FxProviderKind::Wasm]);
    assert!(kinds.contains(&super::FxProviderKind::Builtin));
    assert!(!kinds.contains(&super::FxProviderKind::Rust));
}
