//! Plan-local declaration identity table and its sole raw builder.

use std::num::NonZeroU32;

use thiserror::Error;

use crate::runtime_id::RuntimeLocalDeclarationId;

/// The complete contiguous local-declaration identity domain of one plan.
///
/// Identity `1` names the first local pushed into the builder and `len` names
/// the last. The external lowerer owns the HIR-to-runtime map, while this core
/// table is the final authority for whether a runtime identity belongs to the
/// plan.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeLocalDeclarationTable {
    len: u32,
}

impl RuntimeLocalDeclarationTable {
    /// Number of local declarations in the plan.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns whether the exact local identity belongs to this table.
    #[must_use]
    pub const fn contains(&self, local: RuntimeLocalDeclarationId) -> bool {
        local.get().get() <= self.len
    }
}

/// Sole raw-construction owner for one contiguous plan-local declaration table.
///
/// The builder is deliberately not cloneable. A compiler or other trusted
/// structural integrator pushes locals once in its accepted canonical order,
/// retains the returned IDs in its higher-layer map, and seals the domain
/// before constructing binding coordinates.
#[derive(Debug)]
pub struct RuntimeLocalDeclarationTableBuilder {
    next: Option<NonZeroU32>,
    issued: u32,
}

/// Exhaustion of the bounded plan-local declaration identity domain.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeLocalDeclarationTableError {
    #[error("runtime local-declaration identity space is exhausted")]
    IdentityExhausted,
}

impl RuntimeLocalDeclarationTableBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: Some(NonZeroU32::MIN),
            issued: 0,
        }
    }

    /// Appends one declaration and returns its contiguous plan-local identity.
    pub fn push(&mut self) -> Result<RuntimeLocalDeclarationId, RuntimeLocalDeclarationTableError> {
        let current = self
            .next
            .ok_or(RuntimeLocalDeclarationTableError::IdentityExhausted)?;
        self.issued = current.get();
        self.next = NonZeroU32::new(current.get().wrapping_add(1));
        Ok(RuntimeLocalDeclarationId::from_accepted_ordinal(current))
    }

    /// Seals the complete identity domain. No mutable issuer survives finish.
    #[must_use]
    pub const fn finish(self) -> RuntimeLocalDeclarationTable {
        RuntimeLocalDeclarationTable { len: self.issued }
    }
}

impl Default for RuntimeLocalDeclarationTableBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_issues_one_contiguous_table_and_exhausts_without_wrapping() {
        let mut builder = RuntimeLocalDeclarationTableBuilder::new();
        let first = builder.push().expect("first local");
        let second = builder.push().expect("second local");
        assert_eq!(first.get(), NonZeroU32::MIN);
        assert_eq!(second.get(), NonZeroU32::new(2).unwrap());

        builder.next = Some(NonZeroU32::MAX);
        let last = builder.push().expect("maximum local");
        assert_eq!(last.get(), NonZeroU32::MAX);
        assert_eq!(
            builder.push(),
            Err(RuntimeLocalDeclarationTableError::IdentityExhausted)
        );

        let table = builder.finish();
        assert_eq!(table.len(), u32::MAX);
        assert!(table.contains(first));
        assert!(table.contains(last));
    }

    #[test]
    fn empty_builder_seals_an_empty_table() {
        let table = RuntimeLocalDeclarationTableBuilder::new().finish();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
    }
}
