use super::*;
use crate::effect_row::{EffectFormula, EffectPredicate};
use crate::plan::{
    RuntimeFunctionTypeContract, RuntimeTypeBinder, RuntimeTypeScope, RuntimeTypeScopeError,
};

fn identity(value: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([value; 32])
}

#[test]
fn function_binder_admits_its_scoped_child_and_closed_root_children() {
    let binder = RuntimeTypeBinder::new(1, 0, 0);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let bound = RuntimePlanTypeSeed::new(
        identity(1),
        RuntimePlanTypeProjection::BoundType(scope.bound_type(0, 0).unwrap()),
    )
    .with_scope(scope.clone());
    let closed = RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::Bool);
    let function = RuntimePlanTypeSeed::new(
        identity(3),
        RuntimePlanTypeProjection::Function {
            contract: RuntimeFunctionTypeContract::new(
                binder,
                EffectPredicate::unconstrained(),
                EffectFormula::empty(),
            ),
            parameters: Box::new([identity(1), identity(2)]),
            result: identity(1),
        },
    );
    let mut table = RuntimePlanTypeTableBuilder::new();
    let ids = table.intern_batch([function, bound, closed]).unwrap();
    let table = table.finish().unwrap();
    assert!(table.get(ids[0]).unwrap().scope().is_root());
    assert_eq!(table.get(ids[1]).unwrap().scope(), &scope);
    assert!(!table.is_checked(ids[1]).unwrap());
}

#[test]
fn scoped_type_cannot_escape_its_binder_or_attach_to_a_different_binder() {
    let binder = RuntimeTypeBinder::new(1, 0, 0);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let reference = scope.bound_type(0, 0).unwrap();
    let mut table = RuntimePlanTypeTableBuilder::new();
    assert!(matches!(
        table.intern(RuntimePlanTypeSeed::new(
            identity(1),
            RuntimePlanTypeProjection::BoundType(reference)
        )),
        Err(RuntimePlanTypeTableError::Scope {
            source: RuntimeTypeScopeError::UnknownDepth { .. },
            ..
        })
    ));
    let bound =
        RuntimePlanTypeSeed::new(identity(1), RuntimePlanTypeProjection::BoundType(reference))
            .with_scope(scope);
    let function = RuntimePlanTypeSeed::new(
        identity(2),
        RuntimePlanTypeProjection::Function {
            contract: RuntimeFunctionTypeContract::new(
                RuntimeTypeBinder::new(2, 0, 0),
                EffectPredicate::unconstrained(),
                EffectFormula::empty(),
            ),
            parameters: Box::new([identity(1)]),
            result: identity(1),
        },
    );
    assert!(matches!(
        table.intern_batch([function, bound]),
        Err(RuntimePlanTypeTableError::ChildScope { .. })
    ));
}

#[test]
fn executable_slots_reject_scoped_descendants_but_accept_closed_schemes() {
    use crate::plan::construction::{
        RuntimeFunctionSiteDeclarationSeed, RuntimeLocalDeclarationSeed, RuntimePlanBuildError,
        RuntimePlanBuilder,
    };
    use crate::plan::{RuntimeEffectSet, RuntimeFunctionSiteBodyKind};

    let binder = RuntimeTypeBinder::new(1, 0, 0);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let seeds = [
        RuntimePlanTypeSeed::new(
            identity(1),
            RuntimePlanTypeProjection::BoundType(scope.bound_type(0, 0).unwrap()),
        )
        .with_scope(scope),
        RuntimePlanTypeSeed::new(
            identity(2),
            RuntimePlanTypeProjection::Function {
                contract: RuntimeFunctionTypeContract::new(
                    binder,
                    EffectPredicate::unconstrained(),
                    EffectFormula::empty(),
                ),
                parameters: Box::new([identity(1)]),
                result: identity(1),
            },
        ),
    ];
    let mut builder = RuntimePlanBuilder::new();
    assert!(matches!(
        builder.admit_type_batch(
            seeds.clone(),
            [RuntimeLocalDeclarationSeed::new(identity(1))],
        ),
        Err(RuntimePlanBuildError::InvalidTypeProjection {
            context: "scoped local declaration type",
            ..
        })
    ));
    // Reusing the same builder also verifies failed local admission is atomic.
    builder
        .admit_type_batch(seeds, [RuntimeLocalDeclarationSeed::new(identity(2))])
        .unwrap();
    assert!(matches!(
        builder.reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            inputs: Box::new([]),
            result: identity(1),
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeEffectSet::empty(),
        }),
        Err(RuntimePlanBuildError::InvalidTypeProjection {
            context: "function result",
            ..
        })
    ));
}

#[test]
fn contextual_local_admission_requires_its_function_scope_and_remains_atomic() {
    use crate::plan::construction::{
        RuntimeLocalDeclarationSeed, RuntimePlanBuildError, RuntimePlanBuilder,
    };
    let binder = RuntimeTypeBinder::new(1, 0, 0);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let scoped = RuntimePlanTypeSeed::new(
        identity(1),
        RuntimePlanTypeProjection::BoundType(scope.bound_type(0, 0).unwrap()),
    )
    .with_scope(scope);
    let closed = RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::Bool);
    let function = |id, binder| {
        RuntimePlanTypeSeed::new(
            identity(id),
            RuntimePlanTypeProjection::Function {
                contract: RuntimeFunctionTypeContract::new(
                    binder,
                    EffectPredicate::unconstrained(),
                    EffectFormula::empty(),
                ),
                parameters: Box::new([identity(2)]),
                result: identity(2),
            },
        )
    };
    let seeds = [
        scoped,
        closed,
        function(3, binder),
        function(4, RuntimeTypeBinder::new(2, 0, 0)),
    ];
    let mut builder = RuntimePlanBuilder::new();
    for context in [identity(2), identity(4)] {
        assert!(matches!(
            builder.admit_type_batch(
                seeds.clone(),
                [RuntimeLocalDeclarationSeed::in_function(
                    identity(1),
                    context
                )]
            ),
            Err(RuntimePlanBuildError::InvalidTypeProjection {
                context: "function-owned local declaration type",
                ..
            })
        ));
    }
    let admitted = builder
        .admit_type_batch(
            seeds,
            [RuntimeLocalDeclarationSeed::in_function(
                identity(1),
                identity(3),
            )],
        )
        .unwrap();
    assert_eq!(admitted.local_ids().len(), 1);
    let plan = builder.finish().unwrap();
    let declaration = plan.local_declarations().declarations().next().unwrap();
    assert_eq!(plan.local_declarations().len(), 1);
    assert_eq!(
        plan.type_table()
            .get(declaration.context().unwrap())
            .unwrap()
            .semantic_identity(),
        identity(3)
    );
}
