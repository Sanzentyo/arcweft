//! Borrowed numeric headers and scalar storage for live and inert admission.

use crate::{
    entry::RuntimeSchemaError,
    math::{RuntimeDenseNumericElements, RuntimeDenseNumericPayload, RuntimeDenseNumericType},
    value::{RuntimeScalarView, RuntimeValue},
};

#[derive(Clone, Copy, Debug)]
enum NumericStorage<'a> {
    Live(&'a RuntimeValue),
    Snapshot(&'a RuntimeDenseNumericPayload),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeDenseNumericView<'a> {
    storage: NumericStorage<'a>,
}

impl<'a> RuntimeDenseNumericView<'a> {
    pub(super) const fn live(value: &'a RuntimeValue) -> Self {
        Self {
            storage: NumericStorage::Live(value),
        }
    }

    pub(in crate::value) const fn snapshot(value: &'a RuntimeDenseNumericPayload) -> Self {
        Self {
            storage: NumericStorage::Snapshot(value),
        }
    }

    pub(crate) fn ty(self) -> RuntimeDenseNumericType {
        match self.storage {
            NumericStorage::Live(value) => value
                .dense_numeric_type()
                .expect("numeric views are issued only for dense numeric storage"),
            NumericStorage::Snapshot(value) => value.ty(),
        }
    }

    pub(in crate::value) const fn live_value(self) -> Option<&'a RuntimeValue> {
        match self.storage {
            NumericStorage::Live(value) => Some(value),
            NumericStorage::Snapshot(_) => None,
        }
    }

    pub(crate) fn rank(self) -> usize {
        match self.storage {
            NumericStorage::Live(RuntimeValue::MatrixF32(_) | RuntimeValue::MatrixF64(_)) => 2,
            NumericStorage::Live(RuntimeValue::TensorF32(value)) => value.shape().dims().len(),
            NumericStorage::Live(RuntimeValue::TensorF64(value)) => value.shape().dims().len(),
            NumericStorage::Snapshot(value) => value.dimensions().len(),
            NumericStorage::Live(_) => unreachable!("numeric view retains numeric storage"),
        }
    }

    pub(crate) fn dimension(self, index: usize) -> Result<usize, RuntimeSchemaError> {
        usize::try_from(self.dimension_u64(index)?).map_err(|_| RuntimeSchemaError::Encoding {
            message: "dense numeric dimension exceeds the selected target width".to_owned(),
        })
    }

    /// Borrows the fixed-width source header before any target-sized live
    /// materialization. Zero-element snapshots can retain very large dimensions.
    pub(in crate::value) fn dimension_u64(self, index: usize) -> Result<u64, RuntimeSchemaError> {
        let dimension = match self.storage {
            NumericStorage::Live(RuntimeValue::MatrixF32(value)) => [value.rows(), value.cols()]
                .get(index)
                .copied()
                .map(u64::try_from)
                .transpose(),
            NumericStorage::Live(RuntimeValue::MatrixF64(value)) => [value.rows(), value.cols()]
                .get(index)
                .copied()
                .map(u64::try_from)
                .transpose(),
            NumericStorage::Live(RuntimeValue::TensorF32(value)) => value
                .shape()
                .dims()
                .get(index)
                .copied()
                .map(u64::try_from)
                .transpose(),
            NumericStorage::Live(RuntimeValue::TensorF64(value)) => value
                .shape()
                .dims()
                .get(index)
                .copied()
                .map(u64::try_from)
                .transpose(),
            NumericStorage::Snapshot(value) => Ok(value.dimensions().get(index).copied()),
            NumericStorage::Live(_) => unreachable!("numeric view retains numeric storage"),
        }
        .map_err(|_| RuntimeSchemaError::Encoding {
            message: "dense numeric dimension exceeds its fixed-width source domain".to_owned(),
        })?;
        dimension.ok_or_else(|| RuntimeSchemaError::Encoding {
            message: "dense numeric header is missing a dimension".to_owned(),
        })
    }

    pub(crate) fn len(self) -> usize {
        match self.storage {
            NumericStorage::Live(RuntimeValue::MatrixF32(value)) => value.values().len(),
            NumericStorage::Live(RuntimeValue::MatrixF64(value)) => value.values().len(),
            NumericStorage::Live(RuntimeValue::TensorF32(value)) => value.values().len(),
            NumericStorage::Live(RuntimeValue::TensorF64(value)) => value.values().len(),
            NumericStorage::Snapshot(value) => value.elements().len(),
            NumericStorage::Live(_) => unreachable!("numeric view retains numeric storage"),
        }
    }

    pub(crate) fn values(self) -> impl ExactSizeIterator<Item = RuntimeScalarView<'static>> {
        (0..self.len()).map(move |index| match self.storage {
            NumericStorage::Live(RuntimeValue::MatrixF32(value)) => {
                RuntimeScalarView::F32(value.values()[index])
            }
            NumericStorage::Live(RuntimeValue::MatrixF64(value)) => {
                RuntimeScalarView::F64(value.values()[index])
            }
            NumericStorage::Live(RuntimeValue::TensorF32(value)) => {
                RuntimeScalarView::F32(value.values()[index])
            }
            NumericStorage::Live(RuntimeValue::TensorF64(value)) => {
                RuntimeScalarView::F64(value.values()[index])
            }
            NumericStorage::Snapshot(value) => match value.elements() {
                RuntimeDenseNumericElements::F32(values) => {
                    RuntimeScalarView::F32(f32::from_bits(values[index]))
                }
                RuntimeDenseNumericElements::F64(values) => {
                    RuntimeScalarView::F64(f64::from_bits(values[index]))
                }
            },
            NumericStorage::Live(_) => unreachable!("numeric view retains numeric storage"),
        })
    }
}
