use super::support::*;

#[test]
fn parses_expression_shapes_needed_by_hir_lowering() {
    let pipe = parse_expr("state |> has_affection_at_least(@character.alice, 3)")
        .expect("pipe expr parses");
    assert!(matches!(pipe, Expr::Pipe { .. }));

    let method = parse_expr("choices.filter(_.enabled).map(_.label)").expect("method chain parses");
    assert!(matches!(method, Expr::MethodCall { .. }));
    let mutating_method = parse_expr("nums.reserve(4)").expect("mutating method call parses");
    assert!(matches!(mutating_method, Expr::MethodCall { method, .. } if method == "reserve"));
    let Expr::MethodCall { args, .. } = method else {
        panic!("expected outer map call");
    };
    assert!(matches!(
        args.as_slice(),
        [CallArg::Positional(Expr::Field {
            target,
            field
        })] if matches!(target.as_ref(), Expr::Placeholder(Placeholder::Partial)) && field == "label"
    ));

    let indexed = parse_expr("state.affection[@character.alice]").expect("index expr parses");
    assert!(matches!(indexed, Expr::Index { .. }));

    let dialogue_index =
        parse_expr("alice.say()[聞いて。[p]]").expect("bracket postfix expr parses");
    assert!(matches!(dialogue_index, Expr::Index { .. }));

    let timeline_offset = parse_expr("end-250ms").expect("timeline offset parses");
    assert!(matches!(timeline_offset, Expr::Binary { .. }));

    let placeholder = parse_expr("clamp(0, ^, 100)").expect("placeholder call parses");
    assert!(matches!(placeholder, Expr::Call { .. }));
    let partial = parse_expr("_.score >= ^").expect("partial comparison expression parses");
    assert!(matches!(partial, Expr::Binary { .. }));

    let list = parse_expr("[normal, smile, worried]").expect("bracket sequence parses");
    assert!(matches!(list, Expr::BracketSeq(items) if items.len() == 3));
    let empty_list = parse_expr("[]").expect("empty bracket sequence parses");
    assert!(matches!(empty_list, Expr::BracketSeq(items) if items.is_empty()));
    let nested_list = parse_expr("[@stem.piano, fade(0.2s, [slow, fast])]")
        .expect("nested bracket sequence parses");
    assert!(matches!(nested_list, Expr::BracketSeq(items) if items.len() == 2));
    let record_literal =
        parse_expr("{ player_name = state.player_name }").expect("record literal parses");
    assert!(matches!(record_literal, Expr::RecordLiteral(fields) if fields.len() == 1));
    let empty_record = parse_expr("{}").expect("empty record literal parses");
    assert!(matches!(empty_record, Expr::RecordLiteral(fields) if fields.is_empty()));

    let generic_collect = parse_expr("visible_choices.collect<Vec<ChoiceView>>()")
        .expect("generic method call parses");
    assert!(matches!(
        generic_collect,
        Expr::MethodCall { method, .. } if method == "collect<Vec<ChoiceView>>"
    ));

    let context_closure =
        parse_expr(r#"load_bg(id).with_context(|| "failed")?"#).expect("closure argument parses");
    assert!(matches!(context_closure, Expr::Try { .. }));

    let delimited =
        parse_expr("@<say.opening.dream_hint@sem:b3_9f2a1c>").expect("delimited ref expr parses");
    assert!(matches!(delimited, Expr::EntityRef(entity) if entity.is_delimited()));

    let range = parse_expr("0.0..=1.0").expect("inclusive float range parses");
    assert!(matches!(
        range,
        Expr::Range {
            inclusive: true,
            ..
        }
    ));

    let membership = parse_expr("progress in 0.0..=1.0").expect("range membership parses");
    assert!(matches!(
        membership,
        Expr::Binary {
            op: BinaryOp::In,
            ..
        }
    ));

    let unary_not = parse_expr("!event.is_relevant()").expect("unary not expr parses");
    assert!(matches!(
        unary_not,
        Expr::Unary {
            op: UnaryOp::Not,
            ..
        }
    ));

    let lifetime = parse_expr("'line.focus?").expect("lifetime registry expr parses");
    assert!(matches!(
        lifetime,
        Expr::LifetimePath {
            key,
            optional: true
        } if key.scope() == &LifetimeScopeKind::Line && key.path() == ["focus"]
    ));

    let merged = parse_expr(".smile & .casual & .motion.nod").expect("patch merge parses");
    assert!(matches!(
        merged,
        Expr::Binary {
            op: BinaryOp::Merge,
            ..
        }
    ));

    let thread = parse_expr("thread compute { route_score(state) }").expect("thread expr parses");
    assert!(matches!(thread, Expr::Thread { block } if block.name() == Some("compute")));
}

#[test]
fn parses_char_literal_suffixes() {
    let ascii = parse_expr(r#""a"c"#).expect("ascii char literal parses");
    assert!(matches!(
        ascii,
        Expr::Literal(Literal::Char {
            raw,
            value: 'a'
        }) if raw == "\"a\"c"
    ));

    let escaped = parse_expr(r#""\n"c"#).expect("escaped char literal parses");
    assert!(matches!(
        escaped,
        Expr::Literal(Literal::Char { value: '\n', .. })
    ));

    let unicode = parse_expr(r#""💡"c"#).expect("unicode scalar char literal parses");
    assert!(matches!(
        unicode,
        Expr::Literal(Literal::Char { value: '💡', .. })
    ));

    for invalid in ["\"\"c", r#""ab"c"#, r#""e\u{301}"c"#, r#""🇯🇵"c"#] {
        let error = parse_expr(invalid).expect_err("invalid char literal is rejected");
        assert!(
            error
                .to_string()
                .contains("exactly one Unicode scalar value"),
            "{invalid} produced {error}"
        );
    }

    assert!(parse_expr(r#""a"cat"#).is_err());
}
