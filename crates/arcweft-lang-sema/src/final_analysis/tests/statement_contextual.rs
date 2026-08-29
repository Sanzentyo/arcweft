//! Focused production-context tests for checked Trigger and Select facts.
//!
//! These rows deliberately enter through the ordinary fixture parser/lowering
//! path and the production final analyzer.  The private statement mutation
//! seam is not used to manufacture a positive fact; it is reserved for
//! negative publication checks where authored HIR cannot express the bad
//! payload.

use std::sync::atomic::AtomicBool;

use arcweft_lang_hir::{
    expr::{HirChoicePlanItem, HirExprKind},
    identity::{LocalId, PatternId, StmtId},
    module::HirModule,
    pattern::{HirPatternBinding, HirPatternKind},
    stmt::{
        HirSelectBranchHead, HirSelectStmt, HirStatementBodyRole, HirStatementChild,
        HirStatementChildRole, HirStmtBranchPublicationKind, HirStmtEvaluationPublicationRole,
        HirStmtEvaluationStep, HirStmtKind, HirTrigger,
    },
};

use crate::{
    final_analysis::{
        CheckedAssertionDisposition, CheckedExpressionResolution, CheckedSelectBranchHead,
        CheckedSelectStatementView, CheckedStatementPayload, CheckedTriggerView,
        FinalSemanticAnalysis, FinalSemanticAnalysisControl, FinalSemanticAnalysisError,
        FinalSemanticCatalogs, PreparedStatementPayload,
    },
    types::{EntityKind, TypeKind},
};

use super::{Fixture, analyze, fixture};

const STATEFUL_DECLARATIONS: &str = r#"
struct GameState {
    score: i32
}

enum GameEvent {
    Tick
}

fn initial_game_state() -> GameState
effects {}
{
    GameState { score = 0i32 }
}

fn reduce_game(current: &GameState, event: GameEvent)
    -> Result<Reduction<GameState>, ReducerError>
effects {}
{
    Ok(Reduction.unchanged(current))
}
"#;

const GAME_ENTRY: &str = r#"
entry game @entry.game.main {
    state = GameState
    initializer = initial_game_state
    event = GameEvent
    reducer = reduce_game
    goto @flow.shared
}
"#;

fn stateful_flow_fixture(body: &str, extra: &str) -> Fixture {
    fixture(
        &format!(
            "{STATEFUL_DECLARATIONS}\n\
             flow @flow.shared shared(current: GameState) {{\n{body}\n}}\n\
             {extra}\n{GAME_ENTRY}"
        ),
        None,
    )
}

fn root_module(fixture: &Fixture) -> &HirModule {
    fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .modules()
        .next()
        .expect("root module")
        .1
}

fn find_on(fixture: &Fixture, wanted: impl Fn(&HirTrigger) -> bool) -> (StmtId, HirTrigger) {
    root_module(fixture)
        .statements()
        .find_map(|(owner, statement)| match statement.kind() {
            HirStmtKind::On { trigger, .. } if wanted(trigger) => Some((owner, trigger.clone())),
            _ => None,
        })
        .expect("fixture trigger statement")
}

fn find_select(fixture: &Fixture) -> (StmtId, HirSelectStmt) {
    root_module(fixture)
        .statements()
        .find_map(|(owner, statement)| match statement.kind() {
            HirStmtKind::Select(select) => Some((owner, select.clone())),
            _ => None,
        })
        .expect("fixture Select statement")
}

fn checked_trigger<'a>(
    report: &'a FinalSemanticAnalysis,
    owner: StmtId,
) -> &'a crate::final_analysis::CheckedTrigger {
    match report
        .statement(owner)
        .expect("checked Trigger statement")
        .payload()
    {
        CheckedStatementPayload::Trigger(trigger) => trigger,
        payload => panic!("expected Trigger payload, got {payload:?}"),
    }
}

fn checked_select<'a>(
    report: &'a FinalSemanticAnalysis,
    owner: StmtId,
) -> &'a crate::final_analysis::CheckedSelectStatement {
    match report
        .statement(owner)
        .expect("checked Select statement")
        .payload()
    {
        CheckedStatementPayload::Select(select) => select,
        payload => panic!("expected Select payload, got {payload:?}"),
    }
}

fn binding_local(module: &HirModule, pattern: PatternId) -> Option<LocalId> {
    match module
        .resolve_pattern(pattern)
        .expect("pattern owner")
        .kind()
    {
        HirPatternKind::Binding(HirPatternBinding::Bound { local, .. })
        | HirPatternKind::MutableBinding(HirPatternBinding::Bound { local, .. }) => Some(*local),
        HirPatternKind::Binding(HirPatternBinding::Recovered { .. })
        | HirPatternKind::MutableBinding(HirPatternBinding::Recovered { .. })
        | HirPatternKind::Literal(_)
        | HirPatternKind::EntityReference(_)
        | HirPatternKind::Variant(_)
        | HirPatternKind::Discard
        | HirPatternKind::Tuple { .. }
        | HirPatternKind::Record { .. }
        | HirPatternKind::BracketSequence { .. }
        | HirPatternKind::WholeBinding { .. }
        | HirPatternKind::Or { .. }
        | HirPatternKind::TypedBinding { .. }
        | HirPatternKind::Error(_) => None,
    }
}

fn assert_trigger_view(
    report: &FinalSemanticAnalysis,
    owner: StmtId,
    expected: CheckedTriggerView<'_>,
) {
    assert_eq!(checked_trigger(report, owner).view(), expected);
}

fn assert_pattern_and_local_type(
    report: &FinalSemanticAnalysis,
    module: &HirModule,
    pattern: PatternId,
    expected: &TypeKind,
) {
    assert_eq!(
        report.pattern(pattern).expect("checked pattern").ty(),
        expected
    );
    let local = binding_local(module, pattern).expect("simple binding local");
    assert_eq!(
        report.local(local).expect("checked binding local").ty(),
        expected
    );
}

fn assert_rejected_source(label: &str, source: &str) {
    let fixture = fixture(source, None);
    if fixture.project.executable_view().is_ok() {
        assert!(
            analyze(&fixture).is_err(),
            "{label} must be rejected by final analysis"
        );
    }
}

fn analyze_with_statement_mutation(
    fixture: &Fixture,
    owner: StmtId,
    replacement: PreparedStatementPayload,
) -> Result<FinalSemanticAnalysis, FinalSemanticAnalysisError> {
    let cancellation = AtomicBool::new(false);
    super::super::analyzer::analyze_final_project_with_statement_mutation_for_test(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
        owner,
        replacement,
    )
}

#[test]
fn p01_p03_p04_p05_p08_p09_p10_trigger_rows_use_exact_contextual_types() {
    let cases: &[(
        &str,
        &str,
        fn(&HirTrigger) -> bool,
        CheckedTriggerView<'static>,
    )] = &[
        (
            "P01 Input",
            "flow row { on input(input_event) => defer () }\n",
            |trigger: &HirTrigger| matches!(trigger, HirTrigger::Input(_)),
            CheckedTriggerView::Input,
        ),
        (
            "P05 Timeout",
            "flow row { on timeout(1s) => defer () }\n",
            |trigger: &HirTrigger| matches!(trigger, HirTrigger::Timeout(_)),
            CheckedTriggerView::Timeout,
        ),
        (
            "P08 Task",
            "flow row { on task(task_event) => defer () }\n",
            |trigger: &HirTrigger| matches!(trigger, HirTrigger::Task(_)),
            CheckedTriggerView::Task,
        ),
        (
            "P09 Scope",
            "flow row { on scope(scope_exit) => defer () }\n",
            |trigger: &HirTrigger| matches!(trigger, HirTrigger::Scope(_)),
            CheckedTriggerView::Scope,
        ),
        (
            "P10 Expression",
            "flow row { on true => defer () }\n",
            |trigger: &HirTrigger| matches!(trigger, HirTrigger::Expression(_)),
            CheckedTriggerView::Expression,
        ),
    ];

    for (label, source, wanted, view) in cases {
        let fixture = fixture(source, None);
        assert!(
            fixture.project.executable_view().is_ok(),
            "{label}: recovered HIR"
        );
        let report = analyze(&fixture).unwrap_or_else(|error| panic!("{label}: {error:?}"));
        let (owner, trigger) = find_on(&fixture, wanted);
        assert_trigger_view(&report, owner, *view);
        let module = root_module(&fixture);
        let pattern = match trigger {
            HirTrigger::Input(pattern) | HirTrigger::Task(pattern) | HirTrigger::Scope(pattern) => {
                pattern
            }
            HirTrigger::Timeout(expression) => {
                assert_eq!(
                    report
                        .expression(expression)
                        .expect("timeout expression")
                        .ty(),
                    &TypeKind::Duration
                );
                continue;
            }
            HirTrigger::Expression(expression) => {
                assert_eq!(
                    report
                        .expression(expression)
                        .expect("expression trigger child")
                        .ty(),
                    &TypeKind::Bool
                );
                continue;
            }
            _ => panic!("{label}: unexpected HIR trigger {trigger:?}"),
        };
        let expected = match trigger {
            HirTrigger::Input(_) => fixture.registered.environment().statement_ingress().input(),
            HirTrigger::Task(_) => fixture.registered.environment().statement_ingress().task(),
            HirTrigger::Scope(_) => fixture.registered.environment().statement_ingress().scope(),
            _ => unreachable!(),
        };
        assert_pattern_and_local_type(&report, module, pattern, expected);
        let edges = match report
            .statement(owner)
            .expect("checked statement")
            .payload()
        {
            CheckedStatementPayload::Trigger(_) => module
                .resolve_stmt(owner)
                .expect("HIR statement")
                .kind()
                .try_child_edges()
                .expect("trigger child edges"),
            _ => unreachable!(),
        };
        assert!(edges.iter().any(|edge| {
            edge.child() == HirStatementChild::Pattern(pattern)
                && edge.role() == HirStatementChildRole::TriggerPattern
        }));
    }

    let signal_fixture = fixture(
        "signal ready: bool\nflow row {\n    on signal(@signal.ready) => defer ()\n    on signal(@signal.ready, value) => defer ()\n}\n",
        None,
    );
    let signal_report = analyze(&signal_fixture).expect("P03/P04 Signal rows");
    let signal_module = root_module(&signal_fixture);
    let signal_rows = signal_module
        .statements()
        .filter_map(|(owner, statement)| match statement.kind() {
            HirStmtKind::On {
                trigger: HirTrigger::Signal { target, value },
                ..
            } => Some((owner, *target, *value)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(signal_rows.len(), 2);
    for (owner, target, value) in signal_rows {
        assert_trigger_view(&signal_report, owner, CheckedTriggerView::Signal);
        let target_type = signal_report
            .expression(target)
            .expect("Signal target expression")
            .ty();
        assert!(
            matches!(target_type, TypeKind::Ref(entity) if entity.kind() == &EntityKind::Signal && entity.value() == Some(&TypeKind::Bool))
        );
        let edges = signal_module
            .resolve_stmt(owner)
            .expect("Signal HIR statement")
            .kind()
            .try_child_edges()
            .expect("Signal child edges");
        assert!(edges.iter().any(|edge| {
            edge.child() == HirStatementChild::Expression(target)
                && edge.role() == HirStatementChildRole::TriggerSignalTarget
        }));
        match value {
            None => assert!(
                !edges
                    .iter()
                    .any(|edge| edge.role() == HirStatementChildRole::TriggerSignalValue)
            ),
            Some(value) => {
                assert_pattern_and_local_type(
                    &signal_report,
                    signal_module,
                    value,
                    &TypeKind::Bool,
                );
                assert!(edges.iter().any(|edge| {
                    edge.child() == HirStatementChild::Pattern(value)
                        && edge.role() == HirStatementChildRole::TriggerSignalValue
                }));
            }
        }
    }
}

#[test]
fn p02_event_trigger_uses_the_reachable_stateful_entry_event_type() {
    let fixture = stateful_flow_fixture("    on event(event_value) => defer ()", "");
    let report = analyze(&fixture).expect("P02 Event trigger");
    let module = root_module(&fixture);
    let (owner, HirTrigger::Event(pattern)) =
        find_on(&fixture, |trigger| matches!(trigger, HirTrigger::Event(_)))
    else {
        panic!("stateful Event trigger");
    };
    assert_trigger_view(&report, owner, CheckedTriggerView::Event);
    let expected_digest = report
        .checked_entries()
        .entries()
        .filter_map(crate::entry::CheckedEntryBinding::stateful)
        .map(|entry| entry.event().semantic_type())
        .collect::<Vec<_>>();
    assert_eq!(expected_digest.len(), 1);
    let pattern_type = report.pattern(pattern).expect("Event pattern").ty();
    assert_eq!(pattern_type.semantic_identity_digest(), expected_digest[0]);
    assert_pattern_and_local_type(&report, module, pattern, pattern_type);
}

#[test]
fn p06_mark_trigger_has_no_scrutinee_child_and_publishes_checked_view() {
    let fixture = fixture(
        r#"
pub character @character.alice Alice as alice {}
flow main() -> String {
    alice[before [mark @.checkpoint] after] with {
        on mark(@.checkpoint) => return "done"
    }
    return "done"
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("P06 mark trigger");
    let module = root_module(&fixture);
    let (owner, trigger) = find_on(&fixture, |trigger| matches!(trigger, HirTrigger::Mark(_)));
    assert!(matches!(
        checked_trigger(&report, owner).view(),
        CheckedTriggerView::Mark(_)
    ));
    assert!(matches!(trigger, HirTrigger::Mark(_)));
    let mark_edges = module
        .resolve_stmt(owner)
        .expect("mark HIR statement")
        .kind()
        .try_child_edges()
        .expect("mark child edges");
    assert!(mark_edges.iter().all(|edge| {
        !matches!(
            edge.role(),
            HirStatementChildRole::TriggerPattern
                | HirStatementChildRole::TriggerSignalTarget
                | HirStatementChildRole::TriggerSignalValue
                | HirStatementChildRole::TriggerExpression
        )
    }));
}

#[test]
fn p07_select_trigger_requires_choice_lifecycle_and_choice_option_pattern() {
    let fixture = fixture(
        r#"
flow @flow.done done {}
flow row() {
    choice @choice.opening {
        @.listen "Listen" -> @flow.done
    } with {
        on select outer {
            on select(inner) => defer ()
        }
    }
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("P07 choice lifecycle Select trigger");
    let module = root_module(&fixture);
    let (outer_pattern, outer_locals) = module
        .expressions()
        .find_map(|(_, expression)| match expression.kind() {
            HirExprKind::Choice(choice) => choice.plan().and_then(|plan| {
                plan.items().iter().find_map(|item| match item {
                    HirChoicePlanItem::OnSelect {
                        pattern, locals, ..
                    } => Some((*pattern, locals.as_ref())),
                    HirChoicePlanItem::Assignment { .. }
                    | HirChoicePlanItem::Timeout { .. }
                    | HirChoicePlanItem::Cancel { .. }
                    | HirChoicePlanItem::Error(_) => None,
                })
            }),
            _ => None,
        })
        .expect("Choice plan Select row");
    let expected = TypeKind::entity_ref(EntityKind::ChoiceOption);
    assert_pattern_and_local_type(&report, module, outer_pattern, &expected);
    assert_eq!(outer_locals.len(), 1, "Choice plan Select local");
    assert_eq!(
        report
            .local(outer_locals[0])
            .expect("Choice plan Select local")
            .ty(),
        &expected
    );
    let (owner, HirTrigger::Select(pattern)) =
        find_on(&fixture, |trigger| matches!(trigger, HirTrigger::Select(_)))
    else {
        panic!("choice Select trigger");
    };
    assert_trigger_view(&report, owner, CheckedTriggerView::Select);
    assert_pattern_and_local_type(&report, module, pattern, &expected);
}

#[test]
fn p16_select_operand_retains_one_typed_operand_child() {
    let fixture = fixture("flow row { select true }\n", None);
    let report = analyze(&fixture).expect("P16 Select operand");
    let module = root_module(&fixture);
    let (owner, HirSelectStmt::Operand(expression)) = find_select(&fixture) else {
        panic!("Select operand fixture");
    };
    assert_eq!(
        checked_select(&report, owner).view(),
        CheckedSelectStatementView::Operand
    );
    assert!(report.expression(expression).is_some());
    let edges = module
        .resolve_stmt(owner)
        .expect("Select HIR statement")
        .kind()
        .try_child_edges()
        .expect("Select child edges");
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge.role() == HirStatementChildRole::SelectOperand)
            .count(),
        1
    );
    assert_eq!(edges[0].child(), HirStatementChild::Expression(expression));
}

#[test]
fn p17_select_branches_retain_source_order_and_checked_branch_evidence() {
    let fixture = stateful_flow_fixture(
        r#"    select {
        value = true => { defer () }
        frame frame_value => { defer () }
        event event_value => { defer () }
    }"#,
        "",
    );
    let report = analyze(&fixture).expect("P17 Select branches");
    let module = root_module(&fixture);
    let (owner, HirSelectStmt::Branches { branches, .. }) = find_select(&fixture) else {
        panic!("Select branch fixture");
    };
    let CheckedSelectStatementView::Branches(heads) = checked_select(&report, owner).view() else {
        panic!("checked Select branch view");
    };
    assert_eq!(
        heads,
        [
            CheckedSelectBranchHead::Bind,
            CheckedSelectBranchHead::Frame,
            CheckedSelectBranchHead::Event,
        ]
    );
    assert_eq!(branches.len(), 3);

    let mut direct_roles = module
        .resolve_stmt(owner)
        .expect("Select HIR statement")
        .kind()
        .try_child_edges()
        .expect("Select child edges")
        .into_iter()
        .filter_map(|edge| match edge.role() {
            HirStatementChildRole::SelectBinding { branch }
            | HirStatementChildRole::SelectSource { branch }
            | HirStatementChildRole::SelectPattern { branch } => Some((branch, edge.child())),
            _ => None,
        })
        .collect::<Vec<_>>();
    direct_roles.sort_by_key(|(branch, _)| *branch);
    assert_eq!(direct_roles.len(), 4);
    assert!(
        direct_roles
            .iter()
            .any(|(_, child)| matches!(child, HirStatementChild::Local(_)))
    );

    let body_roles = module
        .resolve_stmt(owner)
        .expect("Select HIR statement")
        .kind()
        .body_projections()
        .expect("Select body projections")
        .into_iter()
        .map(|projection| *projection.role())
        .collect::<Vec<_>>();
    assert_eq!(
        body_roles,
        [
            HirStatementBodyRole::SelectBranch { branch: 0 },
            HirStatementBodyRole::SelectBranch { branch: 1 },
            HirStatementBodyRole::SelectBranch { branch: 2 },
        ]
    );

    let mut steps = Vec::new();
    module
        .resolve_stmt(owner)
        .expect("Select HIR statement")
        .kind()
        .evaluation_plan()
        .try_visit_evaluation_steps(|step| steps.push(step))
        .expect("Select evaluation plan");
    assert!(steps.iter().any(|step| {
        matches!(
            step,
            HirStmtEvaluationStep::Expression {
                role: HirStatementChildRole::SelectSource { branch: 0 },
                ..
            }
        )
    }));
    assert!(steps.iter().any(|step| {
        matches!(
            step,
            HirStmtEvaluationStep::Local {
                role: HirStatementChildRole::SelectBinding { branch: 0 },
                ..
            }
        )
    }));
    assert!(steps.iter().any(|step| {
        matches!(
            step,
            HirStmtEvaluationStep::Pattern {
                role: HirStatementChildRole::SelectPattern { branch: 1 | 2 },
                ..
            }
        )
    }));
    assert!(steps.iter().any(|step| {
        matches!(
            step,
            HirStmtEvaluationStep::Publication {
                role: HirStmtEvaluationPublicationRole::Branch {
                    kind: HirStmtBranchPublicationKind::SelectBranch { branch: 2 },
                },
                ..
            }
        )
    }));

    let expected_event = report
        .checked_entries()
        .entries()
        .filter_map(crate::entry::CheckedEntryBinding::stateful)
        .next()
        .expect("stateful Entry")
        .event()
        .semantic_type();
    for (index, branch) in branches.iter().enumerate() {
        match branch.head() {
            HirSelectBranchHead::Bind { binding, source } => {
                let local = binding.resolved().expect("Bind local");
                assert_eq!(
                    report.expression(*source).expect("Bind source").ty(),
                    &TypeKind::Bool
                );
                assert_eq!(
                    report.local(local).expect("Bind checked local").ty(),
                    &TypeKind::Bool
                );
            }
            HirSelectBranchHead::Frame { pattern, locals } => {
                let expected = fixture.registered.environment().statement_ingress().frame();
                assert_pattern_and_local_type(&report, module, *pattern, expected);
                assert_eq!(locals.len(), 1);
                assert_eq!(report.local(locals[0]).expect("Frame local").ty(), expected);
            }
            HirSelectBranchHead::Event { pattern, locals } => {
                let pattern_type = report.pattern(*pattern).expect("Event pattern").ty();
                assert_eq!(pattern_type.semantic_identity_digest(), expected_event);
                assert_eq!(locals.len(), 1);
                assert_eq!(
                    report.local(locals[0]).expect("Event local").ty(),
                    pattern_type
                );
            }
            HirSelectBranchHead::Recovered => panic!("branch {index} recovered"),
        }
    }
}

#[test]
fn p18_prefix_try_is_checked_as_the_bind_source_child_only() {
    let fixture = fixture(
        r#"
flow row(source: Result<i64, String>) -> Result<i64, String> {
    select {
        value = try source => {}
    }
    return source
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("P18 Select Bind prefix Try");
    let module = root_module(&fixture);
    let (owner, HirSelectStmt::Branches { branches, .. }) = find_select(&fixture) else {
        panic!("Try Select fixture");
    };
    assert_eq!(branches.len(), 1, "one Try Select branch");
    let branch = branches.first().expect("one Try Select branch");
    let HirSelectBranchHead::Bind { source, binding } = branch.head() else {
        panic!("Try Select branch Bind");
    };
    assert!(matches!(
        module
            .resolve_expr(*source)
            .expect("Try source expression")
            .kind(),
        HirExprKind::Try(_)
    ));
    assert!(matches!(
        report
            .expression(*source)
            .expect("checked Try source")
            .resolution(),
        CheckedExpressionResolution::Try(_)
    ));
    let local = binding.resolved().expect("Try binding local");
    assert_eq!(
        report.local(local).expect("checked Try binding local").ty(),
        report.expression(*source).expect("checked Try source").ty()
    );
    assert!(matches!(
        checked_select(&report, owner).view(),
        CheckedSelectStatementView::Branches([CheckedSelectBranchHead::Bind])
    ));
    let edges = module
        .resolve_stmt(owner)
        .expect("Try Select HIR statement")
        .kind()
        .try_child_edges()
        .expect("Try Select child edges");
    assert!(edges.iter().any(|edge| {
        edge.child() == HirStatementChild::Expression(*source)
            && edge.role() == HirStatementChildRole::SelectSource { branch: 0 }
    }));
    assert!(
        !edges
            .iter()
            .any(|edge| edge.role() == HirStatementChildRole::SelectPattern { branch: 0 })
    );
}

#[test]
fn n04_recovered_trigger_and_select_heads_never_reach_checked_construction() {
    assert_rejected_source(
        "N04 recovered Trigger",
        "flow row { on mark(.missing) => defer () }\n",
    );
    assert_rejected_source(
        "N04 recovered Select head",
        "flow row { select { unknown head => {} } }\n",
    );
}

#[test]
fn n05_n06_wrong_select_statement_family_or_branch_roles_never_publish() {
    let fixture = fixture("flow row { select { value = true => {} } }\n", None);
    let report = analyze(&fixture).expect("baseline Select statement");
    let (owner, _) = find_select(&fixture);
    assert!(
        analyze_with_statement_mutation(
            &fixture,
            owner,
            PreparedStatementPayload::Assertion(CheckedAssertionDisposition::Discharged),
        )
        .is_err()
    );
    assert!(report.statement(owner).is_some());

    assert_rejected_source(
        "N06 Frame/Event role type swap",
        &format!(
            "{STATEFUL_DECLARATIONS}\n\
             flow @flow.shared shared(current: GameState) {{\n\
                 select {{ frame true => {{}} event true => {{}} }}\n\
             }}\n{GAME_ENTRY}"
        ),
    );
}

#[test]
fn n08_zero_or_conflicting_reachable_entry_event_types_reject_contextual_events() {
    assert_rejected_source(
        "N08 zero reachable stateful Entry events",
        "flow row { on event(event_value) => defer () }\n",
    );

    assert_rejected_source(
        "N08 incompatible reachable Entry event schemas",
        &format!(
            "{STATEFUL_DECLARATIONS}\n\
             enum OtherEvent {{ Other }}\n\
             fn reduce_other(current: &GameState, event: OtherEvent)\n\
                 -> Result<Reduction<GameState>, ReducerError>\n\
             effects {{}} {{ Ok(Reduction.unchanged(current)) }}\n\
             flow @flow.shared shared(current: GameState) {{\n\
                 on event(event_value) => defer ()\n\
             }}\n\
             {GAME_ENTRY}\n\
             entry editor @entry.editor.main {{\n\
                 state = GameState\n\
                 initializer = initial_game_state\n\
                 event = OtherEvent\n\
                 reducer = reduce_other\n\
                 goto @flow.shared\n\
             }}"
        ),
    );
}

#[test]
fn n09_wrong_input_task_scope_frame_and_event_patterns_are_rejected() {
    for (label, source) in [
        (
            "N09 Input pattern",
            "flow row { on input(true) => defer () }\n",
        ),
        (
            "N09 Task pattern",
            "flow row { on task(true) => defer () }\n",
        ),
        (
            "N09 Scope pattern",
            "flow row { on scope(true) => defer () }\n",
        ),
    ] {
        assert_rejected_source(label, source);
    }
    assert_rejected_source(
        "N09 Select Frame/Event patterns",
        &format!(
            "{STATEFUL_DECLARATIONS}\n\
             flow @flow.shared shared(current: GameState) {{\n\
                 select {{ frame true => {{}} event true => {{}} }}\n\
             }}\n{GAME_ENTRY}"
        ),
    );
}

#[test]
fn n10_signal_target_and_payload_must_be_signal_with_exact_value_type() {
    assert_rejected_source(
        "N10 non-Signal target",
        "flow row { on signal(true) => defer () }\n",
    );
    assert_rejected_source(
        "N10 wrong Signal payload pattern",
        "signal ready: bool\nflow row { on signal(@signal.ready, \"wrong\") => defer () }\n",
    );
}

#[test]
fn n11_timeout_and_expression_triggers_require_duration_and_bool_children() {
    assert_rejected_source(
        "N11 non-Duration Timeout",
        "flow row { on timeout(true) => defer () }\n",
    );
    assert_rejected_source(
        "N11 non-Bool Expression",
        "flow row { on 1i64 => defer () }\n",
    );
}

#[test]
fn n12_select_trigger_without_one_choice_lifecycle_is_rejected() {
    let zero = fixture("flow row { on select(_) => defer () }\n", None);
    zero.project
        .executable_view()
        .expect("N12 zero-lifecycle Trigger remains executable HIR");
    let (owner, trigger) = find_on(&zero, |trigger| matches!(trigger, HirTrigger::Select(_)));
    assert!(matches!(trigger, HirTrigger::Select(_)));
    assert!(
        analyze(&zero).is_err(),
        "N12 zero Choice lifecycle must reject {owner:?}"
    );
}
