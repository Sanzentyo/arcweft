use super::{
    DenseSeq, DenseSeqKind, DenseSeqStorage, RecordSeq, RecordSeqField, RuntimeEvalError,
    RuntimeExactInteger, RuntimeExactIntegerSlice, RuntimeExactIntegerSliceMut, RuntimeFieldValue,
    RuntimeISizeValue, RuntimeInt, RuntimeSeq, RuntimeSeqError, RuntimeUInt, RuntimeUSizeValue,
    RuntimeValue, TupleSeq, materialize_i64_sequence, runtime_sequence_dense_i8,
    runtime_sequence_dense_i16, runtime_sequence_dense_i32, runtime_sequence_dense_i128,
    runtime_sequence_dense_u8, runtime_sequence_dense_u16, runtime_sequence_dense_u32,
    runtime_sequence_dense_u64, runtime_sequence_dense_u128, runtime_value_label,
};
use crate::plan::{RuntimePureInputType, RuntimePureOutputType};
use crate::time::LogicalDuration;

impl TupleSeq {
    pub fn new(len: usize, columns: Vec<RuntimeSeq>) -> Result<Self, RuntimeSeqError> {
        if let Some((ordinal, actual)) = columns
            .iter()
            .enumerate()
            .find_map(|(ordinal, column)| (column.len() != len).then_some((ordinal, column.len())))
        {
            return Err(RuntimeSeqError::ColumnLength {
                ordinal,
                expected: len,
                actual,
            });
        }
        Ok(Self { len, columns })
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn columns(&self) -> &[RuntimeSeq] {
        &self.columns
    }

    pub fn column(&self, ordinal: usize) -> Option<&RuntimeSeq> {
        self.columns.get(ordinal)
    }

    fn into_values(self) -> Vec<RuntimeValue> {
        let row_count = self.len;
        let columns = self.columns;
        (0..row_count)
            .map(|row| {
                RuntimeValue::Tuple(
                    columns
                        .iter()
                        .map(|column| column.value_at(row))
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    }

    #[must_use]
    fn tail_from(&self, index: usize) -> Self {
        Self {
            len: self.len.saturating_sub(index),
            columns: self
                .columns
                .iter()
                .map(|column| column.tail_from(index))
                .collect(),
        }
    }

    fn value_at(&self, index: usize) -> RuntimeValue {
        assert!(
            index < self.len,
            "tuple column sequence index out of bounds"
        );
        RuntimeValue::Tuple(
            self.columns
                .iter()
                .map(|column| column.value_at(index))
                .collect(),
        )
    }
}

impl RecordSeq {
    pub fn new(len: usize, fields: Vec<RecordSeqField>) -> Result<Self, RuntimeSeqError> {
        for (ordinal, field) in fields.iter().enumerate() {
            if field.values.len() != len {
                return Err(RuntimeSeqError::ColumnLength {
                    ordinal,
                    expected: len,
                    actual: field.values.len(),
                });
            }
            if fields[..ordinal]
                .iter()
                .any(|candidate| candidate.name == field.name)
            {
                return Err(RuntimeSeqError::DuplicateRecordField {
                    field: field.name.clone(),
                });
            }
        }
        Ok(Self { len, fields })
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn fields(&self) -> &[RecordSeqField] {
        &self.fields
    }

    pub fn field_by_ordinal(&self, ordinal: usize) -> Option<&RuntimeSeq> {
        self.fields.get(ordinal).map(|field| &field.values)
    }

    pub fn field_by_name(&self, name: &str) -> Option<&RuntimeSeq> {
        self.fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| &field.values)
    }

    fn into_values(self) -> Vec<RuntimeValue> {
        let row_count = self.len;
        let fields = self.fields;
        (0..row_count)
            .map(|row| {
                RuntimeValue::Record(
                    fields
                        .iter()
                        .map(|field| RuntimeFieldValue {
                            name: field.name.clone(),
                            value: field.values.value_at(row),
                        })
                        .collect(),
                )
            })
            .collect()
    }

    #[must_use]
    fn tail_from(&self, index: usize) -> Self {
        Self {
            len: self.len.saturating_sub(index),
            fields: self
                .fields
                .iter()
                .map(|field| RecordSeqField {
                    name: field.name.clone(),
                    values: field.values.tail_from(index),
                })
                .collect(),
        }
    }

    fn value_at(&self, index: usize) -> RuntimeValue {
        assert!(
            index < self.len,
            "record column sequence index out of bounds"
        );
        RuntimeValue::Record(
            self.fields
                .iter()
                .map(|field| RuntimeFieldValue {
                    name: field.name.clone(),
                    value: field.values.value_at(index),
                })
                .collect(),
        )
    }
}

macro_rules! impl_runtime_exact_signed_integer {
    ($ty:ty, $input_type:ident, $output_type:ident, $slice:ident, $dense:ident, $variant:ident) => {
        impl RuntimeExactInteger for $ty {
            const INPUT_TYPE: RuntimePureInputType = RuntimePureInputType::$input_type;
            const OUTPUT_TYPE: RuntimePureOutputType = RuntimePureOutputType::$output_type;

            fn into_runtime_value(self) -> RuntimeValue {
                RuntimeValue::Int(RuntimeInt::$variant(self))
            }

            fn try_from_runtime_value(
                helper: &str,
                value: RuntimeValue,
            ) -> Result<Self, RuntimeEvalError> {
                match value {
                    RuntimeValue::Int(RuntimeInt::$variant(value)) => Ok(value),
                    RuntimeValue::Int(value) => Err(RuntimeEvalError::UnsupportedPure {
                        name: helper.to_owned(),
                        reason: format!(
                            "pure {} result expected {}, got {}",
                            stringify!($ty),
                            stringify!($ty),
                            value.label()
                        ),
                    }),
                    value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
                }
            }

            fn try_sum_as_i64(self, _helper: &str) -> Result<i64, RuntimeEvalError> {
                Ok(i64::from(self))
            }

            fn exact_slice(values: &[Self]) -> RuntimeExactIntegerSlice<'_> {
                RuntimeExactIntegerSlice::$variant(values)
            }

            fn exact_slice_mut(values: &mut [Self]) -> RuntimeExactIntegerSliceMut<'_> {
                RuntimeExactIntegerSliceMut::$variant(values)
            }

            fn seq_slice(seq: &RuntimeSeq) -> Option<&[Self]> {
                seq.$slice()
            }

            fn dense_sequence(values: Vec<Self>) -> RuntimeValue {
                $dense(values)
            }
        }
    };
}

macro_rules! impl_runtime_exact_unsigned_integer {
    ($ty:ty, $input_type:ident, $output_type:ident, $slice:ident, $dense:ident, $variant:ident) => {
        impl RuntimeExactInteger for $ty {
            const INPUT_TYPE: RuntimePureInputType = RuntimePureInputType::$input_type;
            const OUTPUT_TYPE: RuntimePureOutputType = RuntimePureOutputType::$output_type;

            fn into_runtime_value(self) -> RuntimeValue {
                RuntimeValue::UInt(RuntimeUInt::$variant(self))
            }

            fn try_from_runtime_value(
                helper: &str,
                value: RuntimeValue,
            ) -> Result<Self, RuntimeEvalError> {
                match value {
                    RuntimeValue::UInt(RuntimeUInt::$variant(value)) => Ok(value),
                    RuntimeValue::UInt(value) => Err(RuntimeEvalError::UnsupportedPure {
                        name: helper.to_owned(),
                        reason: format!(
                            "pure {} result expected {}, got {}",
                            stringify!($ty),
                            stringify!($ty),
                            value.label()
                        ),
                    }),
                    value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
                }
            }

            fn try_sum_as_i64(self, helper: &str) -> Result<i64, RuntimeEvalError> {
                i64::try_from(self).map_err(|_| RuntimeEvalError::UnsupportedPure {
                    name: helper.to_owned(),
                    reason: format!(
                        "pure {} result `{self}` cannot be represented as an i64 sum",
                        stringify!($ty)
                    ),
                })
            }

            fn exact_slice(values: &[Self]) -> RuntimeExactIntegerSlice<'_> {
                RuntimeExactIntegerSlice::$variant(values)
            }

            fn exact_slice_mut(values: &mut [Self]) -> RuntimeExactIntegerSliceMut<'_> {
                RuntimeExactIntegerSliceMut::$variant(values)
            }

            fn seq_slice(seq: &RuntimeSeq) -> Option<&[Self]> {
                seq.$slice()
            }

            fn dense_sequence(values: Vec<Self>) -> RuntimeValue {
                $dense(values)
            }
        }
    };
}

macro_rules! impl_runtime_exact_wide_signed_integer {
    ($ty:ty, $input_type:ident, $output_type:ident, $slice:ident, $dense:ident, $variant:ident) => {
        impl RuntimeExactInteger for $ty {
            const INPUT_TYPE: RuntimePureInputType = RuntimePureInputType::$input_type;
            const OUTPUT_TYPE: RuntimePureOutputType = RuntimePureOutputType::$output_type;

            fn into_runtime_value(self) -> RuntimeValue {
                RuntimeValue::Int(RuntimeInt::$variant(self))
            }

            fn try_from_runtime_value(
                helper: &str,
                value: RuntimeValue,
            ) -> Result<Self, RuntimeEvalError> {
                match value {
                    RuntimeValue::Int(RuntimeInt::$variant(value)) => Ok(value),
                    value => Err(RuntimeEvalError::UnsupportedPure {
                        name: helper.to_owned(),
                        reason: format!(
                            "pure {} result expected {}, got {}",
                            stringify!($ty),
                            stringify!($ty),
                            runtime_value_label(&value)
                        ),
                    }),
                }
            }

            fn try_sum_as_i64(self, helper: &str) -> Result<i64, RuntimeEvalError> {
                i64::try_from(self).map_err(|_| RuntimeEvalError::UnsupportedPure {
                    name: helper.to_owned(),
                    reason: format!(
                        "pure {} result `{self}` cannot be represented as an i64 sum",
                        stringify!($ty)
                    ),
                })
            }

            fn exact_slice(values: &[Self]) -> RuntimeExactIntegerSlice<'_> {
                RuntimeExactIntegerSlice::$variant(values)
            }

            fn exact_slice_mut(values: &mut [Self]) -> RuntimeExactIntegerSliceMut<'_> {
                RuntimeExactIntegerSliceMut::$variant(values)
            }

            fn seq_slice(seq: &RuntimeSeq) -> Option<&[Self]> {
                seq.$slice()
            }

            fn dense_sequence(values: Vec<Self>) -> RuntimeValue {
                $dense(values)
            }
        }
    };
}

macro_rules! impl_runtime_exact_wide_unsigned_integer {
    ($ty:ty, $input_type:ident, $output_type:ident, $slice:ident, $dense:ident, $variant:ident) => {
        impl RuntimeExactInteger for $ty {
            const INPUT_TYPE: RuntimePureInputType = RuntimePureInputType::$input_type;
            const OUTPUT_TYPE: RuntimePureOutputType = RuntimePureOutputType::$output_type;

            fn into_runtime_value(self) -> RuntimeValue {
                RuntimeValue::UInt(RuntimeUInt::$variant(self))
            }

            fn try_from_runtime_value(
                helper: &str,
                value: RuntimeValue,
            ) -> Result<Self, RuntimeEvalError> {
                match value {
                    RuntimeValue::UInt(RuntimeUInt::$variant(value)) => Ok(value),
                    value => Err(RuntimeEvalError::UnsupportedPure {
                        name: helper.to_owned(),
                        reason: format!(
                            "pure {} result expected {}, got {}",
                            stringify!($ty),
                            stringify!($ty),
                            runtime_value_label(&value)
                        ),
                    }),
                }
            }

            fn try_sum_as_i64(self, helper: &str) -> Result<i64, RuntimeEvalError> {
                i64::try_from(self).map_err(|_| RuntimeEvalError::UnsupportedPure {
                    name: helper.to_owned(),
                    reason: format!(
                        "pure {} result `{self}` cannot be represented as an i64 sum",
                        stringify!($ty)
                    ),
                })
            }

            fn exact_slice(values: &[Self]) -> RuntimeExactIntegerSlice<'_> {
                RuntimeExactIntegerSlice::$variant(values)
            }

            fn exact_slice_mut(values: &mut [Self]) -> RuntimeExactIntegerSliceMut<'_> {
                RuntimeExactIntegerSliceMut::$variant(values)
            }

            fn seq_slice(seq: &RuntimeSeq) -> Option<&[Self]> {
                seq.$slice()
            }

            fn dense_sequence(values: Vec<Self>) -> RuntimeValue {
                $dense(values)
            }
        }
    };
}

impl RuntimeSeq {
    pub fn values(values: Vec<RuntimeValue>) -> Self {
        Self::Values(values)
    }

    pub const fn dense_units(len: usize) -> Self {
        Self::Dense(DenseSeq::units(len))
    }

    pub fn dense_i64(values: Vec<i64>) -> Self {
        Self::Dense(DenseSeq::i64(values))
    }

    pub fn dense_i128(values: Vec<i128>) -> Self {
        Self::Dense(DenseSeq::i128(values))
    }

    pub fn dense_isize(values: Vec<i64>) -> Self {
        Self::Dense(DenseSeq::isize(values))
    }

    pub fn dense_i8(values: Vec<i8>) -> Self {
        Self::Dense(DenseSeq::i8(values))
    }

    pub fn dense_i16(values: Vec<i16>) -> Self {
        Self::Dense(DenseSeq::i16(values))
    }

    pub fn dense_i32(values: Vec<i32>) -> Self {
        Self::Dense(DenseSeq::i32(values))
    }

    pub fn dense_u8(values: Vec<u8>) -> Self {
        Self::Dense(DenseSeq::u8(values))
    }

    pub fn dense_u16(values: Vec<u16>) -> Self {
        Self::Dense(DenseSeq::u16(values))
    }

    pub fn dense_u32(values: Vec<u32>) -> Self {
        Self::Dense(DenseSeq::u32(values))
    }

    pub fn dense_u64(values: Vec<u64>) -> Self {
        Self::Dense(DenseSeq::u64(values))
    }

    pub fn dense_u128(values: Vec<u128>) -> Self {
        Self::Dense(DenseSeq::u128(values))
    }

    pub fn dense_usize(values: Vec<u64>) -> Self {
        Self::Dense(DenseSeq::usize(values))
    }

    pub fn dense_f32(values: Vec<f32>) -> Self {
        Self::Dense(DenseSeq::f32(values))
    }

    pub fn dense_f64(values: Vec<f64>) -> Self {
        Self::Dense(DenseSeq::f64(values))
    }

    pub fn dense_bool(values: Vec<bool>) -> Self {
        Self::Dense(DenseSeq::bool(values))
    }

    pub fn dense_bytes(values: Vec<u8>) -> Self {
        Self::Dense(DenseSeq::bytes(values))
    }

    pub fn dense_chars(values: Vec<char>) -> Self {
        Self::Dense(DenseSeq::chars(values))
    }

    pub fn dense_durations(values: Vec<LogicalDuration>) -> Self {
        Self::Dense(DenseSeq::durations(values))
    }

    pub fn dense_strings(values: Vec<String>) -> Self {
        Self::Dense(DenseSeq::strings(values))
    }

    pub fn dense_entity_refs(values: Vec<String>) -> Self {
        Self::Dense(DenseSeq::entity_refs(values))
    }

    pub fn tuple_columns(len: usize, columns: Vec<RuntimeSeq>) -> Result<Self, RuntimeSeqError> {
        TupleSeq::new(len, columns).map(Self::TupleColumns)
    }

    pub fn record_columns(
        len: usize,
        fields: Vec<RecordSeqField>,
    ) -> Result<Self, RuntimeSeqError> {
        RecordSeq::new(len, fields).map(Self::RecordColumns)
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Values(values) => values.len(),
            Self::Dense(values) => values.len(),
            Self::TupleColumns(values) => values.len(),
            Self::RecordColumns(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_values(&self) -> Option<&[RuntimeValue]> {
        match self {
            Self::Values(values) => Some(values),
            Self::Dense(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn unit_len(&self) -> Option<usize> {
        match self {
            Self::Dense(values) => values.unit_len(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn dense_kind(&self) -> Option<DenseSeqKind> {
        match self {
            Self::Dense(values) => Some(values.kind()),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_i64_slice(&self) -> Option<&[i64]> {
        match self {
            Self::Dense(values) => values.as_i64_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn copy_i64_values_to(&self, out: &mut Vec<i64>) -> bool {
        match self {
            Self::Dense(values) => values.copy_i64_values_to(out),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => false,
        }
    }

    pub fn try_for_each_i64<E>(&self, visit: impl FnMut(i64) -> Result<(), E>) -> Result<bool, E> {
        match self {
            Self::Dense(values) => values.try_for_each_i64(visit),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => Ok(false),
        }
    }

    pub fn first_i64(&self) -> Option<Option<i64>> {
        match self {
            Self::Dense(values) => values.first_i64(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_i128_slice(&self) -> Option<&[i128]> {
        match self {
            Self::Dense(values) => values.as_i128_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_isize_values(&self) -> Option<Vec<i64>> {
        match self {
            Self::Dense(values) => values.as_isize_values(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_isize_storage(&self) -> Option<&[RuntimeISizeValue]> {
        match self {
            Self::Dense(values) => values.as_isize_storage(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_i8_slice(&self) -> Option<&[i8]> {
        match self {
            Self::Dense(values) => values.as_i8_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_i16_slice(&self) -> Option<&[i16]> {
        match self {
            Self::Dense(values) => values.as_i16_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_i32_slice(&self) -> Option<&[i32]> {
        match self {
            Self::Dense(values) => values.as_i32_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_u8_slice(&self) -> Option<&[u8]> {
        match self {
            Self::Dense(values) => values.as_u8_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_u16_slice(&self) -> Option<&[u16]> {
        match self {
            Self::Dense(values) => values.as_u16_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_u32_slice(&self) -> Option<&[u32]> {
        match self {
            Self::Dense(values) => values.as_u32_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_u64_slice(&self) -> Option<&[u64]> {
        match self {
            Self::Dense(values) => values.as_u64_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_u128_slice(&self) -> Option<&[u128]> {
        match self {
            Self::Dense(values) => values.as_u128_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_usize_values(&self) -> Option<Vec<u64>> {
        match self {
            Self::Dense(values) => values.as_usize_values(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_usize_storage(&self) -> Option<&[RuntimeUSizeValue]> {
        match self {
            Self::Dense(values) => values.as_usize_storage(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_f32_slice(&self) -> Option<&[f32]> {
        match self {
            Self::Dense(values) => values.as_f32_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_f64_slice(&self) -> Option<&[f64]> {
        match self {
            Self::Dense(values) => values.as_f64_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_bool_slice(&self) -> Option<&[bool]> {
        match self {
            Self::Dense(values) => values.as_bool_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Dense(values) => values.as_bytes(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_chars(&self) -> Option<&[char]> {
        match self {
            Self::Dense(values) => values.as_chars(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_durations(&self) -> Option<&[LogicalDuration]> {
        match self {
            Self::Dense(values) => values.as_durations(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_strings(&self) -> Option<&[String]> {
        match self {
            Self::Dense(values) => values.as_strings(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_entity_refs(&self) -> Option<&[String]> {
        match self {
            Self::Dense(values) => values.as_entity_refs(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn into_values(self) -> Vec<RuntimeValue> {
        match self {
            Self::Values(values) => values,
            Self::Dense(values) => values.into_values(),
            Self::TupleColumns(values) => values.into_values(),
            Self::RecordColumns(values) => values.into_values(),
        }
    }

    /// Returns the runtime value at `index`.
    ///
    /// # Panics
    ///
    /// Panics when `index` is outside this sequence.
    pub fn value_at(&self, index: usize) -> RuntimeValue {
        match self {
            Self::Values(values) => values[index].clone(),
            Self::Dense(values) => values.value_at(index),
            Self::TupleColumns(values) => values.value_at(index),
            Self::RecordColumns(values) => values.value_at(index),
        }
    }

    #[must_use]
    pub fn tail_from(&self, index: usize) -> Self {
        match self {
            Self::Values(values) => Self::Values(values[index..].to_vec()),
            Self::Dense(values) => Self::Dense(values.tail_from(index)),
            Self::TupleColumns(values) => Self::TupleColumns(values.tail_from(index)),
            Self::RecordColumns(values) => Self::RecordColumns(values.tail_from(index)),
        }
    }

    pub fn into_i64_vec(self) -> Option<Vec<i64>> {
        match self {
            Self::Dense(values) => values.into_i64_vec(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn sum_as_i64(&self) -> Option<i64> {
        match self {
            Self::Dense(values) => values.sum_as_i64(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }
}

impl_runtime_exact_signed_integer!(i8, I8, I8, as_i8_slice, runtime_sequence_dense_i8, I8);
impl_runtime_exact_signed_integer!(i16, I16, I16, as_i16_slice, runtime_sequence_dense_i16, I16);
impl_runtime_exact_signed_integer!(i32, I32, I32, as_i32_slice, runtime_sequence_dense_i32, I32);
impl_runtime_exact_wide_signed_integer!(
    i128,
    I128,
    I128,
    as_i128_slice,
    runtime_sequence_dense_i128,
    I128
);
impl_runtime_exact_unsigned_integer!(u8, U8, U8, as_u8_slice, runtime_sequence_dense_u8, U8);
impl_runtime_exact_unsigned_integer!(u16, U16, U16, as_u16_slice, runtime_sequence_dense_u16, U16);
impl_runtime_exact_unsigned_integer!(u32, U32, U32, as_u32_slice, runtime_sequence_dense_u32, U32);
impl_runtime_exact_unsigned_integer!(u64, U64, U64, as_u64_slice, runtime_sequence_dense_u64, U64);
impl_runtime_exact_wide_unsigned_integer!(
    u128,
    U128,
    U128,
    as_u128_slice,
    runtime_sequence_dense_u128,
    U128
);

impl RuntimeExactInteger for RuntimeISizeValue {
    const INPUT_TYPE: RuntimePureInputType = RuntimePureInputType::ISize;
    const OUTPUT_TYPE: RuntimePureOutputType = RuntimePureOutputType::ISize;

    fn into_runtime_value(self) -> RuntimeValue {
        RuntimeValue::isize(self.0)
    }

    fn try_from_runtime_value(helper: &str, value: RuntimeValue) -> Result<Self, RuntimeEvalError> {
        match value {
            RuntimeValue::Int(RuntimeInt::ISize(value)) => Ok(Self(value)),
            value => Err(RuntimeEvalError::UnsupportedPure {
                name: helper.to_owned(),
                reason: format!(
                    "pure isize result expected isize, got {}",
                    runtime_value_label(&value)
                ),
            }),
        }
    }

    fn try_sum_as_i64(self, _helper: &str) -> Result<i64, RuntimeEvalError> {
        Ok(self.0)
    }

    fn exact_slice(values: &[Self]) -> RuntimeExactIntegerSlice<'_> {
        RuntimeExactIntegerSlice::ISize(values)
    }

    fn exact_slice_mut(values: &mut [Self]) -> RuntimeExactIntegerSliceMut<'_> {
        RuntimeExactIntegerSliceMut::ISize(values)
    }

    fn seq_slice(seq: &RuntimeSeq) -> Option<&[Self]> {
        match seq {
            RuntimeSeq::Dense(DenseSeq::ISize(values)) => Some(values.as_slice()),
            RuntimeSeq::Values(_) | RuntimeSeq::TupleColumns(_) | RuntimeSeq::RecordColumns(_) => {
                None
            }
            RuntimeSeq::Dense(_) => None,
        }
    }

    fn dense_sequence(values: Vec<Self>) -> RuntimeValue {
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::ISize(DenseSeqStorage::new(
            values,
        ))))
    }
}

impl RuntimeExactInteger for RuntimeUSizeValue {
    const INPUT_TYPE: RuntimePureInputType = RuntimePureInputType::USize;
    const OUTPUT_TYPE: RuntimePureOutputType = RuntimePureOutputType::USize;

    fn into_runtime_value(self) -> RuntimeValue {
        RuntimeValue::usize(self.0)
    }

    fn try_from_runtime_value(helper: &str, value: RuntimeValue) -> Result<Self, RuntimeEvalError> {
        match value {
            RuntimeValue::UInt(RuntimeUInt::USize(value)) => Ok(Self(value)),
            value => Err(RuntimeEvalError::UnsupportedPure {
                name: helper.to_owned(),
                reason: format!(
                    "pure usize result expected usize, got {}",
                    runtime_value_label(&value)
                ),
            }),
        }
    }

    fn try_sum_as_i64(self, helper: &str) -> Result<i64, RuntimeEvalError> {
        i64::try_from(self.0).map_err(|_| RuntimeEvalError::UnsupportedPure {
            name: helper.to_owned(),
            reason: format!("pure usize result `{self}` cannot be represented as an i64 sum"),
        })
    }

    fn exact_slice(values: &[Self]) -> RuntimeExactIntegerSlice<'_> {
        RuntimeExactIntegerSlice::USize(values)
    }

    fn exact_slice_mut(values: &mut [Self]) -> RuntimeExactIntegerSliceMut<'_> {
        RuntimeExactIntegerSliceMut::USize(values)
    }

    fn seq_slice(seq: &RuntimeSeq) -> Option<&[Self]> {
        match seq {
            RuntimeSeq::Dense(DenseSeq::USize(values)) => Some(values.as_slice()),
            RuntimeSeq::Values(_) | RuntimeSeq::TupleColumns(_) | RuntimeSeq::RecordColumns(_) => {
                None
            }
            RuntimeSeq::Dense(_) => None,
        }
    }

    fn dense_sequence(values: Vec<Self>) -> RuntimeValue {
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::USize(DenseSeqStorage::new(
            values,
        ))))
    }
}

impl DenseSeq {
    pub const fn units(len: usize) -> Self {
        Self::Units(len)
    }

    pub fn i8(values: Vec<i8>) -> Self {
        Self::I8(DenseSeqStorage::new(values))
    }

    pub fn i16(values: Vec<i16>) -> Self {
        Self::I16(DenseSeqStorage::new(values))
    }

    pub fn i32(values: Vec<i32>) -> Self {
        Self::I32(DenseSeqStorage::new(values))
    }

    pub fn i64(values: Vec<i64>) -> Self {
        Self::I64(DenseSeqStorage::new(values))
    }

    pub fn i128(values: Vec<i128>) -> Self {
        Self::I128(DenseSeqStorage::new(values))
    }

    pub fn isize(values: Vec<i64>) -> Self {
        Self::ISize(DenseSeqStorage::new(
            values.into_iter().map(RuntimeISizeValue::new).collect(),
        ))
    }

    pub fn u8(values: Vec<u8>) -> Self {
        Self::U8(DenseSeqStorage::new(values))
    }

    pub fn u16(values: Vec<u16>) -> Self {
        Self::U16(DenseSeqStorage::new(values))
    }

    pub fn u32(values: Vec<u32>) -> Self {
        Self::U32(DenseSeqStorage::new(values))
    }

    pub fn u64(values: Vec<u64>) -> Self {
        Self::U64(DenseSeqStorage::new(values))
    }

    pub fn u128(values: Vec<u128>) -> Self {
        Self::U128(DenseSeqStorage::new(values))
    }

    pub fn usize(values: Vec<u64>) -> Self {
        Self::USize(DenseSeqStorage::new(
            values.into_iter().map(RuntimeUSizeValue::new).collect(),
        ))
    }

    pub fn f32(values: Vec<f32>) -> Self {
        Self::F32(DenseSeqStorage::new(values))
    }

    pub fn f64(values: Vec<f64>) -> Self {
        Self::F64(DenseSeqStorage::new(values))
    }

    pub fn bool(values: Vec<bool>) -> Self {
        Self::Bool(DenseSeqStorage::new(values))
    }

    pub fn bytes(values: Vec<u8>) -> Self {
        Self::Bytes(DenseSeqStorage::new(values))
    }

    pub fn chars(values: Vec<char>) -> Self {
        Self::Chars(DenseSeqStorage::new(values))
    }

    pub fn durations(values: Vec<LogicalDuration>) -> Self {
        Self::Durations(DenseSeqStorage::new(values))
    }

    pub fn strings(values: Vec<String>) -> Self {
        Self::Strings(DenseSeqStorage::new(values))
    }

    pub fn entity_refs(values: Vec<String>) -> Self {
        Self::EntityRefs(DenseSeqStorage::new(values))
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Units(len) => *len,
            Self::I8(values) => values.len(),
            Self::I16(values) => values.len(),
            Self::I32(values) => values.len(),
            Self::I64(values) => values.len(),
            Self::ISize(values) => values.len(),
            Self::I128(values) => values.len(),
            Self::U8(values) | Self::Bytes(values) => values.len(),
            Self::U16(values) => values.len(),
            Self::U32(values) => values.len(),
            Self::U64(values) => values.len(),
            Self::USize(values) => values.len(),
            Self::U128(values) => values.len(),
            Self::F32(values) => values.len(),
            Self::F64(values) => values.len(),
            Self::Bool(values) => values.len(),
            Self::Chars(values) => values.len(),
            Self::Durations(values) => values.len(),
            Self::Strings(values) | Self::EntityRefs(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub const fn kind(&self) -> DenseSeqKind {
        match self {
            Self::Units(_) => DenseSeqKind::Units,
            Self::I8(_) => DenseSeqKind::I8,
            Self::I16(_) => DenseSeqKind::I16,
            Self::I32(_) => DenseSeqKind::I32,
            Self::I64(_) => DenseSeqKind::I64,
            Self::I128(_) => DenseSeqKind::I128,
            Self::ISize(_) => DenseSeqKind::ISize,
            Self::U8(_) => DenseSeqKind::U8,
            Self::U16(_) => DenseSeqKind::U16,
            Self::U32(_) => DenseSeqKind::U32,
            Self::U64(_) => DenseSeqKind::U64,
            Self::U128(_) => DenseSeqKind::U128,
            Self::USize(_) => DenseSeqKind::USize,
            Self::F32(_) => DenseSeqKind::F32,
            Self::F64(_) => DenseSeqKind::F64,
            Self::Bool(_) => DenseSeqKind::Bool,
            Self::Bytes(_) => DenseSeqKind::Bytes,
            Self::Chars(_) => DenseSeqKind::Chars,
            Self::Durations(_) => DenseSeqKind::Durations,
            Self::Strings(_) => DenseSeqKind::Strings,
            Self::EntityRefs(_) => DenseSeqKind::EntityRefs,
        }
    }

    pub const fn unit_len(&self) -> Option<usize> {
        match self {
            Self::Units(len) => Some(*len),
            Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I64(_)
            | Self::I128(_)
            | Self::ISize(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::U128(_)
            | Self::USize(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::Bytes(_)
            | Self::Chars(_)
            | Self::Durations(_)
            | Self::Strings(_)
            | Self::EntityRefs(_) => None,
        }
    }

    pub fn as_i64_slice(&self) -> Option<&[i64]> {
        match self {
            Self::I64(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn copy_i64_values_to(&self, out: &mut Vec<i64>) -> bool {
        match self {
            Self::I64(values) => {
                out.extend(values.as_slice().iter().copied());
                true
            }
            Self::Units(_)
            | Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I128(_)
            | Self::ISize(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::U128(_)
            | Self::USize(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bytes(_)
            | Self::Bool(_)
            | Self::Chars(_)
            | Self::Durations(_)
            | Self::Strings(_)
            | Self::EntityRefs(_) => false,
        }
    }

    pub fn try_for_each_i64<E>(
        &self,
        mut visit: impl FnMut(i64) -> Result<(), E>,
    ) -> Result<bool, E> {
        match self {
            Self::I64(values) => {
                for value in values.as_slice().iter().copied() {
                    visit(value)?;
                }
                Ok(true)
            }
            Self::Units(_)
            | Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I128(_)
            | Self::ISize(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::U128(_)
            | Self::USize(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::Bytes(_)
            | Self::Chars(_)
            | Self::Durations(_)
            | Self::Strings(_)
            | Self::EntityRefs(_) => Ok(false),
        }
    }

    pub fn first_i64(&self) -> Option<Option<i64>> {
        let first = match self {
            Self::I64(values) => values.as_slice().first().copied(),
            Self::Units(_)
            | Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I128(_)
            | Self::ISize(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::U128(_)
            | Self::USize(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bytes(_)
            | Self::Bool(_)
            | Self::Chars(_)
            | Self::Durations(_)
            | Self::Strings(_)
            | Self::EntityRefs(_) => return None,
        };
        Some(first)
    }

    pub fn as_i128_slice(&self) -> Option<&[i128]> {
        match self {
            Self::I128(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_isize_values(&self) -> Option<Vec<i64>> {
        self.as_isize_storage()
            .map(|values| values.iter().copied().map(RuntimeISizeValue::get).collect())
    }

    pub fn as_isize_storage(&self) -> Option<&[RuntimeISizeValue]> {
        match self {
            Self::ISize(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_i8_slice(&self) -> Option<&[i8]> {
        match self {
            Self::I8(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_i16_slice(&self) -> Option<&[i16]> {
        match self {
            Self::I16(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_i32_slice(&self) -> Option<&[i32]> {
        match self {
            Self::I32(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_u8_slice(&self) -> Option<&[u8]> {
        match self {
            Self::U8(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_u16_slice(&self) -> Option<&[u16]> {
        match self {
            Self::U16(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_u32_slice(&self) -> Option<&[u32]> {
        match self {
            Self::U32(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_u64_slice(&self) -> Option<&[u64]> {
        match self {
            Self::U64(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_u128_slice(&self) -> Option<&[u128]> {
        match self {
            Self::U128(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_usize_values(&self) -> Option<Vec<u64>> {
        self.as_usize_storage()
            .map(|values| values.iter().copied().map(RuntimeUSizeValue::get).collect())
    }

    pub fn as_usize_storage(&self) -> Option<&[RuntimeUSizeValue]> {
        match self {
            Self::USize(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_f32_slice(&self) -> Option<&[f32]> {
        match self {
            Self::F32(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_f64_slice(&self) -> Option<&[f64]> {
        match self {
            Self::F64(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_bool_slice(&self) -> Option<&[bool]> {
        match self {
            Self::Bool(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(values) | Self::U8(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_chars(&self) -> Option<&[char]> {
        match self {
            Self::Chars(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_durations(&self) -> Option<&[LogicalDuration]> {
        match self {
            Self::Durations(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_strings(&self) -> Option<&[String]> {
        match self {
            Self::Strings(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_entity_refs(&self) -> Option<&[String]> {
        match self {
            Self::EntityRefs(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn into_values(self) -> Vec<RuntimeValue> {
        match self {
            Self::Units(len) => vec![RuntimeValue::Unit; len],
            Self::I8(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::i8)
                .collect(),
            Self::I16(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::i16)
                .collect(),
            Self::I32(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::i32)
                .collect(),
            Self::I64(values) => materialize_i64_sequence(values.into_vec()),
            Self::I128(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::i128)
                .collect(),
            Self::ISize(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeISizeValue::get)
                .map(RuntimeValue::isize)
                .collect(),
            Self::U8(values) | Self::Bytes(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::u8)
                .collect(),
            Self::U16(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::u16)
                .collect(),
            Self::U32(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::u32)
                .collect(),
            Self::U64(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::u64)
                .collect(),
            Self::U128(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::u128)
                .collect(),
            Self::USize(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeUSizeValue::get)
                .map(RuntimeValue::usize)
                .collect(),
            Self::F32(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::F32)
                .collect(),
            Self::F64(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::F64)
                .collect(),
            Self::Bool(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::Bool)
                .collect(),
            Self::Chars(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::Char)
                .collect(),
            Self::Durations(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::Duration)
                .collect(),
            Self::Strings(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::String)
                .collect(),
            Self::EntityRefs(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::EntityRef)
                .collect(),
        }
    }

    /// Returns the runtime value at `index`.
    ///
    /// # Panics
    ///
    /// Panics when `index` is outside this sequence.
    pub fn value_at(&self, index: usize) -> RuntimeValue {
        match self {
            Self::Units(len) => {
                assert!(index < *len, "unit dense sequence index out of bounds");
                RuntimeValue::Unit
            }
            Self::I8(values) => RuntimeValue::i8(values.as_slice()[index]),
            Self::I16(values) => RuntimeValue::i16(values.as_slice()[index]),
            Self::I32(values) => RuntimeValue::i32(values.as_slice()[index]),
            Self::I64(values) => RuntimeValue::i64(values.as_slice()[index]),
            Self::I128(values) => RuntimeValue::i128(values.as_slice()[index]),
            Self::ISize(values) => RuntimeValue::isize(values.as_slice()[index].get()),
            Self::U8(values) | Self::Bytes(values) => RuntimeValue::u8(values.as_slice()[index]),
            Self::U16(values) => RuntimeValue::u16(values.as_slice()[index]),
            Self::U32(values) => RuntimeValue::u32(values.as_slice()[index]),
            Self::U64(values) => RuntimeValue::u64(values.as_slice()[index]),
            Self::U128(values) => RuntimeValue::u128(values.as_slice()[index]),
            Self::USize(values) => RuntimeValue::usize(values.as_slice()[index].get()),
            Self::F32(values) => RuntimeValue::F32(values.as_slice()[index]),
            Self::F64(values) => RuntimeValue::F64(values.as_slice()[index]),
            Self::Bool(values) => RuntimeValue::Bool(values.as_slice()[index]),
            Self::Chars(values) => RuntimeValue::Char(values.as_slice()[index]),
            Self::Durations(values) => RuntimeValue::Duration(values.as_slice()[index]),
            Self::Strings(values) => RuntimeValue::String(values.as_slice()[index].clone()),
            Self::EntityRefs(values) => RuntimeValue::EntityRef(values.as_slice()[index].clone()),
        }
    }

    #[must_use]
    pub fn tail_from(&self, index: usize) -> Self {
        match self {
            Self::Units(len) => Self::Units(len.saturating_sub(index)),
            Self::I8(values) => Self::I8(values.tail_from(index)),
            Self::I16(values) => Self::I16(values.tail_from(index)),
            Self::I32(values) => Self::I32(values.tail_from(index)),
            Self::I64(values) => Self::I64(values.tail_from(index)),
            Self::I128(values) => Self::I128(values.tail_from(index)),
            Self::ISize(values) => Self::ISize(values.tail_from(index)),
            Self::U8(values) => Self::U8(values.tail_from(index)),
            Self::U16(values) => Self::U16(values.tail_from(index)),
            Self::U32(values) => Self::U32(values.tail_from(index)),
            Self::U64(values) => Self::U64(values.tail_from(index)),
            Self::U128(values) => Self::U128(values.tail_from(index)),
            Self::USize(values) => Self::USize(values.tail_from(index)),
            Self::F32(values) => Self::F32(values.tail_from(index)),
            Self::F64(values) => Self::F64(values.tail_from(index)),
            Self::Bool(values) => Self::Bool(values.tail_from(index)),
            Self::Bytes(values) => Self::Bytes(values.tail_from(index)),
            Self::Chars(values) => Self::Chars(values.tail_from(index)),
            Self::Durations(values) => Self::Durations(values.tail_from(index)),
            Self::Strings(values) => Self::Strings(values.tail_from(index)),
            Self::EntityRefs(values) => Self::EntityRefs(values.tail_from(index)),
        }
    }

    pub fn into_i64_vec(self) -> Option<Vec<i64>> {
        match self {
            Self::I64(values) => Some(values.into_vec()),
            Self::Units(_)
            | Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I128(_)
            | Self::ISize(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::U128(_)
            | Self::USize(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::Bytes(_)
            | Self::Chars(_)
            | Self::Durations(_)
            | Self::Strings(_)
            | Self::EntityRefs(_) => None,
        }
    }

    pub fn sum_as_i64(&self) -> Option<i64> {
        match self {
            Self::I8(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::I16(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::I32(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::I64(values) => Some(values.as_slice().iter().sum()),
            Self::ISize(values) => Some(
                values
                    .as_slice()
                    .iter()
                    .copied()
                    .map(RuntimeISizeValue::get)
                    .sum(),
            ),
            Self::I128(values) => values.as_slice().iter().try_fold(0_i64, |acc, value| {
                i64::try_from(*value).ok().map(|value| acc + value)
            }),
            Self::U8(values) | Self::Bytes(values) => {
                Some(values.as_slice().iter().copied().map(i64::from).sum())
            }
            Self::U16(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::U32(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::U64(values) => values.as_slice().iter().try_fold(0_i64, |acc, value| {
                i64::try_from(*value).ok().map(|value| acc + value)
            }),
            Self::USize(values) => values.as_slice().iter().try_fold(0_i64, |acc, value| {
                i64::try_from(value.get()).ok().map(|value| acc + value)
            }),
            Self::U128(values) => values.as_slice().iter().try_fold(0_i64, |acc, value| {
                i64::try_from(*value).ok().map(|value| acc + value)
            }),
            Self::Units(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::Chars(_)
            | Self::Durations(_)
            | Self::Strings(_)
            | Self::EntityRefs(_) => None,
        }
    }
}

impl<T> DenseSeqStorage<T> {
    pub fn new(values: Vec<T>) -> Self {
        Self { values }
    }

    pub fn as_slice(&self) -> &[T] {
        &self.values
    }

    pub fn into_vec(self) -> Vec<T> {
        self.values
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl<T: Clone> DenseSeqStorage<T> {
    #[must_use]
    pub fn tail_from(&self, index: usize) -> Self {
        Self::new(self.values[index..].to_vec())
    }
}
