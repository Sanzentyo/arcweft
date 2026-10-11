use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Mul};
use thiserror::Error;

pub use arcweft_manifest_model::{RuntimeDenseNumericType, RuntimeFloatWidth};

mod dense_numeric_payload;
pub use dense_numeric_payload::{
    RuntimeDenseNumericElements, RuntimeDenseNumericPayload, RuntimeDenseNumericPayloadError,
};

/// Row-major two-dimensional matrix shape.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "MatrixShapeWire")]
pub struct MatrixShape {
    rows: usize,
    cols: usize,
}

impl MatrixShape {
    pub const fn new(rows: usize, cols: usize) -> Result<Self, RuntimeMathError> {
        if rows.checked_mul(cols).is_none() {
            return Err(RuntimeMathError::ElementCountOverflow);
        }
        Ok(Self { rows, cols })
    }

    pub const fn rows(self) -> usize {
        self.rows
    }

    pub const fn cols(self) -> usize {
        self.cols
    }

    pub const fn element_count(self) -> usize {
        self.rows * self.cols
    }

    pub fn validate_element_count(self, found: usize) -> Result<(), RuntimeMathError> {
        if self.element_count() == found {
            Ok(())
        } else {
            Err(RuntimeMathError::InvalidElementCount {
                expected: self.element_count(),
                found,
            })
        }
    }
}

#[derive(Deserialize)]
#[serde(rename = "MatrixShape")]
struct MatrixShapeWire {
    rows: usize,
    cols: usize,
}

impl TryFrom<MatrixShapeWire> for MatrixShape {
    type Error = RuntimeMathError;

    fn try_from(shape: MatrixShapeWire) -> Result<Self, Self::Error> {
        Self::new(shape.rows, shape.cols)
    }
}

/// Dense row-major matrix used as the deterministic runtime baseline.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DenseMatrix<T> {
    shape: MatrixShape,
    values: Vec<T>,
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for DenseMatrix<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename = "DenseMatrix")]
        struct Wire<T> {
            shape: MatrixShape,
            values: Vec<T>,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.shape.rows(), wire.shape.cols(), wire.values)
            .map_err(serde::de::Error::custom)
    }
}

pub type DenseMatrixF32 = DenseMatrix<f32>;
pub type DenseMatrixF64 = DenseMatrix<f64>;

impl<T> DenseMatrix<T> {
    pub fn new(rows: usize, cols: usize, values: Vec<T>) -> Result<Self, RuntimeMathError> {
        let shape = MatrixShape::new(rows, cols)?;
        shape.validate_element_count(values.len())?;
        Ok(Self { shape, values })
    }

    pub const fn shape(&self) -> MatrixShape {
        self.shape
    }

    pub const fn rows(&self) -> usize {
        self.shape.rows()
    }

    pub const fn cols(&self) -> usize {
        self.shape.cols()
    }

    pub fn values(&self) -> &[T] {
        &self.values
    }

    pub fn values_mut(&mut self) -> &mut [T] {
        &mut self.values
    }

    pub fn into_values(self) -> Vec<T> {
        self.values
    }
}

impl<T> DenseMatrix<T>
where
    T: Clone + Default,
{
    pub fn zeros(rows: usize, cols: usize) -> Result<Self, RuntimeMathError> {
        let shape = MatrixShape::new(rows, cols)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(shape.element_count())
            .map_err(|_| RuntimeMathError::AllocationFailed {
                elements: shape.element_count(),
            })?;
        values.resize(shape.element_count(), T::default());
        Ok(Self { shape, values })
    }
}

impl<T> DenseMatrix<T>
where
    T: Copy + Default + AddAssign + Mul<Output = T>,
{
    pub fn matmul_scalar(&self, rhs: &Self) -> Result<Self, RuntimeMathError> {
        if self.cols() != rhs.rows() {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: self.shape,
                rhs: rhs.shape,
                op: "matmul",
            });
        }
        let mut out = Self::zeros(self.rows(), rhs.cols())?;
        matmul_row_major(
            self.values(),
            rhs.values(),
            out.values_mut(),
            self.rows(),
            self.cols(),
            rhs.cols(),
        )?;
        Ok(out)
    }
}

impl<T> DenseMatrix<T>
where
    T: Copy + Add<Output = T>,
{
    pub fn add_scalar(&self, rhs: &Self) -> Result<Self, RuntimeMathError> {
        if self.shape != rhs.shape {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: self.shape,
                rhs: rhs.shape,
                op: "add",
            });
        }
        Ok(Self {
            shape: self.shape,
            values: self
                .values
                .iter()
                .copied()
                .zip(rhs.values.iter().copied())
                .map(|(lhs, rhs)| lhs + rhs)
                .collect(),
        })
    }
}

/// Dense row-major tensor shape.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "TensorShapeWire")]
pub struct TensorShape {
    dims: Vec<usize>,
}

impl TensorShape {
    pub fn new(dims: Vec<usize>) -> Result<Self, RuntimeMathError> {
        if dims.is_empty() {
            return Err(RuntimeMathError::InvalidTensorRank { rank: 0 });
        }
        if !dims.contains(&0)
            && dims
                .iter()
                .try_fold(1_usize, |count, dim| count.checked_mul(*dim))
                .is_none()
        {
            return Err(RuntimeMathError::ElementCountOverflow);
        }
        Ok(Self { dims })
    }

    pub fn dims(&self) -> &[usize] {
        &self.dims
    }

    pub fn rank(&self) -> usize {
        self.dims.len()
    }

    pub fn element_count(&self) -> usize {
        if self.dims.contains(&0) {
            0
        } else {
            self.dims.iter().product()
        }
    }

    pub fn validate_element_count(&self, found: usize) -> Result<(), RuntimeMathError> {
        if self.element_count() == found {
            Ok(())
        } else {
            Err(RuntimeMathError::InvalidElementCount {
                expected: self.element_count(),
                found,
            })
        }
    }
}

#[derive(Deserialize)]
#[serde(rename = "TensorShape")]
struct TensorShapeWire {
    dims: Vec<usize>,
}

impl TryFrom<TensorShapeWire> for TensorShape {
    type Error = RuntimeMathError;

    fn try_from(shape: TensorShapeWire) -> Result<Self, Self::Error> {
        Self::new(shape.dims)
    }
}

/// Dense row-major tensor used by runtime values and accelerator views.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DenseTensor<T> {
    shape: TensorShape,
    values: Vec<T>,
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for DenseTensor<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename = "DenseTensor")]
        struct Wire<T> {
            shape: TensorShape,
            values: Vec<T>,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.shape.dims, wire.values).map_err(serde::de::Error::custom)
    }
}

pub type DenseTensorF32 = DenseTensor<f32>;
pub type DenseTensorF64 = DenseTensor<f64>;

impl<T> DenseTensor<T> {
    pub fn new(dims: Vec<usize>, values: Vec<T>) -> Result<Self, RuntimeMathError> {
        let shape = TensorShape::new(dims)?;
        shape.validate_element_count(values.len())?;
        Ok(Self { shape, values })
    }

    pub fn from_matrix(matrix: DenseMatrix<T>) -> Self {
        let dims = vec![matrix.rows(), matrix.cols()];
        Self {
            shape: TensorShape { dims },
            values: matrix.into_values(),
        }
    }

    pub fn as_matrix(&self) -> Option<DenseMatrix<T>>
    where
        T: Clone,
    {
        match self.shape.dims() {
            [rows, cols] => DenseMatrix::new(*rows, *cols, self.values.clone()).ok(),
            _ => None,
        }
    }

    pub fn shape(&self) -> &TensorShape {
        &self.shape
    }

    pub fn values(&self) -> &[T] {
        &self.values
    }

    pub fn values_mut(&mut self) -> &mut [T] {
        &mut self.values
    }

    pub fn into_values(self) -> Vec<T> {
        self.values
    }
}

impl<T> DenseTensor<T>
where
    T: Copy + Add<Output = T>,
{
    pub fn add_scalar(&self, rhs: &Self) -> Result<Self, RuntimeMathError> {
        if self.shape != rhs.shape {
            return Err(RuntimeMathError::TensorShapeMismatch {
                lhs: self.shape.clone(),
                rhs: rhs.shape.clone(),
                op: "add",
            });
        }
        Ok(Self {
            shape: self.shape.clone(),
            values: self
                .values
                .iter()
                .copied()
                .zip(rhs.values.iter().copied())
                .map(|(lhs, rhs)| lhs + rhs)
                .collect(),
        })
    }
}

/// Deterministic scalar baseline for row-major matrix multiplication.
pub fn matmul_row_major<T>(
    lhs: &[T],
    rhs: &[T],
    out: &mut [T],
    rows: usize,
    shared: usize,
    cols: usize,
) -> Result<(), RuntimeMathError>
where
    T: Copy + Default + AddAssign + Mul<Output = T>,
{
    for (shape, found) in [
        (MatrixShape::new(rows, shared)?, lhs.len()),
        (MatrixShape::new(shared, cols)?, rhs.len()),
        (MatrixShape::new(rows, cols)?, out.len()),
    ] {
        shape.validate_element_count(found)?;
    }
    out.fill(T::default());
    if shared == 0 || out.is_empty() {
        return Ok(());
    }
    for row in 0..rows {
        let lhs_row = &lhs[row * shared..(row + 1) * shared];
        let out_row = &mut out[row * cols..(row + 1) * cols];
        for (k, lhs_value) in lhs_row.iter().copied().enumerate() {
            let rhs_row = &rhs[k * cols..(k + 1) * cols];
            for (out_value, rhs_value) in out_row.iter_mut().zip(rhs_row.iter().copied()) {
                *out_value += lhs_value * rhs_value;
            }
        }
    }
    Ok(())
}

pub fn matmul_f32_row_major(
    lhs: &[f32],
    rhs: &[f32],
    out: &mut [f32],
    rows: usize,
    shared: usize,
    cols: usize,
) -> Result<(), RuntimeMathError> {
    matmul_row_major(lhs, rhs, out, rows, shared, cols)
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimeMathError {
    #[error("dense numeric shape element count exceeds usize")]
    ElementCountOverflow,
    #[error("dense numeric allocation for {elements} element(s) failed")]
    AllocationFailed { elements: usize },
    #[error("math value expected {expected} element(s), found {found}")]
    InvalidElementCount { expected: usize, found: usize },
    #[error("tensor rank {rank} is invalid")]
    InvalidTensorRank { rank: usize },
    #[error("matrix {op} shape mismatch: lhs {lhs:?}, rhs {rhs:?}")]
    MatrixShapeMismatch {
        lhs: MatrixShape,
        rhs: MatrixShape,
        op: &'static str,
    },
    #[error("tensor {op} shape mismatch: lhs {lhs:?}, rhs {rhs:?}")]
    TensorShapeMismatch {
        lhs: TensorShape,
        rhs: TensorShape,
        op: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_matmul_uses_row_major_shape() {
        let lhs = DenseMatrixF32::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        let rhs = DenseMatrixF32::new(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]).unwrap();

        let out = lhs.matmul_scalar(&rhs).unwrap();

        assert_eq!(out.shape(), MatrixShape::new(2, 2).unwrap());
        assert_eq!(out.values(), &[58.0, 64.0, 139.0, 154.0]);
    }

    #[test]
    fn scalar_matmul_preserves_f64_storage_width() {
        let lhs = DenseMatrixF64::new(2, 2, vec![1.5, 2.0, 3.25, 4.5]).unwrap();
        let rhs = DenseMatrixF64::new(2, 2, vec![5.0, 6.5, 7.0, 8.25]).unwrap();

        let out = lhs.matmul_scalar(&rhs).unwrap();

        assert_eq!(out.shape(), MatrixShape::new(2, 2).unwrap());
        assert_eq!(out.values(), &[21.5, 26.25, 47.75, 58.25]);
    }

    #[test]
    fn tensor_add_requires_matching_shape() {
        let lhs = DenseTensorF32::new(vec![2, 2], vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        let rhs = DenseTensorF32::new(vec![2, 2], vec![5.0, 6.0, 7.0, 8.0]).unwrap();

        let out = lhs.add_scalar(&rhs).unwrap();

        assert_eq!(out.shape().dims(), &[2, 2]);
        assert_eq!(out.values(), &[6.0, 8.0, 10.0, 12.0]);
    }

    #[test]
    fn tensor_add_preserves_f64_storage_width() {
        let lhs = DenseTensorF64::new(vec![2, 2], vec![1.5, 2.25, 3.75, 4.5]).unwrap();
        let rhs = DenseTensorF64::new(vec![2, 2], vec![5.0, 6.25, 7.5, 8.75]).unwrap();

        let out = lhs.add_scalar(&rhs).unwrap();

        assert_eq!(out.shape().dims(), &[2, 2]);
        assert_eq!(out.values(), &[6.5, 8.5, 11.25, 13.25]);
    }

    #[test]
    fn dense_shape_admission_checks_rank_overflow_and_zero_volume() {
        assert_eq!(
            MatrixShape::new(usize::MAX, 2),
            Err(RuntimeMathError::ElementCountOverflow)
        );
        assert_eq!(
            TensorShape::new(vec![usize::MAX, 2]),
            Err(RuntimeMathError::ElementCountOverflow)
        );
        assert_eq!(
            TensorShape::new(vec![]),
            Err(RuntimeMathError::InvalidTensorRank { rank: 0 })
        );
        assert_eq!(MatrixShape::new(usize::MAX, 0).unwrap().element_count(), 0);
        assert_eq!(
            TensorShape::new(vec![usize::MAX, 2, 0])
                .unwrap()
                .element_count(),
            0
        );
        assert!(DenseMatrixF32::new(usize::MAX, 2, vec![]).is_err());
        assert!(DenseTensorF64::new(vec![usize::MAX, 2], vec![]).is_err());
    }

    #[test]
    fn scalar_matmul_rejects_overflowing_output_before_allocation() {
        let lhs = DenseMatrixF32::new(usize::MAX, 0, vec![]).unwrap();
        let rhs = DenseMatrixF32::new(0, 2, vec![]).unwrap();
        assert_eq!(
            lhs.matmul_scalar(&rhs),
            Err(RuntimeMathError::ElementCountOverflow)
        );
        assert_eq!(
            DenseMatrixF64::zeros(usize::MAX, 1),
            Err(RuntimeMathError::AllocationFailed {
                elements: usize::MAX
            })
        );
    }

    #[test]
    fn raw_matmul_admission_is_failure_atomic() {
        for (lhs, rhs, out_len) in [
            (vec![], vec![3.0], 1),
            (vec![2.0], vec![], 1),
            (vec![2.0], vec![3.0], 2),
        ] {
            let mut out = vec![7.0; out_len];
            let before = out.clone();
            assert!(matmul_f32_row_major(&lhs, &rhs, &mut out, 1, 1, 1).is_err());
            assert_eq!(out, before);
        }
        let mut out = vec![7.0];
        assert_eq!(
            matmul_f32_row_major(&[], &[], &mut out, usize::MAX, 0, 2),
            Err(RuntimeMathError::ElementCountOverflow)
        );
        assert_eq!(out, vec![7.0]);
        assert!(matmul_f32_row_major(&[], &[], &mut [], usize::MAX, 0, 0).is_ok());
        assert!(matmul_f32_row_major(&[2.0], &[3.0], &mut out, 1, 1, 1).is_ok());
        assert_eq!(out, vec![6.0]);
    }

    #[test]
    fn dense_numeric_decoders_reject_invalid_shape_and_storage() {
        for values in [vec![], vec![1.0], vec![1.0, 2.0, 3.0]] {
            let matrix = serde_json::json!({"shape": {"rows": 1, "cols": 2}, "values": values});
            assert!(serde_json::from_value::<DenseMatrixF32>(matrix.clone()).is_err());
            assert!(serde_json::from_value::<DenseMatrixF64>(matrix).is_err());
            let tensor = serde_json::json!({"shape": {"dims": [1, 2]}, "values": values});
            assert!(serde_json::from_value::<DenseTensorF32>(tensor.clone()).is_err());
            assert!(serde_json::from_value::<DenseTensorF64>(tensor).is_err());
        }
        let matrix = serde_json::json!({"shape": {"rows": usize::MAX, "cols": 2}, "values": []});
        assert!(serde_json::from_value::<DenseMatrixF32>(matrix).is_err());
        for dims in [vec![], vec![usize::MAX, 2]] {
            let tensor = serde_json::json!({"shape": {"dims": dims}, "values": []});
            assert!(serde_json::from_value::<DenseTensorF64>(tensor).is_err());
        }
    }

    #[test]
    fn dense_numeric_codec_keeps_shape_and_float_width() {
        let matrix_f32 = DenseMatrixF32::new(1, 2, vec![1.25, 2.5]).unwrap();
        let matrix_f64 = DenseMatrixF64::new(1, 2, vec![1.25, 2.5]).unwrap();
        let tensor_f32 = DenseTensorF32::new(vec![1, 2, 1], vec![1.25, 2.5]).unwrap();
        let tensor_f64 = DenseTensorF64::new(vec![1, 2, 1], vec![1.25, 2.5]).unwrap();
        assert_eq!(
            serde_json::from_value::<DenseMatrixF32>(serde_json::to_value(&matrix_f32).unwrap())
                .unwrap(),
            matrix_f32
        );
        assert_eq!(
            serde_json::from_value::<DenseMatrixF64>(serde_json::to_value(&matrix_f64).unwrap())
                .unwrap(),
            matrix_f64
        );
        assert_eq!(
            serde_json::from_value::<DenseTensorF32>(serde_json::to_value(&tensor_f32).unwrap())
                .unwrap(),
            tensor_f32
        );
        assert_eq!(
            serde_json::from_value::<DenseTensorF64>(serde_json::to_value(&tensor_f64).unwrap())
                .unwrap(),
            tensor_f64
        );
    }
}
