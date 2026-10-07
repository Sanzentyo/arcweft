//! Source-ordered pure program bindings with one derived immutable lookup index.

use super::RuntimePureProgramBinding;
use arcweft_id::runtime_program::RuntimePureProgramId;
use std::collections::{BTreeMap, btree_map::Entry};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimePureProgramTable {
    rows: Vec<RuntimePureProgramBinding>,
    by_program: BTreeMap<RuntimePureProgramId, ProgramPosition>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProgramPosition {
    Unique(usize),
    Ambiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimePureProgramLookupError {
    Missing,
    Ambiguous,
}

impl RuntimePureProgramTable {
    pub(super) fn from_rows(rows: Vec<RuntimePureProgramBinding>) -> Self {
        let mut by_program = BTreeMap::new();
        for (ordinal, binding) in rows.iter().enumerate() {
            match by_program.entry(binding.program()) {
                Entry::Vacant(entry) => {
                    entry.insert(ProgramPosition::Unique(ordinal));
                }
                Entry::Occupied(mut entry) => {
                    entry.insert(ProgramPosition::Ambiguous);
                }
            }
        }
        Self { rows, by_program }
    }
    pub(super) fn as_slice(&self) -> &[RuntimePureProgramBinding] {
        &self.rows
    }
    pub(super) fn resolve(
        &self,
        program: RuntimePureProgramId,
    ) -> Result<&RuntimePureProgramBinding, RuntimePureProgramLookupError> {
        match self.by_program.get(&program) {
            Some(ProgramPosition::Unique(ordinal)) => Ok(&self.rows[*ordinal]),
            Some(ProgramPosition::Ambiguous) => Err(RuntimePureProgramLookupError::Ambiguous),
            None => Err(RuntimePureProgramLookupError::Missing),
        }
    }
}

#[cfg(test)]
mod tests;
