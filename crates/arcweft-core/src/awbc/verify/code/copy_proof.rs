//! Retained-value obligations of the original verifier's Copy lattice.
//!
//! Resource/type admission precedes this borrowed relation. It uses the shared
//! value grammar and the enclosing work counter, never a fresh node/byte quota.

use super::CopyProof;
use crate::entry::RuntimeSchemaError;
use crate::entry::schema::value_budget::ValidationWork;
use crate::entry::schema::value_encoding::{self, ValueAdmission, ValueValidation};
use crate::value::ownership::RuntimeValueOwnershipRequirement;
use crate::value::{RuntimePlaceStorage, RuntimeValueView};

impl CopyProof {
    pub(super) fn requires_retained_check(&self) -> bool {
        match self {
            Self::Affine => false,
            // Shape can become a Copy premise after a later removal, even
            // when all current children are affine.
            Self::Copyable
            | Self::Tuple(_)
            | Self::Record(_)
            | Self::Sequence(_)
            | Self::Function(_)
            | Self::Variant { .. } => true,
            Self::UniformSequence(item) => item.requires_retained_check(),
        }
    }

    pub(in crate::awbc) fn ownership_requirement(&self) -> RuntimeValueOwnershipRequirement {
        if self.permits_copy() {
            RuntimeValueOwnershipRequirement::Unrestricted
        } else {
            RuntimeValueOwnershipRequirement::Owned
        }
    }

    /// Checks surviving children without rejecting their valid affine siblings.
    pub(in crate::awbc) fn validate_storage<'value, T>(
        &self,
        storage: &'value RuntimePlaceStorage<T>,
        work: &ValidationWork,
        view: &impl Fn(&'value T) -> RuntimeValueView<'value>,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        if !self.requires_retained_check() || storage.is_vacant() {
            return Ok(());
        }
        if let Some(value) = storage.as_ref() {
            return value_encoding::validate_admitted_view(
                view(value),
                &mut CopyProofValidation {
                    work,
                    root_depth: depth,
                },
                self,
            );
        }
        work.clone().charge(depth)?;
        let Some((_, children)) = storage.record_parts() else {
            return Err(proof_shape("partial place"));
        };
        match self {
            Self::Copyable => children.iter().enumerate().try_for_each(|(index, child)| {
                self.validate_storage(child, work, view, depth + 1)
                    .map_err(|error| error.within_value_path(&format!("$[{index}]")))
            }),
            Self::Record(proofs) if proofs.len() == children.len() => proofs
                .iter()
                .zip(children)
                .enumerate()
                .try_for_each(|(index, (proof, child))| {
                    proof
                        .validate_storage(child, work, view, depth + 1)
                        .map_err(|error| error.within_value_path(&format!("$[{index}]")))
                }),
            _ => Err(proof_shape("partial record")),
        }
    }
}

fn proof_shape(actual: &'static str) -> RuntimeSchemaError {
    RuntimeSchemaError::Type {
        path: "$".to_owned(),
        expected: "original Copy-proof value shape",
        actual,
    }
}

struct CopyProofValidation<'work> {
    work: &'work ValidationWork,
    root_depth: usize,
}

impl<'proof> ValueValidation for CopyProofValidation<'proof> {
    type Expected = &'proof CopyProof;
    type Children = &'proof CopyProof;
    type Alternatives = std::iter::Empty<&'proof CopyProof>;

    fn preflight(&mut self, _: &Self::Expected, depth: usize) -> Result<(), RuntimeSchemaError> {
        self.work
            .clone()
            .charge(self.root_depth.saturating_add(depth))
    }

    fn alternative(&mut self, depth: usize) -> Result<(), RuntimeSchemaError> {
        self.work
            .clone()
            .charge(self.root_depth.saturating_add(depth))
    }

    fn embedded_node(&mut self, depth: usize) -> Result<(), RuntimeSchemaError> {
        self.alternative(depth)
    }

    fn is_admitted(expected: &Self::Expected) -> bool {
        matches!(expected, CopyProof::Affine)
    }

    fn admitted_children() -> Self::Children {
        &CopyProof::Affine
    }

    fn enter(
        &mut self,
        expected: Self::Expected,
        value: RuntimeValueView<'_>,
    ) -> Result<ValueAdmission<Self::Children, Self::Alternatives>, RuntimeSchemaError> {
        let matches = match (expected, value) {
            (CopyProof::Affine, _)
            | (CopyProof::UniformSequence(_), RuntimeValueView::Sequence(_)) => true,
            (CopyProof::Copyable, _) => {
                if !RuntimeValueOwnershipRequirement::Unrestricted
                    .accepts(value.intrinsic_ownership())
                {
                    return Err(RuntimeSchemaError::Type {
                        path: "$".to_owned(),
                        expected: "unrestricted value graph",
                        actual: value.type_name(),
                    });
                }
                true
            }
            (CopyProof::Tuple(proofs), RuntimeValueView::Tuple(values)) => {
                proofs.len() == values.len()
            }
            (CopyProof::Record(proofs), RuntimeValueView::Record(values)) => {
                proofs.len() == values.len()
            }
            (CopyProof::Record(proofs), RuntimeValueView::NominalRecord(value)) => {
                proofs.len() == value.fields().len()
            }
            (CopyProof::Sequence(proofs), RuntimeValueView::Sequence(values)) => {
                values.len()? == proofs.len()
            }
            (CopyProof::Function(proofs), RuntimeValueView::Callable(value)) => {
                proofs.len() == value.retained().len()
            }
            (CopyProof::Variant { case, payload }, RuntimeValueView::Variant(value)) => {
                *case == value.ordinal() && payload.is_some() == value.payload().is_some()
            }
            _ => false,
        };
        if !matches {
            return Err(proof_shape(value.type_name()));
        }
        Ok(ValueAdmission::Children(expected))
    }

    fn child(
        &mut self,
        children: &Self::Children,
        index: usize,
    ) -> Result<Self::Expected, RuntimeSchemaError> {
        match children {
            CopyProof::Copyable | CopyProof::Affine => Ok(children),
            CopyProof::Tuple(proofs)
            | CopyProof::Record(proofs)
            | CopyProof::Sequence(proofs)
            | CopyProof::Function(proofs) => proofs
                .get(index)
                .ok_or_else(|| proof_shape("absent original child")),
            CopyProof::UniformSequence(item) => Ok(item),
            CopyProof::Variant { payload, .. } if index == 0 => payload
                .as_deref()
                .ok_or_else(|| proof_shape("absent original payload")),
            CopyProof::Variant { .. } => Err(proof_shape("unknown original payload")),
        }
    }
}
