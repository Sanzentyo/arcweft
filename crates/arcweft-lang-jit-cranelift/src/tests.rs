use super::*;
use arcweft_core::pure::{
    PureFunctionBackendKind, VmPureFunctionBackend, compare_pure_function_backend,
};
use arcweft_core::value::RuntimeCallTarget;

fn int_binding(name: &str, value: i64) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value: RuntimeValue::i64(value),
    }
}

fn i32_binding(name: &str, value: i32) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value: RuntimeValue::i32(value),
    }
}

fn i8_binding(name: &str, value: i8) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value: RuntimeValue::i8(value),
    }
}

fn i16_binding(name: &str, value: i16) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value: RuntimeValue::i16(value),
    }
}

fn i128_binding(name: &str, value: i128) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value: RuntimeValue::i128(value),
    }
}

fn u32_binding(name: &str, value: u32) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value: RuntimeValue::u32(value),
    }
}

fn u8_binding(name: &str, value: u8) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value: RuntimeValue::u8(value),
    }
}

fn u16_binding(name: &str, value: u16) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value: RuntimeValue::u16(value),
    }
}

fn u128_binding(name: &str, value: u128) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value: RuntimeValue::u128(value),
    }
}

fn u64_binding(name: &str, value: u64) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value: RuntimeValue::u64(value),
    }
}

fn f32_binding(name: &str, value: f32) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value: RuntimeValue::F32(value),
    }
}

fn f64_binding(name: &str, value: f64) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value: RuntimeValue::F64(value),
    }
}

fn object_symbols_contain(symbols: &[&str], expected: &str) -> bool {
    symbols
        .iter()
        .any(|symbol| *symbol == expected || symbol.strip_prefix('_') == Some(expected))
}

fn assert_object_symbols(object: &ObjectPureInputs) {
    assert!(!object.object_bytes.is_empty());
    assert!(
        !object
            .object_bytes
            .windows(3)
            .any(|window| window == b":\\")
    );

    use cranelift_object::object::{Object, ObjectSymbol};
    let parsed = cranelift_object::object::File::parse(object.object_bytes.as_slice())
        .expect("emitted object parses");
    let symbols = parsed
        .symbols()
        .filter_map(|symbol| symbol.name().ok())
        .collect::<Vec<_>>();
    assert!(object_symbols_contain(&symbols, &object.entry_symbol));
    assert!(object_symbols_contain(&symbols, &object.batch_symbol));
    if let Some(batch_sum_symbol) = object.batch_sum_symbol.as_deref() {
        assert!(object_symbols_contain(&symbols, batch_sum_symbol));
    }
}

fn assert_batch_object_symbols(object: &ObjectPureBatchInputs) {
    assert!(!object.object_bytes.is_empty());
    assert!(
        !object
            .object_bytes
            .windows(3)
            .any(|window| window == b":\\")
    );

    use cranelift_object::object::{Object, ObjectSymbol};
    let parsed = cranelift_object::object::File::parse(object.object_bytes.as_slice())
        .expect("emitted object parses");
    let symbols = parsed
        .symbols()
        .filter_map(|symbol| symbol.name().ok())
        .collect::<Vec<_>>();
    assert!(object_symbols_contain(&symbols, &object.batch_symbol));
    assert!(object_symbols_contain(&symbols, &object.batch_sum_symbol));
}

fn assert_bundle_object_symbols(object: &ObjectPureBundle) {
    assert!(!object.object_bytes.is_empty());
    assert!(
        !object
            .object_bytes
            .windows(3)
            .any(|window| window == b":\\")
    );

    use cranelift_object::object::{Object, ObjectSymbol};
    let parsed = cranelift_object::object::File::parse(object.object_bytes.as_slice())
        .expect("emitted object bundle parses");
    let symbols = parsed
        .symbols()
        .filter_map(|symbol| symbol.name().ok())
        .collect::<Vec<_>>();
    for helper in &object.helpers {
        helper
            .entrypoints
            .for_each_symbol(|symbol| assert!(object_symbols_contain(&symbols, symbol)));
    }
}

#[test]
fn cranelift_benchmark_batch_define_can_emit_object_symbol() {
    let request = PureFunctionRequest::new(
        "bench_score",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        [int_binding("base", 0), int_binding("bonus", 0)],
    );
    let mut module = object_module().expect("object module is available");
    let defined = define_i64_benchmark_batch(
        &mut module,
        "arcweft_test_i64_benchmark_batch",
        &request,
        ["base", "bonus"],
    )
    .expect("benchmark batch defines into object module");
    assert_eq!(defined.param_names, ["base", "bonus"]);
    assert_eq!(defined.stats.evaluated_binary_ops, 1);

    let object_bytes = emit_object_bytes(module).expect("object bytes emit");
    assert!(!object_bytes.windows(3).any(|window| window == b":\\"));

    use cranelift_object::object::{Object, ObjectSymbol};
    let parsed = cranelift_object::object::File::parse(object_bytes.as_slice())
        .expect("emitted benchmark object parses");
    let symbols = parsed
        .symbols()
        .filter_map(|symbol| symbol.name().ok())
        .collect::<Vec<_>>();
    assert!(object_symbols_contain(
        &symbols,
        "arcweft_test_i64_benchmark_batch"
    ));
}

#[test]
fn cranelift_jit_evaluates_integer_helper_and_matches_vm() {
    let request = PureFunctionRequest::new(
        "score",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Call {
                callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::Add),
                args: vec![
                    RuntimeExpr::Local("bonus".to_owned()),
                    RuntimeExpr::Value(RuntimeValue::i64(2)),
                ],
            }),
        },
        [int_binding("base", 3), int_binding("bonus", 4)],
    );

    let conformance = compare_pure_function_backend(
        &VmPureFunctionBackend,
        &CraneliftPureFunctionBackend,
        &request,
    )
    .expect("Cranelift JIT matches VM for supported pure integer helper");

    assert!(conformance.matches_vm);
    assert_eq!(conformance.candidate.backend, PureFunctionBackendKind::Jit);
    assert_eq!(conformance.candidate.value, RuntimeValue::i64(18));
    assert_eq!(conformance.candidate.stats.evaluated_calls, 1);
    assert_eq!(conformance.candidate.stats.evaluated_binary_ops, 1);
}

#[test]
fn cranelift_compiled_helper_can_be_called_repeatedly() {
    let request = PureFunctionRequest::new(
        "score",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(21))),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(21))),
        },
        [],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_i64(&request)
        .expect("Cranelift compiles integer helper");

    assert_eq!(compiled.call(), 42);
    assert_eq!(compiled.call(), 42);
    assert_eq!(compiled.stats().evaluated_binary_ops, 1);
}

#[test]
fn cranelift_define_i64_entry_defines_module_function_without_jit_wrapper() {
    let request = PureFunctionRequest::new(
        "score_entry_define",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
            }),
        },
        [int_binding("base", 3), int_binding("bonus", 4)],
    );
    let mut module = jit_module().expect("JIT module is available");

    let defined = define_i64_entry(&mut module, "arcweft_test_defined_i64_entry", &request)
        .expect("i64 entry is defined into the module");

    assert_eq!(defined.stats.evaluated_binary_ops, 2);
    module
        .finalize_definitions()
        .expect("defined entry finalizes");
    let entry_code = module.get_finalized_function(defined.entry);
    let caller = native_call::I64InputCaller::from_code(entry_code, 0)
        .expect("defined entry has a supported native signature");
    assert_eq!(caller.call(&[]).expect("defined entry call succeeds"), 18);
}

#[test]
fn cranelift_compiled_helper_accepts_runtime_integer_inputs() {
    let request = PureFunctionRequest::new(
        "score_inputs",
        RuntimeExpr::If {
            condition: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Ge,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(3))),
            }),
            then_expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Call {
                    callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::Add),
                    args: vec![
                        RuntimeExpr::Local("bonus".to_owned()),
                        RuntimeExpr::Value(RuntimeValue::i64(2)),
                    ],
                }),
            }),
            else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::i64(0))),
        },
        [int_binding("base", 3), int_binding("bonus", 4)],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_i64_with_inputs(&request, ["base", "bonus"])
        .expect("Cranelift compiles parameterized integer helper");

    assert_eq!(compiled.param_names(), ["base", "bonus"]);
    assert_eq!(compiled.call(&[3, 4]).expect("call succeeds"), 18);
    assert_eq!(
        compiled
            .call_i64_args(RuntimeI64Args::new([3, 4, 0, 0], 2))
            .expect("packed call succeeds"),
        18
    );
    assert_eq!(compiled.call(&[2, 99]).expect("call succeeds"), 0);
    assert_eq!(compiled.call(&[7, 1]).expect("call succeeds"), 21);
    let mut out = [0; 3];
    compiled
        .call_flat_batch(&[3, 4, 2, 99, 7, 1], &mut out)
        .expect("flat rows batch succeeds");
    assert_eq!(out, [18, 0, 21]);
    assert_eq!(
        compiled
            .call_flat_batch_sum(&[3, 4, 2, 99, 7, 1], 3)
            .expect("flat rows batch sum succeeds"),
        39
    );
}

#[test]
fn cranelift_define_i64_with_inputs_defines_module_functions_without_jit_wrapper() {
    let request = PureFunctionRequest::new(
        "score_define_inputs",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
            }),
        },
        [int_binding("base", 3), int_binding("bonus", 4)],
    );
    let mut module = jit_module().expect("JIT module is available");

    let defined = define_i64_with_inputs(
        &mut module,
        "arcweft_test_defined_i64",
        &request,
        ["base", "bonus"],
    )
    .expect("i64 helper is defined into the module");

    assert_eq!(defined.param_names, ["base", "bonus"]);
    assert_eq!(defined.stats.evaluated_binary_ops, 2);
    module
        .finalize_definitions()
        .expect("defined functions finalize");
    let entry_code = module.get_finalized_function(defined.entry);
    let batch_code = module.get_finalized_function(defined.batch);
    let batch_sum_code = module.get_finalized_function(defined.batch_sum);
    let caller = native_call::I64InputCaller::from_code(entry_code, defined.param_names.len())
        .expect("defined entry has a supported native signature");

    assert_eq!(caller.call(&[3, 4]), Some(18));
    let mut out = [0; 3];
    assert!(native_call::call_i64_rows_batch(
        batch_code,
        &[3, 4, 2, 99, 7, 1],
        2,
        &mut out
    ));
    assert_eq!(out, [18, 202, 21]);
    assert_eq!(
        native_call::call_i64_rows_batch_sum(batch_sum_code, &[3, 4, 2, 99, 7, 1], 2, 3),
        Some(241)
    );
}

#[test]
fn cranelift_emits_i64_object_with_entry_and_batch_symbols() {
    let request = PureFunctionRequest::new(
        "score-object i64",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
            }),
        },
        [int_binding("base", 3), int_binding("bonus", 4)],
    );

    let object = CraneliftPureFunctionBackend
        .emit_object_i64_with_inputs(&request, ["base", "bonus"])
        .expect("Cranelift emits an i64 object");

    assert!(!object.object_bytes.is_empty());
    assert_eq!(object.entry_symbol, "arcweft_pure_score_object_i64_entry");
    assert_eq!(
        object.batch_symbol,
        "arcweft_pure_score_object_i64_rows_batch"
    );
    assert_eq!(
        object.batch_sum_symbol,
        Some("arcweft_pure_score_object_i64_rows_batch_sum".to_owned())
    );
    assert_eq!(object.param_names, ["base", "bonus"]);
    assert_eq!(object.stats.evaluated_binary_ops, 2);
    assert_object_symbols(&object);
}

#[test]
fn cranelift_emits_width_specific_objects_with_expected_symbols() {
    let backend = CraneliftPureFunctionBackend;

    let i32_request = PureFunctionRequest::new(
        "score object i32",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        [i32_binding("base", 3), i32_binding("bonus", 4)],
    );
    let i32_object = backend
        .emit_object_i32_with_inputs(&i32_request, ["base", "bonus"])
        .expect("Cranelift emits an i32 object");
    assert_eq!(
        i32_object.entry_symbol,
        "arcweft_pure_score_object_i32_entry"
    );
    assert!(i32_object.batch_sum_symbol.is_some());
    assert_object_symbols(&i32_object);

    let u32_request = PureFunctionRequest::new(
        "score object u32",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        [u32_binding("base", 3), u32_binding("bonus", 4)],
    );
    let u32_object = backend
        .emit_object_u32_with_inputs(&u32_request, ["base", "bonus"])
        .expect("Cranelift emits a u32 object");
    assert_eq!(
        u32_object.entry_symbol,
        "arcweft_pure_score_object_u32_entry"
    );
    assert!(u32_object.batch_sum_symbol.is_some());
    assert_object_symbols(&u32_object);

    let u64_request = PureFunctionRequest::new(
        "score object u64",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        [u64_binding("base", 3), u64_binding("bonus", 4)],
    );
    let u64_object = backend
        .emit_object_u64_with_inputs(&u64_request, ["base", "bonus"])
        .expect("Cranelift emits a u64 object");
    assert_eq!(
        u64_object.entry_symbol,
        "arcweft_pure_score_object_u64_entry"
    );
    assert!(u64_object.batch_sum_symbol.is_some());
    assert_object_symbols(&u64_object);

    let f32_request = PureFunctionRequest::new(
        "score object f32",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
        },
        [f32_binding("base", 3.0), f32_binding("scale", 4.0)],
    );
    let f32_object = backend
        .emit_object_f32_with_inputs(&f32_request, ["base", "scale"])
        .expect("Cranelift emits an f32 object");
    assert_eq!(
        f32_object.entry_symbol,
        "arcweft_pure_score_object_f32_entry"
    );
    assert!(f32_object.batch_sum_symbol.is_none());
    assert_object_symbols(&f32_object);

    let f64_request = PureFunctionRequest::new(
        "score object f64",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
        },
        [f64_binding("base", 3.0), f64_binding("scale", 4.0)],
    );
    let f64_object = backend
        .emit_object_f64_with_inputs(&f64_request, ["base", "scale"])
        .expect("Cranelift emits an f64 object");
    assert_eq!(
        f64_object.entry_symbol,
        "arcweft_pure_score_object_f64_entry"
    );
    assert!(f64_object.batch_sum_symbol.is_none());
    assert_object_symbols(&f64_object);
}

#[test]
fn cranelift_emits_small_and_wide_integer_objects_with_expected_symbols() {
    let backend = CraneliftPureFunctionBackend;

    let i8_request = PureFunctionRequest::new(
        "score object i8",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        [i8_binding("base", 3), i8_binding("bonus", 4)],
    );
    let i8_object = backend
        .emit_object_i8_with_inputs(&i8_request, ["base", "bonus"])
        .expect("Cranelift emits an i8 object");
    assert_eq!(i8_object.entry_symbol, "arcweft_pure_score_object_i8_entry");
    assert!(i8_object.batch_sum_symbol.is_some());
    assert_object_symbols(&i8_object);

    let i16_request = PureFunctionRequest::new(
        "score object i16",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        [i16_binding("base", 3), i16_binding("bonus", 4)],
    );
    let i16_object = backend
        .emit_object_i16_with_inputs(&i16_request, ["base", "bonus"])
        .expect("Cranelift emits an i16 object");
    assert_eq!(
        i16_object.entry_symbol,
        "arcweft_pure_score_object_i16_entry"
    );
    assert!(i16_object.batch_sum_symbol.is_some());
    assert_object_symbols(&i16_object);

    let u8_request = PureFunctionRequest::new(
        "score object u8",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        [u8_binding("base", 3), u8_binding("bonus", 4)],
    );
    let u8_object = backend
        .emit_object_u8_with_inputs(&u8_request, ["base", "bonus"])
        .expect("Cranelift emits a u8 object");
    assert_eq!(u8_object.entry_symbol, "arcweft_pure_score_object_u8_entry");
    assert!(u8_object.batch_sum_symbol.is_some());
    assert_object_symbols(&u8_object);

    let u16_request = PureFunctionRequest::new(
        "score object u16",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        [u16_binding("base", 3), u16_binding("bonus", 4)],
    );
    let u16_object = backend
        .emit_object_u16_with_inputs(&u16_request, ["base", "bonus"])
        .expect("Cranelift emits a u16 object");
    assert_eq!(
        u16_object.entry_symbol,
        "arcweft_pure_score_object_u16_entry"
    );
    assert!(u16_object.batch_sum_symbol.is_some());
    assert_object_symbols(&u16_object);

    let i128_request = PureFunctionRequest::new(
        "score object i128",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        [
            i128_binding("base", i128::MAX - 3),
            i128_binding("bonus", 2),
        ],
    );
    let i128_object = backend
        .emit_object_i128_batch_with_inputs(&i128_request, ["base", "bonus"])
        .expect("Cranelift emits an i128 batch object");
    assert_eq!(
        i128_object.batch_symbol,
        "arcweft_pure_score_object_i128_rows_batch"
    );
    assert_eq!(
        i128_object.batch_sum_symbol,
        "arcweft_pure_score_object_i128_rows_batch_sum"
    );
    assert_batch_object_symbols(&i128_object);

    let u128_request = PureFunctionRequest::new(
        "score object u128",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        [
            u128_binding("base", u128::MAX - 3),
            u128_binding("bonus", 2),
        ],
    );
    let u128_object = backend
        .emit_object_u128_batch_with_inputs(&u128_request, ["base", "bonus"])
        .expect("Cranelift emits a u128 batch object");
    assert_eq!(
        u128_object.batch_symbol,
        "arcweft_pure_score_object_u128_rows_batch"
    );
    assert_eq!(
        u128_object.batch_sum_symbol,
        "arcweft_pure_score_object_u128_rows_batch_sum"
    );
    assert_batch_object_symbols(&u128_object);
}

#[test]
fn cranelift_emits_multi_helper_object_bundle_with_symbol_table() {
    let backend = CraneliftPureFunctionBackend;
    let i32_request = PureFunctionRequest::new(
        "score bundle i32",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        [i32_binding("base", 3), i32_binding("bonus", 4)],
    );
    let f32_request = PureFunctionRequest::new(
        "score bundle f32",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
        },
        [f32_binding("base", 3.0), f32_binding("scale", 4.0)],
    );
    let u128_request = PureFunctionRequest::new(
        "score bundle u128",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        [
            u128_binding("base", u128::MAX - 3),
            u128_binding("bonus", 2),
        ],
    );

    let bundle = backend
        .emit_object_bundle([
            PureObjectBundleRequest::new(&i32_request, PureObjectInputKind::I32, ["base", "bonus"]),
            PureObjectBundleRequest::new(&f32_request, PureObjectInputKind::F32, ["base", "scale"]),
            PureObjectBundleRequest::new(
                &u128_request,
                PureObjectInputKind::U128,
                ["base", "bonus"],
            ),
        ])
        .expect("Cranelift emits a multi-helper object bundle");

    assert_eq!(bundle.helpers.len(), 3);
    assert_eq!(bundle.helpers[0].name, "score bundle i32");
    assert_eq!(bundle.helpers[0].kind, PureObjectInputKind::I32);
    assert_eq!(
        bundle.helpers[0].entrypoints.entry_symbol(),
        Some("arcweft_pure_bundle_0_score_bundle_i32_entry")
    );
    assert_eq!(
        bundle.helpers[0].entrypoints.batch_sum_symbol(),
        Some("arcweft_pure_bundle_0_score_bundle_i32_rows_batch_sum")
    );
    assert_eq!(bundle.helpers[1].kind, PureObjectInputKind::F32);
    assert_eq!(
        bundle.helpers[1].entrypoints.entry_symbol(),
        Some("arcweft_pure_bundle_1_score_bundle_f32_entry")
    );
    assert!(bundle.helpers[1].entrypoints.batch_sum_symbol().is_none());
    assert_eq!(bundle.helpers[2].kind, PureObjectInputKind::U128);
    assert!(bundle.helpers[2].entrypoints.entry_symbol().is_none());
    assert_eq!(
        bundle.helpers[2].entrypoints.batch_symbol(),
        Some("arcweft_pure_bundle_2_score_bundle_u128_rows_batch")
    );
    assert_eq!(
        bundle.helpers[2].entrypoints.batch_sum_symbol(),
        Some("arcweft_pure_bundle_2_score_bundle_u128_rows_batch_sum")
    );
    assert_bundle_object_symbols(&bundle);
}

#[test]
fn cranelift_rejects_empty_object_bundle() {
    let error = match CraneliftPureFunctionBackend.emit_object_bundle([]) {
        Ok(_) => panic!("empty object bundles are invalid"),
        Err(error) => error,
    };

    assert!(matches!(error, CraneliftCodegenError::UnsupportedExpr(_)));
}

#[test]
fn cranelift_define_i32_with_inputs_defines_module_functions_without_jit_wrapper() {
    let request = PureFunctionRequest::new(
        "score_define_i32_inputs",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i32(2))),
            }),
        },
        [i32_binding("base", 3), i32_binding("bonus", 4)],
    );
    let mut module = jit_module().expect("JIT module is available");

    let defined = define_i32_with_inputs(
        &mut module,
        "arcweft_test_defined_i32",
        &request,
        ["base", "bonus"],
    )
    .expect("i32 helper is defined into the module");

    assert_eq!(defined.param_names, ["base", "bonus"]);
    assert_eq!(defined.stats.evaluated_binary_ops, 2);
    module
        .finalize_definitions()
        .expect("defined functions finalize");
    let entry_code = module.get_finalized_function(defined.entry);
    let batch_code = module.get_finalized_function(defined.batch);
    let batch_sum_code = module.get_finalized_function(defined.batch_sum);
    let caller = native_call::I32InputCaller::from_code(entry_code, defined.param_names.len())
        .expect("defined entry has a supported native signature");

    assert_eq!(caller.call(&[3, 4]), Some(18));
    let mut out = [0; 3];
    assert!(native_call::call_i32_rows_batch(
        batch_code,
        &[3, 4, 2, 99, 7, 1],
        2,
        &mut out
    ));
    assert_eq!(out, [18, 202, 21]);
    assert_eq!(
        native_call::call_i32_rows_batch_sum(batch_sum_code, &[3, 4, 2, 99, 7, 1], 2, 3),
        Some(241)
    );
}

#[test]
fn cranelift_define_u32_with_inputs_defines_module_functions_without_jit_wrapper() {
    let request = PureFunctionRequest::new(
        "score_define_u32_inputs",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("divisor".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u32(2))),
            }),
        },
        [u32_binding("base", 3), u32_binding("divisor", 4)],
    );
    let mut module = jit_module().expect("JIT module is available");

    let defined = define_u32_with_inputs(
        &mut module,
        "arcweft_test_defined_u32",
        &request,
        ["base", "divisor"],
    )
    .expect("u32 helper is defined into the module");

    assert_eq!(defined.param_names, ["base", "divisor"]);
    assert_eq!(defined.stats.evaluated_binary_ops, 2);
    module
        .finalize_definitions()
        .expect("defined functions finalize");
    let entry_code = module.get_finalized_function(defined.entry);
    let batch_code = module.get_finalized_function(defined.batch);
    let batch_sum_code = module.get_finalized_function(defined.batch_sum);
    let caller = native_call::U32InputCaller::from_code(entry_code, defined.param_names.len())
        .expect("defined entry has a supported native signature");

    assert_eq!(caller.call(&[3, 4]), Some(18));
    let mut out = [0; 3];
    assert!(native_call::call_u32_rows_batch(
        batch_code,
        &[3, 4, 2, 99, 7, 1],
        2,
        &mut out
    ));
    assert_eq!(out, [18, 202, 21]);
    assert_eq!(
        native_call::call_u32_rows_batch_sum(batch_sum_code, &[3, 4, 2, 99, 7, 1], 2, 3),
        Some(241)
    );
}

#[test]
fn cranelift_define_u64_with_inputs_defines_module_functions_without_jit_wrapper() {
    let request = PureFunctionRequest::new(
        "score_define_u64_inputs",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("divisor".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u64(2))),
            }),
        },
        [u64_binding("base", 3), u64_binding("divisor", 4)],
    );
    let mut module = jit_module().expect("JIT module is available");

    let defined = define_u64_with_inputs(
        &mut module,
        "arcweft_test_defined_u64",
        &request,
        ["base", "divisor"],
    )
    .expect("u64 helper is defined into the module");

    assert_eq!(defined.param_names, ["base", "divisor"]);
    assert_eq!(defined.stats.evaluated_binary_ops, 2);
    module
        .finalize_definitions()
        .expect("defined functions finalize");
    let entry_code = module.get_finalized_function(defined.entry);
    let batch_code = module.get_finalized_function(defined.batch);
    let batch_sum_code = module.get_finalized_function(defined.batch_sum);
    let caller = native_call::U64InputCaller::from_code(entry_code, defined.param_names.len())
        .expect("defined entry has a supported native signature");

    assert_eq!(caller.call(&[3, 4]), Some(18));
    let mut out = [0; 3];
    assert!(native_call::call_u64_rows_batch(
        batch_code,
        &[3, 4, 2, 99, 7, 1],
        2,
        &mut out
    ));
    assert_eq!(out, [18, 202, 21]);
    assert_eq!(
        native_call::call_u64_rows_batch_sum(batch_sum_code, &[3, 4, 2, 99, 7, 1], 2, 3),
        Some(241)
    );
}

#[test]
fn cranelift_define_f32_with_inputs_defines_module_functions_without_jit_wrapper() {
    let request = PureFunctionRequest::new(
        "score_define_f32_inputs",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::f32(0.5))),
            }),
        },
        [f32_binding("base", 1.0), f32_binding("scale", 1.0)],
    );
    let mut module = jit_module().expect("JIT module is available");

    let defined = define_f32_with_inputs(
        &mut module,
        "arcweft_test_defined_f32",
        &request,
        ["base", "scale"],
    )
    .expect("f32 helper is defined into the module");

    assert_eq!(defined.param_names, ["base", "scale"]);
    assert_eq!(defined.stats.evaluated_binary_ops, 2);
    module
        .finalize_definitions()
        .expect("defined functions finalize");
    let entry_code = module.get_finalized_function(defined.entry);
    let batch_code = module.get_finalized_function(defined.batch);
    let caller = native_call::F32InputCaller::from_code(entry_code, defined.param_names.len())
        .expect("defined entry has a supported native signature");

    assert_eq!(caller.call(&[2.0, 3.0]), Some(7.0));
    let mut out = [0.0; 3];
    assert!(native_call::call_f32_rows_batch(
        batch_code,
        &[2.0, 3.0, 4.0, 1.5, -2.0, 0.25],
        2,
        &mut out
    ));
    assert_eq!(out, [7.0, 8.0, -1.5]);
}

#[test]
fn cranelift_define_f64_with_inputs_defines_module_functions_without_jit_wrapper() {
    let request = PureFunctionRequest::new(
        "score_define_f64_inputs",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::f64(0.5))),
            }),
        },
        [f64_binding("base", 1.0), f64_binding("scale", 1.0)],
    );
    let mut module = jit_module().expect("JIT module is available");

    let defined = define_f64_with_inputs(
        &mut module,
        "arcweft_test_defined_f64",
        &request,
        ["base", "scale"],
    )
    .expect("f64 helper is defined into the module");

    assert_eq!(defined.param_names, ["base", "scale"]);
    assert_eq!(defined.stats.evaluated_binary_ops, 2);
    module
        .finalize_definitions()
        .expect("defined functions finalize");
    let entry_code = module.get_finalized_function(defined.entry);
    let batch_code = module.get_finalized_function(defined.batch);
    let caller = native_call::F64InputCaller::from_code(entry_code, defined.param_names.len())
        .expect("defined entry has a supported native signature");

    assert_eq!(caller.call(&[2.0, 3.0]), Some(7.0));
    let mut out = [0.0; 3];
    assert!(native_call::call_f64_rows_batch(
        batch_code,
        &[2.0, 3.0, 4.0, 1.5, -2.0, 0.25],
        2,
        &mut out
    ));
    assert_eq!(out, [7.0, 8.0, -1.5]);
}

#[test]
fn cranelift_define_small_int_with_inputs_defines_module_functions_without_jit_wrapper() {
    let request = PureFunctionRequest::new(
        "score_define_i8_inputs",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i8(2))),
            }),
        },
        [i8_binding("base", 3), i8_binding("bonus", 4)],
    );
    let mut module = jit_module().expect("JIT module is available");

    let defined = define_small_int_with_inputs(
        &mut module,
        "arcweft_test_defined_i8",
        &request,
        ["base", "bonus"],
        SmallIntKind::I8,
    )
    .expect("i8 helper is defined into the module");

    assert_eq!(defined.param_names, ["base", "bonus"]);
    assert_eq!(defined.stats.evaluated_binary_ops, 2);
    module
        .finalize_definitions()
        .expect("defined functions finalize");
    let entry_code = module.get_finalized_function(defined.entry);
    let batch_code = module.get_finalized_function(defined.batch);
    let batch_sum_code = module.get_finalized_function(defined.batch_sum);
    let caller = native_call::I8InputCaller::from_code(entry_code, defined.param_names.len())
        .expect("defined entry has a supported native signature");

    assert_eq!(caller.call(&[3, 4]), Some(18));
    let mut out = [0; 3];
    assert!(native_call::call_i8_rows_batch(
        batch_code,
        &[3, 4, -2, 1, 7, 1],
        2,
        &mut out
    ));
    assert_eq!(out, [18, -6, 21]);
    assert_eq!(
        native_call::call_i8_rows_batch_sum(batch_sum_code, &[3, 4, -2, 1, 7, 1], 2, 3),
        Some(33)
    );
}

#[test]
fn cranelift_define_wide_int_batch_defines_module_functions_without_jit_wrapper() {
    let request_i128 = PureFunctionRequest::new(
        "score_define_i128_inputs",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("value".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("delta".to_owned())),
        },
        [i128_binding("value", 0), i128_binding("delta", 0)],
    );
    let mut module = jit_module().expect("JIT module is available");

    let defined_i128 = define_small_int_batch_with_inputs(
        &mut module,
        "arcweft_test_defined_i128",
        &request_i128,
        ["value", "delta"],
        SmallIntKind::I128,
    )
    .expect("i128 batch helper is defined into the module");
    module
        .finalize_definitions()
        .expect("defined functions finalize");
    let batch_code = module.get_finalized_function(defined_i128.batch);
    let batch_sum_code = module.get_finalized_function(defined_i128.batch_sum);
    let mut out = [0; 3];
    assert!(native_call::call_i128_rows_batch(
        batch_code,
        &[i128::MAX, -1, i128::MIN, 1, 7, -2],
        2,
        &mut out
    ));
    assert_eq!(out, [i128::MAX - 1, i128::MIN + 1, 5]);
    assert_eq!(
        native_call::call_i128_rows_batch_sum(
            batch_sum_code,
            &[i128::MAX, -1, i128::MIN, 1, 7, -2],
            2,
            3
        ),
        Some(4)
    );

    let request_u128 = PureFunctionRequest::new(
        "score_define_u128_inputs",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("value".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("delta".to_owned())),
        },
        [u128_binding("value", 0), u128_binding("delta", 0)],
    );
    let mut module = jit_module().expect("JIT module is available");
    let defined_u128 = define_small_int_batch_with_inputs(
        &mut module,
        "arcweft_test_defined_u128",
        &request_u128,
        ["value", "delta"],
        SmallIntKind::U128,
    )
    .expect("u128 batch helper is defined into the module");
    module
        .finalize_definitions()
        .expect("defined functions finalize");
    let batch_code = module.get_finalized_function(defined_u128.batch);
    let batch_sum_code = module.get_finalized_function(defined_u128.batch_sum);
    let mut out = [0; 3];
    assert!(native_call::call_u128_rows_batch(
        batch_code,
        &[u128::MAX, 1, 10, 5, 7, 2],
        2,
        &mut out
    ));
    assert_eq!(out, [0, 15, 9]);
    assert_eq!(
        native_call::call_u128_rows_batch_sum(batch_sum_code, &[u128::MAX, 1, 10, 5, 7, 2], 2, 3),
        Some(24)
    );
}

#[test]
fn cranelift_compiled_helper_accepts_runtime_i32_inputs_without_widening() {
    let request = PureFunctionRequest::new(
        "score_i32",
        RuntimeExpr::If {
            condition: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Ge,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i32(3))),
            }),
            then_expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i32(2))),
                }),
            }),
            else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::i32(0))),
        },
        [i32_binding("base", 0), i32_binding("bonus", 0)],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_i32_with_inputs(&request, ["base", "bonus"])
        .expect("Cranelift compiles parameterized i32 helper");

    assert_eq!(compiled.call(&[3, 4]).expect("i32 call succeeds"), 18);
    assert_eq!(compiled.call(&[2, 99]).expect("i32 call succeeds"), 0);
    let mut out = [0; 3];
    compiled
        .call_flat_batch(&[3, 4, 2, 99, 7, 1], &mut out)
        .expect("i32 flat rows batch succeeds");
    assert_eq!(out, [18, 0, 21]);
    assert_eq!(
        compiled
            .call_flat_batch_sum(&[3, 4, 2, 99, 7, 1], 3)
            .expect("i32 flat rows batch sum succeeds"),
        39
    );
}

#[test]
fn cranelift_compiled_helper_accepts_small_signed_integer_inputs_without_widening() {
    let request_i8 = PureFunctionRequest::new(
        "score_i8",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i8(2))),
            }),
        },
        [i8_binding("base", 0), i8_binding("bonus", 0)],
    );
    let compiled_i8 = CraneliftPureFunctionBackend
        .compile_i8_with_inputs(&request_i8, ["base", "bonus"])
        .expect("Cranelift compiles parameterized i8 helper");
    assert_eq!(compiled_i8.call(&[3, 4]).expect("i8 call succeeds"), 18);
    let mut out_i8 = [0; 3];
    compiled_i8
        .call_flat_batch(&[3, 4, -2, 1, 7, 1], &mut out_i8)
        .expect("i8 flat rows batch succeeds");
    assert_eq!(out_i8, [18, -6, 21]);
    assert_eq!(
        compiled_i8
            .call_flat_batch_sum(&[3, 4, -2, 1, 7, 1], 3)
            .expect("i8 flat rows batch sum succeeds"),
        33
    );

    let request_i16 = PureFunctionRequest::new(
        "score_i16",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i16(2))),
            }),
        },
        [i16_binding("base", 0), i16_binding("bonus", 0)],
    );
    let compiled_i16 = CraneliftPureFunctionBackend
        .compile_i16_with_inputs(&request_i16, ["base", "bonus"])
        .expect("Cranelift compiles parameterized i16 helper");
    assert_eq!(compiled_i16.call(&[30, 4]).expect("i16 call succeeds"), 180);
    let mut out_i16 = [0; 3];
    compiled_i16
        .call_flat_batch(&[30, 4, -20, 1, 70, 1], &mut out_i16)
        .expect("i16 flat rows batch succeeds");
    assert_eq!(out_i16, [180, -60, 210]);
    assert_eq!(
        compiled_i16
            .call_flat_batch_sum(&[30, 4, -20, 1, 70, 1], 3)
            .expect("i16 flat rows batch sum succeeds"),
        330
    );
}

#[test]
fn cranelift_compiled_helper_accepts_small_unsigned_integer_inputs_without_widening() {
    let request_u8 = PureFunctionRequest::new(
        "score_u8",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u8(2))),
            }),
        },
        [u8_binding("base", 0), u8_binding("bonus", 0)],
    );
    let compiled_u8 = CraneliftPureFunctionBackend
        .compile_u8_with_inputs(&request_u8, ["base", "bonus"])
        .expect("Cranelift compiles parameterized u8 helper");
    assert_eq!(compiled_u8.call(&[3, 4]).expect("u8 call succeeds"), 18);
    let mut out_u8 = [0; 3];
    compiled_u8
        .call_flat_batch(&[3, 4, 2, 1, 7, 1], &mut out_u8)
        .expect("u8 flat rows batch succeeds");
    assert_eq!(out_u8, [18, 6, 21]);
    assert_eq!(
        compiled_u8
            .call_flat_batch_sum(&[3, 4, 2, 1, 7, 1], 3)
            .expect("u8 flat rows batch sum succeeds"),
        45
    );

    let request_u16 = PureFunctionRequest::new(
        "score_u16",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u16(2))),
            }),
        },
        [u16_binding("base", 0), u16_binding("bonus", 0)],
    );
    let compiled_u16 = CraneliftPureFunctionBackend
        .compile_u16_with_inputs(&request_u16, ["base", "bonus"])
        .expect("Cranelift compiles parameterized u16 helper");
    assert_eq!(compiled_u16.call(&[30, 4]).expect("u16 call succeeds"), 180);
    let mut out_u16 = [0; 3];
    compiled_u16
        .call_flat_batch(&[30, 4, 20, 1, 70, 1], &mut out_u16)
        .expect("u16 flat rows batch succeeds");
    assert_eq!(out_u16, [180, 60, 210]);
    assert_eq!(
        compiled_u16
            .call_flat_batch_sum(&[30, 4, 20, 1, 70, 1], 3)
            .expect("u16 flat rows batch sum succeeds"),
        450
    );
}

#[test]
fn cranelift_compiled_helper_accepts_runtime_u32_inputs_without_widening() {
    let request = PureFunctionRequest::new(
        "score_u32",
        RuntimeExpr::If {
            condition: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Ge,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u32(u32::MAX - 4))),
            }),
            then_expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Div,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("divisor".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u32(1))),
                }),
            }),
            else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::u32(0))),
        },
        [u32_binding("base", 0), u32_binding("divisor", 0)],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_u32_with_inputs(&request, ["base", "divisor"])
        .expect("Cranelift compiles parameterized u32 helper");

    assert_eq!(
        compiled
            .call(&[u32::MAX - 1, 1])
            .expect("u32 call succeeds"),
        (u32::MAX - 1) / 2
    );
    assert_eq!(compiled.call(&[3, 99]).expect("u32 call succeeds"), 0);
    let mut out = [0; 3];
    compiled
        .call_flat_batch(&[u32::MAX - 1, 1, 3, 99, u32::MAX, 4], &mut out)
        .expect("u32 flat rows batch succeeds");
    assert_eq!(out, [(u32::MAX - 1) / 2, 0, u32::MAX / 5]);
    assert_eq!(
        compiled
            .call_flat_batch_sum(&[u32::MAX - 1, 1, 3, 99, u32::MAX, 4], 3)
            .expect("u32 flat rows batch sum succeeds"),
        i64::from((u32::MAX - 1) / 2) + i64::from(u32::MAX / 5)
    );
}

#[test]
fn cranelift_compiled_helper_accepts_runtime_u64_inputs_without_widening() {
    let request = PureFunctionRequest::new(
        "score_u64",
        RuntimeExpr::If {
            condition: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Ge,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u64(u64::MAX - 4))),
            }),
            then_expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Div,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("divisor".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u64(1))),
                }),
            }),
            else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::u64(0))),
        },
        [u64_binding("base", 0), u64_binding("divisor", 0)],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_u64_with_inputs(&request, ["base", "divisor"])
        .expect("Cranelift compiles parameterized u64 helper");

    assert_eq!(
        compiled
            .call(&[u64::MAX - 1, 1])
            .expect("u64 call succeeds"),
        (u64::MAX - 1) / 2
    );
    assert_eq!(compiled.call(&[3, 99]).expect("u64 call succeeds"), 0);
    let mut out = [0; 3];
    compiled
        .call_flat_batch(&[u64::MAX - 1, 1, 3, 99, u64::MAX, 4], &mut out)
        .expect("u64 flat rows batch succeeds");
    assert_eq!(out, [(u64::MAX - 1) / 2, 0, u64::MAX / 5]);
}

#[test]
fn cranelift_compiled_helper_accepts_runtime_f32_inputs_without_value_boundary() {
    let request = PureFunctionRequest::new(
        "score_f32",
        RuntimeExpr::If {
            condition: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Gt,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F32(2.0))),
            }),
            then_expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F32(0.5))),
                }),
            }),
            else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::F32(0.0))),
        },
        [f32_binding("base", 0.0), f32_binding("scale", 0.0)],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_f32_with_inputs(&request, ["base", "scale"])
        .expect("Cranelift compiles parameterized f32 helper");

    assert_eq!(compiled.call(&[3.0, 1.5]).expect("f32 call succeeds"), 6.0);
    assert_eq!(compiled.call(&[2.0, 99.0]).expect("f32 call succeeds"), 0.0);
    let mut out = [0.0; 3];
    compiled
        .call_flat_batch(&[3.0, 1.5, 2.0, 99.0, 4.0, 0.5], &mut out)
        .expect("f32 flat rows batch succeeds");
    assert_eq!(out, [6.0, 0.0, 4.0]);
}

#[test]
fn cranelift_compiled_helper_accepts_runtime_f64_inputs_without_value_boundary() {
    let request = PureFunctionRequest::new(
        "score_f64",
        RuntimeExpr::If {
            condition: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Gt,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F64(2.0))),
            }),
            then_expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F64(0.5))),
                }),
            }),
            else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::F64(0.0))),
        },
        [f64_binding("base", 0.0), f64_binding("scale", 0.0)],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_f64_with_inputs(&request, ["base", "scale"])
        .expect("Cranelift compiles parameterized f64 helper");

    assert_eq!(compiled.call(&[3.0, 1.5]).expect("f64 call succeeds"), 6.0);
    assert_eq!(compiled.call(&[2.0, 99.0]).expect("f64 call succeeds"), 0.0);
    let mut out = [0.0; 3];
    compiled
        .call_flat_batch(&[3.0, 1.5, 2.0, 99.0, 4.0, 0.5], &mut out)
        .expect("f64 flat rows batch succeeds");
    assert_eq!(out.map(f64::to_bits), [6.0f64, 0.0, 4.0].map(f64::to_bits));
}

#[test]
fn cranelift_compiled_helper_lowers_supported_std_f32_intrinsics() {
    let request = PureFunctionRequest::new(
        "std_f32_intrinsics",
        RuntimeExpr::Call {
            callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF32MulAdd),
            args: vec![
                RuntimeExpr::Call {
                    callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF32Sqrt),
                    args: vec![RuntimeExpr::Local("base".to_owned())],
                },
                RuntimeExpr::Call {
                    callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF32Abs),
                    args: vec![RuntimeExpr::Local("scale".to_owned())],
                },
                RuntimeExpr::Call {
                    callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF32Fract),
                    args: vec![RuntimeExpr::Local("offset".to_owned())],
                },
            ],
        },
        [
            f32_binding("base", 0.0),
            f32_binding("scale", 0.0),
            f32_binding("offset", 0.0),
        ],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_f32_with_inputs(&request, ["base", "scale", "offset"])
        .expect("Cranelift compiles supported std.f32 intrinsics");

    assert_eq!(
        compiled
            .call(&[9.0, -2.0, 1.25])
            .expect("f32 call succeeds")
            .to_bits(),
        6.25f32.to_bits()
    );
    let mut out = [0.0; 2];
    compiled
        .call_flat_batch(&[9.0, -2.0, 1.25, 16.0, -0.5, 2.75], &mut out)
        .expect("f32 flat rows batch succeeds");
    assert_eq!(out.map(f32::to_bits), [6.25f32, 2.75].map(f32::to_bits));
}

#[test]
fn cranelift_compiled_helper_lowers_supported_std_f64_intrinsics() {
    let request = PureFunctionRequest::new(
        "std_f64_intrinsics",
        RuntimeExpr::Call {
            callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF64MulAdd),
            args: vec![
                RuntimeExpr::Call {
                    callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF64Sqrt),
                    args: vec![RuntimeExpr::Local("base".to_owned())],
                },
                RuntimeExpr::Call {
                    callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF64Ceil),
                    args: vec![RuntimeExpr::Local("scale".to_owned())],
                },
                RuntimeExpr::Call {
                    callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF64Fract),
                    args: vec![RuntimeExpr::Local("offset".to_owned())],
                },
            ],
        },
        [
            f64_binding("base", 0.0),
            f64_binding("scale", 0.0),
            f64_binding("offset", 0.0),
        ],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_f64_with_inputs(&request, ["base", "scale", "offset"])
        .expect("Cranelift compiles supported std.f64 intrinsics");

    assert_eq!(
        compiled
            .call(&[25.0, 1.2, 3.5])
            .expect("f64 call succeeds")
            .to_bits(),
        10.5f64.to_bits()
    );
    let mut out = [0.0; 2];
    compiled
        .call_flat_batch(&[25.0, 1.2, 3.5, 16.0, 2.0, 7.25], &mut out)
        .expect("f64 flat rows batch succeeds");
    assert_eq!(out.map(f64::to_bits), [10.5f64, 8.25].map(f64::to_bits));
}

#[test]
fn cranelift_compiled_batch_matches_repeated_input_calls() {
    let request = PureFunctionRequest::new(
        "score_inputs",
        RuntimeExpr::If {
            condition: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Ge,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(3))),
            }),
            then_expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Call {
                    callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::Add),
                    args: vec![
                        RuntimeExpr::Local("bonus".to_owned()),
                        RuntimeExpr::Value(RuntimeValue::i64(2)),
                    ],
                }),
            }),
            else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::i64(0))),
        },
        [int_binding("base", 3), int_binding("bonus", 4)],
    );

    let backend = CraneliftPureFunctionBackend;
    let compiled = backend
        .compile_i64_with_inputs(&request, ["base", "bonus"])
        .expect("Cranelift compiles parameterized integer helper");
    let batch = backend
        .compile_i64_batch(&request, ["base", "bonus"])
        .expect("Cranelift compiles batch helper");
    let expected = (0..8)
        .map(|iteration| {
            let base = i64::from((7 + iteration) % 5) + 1;
            let bonus = i64::from((14 + iteration) % 6) + 1;
            compiled.call(&[base, bonus]).expect("call succeeds")
        })
        .sum::<i64>();

    assert_eq!(batch.param_names(), ["base", "bonus"]);
    assert_eq!(batch.call(7, 0, 8).expect("batch call succeeds"), expected);
    assert_eq!(batch.stats().evaluated_binary_ops, 2);
}

#[test]
fn cranelift_compiled_helper_evaluates_lexical_let() {
    let request = PureFunctionRequest::new(
        "score_with_local",
        RuntimeExpr::Let {
            name: "boosted".to_owned(),
            expr: Box::new(RuntimeExpr::Call {
                callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::Add),
                args: vec![
                    RuntimeExpr::Local("bonus".to_owned()),
                    RuntimeExpr::Value(RuntimeValue::i64(2)),
                ],
            }),
            body: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Local("boosted".to_owned())),
            }),
        },
        [int_binding("base", 0), int_binding("bonus", 0)],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_i64_with_inputs(&request, ["base", "bonus"])
        .expect("Cranelift compiles lexical let");

    assert_eq!(compiled.call(&[3, 4]).expect("call succeeds"), 18);
    assert_eq!(compiled.call(&[5, 1]).expect("call succeeds"), 15);
}

#[test]
fn cranelift_compiled_helper_accepts_four_runtime_integer_inputs() {
    let request = PureFunctionRequest::new(
        "sum4",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("a".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Local("b".to_owned())),
            }),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("c".to_owned())),
                op: RuntimeBinaryOp::Sub,
                rhs: Box::new(RuntimeExpr::Local("d".to_owned())),
            }),
        },
        [
            int_binding("a", 0),
            int_binding("b", 0),
            int_binding("c", 0),
            int_binding("d", 0),
        ],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_i64_with_inputs(&request, ["a", "b", "c", "d"])
        .expect("Cranelift compiles four-input integer helper");

    assert_eq!(compiled.call(&[2, 3, 10, 4]).expect("call succeeds"), 30);
}

#[test]
fn cranelift_compiled_helper_evaluates_division_and_negation() {
    let request = PureFunctionRequest::new(
        "normalized_delta",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Unary {
                op: RuntimeUnaryOp::Neg,
                expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("score".to_owned())),
                    op: RuntimeBinaryOp::Sub,
                    rhs: Box::new(RuntimeExpr::Local("baseline".to_owned())),
                }),
            }),
            op: RuntimeBinaryOp::Div,
            rhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
        },
        [
            int_binding("score", 0),
            int_binding("baseline", 0),
            int_binding("scale", 1),
        ],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_i64_with_inputs(&request, ["score", "baseline", "scale"])
        .expect("Cranelift compiles i64 div and unary negation");

    assert_eq!(compiled.call(&[21, 9, 3]).expect("call succeeds"), -4);
    assert_eq!(compiled.call(&[8, 20, 4]).expect("call succeeds"), 3);
    assert_eq!(compiled.stats().evaluated_binary_ops, 2);
}

#[test]
fn cranelift_jit_evaluates_integer_if_and_matches_vm() {
    let request = PureFunctionRequest::new(
        "score_branch",
        RuntimeExpr::If {
            condition: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("score".to_owned())),
                op: RuntimeBinaryOp::Ge,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(10))),
            }),
            then_expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("score".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
            }),
            else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::i64(0))),
        },
        [int_binding("score", 12)],
    );

    let conformance = compare_pure_function_backend(
        &VmPureFunctionBackend,
        &CraneliftPureFunctionBackend,
        &request,
    )
    .expect("Cranelift JIT matches VM for integer if helper");

    assert!(conformance.matches_vm);
    assert_eq!(conformance.candidate.value, RuntimeValue::i64(24));
}

#[test]
fn cranelift_jit_rejects_non_integer_helpers() {
    let request = PureFunctionRequest::new(
        "trim_label",
        RuntimeExpr::Value(RuntimeValue::String("x".to_owned())),
        [],
    );

    let error = CraneliftPureFunctionBackend
        .evaluate_jit(&request)
        .expect_err("string-heavy helpers are outside the JIT subset");

    assert!(matches!(error, CraneliftCodegenError::UnsupportedExpr(_)));
}

#[test]
fn cranelift_jit_unsupported_expr_uses_display_label() {
    let request = PureFunctionRequest::new("tuple_value", RuntimeExpr::Tuple(vec![]), []);

    let error = CraneliftPureFunctionBackend
        .evaluate_jit(&request)
        .expect_err("tuple helpers are outside the current JIT subset")
        .to_string();

    assert!(error.contains("expression `tuple/0` is outside the JIT subset"));
    assert!(!error.contains("RuntimeExpr"));
}

#[test]
fn cranelift_jit_unsupported_operator_uses_display_label() {
    let request = PureFunctionRequest::new(
        "bool_and",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(1))),
            op: RuntimeBinaryOp::And,
            rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(1))),
        },
        [],
    );

    let error = CraneliftPureFunctionBackend
        .evaluate_jit(&request)
        .expect_err("boolean operators are outside the i64 JIT subset")
        .to_string();

    assert!(error.contains("binary operator `&&` is outside the JIT subset"));
}

#[test]
fn cranelift_i128_batch_preserves_full_width_runtime_inputs() {
    let request = PureFunctionRequest::new(
        "wide_i128_add",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("value".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("delta".to_owned())),
        },
        [i128_binding("value", 0), i128_binding("delta", 0)],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_i128_batch_with_inputs(&request, ["value", "delta"])
        .expect("Cranelift compiles pointer-ABI i128 batch helper");
    let inputs = [i128::MAX - 5, 3, i128::MIN + 9, -4];
    let mut out = [0_i128; 2];

    compiled
        .call_flat_batch(&inputs, &mut out)
        .expect("full-width i128 runtime inputs are loaded through pointers");

    assert_eq!(out, [i128::MAX - 2, i128::MIN + 5]);
}

#[test]
fn cranelift_u128_batch_preserves_full_width_runtime_inputs() {
    let request = PureFunctionRequest::new(
        "wide_u128_add",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("value".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("delta".to_owned())),
        },
        [u128_binding("value", 0), u128_binding("delta", 0)],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_u128_batch_with_inputs(&request, ["value", "delta"])
        .expect("Cranelift compiles pointer-ABI u128 batch helper");
    let inputs = [u128::MAX - 7, 2, 1_u128 << 100, 5];
    let mut out = [0_u128; 2];

    compiled
        .call_flat_batch(&inputs, &mut out)
        .expect("full-width u128 runtime inputs are loaded through pointers");

    assert_eq!(out, [u128::MAX - 5, (1_u128 << 100) + 5]);
}

#[test]
fn cranelift_i128_batch_lowers_full_width_literals() {
    let request = PureFunctionRequest::new(
        "wide_i128_literal",
        RuntimeExpr::Value(RuntimeValue::i128(i128::MIN + 123)),
        [],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_i128_batch_with_inputs(&request, std::iter::empty::<&str>())
        .expect("Cranelift lowers full-width i128 literal with iconcat");
    let mut out = [0_i128; 2];

    compiled
        .call_flat_batch(&[], &mut out)
        .expect("zero-arity i128 literal batch succeeds");

    assert_eq!(out, [i128::MIN + 123, i128::MIN + 123]);
}

#[test]
fn cranelift_i128_batch_lowers_full_width_captured_bindings() {
    let request = PureFunctionRequest::new(
        "wide_i128_binding",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i128(123))),
        },
        [i128_binding("base", i128::MIN)],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_i128_batch_with_inputs(&request, std::iter::empty::<&str>())
        .expect("Cranelift lowers full-width i128 captured binding with iconcat");
    let mut out = [0_i128; 2];

    compiled
        .call_flat_batch(&[], &mut out)
        .expect("zero-arity i128 captured-binding batch succeeds");

    assert_eq!(out, [i128::MIN + 123, i128::MIN + 123]);
}

#[test]
fn cranelift_u128_batch_lowers_full_width_literals_and_bindings() {
    let request = PureFunctionRequest::new(
        "wide_u128_literal",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u128(1_u128 << 96))),
        },
        [u128_binding("base", 1_u128 << 100)],
    );

    let compiled = CraneliftPureFunctionBackend
        .compile_u128_batch_with_inputs(&request, std::iter::empty::<&str>())
        .expect("Cranelift lowers full-width u128 literal and captured binding with iconcat");
    let mut out = [0_u128; 2];

    compiled
        .call_flat_batch(&[], &mut out)
        .expect("zero-arity u128 literal batch succeeds");

    assert_eq!(out, [(1_u128 << 100) + (1_u128 << 96); 2]);
}
