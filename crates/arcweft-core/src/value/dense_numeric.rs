//! The owning value boundary between live dense storage and its checked bit image.

use super::RuntimeValue;
use crate::math::{
    DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64, MatrixShape,
    RuntimeDenseNumericElements as Elements, RuntimeDenseNumericPayload,
    RuntimeDenseNumericPayloadError, RuntimeDenseNumericType as Dense, RuntimeFloatWidth as Width,
    RuntimeMathError, TensorShape,
};

impl TryFrom<&RuntimeValue> for RuntimeDenseNumericPayload {
    type Error = RuntimeDenseNumericPayloadError;

    fn try_from(value: &RuntimeValue) -> Result<Self, Self::Error> {
        let dimensions = |dims: &[usize]| {
            dims.iter()
                .map(|dim| u64::try_from(*dim).map_err(|_| Self::Error::DimensionOverflow))
                .collect::<Result<Vec<_>, _>>()
        };
        let (ty, dimensions, elements) = match value {
            RuntimeValue::MatrixF32(value) => (
                Dense::Matrix(Width::F32),
                dimensions(&[value.rows(), value.cols()])?,
                Elements::F32(value.values().iter().map(|value| value.to_bits()).collect()),
            ),
            RuntimeValue::MatrixF64(value) => (
                Dense::Matrix(Width::F64),
                dimensions(&[value.rows(), value.cols()])?,
                Elements::F64(value.values().iter().map(|value| value.to_bits()).collect()),
            ),
            RuntimeValue::TensorF32(value) => (
                Dense::Tensor(Width::F32),
                dimensions(value.shape().dims())?,
                Elements::F32(value.values().iter().map(|value| value.to_bits()).collect()),
            ),
            RuntimeValue::TensorF64(value) => (
                Dense::Tensor(Width::F64),
                dimensions(value.shape().dims())?,
                Elements::F64(value.values().iter().map(|value| value.to_bits()).collect()),
            ),
            _ => return Err(Self::Error::NotDense),
        };
        Self::try_new(ty, dimensions, elements)
    }
}

impl TryFrom<&RuntimeDenseNumericPayload> for RuntimeValue {
    type Error = RuntimeMathError;

    fn try_from(payload: &RuntimeDenseNumericPayload) -> Result<Self, Self::Error> {
        let dimensions = payload
            .dimensions()
            .iter()
            .map(|dim| usize::try_from(*dim).map_err(|_| RuntimeMathError::ElementCountOverflow))
            .collect::<Result<Vec<_>, _>>()?;
        // Admit the platform shape before allocating its floating-point storage.
        match payload.ty() {
            Dense::Matrix(_) => MatrixShape::new(dimensions[0], dimensions[1])?
                .validate_element_count(payload.elements().len())?,
            Dense::Tensor(_) => TensorShape::new(dimensions.clone())?
                .validate_element_count(payload.elements().len())?,
        }
        match (payload.ty(), payload.elements()) {
            (Dense::Matrix(Width::F32), Elements::F32(bits)) => {
                DenseMatrixF32::new(dimensions[0], dimensions[1], storage(bits, f32::from_bits)?)
                    .map(Self::MatrixF32)
            }
            (Dense::Matrix(Width::F64), Elements::F64(bits)) => {
                DenseMatrixF64::new(dimensions[0], dimensions[1], storage(bits, f64::from_bits)?)
                    .map(Self::MatrixF64)
            }
            (Dense::Tensor(Width::F32), Elements::F32(bits)) => {
                DenseTensorF32::new(dimensions, storage(bits, f32::from_bits)?).map(Self::TensorF32)
            }
            (Dense::Tensor(Width::F64), Elements::F64(bits)) => {
                DenseTensorF64::new(dimensions, storage(bits, f64::from_bits)?).map(Self::TensorF64)
            }
            _ => unreachable!("checked dense payload retains its scalar width"),
        }
    }
}

fn storage<Bits: Copy, Scalar>(
    bits: &[Bits],
    decode: fn(Bits) -> Scalar,
) -> Result<Vec<Scalar>, RuntimeMathError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(bits.len())
        .map_err(|_| RuntimeMathError::AllocationFailed {
            elements: bits.len(),
        })?;
    values.extend(bits.iter().copied().map(decode));
    Ok(values)
}
