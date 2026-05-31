use std::ops::{Add, AddAssign, Mul};
use thiserror::Error;

/// Row-major two-dimensional matrix shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatrixShape {
    rows: usize,
    cols: usize,
}

impl MatrixShape {
    pub const fn new(rows: usize, cols: usize) -> Self {
        Self { rows, cols }
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
}

/// Dense row-major matrix used as the deterministic runtime baseline.
#[derive(Clone, Debug, PartialEq)]
pub struct DenseMatrix<T> {
    shape: MatrixShape,
    values: Vec<T>,
}

pub type DenseMatrixF32 = DenseMatrix<f32>;
pub type DenseMatrixF64 = DenseMatrix<f64>;

impl<T> DenseMatrix<T> {
    pub fn new(rows: usize, cols: usize, values: Vec<T>) -> Result<Self, RuntimeMathError> {
        let shape = MatrixShape::new(rows, cols);
        if values.len() != shape.element_count() {
            return Err(RuntimeMathError::InvalidElementCount {
                expected: shape.element_count(),
                found: values.len(),
            });
        }
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
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            shape: MatrixShape::new(rows, cols),
            values: vec![T::default(); rows * cols],
        }
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
        let mut out = vec![T::default(); self.rows() * rhs.cols()];
        matmul_row_major(
            self.values(),
            rhs.values(),
            &mut out,
            self.rows(),
            self.cols(),
            rhs.cols(),
        );
        Self::new(self.rows(), rhs.cols(), out)
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TensorShape {
    dims: Vec<usize>,
}

impl TensorShape {
    pub fn new(dims: Vec<usize>) -> Result<Self, RuntimeMathError> {
        if dims.is_empty() {
            return Err(RuntimeMathError::InvalidTensorRank { rank: 0 });
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
        self.dims.iter().product()
    }
}

/// Dense row-major tensor used by runtime values and accelerator views.
#[derive(Clone, Debug, PartialEq)]
pub struct DenseTensor<T> {
    shape: TensorShape,
    values: Vec<T>,
}

pub type DenseTensorF32 = DenseTensor<f32>;
pub type DenseTensorF64 = DenseTensor<f64>;

impl<T> DenseTensor<T> {
    pub fn new(dims: Vec<usize>, values: Vec<T>) -> Result<Self, RuntimeMathError> {
        let shape = TensorShape::new(dims)?;
        if values.len() != shape.element_count() {
            return Err(RuntimeMathError::InvalidElementCount {
                expected: shape.element_count(),
                found: values.len(),
            });
        }
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
) where
    T: Copy + Default + AddAssign + Mul<Output = T>,
{
    out.fill(T::default());
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
}

pub fn matmul_f32_row_major(
    lhs: &[f32],
    rhs: &[f32],
    out: &mut [f32],
    rows: usize,
    shared: usize,
    cols: usize,
) {
    matmul_row_major(lhs, rhs, out, rows, shared, cols);
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimeMathError {
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

        assert_eq!(out.shape(), MatrixShape::new(2, 2));
        assert_eq!(out.values(), &[58.0, 64.0, 139.0, 154.0]);
    }

    #[test]
    fn scalar_matmul_preserves_f64_storage_width() {
        let lhs = DenseMatrixF64::new(2, 2, vec![1.5, 2.0, 3.25, 4.5]).unwrap();
        let rhs = DenseMatrixF64::new(2, 2, vec![5.0, 6.5, 7.0, 8.25]).unwrap();

        let out = lhs.matmul_scalar(&rhs).unwrap();

        assert_eq!(out.shape(), MatrixShape::new(2, 2));
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
}
