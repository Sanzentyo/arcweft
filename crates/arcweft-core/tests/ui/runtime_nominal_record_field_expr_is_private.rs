#![allow(unreachable_code)]

use arcweft_core::value::{RuntimeExpr, RuntimeNominalRecordFieldExpr, RuntimeValue};

fn main() {
    let _ = RuntimeNominalRecordFieldExpr {
        field: todo!(),
        name: "field".to_owned(),
        value: RuntimeExpr::Value(RuntimeValue::Unit),
    };
}
