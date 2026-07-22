use super::support::*;

#[test]
fn parses_expression_shapes_needed_by_hir_lowering() {
    let pipe = parse_expr("state |> has_affection_at_least(@character.alice, 3)")
        .expect("pipe expr parses");
    assert!(matches!(pipe, Expr::Pipe { .. }));

    let method = parse_expr("choices.filter(_.enabled).map(_.label)").expect("method chain parses");
    assert_eq!(selected_call_member(&method), Some("map"));
    let mutating_method = parse_expr("nums.reserve(4)").expect("mutating method call parses");
    assert_eq!(selected_call_member(&mutating_method), Some("reserve"));
    let args = selected_call_args(&method).expect("expected outer map call");
    assert!(matches!(
        args,
        [CallArg::Positional(value)]
            if matches!(value.as_ref(), Expr::Select(select)
            if matches!(select.target(), Expr::Placeholder(Placeholder::Partial))
                && select.member().as_str() == "label"
            )
    ));

    let indexed = parse_expr("state.affection[@character.alice]").expect("index expr parses");
    assert!(matches!(indexed, Expr::Index { .. }));

    let call_index = parse_expr("alice.lines()[0]").expect("call result index parses");
    assert!(matches!(call_index, Expr::Index { .. }));

    let timeline_offset = parse_expr("end-250ms").expect("timeline offset parses");
    assert!(matches!(timeline_offset, Expr::Binary { .. }));

    let placeholder = parse_expr("clamp(0, ^, 100)").expect("placeholder call parses");
    assert!(matches!(placeholder, Expr::Call(_)));
    let partial = parse_expr("_.score >= ^").expect("partial comparison expression parses");
    assert!(matches!(partial, Expr::Binary { .. }));
    let grouped_partial =
        parse_expr("(_ > 80i64)").expect("parenthesized partial comparison expression parses");
    assert!(matches!(grouped_partial, Expr::Binary { .. }));

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
    assert_eq!(
        selected_call_member(&generic_collect),
        Some("collect<Vec<ChoiceView>>")
    );

    let context_closure =
        parse_expr(r#"load_bg(id).with_context(|| "failed")?"#).expect("closure argument parses");
    assert!(matches!(context_closure, Expr::Try(_)));

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
fn typechecker_lowers_pipe_placeholder_and_data_last_calls() {
    let tree = parse_ok(
        r"
struct Choice {
    label: String,
    enabled: bool,
}

flow main() -> i64 {
  let clamped = 10i64 |> clamp(0i64, ^, 100i64)
  let next = clamped |> plus_one
  let summed: i64 = 2i64 |> add(1i64)
  let named: i64 = 2i64 |> add(lhs = 1i64)
  let labels: Vec<String> = choices |> filter(|choice: Choice| -> bool { choice.enabled }) |> map(|choice: Choice| -> String { choice.label })
  log.info(labels)
  return next
}
",
    );
    let hir = lower_to_hir(&tree).expect("pipe fixture lowers to HIR");
    validate_typecheck_ready(&hir).expect("pipe expressions are typecheck-ready");
    let choice = TypeKind::Named("Choice".to_owned());
    let choices = TypeKind::Vec(Box::new(choice.clone()));
    let env = TypeCheckEnv::new()
        .with_function_signature(
            "clamp",
            FunctionSignature::new(
                TypeKind::I64,
                [
                    FunctionParam::required("min", TypeKind::I64),
                    FunctionParam::required("value", TypeKind::I64),
                    FunctionParam::required("max", TypeKind::I64),
                ],
            ),
        )
        .with_function_signature(
            "plus_one",
            FunctionSignature::new(
                TypeKind::I64,
                [FunctionParam::required("value", TypeKind::I64)],
            ),
        )
        .with_function_signature(
            "add",
            FunctionSignature::new(
                TypeKind::I64,
                [
                    FunctionParam::required("lhs", TypeKind::I64),
                    FunctionParam::required("rhs", TypeKind::I64),
                ],
            ),
        )
        .with_function_signature(
            "filter",
            FunctionSignature::new(
                TypeKind::function([choices.clone()], choices.clone()),
                [FunctionParam::required(
                    "predicate",
                    TypeKind::function([choice.clone()], TypeKind::Bool),
                )],
            )
            .with_remaining_param_groups([[FunctionParam::required("values", choices.clone())]]),
        )
        .with_function_signature(
            "map",
            FunctionSignature::new(
                TypeKind::function([choices.clone()], TypeKind::Vec(Box::new(TypeKind::String))),
                [FunctionParam::required(
                    "project",
                    TypeKind::function([choice], TypeKind::String),
                )],
            )
            .with_remaining_param_groups([[FunctionParam::required("values", choices.clone())]]),
        )
        .with_symbol("choices", choices);

    typecheck_hir(&hir, &env).expect("pipe expressions typecheck");
}

#[test]
fn typechecker_rejects_pipe_left_placeholder_outside_pipe_rhs() {
    let tree = parse_ok(
        r"
flow main {
  let value = ^
  return value
}
",
    );
    let hir = lower_to_hir(&tree).expect("placeholder fixture lowers to HIR");
    validate_typecheck_ready(&hir).expect("placeholder expression is structured");

    let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect_err("pipe-left placeholder outside pipe rhs is rejected");

    assert!(errors.iter().any(|error| error.to_string().contains("`^`")));
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
