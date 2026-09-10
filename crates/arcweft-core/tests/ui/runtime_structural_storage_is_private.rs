use arcweft_core::value::{RecordSeq, RuntimeRecordValue, TupleSeq};

fn main() {
    let _ = RuntimeRecordValue { fields: Vec::new() };
    let _ = RecordSeq { len: 0, fields: Vec::new() };
    let _ = TupleSeq { len: 0, columns: Vec::new() };
}
