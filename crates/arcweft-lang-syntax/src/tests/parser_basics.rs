use super::support::*;

#[test]
fn stub_is_now_real_source_parser() {
    let tree = parse_stub("alice: おはよう。[p]").expect("speaker line parses");
    assert_eq!(tree.items().len(), 1);
    assert!(matches!(
        &tree.items()[0],
        Item::FlowItem(FlowItem::SpeakerLine(_))
    ));
}

#[test]
fn parses_module_use_and_pub_flow() {
    let tree = parse_source(
        r"
mod game::routes::opening

use game::prelude::*
pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    @bg #asset.bg.room fade=300ms
    include #frag.alice_enters
}
",
    )
    .expect("module, use, and flow parse");

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
    assert!(
        matches!(&flow.body()[0], FlowItem::ScenarioCommand(command) if command.args().len() == 2)
    );
    assert!(matches!(&flow.body()[1], FlowItem::Include(_)));
}

#[test]
fn parses_scenario_command_args_as_expressions() {
    let tree = parse_source(
        r"
flow #flow.opening opening {
    @show alice normal at=right fade=220ms
}
",
    )
    .expect("scenario command args parse");

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::ScenarioCommand(command) = &flow.body()[0] else {
        panic!("expected scenario command");
    };
    assert_eq!(command.name(), "show");
    assert!(matches!(&command.args()[0], Expr::Path(path) if path == "alice"));
    assert!(matches!(
        &command.args()[2],
        Expr::NamedArg { name, value } if name == "at" && matches!(value.as_ref(), Expr::Path(path) if path == "right")
    ));
    assert!(matches!(
        &command.args()[3],
        Expr::NamedArg { name, value } if name == "fade" && matches!(value.as_ref(), Expr::Literal(_))
    ));
}

#[test]
fn parses_delimited_entity_refs_with_semantic_hashes() {
    let tree = parse_source(
        r"
flow #<flow.alice_intro@sem:b3_9f2a1c> opening {
    include #<frag.alice_enters@sem:f0_00aa>
}
",
    )
    .expect("delimited entity refs parse");

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
fn parses_source_locale_block() {
    let tree = parse_source(
        r"
source locale en-US {
    alice(id=#say.opening.alice.english_quote):
        Good morning.[p]
}
",
    )
    .expect("source locale block parses");

    let Item::FlowItem(FlowItem::SourceLocale(block)) = &tree.items()[0] else {
        panic!("expected source locale block");
    };
    assert_eq!(block.locale(), "en-US");
    assert_eq!(block.body().len(), 1);
}

#[test]
fn rejects_relative_id_syntax_in_module_and_use_paths() {
    for source in ["mod .routes::opening", "use .characters::{alice}"] {
        let errors = parse_source(source).expect_err("relative module path is rejected");
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("relative `.suffix` ID syntax")),
            "expected relative-id diagnostic for {source:?}, got {errors:?}"
        );
    }
}

#[test]
fn normalizes_parent_module_root_alias() {
    let tree = parse_source(
        r"
mod parent::shared
lazy use parent::common::{route_gate}
",
    )
    .expect("parent module root parses as alias");

    assert_eq!(tree.module().expect("module").path(), "super::shared");
    assert_eq!(tree.uses()[0].tree(), "super::common::{route_gate}");
}

#[test]
fn parses_and_typechecks_memo_expression_block_binding() {
    let tree = parse_source(
        r"
flow #flow.memo memo_example {
    let value = memo(scope=scene, key=(score)) {
        let next = score
        next
    }
    goto #flow.title
}

flow #flow.title title {}
",
    )
    .expect("memo expression block fixture parses");

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
    .expect("memo expression block typechecks");
}

#[test]
fn parses_attributes_and_wiki_links() {
    let tree = parse_source(
        r"
/// links to [[flow.alice_intro]]
@derive(Debug)
flow #flow.opening opening {}
",
    )
    .expect("attributes and wiki links parse");

    assert_eq!(tree.wiki_links()[0].body(), "flow.alice_intro");
    assert!(matches!(&tree.items()[0], Item::Attribute(attr) if attr.name() == "derive"));
    assert!(matches!(&tree.items()[1], Item::Flow(_)));
}
