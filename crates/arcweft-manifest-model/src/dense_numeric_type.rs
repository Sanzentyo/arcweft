//! Closed scalar width and container roles for dense numeric values.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeFloatWidth {
    F32,
    F64,
}

/// One complete source and runtime type. Shapes belong to the actual value.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeDenseNumericType {
    Matrix(RuntimeFloatWidth),
    Tensor(RuntimeFloatWidth),
}

impl RuntimeDenseNumericType {
    pub const ALL: [Self; 4] = [
        Self::Matrix(RuntimeFloatWidth::F32),
        Self::Matrix(RuntimeFloatWidth::F64),
        Self::Tensor(RuntimeFloatWidth::F32),
        Self::Tensor(RuntimeFloatWidth::F64),
    ];
    pub const fn scalar_width(self) -> RuntimeFloatWidth {
        match self {
            Self::Matrix(width) | Self::Tensor(width) => width,
        }
    }

    pub const fn accepts_rank(self, rank: usize) -> bool {
        match self {
            Self::Matrix(_) => rank == 2,
            Self::Tensor(_) => rank > 0,
        }
    }

    pub const fn source_label(self) -> &'static str {
        match self {
            Self::Matrix(RuntimeFloatWidth::F32) => "MatrixF32",
            Self::Matrix(RuntimeFloatWidth::F64) => "MatrixF64",
            Self::Tensor(RuntimeFloatWidth::F32) => "TensorF32",
            Self::Tensor(RuntimeFloatWidth::F64) => "TensorF64",
        }
    }

    pub fn from_source_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|ty| ty.source_label() == label)
    }

    /// Canonical ordered container and scalar roles in semantic transcripts.
    pub const fn semantic_parts(self) -> [u8; 2] {
        let (container, width) = match self {
            Self::Matrix(width) => (0, width),
            Self::Tensor(width) => (1, width),
        };
        [
            container,
            match width {
                RuntimeFloatWidth::F32 => 0,
                RuntimeFloatWidth::F64 => 1,
            },
        ]
    }

    pub const fn from_semantic_parts(parts: [u8; 2]) -> Option<Self> {
        let width = match parts[1] {
            0 => RuntimeFloatWidth::F32,
            1 => RuntimeFloatWidth::F64,
            _ => return None,
        };
        match parts[0] {
            0 => Some(Self::Matrix(width)),
            1 => Some(Self::Tensor(width)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dense_roles_labels_and_rank_are_one_complete_contract() {
        let mut labels = std::collections::BTreeSet::new();
        let mut roles = std::collections::BTreeSet::new();
        for ty in RuntimeDenseNumericType::ALL {
            assert!(labels.insert(ty.source_label()));
            assert!(roles.insert(ty.semantic_parts()));
            assert_eq!(
                RuntimeDenseNumericType::from_source_label(ty.source_label()),
                Some(ty)
            );
            assert_eq!(
                RuntimeDenseNumericType::from_semantic_parts(ty.semantic_parts()),
                Some(ty)
            );
            assert!(!ty.accepts_rank(0));
            assert!(ty.accepts_rank(2));
            assert_eq!(
                ty.accepts_rank(1),
                matches!(ty, RuntimeDenseNumericType::Tensor(_))
            );
        }
        assert_eq!(RuntimeDenseNumericType::from_semantic_parts([2, 0]), None);
        assert_eq!(RuntimeDenseNumericType::from_semantic_parts([0, 2]), None);
        assert_eq!(
            RuntimeDenseNumericType::from_source_label("TensorF16"),
            None
        );
    }
}
