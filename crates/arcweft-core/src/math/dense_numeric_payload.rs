//! Checked portable dense images shared by AWBC constants and value snapshots.

use crate::math::{RuntimeDenseNumericType, RuntimeFloatWidth};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeDenseNumericElements {
    F32(Vec<u32>),
    F64(Vec<u64>),
}

impl RuntimeDenseNumericElements {
    pub const fn scalar_width(&self) -> RuntimeFloatWidth {
        match self {
            Self::F32(_) => RuntimeFloatWidth::F32,
            Self::F64(_) => RuntimeFloatWidth::F64,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::F32(values) => values.len(),
            Self::F64(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// An immutable admitted shape/storage image; no matrix is erased to a tensor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "DenseNumericPayloadWire")]
pub struct RuntimeDenseNumericPayload {
    ty: RuntimeDenseNumericType,
    dimensions: Vec<u64>,
    elements: RuntimeDenseNumericElements,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeDenseNumericPayloadError {
    #[error("runtime value is not dense numeric storage")]
    NotDense,
    #[error("dense dimension exceeds the portable u64 image")]
    DimensionOverflow,
    #[error("dense payload scalar width {actual:?} does not match {expected:?}")]
    ScalarWidth {
        expected: RuntimeFloatWidth,
        actual: RuntimeFloatWidth,
    },
    #[error("dense payload {ty:?} does not admit rank {rank}")]
    Rank {
        ty: RuntimeDenseNumericType,
        rank: usize,
    },
    #[error("dense payload element count exceeds u64")]
    ElementCountOverflow,
    #[error("dense payload expected {expected} elements, found {actual}")]
    ElementCount { expected: u64, actual: u64 },
}

impl RuntimeDenseNumericPayload {
    pub fn try_new(
        ty: RuntimeDenseNumericType,
        dimensions: Vec<u64>,
        elements: RuntimeDenseNumericElements,
    ) -> Result<Self, RuntimeDenseNumericPayloadError> {
        if ty.scalar_width() != elements.scalar_width() {
            return Err(RuntimeDenseNumericPayloadError::ScalarWidth {
                expected: ty.scalar_width(),
                actual: elements.scalar_width(),
            });
        }
        Self::validate_shape(ty, &dimensions, elements.len())?;
        Ok(Self {
            ty,
            dimensions,
            elements,
        })
    }

    pub(crate) fn validate_shape(
        ty: RuntimeDenseNumericType,
        dimensions: &[u64],
        element_count: usize,
    ) -> Result<(), RuntimeDenseNumericPayloadError> {
        if !ty.accepts_rank(dimensions.len()) {
            return Err(RuntimeDenseNumericPayloadError::Rank {
                ty,
                rank: dimensions.len(),
            });
        }
        let expected = if dimensions.contains(&0) {
            0
        } else {
            dimensions
                .iter()
                .try_fold(1_u64, |count, dimension| count.checked_mul(*dimension))
                .ok_or(RuntimeDenseNumericPayloadError::ElementCountOverflow)?
        };
        let actual = u64::try_from(element_count)
            .map_err(|_| RuntimeDenseNumericPayloadError::ElementCountOverflow)?;
        if expected != actual {
            return Err(RuntimeDenseNumericPayloadError::ElementCount { expected, actual });
        }
        Ok(())
    }

    pub const fn ty(&self) -> RuntimeDenseNumericType {
        self.ty
    }
    pub fn dimensions(&self) -> &[u64] {
        &self.dimensions
    }
    pub const fn elements(&self) -> &RuntimeDenseNumericElements {
        &self.elements
    }
}

#[derive(Deserialize)]
#[serde(rename = "RuntimeDenseNumericPayload", deny_unknown_fields)]
struct DenseNumericPayloadWire {
    ty: RuntimeDenseNumericType,
    dimensions: Vec<u64>,
    elements: RuntimeDenseNumericElements,
}

impl TryFrom<DenseNumericPayloadWire> for RuntimeDenseNumericPayload {
    type Error = RuntimeDenseNumericPayloadError;

    fn try_from(wire: DenseNumericPayloadWire) -> Result<Self, Self::Error> {
        Self::try_new(wire.ty, wire.dimensions, wire.elements)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_constant_admission_retains_rank_width_and_storage() {
        let matrix = RuntimeDenseNumericType::Matrix(RuntimeFloatWidth::F32);
        let tensor = RuntimeDenseNumericType::Tensor(RuntimeFloatWidth::F64);
        assert!(
            RuntimeDenseNumericPayload::try_new(
                matrix,
                vec![1],
                RuntimeDenseNumericElements::F32(vec![0])
            )
            .is_err()
        );
        assert!(
            RuntimeDenseNumericPayload::try_new(
                matrix,
                vec![1, 1],
                RuntimeDenseNumericElements::F64(vec![0])
            )
            .is_err()
        );
        assert!(
            RuntimeDenseNumericPayload::try_new(
                matrix,
                vec![1, 2],
                RuntimeDenseNumericElements::F32(vec![0])
            )
            .is_err()
        );
        assert!(
            RuntimeDenseNumericPayload::try_new(
                tensor,
                vec![],
                RuntimeDenseNumericElements::F64(vec![0])
            )
            .is_err()
        );
        assert!(
            RuntimeDenseNumericPayload::try_new(
                tensor,
                vec![u64::MAX, 2],
                RuntimeDenseNumericElements::F64(vec![])
            )
            .is_err()
        );
        assert!(
            RuntimeDenseNumericPayload::try_new(
                tensor,
                vec![u64::MAX, 2, 0],
                RuntimeDenseNumericElements::F64(vec![])
            )
            .is_ok()
        );
        let valid = RuntimeDenseNumericPayload::try_new(
            matrix,
            vec![1, 2],
            RuntimeDenseNumericElements::F32(vec![1, 2]),
        )
        .unwrap();
        let restored: RuntimeDenseNumericPayload =
            serde_json::from_value(serde_json::to_value(&valid).unwrap()).unwrap();
        assert_eq!(restored, valid);
        let mut invalid = serde_json::to_value(&valid).unwrap();
        invalid["dimensions"] = serde_json::json!([1, 3]);
        assert!(serde_json::from_value::<RuntimeDenseNumericPayload>(invalid).is_err());
    }
}
