use std::sync::Arc;

use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_syntax::{ast::module_path::CanonicalModulePath, parser::parse_source};

use crate::{
    env::{FunctionParam, FunctionSignature, TypeCheckEnv},
    registration::ProjectRegistrationFacts,
    test_support::character_project::{PACKAGE, project_modules, register, source_document},
    types::TypeKind,
};

use super::{
    CheckedEntryBinding, CheckedEntryCatalog, CheckedEntryDiagnostic, CheckedEntryId,
    check_project_entries,
    tests::{SOURCE, checked_project},
};

fn diagnostics(source: &str) -> Vec<CheckedEntryDiagnostic> {
    checked_project(&[("", source)]).expect_err("fixture must reject the entry binding")
}

fn diagnostics_with_read_text(source: &str) -> Vec<CheckedEntryDiagnostic> {
    let (documents, project, world) = project_modules("checked-entry-read-text", &[("", source)]);
    let facts = ProjectRegistrationFacts::try_new(world, documents, Vec::new(), Vec::new())
        .expect("entry fixture registration facts");
    let environment = TypeCheckEnv::standard()
        .with_function_signature(
            "adapter.read_text",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("path", TypeKind::String)],
            ),
        )
        .with_function_effects("adapter.read_text", ["fs.read".to_owned()]);
    let registered =
        register(&project, &facts, environment, None).expect("entry fixture semantic world");
    let typecheck =
        crate::checker::analyze_registered_project_types(&project.linked_module(), &registered);
    check_project_entries(
        &project,
        registered.symbols(),
        registered.environment().callable_catalog(),
        &typecheck,
    )
    .expect_err("fixture must reject the entry binding")
}

fn assert_code(diagnostics: &[CheckedEntryDiagnostic], code: &str) {
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code() == code),
        "expected {code}, got {diagnostics:#?}"
    );
}

fn checked_project_ignoring_syntax_errors(
    profile: &str,
    source: &str,
) -> Result<CheckedEntryCatalog, Vec<CheckedEntryDiagnostic>> {
    let document = source_document(
        &format!("arcweft-project://registration-tests/src/{profile}.arcw"),
        source,
    );
    let parsed = parse_source(source);
    assert!(
        !parsed.errors().is_empty(),
        "fixture is expected to exercise a semantic backstop after parser recovery"
    );
    let hir = lower_document_to_hir(&document, parsed.typed_tree())
        .expect("recovered typed syntax lowers to HIR");
    let project = HirProject::new(
        PACKAGE,
        [HirProjectModule::try_new(
            CanonicalModulePath::crate_root(),
            document.identity().clone(),
            hir,
        )
        .expect("fixture module")],
    )
    .expect("fixture project");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(PACKAGE).expect("package"),
        document.identity().id().clone(),
        profile,
    )
    .expect("world");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("recovered semantic world");
    let typecheck =
        crate::checker::analyze_registered_project_types(&project.linked_module(), &registered);
    check_project_entries(
        &project,
        registered.symbols(),
        registered.environment().callable_catalog(),
        &typecheck,
    )
}

fn entry(catalog: &CheckedEntryCatalog, id: &str) -> CheckedEntryBinding {
    catalog
        .get(&CheckedEntryId::try_new(id).expect("entry ID"))
        .expect("checked entry")
        .clone()
}

#[test]
fn bind_002_two_game_entries_keep_independent_role_sets() {
    let source = format!(
        "{SOURCE}
struct OtherState {{ value: i64 }}
enum OtherEvent {{ Tick }}
fn initial_other() -> OtherState
effects {{}}
{{
    initial_other()
}}
fn reduce_other(state: &OtherState, event: OtherEvent)
    -> Result<Reduction<OtherState>, ReducerError>
effects {{}}
{{
    reduce_other(state, event)
}}
flow @flow.other other(state: OtherState) {{}}
entry game @entry.game.other {{
    state = OtherState
    initializer = initial_other
    event = OtherEvent
    reducer = reduce_other
    goto @flow.other
}}
"
    );
    let catalog = checked_project(&[("", &source)]).expect("both bindings check");
    assert_eq!(catalog.len(), 2);
    let first = entry(&catalog, "entry.game.main");
    let second = entry(&catalog, "entry.game.other");
    assert_ne!(first.binding_digest(), second.binding_digest());
    assert_ne!(
        first.stateful().unwrap().state().key(),
        second.stateful().unwrap().state().key()
    );
}

#[test]
fn bind_003_game_editor_and_test_can_share_one_reducer_explicitly() {
    let source = format!(
        "{SOURCE}
entry editor @entry.editor.main {{
    state = GameState
    initializer = initial_game_state
    event = GameEvent
    reducer = reduce_game
    goto @flow.opening
}}
entry test @entry.test.main {{
    state = GameState
    initializer = initial_game_state
    event = GameEvent
    reducer = reduce_game
    goto @flow.opening
}}
"
    );
    let catalog = checked_project(&[("", &source)]).expect("shared reducer bindings check");
    let reducers = ["entry.game.main", "entry.editor.main", "entry.test.main"].map(|id| {
        entry(&catalog, id)
            .stateful()
            .unwrap()
            .reducer()
            .declaration()
            .clone()
    });
    assert!(reducers.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(
        catalog
            .entries()
            .map(CheckedEntryBinding::binding_digest)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
}

#[test]
fn bind_004_each_missing_stateful_role_has_one_stable_diagnostic() {
    for (fragment, expected) in [
        ("    state = GameState\n", "state"),
        ("    initializer = initial_game_state\n", "initializer"),
        ("    event = GameEvent\n", "event"),
        ("    reducer = reduce_game\n", "reducer"),
    ] {
        let source = SOURCE.replace(fragment, "");
        let diagnostics =
            checked_project_ignoring_syntax_errors(&format!("missing-{expected}"), &source)
                .expect_err("missing role must fail sema");
        let matching = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code() == "sema.entry.missing_role"
                    && diagnostic.message().contains(expected)
            })
            .count();
        assert_eq!(matching, 1, "{diagnostics:#?}");
    }
    let source = SOURCE.replace("    goto @flow.opening\n", "");
    let diagnostics = checked_project_ignoring_syntax_errors("missing-goto", &source)
        .expect_err("missing goto must fail sema");
    assert_code(&diagnostics, "sema.entry.goto_cardinality");
}

#[test]
fn bind_005_state_root_type_alias_is_rejected_at_the_rhs() {
    let source = SOURCE
        .replace(
            "entry game @entry.game.main",
            "type StateAlias = GameState\n\nentry game @entry.game.main",
        )
        .replace("state = GameState", "state = StateAlias");
    let diagnostics = diagnostics(&source);
    assert_code(&diagnostics, "sema.entry.alias_nominal_root");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message().contains("type alias") && !diagnostic.related().is_empty()
    }));
}

#[test]
fn bind_006_generic_state_root_is_rejected_as_open_identity() {
    let source = SOURCE
        .replace("struct GameState {", "struct GameState<T> {")
        .replace("score: i32", "score: T");
    let diagnostics = diagnostics(&source);
    assert_code(&diagnostics, "sema.entry.generic_nominal_root");
}

#[test]
fn bind_007_state_rejects_each_non_persistent_transitive_field_with_path() {
    for ty in [
        "i32 -> i32 effects { }",
        "&i32",
        "Need<i32, ArcError>",
        "ThreadHandle<i32>",
    ] {
        let source = SOURCE.replace("score: i32", &format!("score: {ty}"));
        let diagnostics = diagnostics(&source);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code() == "sema.entry.invalid_nominal_schema"
                    && diagnostic.message().contains("field `score`")
            }),
            "{ty}: {diagnostics:#?}"
        );
    }
}

#[test]
fn bind_008_event_rejects_non_replay_payload_with_variant_path() {
    let source = SOURCE.replace("    Start", "    Start(i32 -> i32 effects { })");
    let diagnostics = diagnostics(&source);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == "sema.entry.invalid_nominal_schema"
            && diagnostic.message().contains("variant `Start` payload")
    }));
}

#[test]
fn bind_009_initializer_with_parameter_is_rejected() {
    let source = SOURCE.replace(
        "fn initial_game_state() -> GameState",
        "fn initial_game_state(seed: i32) -> GameState",
    );
    assert_code(
        &diagnostics(&source),
        "sema.entry.invalid_initializer_contract",
    );
}

#[test]
fn bind_010_initializer_wrong_return_reports_expected_and_actual_types() {
    let source = SOURCE.replace(
        "fn initial_game_state() -> GameState",
        "fn initial_game_state() -> GameEvent",
    );
    let diagnostics = diagnostics(&source);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == "sema.entry.invalid_initializer_contract"
            && diagnostic.message().contains("GameState")
            && diagnostic.message().contains("GameEvent")
    }));
}

#[test]
fn bind_011_initializer_rejects_omitted_open_and_nonempty_effect_contracts() {
    let omitted = SOURCE.replace(
        "fn initial_game_state() -> GameState\neffects {}\n",
        "fn initial_game_state() -> GameState\n",
    );
    let nonempty = SOURCE.replace(
        "fn initial_game_state() -> GameState\neffects {}",
        "fn initial_game_state() -> GameState\neffects { fs.read }",
    );
    for source in [omitted, nonempty] {
        assert_code(
            &diagnostics(&source),
            "sema.entry.invalid_initializer_contract",
        );
    }
}

#[test]
fn bind_012_reducer_rejects_wrong_parameter_count_and_order() {
    let wrong_count = SOURCE.replace("state: &GameState, event: GameEvent", "state: &GameState");
    let wrong_order = SOURCE.replace(
        "state: &GameState, event: GameEvent",
        "event: GameEvent, state: &GameState",
    );
    for source in [wrong_count, wrong_order] {
        assert_code(&diagnostics(&source), "sema.entry.invalid_reducer_contract");
    }
}

#[test]
fn bind_013_reducer_requires_immutable_borrowed_state() {
    for replacement in ["state: GameState", "state: &mut GameState"] {
        let source = SOURCE.replace("state: &GameState", replacement);
        assert_code(&diagnostics(&source), "sema.entry.invalid_reducer_contract");
    }
}

#[test]
fn bind_014_reducer_requires_owned_event() {
    let source = SOURCE.replace("event: GameEvent", "event: &GameEvent");
    assert_code(&diagnostics(&source), "sema.entry.invalid_reducer_contract");
}

#[test]
fn bind_015_reducer_requires_exact_result_reduction_and_canonical_error() {
    for replacement in [
        "Result<Reduction<GameEvent>, ReducerError>",
        "Result<Reduction<GameState>, ArcError>",
        "GameState",
    ] {
        let source = SOURCE.replace("Result<Reduction<GameState>, ReducerError>", replacement);
        let diagnostics = diagnostics(&source);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code() == "sema.entry.invalid_reducer_contract"
                    && diagnostic.message().contains("return type")
            }),
            "{replacement}: {diagnostics:#?}"
        );
    }
}

#[test]
fn bind_016_reducer_rejects_declared_or_inferred_effects() {
    let declared = SOURCE.replace(
        "-> Result<Reduction<GameState>, ReducerError>\neffects {}",
        "-> Result<Reduction<GameState>, ReducerError>\neffects { fs.read }",
    );
    assert_code(
        &diagnostics(&declared),
        "sema.entry.invalid_reducer_contract",
    );
    let inferred = SOURCE.replace(
        "{\n    reduce_game(state, event)\n}\n\nflow",
        "{\n    adapter.read_text(path = \"story.arcw\")\n    reduce_game(state, event)\n}\n\nflow",
    );
    assert_code(
        &diagnostics_with_read_text(&inferred),
        "sema.entry.invalid_reducer_contract",
    );
}

#[test]
fn bind_017_unresolved_and_ambiguous_role_paths_keep_rhs_and_candidates() {
    let unresolved = SOURCE.replace(
        "initializer = initial_game_state",
        "initializer = missing_initializer",
    );
    let diagnostics = diagnostics(&unresolved);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code() == "sema.entry.unresolved_callable")
        .expect("unresolved role diagnostic");
    assert!(diagnostic.message().contains("missing_initializer"));

    let root = SOURCE
        .replace("struct GameState", "struct RootState")
        .replace("GameState", "RootState")
        .replace("state = RootState", "state = SharedState")
        .replacen(
            "struct RootState",
            "use crate.left.*\nuse crate.right.*\n\nstruct RootState",
            1,
        );
    let candidate = "pub struct SharedState { value: i32 }\n";
    let ambiguous = checked_project(&[("", &root), ("left", candidate), ("right", candidate)])
        .expect_err("glob-visible nominal candidates must remain ambiguous");
    let diagnostic = ambiguous
        .iter()
        .find(|diagnostic| diagnostic.code() == "sema.entry.ambiguous_nominal")
        .expect("ambiguous role diagnostic");
    assert!(diagnostic.related().len() >= 2, "{diagnostic:#?}");
}

#[test]
fn bind_018_agent_controller_requires_zero_args_and_exact_result() {
    let base = r"
fn smoke() -> Result<Unit, AgentError>
effects {}
{
    ()
}
entry agent @entry.agent.smoke {
    controller = smoke
}
";
    for source in [
        base.replace("smoke()", "smoke(value: i32)"),
        base.replace("Result<Unit, AgentError>", "Result<String, AgentError>"),
    ] {
        assert_code(&diagnostics(&source), "sema.entry.invalid_agent_contract");
    }
}

#[test]
fn bind_019_agent_effect_outside_declared_policy_is_rejected() {
    let source = r#"
fn smoke() -> Result<Unit, AgentError>
effects {}
{
    adapter.read_text(path = "story.arcw")
    ()
}
entry agent @entry.agent.smoke {
    controller = smoke
}
"#;
    let diagnostics = diagnostics_with_read_text(source);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == "sema.entry.invalid_agent_contract"
            && diagnostic.message().contains("fs.read")
            && diagnostic.message().contains("declared policy")
    }));
}

#[test]
fn bind_020_entry_kind_role_mismatches_have_semantic_backstops() {
    let server = r"
struct GameState { value: i32 }
entry server @entry.server.main {
    state = GameState
}
";
    let diagnostics = checked_project_ignoring_syntax_errors("server-state-role", server)
        .expect_err("server state role must fail");
    assert_code(&diagnostics, "sema.entry.incompatible_role");

    let game = SOURCE.replace(
        "    goto @flow.opening",
        "    controller = initial_game_state\n    goto @flow.opening",
    );
    let diagnostics = checked_project_ignoring_syntax_errors("game-controller-role", &game)
        .expect_err("game controller role must fail");
    assert_code(&diagnostics, "sema.entry.incompatible_role");
}

#[test]
fn bind_022_removed_controller_role_attributes_are_rejected() {
    let base = r"
fn smoke() -> Result<Unit, AgentError>
effects {}
{
    ()
}
entry agent @entry.agent.smoke {
    controller = smoke
}
";
    for marker in ["agent", "launch", "bind"] {
        let source = base.replace("fn smoke", &format!("#[{marker}]\nfn smoke"));
        assert_code(&diagnostics(&source), "sema.entry.forbidden_role_attribute");
    }
}

#[test]
fn bind_023_initial_flow_requires_one_fixed_owned_selected_state_parameter() {
    let cases = [
        SOURCE.replace("@flow.opening", "@character.opening"),
        SOURCE.replace("opening(state: GameState)", "opening()"),
        SOURCE.replace(
            "opening(state: GameState)",
            "opening(state: GameState, event: GameEvent)",
        ),
        SOURCE.replace("opening(state: GameState)", "opening(state: &GameState)"),
        SOURCE.replace("opening(state: GameState)", "opening<T>(state: GameState)"),
        SOURCE.replace(
            "opening(state: GameState)",
            "opening(state: GameState = initial_game_state())",
        ),
        SOURCE.replace("opening(state: GameState)", "opening(state: ...GameState)"),
        SOURCE.replace("opening(state: GameState)", "opening(state: GameEvent)"),
    ];
    for source in cases {
        let diagnostics = diagnostics(&source);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                matches!(
                    diagnostic.code(),
                    "sema.entry.invalid_flow_family" | "sema.entry.invalid_initial_flow_contract"
                )
            }),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn bind_024_initializer_accepts_only_ordinary_function_declarations() {
    for kind in ["task fn", "dialogue fn", "stream fn"] {
        let source = SOURCE.replace(
            "fn initial_game_state",
            &format!("{kind} initial_game_state"),
        );
        assert_code(
            &diagnostics(&source),
            "sema.entry.invalid_initializer_contract",
        );
    }

    for rhs in ["1", "|value: i32| value", "initial_game_state()"] {
        let source = SOURCE.replace(
            "initializer = initial_game_state",
            &format!("initializer = {rhs}"),
        );
        let parsed = parse_source(&source);
        assert!(
            !parsed.errors().is_empty(),
            "non-path initializer value `{rhs}` must be rejected before HIR"
        );
    }
}

#[test]
fn bind_025_event_role_rejects_alias_and_generic_nominal_roots() {
    let alias = SOURCE
        .replace(
            "entry game @entry.game.main",
            "type EventAlias = GameEvent\n\nentry game @entry.game.main",
        )
        .replace("event = GameEvent", "event = EventAlias");
    assert_code(&diagnostics(&alias), "sema.entry.alias_nominal_root");

    let generic = SOURCE
        .replace("enum GameEvent {", "enum GameEvent<T> {")
        .replace("    Start", "    Start(T)");
    assert_code(&diagnostics(&generic), "sema.entry.generic_nominal_root");
}

#[test]
fn bind_agent_budget_is_rejected_on_unselected_function() {
    let source = r"
#[budget(timeout = 1s)]
fn helper() -> Unit
effects {}
{
    ()
}
fn smoke() -> Result<Unit, AgentError>
effects {}
{
    ()
}
entry agent @entry.agent.smoke {
    controller = smoke
}
";
    assert_code(&diagnostics(source), "sema.entry.unbound_agent_budget");
}

#[test]
fn hir_entry_is_owned_and_retains_its_project_module() {
    let (_, project, _) =
        crate::test_support::character_project::root_project_source("hir-entry-owned", SOURCE);
    let entry = project
        .modules()
        .flat_map(|(_, module)| module.declarations())
        .find_map(|declaration| match declaration {
            arcweft_lang_hir::model::HirTopLevelDecl::Entry(entry) => Some(entry),
            _ => None,
        })
        .expect("HIR entry");
    assert_eq!(
        entry.module_path(),
        Some(&CanonicalModulePath::crate_root())
    );
}
