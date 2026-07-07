use super::*;
use arcweft_core::plan::{RuntimePureHelperOrigin, RuntimePureInputType, RuntimePureOutputType};
use arcweft_core::value::RuntimeIntrinsic;

#[test]
fn strict_runtime_value_lowering_preserves_calls() {
    let expr = Expr::Call {
        callee: Box::new(Expr::Path("compute".into())),
        args: Vec::new(),
    };

    let lowered = lower_runtime_expr_strict(&expr).expect("calls are runtime values");

    assert!(matches!(lowered, RuntimeExpr::Call { callee, .. } if callee.as_label() == "compute"));
}

#[test]
fn strict_runtime_value_lowering_can_emit_pure_calls() {
    let expr = Expr::Call {
        callee: Box::new(Expr::Path("add".into())),
        args: vec![
            CallArg::Positional(Expr::Literal(Literal::Int {
                raw: "3i64".to_owned(),
                value: 3,
                suffix: Some("i64".to_owned()),
            })),
            CallArg::Positional(Expr::Literal(Literal::Int {
                raw: "4i64".to_owned(),
                value: 4,
                suffix: Some("i64".to_owned()),
            })),
        ],
    };
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
    let expr = Expr::Call {
        callee: Box::new(Expr::Path("add".into())),
        args: vec![CallArg::Positional(Expr::Literal(Literal::Int {
            raw: "2i64".to_owned(),
            value: 2,
            suffix: Some("i64".to_owned()),
        }))],
    };

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
        rhs: Box::new(Expr::Literal(Literal::Int {
            raw: "80i64".to_owned(),
            value: 80,
            suffix: Some("i64".to_owned()),
        })),
    };
    let expected = TypeRef::Function {
        params: vec![TypeRef::Path("i64".to_owned())],
        return_type: Box::new(TypeRef::Path("bool".to_owned())),
    };
    let ids = BTreeMap::new();
    let helpers = Vec::new();

    let lowered = lower_runtime_expr_strict_with_expected_type(
        &expr,
        Some(&expected),
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
    let expr = Expr::Closure {
        params: vec![ClosureParam::new(Pattern::Ident("score".to_owned()), None)],
        body: Box::new(Expr::Binary {
            lhs: Box::new(Expr::Path("score".into())),
            op: BinaryOp::Gt,
            rhs: Box::new(Expr::Literal(Literal::Int {
                raw: "80i64".to_owned(),
                value: 80,
                suffix: Some("i64".to_owned()),
            })),
        }),
    };

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
fn strict_runtime_lowers_expression_callee_call_to_apply() {
    let expr = Expr::Call {
        callee: Box::new(Expr::Call {
            callee: Box::new(Expr::Path("make_adder".into())),
            args: vec![CallArg::Positional(Expr::Literal(Literal::Int {
                raw: "2i64".to_owned(),
                value: 2,
                suffix: Some("i64".to_owned()),
            }))],
        }),
        args: vec![CallArg::Positional(Expr::Literal(Literal::Int {
            raw: "5i64".to_owned(),
            value: 5,
            suffix: Some("i64".to_owned()),
        }))],
    };

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
    let expr = Expr::MethodCall {
        receiver: Box::new(Expr::Path("math".into())),
        method: "matmul_f64".to_owned(),
        args: vec![
            CallArg::Positional(Expr::Path("lhs".into())),
            CallArg::Positional(Expr::Path("rhs".into())),
        ],
    };

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
    let expr = Expr::MethodCall {
        receiver: Box::new(Expr::Path("infer".into())),
        method: "matmul_bias_add_f32".to_owned(),
        args: vec![
            CallArg::Positional(Expr::Path("lhs".into())),
            CallArg::Positional(Expr::Path("rhs".into())),
            CallArg::Positional(Expr::Path("bias".into())),
        ],
    };

    let lowered = lower_runtime_expr_strict(&expr).expect("adapter method lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Call { callee, args }
            if callee.as_label() == "infer.matmul_bias_add_f32" && args.len() == 3
    ));
}

#[test]
fn strict_runtime_substitutes_pipe_left_placeholder() {
    let expr = Expr::Pipe {
        lhs: Box::new(Expr::Path("value".into())),
        rhs: Box::new(Expr::Call {
            callee: Box::new(Expr::Path("clamp".into())),
            args: vec![
                CallArg::Positional(Expr::Literal(Literal::Int {
                    raw: "0i64".to_owned(),
                    value: 0,
                    suffix: Some("i64".to_owned()),
                })),
                CallArg::Positional(Expr::Placeholder(Placeholder::PipeLeft)),
                CallArg::Positional(Expr::Literal(Literal::Int {
                    raw: "100i64".to_owned(),
                    value: 100,
                    suffix: Some("i64".to_owned()),
                })),
            ],
        }),
    };

    let lowered = lower_runtime_expr_strict(&expr).expect("pipe placeholder lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Call { callee, args }
            if callee.as_label() == "clamp"
                && matches!(args.as_slice(), [
                    RuntimeExpr::Value(_),
                    RuntimeExpr::Local(name),
                    RuntimeExpr::Value(_),
                ] if name == "value")
    ));
}

#[test]
fn strict_runtime_lowers_data_last_pipe_to_direct_call() {
    let expr = Expr::Pipe {
        lhs: Box::new(Expr::Path("value".into())),
        rhs: Box::new(Expr::Call {
            callee: Box::new(Expr::Path("normalize".into())),
            args: vec![CallArg::Positional(Expr::Literal(Literal::Int {
                raw: "2i64".to_owned(),
                value: 2,
                suffix: Some("i64".to_owned()),
            }))],
        }),
    };

    let lowered = lower_runtime_expr_strict(&expr).expect("data-last pipe lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::Call { callee, args }
            if callee.as_label() == "normalize"
                && matches!(args.as_slice(), [
                    RuntimeExpr::Value(_),
                    RuntimeExpr::Local(name),
                ] if name == "value")
    ));
}

#[test]
fn strict_runtime_lowers_data_last_pipe_to_partial_helper_apply() {
    let expr = Expr::Pipe {
        lhs: Box::new(Expr::Literal(Literal::Int {
            raw: "2i64".to_owned(),
            value: 2,
            suffix: Some("i64".to_owned()),
        })),
        rhs: Box::new(Expr::Path("add".into())),
    };
    let helpers = vec![add_i64_helper()];
    let ids = BTreeMap::from([("add".to_owned(), helpers[0].id)]);

    let lowered =
        lower_runtime_expr_strict_with_pure(&expr, RuntimePureHelperLookup::new(&ids, &helpers))
            .expect("data-last helper pipe lowers");

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
fn strict_runtime_lowers_data_last_pipe_call_to_exact_helper_call() {
    let expr = Expr::Pipe {
        lhs: Box::new(Expr::Literal(Literal::Int {
            raw: "2i64".to_owned(),
            value: 2,
            suffix: Some("i64".to_owned()),
        })),
        rhs: Box::new(Expr::Call {
            callee: Box::new(Expr::Path("add".into())),
            args: vec![CallArg::Positional(Expr::Literal(Literal::Int {
                raw: "1i64".to_owned(),
                value: 1,
                suffix: Some("i64".to_owned()),
            }))],
        }),
    };
    let helpers = vec![add_i64_helper()];
    let ids = BTreeMap::from([("add".to_owned(), helpers[0].id)]);

    let lowered =
        lower_runtime_expr_strict_with_pure(&expr, RuntimePureHelperLookup::new(&ids, &helpers))
            .expect("data-last exact helper pipe lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::PureCall { helper, args }
            if helper == RuntimePureHelperId(0)
                && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(lhs), RuntimeExpr::Value(rhs)]
                        if lhs == &RuntimeValue::i64(1) && rhs == &RuntimeValue::i64(2)
                )
    ));
}

#[test]
fn strict_runtime_lowers_partial_placeholder_map_body() {
    let expr = Expr::MethodCall {
        receiver: Box::new(Expr::Path("values".into())),
        method: "map".to_owned(),
        args: vec![CallArg::Positional(Expr::Binary {
            lhs: Box::new(Expr::Placeholder(Placeholder::Partial)),
            op: BinaryOp::Add,
            rhs: Box::new(Expr::Literal(Literal::Int {
                raw: "1i64".to_owned(),
                value: 1,
                suffix: Some("i64".to_owned()),
            })),
        })],
    };

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
    let expr = Expr::MethodCall {
        receiver: Box::new(Expr::Path("choices".into())),
        method: "filter".to_owned(),
        args: vec![CallArg::Positional(Expr::Field {
            target: Box::new(Expr::Placeholder(Placeholder::Partial)),
            field: "enabled".to_owned(),
        })],
    };

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
fn strict_runtime_lowers_data_last_filter_map_pipeline() {
    let expr = Expr::Pipe {
        lhs: Box::new(Expr::Pipe {
            lhs: Box::new(Expr::Path("choices".into())),
            rhs: Box::new(Expr::Call {
                callee: Box::new(Expr::Path("filter".into())),
                args: vec![CallArg::Positional(Expr::Field {
                    target: Box::new(Expr::Placeholder(Placeholder::Partial)),
                    field: "enabled".to_owned(),
                })],
            }),
        }),
        rhs: Box::new(Expr::Call {
            callee: Box::new(Expr::Path("map".into())),
            args: vec![CallArg::Positional(Expr::Field {
                target: Box::new(Expr::Placeholder(Placeholder::Partial)),
                field: "label".to_owned(),
            })],
        }),
    };

    let lowered = lower_runtime_expr_strict(&expr).expect("data-last pipeline lowers");

    assert!(matches!(
            lowered,
            RuntimeExpr::Map { source, param, body }
                if param == "_item"
                    && matches!(source.as_ref(), RuntimeExpr::Filter { source, param, body }
                        if param == "_item"
                            && matches!(source.as_ref(), RuntimeExpr::Local(name) if name == "choices")
                            && matches!(
                                body.as_ref(),
                                RuntimeExpr::Field { target, field }
                                    if field == "enabled"
                                        && matches!(
                                            target.as_ref(),
                                            RuntimeExpr::Local(name) if name == "_item"
                                        )
                            )
                    )
                    && matches!(
                        body.as_ref(),
                        RuntimeExpr::Field { target, field }
                            if field == "label"
                                && matches!(
                                    target.as_ref(),
                                    RuntimeExpr::Local(name) if name == "_item"
                            )
                )
    ));
}

#[test]
fn strict_runtime_unwraps_try_around_runtime_method_calls() {
    let expr = Expr::Try {
        expr: Box::new(Expr::MethodCall {
            receiver: Box::new(Expr::Field {
                target: Box::new(Expr::Path("frame".into())),
                field: "objects".to_owned(),
            }),
            method: "require_role".to_owned(),
            args: vec![CallArg::Positional(Expr::Literal(Literal::String(
                "dialogue_textbox".to_owned(),
            )))],
        }),
    };

    let lowered = lower_runtime_expr_strict(&expr).expect("try method call lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::MethodCall { method, args, .. }
            if method == "require_role" && args.len() == 1
    ));
}

#[test]
fn strict_runtime_array_repeat_folds_literal_value_sequence() {
    let expr = Expr::ArrayRepeat {
        value: Box::new(Expr::Literal(Literal::Int {
            raw: "2i64".to_owned(),
            value: 2,
            suffix: Some("i64".to_owned()),
        })),
        len: Box::new(Expr::Literal(Literal::Int {
            raw: "4".to_owned(),
            value: 4,
            suffix: None,
        })),
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
        let expr = Expr::Literal(Literal::Int {
            raw: format!("7{suffix}"),
            value: 7,
            suffix: Some(suffix.to_owned()),
        });

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
        Expr::Literal(Literal::Int {
            raw: "1i64".to_owned(),
            value: 1,
            suffix: Some("i64".to_owned()),
        }),
        Expr::Literal(Literal::Int {
            raw: "2i64".to_owned(),
            value: 2,
            suffix: Some("i64".to_owned()),
        }),
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
    let nan_expr = Expr::Field {
        target: Box::new(Expr::Field {
            target: Box::new(Expr::Path("std".into())),
            field: "f32".to_owned(),
        }),
        field: "nan".to_owned(),
    };
    let nan_lowered = lower_runtime_expr_strict(&nan_expr).expect("std f32 nan lowers");
    assert!(matches!(
        nan_lowered,
        RuntimeExpr::Value(RuntimeValue::F32(value)) if value.is_nan()
    ));

    let sqrt_expr = Expr::Call {
        callee: Box::new(Expr::Field {
            target: Box::new(Expr::Field {
                target: Box::new(Expr::Path("std".into())),
                field: "f64".to_owned(),
            }),
            field: "sqrt".to_owned(),
        }),
        args: vec![CallArg::Positional(Expr::Literal(Literal::Float {
            raw: "4.0f64".to_owned(),
            suffix: Some(FloatSuffix::F64),
        }))],
    };
    let sqrt_lowered = lower_runtime_expr_strict(&sqrt_expr).expect("std f64 sqrt lowers");
    assert!(matches!(
        sqrt_lowered,
        RuntimeExpr::Call { callee, .. } if callee.as_label() == "std.f64.sqrt"
    ));
}

#[test]
fn strict_runtime_field_lowering_uses_record_projection_when_ordinal_is_known() {
    let expr = Expr::Field {
        target: Box::new(Expr::RecordLiteral(vec![
            (
                "score".to_owned(),
                Expr::Literal(Literal::Int {
                    raw: "7".to_owned(),
                    value: 7,
                    suffix: None,
                }),
            ),
            (
                "label".to_owned(),
                Expr::Literal(Literal::String("ok".to_owned())),
            ),
        ])),
        field: "label".to_owned(),
    };

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
            Expr::Literal(Literal::Int {
                raw: "1".to_owned(),
                value: 1,
                suffix: None,
            }),
            Expr::Literal(Literal::Bool(true)),
        ])),
        index: Box::new(Expr::Literal(Literal::Int {
            raw: "1".to_owned(),
            value: 1,
            suffix: None,
        })),
    };

    let lowered = lower_runtime_expr_strict(&expr).expect("tuple index lowers");

    assert!(matches!(
        lowered,
        RuntimeExpr::ProjectTuple { ordinal: 1, .. }
    ));
}

#[test]
fn numeric_bracket_seq_lowers_to_dense_i32_sequence() {
    let expr = Expr::NumericBracketSeq(arcweft_lang_hir::syntax::expr::NumericBracketSeq::new(
        vec![1, 2, 3],
        None,
    ));

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
    let expr = Expr::NumericBracketSeq(arcweft_lang_hir::syntax::expr::NumericBracketSeq::new(
        vec![1, 2, 3],
        Some(suffix.to_owned()),
    ));
    lower_runtime_expr_strict(&expr).expect("suffixed numeric seq lowers")
}
