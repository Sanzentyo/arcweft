use super::support::*;

#[test]
fn stub_is_now_real_source_parser() {
    let tree = parse_flow_body_ok("alice: おはよう。[p]");
    assert_eq!(tree.items().len(), 1);
    assert!(matches!(flow_body(&tree), [FlowItem::SpeakerLine(_)]));
}

#[test]
fn parses_module_use_and_pub_flow() {
    let tree = parse_ok(
        r"
mod game.routes.opening

use game.prelude.*
 pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    bg(@asset:.bg.room, fade = 300ms)
    include @flow.alice_enters
}
",
    );

    assert_eq!(tree.module().expect("module").path(), "game.routes.opening");
    assert_eq!(tree.uses().len(), 1);
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow item");
    };
    assert_eq!(flow.visibility(), Some(Visibility::Public));
    assert_eq!(
        flow.id()
            .expect("flow id")
            .as_absolute()
            .expect("absolute flow id")
            .body(),
        "flow.opening"
    );
    let signature = flow.signature().expect("flow signature");
    assert!(ident_pattern(
        signature.param_groups()[0].params()[0].pattern(),
        "state"
    ));
    assert_eq!(flow.body().len(), 2);
    assert!(matches!(
        &flow.body()[0],
        FlowItem::Stmt(Stmt::Expr {
            expr: Expr::Call(_),
            ..
        })
    ));
    assert!(matches!(&flow.body()[1], FlowItem::Include(_)));
}

#[test]
fn flow_relative_decl_ids_normalize_like_implicit_names() {
    let tree = parse_ok(
        r"
flow @.opening {
    alice(id=@.hello): おはよう。[p]
}

flow @. prologue {
}

flow @flow:. routed {
}

flow named {
    alice(id=@.hello): おはよう。[p]
}

flow @flow:.intro {
}

flow @flow:. shared {
}
",
    );
    let hir = lower_to_hir(&tree).expect("relative flow ids lower");

    assert_eq!(hir.flows()[0].id().expect("flow id").body(), "flow.opening");
    assert_eq!(hir.flows()[0].name(), Some("opening"));
    assert_eq!(
        hir.flows()[1].id().expect("empty marker flow id").body(),
        "flow.prologue"
    );
    assert_eq!(hir.flows()[1].name(), Some("prologue"));
    assert_eq!(
        hir.flows()[2]
            .id()
            .expect("empty family marker flow id")
            .body(),
        "flow.routed"
    );
    assert_eq!(
        hir.flows()[3].id().expect("implicit flow id").body(),
        "flow.named"
    );
    assert_eq!(hir.flows()[4].id().expect("flow id").body(), "flow.intro");
    assert_eq!(
        hir.flows()[5].id().expect("empty marker flow id").body(),
        "flow.shared"
    );
    let HirFlowItem::Dialogue(line) = &hir.flows()[0].body()[0] else {
        panic!("expected dialogue");
    };
    assert_eq!(
        line.id().expect("line id").body(),
        "say.opening.alice.hello"
    );
}

#[test]
fn parses_staging_calls_as_expression_statements() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    show(@character.alice, .normal, at = .right, fade = 220ms)
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Stmt(Stmt::Expr {
        expr: Expr::Call(call),
        ..
    }) = &flow.body()[0]
    else {
        panic!("expected call statement");
    };
    assert!(matches!(call.callee(), Expr::Path(path) if path == "show"));
    assert!(matches!(
        &call.args()[0],
        CallArg::Positional(value) if matches!(value.as_ref(), Expr::EntityRef(_))
    ));
    assert!(matches!(
        &call.args()[2],
        CallArg::Named { name, value } if name == "at" && matches!(value.as_ref(), Expr::ShortVariant(path) if path == "right")
    ));
    assert!(matches!(
        &call.args()[3],
        CallArg::Named { name, value } if name == "fade" && matches!(value.as_ref(), Expr::Literal(_))
    ));
}

#[test]
fn parses_delimited_entity_refs_with_semantic_hashes() {
    let tree = parse_ok(
        r"
flow @<flow.alice_intro@sem:b3_9f2a1c> opening {
    include @<flow.alice_enters@sem:f0_00aa>
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let id = flow
        .id()
        .expect("flow id")
        .as_absolute()
        .expect("absolute flow id");
    assert!(id.is_delimited());
    assert_eq!(id.body(), "flow.alice_intro@sem:b3_9f2a1c");
    let FlowItem::Include(included_flow) = &flow.body()[0] else {
        panic!("expected include");
    };
    assert!(included_flow.is_delimited());
    assert_eq!(included_flow.body(), "flow.alice_enters@sem:f0_00aa");
}

#[test]
fn lowers_family_relative_entity_refs_in_general_reference_contexts() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    scope prologue {
        include @flow:.alice_enters
    }
}
",
    );

    let hir = lower_to_hir(&tree).expect("family-relative include lowers");
    let HirFlowItem::Scope(scope) = &hir.flows()[0].body()[0] else {
        panic!("expected scope");
    };
    let HirFlowItem::Include(include) = &scope.body()[0] else {
        panic!("expected include");
    };
    assert_eq!(include.body(), "flow.alice_enters");
}

#[test]
fn rejects_unqualified_relative_entity_refs_in_general_reference_contexts() {
    let errors = parse_errors(
        r"
flow @flow.opening opening {
    include @.next
}
",
    );

    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("relative entity references must include a family")),
        "expected relative entity ref diagnostic, got {errors:?}"
    );
}

#[test]
fn parses_source_locale_block() {
    let tree = parse_flow_body_ok(
        r"
source locale en-US {
    alice(id=@say.opening.alice.english_quote):
        Good morning.[p]
}
",
    );

    let [FlowItem::SourceLocale(block)] = flow_body(&tree) else {
        panic!("expected source locale block");
    };
    assert_eq!(block.locale(), "en-US");
    assert_eq!(block.body().len(), 1);
}

#[test]
fn rejects_relative_id_syntax_in_module_and_use_paths() {
    for source in [
        "mod @.routes::opening",
        "use @.characters::{alice}",
        "mod @.routes",
        "use @super.characters::{alice}",
    ] {
        let errors = parse_errors(source);
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("relative ID syntax")),
            "expected relative-id diagnostic for {source:?}, got {errors:?}"
        );
    }
}

#[test]
fn lints_deep_dot_run_relative_ids_and_module_flow_mismatch() {
    let tree = parse_ok(
        r"
mod game.routes.opening

flow @flow.title title {
    scope outer {
        scope inner {
            alice(id=@...shared): おはよう。[p]
        }
    }
}
",
    );
    let lints = lint_id_policy(&tree);

    assert!(
        lints
            .iter()
            .any(|lint| lint.code() == SyntaxLintCode::DeepDotRunRelativeId)
    );
    assert!(
        lints
            .iter()
            .any(|lint| lint.code() == SyntaxLintCode::FlowIdModuleMismatch)
    );
    assert!(
        lints
            .iter()
            .any(|lint| lint.code() == SyntaxLintCode::RedundantDeclIdentity)
    );
}

#[test]
fn lints_declaration_identity_mismatch_and_respects_redundant_allow() {
    let tree = parse_ok(
        r"
#[allow(style::redundant_decl_identity)]
flow @flow.opening opening {
}

flow @flow.opening start {
}
",
    );
    let lints = lint_id_policy(&tree);

    assert!(
        !lints
            .iter()
            .any(|lint| lint.code() == SyntaxLintCode::RedundantDeclIdentity)
    );
    assert!(
        lints
            .iter()
            .any(|lint| lint.code() == SyntaxLintCode::DeclBindingMismatch)
    );
}

#[test]
fn normalizes_parent_module_root_alias() {
    let tree = parse_ok(
        r"
mod parent.shared
use parent.common.{route_gate}
",
    );

    assert_eq!(tree.module().expect("module").path(), "super.shared");
    assert_eq!(tree.uses()[0].tree().source(), "super.common.{route_gate}");
}

#[test]
fn removed_memo_block_does_not_reach_typechecked_hir() {
    let parsed = parse_source(
        r"
flow @flow.memo memo_example {
    let value = memo(scope=scene, key=(score)) {
        let next = score
        next
    }
    goto @flow.title
}

flow @flow.title title {}
",
    );
    let rejected = !parsed.errors().is_empty()
        || lower_to_hir(parsed.typed_tree()).map_or(true, |hir| {
            validate_typecheck_ready(&hir).is_err()
                || typecheck_hir(&hir, &TypeCheckEnv::new()).is_err()
        });
    assert!(
        rejected,
        "removed memo block must not reach typed execution"
    );
}

#[test]
fn parses_attributes_and_wiki_links() {
    let tree = parse_ok(
        r"
#![generated(tool)]
/// links to [[flow.alice_intro]]
#[derive(Debug)]
    flow @flow.opening opening {}
",
    );

    assert_eq!(tree.attrs().len(), 1);
    assert_eq!(tree.attrs()[0].name(), "generated");
    assert_eq!(tree.attrs()[0].args(), Some("tool"));
    assert_eq!(tree.wiki_links()[0].body(), "flow.alice_intro");
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    assert_eq!(flow.attrs().len(), 1);
    assert_eq!(flow.attrs()[0].name(), "derive");
}
