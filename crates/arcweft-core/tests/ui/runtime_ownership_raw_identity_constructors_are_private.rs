use arcweft_core::{
    runtime_id::ExecutionInstanceId,
    value::RuntimeRecordFieldId,
};
use std::num::{NonZeroU32, NonZeroU64};

fn main() {
    let _execution = ExecutionInstanceId(NonZeroU64::MIN);
    let _field = RuntimeRecordFieldId(NonZeroU32::MIN);
}
