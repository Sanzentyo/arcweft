use super::*;
use crate::stream::StreamOp;

fn condition() -> crate::value::RuntimeExpr {
    let (plan, _, function) = crate::plan::body_semantic::tests::producer_host_plan(true);
    let RuntimeFunctionSiteBody::Executable(body) =
        plan.function_sites().get(function).unwrap().body()
    else {
        panic!("actual Host fixture body");
    };
    let crate::plan::FlowOp::If { condition, .. } = &body.ops()[0] else {
        panic!("actual Host fixture condition");
    };
    condition.clone()
}

#[test]
fn stream_child_preflight_preserves_then_before_else_and_poison() {
    for (then_count, else_count) in [(3, 4), (4, 3)] {
        let ops = vec![StreamOp::If {
            condition: condition(),
            then_ops: vec![StreamOp::Return; then_count],
            else_ops: vec![StreamOp::Return; else_count],
        }];
        let mut meter = TaskSemanticMeter::new(0, 0);
        let mut check = Children {
            table: 13,
            ordinal: 0,
            maximum: 2,
            meter: &mut meter,
        };
        assert!(
            matches!(check.stream(&ops),Err(RuntimeTaskPlanImageError::Children{table:13,ordinal:0,actual,maximum:2}) if actual==then_count)
        );
        assert!(check.stream(&[]).is_err());
        assert_eq!(meter.totals(), (0, 0));
    }
}

#[test]
fn deep_stream_child_count_uses_cursors_without_recursive_rust_calls() {
    let source = condition();
    let pattern = crate::pattern::RuntimePattern::from_admitted_parts(
        source.ty(),
        crate::pattern::RuntimePatternKind::Discard,
    );
    let mut ops = vec![StreamOp::Return];
    for _ in 0..8192 {
        ops = vec![StreamOp::ForNext {
            pattern: pattern.clone(),
            source: source.clone(),
            body: ops,
        }];
    }
    let mut meter = TaskSemanticMeter::new(0, 0);
    Children {
        table: 13,
        ordinal: 0,
        maximum: 1,
        meter: &mut meter,
    }
    .stream(&ops)
    .unwrap();
    assert_eq!(meter.totals(), (0, 0));
    // Drop the deliberately deep owned fixture iteratively too.
    while let Some(op) = ops.pop() {
        if let StreamOp::ForNext { body, .. } = op {
            ops = body;
        }
    }
}
