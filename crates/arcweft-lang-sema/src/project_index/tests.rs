use super::agent_prelude::{agent_prelude_callables, agent_result};
use super::*;
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_syntax::parser::parse_source;
use arcweft_source::{SourceAnchor, SourceDocument, SourceDocumentId, SourceName, SourceRange};

fn public_id(value: &str) -> PublicId {
    PublicId::try_new(value).expect("valid public id")
}

fn document_for_hir(hir: &arcweft_lang_hir::model::HirModule, path: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new(path).expect("test document id"),
        SourceName::path(path),
        " ".repeat(hir.source_len().unwrap_or_default()),
    )
    .expect("test source document")
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
    let tree = parse_source(
        r#"
entry game @entry.main {
    goto @flow.opening
    goto @flow.listen
}

signal @signal.current_flow: Watch<Ref<Flow>>

flow @flow.opening opening {
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

flow @flow.listen listen {
    return "listen"
}
"#,
    )
    .into_typed_tree();
    let hir = lower_to_hir(&tree).expect("source lowers");
    let document = document_for_hir(&hir, "test.arcw");
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
    let tree = parse_source(
        r#"
content chapter_two {
    roots = [ @flow.chapter_two, @asset:.bg ]
}

asset bg {
    file = "bg/chapter-two.png"
    kind = image
}

flow chapter_two {
    return "chapter two"
}
"#,
    )
    .into_typed_tree();
    let hir = lower_to_hir(&tree).expect("source lowers");
    let document = document_for_hir(&hir, "test.arcw");
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
fn project_index_from_hir_preserves_flow_and_signal_ref_value_types() {
    let tree = parse_source(
        r#"
signal @signal.current_flow: Watch<Ref<Flow>>
flow @flow.opening opening {
    return "ok"
}
"#,
    )
    .into_typed_tree();
    let hir = lower_to_hir(&tree).expect("source lowers to HIR");
    let document = document_for_hir(&hir, "game.arcw");
    let index = project_semantic_index_from_hir(&hir, ProgramHash::new("program-a"), &document)
        .expect("HIR indexes for Agent Script");

    assert_eq!(
        index.typecheck_env().symbol_type("flow.opening"),
        Some(&TypeKind::entity_ref(EntityKind::Flow))
    );
    assert_eq!(
        index.typecheck_env().symbol_type("signal.current_flow"),
        Some(&TypeKind::entity_ref_with_value(
            EntityKind::Signal,
            TypeKind::entity_ref(EntityKind::Flow)
        ))
    );
}

#[test]
fn project_index_from_hir_preserves_view_text_control_inputs() {
    let tree = parse_source(
        r#"
view FeedbackForm() {
    TextField(id: @input:.feedback, value: "")
}
"#,
    )
    .into_typed_tree();
    let hir = lower_to_hir(&tree).expect("source lowers to HIR");
    let document = document_for_hir(&hir, "game.arcw");
    let index = project_semantic_index_from_hir(&hir, ProgramHash::new("program-a"), &document)
        .expect("HIR indexes for Agent Script");

    assert_eq!(
        index.typecheck_env().symbol_type("input.feedback"),
        Some(&TypeKind::entity_ref(EntityKind::Input))
    );
}

#[test]
fn project_index_from_hir_preserves_project_callables_separately_from_agent_prelude() {
    let tree = parse_source(
        r#"
pub reducer update_route(state: GameState, event: GameEvent) -> GameState {
    let route = current_route(state)
    state
}

pub reducer current_route(state: GameState) -> Ref<Flow> {
    @flow.opening
}

flow @flow.opening opening {
    let route = current_route()
    goto @flow.done
    goto route
    return "ok"
}

flow @flow.done done {
    return "done"
}
"#,
    )
    .into_typed_tree();
    let hir = lower_to_hir(&tree).expect("source lowers to HIR");
    let document = document_for_hir(&hir, "game.arcw");
    let index = project_semantic_index_from_hir(&hir, ProgramHash::new("program-a"), &document)
        .expect("HIR indexes project callables");

    assert!(
        index
            .callable(&QualifiedName::new("update_route"))
            .is_none()
    );
    let reducer = index
        .project_callable(&QualifiedName::new("update_route"))
        .expect("reducer callable indexed");
    assert_eq!(reducer.kind(), ProjectCallableKind::Reducer);
    assert_eq!(reducer.signature().params().len(), 2);
    assert_eq!(
        reducer.signature().return_type(),
        &TypeKind::Named("GameState".to_owned())
    );
    assert!(
        reducer
            .semantic_hash()
            .as_str()
            .contains("hir:callable:reducer:update_route")
    );

    let current_route = index
        .project_callable(&QualifiedName::new("current_route"))
        .expect("current_route callable indexed");
    assert_eq!(current_route.kind(), ProjectCallableKind::Reducer);
    assert_eq!(
        current_route.signature().return_type(),
        &TypeKind::entity_ref(EntityKind::Flow)
    );
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
    assert!(relations.iter().any(|(from, to, kind)| {
        matches!(from, ProjectGraphSymbolRef::Callable(name) if name.as_str() == "current_route")
            && matches!(to, ProjectGraphSymbolRef::Entity(id) if id.as_str() == "flow.opening")
            && *kind == "references_entity"
    }));
    let control = index
        .flow_control_summary(&public_id("flow.opening"))
        .expect("flow control summary indexed");
    assert_eq!(control.static_goto_count(), 1);
    assert_eq!(control.dynamic_goto_count(), 1);
    assert!(control.has_dynamic_control());
}

#[test]
fn project_index_from_hir_projects_inline_image_agent_actions() {
    let tree = parse_source(
            r#"
asset bg.pulse {
    file = "bg/pulse.gif"
    kind = image
}

flow @flow.opening opening {
    let pulse = image(asset = @asset:.bg.pulse, target = "target.sample.pulse", layer = "layer.foreground", x = 96px, y = 72px, width = 360px, height = 180px, action = "action.inspect.pulse")
}
"#,
        )
        .into_typed_tree();
    let hir = lower_to_hir(&tree).expect("source lowers to HIR");
    let document = document_for_hir(&hir, "game.arcw");
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
    let tree = parse_source(
        r#"
flow @flow.opening opening {
    mystery_present(asset = @asset:.bg.pulse, target = "target.sample.pulse", action = "action.inspect.pulse")
}
"#,
    )
    .into_typed_tree();
    let hir = lower_to_hir(&tree).expect("unknown call lowers to HIR");
    let document = document_for_hir(&hir, "game.arcw");
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
