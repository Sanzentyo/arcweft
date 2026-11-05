use super::*;
use crate::effect_row::{DecisionControl, DecisionWork, EffectFormula, EffectSet};
use crate::pattern::{RuntimePattern, RuntimeSemanticTypeId};
use crate::plan::{
    RuntimeEffectSet, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionDefinitionIdentity,
    RuntimeFunctionSemanticRole, RuntimeFunctionSiteBodyKind, RuntimeFunctionSiteDeclarationSeed,
    RuntimeFunctionTypeContract, RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimeTypeBinder, RuntimeTypeScope,
};
use crate::value::RuntimeRetainedValueAdmission;
use arcweft_id::PublicId;
use arcweft_interaction_model::presentation::PresentationSlotAddress;

struct Fixture {
    engine: Engine,
    replacing_site: crate::runtime_id::RuntimeFunctionSiteId,
    boolean: RuntimeSemanticTypeId,
    string: RuntimeSemanticTypeId,
    original_header: RuntimeSemanticTypeId,
    result: RuntimePattern,
}

struct Work;
impl DecisionControl for Work {
    type Error = std::convert::Infallible;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn fixture() -> Fixture {
    let unit = RuntimeSemanticTypeId::from_bytes([0xb0; 32]);
    let boolean = RuntimeSemanticTypeId::from_bytes([0xb1; 32]);
    let string = RuntimeSemanticTypeId::from_bytes([0xb2; 32]);
    let headers = [0xb3, 0xb4].map(|tag| RuntimeSemanticTypeId::from_bytes([tag; 32]));
    let binder = RuntimeTypeBinder::new(0, 0, 1);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let variable = EffectFormula::literal(
        EffectSet::default(),
        Some(scope.bound_effect(0, 0).unwrap()),
    );
    let mut builder = RuntimePlanBuilder::new();
    let mut types = vec![
        RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
        RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
        RuntimePlanTypeSeed::new(string, RuntimePlanTypeProjection::String),
    ];
    for (header, effect) in headers.into_iter().zip(["io.read", "io.write"]) {
        let required = EffectFormula::literal(EffectSet::from_labels([effect]).unwrap(), None);
        types.push(RuntimePlanTypeSeed::new(
            header,
            RuntimePlanTypeProjection::Function {
                contract: RuntimeFunctionTypeContract::new(
                    binder,
                    required.subset(&variable, &mut Work).unwrap(),
                    EffectFormula::empty(),
                ),
                parameters: Box::new([]),
                result: unit,
            },
        ));
    }
    builder.admit_type_batch(types, []).unwrap();
    let result = builder
        .lower_pattern_seed_for_test(RuntimePatternSeed::new(
            unit,
            RuntimePatternSeedKind::Discard,
        ))
        .unwrap();
    for (header, identity) in headers.into_iter().zip([0xc0_u8, 0xc1_u8]) {
        let site = builder
            .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [identity; 32],
                ),
                role: RuntimeFunctionSemanticRole::Closure,
                function_type: Some(header),
                inputs: Box::new([]),
                result: unit,
                body_kind: RuntimeFunctionSiteBodyKind::Expression,
                effects: RuntimeEffectSet::empty(),
            })
            .unwrap();
        builder
            .define_function_site_seed(
                &site,
                RuntimeExprSeed::new(unit, RuntimeExprSeedKind::Value(RuntimeValue::Unit)),
            )
            .unwrap();
    }
    let mut plan = builder.finish().unwrap();
    plan.bind_artifact(
        crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0xc2; 32]).unwrap(),
    )
    .unwrap();
    let plan = Arc::new(plan);
    let sites = plan
        .function_sites()
        .iter_with_ids()
        .map(|(site, _)| site)
        .collect::<Vec<_>>();
    let function = crate::pure::RuntimePureFunctionRef::resolve(&plan, sites[0]).unwrap();
    Fixture {
        engine: Engine::for_function_invocation(function, vec![]).unwrap(),
        replacing_site: sites[1],
        boolean,
        string,
        original_header: headers[0],
        result,
    }
}

fn assert_private_roundtrip(engine: &Engine) {
    let image = engine.inert_rollback_image().unwrap();
    assert_eq!(
        Engine::from_rollback_image(image.clone(), &engine.fiber.frame.program)
            .unwrap()
            .inert_rollback_image()
            .unwrap(),
        image
    );
}

#[test]
fn native_slot_replacement_returns_the_original_body_type_and_invocation() {
    let Fixture {
        mut engine,
        replacing_site,
        boolean,
        string,
        original_header,
        result,
    } = fixture();
    let plan = engine.program_plan();
    let original_binding = Arc::clone(
        engine
            .fiber
            .frame
            .env
            .function_instantiation_lease()
            .unwrap(),
    );
    let address = PresentationSlotAddress::try_new(
        PublicId::try_new("target.scene").unwrap(),
        PublicId::try_new("slot.background.default").unwrap(),
        PresentationResourceKind::Background,
    )
    .unwrap();
    let (original_token, previous) = engine
        .register_presentation_body(
            PresentationRegistrationSite::Slot(address.clone()),
            boolean,
            RuntimePayload(RuntimeValue::Bool(true)),
        )
        .unwrap();
    assert!(previous.is_none());
    let invocation = plan
        .validate_function_site_input_refs(replacing_site, &[], &[])
        .unwrap();
    let replacing_binding = Arc::clone(invocation.type_instantiation.as_ref().unwrap());
    engine
        .stage_admitted_function_site_call(
            invocation,
            crate::value::RuntimeCallableInvocation::positional_plan(
                replacing_site,
                vec![],
                vec![],
            )
            .unwrap(),
            FunctionCallFrame::new(replacing_site, FunctionReturnContinuation::Bind { result }),
            None,
        )
        .unwrap();
    let (_, previous) = engine
        .register_presentation_body(
            PresentationRegistrationSite::Slot(address),
            string,
            RuntimePayload(RuntimeValue::String("replacement".into())),
        )
        .unwrap();
    let retired = previous.expect("the occupied slot moves its complete original body");
    assert_eq!(retired.token, original_token);
    let previous = retired.body;
    assert_eq!(previous.expected, boolean);
    assert_eq!(previous.value.value(), &RuntimeValue::Bool(true));
    let original = previous.binding.as_ref().expect("original invocation");
    assert!(Arc::ptr_eq(original, &original_binding));
    assert!(!Arc::ptr_eq(original, &replacing_binding));
    assert_eq!(original.context(), original_header);
    let owner = engine.fiber.frame.program.clone();
    previous
        .admit_retained(&mut RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        ))
        .unwrap();
    assert_private_roundtrip(&engine);
    let mut output = RuntimeStepOutput::default();
    assert!(
        engine
            .return_typed_value(
                RuntimeValue::Unit,
                &mut output,
                &mut crate::pure::VmRuntimePureCallBackend::default(),
            )
            .is_none()
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let retirement = &engine.presentation.as_ref().unwrap().retirements()[0];
    assert_eq!(retirement.body.expected, string);
    assert_eq!(
        retirement.body.value.value(),
        &RuntimeValue::String("replacement".into())
    );
    assert!(Arc::ptr_eq(
        retirement.body.binding.as_ref().unwrap(),
        &replacing_binding
    ));
    retirement
        .body
        .admit_retained(&mut RuntimeRetainedValueAdmission::for_program(
            owner,
            crate::entry::RuntimeSchemaLimits::engine_default(),
        ))
        .unwrap();
    assert_private_roundtrip(&engine);
}
