use super::support::*;

#[test]
fn validates_hir_entity_references_against_registry() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    choice @choice.opening.first {
        @choice.opening.listen "聞く" -> @flow.alice_intro
    }
}

flow @flow.alice_intro alice_intro {
    goto @flow.opening
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("registry fixture lowers");
    let registry = registry_from_hir(&hir);

    validate_hir_references(&hir, &registry).expect("all local refs resolve");
}

#[test]
fn reports_unresolved_hir_entity_reference() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    goto @flow.missing
}
",
    );
    let hir = lower_to_hir(&tree).expect("missing ref fixture lowers");
    let registry = NameRegistry::new().with_entity("flow.opening", EntityKind::Flow);
    let errors = validate_hir_references(&hir, &registry).expect_err("missing ref should fail");

    assert!(errors[0].message().contains("flow.missing"));
}

#[test]
fn resolves_hir_entity_references_from_external_semantic_env() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    show(@character.zundamon)
}
",
    );
    let hir = lower_to_hir(&tree).expect("external symbol fixture lowers");
    let env = TypeCheckEnv::new().with_symbol(
        "character.zundamon",
        TypeKind::entity_ref(EntityKind::Character),
    );
    let registry = registry_from_hir_and_env(&hir, &env);

    validate_hir_references(&hir, &registry).expect("manifest-backed character resolves");
}

#[test]
fn resolves_component_view_text_control_inputs() {
    let tree = parse_ok(
        r#"
component FeedbackForm() {
    TextField(id: @input:.feedback, value: "")
}

flow @flow.submit submit {
    let submitted = text_submit @input.feedback
    return submitted
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("component input fixture lowers");
    let registry = registry_from_hir(&hir);

    validate_hir_references(&hir, &registry).expect("component text input resolves");
}

#[test]
fn resolves_declared_action_entity_references() {
    let tree = parse_ok(
        r#"
action feedback.submit(value: String)

flow @flow.submit submit {
    let target = @action.feedback.submit
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("action fixture lowers");
    let registry = registry_from_hir(&hir);

    validate_hir_references(&hir, &registry).expect("declared action entity resolves");
}

#[test]
fn collects_hir_symbol_uses_for_type_checking_without_reparsing() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    let (actor, (_, voice)) = alice.say()[聞いて。[p]]
    alice[
        #[fmt("夢", color=blue)]を見た。[p]
    ]
    with:
        at(0.42s): alice.stage.face(worried)
    choice @choice.opening.first {
        @choice.opening.listen "聞く" if state.affection[@character.alice] >= 3 -> @flow.alice_intro
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("symbol fixture lowers");
    let uses = collect_symbol_uses(&hir);

    assert!(
        uses.iter().any(
            |symbol| symbol.kind() == SymbolUseKind::DialogueCallee && symbol.name() == "alice"
        )
    );
    assert!(
        uses.iter()
            .any(|symbol| symbol.kind() == SymbolUseKind::Method && symbol.name() == "face")
    );
    assert!(
        uses.iter()
            .any(|symbol| symbol.kind() == SymbolUseKind::EntityRef
                && symbol.name() == "character.alice")
    );
    assert!(
        uses.iter()
            .all(|symbol| symbol.kind() != SymbolUseKind::RawExpr)
    );
    validate_typecheck_ready(&hir).expect("edge fixture is typecheck-ready");
}
