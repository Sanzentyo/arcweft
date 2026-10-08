use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionDefinitionIdentity,
    RuntimeFunctionSemanticRole, RuntimeGeneratedLocalOrigin, RuntimeLocalDeclarationSeed,
    RuntimeLocalDeclarationSource, RuntimePlanBuildError, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use crate::value::RuntimeValue;

fn fixture() -> (
    RuntimePlanBuilder,
    crate::plan::RuntimeLocalSeedId,
    RuntimeSemanticTypeId,
) {
    let mut builder = RuntimePlanBuilder::new();
    let ty = RuntimeSemanticTypeId::from_bytes([3; 32]);
    let local = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                ty,
                RuntimePlanTypeProjection::Bool,
            )],
            [RuntimeLocalDeclarationSeed::new(
                RuntimeLocalDeclarationSource::Generated(
                    RuntimeGeneratedLocalOrigin::from_accepted_identity([4; 32]),
                ),
                ty,
            )],
        )
        .unwrap()
        .local_ids()[0]
        .clone();
    (builder, local, ty)
}

fn declaration(
    local: crate::plan::RuntimeLocalSeedId,
    ty: RuntimeSemanticTypeId,
) -> RuntimeExprSeed {
    let literal = || RuntimeExprSeed::new(ty, RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)));
    RuntimeExprSeed::new(
        ty,
        RuntimeExprSeedKind::Let {
            binding: local,
            expr: Box::new(literal()),
            body: Box::new(literal()),
        },
    )
}

#[test]
fn two_actual_function_bodies_cannot_declare_the_same_slot() {
    let (mut builder, local, ty) = fixture();
    for definition in [11, 12] {
        builder
            .push_function_site_seed(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([definition; 32]),
                RuntimeFunctionSemanticRole::Ordinary,
                [],
                declaration(local.clone(), ty),
            )
            .unwrap();
    }
    assert!(
        matches!(builder.finish(), Err(RuntimePlanBuildError::LocalPlacement(RuntimeLocalPlacementError::Conflict { first, second, .. })) if first.owner() != second.owner())
    );
}

#[test]
fn materialized_pattern_without_an_executable_owner_cannot_publish() {
    let (builder, local, ty) = fixture();
    builder
        .lower_pattern_seed_for_test(crate::plan::RuntimePatternSeed::new(
            ty,
            crate::plan::RuntimePatternSeedKind::Bind {
                mutable: false,
                local,
            },
        ))
        .unwrap();
    assert!(matches!(
        builder.finish(),
        Err(RuntimePlanBuildError::LocalPlacement(
            RuntimeLocalPlacementError::Unowned { .. }
        ))
    ));
}

#[test]
fn local_semantic_row_commits_owner_identity_without_body_digest_or_arena_address() {
    let make = |padding, definition| {
        let (mut builder, local, ty) = fixture();
        if padding {
            builder
                .push_function_site_seed(
                    RuntimeFunctionDefinitionIdentity::from_accepted_identity([99; 32]),
                    RuntimeFunctionSemanticRole::Ordinary,
                    [],
                    RuntimeExprSeed::new(ty, RuntimeExprSeedKind::Value(RuntimeValue::Bool(false))),
                )
                .unwrap();
        }
        builder
            .push_function_site_seed(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([definition; 32]),
                RuntimeFunctionSemanticRole::Ordinary,
                [],
                declaration(local, ty),
            )
            .unwrap();
        builder.finish().unwrap()
    };
    let digest = |plan: &crate::plan::RuntimePlan, work, bytes| {
        let mut meter = crate::task::semantic::TaskSemanticMeter::new(work, bytes);
        let result = crate::plan::body_semantic::RuntimeBodySemanticContext::new(plan)
            .local_row_digest(
                &mut meter,
                RuntimeLocalDeclarationId::from_accepted_ordinal(std::num::NonZeroU32::MIN),
            );
        (result, meter.totals())
    };
    let first = make(false, 21);
    let (expected, (work, bytes)) = digest(&first, 1000, 10000);
    let expected = expected.unwrap();
    assert_eq!(expected, digest(&make(true, 21), 1000, 10000).0.unwrap());
    assert_ne!(expected, digest(&make(false, 22), 1000, 10000).0.unwrap());
    assert_eq!(expected, digest(&first, work, bytes).0.unwrap());
    assert!(digest(&first, work - 1, bytes).0.is_err());
    assert!(digest(&first, work, bytes - 1).0.is_err());
}
