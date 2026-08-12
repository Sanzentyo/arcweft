use arcweft_core::value::{RecordSeqField, RuntimeSeq};

fn main() {
    let fields: Vec<RecordSeqField> = Vec::new();
    let _ = RuntimeSeq::record_columns(0, fields);
}
