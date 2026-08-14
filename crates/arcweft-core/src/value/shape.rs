use super::RuntimeValue;

/// Physical outer shape of one runtime value.
///
/// This reports the stored runtime family without inferring a semantic checked
/// type. In particular, byte sequences have the physical [`Self::Sequence`]
/// shape because there is no distinct `RuntimeValue::Bytes` carrier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeValueShape {
    Unit,
    Bool,
    Signed,
    Unsigned,
    F32,
    F64,
    MatrixF32,
    MatrixF64,
    TensorF32,
    TensorF64,
    String,
    Char,
    Duration,
    Range,
    Iterator,
    EntityReference,
    Tuple,
    Sequence,
    Record,
    NominalRecord,
    Opaque,
    Function,
    Variant,
}

impl RuntimeValue {
    /// Returns this value's exact physical outer shape.
    #[must_use]
    pub const fn shape(&self) -> RuntimeValueShape {
        match self {
            Self::Unit => RuntimeValueShape::Unit,
            Self::Bool(_) => RuntimeValueShape::Bool,
            Self::Int(_) => RuntimeValueShape::Signed,
            Self::UInt(_) => RuntimeValueShape::Unsigned,
            Self::F32(_) => RuntimeValueShape::F32,
            Self::F64(_) => RuntimeValueShape::F64,
            Self::MatrixF32(_) => RuntimeValueShape::MatrixF32,
            Self::MatrixF64(_) => RuntimeValueShape::MatrixF64,
            Self::TensorF32(_) => RuntimeValueShape::TensorF32,
            Self::TensorF64(_) => RuntimeValueShape::TensorF64,
            Self::String(_) => RuntimeValueShape::String,
            Self::Char(_) => RuntimeValueShape::Char,
            Self::Duration(_) => RuntimeValueShape::Duration,
            Self::Range(_) => RuntimeValueShape::Range,
            Self::Iterator(_) => RuntimeValueShape::Iterator,
            Self::EntityRef(_) => RuntimeValueShape::EntityReference,
            Self::Tuple(_) => RuntimeValueShape::Tuple,
            Self::Seq(_) => RuntimeValueShape::Sequence,
            Self::Record(_) => RuntimeValueShape::Record,
            Self::NominalRecord(_) => RuntimeValueShape::NominalRecord,
            Self::Opaque(_) => RuntimeValueShape::Opaque,
            Self::Function(_) => RuntimeValueShape::Function,
            Self::Variant { .. } => RuntimeValueShape::Variant,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{RuntimeUInt, RuntimeUnsignedIntWidth, runtime_sequence_dense_bytes};

    #[test]
    fn scalar_and_aggregate_values_report_physical_shapes() {
        assert_eq!(RuntimeValue::Unit.shape(), RuntimeValueShape::Unit);
        assert_eq!(RuntimeValue::Bool(true).shape(), RuntimeValueShape::Bool);
        assert_eq!(
            RuntimeValue::String(String::new()).shape(),
            RuntimeValueShape::String
        );
        assert_eq!(
            RuntimeValue::Tuple(Vec::new()).shape(),
            RuntimeValueShape::Tuple
        );
        assert_eq!(
            RuntimeValue::Variant {
                owner: crate::pattern::RuntimeVariantIdentity::Option,
                ordinal: 1,
                name: "None".to_owned(),
                payload: None,
            }
            .shape(),
            RuntimeValueShape::Variant
        );

        let byte = RuntimeUInt::from_u128(RuntimeUnsignedIntWidth::U8, 7)
            .expect("u8 value is representable");
        assert_eq!(
            RuntimeValue::UInt(byte).shape(),
            RuntimeValueShape::Unsigned
        );
        assert_eq!(
            runtime_sequence_dense_bytes(vec![7]).shape(),
            RuntimeValueShape::Sequence
        );
    }
}
