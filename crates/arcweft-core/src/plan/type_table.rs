//! Canonical plan-local semantic type declaration interning.

use std::{collections::BTreeMap, num::NonZeroU32};

use thiserror::Error;

use crate::{pattern::RuntimeSemanticTypeId, runtime_id::RuntimePlanTypeId};

use super::RuntimePlanTypeKind;

/// One exact semantic identity and its selected runtime representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePlanTypeDeclaration {
    semantic_identity: RuntimeSemanticTypeId,
    kind: RuntimePlanTypeKind,
}

impl RuntimePlanTypeDeclaration {
    #[must_use]
    pub const fn new(semantic_identity: RuntimeSemanticTypeId, kind: RuntimePlanTypeKind) -> Self {
        Self {
            semantic_identity,
            kind,
        }
    }

    #[must_use]
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        self.semantic_identity
    }

    #[must_use]
    pub const fn kind(&self) -> &RuntimePlanTypeKind {
        &self.kind
    }
}

/// Immutable contiguous plan-local semantic type table.
///
/// ID `1` resolves the first distinct declaration interned in the accepted
/// canonical traversal, and every later distinct declaration follows without
/// a gap. Exact duplicates do not add rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePlanTypeTable {
    declarations: Box<[RuntimePlanTypeDeclaration]>,
}

impl RuntimePlanTypeTable {
    #[must_use]
    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    /// Resolves one ID issued by this table's builder.
    #[must_use]
    pub fn get(&self, id: RuntimePlanTypeId) -> Option<&RuntimePlanTypeDeclaration> {
        usize::try_from(id.get().get() - 1)
            .ok()
            .and_then(|index| self.declarations.get(index))
    }
}

/// Sole issuer for one plan's semantic type declaration identities.
///
/// The trusted lowerer calls [`Self::intern`] in its accepted canonical node
/// traversal. Because IDs are returned immediately for typed nodes, the
/// builder preserves that first-seen order rather than sorting and remapping
/// IDs at finish. Aggregate plan construction owns and delegates to this same
/// issuer; it does not introduce another type-ID allocator.
#[derive(Debug)]
pub struct RuntimePlanTypeTableBuilder {
    by_semantic_identity: BTreeMap<RuntimeSemanticTypeId, InternedRuntimePlanType>,
    #[cfg(test)]
    maximum: u32,
}

#[derive(Debug)]
struct InternedRuntimePlanType {
    id: RuntimePlanTypeId,
    declaration: RuntimePlanTypeDeclaration,
}

/// Failure to intern one plan-local semantic type declaration.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePlanTypeTableError {
    #[error("semantic type {semantic_identity:?} has conflicting runtime plan kinds")]
    ConflictingKind {
        semantic_identity: RuntimeSemanticTypeId,
    },
    #[error("runtime plan type identity space is exhausted")]
    IdentityExhausted,
}

impl RuntimePlanTypeTableBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            by_semantic_identity: BTreeMap::new(),
            #[cfg(test)]
            maximum: u32::MAX,
        }
    }

    /// Returns the existing ID for an exact duplicate or issues the next
    /// contiguous ID for a new semantic identity.
    ///
    /// A semantic identity already paired with a different kind is rejected
    /// without changing either the canonical row sequence or identity map.
    pub fn intern(
        &mut self,
        declaration: RuntimePlanTypeDeclaration,
    ) -> Result<RuntimePlanTypeId, RuntimePlanTypeTableError> {
        if let Some(existing) = self
            .by_semantic_identity
            .get(&declaration.semantic_identity())
        {
            if existing.declaration.kind() == declaration.kind() {
                return Ok(existing.id);
            }
            return Err(RuntimePlanTypeTableError::ConflictingKind {
                semantic_identity: declaration.semantic_identity(),
            });
        }

        let ordinal = self.next_ordinal()?;
        let id = RuntimePlanTypeId::from_accepted_ordinal(ordinal);
        self.by_semantic_identity.insert(
            declaration.semantic_identity(),
            InternedRuntimePlanType { id, declaration },
        );
        Ok(id)
    }

    /// Seals the declaration rows. No mutable issuer survives finish.
    #[must_use]
    pub fn finish(self) -> RuntimePlanTypeTable {
        let mut rows = self
            .by_semantic_identity
            .into_values()
            .collect::<Vec<InternedRuntimePlanType>>();
        rows.sort_unstable_by_key(|row| row.id);
        RuntimePlanTypeTable {
            declarations: rows
                .into_iter()
                .map(|row| row.declaration)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    fn next_ordinal(&self) -> Result<NonZeroU32, RuntimePlanTypeTableError> {
        let next = self
            .by_semantic_identity
            .len()
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .and_then(NonZeroU32::new)
            .ok_or(RuntimePlanTypeTableError::IdentityExhausted)?;
        #[cfg(test)]
        if next.get() > self.maximum {
            return Err(RuntimePlanTypeTableError::IdentityExhausted);
        }
        Ok(next)
    }

    #[cfg(test)]
    fn with_maximum_for_test(maximum: u32) -> Self {
        Self {
            maximum,
            ..Self::new()
        }
    }
}

impl Default for RuntimePlanTypeTableBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{pattern::RuntimeCheckedType, plan::RuntimeOperationalType};

    fn identity(marker: u8) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes([marker; 32])
    }

    fn checked(marker: u8, checked: RuntimeCheckedType) -> RuntimePlanTypeDeclaration {
        RuntimePlanTypeDeclaration::new(identity(marker), RuntimePlanTypeKind::Checked(checked))
    }

    #[test]
    fn distinct_declarations_follow_canonical_intern_order_without_gaps() {
        let first = checked(1, RuntimeCheckedType::Unit);
        let second = checked(2, RuntimeCheckedType::Bool);
        let mut builder = RuntimePlanTypeTableBuilder::new();

        let first_id = builder.intern(first.clone()).expect("first type");
        let second_id = builder.intern(second.clone()).expect("second type");
        assert_eq!(first_id.get(), NonZeroU32::MIN);
        assert_eq!(second_id.get(), NonZeroU32::new(2).unwrap());

        let table = builder.finish();
        assert_eq!(table.len(), 2);
        assert_eq!(table.get(first_id), Some(&first));
        assert_eq!(table.get(second_id), Some(&second));
    }

    #[test]
    fn exact_duplicates_reuse_the_first_id_and_row() {
        let declaration = checked(3, RuntimeCheckedType::String);
        let mut builder = RuntimePlanTypeTableBuilder::new();
        let first = builder
            .intern(declaration.clone())
            .expect("first declaration");
        let duplicate = builder
            .intern(declaration.clone())
            .expect("exact duplicate");

        assert_eq!(duplicate, first);
        let table = builder.finish();
        assert_eq!(table.len(), 1);
        assert_eq!(table.get(first), Some(&declaration));
    }

    #[test]
    fn one_semantic_identity_cannot_change_runtime_kind() {
        let semantic_identity = identity(4);
        let mut builder = RuntimePlanTypeTableBuilder::new();
        let first = RuntimePlanTypeDeclaration::new(
            semantic_identity,
            RuntimePlanTypeKind::Checked(RuntimeCheckedType::Unit),
        );
        let conflict = RuntimePlanTypeDeclaration::new(
            semantic_identity,
            RuntimePlanTypeKind::Operational(RuntimeOperationalType::Range),
        );
        let first_id = builder.intern(first.clone()).expect("first declaration");

        assert_eq!(
            builder.intern(conflict),
            Err(RuntimePlanTypeTableError::ConflictingKind { semantic_identity })
        );
        let table = builder.finish();
        assert_eq!(table.len(), 1);
        assert_eq!(table.get(first_id), Some(&first));
    }

    #[test]
    fn exhaustion_is_transactional_and_still_allows_exact_deduplication() {
        let first = checked(5, RuntimeCheckedType::Unit);
        let second = checked(6, RuntimeCheckedType::Bool);
        let mut builder = RuntimePlanTypeTableBuilder::with_maximum_for_test(1);
        let first_id = builder.intern(first.clone()).expect("bounded first type");

        assert_eq!(
            builder.intern(second),
            Err(RuntimePlanTypeTableError::IdentityExhausted)
        );
        assert_eq!(
            builder.intern(first.clone()),
            Ok(first_id),
            "capacity cannot shadow exact deduplication"
        );
        let table = builder.finish();
        assert_eq!(table.len(), 1);
        assert_eq!(table.get(first_id), Some(&first));
    }
}
