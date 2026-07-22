use std::sync::Arc;

use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_syntax::{ast::module_path::CanonicalModulePath, parser::parse_source};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRevision};

use crate::{
    checker::analyze_registered_project_types,
    entry::{CheckedEntryBinding, CheckedEntryId},
    env::TypeCheckEnv,
    registration::ProjectRegistrationFacts,
    test_support::character_project::{project_modules, register},
};

use super::{CheckedEntryDiagnostic, check_project_entries};

const SOURCE: &str = r"
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

fn source_document(id: &str, source: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).unwrap(),
            SourceName::path(id),
            source,
        )
        .unwrap(),
    )
}

fn checked_single(
    package: &str,
    document_id: &str,
    source: &str,
) -> Result<(CheckedEntryBinding, SourceRevision), Vec<CheckedEntryDiagnostic>> {
    let document = source_document(document_id, source);
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_document_to_hir(&document, parsed.typed_tree()).unwrap();
    let project = HirProject::new(
        package,
        [HirProjectModule::try_new(
            CanonicalModulePath::crate_root(),
            document.identity().clone(),
            hir,
        )
        .unwrap()],
    )
    .unwrap();
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(package).unwrap(),
        document.identity().id().clone(),
        "entry-identity",
    )
    .unwrap();
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None).unwrap();
    let typecheck = analyze_registered_project_types(&project.linked_module(), &registered);
    let catalog = check_project_entries(
        &project,
        registered.symbols(),
        registered.environment().callable_catalog(),
        &typecheck,
    )?;
    assert!(
        typecheck.diagnostics.is_empty(),
        "identity fixtures that reach a binding must be type-valid: {:?}",
        typecheck.diagnostics
    );
    let entry = catalog
        .entries()
        .next()
        .expect("identity fixture contains one entry")
        .clone();
    Ok((entry, document.identity().revision()))
}

fn checked(source: &str) -> CheckedEntryBinding {
    checked_single(
        "registration-tests",
        "C:/workspace/arcweft/src/main.arcw",
        source,
    )
    .unwrap()
    .0
}

fn checked_modules(sources: &[(&str, &str)]) -> CheckedEntryBinding {
    let (documents, project, world) = project_modules("entry-identity-order", sources);
    let facts =
        ProjectRegistrationFacts::try_new(world, documents, Vec::new(), Vec::new(), Vec::new())
            .unwrap();
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None).unwrap();
    let typecheck = analyze_registered_project_types(&project.linked_module(), &registered);
    let catalog = check_project_entries(
        &project,
        registered.symbols(),
        registered.environment().callable_catalog(),
        &typecheck,
    )
    .unwrap();
    catalog
        .get(&CheckedEntryId::try_new("entry.game.main").unwrap())
        .unwrap()
        .clone()
}

#[test]
fn id_001_rebuilding_identical_project_repeats_every_digest() {
    let first = checked(SOURCE);
    let second = checked(SOURCE);
    let first_stateful = first.stateful().unwrap();
    let second_stateful = second.stateful().unwrap();

    assert_eq!(first.binding_digest(), second.binding_digest());
    assert_eq!(
        first_stateful.state().schema_digest(),
        second_stateful.state().schema_digest()
    );
    assert_eq!(
        first_stateful.event().schema_digest(),
        second_stateful.event().schema_digest()
    );
    assert_eq!(
        first_stateful.reducer().contract_digest(),
        second_stateful.reducer().contract_digest()
    );
    assert_eq!(
        first_stateful.initial_flow().contract_digest(),
        second_stateful.initial_flow().contract_digest()
    );
}

#[test]
fn id_002_reversing_module_traversal_order_preserves_binding() {
    let helper = "pub fn unrelated() -> Unit { () }\n";
    let first = checked_modules(&[("", SOURCE), ("support", helper)]);
    let second = checked_modules(&[("support", helper), ("", SOURCE)]);
    assert_eq!(first.binding_digest(), second.binding_digest());
}

#[test]
fn id_003_state_field_name_order_and_type_each_change_schema_and_binding() {
    let baseline_source = SOURCE.replace("score: i32", "first: i32\n    second: String");
    let baseline = checked(&baseline_source);
    for changed_source in [
        baseline_source.replace("first: i32", "renamed: i32"),
        baseline_source.replace(
            "first: i32\n    second: String",
            "second: String\n    first: i32",
        ),
        baseline_source.replace("first: i32", "first: i64"),
    ] {
        let changed = checked(&changed_source);
        assert_ne!(
            baseline.stateful().unwrap().state().schema_digest(),
            changed.stateful().unwrap().state().schema_digest()
        );
        assert_ne!(baseline.binding_digest(), changed.binding_digest());
    }
}

#[test]
fn id_004_event_variant_change_changes_schema_and_binding() {
    let first = checked(SOURCE);
    let second = checked(&SOURCE.replace("Start\n}", "Start\n    Stop\n}"));
    assert_ne!(
        first.stateful().unwrap().event().schema_digest(),
        second.stateful().unwrap().event().schema_digest()
    );
    assert_ne!(first.binding_digest(), second.binding_digest());
}

#[test]
fn id_005_reducer_body_only_preserves_binding_but_changes_code_identity() {
    let changed = SOURCE.replace(
        "{\n    reduce_game(state, event)\n}\n\nflow",
        "{\n    let marker = 1\n    reduce_game(state, event)\n}\n\nflow",
    );
    let (first, first_code_revision) = checked_single(
        "registration-tests",
        "C:/workspace/arcweft/src/main.arcw",
        SOURCE,
    )
    .unwrap();
    let (second, second_code_revision) = checked_single(
        "registration-tests",
        "C:/workspace/arcweft/src/main.arcw",
        &changed,
    )
    .unwrap();

    assert_eq!(first.binding_digest(), second.binding_digest());
    assert_eq!(
        first.stateful().unwrap().reducer().contract_digest(),
        second.stateful().unwrap().reducer().contract_digest()
    );
    // SourceRevision is the existing content identity consumed by compilation;
    // entry binding deliberately excludes function bodies from its contract identity.
    assert_ne!(first_code_revision, second_code_revision);
}

#[test]
fn id_006_reducer_rename_and_rebind_each_change_binding() {
    let baseline = checked(SOURCE);
    let renamed = checked(&SOURCE.replace("reduce_game", "advance_game"));
    let alternate = r"
fn alternate_reduce(state: &GameState, event: GameEvent)
    -> Result<Reduction<GameState>, ReducerError>
effects {}
{
    alternate_reduce(state, event)
}

";
    let rebound_source = SOURCE
        .replace(
            "flow @flow.opening",
            &format!("{alternate}flow @flow.opening"),
        )
        .replace("reducer = reduce_game", "reducer = alternate_reduce");
    let rebound = checked(&rebound_source);

    assert_ne!(baseline.binding_digest(), renamed.binding_digest());
    assert_ne!(baseline.binding_digest(), rebound.binding_digest());
    assert_ne!(
        baseline.stateful().unwrap().reducer().declaration(),
        renamed.stateful().unwrap().reducer().declaration()
    );
    assert_ne!(
        baseline.stateful().unwrap().reducer().declaration(),
        rebound.stateful().unwrap().reducer().declaration()
    );
}

#[test]
fn id_006_invalid_reducer_signature_and_effect_are_rejected_before_binding() {
    let signature = SOURCE.replace("state: &GameState", "state: GameState");
    let effect = SOURCE.replace(
        "-> Result<Reduction<GameState>, ReducerError>\neffects {}",
        "-> Result<Reduction<GameState>, ReducerError>\neffects { fs.read }",
    );
    for source in [signature, effect] {
        let diagnostics = checked_single(
            "registration-tests",
            "C:/workspace/arcweft/src/main.arcw",
            &source,
        )
        .unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code() == "sema.entry.invalid_reducer_contract" })
        );
    }
}

#[test]
fn id_007_absolute_source_path_does_not_enter_binding() {
    let (first, _) = checked_single(
        "registration-tests",
        "C:/agent-a/work/arcweft/src/main.arcw",
        SOURCE,
    )
    .unwrap();
    let (second, _) = checked_single(
        "registration-tests",
        "D:/agent-b/other/arcweft/src/main.arcw",
        SOURCE,
    )
    .unwrap();
    assert_eq!(first.binding_digest(), second.binding_digest());
}

#[test]
fn id_008_flow_id_or_valid_contract_changes_binding_while_body_only_does_not() {
    let baseline = checked(SOURCE);
    let changed_id = checked(&SOURCE.replace("flow.opening", "flow.alternate"));
    let changed_contract = checked(&SOURCE.replace(
        "flow @flow.opening opening(state: GameState) {\n}",
        "flow @flow.opening opening(state: GameState) -> i32 {\n    return 1\n}",
    ));
    let flow_body = SOURCE.replace(
        "flow @flow.opening opening(state: GameState) {\n}",
        "flow @flow.opening opening(state: GameState) {\n    let marker = 1\n}",
    );
    let (body_changed, body_revision) = checked_single(
        "registration-tests",
        "C:/workspace/arcweft/src/main.arcw",
        &flow_body,
    )
    .unwrap();
    let (_, baseline_revision) = checked_single(
        "registration-tests",
        "C:/workspace/arcweft/src/main.arcw",
        SOURCE,
    )
    .unwrap();

    assert_ne!(baseline.binding_digest(), changed_id.binding_digest());
    assert_ne!(
        baseline
            .stateful()
            .unwrap()
            .initial_flow()
            .contract_digest(),
        changed_contract
            .stateful()
            .unwrap()
            .initial_flow()
            .contract_digest()
    );
    assert_ne!(baseline.binding_digest(), changed_contract.binding_digest());
    assert_eq!(baseline.binding_digest(), body_changed.binding_digest());
    assert_eq!(
        baseline
            .stateful()
            .unwrap()
            .initial_flow()
            .contract_digest(),
        body_changed
            .stateful()
            .unwrap()
            .initial_flow()
            .contract_digest()
    );
    // The valid flow body changes the existing compiler content identity, while
    // the entry digest remains a role/contract identity.
    assert_ne!(baseline_revision, body_revision);
}

#[test]
fn package_identity_changes_binding_without_changing_source_contract() {
    let (first, _) = checked_single(
        "registration-tests-a",
        "C:/workspace/arcweft/src/main.arcw",
        SOURCE,
    )
    .unwrap();
    let (second, _) = checked_single(
        "registration-tests-b",
        "C:/workspace/arcweft/src/main.arcw",
        SOURCE,
    )
    .unwrap();
    assert_ne!(first.binding_digest(), second.binding_digest());
}

#[test]
fn alias_import_and_qualified_signature_spelling_preserve_binding() {
    let baseline = checked(SOURCE);
    let aliased = SOURCE
        .replace(
            "struct GameState {",
            "type StateAlias = GameState\ntype EventAlias = GameEvent\n\nstruct GameState {",
        )
        .replace("-> GameState\neffects", "-> StateAlias\neffects")
        .replace("state: &GameState", "state: &StateAlias")
        .replace("event: GameEvent", "event: EventAlias")
        .replace("Reduction<GameState>", "Reduction<StateAlias>")
        .replace("opening(state: GameState)", "opening(state: StateAlias)");
    let alias_binding = checked(&aliased);
    assert_eq!(baseline.binding_digest(), alias_binding.binding_digest());
}
