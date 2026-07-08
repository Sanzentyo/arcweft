use arcweft_lang_syntax::{
    ast::{
        flow::{FlowItem, Stmt},
        items::Item,
    },
    expr::{BinaryOp, DurationUnit, Expr, Literal, UnaryOp, UnitNumberSuffix, parse_expr},
    types::{TypeRef, parse_type_ref},
};

fn parse_ok(source: impl Into<String>) -> arcweft_lang_syntax::ast::items::TypedSyntaxTree {
    let parsed = arcweft_lang_syntax::parser::parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "expected source to parse without errors, got {:?}",
        parsed.errors()
    );
    parsed.into_typed_tree()
}

fn select_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => Some(path.as_label().to_owned()),
        Expr::Select(select) => Some(format!(
            "{}.{}",
            select_path(select.target())?,
            select.member().as_str()
        )),
        _ => None,
    }
}

#[test]
fn pratt_parser_keeps_documented_precedence() {
    let expr = parse_expr("a + b * c").expect("multiplicative precedence parses");
    assert!(matches!(
        expr,
        Expr::Binary {
            op: BinaryOp::Add,
            rhs,
            ..
        } if matches!(rhs.as_ref(), Expr::Binary { op: BinaryOp::Mul, .. })
    ));

    let expr = parse_expr("a < b + c").expect("comparison with additive rhs parses");
    assert!(matches!(
        expr,
        Expr::Binary {
            op: BinaryOp::Lt,
            rhs,
            ..
        } if matches!(rhs.as_ref(), Expr::Binary { op: BinaryOp::Add, .. })
    ));

    let expr = parse_expr("-score").expect("unary negation parses");
    assert!(matches!(
        expr,
        Expr::Unary {
            op: UnaryOp::Neg,
            ..
        }
    ));
}

#[test]
fn generic_expr_brackets_are_indexes_not_dialogue_calls() {
    let expr = parse_expr("alice.say()[text]").expect("bracket postfix parses");
    assert!(matches!(expr, Expr::Index { .. }));

    let tree = parse_ok(
        r"
flow @flow.opening opening {
    let handles = alice.say()[本文です。[p]]
}
",
    );
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    assert!(matches!(
        &flow.body()[0],
        FlowItem::Stmt(Stmt::Let {
            expr: Expr::DialogueCall { .. },
            ..
        })
    ));
}

#[test]
fn select_and_index_are_structured_for_later_typechecking() {
    let expr = parse_expr("state.affection[@character.alice]").expect("select index parses");
    let Expr::Index { target, index } = expr else {
        panic!("expected index");
    };
    assert_eq!(select_path(&target), Some("state.affection".to_owned()));
    assert!(matches!(index.as_ref(), Expr::EntityRef(_)));

    assert!(matches!(
        parse_type_ref("!").expect("never type parses"),
        TypeRef::Never
    ));
    assert!(matches!(
        parse_type_ref("Never").expect("canonical never type parses"),
        TypeRef::Never
    ));
}

#[test]
fn array_types_and_repeat_literals_are_structured() {
    assert!(matches!(
        parse_type_ref("Array<i32, 3>").expect("array type parses"),
        TypeRef::Generic { base, args }
            if base == "Array"
                && args.len() == 2
                && matches!(&args[1], TypeRef::ConstInt(3))
    ));

    let expr = parse_expr("[0; 4]").expect("array repeat literal parses");
    assert!(matches!(
        expr,
        Expr::ArrayRepeat { value, len }
            if matches!(value.as_ref(), Expr::Literal(_))
                && matches!(len.as_ref(), Expr::Literal(_))
    ));
}

#[test]
fn large_flat_literal_sequences_parse_as_bracket_seq() {
    let values = (0..128)
        .map(|value| format!("{value}i64"))
        .collect::<Vec<_>>()
        .join(", ");
    let expr = parse_expr(&format!("[{values}]")).expect("large literal sequence parses");
    let Expr::NumericBracketSeq(seq) = expr else {
        panic!("expected numeric bracket sequence");
    };
    assert_eq!(seq.len(), 128);
    assert_eq!(seq.suffix(), Some("i64"));
    assert_eq!(seq.values()[0], 0);
    assert_eq!(seq.values()[127], 127);

    let repeat = parse_expr("[0i64; 4]").expect("array repeat still parses");
    assert!(matches!(repeat, Expr::ArrayRepeat { .. }));

    let indexed = parse_expr("[1i64, 2i64][0i64]").expect("literal sequence index parses");
    assert!(matches!(
        indexed,
        Expr::Index { target, index }
            if matches!(target.as_ref(), Expr::NumericBracketSeq(_))
                && matches!(index.as_ref(), Expr::Literal(Literal::Int { .. }))
    ));

    let mixed = parse_expr("[1i64, false]").expect("mixed sequence falls back");
    assert!(matches!(mixed, Expr::BracketSeq(_)));

    let mixed_suffix = parse_expr("[1i32, 2i64]").expect("mixed suffix sequence falls back");
    assert!(matches!(
        mixed_suffix,
        Expr::BracketSeq(items)
            if items.len() == 2
                && matches!(&items[0], Expr::Literal(Literal::Int { suffix, .. }) if suffix.as_deref() == Some("i32"))
                && matches!(&items[1], Expr::Literal(Literal::Int { suffix, .. }) if suffix.as_deref() == Some("i64"))
    ));
}

#[test]
fn float_suffix_and_unit_number_literals_are_typed_syntax() {
    let f32_lit = parse_expr("1.5f32").expect("f32 literal parses");
    assert!(matches!(
        f32_lit,
        Expr::Literal(Literal::Float {
            suffix: Some(arcweft_lang_syntax::expr::FloatSuffix::F32),
            ..
        })
    ));

    let f64_lit = parse_expr("1e3f64").expect("exponent f64 literal parses");
    assert!(matches!(
        f64_lit,
        Expr::Literal(Literal::Float {
            suffix: Some(arcweft_lang_syntax::expr::FloatSuffix::F64),
            ..
        })
    ));

    for (source, expected) in [
        ("85%", UnitNumberSuffix::Percent),
        ("24px", UnitNumberSuffix::Px),
        ("12pt", UnitNumberSuffix::Pt),
        ("1.5em", UnitNumberSuffix::Em),
        ("2rem", UnitNumberSuffix::Rem),
        ("100vw", UnitNumberSuffix::Vw),
        ("50vh", UnitNumberSuffix::Vh),
        ("90deg", UnitNumberSuffix::Deg),
        ("2rad", UnitNumberSuffix::Rad),
        ("0.25turn", UnitNumberSuffix::Turn),
        ("6db", UnitNumberSuffix::Db),
        ("18lufs", UnitNumberSuffix::Lufs),
        ("92bpm", UnitNumberSuffix::Bpm),
        ("4bars", UnitNumberSuffix::Bars),
    ] {
        let expr = parse_expr(source).expect("unit-number literal parses");
        assert!(
            matches!(expr, Expr::Literal(Literal::UnitNumber { suffix, .. }) if suffix == expected)
        );
    }

    for (source, expected) in [
        ("16_666us", DurationUnit::Micros),
        ("5ns", DurationUnit::Nanos),
        ("120ms", DurationUnit::Millis),
        ("1.5s", DurationUnit::Seconds),
        ("2min", DurationUnit::Minutes),
        ("1h", DurationUnit::Hours),
    ] {
        let expr = parse_expr(source).expect("duration literal parses");
        assert!(matches!(expr, Expr::Literal(Literal::Duration { unit, .. }) if unit == expected));
    }

    assert!(matches!(
        parse_expr("0xff_u8").expect("hex integer parses"),
        Expr::Literal(Literal::Int { value: 255, suffix, .. }) if suffix.as_deref() == Some("u8")
    ));
    assert!(matches!(
        parse_expr("0b1010_0101u8").expect("binary integer parses"),
        Expr::Literal(Literal::Int { value: 0b1010_0101, suffix, .. }) if suffix.as_deref() == Some("u8")
    ));
    assert!(matches!(
        parse_expr("0o755u32").expect("octal integer parses"),
        Expr::Literal(Literal::Int { value: 0o755, suffix, .. }) if suffix.as_deref() == Some("u32")
    ));
    assert!(matches!(
        parse_expr("1_000i32").expect("underscored decimal integer parses"),
        Expr::Literal(Literal::Int { value: 1000, suffix, .. }) if suffix.as_deref() == Some("i32")
    ));

    assert!(parse_expr("1.0NaN").is_err());
    assert!(parse_expr("1.0Inf").is_err());
}
