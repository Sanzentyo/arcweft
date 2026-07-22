use super::*;
use crate::typed_evidence::{
    RuntimeNumericType, RuntimeTypedExpressionId, RuntimeTypedLoweringEvidence,
    RuntimeTypedLoweringEvidenceKind,
};
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::plan::{RuntimePureHelperOrigin, RuntimePureInputType, RuntimePureOutputType};
use arcweft_core::value::RuntimeIntrinsic;
use arcweft_lang_hir::syntax::{
    expr::{IntSuffix, Placeholder},
    types::parse_type_ref,
};

fn int(value: u128, suffix: Option<IntSuffix>) -> Expr {
    Expr::Literal(Literal::Int(int_literal(value, suffix)))
}

fn parsed_expr(source: &str) -> Expr {
    arcweft_lang_hir::syntax::expr::parse_expr(source)
        .expect("test fixture must use valid authored expression syntax")
}

fn int_literal(value: u128, suffix: Option<IntSuffix>) -> IntLiteral {
    IntLiteral::decimal(value, suffix)
}

fn parsed_int_suffix(suffix: &str) -> IntSuffix {
    IntSuffix::parse(suffix).expect("test uses a canonical integer suffix")
}

fn lower_with_numeric_evidence(
    expr: &Expr,
    evidence: &[RuntimeTypedLoweringEvidence],
) -> Result<RuntimeExpr, String> {
    let ids = BTreeMap::new();
    let helpers = Vec::new();
    let cursor = Cell::new(0);
    lower_runtime_expr_strict_with_pure(
        expr,
        RuntimePureHelperLookup::new(&ids, &helpers)
            .with_typed_lowering_evidence(evidence, &cursor),
    )
}

fn resolved_numeric_evidence(
    expression: usize,
    target: RuntimeNumericType,
) -> RuntimeTypedLoweringEvidence {
    RuntimeTypedLoweringEvidence {
        expression_id: RuntimeTypedExpressionId::from_index(expression),
        owner: None,
        kind: RuntimeTypedLoweringEvidenceKind::ResolvedNumericType { target },
    }
}

#[test]
fn strict_runtime_value_lowering_preserves_calls() {
    let expr = parsed_expr("compute()");

    let lowered = lower_runtime_expr_strict(&expr).expect("calls are runtime values");

    assert!(matches!(lowered, RuntimeExpr::Call { callee, .. } if callee.as_label() == "compute"));
}

#[test]
fn runtime_string_lowering_uses_the_shared_syntax_decoder() {
    let expr = parsed_expr(r#""line\nreview \u{732b}""#);

    let lowered = lower_runtime_expr_strict(&expr).expect("string literal lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Value(RuntimeValue::String(value))
            if value == "line\nreview 猫"
    ));
}

#[test]
fn strict_runtime_value_lowering_can_emit_pure_calls() {
    let expr = parsed_expr("add(3i64, 4i64)");
    let helpers = vec![add_i64_helper()];
    let ids = BTreeMap::from([("add".to_owned(), helpers[0].id)]);

    let lowered =
        lower_runtime_expr_strict_with_pure(&expr, RuntimePureHelperLookup::new(&ids, &helpers))
            .expect("pure calls lower");

    assert!(matches!(
        lowered,
        RuntimeExpr::PureCall { helper, args }
            if helper == RuntimePureHelperId(0)
                && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(lhs), RuntimeExpr::Value(rhs)]
                        if lhs == &RuntimeValue::i64(3) && rhs == &RuntimeValue::i64(4)
                )
    ));
}

#[test]
fn strict_runtime_reorders_named_pure_helper_args_by_input_name() {
    let expr = parsed_expr("add(rhs = 4i64, lhs = 3i64)");
    let helpers = vec![add_i64_helper()];
    let ids = BTreeMap::from([("add".to_owned(), helpers[0].id)]);

    let lowered =
        lower_runtime_expr_strict_with_pure(&expr, RuntimePureHelperLookup::new(&ids, &helpers))
            .expect("named pure calls lower");

    assert!(matches!(
        lowered,
        RuntimeExpr::PureCall { helper, args }
            if helper == RuntimePureHelperId(0)
                && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(lhs), RuntimeExpr::Value(rhs)]
                        if lhs == &RuntimeValue::i64(3) && rhs == &RuntimeValue::i64(4)
                )
    ));
}

#[test]
fn strict_runtime_lowers_named_missing_pure_helper_input_to_function() {
    let expr = parsed_expr("add(rhs = 4i64)");
    let helpers = vec![add_i64_helper()];
    let ids = BTreeMap::from([("add".to_owned(), helpers[0].id)]);

    let lowered =
        lower_runtime_expr_strict_with_pure(&expr, RuntimePureHelperLookup::new(&ids, &helpers))
            .expect("named partial pure call lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["lhs"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::PureCall { helper, args }
                        if helper == &RuntimePureHelperId(0)
                            && matches!(
                                args.as_slice(),
                                [RuntimeExpr::Local(lhs), RuntimeExpr::Value(rhs)]
                                    if lhs == "lhs" && rhs == &RuntimeValue::i64(4)
                            )
                )
    ));
}

#[test]
fn strict_runtime_lowers_bare_pure_helper_path_to_function_value() {
    let helpers = vec![add_i64_helper()];
    let ids = BTreeMap::from([("add".to_owned(), helpers[0].id)]);
    let expr = Expr::Path("add".into());

    let lowered =
        lower_runtime_expr_strict_with_pure(&expr, RuntimePureHelperLookup::new(&ids, &helpers))
            .expect("function path lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Function { params, body }
            if params == ["lhs", "rhs"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Binary { lhs, .. }
                        if matches!(lhs.as_ref(), RuntimeExpr::Local(name) if name == "lhs")
                )
    ));
}

#[test]
fn strict_runtime_lowers_partial_pure_helper_call_to_apply() {
    let helpers = vec![add_i64_helper()];
    let ids = BTreeMap::from([("add".to_owned(), helpers[0].id)]);
    let expr = parsed_expr("add(2i64)");

    let lowered =
        lower_runtime_expr_strict_with_pure(&expr, RuntimePureHelperLookup::new(&ids, &helpers))
            .expect("partial helper call lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Apply { callee, args }
            if matches!(
                callee.as_ref(),
                RuntimeExpr::Function { params, .. } if params.as_slice() == ["lhs", "rhs"]
            ) && matches!(
                args.as_slice(),
                [RuntimeExpr::Value(value)] if value == &RuntimeValue::i64(2)
            )
    ));
}

#[test]
fn strict_runtime_lowers_expected_partial_placeholder_to_function_expr() {
    let expr = Expr::Binary {
        lhs: Box::new(Expr::Placeholder(Placeholder::Partial)),
        op: BinaryOp::Gt,
        rhs: Box::new(int(80, Some(IntSuffix::I64))),
    };
    let expected =
        parse_type_ref("i64 -> bool").expect("test fixture must use valid authored type syntax");
    let ids = BTreeMap::new();
    let helpers = Vec::new();

    let lowered = lower_runtime_expr_strict_with_expected_type(
        &expr,
        Some(expected.value()),
        RuntimePureHelperLookup::new(&ids, &helpers),
    )
    .expect("expected placeholder function lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Function { params, body }
            if params == ["__arcweft_partial"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Binary { lhs, .. }
                        if matches!(
                            lhs.as_ref(),
                            RuntimeExpr::Local(name) if name == "__arcweft_partial"
                        )
                )
    ));
}

fn add_i64_helper() -> RuntimePureHelper {
    RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "add".to_owned(),
        input_names: vec!["lhs".to_owned(), "rhs".to_owned()],
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("lhs".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("rhs".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Inferred,
    }
}

#[test]
fn strict_runtime_lowers_closure_to_function_expr() {
    let expr = parsed_expr("|score| score > 80i64");

    let lowered = lower_runtime_expr_strict(&expr).expect("closure lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Function { params, body }
            if params == ["score"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Binary { lhs, .. }
                        if matches!(lhs.as_ref(), RuntimeExpr::Local(name) if name == "score")
                )
    ));
}

#[test]
fn strict_runtime_lowers_destructured_closure_param_to_match_body() {
    let expr = parsed_expr("|(left, right)| right");

    let lowered = lower_runtime_expr_strict(&expr).expect("destructured closure lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["$arcweft.closure.arg.0"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Match { scrutinee, arms }
                        if matches!(
                            scrutinee.as_ref(),
                            RuntimeExpr::Local(name) if name == "$arcweft.closure.arg.0"
                        )
                        && matches!(
                            arms.as_slice(),
                            [RuntimeExprMatchArm {
                                pattern: RuntimePattern::Tuple(items),
                                guard: None,
                                value,
                            }] if matches!(
                                items.as_slice(),
                                [
                                    RuntimePattern::Ident(left),
                                    RuntimePattern::Ident(right),
                                ] if left == "left" && right == "right"
                            ) && matches!(value, RuntimeExpr::Local(name) if name == "right")
                        )
                )
    ));
}

#[test]
fn strict_runtime_lowers_expression_callee_call_to_apply() {
    let expr = parsed_expr("make_adder(2i64)(5i64)");

    let lowered = lower_runtime_expr_strict(&expr).expect("expression callee lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Apply { callee, args }
            if matches!(
                callee.as_ref(),
                RuntimeExpr::Call { callee, args }
                    if callee.as_label() == "make_adder" && args.len() == 1
            ) && matches!(
                args.as_slice(),
                [RuntimeExpr::Value(value)] if value == &RuntimeValue::i64(5)
            )
    ));
}

#[test]
fn strict_runtime_lowers_f64_math_method_calls_to_intrinsics() {
    let expr = parsed_expr("math.matmul_f64(lhs, rhs)");

    let lowered = lower_runtime_expr_strict(&expr).expect("math intrinsic lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Call { callee, args }
            if callee.as_intrinsic() == Some(RuntimeIntrinsic::MathMatmulF64)
                && args.len() == 2
    ));
}

#[test]
fn strict_runtime_lowers_adapter_namespace_methods_to_external_calls() {
    let expr = parsed_expr("infer.matmul_bias_add_f32(lhs, rhs, bias)");

    let lowered = lower_runtime_expr_strict(&expr).expect("adapter method lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Call { callee, args }
            if callee.as_label() == "infer.matmul_bias_add_f32" && args.len() == 3
    ));
}

#[test]
fn strict_runtime_binds_pipe_left_once() {
    let expr = parsed_expr("value |> clamp(0i64, ^, 100i64)");

    let lowered = lower_runtime_expr_strict(&expr).expect("pipe placeholder lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Let { name, expr, body }
            if name.starts_with('\0')
                && matches!(expr.as_ref(), RuntimeExpr::Local(value) if value == "value")
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Call { callee, args }
                        if callee.as_label() == "clamp"
                            && matches!(args.as_slice(), [
                                RuntimeExpr::Value(_),
                                RuntimeExpr::Local(pipe),
                                RuntimeExpr::Value(_),
                            ] if pipe == &name)
                )
    ));
}

#[test]
fn strict_runtime_keeps_data_last_pipe_as_two_stages() {
    let expr = parsed_expr("value |> normalize(2i64)");

    let lowered = lower_runtime_expr_strict(&expr).expect("data-last pipe lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Let { name, expr, body }
            if name.starts_with('\0')
                && matches!(expr.as_ref(), RuntimeExpr::Local(value) if value == "value")
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Apply { callee, args }
                        if matches!(
                            callee.as_ref(),
                            RuntimeExpr::Call { callee, args }
                                if callee.as_label() == "normalize" && args.len() == 1
                        ) && matches!(
                            args.as_slice(),
                            [RuntimeExpr::Local(pipe)] if pipe == &name
                        )
                )
    ));
}

#[test]
fn nested_pipe_in_no_placeholder_rhs_uses_a_distinct_binding_depth() {
    let expr = arcweft_lang_syntax::expr::parse_expr("outer |> (|| (inner |> (^, ^)))")
        .expect("nested closure pipe parses");

    let lowered = lower_runtime_expr_strict(&expr).expect("nested closure pipe lowers");

    let RuntimeExpr::Let {
        name: outer_name,
        body,
        ..
    } = lowered
    else {
        panic!("outer pipe must own an exact-once binding")
    };
    let RuntimeExpr::Apply { callee, .. } = body.as_ref() else {
        panic!("outer pipe must apply its RHS to the bound left value")
    };
    let RuntimeExpr::Function {
        body: closure_body, ..
    } = callee.as_ref()
    else {
        panic!("outer pipe RHS must remain a closure")
    };
    let RuntimeExpr::Let {
        name: inner_name, ..
    } = closure_body.as_ref()
    else {
        panic!("inner pipe must own an exact-once binding")
    };

    assert_ne!(outer_name, *inner_name);
    assert!(outer_name.ends_with(".0"));
    assert!(inner_name.ends_with(".1"));
}

#[test]
fn strict_runtime_lowers_data_last_pipe_to_partial_helper_apply() {
    let expr = Expr::Pipe {
        lhs: Box::new(int(2, Some(IntSuffix::I64))),
        rhs: Box::new(Expr::Path("add".into())),
    };
    let helpers = vec![add_i64_helper()];
    let ids = BTreeMap::from([("add".to_owned(), helpers[0].id)]);

    let lowered =
        lower_runtime_expr_strict_with_pure(&expr, RuntimePureHelperLookup::new(&ids, &helpers))
            .expect("data-last helper pipe lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Let { name, expr, body }
            if name.starts_with('\0')
                && matches!(
                    expr.as_ref(),
                    RuntimeExpr::Value(value) if value == &RuntimeValue::i64(2)
                ) && matches!(
                    body.as_ref(),
                    RuntimeExpr::Apply { callee, args }
                        if matches!(
                            callee.as_ref(),
                            RuntimeExpr::Function { params, .. }
                                if params.as_slice() == ["lhs", "rhs"]
                        ) && matches!(
                            args.as_slice(),
                            [RuntimeExpr::Local(pipe)] if pipe == &name
                        )
                )
    ));
}

#[test]
fn strict_runtime_lowers_data_last_pipe_call_to_exact_helper_call() {
    let expr = parsed_expr("2i64 |> add(1i64)");
    let helpers = vec![add_i64_helper()];
    let ids = BTreeMap::from([("add".to_owned(), helpers[0].id)]);

    let lowered =
        lower_runtime_expr_strict_with_pure(&expr, RuntimePureHelperLookup::new(&ids, &helpers))
            .expect("data-last exact helper pipe lowers");

    let RuntimeExpr::Let { name, expr, body } = lowered else {
        panic!("pipe must bind its left value once")
    };
    assert!(name.starts_with('\0'));
    assert!(matches!(
        expr.as_ref(),
        RuntimeExpr::Value(value) if value == &RuntimeValue::i64(2)
    ));
    assert!(matches!(
        body.as_ref(),
        RuntimeExpr::Apply { callee, args }
            if matches!(
                callee.as_ref(),
                RuntimeExpr::Apply { callee, args }
                    if matches!(
                        callee.as_ref(),
                        RuntimeExpr::Function { params, .. }
                            if params.as_slice() == ["lhs", "rhs"]
                    ) && matches!(
                        args.as_slice(),
                        [RuntimeExpr::Value(value)] if value == &RuntimeValue::i64(1)
                    )
            ) && matches!(
                args.as_slice(),
                [RuntimeExpr::Local(pipe)] if pipe == &name
            )
    ));
}

#[test]
fn strict_runtime_lowers_partial_placeholder_map_body() {
    let expr = parsed_expr("values.map(_ + 1i64)");

    let lowered = lower_runtime_expr_strict(&expr).expect("partial map lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Map { param, body, .. }
            if param == "_item"
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Binary { lhs, .. }
                        if matches!(lhs.as_ref(), RuntimeExpr::Local(name) if name == "_item")
                )
    ));
}

#[test]
fn strict_runtime_lowers_partial_placeholder_filter_body() {
    let expr = parsed_expr("choices.filter(_.enabled)");

    let lowered = lower_runtime_expr_strict(&expr).expect("partial filter lowers");

    assert!(matches!(
    lowered,
    RuntimeExpr::Filter { param, body, .. }
        if param == "_item"
            && matches!(
                body.as_ref(),
                RuntimeExpr::Field { target, field }
                    if field == "enabled"
                        && matches!(
                            target.as_ref(),
                            RuntimeExpr::Local(name) if name == "_item"
                        )
            )
    ));
}

#[test]
fn strict_runtime_rejects_try_and_await_without_control_boundaries() {
    let try_expr = parsed_expr("frame.objects.require_role(\"dialogue_view\")?");
    let await_expr = parsed_expr("await frame.objects.require_role(\"dialogue_view\")");

    let try_error =
        lower_runtime_expr_strict(&try_expr).expect_err("try requires control lowering");
    let await_error =
        lower_runtime_expr_strict(&await_expr).expect_err("await requires control lowering");

    assert!(try_error.contains("error-propagation boundary"));
    assert!(await_error.contains("suspension-aware statement lowering"));
}

#[test]
fn lossy_runtime_projection_treats_prefix_and_postfix_try_equivalently() {
    let prefix = lower_runtime_expr(&parsed_expr("try value"));
    let postfix = lower_runtime_expr(&parsed_expr("value?"));

    assert_eq!(prefix, postfix);
}

#[test]
fn lossy_runtime_label_lowering_remains_non_executable() {
    let expr = parsed_expr("frame.objects.require_role(\"dialogue_view\")?");

    let lowered = lower_runtime_expr(&expr);

    assert!(matches!(
        lowered,
        RuntimeExpr::MethodCall { method, args, .. }
            if method == "require_role" && args.len() == 1
    ));
}

#[test]
fn strict_runtime_array_repeat_folds_literal_value_sequence() {
    let expr = Expr::ArrayRepeat {
        value: Box::new(int(2, Some(IntSuffix::I64))),
        len: Box::new(int(4, None)),
    };

    let lowered = lower_runtime_expr_strict(&expr).expect("array repeat lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::RepeatSeq { value, len: 4 }
            if matches!(value.as_ref(), RuntimeExpr::Value(value) if value == &RuntimeValue::i64(2))
    ));
}

#[test]
fn suffixed_integer_literals_lower_to_width_preserving_runtime_scalars() {
    for (suffix, expected) in [
        ("i8", RuntimeValue::i8(7)),
        ("i16", RuntimeValue::i16(7)),
        ("i32", RuntimeValue::i32(7)),
        ("i64", RuntimeValue::i64(7)),
        ("i128", RuntimeValue::i128(7)),
        ("isize", RuntimeValue::isize(7)),
        ("u8", RuntimeValue::u8(7)),
        ("u16", RuntimeValue::u16(7)),
        ("u32", RuntimeValue::u32(7)),
        ("u64", RuntimeValue::u64(7)),
        ("u128", RuntimeValue::u128(7)),
        ("usize", RuntimeValue::usize(7)),
    ] {
        let expr = int(7, Some(parsed_int_suffix(suffix)));

        let lowered = lower_runtime_expr_strict(&expr).expect("suffixed integer literal lowers");

        assert_eq!(lowered, RuntimeExpr::Value(expected));
    }
}

#[test]
fn strict_runtime_bracket_seq_folds_literal_values_to_dense_storage() {
    let unit_expr = Expr::BracketSeq(vec![
        Expr::Tuple(Vec::new()),
        Expr::Tuple(Vec::new()),
        Expr::Tuple(Vec::new()),
    ]);

    let lowered = lower_runtime_expr_strict(&unit_expr).expect("unit bracket seq lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq)) if seq.unit_len() == Some(3)
    ));

    let i64_expr = Expr::BracketSeq(vec![
        int(1, Some(IntSuffix::I64)),
        int(2, Some(IntSuffix::I64)),
    ]);

    let lowered = lower_runtime_expr_strict(&i64_expr).expect("i64 bracket seq lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_i64_slice() == Some([1, 2].as_slice())
    ));

    let bool_expr = Expr::BracketSeq(vec![
        Expr::Literal(Literal::Bool(true)),
        Expr::Literal(Literal::Bool(false)),
    ]);
    let lowered = lower_runtime_expr_strict(&bool_expr).expect("bool bracket seq lowers");
    assert!(matches!(
        lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_bool_slice() == Some([true, false].as_slice())
    ));

    let char_expr = Expr::BracketSeq(vec![
        Expr::Literal(Literal::Char {
            raw: "\"a\"c".to_owned(),
            value: 'a',
        }),
        Expr::Literal(Literal::Char {
            raw: "\"b\"c".to_owned(),
            value: 'b',
        }),
    ]);
    let lowered = lower_runtime_expr_strict(&char_expr).expect("char bracket seq lowers");
    assert!(matches!(
        lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_chars() == Some(['a', 'b'].as_slice())
    ));

    let duration_expr = Expr::BracketSeq(vec![
        Expr::Literal(Literal::Duration {
            amount: "5".to_owned(),
            unit: arcweft_lang_hir::syntax::expr::DurationUnit::Nanos,
        }),
        Expr::Literal(Literal::Duration {
            amount: "16_666".to_owned(),
            unit: arcweft_lang_hir::syntax::expr::DurationUnit::Micros,
        }),
        Expr::Literal(Literal::Duration {
            amount: "2".to_owned(),
            unit: arcweft_lang_hir::syntax::expr::DurationUnit::Minutes,
        }),
        Expr::Literal(Literal::Duration {
            amount: "1".to_owned(),
            unit: arcweft_lang_hir::syntax::expr::DurationUnit::Hours,
        }),
    ]);
    let lowered = lower_runtime_expr_strict(&duration_expr).expect("duration bracket seq lowers");
    assert!(matches!(
        lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_durations() == Some([
                arcweft_core::time::LogicalDuration::from_nanos(5),
                arcweft_core::time::LogicalDuration::from_nanos(16_666_000),
                arcweft_core::time::LogicalDuration::from_nanos(120_000_000_000),
                arcweft_core::time::LogicalDuration::from_nanos(3_600_000_000_000),
            ].as_slice())
    ));
}

#[test]
fn strict_runtime_bracket_seq_folds_typed_float_literals_to_dense_storage() {
    let f32_expr = Expr::BracketSeq(vec![
        Expr::Literal(Literal::Float {
            raw: "1.5f32".to_owned(),
            suffix: Some(FloatSuffix::F32),
        }),
        Expr::Literal(Literal::Float {
            raw: "2.5f32".to_owned(),
            suffix: Some(FloatSuffix::F32),
        }),
    ]);
    let f64_expr = Expr::BracketSeq(vec![
        Expr::Literal(Literal::Float {
            raw: "3.25f64".to_owned(),
            suffix: Some(FloatSuffix::F64),
        }),
        Expr::Literal(Literal::Float {
            raw: "-0.0f64".to_owned(),
            suffix: Some(FloatSuffix::F64),
        }),
    ]);

    let f32_lowered = lower_runtime_expr_strict(&f32_expr).expect("f32 bracket seq lowers");
    let f64_lowered = lower_runtime_expr_strict(&f64_expr).expect("f64 bracket seq lowers");

    assert!(matches!(
        f32_lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_f32_slice() == Some([
                (1.5),
                (2.5),
            ].as_slice())
    ));
    assert!(matches!(
        f64_lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_f64_slice() == Some([
                (3.25),
                (-0.0),
            ].as_slice())
    ));
}

#[test]
fn strict_runtime_lowers_std_float_constants_and_intrinsic_calls() {
    let nan_expr = Expr::select(Expr::select(Expr::Path("std".into()), "f32"), "nan");
    let nan_lowered = lower_runtime_expr_strict(&nan_expr).expect("std f32 nan lowers");
    assert!(matches!(
        nan_lowered,
        RuntimeExpr::Value(RuntimeValue::F32(value)) if value.is_nan()
    ));

    let sqrt_expr = parsed_expr("std.f64.sqrt(4.0f64)");
    let sqrt_lowered = lower_runtime_expr_strict(&sqrt_expr).expect("std f64 sqrt lowers");
    assert!(matches!(
        sqrt_lowered,
        RuntimeExpr::Call { callee, .. } if callee.as_label() == "std.f64.sqrt"
    ));
}

#[test]
fn strict_runtime_field_lowering_uses_record_projection_when_ordinal_is_known() {
    let expr = Expr::select(
        Expr::RecordLiteral(vec![
            ("score".to_owned(), int(7, None)),
            (
                "label".to_owned(),
                Expr::Literal(Literal::String("ok".to_owned())),
            ),
        ]),
        "label",
    );

    let lowered = lower_runtime_expr_strict(&expr).expect("record field lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::ProjectRecord { ordinal: 1, .. }
    ));
}

#[test]
fn strict_runtime_tuple_index_lowering_uses_tuple_projection_when_ordinal_is_known() {
    let expr = Expr::Index {
        target: Box::new(Expr::Tuple(vec![
            int(1, None),
            Expr::Literal(Literal::Bool(true)),
        ])),
        index: Box::new(int(1, None)),
    };

    let lowered = lower_runtime_expr_strict(&expr).expect("tuple index lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::ProjectTuple { ordinal: 1, .. }
    ));
}

#[test]
fn numeric_bracket_seq_lowers_to_dense_i32_sequence() {
    let expr = Expr::NumericBracketSeq(
        NumericBracketSeq::new(vec![
            int_literal(1, None),
            int_literal(2, None),
            int_literal(3, None),
        ])
        .expect("test sequence uses one suffix"),
    );

    let lowered = lower_runtime_expr_strict(&expr).expect("numeric seq lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_i32_slice() == Some([1, 2, 3].as_slice())
    ));
}

#[test]
fn suffixed_numeric_bracket_seq_lowers_to_width_specific_dense_sequence() {
    let i8_lowered = lower_suffixed_numeric_seq("i8");
    let i16_lowered = lower_suffixed_numeric_seq("i16");
    let i32_lowered = lower_suffixed_numeric_seq("i32");
    let i128_lowered = lower_suffixed_numeric_seq("i128");
    let isize_lowered = lower_suffixed_numeric_seq("isize");
    let u8_lowered = lower_suffixed_numeric_seq("u8");
    let u16_lowered = lower_suffixed_numeric_seq("u16");
    let u32_lowered = lower_suffixed_numeric_seq("u32");
    let u64_lowered = lower_suffixed_numeric_seq("u64");
    let u128_lowered = lower_suffixed_numeric_seq("u128");
    let usize_lowered = lower_suffixed_numeric_seq("usize");

    assert!(matches!(
        i8_lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_i8_slice() == Some([1, 2, 3].as_slice())
    ));
    assert!(matches!(
        i16_lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_i16_slice() == Some([1, 2, 3].as_slice())
    ));
    assert!(matches!(
        i32_lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_i32_slice() == Some([1, 2, 3].as_slice())
    ));
    assert!(matches!(
        i128_lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_i128_slice() == Some([1, 2, 3].as_slice())
    ));
    assert!(matches!(
        isize_lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_isize_values() == Some(vec![1, 2, 3])
    ));
    assert!(matches!(
        u8_lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_u8_slice() == Some([1, 2, 3].as_slice())
    ));
    assert!(matches!(
        u16_lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_u16_slice() == Some([1, 2, 3].as_slice())
    ));
    assert!(matches!(
        u32_lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_u32_slice() == Some([1, 2, 3].as_slice())
    ));
    assert!(matches!(
        u64_lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_u64_slice() == Some([1, 2, 3].as_slice())
    ));
    assert!(matches!(
        u128_lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_u128_slice() == Some([1, 2, 3].as_slice())
    ));
    assert!(matches!(
        usize_lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_usize_values() == Some(vec![1, 2, 3])
    ));
}

fn lower_suffixed_numeric_seq(suffix: &str) -> RuntimeExpr {
    let suffix = Some(parsed_int_suffix(suffix));
    let expr = Expr::NumericBracketSeq(
        NumericBracketSeq::new(vec![
            int_literal(1, suffix),
            int_literal(2, suffix),
            int_literal(3, suffix),
        ])
        .expect("test sequence uses one suffix"),
    );
    lower_runtime_expr_strict(&expr).expect("suffixed numeric seq lowers")
}

#[test]
fn resolved_numeric_evidence_controls_unsuffixed_runtime_widths() {
    let wide =
        arcweft_lang_hir::syntax::expr::parse_expr("340282366920938463463374607431768211455")
            .expect("u128 magnitude parses");
    assert_eq!(
        lower_with_numeric_evidence(
            &wide,
            &[resolved_numeric_evidence(0, RuntimeNumericType::U128)],
        ),
        Ok(RuntimeExpr::Value(RuntimeValue::u128(u128::MAX)))
    );

    let precise =
        arcweft_lang_hir::syntax::expr::parse_expr("1_2.5_0").expect("underscored float parses");
    assert_eq!(
        lower_with_numeric_evidence(
            &precise,
            &[resolved_numeric_evidence(0, RuntimeNumericType::F32)],
        ),
        Ok(RuntimeExpr::Value(RuntimeValue::F32(12.5)))
    );
}

#[test]
fn resolved_numeric_sequence_evidence_controls_dense_item_width() {
    let expr = arcweft_lang_hir::syntax::expr::parse_expr("[4294967296, 4294967297]")
        .expect("wide numeric sequence parses");
    let lowered = lower_with_numeric_evidence(
        &expr,
        &[resolved_numeric_evidence(0, RuntimeNumericType::U64)],
    )
    .expect("typed numeric sequence lowers");
    assert!(matches!(
        lowered,
        RuntimeExpr::Value(RuntimeValue::Seq(seq))
            if seq.as_u64_slice() == Some([4_294_967_296, 4_294_967_297].as_slice())
    ));
}

#[test]
fn resolved_numeric_evidence_preserves_expected_signed_minimum() {
    let expr = arcweft_lang_hir::syntax::expr::parse_expr("-32768")
        .expect("signed minimum expression parses");
    let lowered = lower_with_numeric_evidence(
        &expr,
        &[resolved_numeric_evidence(1, RuntimeNumericType::I16)],
    )
    .expect("typed signed minimum lowers");
    assert_eq!(lowered, RuntimeExpr::Value(RuntimeValue::i16(i16::MIN)));
}
