use crate::{diagnostics::TypeCheckErrorKind, propagation::PropagationBoundaryKind};
use crate::{
    effect_model::CallableId,
    entry::{CheckedEntryBinding, CheckedEntryId},
    env::{FunctionParam, FunctionSignature, TypeCheckEnv},
    registration::ProjectRegistrationFacts,
    test_support::character_project::{project_modules, register, root_project_source},
    types::TypeKind,
};
use arcweft_lang_hir::symbol::CallableDeclarationId;

use super::check_project_entries;

pub(super) const SOURCE: &str = r"
struct GameState {
    score: i32
}

enum GameEvent {
    Start
}

fn initial_game_state() -> GameState
effects {}
{
    initial_game_state()
}

fn reduce_game(state: &GameState, event: GameEvent)
    -> Result<Reduction<GameState>, ReducerError>
effects {}
{
    reduce_game(state, event)
}

flow @flow.opening opening(state: GameState) {
}

entry game @entry.game.main {
    state = GameState
    initializer = initial_game_state
    event = GameEvent
    reducer = reduce_game
    goto @flow.opening
}
";

pub(super) fn checked(source: &str) -> CheckedEntryBinding {
    let (document, project, world) = root_project_source("checked-entry", source);
    let facts = ProjectRegistrationFacts::try_new(world, vec![document], Vec::new(), Vec::new())
        .expect("entry fixture registration facts");
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("entry fixture semantic world");
    let typecheck =
        crate::checker::analyze_registered_project_types(&project.linked_module(), &registered);
    let catalog = check_project_entries(
        &project,
        registered.symbols(),
        registered.environment().callable_catalog(),
        &typecheck,
    )
    .expect("entry fixture checks");
    catalog
        .get(&CheckedEntryId::try_new("entry.game.main").unwrap())
        .expect("checked entry")
        .clone()
}

pub(super) fn checked_project(
    sources: &[(&str, &str)],
) -> Result<super::CheckedEntryCatalog, Vec<super::CheckedEntryDiagnostic>> {
    let (documents, project, world) = project_modules("checked-entry-project", sources);
    let facts = ProjectRegistrationFacts::try_new(world, documents, Vec::new(), Vec::new())
        .expect("entry fixture registration facts");
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("entry fixture semantic world");
    let typecheck =
        crate::checker::analyze_registered_project_types(&project.linked_module(), &registered);
    check_project_entries(
        &project,
        registered.symbols(),
        registered.environment().callable_catalog(),
        &typecheck,
    )
}

fn checked_agent(source: &str) -> CheckedEntryBinding {
    let (document, project, world) = root_project_source("checked-agent-entry", source);
    let facts = ProjectRegistrationFacts::try_new(world, vec![document], Vec::new(), Vec::new())
        .expect("Agent entry registration facts");
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("Agent entry semantic world");
    let typecheck =
        crate::checker::analyze_registered_project_types(&project.linked_module(), &registered);
    let catalog = check_project_entries(
        &project,
        registered.symbols(),
        registered.environment().callable_catalog(),
        &typecheck,
    )
    .expect("Agent entry checks");
    catalog
        .get(&CheckedEntryId::try_new("entry.agent.smoke").unwrap())
        .expect("checked Agent entry")
        .clone()
}

fn assert_unbound_agent_call(
    diagnostics: &[super::CheckedEntryDiagnostic],
    source: &str,
    call: &str,
) {
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code() == "sema.entry.unbound_agent_intrinsic")
        .expect("typed Agent role diagnostic");
    let range = diagnostic.primary().range();
    assert_eq!(&source[range.start()..range.end()], call);
}

#[test]
fn bind_001_resolves_stateful_roles_to_original_declarations() {
    let entry = checked(SOURCE);
    let stateful = entry.stateful().expect("stateful binding");

    assert_eq!(entry.kind().as_str(), "game");
    assert_eq!(stateful.state().key().name(), "GameState");
    assert_eq!(stateful.event().key().name(), "GameEvent");
    assert_eq!(
        stateful.initializer().declaration().name(),
        "initial_game_state"
    );
    assert_eq!(stateful.reducer().declaration().name(), "reduce_game");
    assert_eq!(
        stateful.initial_flow().id().public_id().as_str(),
        "flow.opening"
    );
}

#[test]
fn reducer_body_only_change_preserves_binding_digest() {
    let first = checked(SOURCE);
    let second = checked(&SOURCE.replace(
        "{\n    reduce_game(state, event)\n}\n\nflow",
        "{\n    let marker = 1\n    reduce_game(state, event)\n}\n\nflow",
    ));

    assert_eq!(first.binding_digest(), second.binding_digest());
    assert_eq!(
        first.stateful().unwrap().reducer().contract_digest(),
        second.stateful().unwrap().reducer().contract_digest()
    );
}

#[test]
fn schema_change_changes_schema_and_binding_digests() {
    let first = checked(SOURCE);
    let second = checked(&SOURCE.replace("score: i32", "score: i64"));
    let first = first.stateful().unwrap();
    let second = second.stateful().unwrap();

    assert_ne!(
        first.state().schema_digest(),
        second.state().schema_digest()
    );
    assert_ne!(first.binding_digest, second.binding_digest);
}

#[test]
fn flow_id_change_changes_flow_contract_and_binding_digests() {
    let first = checked(SOURCE);
    let second = checked(&SOURCE.replace("flow.opening", "flow.alternate"));
    let first = first.stateful().unwrap();
    let second = second.stateful().unwrap();

    assert_ne!(
        first.initial_flow().contract_digest(),
        second.initial_flow().contract_digest()
    );
    assert_ne!(first.binding_digest, second.binding_digest);
}

#[test]
fn bind_021_duplicate_entry_ids_across_modules_are_rejected() {
    let (documents, project, world) = project_modules(
        "duplicate-entry",
        &[("", SOURCE), ("other", "entry cli @entry.game.main { }\n")],
    );
    let facts = ProjectRegistrationFacts::try_new(world, documents, Vec::new(), Vec::new())
        .expect("entry fixture registration facts");
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("entry fixture semantic world");
    let typecheck =
        crate::checker::analyze_registered_project_types(&project.linked_module(), &registered);
    let diagnostics = check_project_entries(
        &project,
        registered.symbols(),
        registered.environment().callable_catalog(),
        &typecheck,
    )
    .expect_err("duplicate canonical entry IDs must fail");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code() == "sema.entry.duplicate_id")
    );
}

#[test]
fn nominal_schema_is_invariant_under_qualified_alias_and_import_spelling() {
    let stateful = |prefix: &str, field_type: &str| {
        format!(
            "{prefix}{}",
            SOURCE.replace(
                "struct GameState {\n    score: i32\n}",
                &format!("struct GameState {{\n    shared: {field_type}\n}}"),
            )
        )
    };
    let shared = "pub struct SharedState { value: i32 }\n";
    let qualified = stateful("", "crate.shared.SharedState");
    let alias = stateful(
        "type SharedAlias = crate.shared.SharedState\n",
        "SharedAlias",
    );
    let imported = stateful("use crate.shared.SharedState\n", "SharedState");

    let digest = |source: &str| {
        let catalog = checked_project(&[("", source), ("shared", shared)])
            .expect("canonical nominal spelling checks");
        *catalog
            .get(&CheckedEntryId::try_new("entry.game.main").unwrap())
            .unwrap()
            .stateful()
            .unwrap()
            .state()
            .schema_digest()
    };

    assert_eq!(digest(&qualified), digest(&alias));
    assert_eq!(digest(&qualified), digest(&imported));
}

#[test]
fn cyclic_alias_in_nominal_schema_is_rejected() {
    let source = format!(
        "type First = Second\ntype Second = First\n{}",
        SOURCE.replace(
            "struct GameState {\n    score: i32\n}",
            "struct GameState { value: First }",
        )
    );
    let diagnostics = checked_project(&[("", &source)])
        .expect_err("cyclic aliases must not produce a nominal digest");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == "sema.entry.invalid_nominal_schema"
            && diagnostic.message().contains("poisoned type")
    }));
}

#[test]
fn entity_refs_are_checked_then_rejected_as_persisted_entry_data() {
    let source = SOURCE.replace(
        "struct GameState {\n    score: i32\n}",
        "struct GameState { route: Ref<Flow> }",
    );
    let diagnostics = checked_project(&[("", &source)])
        .expect_err("entity references are not canonical persisted data shapes");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == "sema.entry.invalid_nominal_schema"
            && diagnostic.message().contains("Ref<Flow>")
            && diagnostic
                .message()
                .contains("not a canonical persisted data shape")
    }));
}

#[test]
fn authored_agent_budget_changes_policy_and_binding_digests() {
    let source = |budget: &str| {
        format!(
            r"
{budget}
fn smoke() -> Result<Unit, AgentError>
effects {{ agent.observe }}
{{
    ()
}}

entry agent @entry.agent.smoke {{
    controller = smoke
}}
",
        )
    };
    let defaults = checked_agent(&source(""));
    let authored = checked_agent(&source(
        "#[budget(timeout = 20s, steps = 96usize, host_calls = 9, observations = 8, captures = 7, stored_bytes = 12345, rag_queries = 6, context_bytes = 4096)]",
    ));
    let defaults = defaults.agent().unwrap();
    let authored = authored.agent().unwrap();

    assert_eq!(authored.budget().logical_timeout_millis(), 20_000);
    assert_eq!(authored.budget().max_vm_steps(), 96);
    assert_ne!(defaults.policy_digest(), authored.policy_digest());
    assert_ne!(defaults.binding_digest, authored.binding_digest);
}

#[test]
fn agent_callable_family_is_scoped_to_the_exact_selected_controller() {
    let selected = r"
fn controller() -> Result<Unit, AgentError>
effects { agent.observe }
{
    observe()
    Ok(())
}

entry agent @entry.agent.controller {
    controller = controller
}
";
    checked_project(&[("", selected)])
        .expect("selected Agent controller receives Agent callable scope");

    let unselected = r"
fn stray() -> Result<Unit, AgentError>
effects { agent.observe }
{
    observe()
    Ok(())
}

fn controller() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

entry agent @entry.agent.controller {
    controller = controller
}
";
    let diagnostics = checked_project(&[("", unselected)])
        .expect_err("unselected ordinary function must not receive Agent callable scope");
    assert_unbound_agent_call(&diagnostics, unselected, "observe()");
}

#[test]
fn agent_role_reuses_the_ordinary_function_propagation_identity() {
    let function = r"
fn controller() -> Result<Unit, AgentError>
effects {}
{
    let value = input()?
    Ok(value)
}
";
    let with_entry = format!(
        r"{function}
entry agent @entry.agent.smoke {{
    controller = controller
}}
"
    );
    let environment = || {
        TypeCheckEnv::standard().with_function(
            "input",
            TypeKind::Result {
                ok: Box::new(TypeKind::Unit),
                error: Box::new(TypeKind::String),
            },
        )
    };

    let (document, project, world) = root_project_source("agent-propagation-role", &with_entry);
    let facts = ProjectRegistrationFacts::try_new(world, vec![document], Vec::new(), Vec::new())
        .expect("Agent propagation registration facts");
    let registered =
        register(&project, &facts, environment(), None).expect("Agent propagation semantic world");
    let report =
        crate::checker::analyze_registered_project_types(&project.linked_module(), &registered);
    let mismatch = report
        .diagnostics
        .iter()
        .find(|error| error.stable_code() == "sema.try.error_mismatch")
        .expect("controller body keeps the ordinary propagation diagnostic");
    let TypeCheckErrorKind::TryErrorMismatch { boundary, .. } = mismatch.kind() else {
        panic!("unexpected propagation diagnostic: {mismatch:#?}")
    };
    assert_eq!(boundary.kind(), PropagationBoundaryKind::Function);
    let declaration = boundary
        .declaration()
        .expect("registered ordinary function has a declaration identity");

    let catalog = check_project_entries(
        &project,
        registered.symbols(),
        registered.environment().callable_catalog(),
        &report,
    )
    .expect("body diagnostics do not create a second entry-role identity");
    let controller = catalog
        .get(&CheckedEntryId::try_new("entry.agent.smoke").unwrap())
        .expect("checked Agent entry")
        .agent()
        .expect("Agent binding")
        .controller();
    assert_eq!(controller.declaration(), declaration);

    let (document, project, world) = root_project_source("agent-propagation-role", function);
    let facts = ProjectRegistrationFacts::try_new(world, vec![document], Vec::new(), Vec::new())
        .expect("ordinary propagation registration facts");
    let registered = register(&project, &facts, environment(), None)
        .expect("ordinary propagation semantic world");
    let ordinary =
        crate::checker::analyze_registered_project_types(&project.linked_module(), &registered);
    let ordinary_mismatch = ordinary
        .diagnostics
        .iter()
        .find(|error| error.stable_code() == "sema.try.error_mismatch")
        .expect("ordinary function keeps the same propagation diagnostic");
    let TypeCheckErrorKind::TryErrorMismatch {
        boundary: ordinary_boundary,
        ..
    } = ordinary_mismatch.kind()
    else {
        panic!("unexpected ordinary propagation diagnostic: {ordinary_mismatch:#?}")
    };
    assert_eq!(ordinary_boundary.declaration(), Some(declaration));
    assert_eq!(ordinary_mismatch.message(), mismatch.message());
}

#[test]
fn established_agent_intrinsic_is_rejected_in_an_unselected_helper() {
    let source = r"
fn stray() -> Unit
effects {}
{
    expect(true)
}

fn controller() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

entry agent @entry.agent.controller {
    controller = controller
}
";
    let diagnostics = checked_project(&[("", source)])
        .expect_err("the established `expect` intrinsic remains Agent-role scoped");

    assert_unbound_agent_call(&diagnostics, source, "expect(true)");
}

#[test]
fn selected_agent_controller_nested_closure_keeps_its_lexical_role() {
    let source = r"
fn controller() -> Result<Unit, AgentError>
effects {}
{
    let check = || -> Unit {
        expect(true)
    }
    check()
    Ok(())
}

entry agent @entry.agent.controller {
    controller = controller
}
";

    checked_project(&[("", source)])
        .expect("a nested closure retains its selected controller's lexical declaration owner");
}

#[test]
fn selected_controller_does_not_lend_its_role_to_an_ordinary_helper() {
    let source = r"
fn helper() -> Unit
effects {}
{
    expect(true)
}

fn controller() -> Result<Unit, AgentError>
effects {}
{
    helper()
    Ok(())
}

entry agent @entry.agent.controller {
    controller = controller
}
";
    let diagnostics = checked_project(&[("", source)])
        .expect_err("controller reachability must not grant an ordinary helper the Agent role");

    assert_unbound_agent_call(&diagnostics, source, "expect(true)");
}

#[test]
fn multiple_agent_entries_grant_each_exact_controller_its_own_role() {
    let source = r"
fn first() -> Result<Unit, AgentError>
effects {}
{
    expect(true)
    Ok(())
}

fn second() -> Result<Unit, AgentError>
effects {}
{
    expect(true)
    Ok(())
}

entry agent @entry.agent.first {
    controller = first
}

entry agent @entry.agent.second {
    controller = second
}
";

    let catalog = checked_project(&[("", source)])
        .expect("each exact Agent entry controller receives the Agent callable role");
    assert!(
        catalog
            .get(&CheckedEntryId::try_new("entry.agent.first").unwrap())
            .is_some()
    );
    assert!(
        catalog
            .get(&CheckedEntryId::try_new("entry.agent.second").unwrap())
            .is_some()
    );
}

#[test]
fn same_named_cross_module_function_does_not_inherit_selected_controller_role() {
    let root = "";
    let selected = r"
fn controller() -> Result<Unit, AgentError>
effects {}
{
    expect(true)
    Ok(())
}

entry agent @entry.agent.selected {
    controller = controller
}
";
    let unselected = r"
fn controller() -> Unit
effects {}
{
    expect(true)
}
";
    let diagnostics = checked_project(&[("", root), ("selected", selected), ("other", unselected)])
        .expect_err("a same-named declaration in another module has a distinct callable identity");

    assert_unbound_agent_call(&diagnostics, unselected, "expect(true)");
}

#[test]
fn agent_intrinsic_in_flow_has_no_controller_owner_and_is_rejected() {
    let source = r"
fn controller() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

flow @flow.unbound_agent_call unbound_agent_call {
    expect(true)
}

entry agent @entry.agent.controller {
    controller = controller
}
";
    let diagnostics = checked_project(&[("", source)])
        .expect_err("a flow cannot acquire an Agent controller function role");

    assert_unbound_agent_call(&diagnostics, source, "expect(true)");
}

#[test]
fn same_named_cross_module_calls_keep_distinct_canonical_effect_identities() {
    let sources = [
        (
            "",
            r#"
use crate.left.work as left_work
use crate.right.work as right_work

fn call_left() -> String {
    left_work("story.arcw")
}

fn call_right() -> Unit {
    right_work()
}
"#,
        ),
        (
            "left",
            r"
pub fn work(path: String) -> String {
    adapter.read_text(path = path)
}
",
        ),
        (
            "right",
            r"
pub fn work() -> Unit {
    ()
}
",
        ),
    ];
    let (documents, project, world) = project_modules("same-name-effects", &sources);
    let facts = ProjectRegistrationFacts::try_new(world, documents, Vec::new(), Vec::new())
        .expect("same-name effect registration facts");
    let env = TypeCheckEnv::standard()
        .with_function_signature(
            "adapter.read_text",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("path", TypeKind::String)],
            ),
        )
        .with_function_effects("adapter.read_text", ["fs.read".to_owned()]);
    let registered =
        register(&project, &facts, env, None).expect("same-name effect semantic world");
    let report =
        crate::checker::analyze_registered_project_types(&project.linked_module(), &registered);
    let closed = report
        .effects
        .closed_effect_rows()
        .expect("same-name effect rows close");

    let callable = |module: &str, name: &str| {
        let function = project
            .modules()
            .find(|(path, _)| path.to_string() == module)
            .and_then(|(_, module)| {
                module
                    .functions()
                    .iter()
                    .find(|function| function.name() == name)
            })
            .expect("fixture function");
        let declaration = CallableDeclarationId::for_function(project.package(), function)
            .expect("canonical fixture declaration");
        CallableId::project_function(&declaration)
    };
    let labels = |module: &str, name: &str| {
        closed
            .summary(&callable(module, name))
            .expect("canonical callable effect summary")
            .inferred()
            .to_labels()
    };

    assert_eq!(labels("crate.left", "work"), ["fs.read"]);
    assert!(labels("crate.right", "work").is_empty());
    assert_eq!(labels("crate", "call_left"), ["fs.read"]);
    assert!(labels("crate", "call_right").is_empty());
}
