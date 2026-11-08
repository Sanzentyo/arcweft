use super::*;
use crate::pattern::RuntimeCheckedType;
use crate::plan::{
    RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeFlowSeed,
    RuntimeFunctionDefinitionIdentity, RuntimeFunctionSiteDeclarationSeed, RuntimePlan,
    RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};

fn plan(ids: impl IntoIterator<Item = FlowRuntimeId>) -> RuntimePlan {
    plan_result(ids.into_iter().map(|id| (id, Box::new([]) as Box<[_]>))).unwrap()
}

fn plan_result(
    bodies: impl IntoIterator<Item = (FlowRuntimeId, Box<[crate::plan::RuntimeFlowOpSeed]>)>,
) -> Result<RuntimePlan, crate::plan::RuntimePlanBuildError> {
    let mut builder = RuntimePlanBuilder::new();
    let unit = RuntimeCheckedType::Unit.semantic_identity_digest();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                unit,
                RuntimePlanTypeProjection::Unit,
            )],
            [],
        )
        .unwrap();
    for (id, ops) in bodies {
        builder
            .push_flow_schema(RuntimeFlowSchema {
                flow: id.clone(),
                parameters: vec![],
            })
            .unwrap();
        builder
            .push_flow_seed(RuntimeFlowSeed::new(
                id,
                RuntimeFunctionSiteDeclarationSeed::flow(
                    RuntimeFunctionDefinitionIdentity::from_accepted_identity([61; 32]),
                    None,
                    Box::new([]),
                    unit,
                    RuntimeEffectSet::empty(),
                ),
                RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops,
                },
            ))
            .unwrap();
    }
    builder.finish()
}

#[test]
fn missing_static_flow_transfer_cannot_publish_an_executable_plan() {
    let source = FlowRuntimeId::canonical("source").unwrap();
    let target = FlowRuntimeId::canonical("missing").unwrap();
    let error = plan_result([(
        source,
        Box::new([crate::plan::RuntimeFlowOpSeed::Goto(
            crate::plan::RuntimeFlowTransfer::static_without_arguments(target.clone()),
        )]) as Box<[_]>,
    )])
    .unwrap_err();
    assert!(matches!(error,
        crate::plan::RuntimePlanBuildError::Plan(crate::plan::RuntimePlanError::FlowTransfer(error))
            if *error == crate::plan::RuntimeFlowInvocationError::UnknownFlow { flow: target.canonical_label() }));
}

#[test]
fn flow_lookup_preserves_source_order_and_returns_actual_rows_and_schemas() {
    let ids = [
        FlowRuntimeId::canonical("z").unwrap(),
        FlowRuntimeId::canonical("a").unwrap(),
    ];
    let plan = plan(ids.clone());
    for (ordinal, id) in ids.iter().enumerate() {
        assert_eq!(plan.flows()[ordinal].id, *id);
        assert_eq!(plan.flows.position(id), Some(ordinal));
        assert!(std::ptr::eq(
            plan.flows.flow(id).unwrap(),
            &raw const plan.flows()[ordinal]
        ));
        assert!(std::ptr::eq(
            plan.flows.schema(id).unwrap(),
            &raw const plan.flow_schemas()[ordinal]
        ));
    }
    let missing = FlowRuntimeId::canonical("missing").unwrap();
    assert!(plan.flows.position(&missing).is_none());
    assert!(plan.flows.flow(&missing).is_none());
    assert!(plan.flows.schema(&missing).is_none());
}

#[test]
fn indexed_target_lookup_preserves_exact_priority_and_selector_ambiguity() {
    let manual = FlowRuntimeId::canonical("opening").unwrap();
    let first = FlowRuntimeId::from_checked_declaration_digest([11; 32], "flow.opening").unwrap();
    let second = FlowRuntimeId::from_checked_declaration_digest([12; 32], "flow.opening").unwrap();
    let exact = plan([first.clone(), manual.clone(), second.clone()]);
    assert_eq!(exact.resolve_flow_target_value("opening").unwrap(), manual);
    let ambiguous = plan([first.clone(), second]);
    assert!(matches!(
        ambiguous.resolve_flow_target_value("flow.opening"),
        Err(RuntimeFlowTargetError::Ambiguous { matches: 2, .. })
    ));
    let unique = plan([first.clone()]);
    assert_eq!(unique.resolve_flow_target_value("opening").unwrap(), first);
    assert!(matches!(
        unique.resolve_flow_target_value("unknown"),
        Err(RuntimeFlowTargetError::Missing { .. })
    ));
    assert!(matches!(
        unique.resolve_flow_target_value("invalid..target"),
        Err(RuntimeFlowTargetError::Invalid(_))
    ));
}

#[test]
fn ambiguous_inert_rows_are_rejected_in_existing_structural_error_order() {
    let original = plan([
        FlowRuntimeId::canonical("one").unwrap(),
        FlowRuntimeId::canonical("two").unwrap(),
    ]);
    let (rows, schemas) = original.inventory.flows.clone().into_parts();
    let mut duplicate_rows = rows.clone();
    duplicate_rows[1].id = duplicate_rows[0].id.clone();
    let duplicated = RuntimeFlowTable::from_rows(duplicate_rows, schemas.clone());
    assert!(duplicated.position(&rows[0].id).is_none());
    assert!(duplicated.flow(&rows[0].id).is_none());
    assert_eq!(duplicated.resolve_target("one").unwrap(), rows[0].id);
    let mut candidate = original.clone();
    candidate.inventory.flows = duplicated;
    assert!(matches!(
        candidate.verify(),
        Err(crate::plan::RuntimePlanError::DuplicateFlow(_))
    ));
    let mut candidate = original;
    candidate.inventory.flows = RuntimeFlowTable::from_rows(
        rows,
        vec![schemas[0].clone(), schemas[0].clone(), schemas[1].clone()],
    );
    assert!(candidate.flows.schema(&schemas[0].flow).is_none());
    assert!(matches!(
        candidate.verify(),
        Err(crate::plan::RuntimePlanError::DuplicateFlowSchema(_))
    ));
}
