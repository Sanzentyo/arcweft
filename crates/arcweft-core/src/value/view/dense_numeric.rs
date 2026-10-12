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
        let dimension = match self.storage {
            NumericStorage::Live(RuntimeValue::MatrixF32(value)) => {
                [value.rows(), value.cols()].get(index).copied()
            }
            NumericStorage::Live(RuntimeValue::MatrixF64(value)) => {
                [value.rows(), value.cols()].get(index).copied()
            }
            NumericStorage::Live(RuntimeValue::TensorF32(value)) => {
                value.shape().dims().get(index).copied()
            }
            NumericStorage::Live(RuntimeValue::TensorF64(value)) => {
                value.shape().dims().get(index).copied()
            }
            NumericStorage::Snapshot(value) => value
                .dimensions()
                .get(index)
                .copied()
                .map(usize::try_from)
                .transpose()
                .map_err(|_| RuntimeSchemaError::Encoding {
                    message: "dense numeric dimension exceeds the selected target width".to_owned(),
                })?,
            NumericStorage::Live(_) => unreachable!("numeric view retains numeric storage"),
        };
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
