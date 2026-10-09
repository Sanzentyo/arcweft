#[path = "support/execution.rs"]
mod execution;

use arcweft_core::value::RuntimeValue;

#[test]
fn constant_type_arguments_execute_through_ordinary_native_and_decoded_awbc_calls() {
    let source = "entry cli @entry.main { goto @flow.main }\nfn pair() -> Array<String, 2> { [\"left\", \"right\"] }\nflow main() -> String { let values = pair(); return values[1usize] }";
    execution::assert_native_return(source, "right");
    execution::assert_awbc_return(source, RuntimeValue::String("right".to_owned()));
}
