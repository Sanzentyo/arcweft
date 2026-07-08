use super::support::*;

#[test]
fn parses_flow_contracts_before_body_block() {
    let tree = parse_ok(
        r"
pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError>
requires delta >= -100 && delta <= 100
ensures check result.affection[character] >= 0
requires progress in 0.0..=1.0
effects { asset.read, view.show }
ensures no_effect network.request
{
    goto @flow.title
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    assert_eq!(flow.contracts().len(), 5);
    assert!(matches!(
        &flow.contracts()[0],
        ContractClause::Requires {
            expr: Expr::Binary { .. },
            ..
        }
    ));
    assert!(matches!(
        &flow.contracts()[1],
        ContractClause::Ensures {
            mode: Some(mode),
            expr: Expr::Binary { .. },
        } if mode == "check"
    ));
    assert!(matches!(&flow.contracts()[4], ContractClause::NoEffect(_)));
    assert!(matches!(&flow.body()[0], FlowItem::Stmt(Stmt::Goto(_))));
}

#[test]
fn parses_documented_contract_clauses_and_logical_ops() {
    let tree = parse_ok(
        r"
pub fn add_affection(character: Ref<Character>, delta: i32)(state: GameState) -> GameState
requires delta >= -100 && delta <= 100
ensures no_effect network.request
invariant affection_bounds_ok
reads state.affection[character]
modifies state.affection[character]
assume external_plugin_is_deterministic
{
    state
}
",
    );

    let Item::Function(function) = &tree.items()[0] else {
        panic!("expected function item");
    };
    assert_eq!(function.contracts().len(), 6);
    assert!(matches!(
        &function.contracts()[0],
        ContractClause::Requires {
            expr: Expr::Binary {
                op: BinaryOp::And,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        &function.contracts()[1],
        ContractClause::NoEffect(expr) if expr_path_eq(expr, "network.request")
    ));
    assert!(matches!(
        &function.contracts()[2],
        ContractClause::Invariant {
            expr: Expr::Path(path),
            ..
        } if path == "affection_bounds_ok"
    ));
    assert!(matches!(&function.contracts()[3], ContractClause::Reads(_)));
    assert!(matches!(
        &function.contracts()[5],
        ContractClause::Assume {
            expr: Expr::Path(path)
        } if path == "external_plugin_is_deterministic"
    ));
}

#[test]
fn parses_function_item_with_lifetimes_and_contracts() {
    let tree = parse_ok(
        r"
pub fn first<'a>(xs: &'a [ChoiceView]) -> Option<&'a ChoiceView>
requires xs.len() > 0
ensures check result.is_some()
effects { asset.read }
{
    xs[0]
}
",
    );

    let Item::Function(function) = &tree.items()[0] else {
        panic!("expected function item");
    };
    assert_eq!(function.visibility(), Some(Visibility::Public));
    assert_eq!(function.signature().name(), "first");
    assert_eq!(
        function.signature().generic_params()[0]
            .as_lifetime()
            .expect("lifetime generic")
            .name(),
        "a"
    );
    assert!(function.signature_text().contains("Option<&'a ChoiceView>"));
    assert_eq!(function.contracts().len(), 3);
    assert!(matches!(
        &function.contracts()[0],
        ContractClause::Requires {
            expr: Expr::Binary { .. },
            ..
        }
    ));
    assert!(function.body().contains("xs[0]"));
    assert!(function.body_statements().is_empty());
    assert!(matches!(function.body_value(), Some(Expr::Index { .. })));
}

#[test]
fn typechecks_flow_contract_expressions() {
    let tree = parse_ok(
        r"
pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError>
requires delta >= -100 && delta <= 100
ensures check result.affection[character] >= 0
effects { asset.read, view.show }
ensures no_effect network.request
{
    goto @flow.title
}
",
    );
    let hir = lower_to_hir(&tree).expect("contract typecheck fixture lowers");
    let env = TypeCheckEnv::new()
        .with_symbol("delta", TypeKind::I64)
        .with_symbol("progress", TypeKind::F64)
        .with_symbol(
            "result.affection",
            TypeKind::Named("OrderedMap<Character, i64>".to_owned()),
        )
        .with_symbol("character", TypeKind::entity_ref(EntityKind::Character))
        .with_symbol("asset.read", TypeKind::Named("Effect".to_owned()))
        .with_symbol("view.show", TypeKind::Named("Effect".to_owned()))
        .with_symbol("network.request", TypeKind::Named("Effect".to_owned()))
        .with_index(
            TypeKind::Named("OrderedMap<Character, i64>".to_owned()),
            TypeKind::I64,
        );

    typecheck_hir(&hir, &env).expect("contract expressions typecheck");
}
