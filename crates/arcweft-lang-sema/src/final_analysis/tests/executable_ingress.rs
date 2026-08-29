use std::sync::atomic::AtomicBool;

use arcweft_lang_hir::{
    stmt::{HirStmtKind, HirTrigger},
    symbol::{CallableDeclarationKey, ProjectSymbolRevision, ProjectSymbolWorldId},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use crate::{
    callable::{CheckedCallableContext, CheckedCallableId},
    final_analysis::{
        CheckedStatementPayload, CheckedTriggerView, FinalSemanticAnalysis,
        FinalSemanticAnalysisControl, FinalSemanticAnalysisError, FinalSemanticCatalogs,
        FinalSemanticProjectError,
    },
    types::SemanticTypeDigest,
};

use super::{Fixture, analyze, fixture};
use crate::final_analysis::analyzer::FinalAuthorityMutationForTest;

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

const EDITOR_ENTRY: &str = r#"
entry editor @entry.editor.main {
    state = GameState
    initializer = initial_game_state
    event = GameEvent
    reducer = reduce_game
    goto @flow.shared
}
"#;

fn shared_event_fixture(reverse_entries: bool) -> Fixture {
    let entries = if reverse_entries {
        format!("{EDITOR_ENTRY}{GAME_ENTRY}")
    } else {
        format!("{GAME_ENTRY}{EDITOR_ENTRY}")
    };
    fixture(
        &format!(
            "{STATEFUL_DECLARATIONS}\n\
             flow @flow.shared shared(current: GameState) {{\n\
                 on event(event) => defer ()\n\
             }}\n\
             {entries}"
        ),
        None,
    )
}

fn one_entry_fixture(flow_body: &str, extra_flows: &str) -> Fixture {
    fixture(
        &format!(
            "{STATEFUL_DECLARATIONS}\n\
             flow @flow.shared shared(current: GameState) {{\n{flow_body}\n}}\n\
             {extra_flows}\n\
             {GAME_ENTRY}"
        ),
        None,
    )
}

fn event_statement(fixture: &Fixture) -> arcweft_lang_hir::identity::StmtId {
    fixture
        .project
        .executable_view()
        .expect("executable stateful Entry fixture")
        .modules()
        .flat_map(|(_, module)| module.statements())
        .find_map(|(owner, statement)| {
            matches!(
                statement.kind(),
                HirStmtKind::On {
                    trigger: HirTrigger::Event(_),
                    ..
                }
            )
            .then_some(owner)
        })
        .expect("fixture Event trigger statement")
}

fn flow_declaration(fixture: &Fixture, public_id: &str) -> CallableDeclarationKey {
    fixture
        .symbols
        .callable_symbols()
        .find_map(|symbol| match symbol.declaration() {
            CallableDeclarationKey::Flow(flow) if flow.public_id().as_str() == public_id => {
                Some(symbol.declaration().clone())
            }
            CallableDeclarationKey::Existing(_) => None,
            CallableDeclarationKey::Flow(_) => None,
            CallableDeclarationKey::TraitRequirement(_) | CallableDeclarationKey::ImplMethod(_) => {
                None
            }
        })
        .unwrap_or_else(|| panic!("missing Flow declaration `{public_id}`"))
}

fn analyze_with_mutation(
    fixture: &Fixture,
    mutation: FinalAuthorityMutationForTest,
) -> Result<FinalSemanticAnalysis, FinalSemanticProjectError> {
    let cancellation = AtomicBool::new(false);
    crate::final_analysis::analyzer::analyze_final_project_with_authority_mutation_for_test(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
        mutation,
    )
}

fn assert_entry_diagnostic(
    result: Result<FinalSemanticAnalysis, FinalSemanticProjectError>,
    expected: &'static str,
) {
    let Err(FinalSemanticProjectError::Entry(diagnostics)) = result else {
        panic!("mutation must fail in the late Entry seal")
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code() == expected),
        "expected `{expected}`, got {diagnostics:#?}"
    );
}

fn assert_checked_catalog_rejection(
    result: Result<FinalSemanticAnalysis, FinalSemanticProjectError>,
) {
    assert!(matches!(
        result,
        Err(FinalSemanticProjectError::Semantic(error))
            if matches!(*error, FinalSemanticAnalysisError::CheckedCallableCatalog)
    ));
}

fn rejected_checked_id(
    report: &FinalSemanticAnalysis,
    declaration: &CallableDeclarationKey,
    kind: RejectedCheckedIdKind,
) -> CheckedCallableId {
    let checked = report
        .checked_callables()
        .project_callable(declaration)
        .expect("baseline checked Flow row");
    let CheckedCallableContext::Project {
        world,
        revision,
        catalog,
        standard,
    } = checked.id().context()
    else {
        panic!("source Flow must own a project checked identity")
    };
    let (world, revision) = match kind {
        RejectedCheckedIdKind::ForeignWorld => (
            ProjectSymbolWorldId::try_new(
                declaration.package().clone(),
                SourceDocumentId::try_new("arcweft-test://sema/final/foreign-root")
                    .expect("foreign document ID"),
                "foreign",
            )
            .expect("foreign symbol world"),
            *revision,
        ),
        RejectedCheckedIdKind::StaleRevision => {
            let document = SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://sema/final/stale-revision")
                    .expect("stale document ID"),
                SourceName::path("stale.arcw"),
                "flow @flow.stale stale {}\n",
            )
            .expect("stale revision source");
            let stale = ProjectSymbolRevision::try_for_documents([document.identity()])
                .expect("stale symbol revision");
            assert_ne!(&stale, revision);
            (world.clone(), stale)
        }
    };
    CheckedCallableId::for_project(world, revision, *catalog, *standard, declaration.clone())
        .expect("well-formed rejected checked identity")
}

#[derive(Clone, Copy)]
enum RejectedCheckedIdKind {
    ForeignWorld,
    StaleRevision,
}

#[test]
fn p21_two_equal_entry_roots_converge_at_one_event_statement_in_either_source_order() {
    for reverse in [false, true] {
        let fixture = shared_event_fixture(reverse);
        let report = analyze(&fixture).expect("equal Event Entry roots converge");
        let stateful = report
            .checked_entries()
            .entries()
            .filter_map(crate::entry::CheckedEntryBinding::stateful)
            .collect::<Vec<_>>();
        assert_eq!(stateful.len(), 2);
        assert_eq!(
            stateful[0].event().semantic_type(),
            stateful[1].event().semantic_type()
        );
        let owner = event_statement(&fixture);
        let CheckedStatementPayload::Trigger(trigger) = report
            .statement(owner)
            .expect("checked shared Event statement")
            .payload()
        else {
            panic!("shared statement must retain a Trigger payload")
        };
        assert!(matches!(trigger.view(), CheckedTriggerView::Event));
    }
}

#[test]
fn p22_recursive_call_scc_and_include_edge_terminate_with_complete_event_reachability() {
    let fixture = one_entry_fixture(
        "    cycle_a()\n    include @flow.branch",
        r#"
fn cycle_a() effects {} { cycle_b() }
fn cycle_b() effects {} { cycle_a() }

flow @flow.branch branch {
    on event(event) => defer ()
}
"#,
    );
    let report = analyze(&fixture).expect("recursive executable ingress reaches Include target");
    assert!(
        report.statements().any(|(_, statement)| matches!(
            statement.payload(),
            CheckedStatementPayload::Include(_)
        ))
    );
    let owner = event_statement(&fixture);
    assert!(matches!(
        report.statement(owner).map(|statement| statement.payload()),
        Some(CheckedStatementPayload::Trigger(trigger))
            if matches!(trigger.view(), CheckedTriggerView::Event)
    ));
}

#[test]
fn n23_mutated_event_digest_is_rejected_by_the_consuming_entry_seal() {
    let fixture = one_entry_fixture("    on event(event) => defer ()", "");
    analyze(&fixture).expect("baseline Event ingress transaction");
    assert_entry_diagnostic(
        analyze_with_mutation(
            &fixture,
            FinalAuthorityMutationForTest::EventDigest {
                statement: event_statement(&fixture),
                replacement: SemanticTypeDigest::from_bytes([0xD3; 32]),
            },
        ),
        "sema.entry.event_ingress_mismatch",
    );
}

#[test]
fn initial_flow_late_join_rejects_missing_foreign_and_stale_checked_rows() {
    let fixture = one_entry_fixture("", "");
    let report = analyze(&fixture).expect("baseline stateful Entry publication");
    let declaration = flow_declaration(&fixture, "flow.shared");

    assert_entry_diagnostic(
        analyze_with_mutation(
            &fixture,
            FinalAuthorityMutationForTest::MissingCheckedCallable {
                declaration: declaration.clone(),
            },
        ),
        "sema.entry.initial_flow_not_checked",
    );
    for kind in [
        RejectedCheckedIdKind::ForeignWorld,
        RejectedCheckedIdKind::StaleRevision,
    ] {
        assert_entry_diagnostic(
            analyze_with_mutation(
                &fixture,
                FinalAuthorityMutationForTest::SubstituteCheckedCallable {
                    declaration: declaration.clone(),
                    replacement: rejected_checked_id(&report, &declaration, kind),
                },
            ),
            "sema.entry.initial_flow_not_checked",
        );
    }
}

#[test]
fn include_late_join_rejects_missing_foreign_and_stale_checked_rows() {
    let fixture = fixture(
        concat!(
            "flow @flow.root root { include @flow.child }\n",
            "flow @flow.child child {}\n",
        ),
        None,
    );
    let report = analyze(&fixture).expect("baseline Include publication");
    let declaration = flow_declaration(&fixture, "flow.child");

    assert_checked_catalog_rejection(analyze_with_mutation(
        &fixture,
        FinalAuthorityMutationForTest::MissingCheckedCallable {
            declaration: declaration.clone(),
        },
    ));
    for kind in [
        RejectedCheckedIdKind::ForeignWorld,
        RejectedCheckedIdKind::StaleRevision,
    ] {
        assert_checked_catalog_rejection(analyze_with_mutation(
            &fixture,
            FinalAuthorityMutationForTest::SubstituteCheckedCallable {
                declaration: declaration.clone(),
                replacement: rejected_checked_id(&report, &declaration, kind),
            },
        ));
    }
}
