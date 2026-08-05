use super::support::*;

#[test]
fn flow_typed_statements_keep_patterns_and_exprs() {
    let tree = parse_ok(
        r"
flow opening {
    let (actor, (_, voice)) = alice.say()[聞いて。[p]]
    return Ok(FlowExit.Done)
    goto @flow.title
}
",
    );

    let Item::Flow(flow) = &tree.typed_tree().items()[0] else {
        panic!("expected flow");
    };
    assert!(matches!(
        &flow.body()[0],
        FlowItem::Stmt(Stmt::Let {
            pattern: Pattern::Tuple(_),
            expr: Expr::DialogueCall { .. },
            ..
        })
    ));
    assert!(matches!(
        &flow.body()[1],
        FlowItem::Stmt(Stmt::Return {
            expr: Expr::Call(_),
            ..
        })
    ));
    assert!(matches!(
        &flow.body()[2],
        FlowItem::Stmt(Stmt::Goto(target))
            if matches!(target.expr(), Expr::EntityRef(entity) if entity.body() == "flow.title")
    ));
}

#[test]
fn typed_patterns_keep_lifetime_borrow_types() {
    let tree = parse_ok(
        r"
flow borrow {
    let pixels: &'asset [Rgba8] = bg.pixels()
}
",
    );

    let Item::Flow(flow) = &tree.typed_tree().items()[0] else {
        panic!("expected flow");
    };
    assert!(matches!(
        &flow.body()[0],
        FlowItem::Stmt(Stmt::Let {
            pattern: Pattern::Ident(name),
            ty: Some(ty),
            ..
        }) if name == "pixels" && matches!(ty.value(), TypeRef::Reference(_))
    ));
}

#[test]
fn parses_documented_structured_pattern_shapes() {
    let tree = parse_ok(
        r"
flow patterns {
    let mut route = current_route
    let 42 = answer
    let @choice.opening.listen = selected
    let TruckResult { score, rank, .. } = result
    let [first, ..rest] = items
    let ev .ChoiceSelected { id } = event
}
",
    );

    let Item::Flow(flow) = &tree.typed_tree().items()[0] else {
        panic!("expected flow");
    };
    assert!(matches!(
        &flow.body()[0],
        FlowItem::Stmt(Stmt::Let {
            pattern: Pattern::MutIdent(name),
            ..
        }) if name == "route"
    ));
    assert!(matches!(
        &flow.body()[1],
        FlowItem::Stmt(Stmt::Let {
            pattern: Pattern::Literal(Expr::Literal(_)),
            ..
        })
    ));
    assert!(matches!(
        &flow.body()[2],
        FlowItem::Stmt(Stmt::Let {
            pattern: Pattern::Entity(entity),
            ..
        }) if entity.body() == "choice.opening.listen"
    ));
    assert!(matches!(
        &flow.body()[3],
        FlowItem::Stmt(Stmt::Let {
            pattern: Pattern::Record {
                path: Some(path),
                fields,
                rest: true,
            },
            ..
        }) if path == "TruckResult" && fields.len() == 2
    ));
    assert!(matches!(
        &flow.body()[4],
        FlowItem::Stmt(Stmt::Let {
            pattern: Pattern::BracketSeq {
                items,
                rest: Some(rest),
            },
            ..
        }) if items.len() == 1 && rest == "rest"
    ));
    let FlowItem::Stmt(Stmt::Let {
        pattern: Pattern::Whole { name, pattern },
        ..
    }) = &flow.body()[5]
    else {
        panic!("expected whole-pattern variant binding");
    };
    assert_eq!(name, "ev");
    assert!(matches!(
        pattern.as_ref(),
        Pattern::Variant {
            name,
            payload: Some(VariantPatternPayload::Record { fields, rest: false }),
            ..
        } if name == "ChoiceSelected" && fields.len() == 1 && fields[0].name() == "id"
    ));

    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("structured pattern fixture lowers");
    validate_typecheck_ready(&hir).expect("structured patterns do not introduce raw HIR");
}
