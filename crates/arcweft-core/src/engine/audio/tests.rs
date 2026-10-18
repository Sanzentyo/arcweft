use super::*;
use crate::pattern::RuntimeCheckedType;
use crate::plan::*;
use crate::value::{RuntimeExprKind, RuntimeLocalReadMode};
use arcweft_id::{DeclarationName, LifetimeRegistryKey, LifetimeScopeKind};
use std::sync::Arc;

#[test]
fn native_registry_context_rejection_preserves_the_original_affine_rhs_slot() {
    let boolean = RuntimeCheckedType::Bool.semantic_identity_digest();
    let need = crate::pattern::RuntimeSemanticTypeId::from_bytes([0xab; 32]);
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(need, RuntimePlanTypeProjection::Need(boolean)),
            ],
            [RuntimeLocalDeclarationSeed::new(
                RuntimeLocalDeclarationSource::Binding {
                    identity: [0xac; 32],
                    declaration: RuntimeLocalBindingDeclaration::new(
                        RuntimeLocalBindingKind::PatternBinding,
                        false,
                        RuntimeLocalBindingStorage::Derived,
                    ),
                },
                need,
            )],
        )
        .unwrap();
    let local = admitted.local_ids()[0].clone();
    let site = builder
        .push_function_site_seed(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xad; 32]),
            RuntimeFunctionSemanticRole::Ordinary,
            [RuntimeFunctionInputBindingSeed {
                transfer: RuntimeFunctionInputTransfer::Formal,
                origin: RuntimeFunctionInputOrigin::Parameter(
                    RuntimeFunctionParameterIdentity::from_accepted_identity([0xae; 32]),
                ),
                source: RuntimeFunctionInputSource::Parameter {
                    position: 0,
                    passing: RuntimeFunctionParameterPassing::Affine,
                },
                input_local: local.clone(),
                pattern: RuntimePatternSeed::new(
                    need,
                    RuntimePatternSeedKind::Bind {
                        local: local.clone(),
                        mutable: false,
                    },
                ),
                ownership: RuntimeValueOwnershipRequirement::Owned,
                unrestricted_bindings: Box::new([]),
            }],
            RuntimeExprSeed::new(
                need,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    local,
                    RuntimeLocalReadMode::Move,
                )),
            ),
        )
        .unwrap();
    let program =
        arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest([0xaf; 32]);
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
            program,
            site: site.clone(),
        })
        .unwrap();
    let plan = Arc::new(builder.finish().unwrap());
    let rhs = plan
        .function_sites()
        .get(site.id())
        .unwrap()
        .body()
        .expression()
        .unwrap()
        .clone();
    let RuntimeExprKind::Local(read) = rhs.kind() else {
        panic!("the fixture's actual function body moves its input")
    };
    let local = read.local();
    let handle = crate::tests::reusable_need_with_outcome(
        "registry.context.original",
        crate::task::TaskOutcomeContract::program(boolean),
    );
    let correlation = handle.correlation();
    let mut engine =
        Engine::for_program_invocation(plan, program, vec![RuntimeValue::NeedHandle(handle)])
            .unwrap();
    let effect = RuntimeEffectExpr::RegistryWrite {
        key: LifetimeRegistryKey::new(
            LifetimeScopeKind::Flow,
            vec![DeclarationName::try_new("value").unwrap()].into_boxed_slice(),
        ),
        value: rhs,
    };
    let mut backend = crate::pure::VmRuntimePureCallBackend::default();
    let error = match engine.evaluate_effect_expr(&effect, &mut backend) {
        Err(error) => error,
        Ok(_) => panic!("a program invocation without a Flow scope must refuse the write"),
    };
    assert_eq!(
        error,
        RuntimeEffectExecutionError::Materialization(
            RuntimeEffectMaterializeError::RegistryContextRequired
        )
    );
    let Some(RuntimeValue::NeedHandle(retained)) = engine.fiber.frame.env.get(local) else {
        panic!("rejection must leave the original Need in its actual owning slot")
    };
    assert_eq!(retained.correlation(), correlation);
    assert!(
        !engine
            .fiber
            .frame
            .env
            .get(local)
            .unwrap()
            .ownership()
            .permits_copy()
    );
}
