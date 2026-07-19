use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::document::parse_shadow_document;
use crate::grammar::build::{GrammarBuild, UnattachedGrammarEntry};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/entry-shadow").unwrap(),
        SourceName::path("entry-shadow.arcw"),
        text,
    )
    .unwrap()
}

fn parse(text: &str) -> GrammarBuild {
    parse_shadow_document(&document(text)).unwrap()
}

fn kind_count(entries: &[UnattachedGrammarEntry], kind: SyntaxKind) -> usize {
    entries.iter().filter(|entry| entry.kind() == kind).count()
}

fn kind_roles(entries: &[UnattachedGrammarEntry], kind: SyntaxKind) -> Vec<SyntaxRole> {
    entries
        .iter()
        .filter(|entry| entry.kind() == kind)
        .map(UnattachedGrammarEntry::role)
        .collect()
}

fn source_range(source: &str, fragment: &str) -> SourceRange {
    let start = source.find(fragment).expect("fixture fragment");
    SourceRange::new(start, start + fragment.len())
}

#[test]
fn stateful_entry_emits_typed_roles_and_goto_losslessly() {
    let source = r"/// Main game launch.
#[launch(primary)]
pub entry game @entry.game.main {
    state = GameState
    initializer = game.initial_state
    event = GameEvent
    reducer = game.reduce
    goto @flow.opening
}
";
    let built = parse(source);
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::EntryDeclarationItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::DocBlock), 1);
    assert_eq!(kind_count(entries, SyntaxKind::OuterAttribute), 1);
    assert_eq!(kind_count(entries, SyntaxKind::Visibility), 1);
    assert_eq!(kind_count(entries, SyntaxKind::EntryBody), 1);
    assert_eq!(
        kind_roles(entries, SyntaxKind::EntryRoleBinding),
        vec![
            SyntaxRole::Element(0),
            SyntaxRole::Element(1),
            SyntaxRole::Element(2),
            SyntaxRole::Element(3),
        ]
    );
    assert_eq!(
        kind_roles(entries, SyntaxKind::EntryGoto),
        vec![SyntaxRole::Element(4)]
    );
    assert_eq!(kind_count(entries, SyntaxKind::PathType), 2);
    assert_eq!(
        kind_roles(entries, SyntaxKind::Path),
        vec![
            SyntaxRole::Target,
            SyntaxRole::Initializer,
            SyntaxRole::Target,
            SyntaxRole::Initializer,
        ]
    );
    assert_eq!(
        kind_roles(entries, SyntaxKind::EntityReferenceExpression),
        vec![SyntaxRole::Target]
    );
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn agent_entry_emits_controller_path_losslessly() {
    let source = "entry agent @entry.agent.smoke {\n    controller = agents.opening_smoke\n}\n";
    let built = parse(source);
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::EntryDeclarationItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::EntryRoleBinding), 1);
    assert_eq!(
        kind_roles(entries, SyntaxKind::Path),
        vec![SyntaxRole::Initializer]
    );
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn server_routes_emit_method_path_target_and_bindings() {
    let source = r#"entry server @entry.http {
    route GET "/health" -> @flow.health
    route GET "/hello/:name" -> @flow.hello(name = :name)
}
"#;
    let built = parse(source);
    let entries = built.index().entries();

    assert_eq!(
        kind_roles(entries, SyntaxKind::EntryRoute),
        vec![SyntaxRole::Element(0), SyntaxRole::Element(1)]
    );
    assert_eq!(kind_count(entries, SyntaxKind::LiteralExpression), 2);
    assert_eq!(
        kind_roles(entries, SyntaxKind::EntityReferenceExpression),
        vec![SyntaxRole::Target, SyntaxRole::Target]
    );
    assert_eq!(
        kind_roles(entries, SyntaxKind::EntryRouteBinding),
        vec![SyntaxRole::Argument(0)]
    );
    assert_eq!(kind_count(entries, SyntaxKind::OpenParenNode), 1);
    assert_eq!(kind_count(entries, SyntaxKind::CloseParenNode), 1);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn entry_options_use_shared_expression_grammar() {
    let source = r"entry cli @entry.cli.main {
    goto @flow.cli_main;
    budget = policy(1 + 2 * 3)
}
";
    let built = parse(source);
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::EntryGoto), 1);
    assert_eq!(
        kind_roles(entries, SyntaxKind::EntryOption),
        vec![SyntaxRole::Element(1)]
    );
    assert_eq!(kind_count(entries, SyntaxKind::CallExpression), 1);
    assert_eq!(kind_count(entries, SyntaxKind::CallArgument), 1);
    assert_eq!(kind_count(entries, SyntaxKind::BinaryExpression), 2);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn entry_head_diagnostics_own_exact_ranges() {
    let missing_kind = "entry @entry.game.main {}\n";
    let built = parse(missing_kind);
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.entry.missing_kind"
            && diagnostic.range() == source_range(missing_kind, "@entry.game.main")
    }));
    assert_eq!(built.green().to_string(), missing_kind);

    let missing_id = "entry game {}\n";
    let built = parse(missing_id);
    let body = missing_id.find('{').unwrap();
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.entry.missing_id"
            && diagnostic.range() == SourceRange::new(body, body)
    }));
    assert_eq!(built.green().to_string(), missing_id);

    let wrong_family = "entry game @flow.main {}\n";
    let built = parse(wrong_family);
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.entry.id_family"
            && diagnostic.range() == source_range(wrong_family, "@flow.main")
    }));
    assert_eq!(built.green().to_string(), wrong_family);

    let trailing = "entry game @entry.game.main trailing {}\n";
    let built = parse(trailing);
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.entry.trailing_head"
            && diagnostic.range() == source_range(trailing, "trailing")
    }));
    assert_eq!(built.green().to_string(), trailing);
}

#[test]
fn malformed_role_and_route_keep_later_members() {
    let source = r#"entry server @entry.http {
    controller agents.smoke
    route GET "/hello/:name" -> @flow.hello(name = :name
    fallback = policy.default
    route GET "/health" -> @flow.health
}
"#;
    let built = parse(source);
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::EntryRoleBinding), 1);
    assert_eq!(kind_count(entries, SyntaxKind::EntryRoute), 2);
    assert_eq!(kind_count(entries, SyntaxKind::EntryOption), 1);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.entry.role_binding" })
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.entry.route_binding_close" })
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn malformed_entry_body_recovers_before_following_proof() {
    let source = concat!(
        "entry cli @entry.cli.main {\n",
        "    goto @flow.cli_main\n",
        "/// The proof keeps its documentation.\n",
        "#[verify]\n",
        "proof next() = ()\n",
    );
    let proof_prefix = source.find("/// The proof").unwrap();
    let built = parse(source);
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::EntryDeclarationItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::DocBlock), 1);
    assert_eq!(kind_count(entries, SyntaxKind::OuterAttribute), 1);
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.entry.missing_body_close"
            && diagnostic.range() == SourceRange::new(proof_prefix, proof_prefix)
    }));
    assert!(
        built
            .missing_tokens()
            .iter()
            .any(|missing| missing.at() == proof_prefix)
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_entry_body_does_not_consume_following_proof() {
    let source = "entry cli @entry.cli.main\nproof next() = ()\n";
    let proof = source.find("proof next").unwrap();
    let built = parse(source);
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::EntryDeclarationItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::MissingBody), 1);
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.entry.missing_body"
            && diagnostic.range() == SourceRange::new(proof, proof)
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn invalid_entry_member_is_ordinary_recovery() {
    let source = r"entry cli @entry.cli.main {
    unsupported @flow.legacy
    goto @flow.current
}
";
    let built = parse(source);
    let entries = built.index().entries();
    let diagnostic = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.entry.invalid_member")
        .expect("ordinary invalid-member diagnostic");

    assert_eq!(
        diagnostic.range(),
        source_range(source, "unsupported @flow.legacy")
    );
    assert_eq!(
        kind_roles(entries, SyntaxKind::ErrorNode)
            .into_iter()
            .filter(|role| matches!(role, SyntaxRole::Element(_)))
            .collect::<Vec<_>>(),
        vec![SyntaxRole::Element(0)]
    );
    assert_eq!(
        kind_roles(entries, SyntaxKind::EntryGoto),
        vec![SyntaxRole::Element(1)]
    );
    assert!(built.diagnostics().iter().all(|diagnostic| {
        let message = diagnostic.message().to_ascii_lowercase();
        !message.contains("removed") && !message.contains("deprecated")
    }));
    assert_eq!(built.green().to_string(), source);
}
