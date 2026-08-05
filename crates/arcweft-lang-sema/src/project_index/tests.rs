use super::agent_prelude::{agent_prelude_callables, agent_result};
use super::*;
use arcweft_lang_hir::lower::lower_document_to_hir;
use arcweft_lang_syntax::{
    parser::{ParseOptions, parse_document_with_source},
    source::ParsedSource,
};
use arcweft_source::{SourceAnchor, SourceDocument, SourceDocumentId, SourceName, SourceRange};

use crate::{
    checker::analyze_registered_project_types,
    entry::{CheckedEntryCatalog, check_project_entries},
    env::TypeCheckEnv,
    registration::ProjectRegistrationFacts,
    test_support::character_project::{project_modules, register, root_project_source},
};

fn public_id(value: &str) -> PublicId {
    PublicId::try_new(value).expect("valid public id")
}

fn document_for_hir(hir: &arcweft_lang_hir::model::HirModule) -> SourceDocument {
    hir.source_document()
        .expect("document-bound HIR retains its source")
        .clone()
}

fn parse_project_index_fixture(source: impl Into<String>) -> ParsedSource {
    let document = std::sync::Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://sema/project-index.arcw")
                .expect("test document id"),
            SourceName::Generated,
            source.into(),
        )
        .expect("test source document"),
    );
    parse_document_with_source(document, ParseOptions::default())
}

fn test_source_anchor() -> SourceAnchor {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("generated://arcweft/sema-test").expect("test document id"),
        SourceName::Generated,
        "",
    )
    .expect("test source document");
    SourceAnchor::from_span(document.span(SourceRange::new(0, 0)).expect("empty span"))
}

fn checked_project_index(
    profile: &str,
    source: &str,
) -> (CheckedEntryCatalog, ProjectSemanticIndex) {
    let (document, project, world) = root_project_source(profile, source);
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![document],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("entry role registration facts");
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("entry role semantic world");
    let typecheck = analyze_registered_project_types(&project.linked_module(), &registered);
    let catalog = check_project_entries(
        &project,
        registered.symbols(),
        registered.environment().callable_catalog(),
        &typecheck,
    )
    .expect("entry roles check");
    let index = project_semantic_index_from_checked_project(
        &project,
        registered.symbols(),
        &typecheck,
        ProgramHash::new(format!("program-{profile}")),
        &catalog,
    )
    .expect("checked project index builds");
    (catalog, index)
}

fn accepted_project_index(
    project: &HirProject,
    documents: Vec<std::sync::Arc<SourceDocument>>,
    world: arcweft_lang_hir::symbol::ProjectSymbolWorldId,
    program_hash: &str,
) -> ProjectSemanticIndex {
    let facts =
        ProjectRegistrationFacts::try_new(world, documents, Vec::new(), Vec::new(), Vec::new())
            .expect("project index registration facts");
    let registered = register(project, &facts, TypeCheckEnv::standard(), None)
        .expect("project index registered world");
    let typecheck = analyze_registered_project_types(&project.linked_module(), &registered);
    assert!(
        typecheck.diagnostics.is_empty(),
        "accepted project must typecheck: {:#?}",
        typecheck.diagnostics
    );
    project_semantic_index_from_checked_project(
        project,
        registered.symbols(),
        &typecheck,
        ProgramHash::new(program_hash),
        &CheckedEntryCatalog::default(),
    )
    .expect("accepted project index builds")
}

#[test]
fn project_index_preserves_entity_payload_type() {
    let signal = EntitySymbol::new(
        public_id("signal.ready"),
        EntityType::new(EntityKind::Signal, Some(TypeKind::Bool)),
        test_source_anchor(),
        SemanticHash::new("shape.signal.ready.v1"),
    );
    let index =
        ProjectSemanticIndex::new(ProgramHash::new("program-a")).with_entity(signal.clone());

    let stored = index.entity(signal.id()).expect("signal stored");

    assert_eq!(stored.ty().kind(), &EntityKind::Signal);
    assert_eq!(stored.ty().value(), Some(&TypeKind::Bool));
    assert_eq!(stored.semantic_hash().as_str(), "shape.signal.ready.v1");
    assert_eq!(
        index.typecheck_env().symbol_type("signal.ready"),
        Some(&TypeKind::entity_ref_with_value(
            EntityKind::Signal,
            TypeKind::Bool
        ))
    );
}

#[test]
fn project_index_records_entry_and_flow_entity_relations() {
    let tree = parse_project_index_fixture(
        r#"
entry cli @entry.main {
    goto @flow.opening
    goto @flow.listen
}

signal current_flow: Watch<Ref<Flow>>

flow opening() -> String {
    narrator.say(id=@say.opening)[hello]
    let current = @signal.current_flow
    include @flow.intro
    choice @choice.opening {
        @choice.opening.listen "Listen" -> @flow.listen
    }
    goto @flow.listen
}

pub flow intro {
    return "intro"
}

flow listen {
    return "listen"
}
"#,
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree()).expect("source lowers");
    let document = document_for_hir(&hir);
    let index = project_semantic_index_from_hir(&hir, ProgramHash::new("program-test"), &document)
        .expect("project index builds");

    let relations = index
        .relations()
        .iter()
        .map(|relation| {
            (
                relation.from().as_str(),
                relation.to().as_str(),
                relation.edge_kind().as_str(),
            )
        })
        .collect::<Vec<_>>();

    assert!(relations.contains(&("entry.main", "flow.opening", "entry_goto")));
    assert!(relations.contains(&("entry.main", "flow.listen", "entry_goto")));
    assert!(relations.contains(&("flow.opening", "say.opening", "contains_dialogue")));
    assert!(relations.contains(&("flow.opening", "choice.opening", "contains_choice")));
    assert!(relations.contains(&(
        "choice.opening",
        "choice.opening.listen",
        "contains_choice_option"
    )));
    assert!(relations.contains(&("choice.opening.listen", "flow.listen", "choice_option_goto")));
    assert!(relations.contains(&("flow.opening", "flow.intro", "flow_include")));
    assert!(relations.contains(&("flow.opening", "flow.listen", "flow_goto")));
    assert!(relations.contains(&("flow.opening", "signal.current_flow", "references_entity")));
}

#[test]
fn project_index_records_content_root_relations() {
    let tree = parse_project_index_fixture(
        r#"
content chapter_two {
    roots = [ @flow.chapter_two, @asset:.bg ]
}

flow chapter_two {
    return "chapter two"
}
"#,
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree()).expect("source lowers");
    let document = document_for_hir(&hir);
    let index = project_semantic_index_from_hir(&hir, ProgramHash::new("program-test"), &document)
        .expect("project index builds");

    let relations = index
        .relations()
        .iter()
        .map(|relation| {
            (
                relation.from().as_str(),
                relation.to().as_str(),
                relation.edge_kind().as_str(),
            )
        })
        .collect::<Vec<_>>();

    assert!(relations.contains(&("content.chapter_two", "flow.chapter_two", "content_root")));
    assert!(relations.contains(&("content.chapter_two", "asset.bg", "content_root")));
}

#[test]
fn project_index_projects_entities_and_agent_prelude_to_env() {
    let index =
        ProjectSemanticIndex::new(ProgramHash::new("program-a")).with_entity(EntitySymbol::new(
            public_id("choice.opening.listen"),
            EntityType::new(EntityKind::ChoiceOption, None),
            test_source_anchor(),
            SemanticHash::new("shape.choice.opening.listen.v1"),
        ));
    let env = index.typecheck_env();

    assert_eq!(
        env.symbol_type("choice.opening.listen"),
        Some(&TypeKind::entity_ref(EntityKind::ChoiceOption))
    );
    assert_eq!(
        env.function_signature("choose")
            .map(FunctionSignature::return_type),
        Some(&agent_result(TypeKind::ActionResult))
    );
    assert_eq!(
        env.function_effects("choose").map(|effects| {
            effects
                .iter()
                .map(EffectCapability::as_str)
                .collect::<Vec<_>>()
        }),
        Some(vec!["agent.act.semantic"])
    );
}

#[test]
fn agent_prelude_marks_structured_intrinsic_lowering() {
    let prelude = agent_prelude_callables();
    let wait = prelude
        .get(&QualifiedName::new("wait"))
        .expect("wait intrinsic");

    assert_eq!(
        wait.lowering(),
        &CallableLowering::AgentIntrinsic(AgentIntrinsic::Wait)
    );
    assert_eq!(
        wait.effects()
            .iter()
            .map(EffectCapability::as_str)
            .collect::<Vec<_>>(),
        vec!["agent.wait", "agent.observe"]
    );

    let advance_text = prelude
        .get(&QualifiedName::new("advance_text"))
        .expect("advance_text intrinsic");
    assert_eq!(
        advance_text.lowering(),
        &CallableLowering::AgentIntrinsic(AgentIntrinsic::AdvanceText)
    );
    assert_eq!(
        advance_text
            .effects()
            .iter()
            .map(EffectCapability::as_str)
            .collect::<Vec<_>>(),
        vec!["agent.act.semantic"]
    );
}

#[test]
fn project_index_from_hir_does_not_reparse_raw_signal_type_tails() {
    let tree = parse_project_index_fixture(
        r#"
signal current_flow: Watch<Ref<Flow>>
flow opening() -> String {
    return "ok"
}
"#,
    );
    let hir =
        lower_document_to_hir(tree.document(), tree.typed_tree()).expect("source lowers to HIR");
    let document = document_for_hir(&hir);
    let index = project_semantic_index_from_hir(&hir, ProgramHash::new("program-a"), &document)
        .expect("HIR indexes for Agent Script");

    assert_eq!(
        index.typecheck_env().symbol_type("flow.opening"),
        Some(&TypeKind::entity_ref(EntityKind::Flow))
    );
    assert_eq!(
        index.typecheck_env().symbol_type("signal.current_flow"),
        Some(&TypeKind::entity_ref(EntityKind::Signal))
    );
}

#[test]
fn project_index_from_hir_preserves_view_text_control_inputs() {
    let tree = parse_project_index_fixture(
        r#"
view FeedbackForm() {
    TextField(id: @input:.feedback, value: "")
}
"#,
    );
    let hir =
        lower_document_to_hir(tree.document(), tree.typed_tree()).expect("source lowers to HIR");
    let document = document_for_hir(&hir);
    let index = project_semantic_index_from_hir(&hir, ProgramHash::new("program-a"), &document)
        .expect("HIR indexes for Agent Script");

    assert_eq!(
        index.typecheck_env().symbol_type("input.feedback"),
        Some(&TypeKind::entity_ref(EntityKind::Input))
    );
}

#[test]
fn project_index_from_hir_preserves_project_callables_separately_from_agent_prelude() {
    let source = r#"
struct GameState {}
struct GameEvent {}

pub fn update_route(state: GameState, event: GameEvent) -> GameState {
    let route = current_route()
    state
}

pub fn current_route() -> String {
    "opening"
}

flow opening() -> String {
    let route = current_route()
    goto @flow.done
    return "ok"
}

flow done() -> String {
    return "done"
}
"#;
    let (document, project, world) = root_project_source("project-index-callables", source);
    let index = accepted_project_index(&project, vec![document], world, "program-a");

    assert!(
        index
            .callable(&QualifiedName::new("update_route"))
            .is_none()
    );
    let update_route = index
        .project_callable(&QualifiedName::new("update_route"))
        .expect("ordinary function indexed");
    assert_eq!(update_route.kind(), ProjectCallableKind::Function);
    assert_eq!(update_route.signature().params().len(), 2);
    let TypeKind::ProjectNominal(return_type) = update_route.signature().return_type() else {
        panic!("checked project callable return type must retain nominal identity");
    };
    assert_eq!(return_type.declaration().name().as_str(), "GameState");
    let game_state = index
        .project_nominals()
        .values()
        .find(|record| record.id().name().as_str() == "GameState")
        .expect("accepted nominal declaration indexed");
    let state_references = index
        .project_nominal_references()
        .iter()
        .filter(|edge| edge.declaration() == game_state.id())
        .collect::<Vec<_>>();
    assert!(!state_references.is_empty());
    assert!(state_references.iter().all(|edge| {
        source.get(edge.terminal_source().range().start()..edge.terminal_source().range().end())
            == Some("GameState")
    }));
    let declaration = update_route.declaration();
    assert_eq!(declaration.package().as_str(), "registration-tests");
    assert_eq!(declaration.qualified_name(), "update_route");
    assert_eq!(
        index.project_callable_by_declaration(declaration),
        Some(update_route)
    );
    assert!(
        update_route
            .semantic_hash()
            .as_str()
            .contains("hir:callable:function:registration-tests:update_route")
    );

    let current_route = index
        .project_callable(&QualifiedName::new("current_route"))
        .expect("current_route callable indexed");
    assert_eq!(current_route.kind(), ProjectCallableKind::Function);
    assert_eq!(current_route.signature().return_type(), &TypeKind::String);
    let relations = index
        .dependency_relations()
        .iter()
        .map(|relation| {
            (
                relation.from(),
                relation.to(),
                relation.edge_kind().as_str(),
            )
        })
        .collect::<Vec<_>>();
    assert!(relations.iter().any(|(from, to, kind)| {
            matches!(from, ProjectGraphSymbolRef::Entity(id) if id.as_str() == "flow.opening")
                && matches!(to, ProjectGraphSymbolRef::Callable(name) if name.as_str() == "current_route")
                && *kind == "calls_callable"
        }));
    assert!(relations.iter().any(|(from, to, kind)| {
            matches!(from, ProjectGraphSymbolRef::Callable(name) if name.as_str() == "update_route")
                && matches!(to, ProjectGraphSymbolRef::Callable(name) if name.as_str() == "current_route")
                && *kind == "calls_callable"
        }));
    let control = index
        .flow_control_summary(&public_id("flow.opening"))
        .expect("flow control summary indexed");
    assert_eq!(control.static_goto_count(), 1);
    assert_eq!(control.dynamic_goto_count(), 0);
    assert!(!control.has_dynamic_control());
}

#[test]
fn project_index_keeps_same_named_functions_distinct_by_canonical_declaration() {
    let (documents, project, world) = project_modules(
        "project-index-same-name-callables",
        &[
            ("", "fn resolve() -> Unit { () }\n"),
            ("ui", "fn resolve() -> Unit { () }\n"),
        ],
    );
    let index = accepted_project_index(&project, documents, world, "program-same-name-callables");

    let root = index
        .project_callable(&QualifiedName::new("resolve"))
        .map(ProjectCallableSymbol::declaration)
        .expect("root declaration");
    let child = index
        .project_callable(&QualifiedName::new("ui.resolve"))
        .map(ProjectCallableSymbol::declaration)
        .expect("child declaration");
    assert_eq!(root.name(), child.name());
    assert_ne!(root, child);
    assert_ne!(root.module(), child.module());
    assert_eq!(index.project_callables().len(), 2);
}

#[test]
fn project_index_keeps_same_named_function_and_view_owners_distinct() {
    let (document, project, world) = root_project_source(
        "project-index-function-view-identity",
        "fn Card() -> Unit { () }\npub view Card() {\n    Panel()\n}\n",
    );
    let index = accepted_project_index(
        &project,
        vec![document],
        world,
        "program-function-view-identity",
    );
    let package = project.package().clone();
    let module = CanonicalModulePath::crate_root();
    let function = CallableDeclarationId::try_new(
        package.clone(),
        module.clone(),
        CallableDeclarationOwner::Function,
        "Card",
    )
    .expect("Function identity");
    let view =
        CallableDeclarationId::try_new(package, module, CallableDeclarationOwner::View, "Card")
            .expect("View identity");

    assert_ne!(function, view);
    assert!(
        index
            .project_callable(&QualifiedName::new("Card"))
            .is_none(),
        "same-spelling owners must not gain an implicit resolution priority"
    );
    assert_eq!(
        index
            .project_callable_by_declaration(&function)
            .map(ProjectCallableSymbol::kind),
        Some(ProjectCallableKind::Function)
    );
    assert_eq!(
        index
            .project_callable_by_declaration(&view)
            .map(ProjectCallableSymbol::kind),
        Some(ProjectCallableKind::View)
    );
    assert_eq!(index.project_callables().len(), 2);
}

fn indexed_view_semantic_hash(source: &str) -> String {
    let (document, project, world) =
        root_project_source("project-index-view-semantic-hash", source);
    accepted_project_index(
        &project,
        vec![document],
        world,
        "program-view-semantic-hash",
    )
    .project_callable(&QualifiedName::new("Card"))
    .expect("Card View callable")
    .semantic_hash()
    .as_str()
    .to_owned()
}

#[test]
fn view_semantic_hash_is_stable_across_signature_whitespace() {
    let compact = indexed_view_semantic_hash(
        "pub view Card<T>(value: T = make_default(1), labels: ...String) {\n    Panel()\n}\n",
    );
    let spaced = indexed_view_semantic_hash(
        "pub view Card< T >( value : T = make_default( 1 ), labels : ...String ) {\n    Panel()\n}\n",
    );

    assert_eq!(compact, spaced);
}

#[test]
fn view_semantic_hash_distinguishes_typed_callable_contract_changes() {
    let baseline = indexed_view_semantic_hash(
        "pub view Card<T>(value: T = make_default(1), labels: ...String) {\n    Panel()\n}\n",
    );
    let changed_contracts = [
        (
            "required parameter",
            "pub view Card<T>(value: T, labels: ...String) {\n    Panel()\n}\n",
        ),
        (
            "default expression",
            "pub view Card<T>(value: T = make_default(2), labels: ...String) {\n    Panel()\n}\n",
        ),
        (
            "fixed rather than rest parameter",
            "pub view Card<T>(value: T = make_default(1), labels: String) {\n    Panel()\n}\n",
        ),
        (
            "generic parameter name",
            "pub view Card<U>(value: U = make_default(1), labels: ...String) {\n    Panel()\n}\n",
        ),
        (
            "parameter type",
            "pub view Card<T>(value: String = make_default(1), labels: ...String) {\n    Panel()\n}\n",
        ),
    ];

    for (dimension, source) in changed_contracts {
        assert_ne!(
            baseline,
            indexed_view_semantic_hash(source),
            "{dimension} must participate in View semantic identity"
        );
    }
}

#[test]
fn view_contract_label_includes_declared_return_type() {
    let string_signature =
        arcweft_lang_syntax::types::parse_fn_signature("fn view(value: i32) -> String")
            .expect("String-returning typed signature");
    let unit_signature =
        arcweft_lang_syntax::types::parse_fn_signature("fn view(value: i32) -> Unit")
            .expect("Unit-returning typed signature");

    assert_ne!(
        super::entities::view_callable_contract_label(&string_signature, "(value: i32) -> String"),
        super::entities::view_callable_contract_label(&unit_signature, "(value: i32) -> Unit")
    );
}

#[test]
fn checked_project_index_records_exact_entry_roles_to_original_declarations() {
    let source = r"
struct GameState {
    score: i32
}

enum GameEvent {
    Start
}

fn initial_game_state() -> GameState
effects {}
{
    ()
}

fn reduce_game(state: &GameState, event: GameEvent)
    -> Result<Reduction<GameState>, ReducerError>
effects {}
{
    ()
}

flow opening(state: GameState) {
}

entry game @entry.game.main {
    state = GameState
    initializer = initial_game_state
    event = GameEvent
    reducer = reduce_game
    goto @flow.opening
}
";
    let (catalog, index) = checked_project_index("project-index-entry-roles", source);
    let binding = catalog.entries().next().expect("one checked entry");
    let entry_id = binding.id().clone();

    let record = index
        .entry_record(&entry_id)
        .expect("checked entry record indexed");
    assert_eq!(record.id(), &entry_id);
    assert_eq!(record.kind(), &binding.kind());
    assert_eq!(record.binding_digest(), binding.binding_digest());
    assert!(record.agent_policy_digest().is_none());

    let edges = index.entry_role_edges_for(&entry_id).collect::<Vec<_>>();
    assert_eq!(
        edges.iter().map(|edge| edge.role()).collect::<Vec<_>>(),
        vec![
            ProjectEntryRoleKind::State,
            ProjectEntryRoleKind::Initializer,
            ProjectEntryRoleKind::Event,
            ProjectEntryRoleKind::Reducer,
            ProjectEntryRoleKind::InitialFlow,
        ]
    );
    let stateful = binding.stateful().expect("stateful checked entry");
    assert!(matches!(
        edges[0].target(),
        ProjectEntryRoleTarget::Nominal { key, schema_digest }
            if key == stateful.state().key()
                && schema_digest == stateful.state().schema_digest()
    ));
    assert!(matches!(
        edges[1].target(),
        ProjectEntryRoleTarget::Callable {
            declaration,
            contract_digest,
        } if declaration == stateful.initializer().declaration()
            && contract_digest == stateful.initializer().contract_digest()
    ));
    assert!(matches!(
        edges[3].target(),
        ProjectEntryRoleTarget::Callable {
            declaration,
            contract_digest,
        } if declaration == stateful.reducer().declaration()
            && contract_digest == stateful.reducer().contract_digest()
    ));
    assert!(matches!(
        edges[4].target(),
        ProjectEntryRoleTarget::Flow { id, contract_digest }
            if id == stateful.initial_flow().id()
                && contract_digest == stateful.initial_flow().contract_digest()
    ));
    assert_eq!(
        index
            .project_callable_by_declaration(stateful.reducer().declaration())
            .map(ProjectCallableSymbol::declaration),
        Some(stateful.reducer().declaration())
    );
}

#[test]
fn checked_project_index_records_agent_policy_and_controller_edge() {
    let source = r"
#[budget(timeout = 20s, steps = 96usize)]
fn smoke() -> Result<Unit, AgentError>
effects { agent.observe }
{
    ()
}

entry agent @entry.agent.smoke {
    controller = smoke
}
";
    let (catalog, index) = checked_project_index("project-index-agent-role", source);
    let binding = catalog.entries().next().expect("one checked Agent entry");
    let agent = binding.agent().expect("Agent binding");

    let record = index
        .entry_record(binding.id())
        .expect("Agent entry record indexed");
    assert_eq!(record.agent_policy_digest(), Some(agent.policy_digest()));
    let edges = index.entry_role_edges_for(binding.id()).collect::<Vec<_>>();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].role(), ProjectEntryRoleKind::Controller);
    assert!(matches!(
        edges[0].target(),
        ProjectEntryRoleTarget::Callable {
            declaration,
            contract_digest,
        } if declaration == agent.controller().declaration()
            && contract_digest == agent.controller().contract_digest()
    ));
    assert_eq!(
        index
            .project_callable_by_declaration(agent.controller().declaration())
            .map(ProjectCallableSymbol::declaration),
        Some(agent.controller().declaration())
    );
}

#[test]
fn project_index_from_hir_projects_inline_image_agent_actions() {
    let tree = parse_project_index_fixture(
        r#"
flow opening {
    let pulse = image(asset = @asset:.bg.pulse, target = "target.sample.pulse", layer = "layer.foreground", x = 96px, y = 72px, width = 360px, height = 180px, action = "action.inspect.pulse")
}
"#,
    );
    let hir =
        lower_document_to_hir(tree.document(), tree.typed_tree()).expect("source lowers to HIR");
    let document = document_for_hir(&hir);
    let index = project_semantic_index_from_hir(&hir, ProgramHash::new("program-a"), &document)
        .expect("HIR indexes inline image actions");
    let env = index.typecheck_env();

    assert_eq!(
        env.symbol_type("target.sample.pulse"),
        Some(&TypeKind::entity_ref(EntityKind::Target))
    );
    let actions = env
        .agent_actions("target.sample.pulse")
        .expect("target exposes image action");
    assert_eq!(actions[0].action(), "action.inspect.pulse");
    assert_eq!(actions[0].return_type(), &TypeKind::ActionResult);
}

#[test]
fn project_index_does_not_project_unknown_call_actions() {
    let tree = parse_project_index_fixture(
        r#"
flow opening {
    mystery_present(asset = @asset:.bg.pulse, target = "target.sample.pulse", action = "action.inspect.pulse")
}
"#,
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("unknown call lowers to HIR");
    let document = document_for_hir(&hir);
    let index = project_semantic_index_from_hir(&hir, ProgramHash::new("program-a"), &document)
        .expect("invalid call remains indexable without projecting image metadata");

    assert!(
        index
            .typecheck_env()
            .agent_actions("target.sample.pulse")
            .is_none()
    );
}

#[test]
fn project_index_projects_agent_action_signatures() {
    let index = ProjectSemanticIndex::new(ProgramHash::new("program-a")).with_entity(
        EntitySymbol::new(
            public_id("activity.inventory"),
            EntityType::new(EntityKind::Activity, None),
            test_source_anchor(),
            SemanticHash::new("shape.activity.inventory.v1"),
        )
        .with_agent_action(AgentActionSignature::new(
            QualifiedName::new("open"),
            [AgentActionParam::required("label", TypeKind::String)],
            TypeKind::ActionResult,
        )),
    );
    let env = index.typecheck_env();
    let actions = env
        .agent_actions("activity.inventory")
        .expect("agent action projected");

    assert_eq!(actions[0].action(), "open");
    assert_eq!(actions[0].params()[0].name(), "label");
    assert_eq!(actions[0].params()[0].ty(), &TypeKind::String);
    assert_eq!(actions[0].return_type(), &TypeKind::ActionResult);
    assert_eq!(
        env.function_signature("invoke")
            .map(FunctionSignature::return_type),
        Some(&agent_result(TypeKind::ActionResult))
    );
}
