use arcweft_core::{
    entry::{RuntimeNominalTypeId, TypeLayoutHash},
    pattern::RuntimeSemanticTypeId,
    value::RuntimeNominalRecordValue,
};

fn main() {
    let _ = RuntimeNominalRecordValue::new(
        RuntimeNominalTypeId::try_new("test.Record").unwrap(),
        RuntimeSemanticTypeId::from_bytes([1; 32]),
        TypeLayoutHash::from_bytes([2; 32]),
        vec![],
    );
}
