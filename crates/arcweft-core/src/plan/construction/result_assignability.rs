use super::*;
use crate::pattern::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
};
use crate::plan::*;
use crate::value::{RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeValue};

#[derive(Clone, Copy, Debug)]
pub(super) enum Boundary {
    Function,
    Helper,
    TraitMethod,
}

fn semantic(tag: u8) -> crate::pattern::RuntimeSemanticTypeId {
    crate::pattern::RuntimeSemanticTypeId::from_bytes([tag; 32])
}

fn producer(name: &str) -> RuntimeOpaqueTypeProducerId {
    RuntimeOpaqueTypeProducerId::try_new(name).unwrap()
}

fn opaque(tag: u8, name: &str, admission: RuntimeOpaqueTypeAdmission) -> RuntimePlanTypeSeed {
    RuntimePlanTypeSeed::new(
        semantic(tag),
        RuntimePlanTypeProjection::Opaque {
            producer: producer(name),
            admission,
            value_class: RuntimeOpaqueValueClass::Plain,
            persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments: Box::new([]),
        },
    )
}

fn declarations() -> [RuntimePlanTypeSeed; 6] {
    [
        opaque(
            1,
            "std.test.result",
            RuntimeOpaqueTypeAdmission::ExactIdentity,
        ),
        opaque(
            2,
            "std.test.result",
            RuntimeOpaqueTypeAdmission::ExactIdentity,
        ),
        opaque(
            3,
            "std.test.result",
            RuntimeOpaqueTypeAdmission::ProducerWide,
        ),
        opaque(
            4,
            "std.test.other",
            RuntimeOpaqueTypeAdmission::ExactIdentity,
        ),
        RuntimePlanTypeSeed::new(
            semantic(5),
            RuntimePlanTypeProjection::Choice(Box::new([semantic(1), semantic(2)])),
        ),
        RuntimePlanTypeSeed::new(semantic(6), RuntimePlanTypeProjection::Bool),
    ]
}

pub(super) fn define(
    boundary: Boundary,
    expected: u8,
    actual: u8,
) -> Result<RuntimePlan, Box<RuntimePlanBuildError>> {
    let mut builder = RuntimePlanBuilder::new();
    let parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([0x70; 32]);
    let admitted = builder
        .admit_type_batch(
            declarations(),
            [RuntimeLocalDeclarationSeed::new(
                RuntimeLocalDeclarationSource::Parameter(parameter),
                semantic(6),
            )],
        )
        .unwrap();
    let exact = if actual == 3 { 1 } else { actual };
    let owner = RuntimeOpaqueTypeOwner::exact(
        producer(if exact == 4 {
            "std.test.other"
        } else {
            "std.test.result"
        }),
        semantic(exact),
    );
    let body = RuntimeExprSeed::new(
        semantic(actual),
        RuntimeExprSeedKind::Value(owner.try_wrap(RuntimeValue::Unit).unwrap()),
    );
    let definition = RuntimeFunctionDefinitionIdentity::from_accepted_identity([0x71; 32]);
    match boundary {
        Boundary::Function => {
            let site = builder.reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                definition,
                role: RuntimeFunctionSemanticRole::Ordinary,
                function_type: None,
                inputs: Box::new([]),
                result: semantic(expected),
                body_kind: RuntimeFunctionSiteBodyKind::Expression,
                effects: RuntimeEffectSet::empty(),
            })?;
            builder.define_function_site_seed(&site, body)?;
        }
        Boundary::Helper => {
            let helper = builder.reserve_pure_helper_seed(RuntimePureHelperDeclarationSeed {
                definition,
                name: "result".to_owned(),
                inputs: Box::new([]),
                result: semantic(expected),
                output_abi: RuntimePureOutputType::Value,
                scalar_eval_supported: false,
                origin: RuntimePureHelperOrigin::Inferred,
            })?;
            builder.define_pure_helper_seed(&helper, body)?;
        }
        Boundary::TraitMethod => {
            let method = builder.reserve_trait_method_seed(RuntimeTraitMethodDeclarationSeed {
                definition,
                identity: RuntimeTraitMethodIdentity {
                    impl_id: 0,
                    trait_id: Some(0),
                    witness: Some(0),
                    trait_name: Some("ResultProducer".to_owned()),
                    self_type: "Bool".to_owned(),
                    method_name: "result".to_owned(),
                    monomorph_label: "Bool.result".to_owned(),
                },
                receiver: RuntimeReceiverMode::Owned,
                inputs: Box::new([RuntimeCallableParameterSeed {
                    identity: parameter,
                    local: admitted.local_ids()[0].clone(),
                    passing: RuntimeFunctionParameterPassing::Value,
                    abi: RuntimePureInputType::Value,
                }]),
                result: semantic(expected),
                output_abi: RuntimePureOutputType::Value,
            })?;
            builder.define_trait_method_seed(&method, body)?;
        }
    }
    let plan = builder.finish()?;
    assert_result_contract(&plan, boundary, expected, actual);
    Ok(plan)
}

fn assert_result_contract(plan: &RuntimePlan, boundary: Boundary, expected: u8, actual: u8) {
    let (declared, body) = match boundary {
        Boundary::Function => {
            let function = plan.function_sites().iter().next().unwrap();
            (
                function.result(),
                function.body().expression().unwrap().ty(),
            )
        }
        Boundary::Helper => (
            plan.pure_helpers()[0].result,
            plan.pure_helpers()[0].expr.ty(),
        ),
        Boundary::TraitMethod => (
            plan.trait_methods()[0].result,
            plan.trait_methods()[0].body.ty(),
        ),
    };
    assert_eq!(
        plan.type_table().get(declared).unwrap().semantic_identity(),
        semantic(expected)
    );
    assert_eq!(
        plan.type_table().get(body).unwrap().semantic_identity(),
        semantic(actual)
    );
}

#[test]
fn every_callable_result_accepts_its_existing_opaque_and_choice_supertypes() {
    for boundary in [Boundary::Function, Boundary::Helper, Boundary::TraitMethod] {
        for (expected, actual) in [(1, 1), (3, 1), (3, 2), (5, 1), (5, 2)] {
            define(boundary, expected, actual).unwrap_or_else(|error| {
                panic!("{boundary:?} rejected admitted result {expected} <- {actual}: {error}")
            });
        }
    }
}

#[test]
fn every_callable_result_refuses_narrowing_foreign_producers_and_unrelated_alternatives() {
    for boundary in [Boundary::Function, Boundary::Helper, Boundary::TraitMethod] {
        for (expected, actual) in [(1, 2), (1, 3), (3, 4), (5, 4)] {
            assert!(
                matches!(
                    define(boundary, expected, actual),
                    Err(error) if matches!(*error, RuntimePlanBuildError::TypeMismatch { .. })
                ),
                "{boundary:?} accepted invalid result {expected} <- {actual}"
            );
        }
    }
}
