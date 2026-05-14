use super::support::*;

#[test]
fn stub_is_now_real_source_parser() {
    let tree = parse_ok("alice: おはよう。[p]");
    assert_eq!(tree.items().len(), 1);
    assert!(matches!(
        &tree.items()[0],
        Item::FlowItem(item) if matches!(item.as_ref(), FlowItem::SpeakerLine(_))
    ));
}

#[test]
fn parses_module_use_and_pub_flow() {
    let tree = parse_ok(
        r"
mod game::routes::opening

use game::prelude::*
 pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    bg(@asset.bg.room, fade = 300ms)
    include @frag.alice_enters
}
",
    );

    assert_eq!(
        tree.module().expect("module").path(),
        "game::routes::opening"
    );
    assert_eq!(tree.uses().len(), 1);
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow item");
    };
    assert_eq!(flow.visibility(), Some(Visibility::Public));
    assert_eq!(flow.kind(), FlowKind::Flow);
    assert_eq!(flow.id().expect("flow id").body(), "flow.opening");
    let signature = flow.signature().expect("flow signature");
    assert!(ident_pattern(
        signature.param_groups()[0].params()[0].pattern(),
        "state"
    ));
    assert_eq!(flow.body().len(), 2);
    assert!(matches!(
        &flow.body()[0],
        FlowItem::Stmt(Stmt::Expr(Expr::Call { .. }))
    ));
    assert!(matches!(&flow.body()[1], FlowItem::Include(_)));
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
    let FlowItem::Stmt(Stmt::Expr(Expr::Call { callee, args })) = &flow.body()[0] else {
        panic!("expected call statement");
    };
    assert!(matches!(callee.as_ref(), Expr::Path(path) if path == "show"));
    assert!(matches!(&args[0], Expr::EntityRef(_)));
    assert!(matches!(
        &args[2],
        Expr::NamedArg { name, value } if name == "at" && matches!(value.as_ref(), Expr::Path(path) if path == ".right")
    ));
    assert!(matches!(
        &args[3],
        Expr::NamedArg { name, value } if name == "fade" && matches!(value.as_ref(), Expr::Literal(_))
    ));
}

#[test]
fn rejects_legacy_staging_command_sugar() {
    let errors = parse_errors(
        r"
flow @flow.opening opening {
    bg @asset.bg.room fade=300ms
}
",
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("canonical function-call syntax")),
        "expected canonical call diagnostic, got {errors:?}"
    );
}

#[test]
fn parses_delimited_entity_refs_with_semantic_hashes() {
    let tree = parse_ok(
        r"
flow @<flow.alice_intro@sem:b3_9f2a1c> opening {
    include @<frag.alice_enters@sem:f0_00aa>
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let id = flow.id().expect("flow id");
    assert!(id.is_delimited());
    assert_eq!(id.body(), "flow.alice_intro@sem:b3_9f2a1c");
    let FlowItem::Include(fragment) = &flow.body()[0] else {
        panic!("expected include");
    };
    assert!(fragment.is_delimited());
    assert_eq!(fragment.body(), "frag.alice_enters@sem:f0_00aa");
}

#[test]
fn lowers_family_relative_entity_refs_in_general_reference_contexts() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    scope prologue {
        include @frag:.alice_enters
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
    assert_eq!(include.body(), "frag.opening.prologue.alice_enters");
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
    let tree = parse_ok(
        r"
source locale en-US {
    alice(id=@say.opening.alice.english_quote):
        Good morning.[p]
}
",
    );

    let Item::FlowItem(item) = &tree.items()[0] else {
        panic!("expected source locale block");
    };
    let FlowItem::SourceLocale(block) = item.as_ref() else {
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
mod game::routes::opening

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
}

#[test]
fn normalizes_parent_module_root_alias() {
    let tree = parse_ok(
        r"
mod parent::shared
lazy use parent::common::{route_gate}
",
    );

    assert_eq!(tree.module().expect("module").path(), "super::shared");
    assert_eq!(tree.uses()[0].tree(), "super::common::{route_gate}");
}

#[test]
fn parses_and_typechecks_memo_expression_block_binding() {
    let tree = parse_ok(
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

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Stmt(Stmt::Let {
        expr:
            Expr::MemoBlock {
                options,
                statements,
                value: Some(_),
            },
        ..
    }) = &flow.body()[0]
    else {
        panic!("expected memo expression block binding");
    };
    assert_eq!(options.len(), 2);
    assert_eq!(statements.len(), 1);

    let hir = lower_to_hir(&tree).expect("memo expression block fixture lowers");
    validate_typecheck_ready(&hir).expect("memo expression block is typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("score", TypeKind::Int)
            .with_symbol("scene", TypeKind::Named("MemoScope".to_owned())),
    )
    .expect("typecheck succeeds");
}

#[test]
fn parses_attributes_and_wiki_links() {
    let tree = parse_ok(
        r"
/// links to [[flow.alice_intro]]
#[derive(Debug)]
    flow @flow.opening opening {}
",
    );

    assert_eq!(tree.wiki_links()[0].body(), "flow.alice_intro");
    assert!(matches!(&tree.items()[0], Item::Attribute(attr) if attr.name() == "derive"));
    assert!(matches!(&tree.items()[1], Item::Flow(_)));
}
