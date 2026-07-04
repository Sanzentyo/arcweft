use super::{RuntimeInt, RuntimeSeq, RuntimeUInt, RuntimeValue, runtime_sequence_values};
use crate::time::LogicalDuration;
pub fn runtime_sequence_repeat_value(value: &RuntimeValue, len: usize) -> RuntimeValue {
    match value {
        RuntimeValue::Unit => runtime_sequence_dense_units(len),
        RuntimeValue::Bool(value) => runtime_sequence_dense_bool(vec![*value; len]),
        RuntimeValue::Int(value) => repeat_runtime_int(*value, len),
        RuntimeValue::UInt(value) => repeat_runtime_uint(*value, len),
        RuntimeValue::F32(value) => runtime_sequence_dense_f32(vec![*value; len]),
        RuntimeValue::F64(value) => runtime_sequence_dense_f64(vec![*value; len]),
        RuntimeValue::Char(value) => runtime_sequence_dense_chars(vec![*value; len]),
        RuntimeValue::Duration(value) => runtime_sequence_dense_durations(vec![*value; len]),
        RuntimeValue::String(value) => runtime_sequence_dense_strings(vec![value.clone(); len]),
        RuntimeValue::EntityRef(value) => {
            runtime_sequence_dense_entity_refs(vec![value.clone(); len])
        }
        value => runtime_sequence_values(vec![value.clone(); len]),
    }
}

fn repeat_runtime_int(value: RuntimeInt, len: usize) -> RuntimeValue {
    match value {
        RuntimeInt::I8(value) => runtime_sequence_dense_i8(vec![value; len]),
        RuntimeInt::I16(value) => runtime_sequence_dense_i16(vec![value; len]),
        RuntimeInt::I32(value) => runtime_sequence_dense_i32(vec![value; len]),
        RuntimeInt::I64(value) => runtime_sequence_dense_i64(vec![value; len]),
        RuntimeInt::I128(value) => runtime_sequence_dense_i128(vec![value; len]),
        RuntimeInt::ISize(value) => runtime_sequence_dense_isize(vec![value; len]),
    }
}

fn repeat_runtime_uint(value: RuntimeUInt, len: usize) -> RuntimeValue {
    match value {
        RuntimeUInt::U8(value) => runtime_sequence_dense_u8(vec![value; len]),
        RuntimeUInt::U16(value) => runtime_sequence_dense_u16(vec![value; len]),
        RuntimeUInt::U32(value) => runtime_sequence_dense_u32(vec![value; len]),
        RuntimeUInt::U64(value) => runtime_sequence_dense_u64(vec![value; len]),
        RuntimeUInt::U128(value) => runtime_sequence_dense_u128(vec![value; len]),
        RuntimeUInt::USize(value) => runtime_sequence_dense_usize(vec![value; len]),
    }
}

pub fn runtime_sequence_dense_units(len: usize) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_units(len))
}

pub fn runtime_sequence_dense_i64(values: Vec<i64>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_i64(values))
}

pub fn runtime_sequence_dense_i128(values: Vec<i128>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_i128(values))
}

pub fn runtime_sequence_dense_isize(values: Vec<i64>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_isize(values))
}

pub fn runtime_sequence_dense_i8(values: Vec<i8>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_i8(values))
}

pub fn runtime_sequence_dense_i16(values: Vec<i16>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_i16(values))
}

pub fn runtime_sequence_dense_i32(values: Vec<i32>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_i32(values))
}

pub fn runtime_sequence_dense_u8(values: Vec<u8>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_u8(values))
}

pub fn runtime_sequence_dense_u16(values: Vec<u16>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_u16(values))
}

pub fn runtime_sequence_dense_u32(values: Vec<u32>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_u32(values))
}

pub fn runtime_sequence_dense_u64(values: Vec<u64>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_u64(values))
}

pub fn runtime_sequence_dense_u128(values: Vec<u128>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_u128(values))
}

pub fn runtime_sequence_dense_usize(values: Vec<u64>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_usize(values))
}

pub fn runtime_sequence_dense_f32(values: Vec<f32>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_f32(values))
}

pub fn runtime_sequence_dense_f64(values: Vec<f64>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_f64(values))
}

pub fn runtime_sequence_dense_bool(values: Vec<bool>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_bool(values))
}

pub fn runtime_sequence_dense_bytes(values: Vec<u8>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_bytes(values))
}

pub fn runtime_sequence_dense_chars(values: Vec<char>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_chars(values))
}

pub fn runtime_sequence_dense_durations(values: Vec<LogicalDuration>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_durations(values))
}

pub fn runtime_sequence_dense_strings(values: Vec<String>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_strings(values))
}

pub fn runtime_sequence_dense_entity_refs(values: Vec<String>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_entity_refs(values))
}
