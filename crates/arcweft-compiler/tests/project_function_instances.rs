use arcweft_compiler::source::compile_source;
use arcweft_core::plan::{
    FlowOp, RuntimeFunctionSiteBody, RuntimeProjectCallOrdinaryMaterialization,
};
use arcweft_core::value::{
    RuntimeAgentExpr, RuntimeCallArgumentMode, RuntimeExprKind, RuntimeValue,
};
use arcweft_lang_sema::callable::{
    CheckedProjectFunctionRuntimeInput, CheckedProjectFunctionRuntimeOutcome,
    select_project_function_runtime,
};

#[path = "support/execution.rs"]
mod execution;

#[test]
fn growing_instance_graph_stops_at_the_configured_inclusive_bound() {
    use arcweft_compiler::{
        lower::{ProjectInstantiationControl, ProjectInstantiationLimits},
        source::compile_source_with_env_and_control,
    };
    let error = compile_source_with_env_and_control(
        "fn grow<T>(value: T) -> i64 { grow([value]) }\nflow main() -> i64 { return grow(1i64) }\n",
        &arcweft_lang_sema::env::TypeCheckEnv::standard(),
        ProjectInstantiationControl::default().with_limits(ProjectInstantiationLimits::new(
            2,
            10,
            u64::MAX,
            128,
            100_000,
        )),
    )
    .expect_err("growing specialization must stop before publishing a runtime plan");
    assert_eq!(error.project().stage(), "runtime-plan-lower");
    assert!(
        error.project().diagnostics().iter().any(|diagnostic| {
            diagnostic
                .diagnostic()
                .code()
                .is_some_and(|code| code.as_str() == "compiler.project_instantiation.limit")
        }),
        "{error:?}"
    );
}

#[test]
fn growing_types_stop_at_the_depth_bound_before_exhausting_instance_slots() {
    use arcweft_compiler::{
        lower::{ProjectInstantiationControl, ProjectInstantiationLimits},
        source::compile_source_with_env_and_control,
    };
    let error = compile_source_with_env_and_control(
        "fn grow<T>(value: T) -> i64 { grow([value]) }\nflow main() -> i64 { return grow(1i64) }\n",
        &arcweft_lang_sema::env::TypeCheckEnv::standard(),
        ProjectInstantiationControl::default().with_limits(ProjectInstantiationLimits::new(
            100, 100, 100_000, 4, 100_000,
        )),
    )
    .expect_err("unbounded type growth is rejected at the structural depth boundary");
    assert_eq!(error.project().stage(), "runtime-plan-lower");
    assert!(
        error.project().diagnostics().iter().any(|diagnostic| {
            diagnostic
                .diagnostic()
                .code()
                .is_some_and(|code| code.as_str() == "compiler.project_instantiation.limit")
                && diagnostic.diagnostic().message().contains("TypeDepth")
        }),
        "{error:?}"
    );
}

#[test]
fn instance_closure_obeys_the_structural_node_budget() {
    use arcweft_compiler::{
        lower::{ProjectInstantiationControl, ProjectInstantiationLimits},
        source::compile_source_with_env_and_control,
    };
    let error = compile_source_with_env_and_control(
        "fn identity<T>(value: T) -> T { value }\nflow main() -> i64 { return identity(42i64) }\n",
        &arcweft_lang_sema::env::TypeCheckEnv::standard(),
        ProjectInstantiationControl::default()
            .with_limits(ProjectInstantiationLimits::new(100, 100, 1, 128, 100_000)),
    )
    .expect_err("closing a substitution must consume the structural budget");
    assert_eq!(error.project().stage(), "runtime-plan-lower");
    assert!(
        error.project().diagnostics().iter().any(|diagnostic| {
            diagnostic
                .diagnostic()
                .code()
                .is_some_and(|code| code.as_str() == "compiler.project_instantiation.limit")
                && diagnostic
                    .diagnostic()
                    .message()
                    .contains("StructuralNodes")
        }),
        "{error:?}"
    );
}

#[test]
fn body_and_nested_closure_projection_obey_the_same_depth_budget() {
    use arcweft_compiler::{
        lower::{ProjectInstantiationControl, ProjectInstantiationLimits},
        source::compile_source_with_env_and_control,
    };
    for body in [
        "let values = [[[1i64]]]; 42i64",
        "let run = || { let values = [[[1i64]]]; 42i64 }; run()",
    ] {
        let source =
            format!("fn nested() -> i64 {{ {body} }}\nflow main() -> i64 {{ return nested() }}\n");
        compile_source(&source).expect("both bodies compile with the production budget");
        let error = compile_source_with_env_and_control(
            &source,
            &arcweft_lang_sema::env::TypeCheckEnv::standard(),
            ProjectInstantiationControl::default().with_limits(ProjectInstantiationLimits::new(
                100, 100, 100_000, 2, 100_000,
            )),
        )
        .expect_err("the shallow callable signature does not exempt its body from depth control");
        assert_eq!(error.project().stage(), "runtime-plan-lower");
        assert!(
            error.project().diagnostics().iter().any(|diagnostic| {
                diagnostic
                    .diagnostic()
                    .code()
                    .is_some_and(|code| code.as_str() == "compiler.project_instantiation.limit")
                    && diagnostic.diagnostic().message().contains("TypeDepth")
            }),
            "{error:?}"
        );
    }
}

#[test]
fn same_type_recursion_reuses_the_single_allowed_instance() {
    use arcweft_compiler::{
        lower::{ProjectInstantiationControl, ProjectInstantiationLimits},
        source::compile_source_with_env_and_control,
    };
    compile_source_with_env_and_control(
        "fn again<T>(n: i64, value: T) -> i64 { if n == 0i64 { 42i64 } else { again(n - 1i64, value) } }\nflow main() -> i64 { return again(2i64, 1i64) }\n",
        &arcweft_lang_sema::env::TypeCheckEnv::standard(),
        ProjectInstantiationControl::default().with_limits(ProjectInstantiationLimits::new(1, 1, u64::MAX, 128, 100_000)),
    ).expect("a finite self-recursive graph fits exactly one instance and one edge");
}

#[test]
fn shared_curried_prefix_keeps_one_lineage_and_one_closed_instance_key() {
    let compiled = compile_source(
        r#"
fn make(first: i64)(second: i64) -> i64 { second }

flow main() -> i64 {
    let partial = make(1i64)
    partial(2i64)
    return partial(3i64)
}
"#,
    )
    .expect("shared curried prefix compiles through runtime instance fact publication");
    let analysis = compiled.analysis.final_analysis().as_ref();
    let selections = analysis
        .calls()
        .filter_map(|(owner, facts)| {
            let application = facts.selected_application()?;
            let join = analysis
                .checked_callable_join(owner)
                .expect("selected call has a checked join");
            select_project_function_runtime(application, join, analysis.checked_callables())
                .expect("project-function runtime selection")
        })
        .collect::<Vec<_>>();
    assert_eq!(selections.len(), 3);
    let origin = selections
        .iter()
        .find(|selection| {
            matches!(
                selection.outcome(),
                CheckedProjectFunctionRuntimeOutcome::Continue { .. }
            )
        })
        .expect("prefix Continue selection");
    assert!(matches!(
        origin.input(),
        CheckedProjectFunctionRuntimeInput::Direct
    ));
    let CheckedProjectFunctionRuntimeOutcome::Continue {
        abi: origin_abi,
        next_group,
        ..
    } = origin.outcome()
    else {
        unreachable!("selected origin continues")
    };
    let origin_lineage = origin_abi.lineage();
    assert_eq!(next_group.get(), 1);
    let terminals = selections
        .iter()
        .filter(|selection| {
            matches!(
                selection.outcome(),
                CheckedProjectFunctionRuntimeOutcome::Invoke { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(terminals.len(), 2);
    assert_eq!(terminals[0].instantiation(), terminals[1].instantiation());
    for terminal in terminals {
        let CheckedProjectFunctionRuntimeInput::Continuation { abi } = terminal.input() else {
            panic!("terminal shared-prefix call consumes a continuation")
        };
        assert_eq!(abi.lineage(), origin_lineage);
        assert_eq!(terminal.group().get(), 1);
    }
}

#[test]
fn distinct_generic_substitutions_publish_distinct_closed_instances() {
    let compiled = compile_source(
        r#"
fn choose<T>(first: T)(second: T) -> T { second }

flow main() -> i64 {
    let integers = choose(1i64)
    integers(2i64)
    let strings = choose("first")
    strings("second")
    return 0i64
}
"#,
    )
    .expect("two generic substitutions compile as distinct closed instance facts");
    let analysis = compiled.analysis.final_analysis().as_ref();
    let terminal_instantiations = analysis
        .calls()
        .filter_map(|(owner, facts)| {
            let application = facts.selected_application()?;
            let join = analysis.checked_callable_join(owner).ok()?;
            let selection =
                select_project_function_runtime(application, join, analysis.checked_callables())
                    .ok()??;
            matches!(
                selection.outcome(),
                CheckedProjectFunctionRuntimeOutcome::Invoke { .. }
            )
            .then_some(selection.instantiation())
        })
        .collect::<Vec<_>>();
    assert_eq!(terminal_instantiations.len(), 2);
    assert_ne!(terminal_instantiations[0], terminal_instantiations[1]);
}

#[test]
fn named_project_call_lowers_source_ordered_operands_into_typed_anf() {
    let source = r#"
entry cli @entry.main { goto @flow.main }
struct State { value: i64 }
fn reorder(first: i64, second: i64) -> i64 { first * 10i64 + second }

flow main() -> i64 {
    let state = State { value = 0i64 }
    let ordered = reorder(
        second = { let value = state.value + 1i64
            state.value = value
            value },
        first = { let value = state.value + 1i64
            state.value = value
            value },
    )
    return ordered * 10i64 + state.value
}
"#;
    let compiled =
        compile_source(source).expect("named project call lowers through the typed flow plan");

    let project_call = compiled
        .plan
        .flows()
        .iter()
        .flat_map(|flow| flow.body().ops().iter())
        .find_map(|op| match op {
            FlowOp::ProjectCall { site } => compiled
                .plan
                .project_call_sites()
                .get(*site)
                .map(|site| site.plan()),
            _ => None,
        })
        .expect("the named call is represented by one FlowOp::ProjectCall");

    assert_eq!(project_call.operands().len(), 2);
    assert!(matches!(
        project_call.operands()[0].value().kind(),
        RuntimeExprKind::Local(_)
    ));
    assert!(matches!(
        project_call.operands()[1].value().kind(),
        RuntimeExprKind::Local(_)
    ));
    let [first, second] = project_call.ordinary() else {
        panic!("reorder has exactly two fixed logical parameters")
    };
    assert!(matches!(
        first,
        RuntimeProjectCallOrdinaryMaterialization::Fixed(row)
            if row.parameter() == 0 && row.source_index() == 1
    ));
    assert!(matches!(
        second,
        RuntimeProjectCallOrdinaryMaterialization::Fixed(row)
            if row.parameter() == 1 && row.source_index() == 0
    ));
    assert!(compiled.plan.pure_helpers().is_empty());
    execution::assert_native_return(source, "212");
    execution::assert_awbc_return(source, RuntimeValue::i64(212));
}

#[test]
fn rest_project_call_lowers_spread_once_and_materializes_source_indices() {
    let compiled = compile_source(
        r#"
fn collect(head: i64, tail: ...i64) -> i64 { head }

flow main() -> i64 {
    return collect(1i64, [2i64, 3i64]...)
}
"#,
    )
    .expect("rest project call lowers through the typed flow plan");

    let project_call = compiled
        .plan
        .flows()
        .iter()
        .flat_map(|flow| flow.body().ops().iter())
        .find_map(|op| match op {
            FlowOp::ProjectCall { site } => compiled
                .plan
                .project_call_sites()
                .get(*site)
                .map(|site| site.plan()),
            _ => None,
        })
        .expect("the rest call is represented by one FlowOp::ProjectCall");

    assert_eq!(project_call.operands().len(), 3);
    assert_eq!(
        project_call.operands()[0].mode(),
        RuntimeCallArgumentMode::Value
    );
    assert_eq!(
        project_call.operands()[1].mode(),
        RuntimeCallArgumentMode::Value
    );
    assert_eq!(
        project_call.operands()[2].mode(),
        RuntimeCallArgumentMode::Value
    );
    let [head, tail] = project_call.ordinary() else {
        panic!("collect has exactly two logical parameters")
    };
    assert!(matches!(
        head,
        RuntimeProjectCallOrdinaryMaterialization::Fixed(row)
            if row.parameter() == 0 && row.source_index() == 0
    ));
    assert!(matches!(
        tail,
        RuntimeProjectCallOrdinaryMaterialization::Rest(row)
            if row.parameter() == 1 && row.source_indices() == [1, 2]
    ));
}

#[test]
fn curried_project_call_lowers_continue_then_invoke_without_pure_helper_fallback() {
    let compiled = compile_source(
        r#"
fn make(first: i64)(second: i64) -> i64 { second }

flow main() -> i64 {
    let partial = make(1i64)
    return partial(2i64)
}
"#,
    )
    .expect("curried project calls lower through the typed flow plan");

    let calls = compiled
        .plan
        .flows()
        .iter()
        .flat_map(|flow| flow.body().ops().iter())
        .filter_map(|op| match op {
            FlowOp::ProjectCall { site } => compiled
                .plan
                .project_call_sites()
                .get(*site)
                .map(|site| site.plan()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert!(matches!(
        calls[0].input(),
        arcweft_core::plan::RuntimeProjectCallInput::Direct
    ));
    assert!(matches!(
        calls[0].outcome(),
        arcweft_core::plan::RuntimeProjectCallOutcome::Continue { .. }
    ));
    assert!(matches!(
        calls[1].input(),
        arcweft_core::plan::RuntimeProjectCallInput::Continuation { .. }
    ));
    assert!(matches!(
        calls[1].outcome(),
        arcweft_core::plan::RuntimeProjectCallOutcome::Invoke { .. }
    ));
    assert_eq!(compiled.plan.function_sites().len(), 1);
    assert!(compiled.plan.pure_helpers().is_empty());
}

#[test]
fn specialized_agent_call_materializes_source_order_before_abi_reads() {
    let compiled = compile_source(
        r#"
fn run() -> Result<Unit, AgentError>
effects { agent.act.physical }
{
    let point = viewport_point(y = 34u32, x = 12u32)
    return Ok(())
}

entry agent @entry.agent.main { controller = run }
"#,
    )
    .expect("named Agent operands compile through source-row ANF in an Agent controller");

    assert!(compiled.plan.pure_helpers().is_empty());
    let expression = compiled
        .plan
        .function_sites()
        .iter()
        .find_map(|site| {
            let RuntimeFunctionSiteBody::Executable(body) = site.body() else {
                return None;
            };
            body.ops().iter().find_map(|op| match op {
                FlowOp::Let { expr, .. } if matches!(expr.kind(), RuntimeExprKind::Let { .. }) => {
                    Some(expr)
                }
                _ => None,
            })
        })
        .expect("Agent controller function site contains the specialized call");
    let RuntimeExprKind::Let {
        binding: source_y,
        expr: first,
        body: inner,
    } = expression.kind()
    else {
        panic!("specialized call must begin with the first source-row Let")
    };
    assert_eq!(first.kind(), &RuntimeExprKind::Value(RuntimeValue::u32(34)));
    let RuntimeExprKind::Let {
        binding: source_x,
        expr: second,
        body: structural,
    } = inner.kind()
    else {
        panic!("specialized call must bind its second source operand next")
    };
    assert_eq!(
        second.kind(),
        &RuntimeExprKind::Value(RuntimeValue::u32(12))
    );
    let RuntimeExprKind::Agent(RuntimeAgentExpr::ViewportPoint { x, y }) = structural.kind() else {
        panic!("specialized body must retain the Agent structural payload")
    };
    assert!(matches!(x.kind(), RuntimeExprKind::Local(local) if local == source_x));
    assert!(matches!(y.kind(), RuntimeExprKind::Local(local) if local == source_y));
}
