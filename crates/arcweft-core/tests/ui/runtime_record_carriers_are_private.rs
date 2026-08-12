use arcweft_core::value::{RecordSeqField, RuntimeFieldValue, RuntimeSeq, RuntimeValue};

fn main() {
    let _ = RuntimeFieldValue {
        name: "field".to_owned(),
        value: RuntimeValue::Unit,
    };
    let _ = RecordSeqField {
        name: "field".to_owned(),
        values: RuntimeSeq::values(Vec::new()),
    };
}
