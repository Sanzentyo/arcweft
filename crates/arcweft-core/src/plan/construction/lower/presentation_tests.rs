use super::*;
use crate::{
    presentation::factory_test_support::{ALIAS, BOOL, HANDLE, fixture},
    value::RuntimeCallTarget,
};
#[test]
fn factory_builder_rejects_wrong_input_and_result_types_before_expression_publication() {
    for (input, result, accepted) in [
        (BOOL, HANDLE, true),
        (ALIAS, HANDLE, false),
        (BOOL, BOOL, false),
    ] {
        let (builder, call) = fixture();
        let expression = builder.lower_expression(RuntimeExprSeed::new(
            result,
            RuntimeExprSeedKind::Call {
                callee: RuntimeCallTarget::PresentationFactory(call),
                args: Box::new([RuntimeCallArgumentSeed::new(
                    RuntimeExprSeed::new(
                        input,
                        RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
                    ),
                    RuntimeCallArgumentMode::Value,
                    0,
                )]),
            },
        ));
        assert_eq!(
            expression.is_ok(),
            accepted,
            "input {input:?}, result {result:?}: {expression:?}"
        );
    }
}
