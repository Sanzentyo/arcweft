//! Canonical plan-local semantic type graph admission.

use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroU32,
};

use thiserror::Error;

use crate::pattern::RuntimeSemanticTypeId;
use crate::runtime_id::RuntimePlanTypeId;

use super::{RuntimeAgentTypeProjection, RuntimePlanTypeClass, RuntimePlanTypeProjection};

/// Maximum number of declarations on one semantic type path.
pub const MAX_RUNTIME_PLAN_TYPE_DEPTH: usize = 64;

/// One pre-admission semantic declaration.
///
/// Seeds are inert, non-serializable data. Only [`super::RuntimePlanBuilder`]
/// can rewrite their semantic child references into plan-local IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePlanTypeSeed {
    semantic_identity: RuntimeSemanticTypeId,
    projection: RuntimePlanTypeProjection<RuntimeSemanticTypeId>,
}

impl RuntimePlanTypeSeed {
    #[must_use]
    pub const fn new(
        semantic_identity: RuntimeSemanticTypeId,
        projection: RuntimePlanTypeProjection<RuntimeSemanticTypeId>,
    ) -> Self {
        Self {
            semantic_identity,
            projection,
        }
    }

    #[must_use]
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        self.semantic_identity
    }

    #[must_use]
    pub const fn projection(&self) -> &RuntimePlanTypeProjection<RuntimeSemanticTypeId> {
        &self.projection
    }
}

/// One exact semantic identity and its final plan-local projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePlanTypeDeclaration {
    semantic_identity: RuntimeSemanticTypeId,
    projection: RuntimePlanTypeProjection<RuntimePlanTypeId>,
}

impl RuntimePlanTypeDeclaration {
    #[must_use]
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        self.semantic_identity
    }

    #[must_use]
    pub const fn projection(&self) -> &RuntimePlanTypeProjection<RuntimePlanTypeId> {
        &self.projection
    }
}

/// Immutable contiguous plan-local semantic type table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePlanTypeTable {
    rows: Box<[RuntimePlanTypeTableRow]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RuntimePlanTypeTableRow {
    id: RuntimePlanTypeId,
    declaration: RuntimePlanTypeDeclaration,
}

impl RuntimePlanTypeTable {
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Resolves one ID issued by this table's builder.
    #[must_use]
    pub fn get(&self, id: RuntimePlanTypeId) -> Option<&RuntimePlanTypeDeclaration> {
        declaration_index(id)
            .and_then(|index| self.rows.get(index))
            .map(|row| &row.declaration)
    }

    /// Declarations in their canonical plan-local ID order.
    pub fn declarations(&self) -> impl ExactSizeIterator<Item = &RuntimePlanTypeDeclaration> {
        self.rows.iter().map(|row| &row.declaration)
    }

    /// Declarations paired with their exact contiguous plan-local identity.
    pub fn declarations_with_ids(
        &self,
    ) -> impl ExactSizeIterator<Item = (RuntimePlanTypeId, &RuntimePlanTypeDeclaration)> {
        self.rows.iter().map(|row| (row.id, &row.declaration))
    }

    /// Resolves the plan-local ID for one exact semantic identity.
    #[must_use]
    pub fn id_for_semantic(
        &self,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Option<RuntimePlanTypeId> {
        self.rows
            .iter()
            .find(|row| row.declaration.semantic_identity == semantic_identity)
            .map(|row| row.id)
    }

    pub(crate) fn is_checked(
        &self,
        id: RuntimePlanTypeId,
    ) -> Result<bool, RuntimePlanTypeResolutionError> {
        let mut memo = BTreeMap::new();
        self.is_checked_memoized(id, &mut memo)
    }

    /// Derives the execution class without retaining a parallel kind in the
    /// declaration row.
    pub(crate) fn class(
        &self,
        id: RuntimePlanTypeId,
    ) -> Result<RuntimePlanTypeClass, RuntimePlanTypeResolutionError> {
        if self.is_checked(id)? {
            return Ok(RuntimePlanTypeClass::Checked);
        }
        let declaration = self
            .get(id)
            .ok_or(RuntimePlanTypeResolutionError::UnknownType { ty: id })?;
        declaration
            .projection
            .operational_type()
            .map(RuntimePlanTypeClass::Operational)
            .ok_or(RuntimePlanTypeResolutionError::MissingRuntimeClass { ty: id })
    }

    fn is_checked_memoized(
        &self,
        id: RuntimePlanTypeId,
        memo: &mut BTreeMap<RuntimePlanTypeId, bool>,
    ) -> Result<bool, RuntimePlanTypeResolutionError> {
        if let Some(checked) = memo.get(&id) {
            return Ok(*checked);
        }
        let declaration = self
            .get(id)
            .ok_or(RuntimePlanTypeResolutionError::UnknownType { ty: id })?;
        let checked = match &declaration.projection {
            RuntimePlanTypeProjection::Never
            | RuntimePlanTypeProjection::Unit
            | RuntimePlanTypeProjection::Bool
            | RuntimePlanTypeProjection::Signed(_)
            | RuntimePlanTypeProjection::Unsigned(_)
            | RuntimePlanTypeProjection::F32
            | RuntimePlanTypeProjection::F64
            | RuntimePlanTypeProjection::String
            | RuntimePlanTypeProjection::Char
            | RuntimePlanTypeProjection::Bytes
            | RuntimePlanTypeProjection::Duration
            | RuntimePlanTypeProjection::Progress
            | RuntimePlanTypeProjection::EntityReference
            | RuntimePlanTypeProjection::AgentValue
            | RuntimePlanTypeProjection::ProjectNominal { .. }
            | RuntimePlanTypeProjection::Opaque { .. } => true,
            RuntimePlanTypeProjection::Sequence { item, .. }
            | RuntimePlanTypeProjection::Array { item, .. }
            | RuntimePlanTypeProjection::Option(item) => self.is_checked_memoized(*item, memo)?,
            RuntimePlanTypeProjection::Tuple(items) | RuntimePlanTypeProjection::Choice(items) => {
                let mut all_checked = true;
                for child in items {
                    all_checked &= self.is_checked_memoized(*child, memo)?;
                }
                all_checked
            }
            RuntimePlanTypeProjection::BuiltinVariant { cases, .. } => {
                let mut all_checked = true;
                for child in cases.iter().flatten() {
                    all_checked &= self.is_checked_memoized(*child, memo)?;
                }
                all_checked
            }
            RuntimePlanTypeProjection::Result { value, error } => {
                self.is_checked_memoized(*value, memo)? && self.is_checked_memoized(*error, memo)?
            }
            RuntimePlanTypeProjection::Agent(agent) => {
                !matches!(agent, RuntimeAgentTypeProjection::Probe(_))
            }
            RuntimePlanTypeProjection::Range(_)
            | RuntimePlanTypeProjection::Iterator(_)
            | RuntimePlanTypeProjection::Map { .. }
            | RuntimePlanTypeProjection::Need(_)
            | RuntimePlanTypeProjection::Stream { .. }
            | RuntimePlanTypeProjection::ThreadHandle(_)
            | RuntimePlanTypeProjection::Shared(_)
            | RuntimePlanTypeProjection::Reference(_)
            | RuntimePlanTypeProjection::Function { .. } => false,
        };
        memo.insert(id, checked);
        Ok(checked)
    }
}

/// Failure to derive a checked or operational view from a sealed table.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePlanTypeResolutionError {
    #[error("runtime plan type {ty} does not belong to the table")]
    UnknownType { ty: RuntimePlanTypeId },
    #[error("runtime plan type {ty} has neither a checked nor operational class")]
    MissingRuntimeClass { ty: RuntimePlanTypeId },
    #[error("checked type projection contains a recursive nominal cycle at {ty}")]
    CheckedProjectionCycle { ty: RuntimePlanTypeId },
}

/// Sole internal issuer for one plan's semantic type declaration identities.
#[derive(Debug)]
pub(crate) struct RuntimePlanTypeTableBuilder {
    rows: Vec<RuntimePlanTypeDeclaration>,
    by_semantic_identity: BTreeMap<RuntimeSemanticTypeId, RuntimePlanTypeId>,
    maximum: u32,
}

pub(crate) struct PreparedRuntimePlanTypeBatch {
    result_ids: Box<[RuntimePlanTypeId]>,
    candidate_ids: BTreeMap<RuntimeSemanticTypeId, RuntimePlanTypeId>,
    candidate_rows: Vec<RuntimePlanTypeDeclaration>,
}

impl PreparedRuntimePlanTypeBatch {
    pub(crate) fn id_for_semantic(
        &self,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Option<RuntimePlanTypeId> {
        self.candidate_ids.get(&semantic_identity).copied()
    }

    pub(crate) fn get(&self, id: RuntimePlanTypeId) -> Option<&RuntimePlanTypeDeclaration> {
        declaration_index(id).and_then(|index| self.candidate_rows.get(index))
    }
}

/// Failure to admit one atomic semantic type graph batch.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePlanTypeTableError {
    #[error("semantic type {semantic_identity:?} has conflicting projections")]
    ConflictingProjection {
        semantic_identity: RuntimeSemanticTypeId,
    },
    #[error("semantic type {owner:?} references undeclared semantic type {referenced:?}")]
    DanglingReference {
        owner: RuntimeSemanticTypeId,
        referenced: RuntimeSemanticTypeId,
    },
    #[error("runtime plan type graph contains a cycle at semantic type {semantic_identity:?}")]
    Cycle {
        semantic_identity: RuntimeSemanticTypeId,
    },
    #[error(
        "runtime plan type graph exceeds the maximum depth of {maximum} at semantic type {semantic_identity:?}"
    )]
    NestingTooDeep {
        semantic_identity: RuntimeSemanticTypeId,
        maximum: usize,
    },
    #[error("runtime plan type identity space is exhausted")]
    IdentityExhausted,
    #[error("semantic type {semantic_identity:?} has a non-canonical builtin variant schema")]
    InvalidBuiltinVariantSchema {
        semantic_identity: RuntimeSemanticTypeId,
    },
}

impl RuntimePlanTypeTableBuilder {
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            rows: Vec::new(),
            by_semantic_identity: BTreeMap::new(),
            maximum: u32::MAX,
        }
    }

    pub(crate) fn get(&self, id: RuntimePlanTypeId) -> Option<&RuntimePlanTypeDeclaration> {
        declaration_index(id).and_then(|index| self.rows.get(index))
    }

    #[must_use]
    pub(crate) fn id_for_semantic(
        &self,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Option<RuntimePlanTypeId> {
        self.by_semantic_identity.get(&semantic_identity).copied()
    }

    /// Returns the existing ID for an exact duplicate or issues the next
    /// contiguous ID for a complete single-node graph.
    #[cfg(test)]
    pub(crate) fn intern(
        &mut self,
        seed: RuntimePlanTypeSeed,
    ) -> Result<RuntimePlanTypeId, RuntimePlanTypeTableError> {
        self.intern_batch([seed])?
            .first()
            .copied()
            .ok_or(RuntimePlanTypeTableError::IdentityExhausted)
    }

    /// Atomically admits semantic projections in supplied canonical pre-order.
    ///
    /// Existing and intra-batch conflicts, the complete capacity requirement,
    /// all child references, cycles, and maximum depth are checked before the
    /// first new declaration becomes visible.
    #[cfg(test)]
    pub(crate) fn intern_batch(
        &mut self,
        seeds: impl IntoIterator<Item = RuntimePlanTypeSeed>,
    ) -> Result<Box<[RuntimePlanTypeId]>, RuntimePlanTypeTableError> {
        let prepared = self.prepare_batch(seeds)?;
        Ok(self.commit_batch(prepared))
    }

    pub(crate) fn prepare_batch(
        &self,
        seeds: impl IntoIterator<Item = RuntimePlanTypeSeed>,
    ) -> Result<PreparedRuntimePlanTypeBatch, RuntimePlanTypeTableError> {
        let seeds = seeds.into_iter().collect::<Box<[_]>>();
        let mut unique = Vec::<RuntimePlanTypeSeed>::new();
        let mut unique_indices = BTreeMap::<RuntimeSemanticTypeId, usize>::new();

        for seed in &seeds {
            if let Some(index) = unique_indices.get(&seed.semantic_identity).copied() {
                if unique[index].projection != seed.projection {
                    return Err(RuntimePlanTypeTableError::ConflictingProjection {
                        semantic_identity: seed.semantic_identity,
                    });
                }
            } else {
                unique_indices.insert(seed.semantic_identity, unique.len());
                unique.push(seed.clone());
            }
        }

        let new_count = unique
            .iter()
            .filter(|seed| {
                !self
                    .by_semantic_identity
                    .contains_key(&seed.semantic_identity)
            })
            .count();
        let final_len = self
            .rows
            .len()
            .checked_add(new_count)
            .and_then(|len| u32::try_from(len).ok())
            .filter(|len| *len <= self.maximum)
            .ok_or(RuntimePlanTypeTableError::IdentityExhausted)?;
        let _ = final_len;

        let mut candidate_ids = self.by_semantic_identity.clone();
        let mut next_index = self.rows.len();
        for seed in &unique {
            if candidate_ids.contains_key(&seed.semantic_identity) {
                continue;
            }
            let id = plan_type_id_for_index(next_index)?;
            candidate_ids.insert(seed.semantic_identity, id);
            next_index = next_index
                .checked_add(1)
                .ok_or(RuntimePlanTypeTableError::IdentityExhausted)?;
        }

        let mut staged = Vec::with_capacity(new_count);
        for seed in unique {
            let projection = rewrite_projection(&seed, &candidate_ids)?;
            let declaration = RuntimePlanTypeDeclaration {
                semantic_identity: seed.semantic_identity,
                projection,
            };
            if let Some(existing_id) = self
                .by_semantic_identity
                .get(&seed.semantic_identity)
                .copied()
            {
                let Some(existing) = self.get(existing_id) else {
                    return Err(RuntimePlanTypeTableError::DanglingReference {
                        owner: seed.semantic_identity,
                        referenced: seed.semantic_identity,
                    });
                };
                if existing != &declaration {
                    return Err(RuntimePlanTypeTableError::ConflictingProjection {
                        semantic_identity: seed.semantic_identity,
                    });
                }
            } else {
                staged.push(declaration);
            }
        }

        let mut candidate_rows = self.rows.clone();
        candidate_rows.extend(staged.iter().cloned());
        validate_candidate_graph(&candidate_rows)?;

        let result_ids = seeds
            .iter()
            .map(|seed| {
                candidate_ids.get(&seed.semantic_identity).copied().ok_or(
                    RuntimePlanTypeTableError::DanglingReference {
                        owner: seed.semantic_identity,
                        referenced: seed.semantic_identity,
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)?;
        Ok(PreparedRuntimePlanTypeBatch {
            result_ids,
            candidate_ids,
            candidate_rows,
        })
    }

    pub(crate) fn commit_batch(
        &mut self,
        prepared: PreparedRuntimePlanTypeBatch,
    ) -> Box<[RuntimePlanTypeId]> {
        self.rows = prepared.candidate_rows;
        self.by_semantic_identity = prepared.candidate_ids;
        prepared.result_ids
    }

    pub(crate) fn finish(self) -> Result<RuntimePlanTypeTable, RuntimePlanTypeTableError> {
        let rows = self
            .rows
            .into_iter()
            .enumerate()
            .map(|(index, declaration)| {
                Ok(RuntimePlanTypeTableRow {
                    id: plan_type_id_for_index(index)?,
                    declaration,
                })
            })
            .collect::<Result<Vec<_>, RuntimePlanTypeTableError>>()?
            .into_boxed_slice();
        Ok(RuntimePlanTypeTable { rows })
    }

    #[cfg(test)]
    fn with_maximum_for_test(maximum: u32) -> Self {
        Self {
            maximum,
            ..Self::new()
        }
    }
}

fn rewrite_projection(
    seed: &RuntimePlanTypeSeed,
    ids: &BTreeMap<RuntimeSemanticTypeId, RuntimePlanTypeId>,
) -> Result<RuntimePlanTypeProjection<RuntimePlanTypeId>, RuntimePlanTypeTableError> {
    seed.projection.clone().try_map(|referenced| {
        ids.get(&referenced)
            .copied()
            .ok_or(RuntimePlanTypeTableError::DanglingReference {
                owner: seed.semantic_identity,
                referenced,
            })
    })
}

fn validate_candidate_graph(
    rows: &[RuntimePlanTypeDeclaration],
) -> Result<(), RuntimePlanTypeTableError> {
    let mut edges = vec![Vec::<usize>::new(); rows.len()];
    let mut incoming = vec![0_usize; rows.len()];
    for (owner, row) in rows.iter().enumerate() {
        if let RuntimePlanTypeProjection::BuiltinVariant { owner, cases } = &row.projection
            && (cases.len() != owner.cases().len()
                || cases
                    .iter()
                    .map(Option::is_some)
                    .ne(owner.cases().iter().map(|case| case.has_payload())))
        {
            return Err(RuntimePlanTypeTableError::InvalidBuiltinVariantSchema {
                semantic_identity: row.semantic_identity,
            });
        }
        for child_id in row.projection.children() {
            let child = declaration_index(*child_id).ok_or(
                RuntimePlanTypeTableError::DanglingReference {
                    owner: row.semantic_identity,
                    referenced: row.semantic_identity,
                },
            )?;
            if rows.get(child).is_none() {
                return Err(RuntimePlanTypeTableError::DanglingReference {
                    owner: row.semantic_identity,
                    referenced: row.semantic_identity,
                });
            }
            edges[owner].push(child);
            incoming[child] = incoming[child]
                .checked_add(1)
                .ok_or(RuntimePlanTypeTableError::IdentityExhausted)?;
        }
    }

    let mut queue = incoming
        .iter()
        .enumerate()
        .filter_map(|(index, incoming)| (*incoming == 0).then_some(index))
        .collect::<VecDeque<_>>();
    let mut longest_depth = vec![1_usize; rows.len()];
    let mut visited = 0_usize;
    while let Some(owner) = queue.pop_front() {
        visited += 1;
        for child in &edges[owner] {
            let child_depth = longest_depth[owner].checked_add(1).ok_or(
                RuntimePlanTypeTableError::NestingTooDeep {
                    semantic_identity: rows[*child].semantic_identity,
                    maximum: MAX_RUNTIME_PLAN_TYPE_DEPTH,
                },
            )?;
            if child_depth > MAX_RUNTIME_PLAN_TYPE_DEPTH {
                return Err(RuntimePlanTypeTableError::NestingTooDeep {
                    semantic_identity: rows[*child].semantic_identity,
                    maximum: MAX_RUNTIME_PLAN_TYPE_DEPTH,
                });
            }
            longest_depth[*child] = longest_depth[*child].max(child_depth);
            incoming[*child] -= 1;
            if incoming[*child] == 0 {
                queue.push_back(*child);
            }
        }
    }
    if visited != rows.len() {
        let Some(semantic_identity) = incoming
            .iter()
            .position(|incoming| *incoming != 0)
            .and_then(|index| rows.get(index))
            .map(|row| row.semantic_identity)
        else {
            return Err(RuntimePlanTypeTableError::IdentityExhausted);
        };
        return Err(RuntimePlanTypeTableError::Cycle { semantic_identity });
    }
    Ok(())
}

fn declaration_index(id: RuntimePlanTypeId) -> Option<usize> {
    usize::try_from(id.get().get() - 1).ok()
}

fn plan_type_id_for_index(index: usize) -> Result<RuntimePlanTypeId, RuntimePlanTypeTableError> {
    index
        .checked_add(1)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .and_then(NonZeroU32::new)
        .map(RuntimePlanTypeId::from_accepted_ordinal)
        .ok_or(RuntimePlanTypeTableError::IdentityExhausted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{RuntimeOperationalType, RuntimePlanSequenceKind, RuntimePlanTypeProjection};

    fn identity(marker: u8) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes([marker; 32])
    }

    fn leaf(
        marker: u8,
        projection: RuntimePlanTypeProjection<RuntimeSemanticTypeId>,
    ) -> RuntimePlanTypeSeed {
        RuntimePlanTypeSeed::new(identity(marker), projection)
    }

    #[test]
    fn batch_rewrites_semantic_edges_and_derives_checked_view() {
        let root = leaf(
            1,
            RuntimePlanTypeProjection::Sequence {
                kind: RuntimePlanSequenceKind::Vec,
                item: identity(2),
            },
        );
        let item = leaf(2, RuntimePlanTypeProjection::Bool);
        let mut builder = RuntimePlanTypeTableBuilder::new();
        let ids = builder
            .intern_batch([root, item])
            .expect("valid semantic graph");
        let table = builder.finish().expect("valid semantic graph");

        assert_eq!(ids.len(), 2);
        assert_eq!(table.class(ids[0]), Ok(RuntimePlanTypeClass::Checked));
    }

    #[test]
    fn conflict_dangling_cycle_and_capacity_fail_without_mutation() {
        let retained = leaf(3, RuntimePlanTypeProjection::Unit);
        let mut builder = RuntimePlanTypeTableBuilder::with_maximum_for_test(3);
        let retained_id = builder.intern(retained.clone()).expect("retained row");

        let conflict = leaf(3, RuntimePlanTypeProjection::String);
        assert!(matches!(
            builder.intern_batch([leaf(4, RuntimePlanTypeProjection::Bool), conflict]),
            Err(RuntimePlanTypeTableError::ConflictingProjection { .. })
        ));
        assert!(matches!(
            builder.intern(leaf(5, RuntimePlanTypeProjection::Option(identity(9)))),
            Err(RuntimePlanTypeTableError::DanglingReference { .. })
        ));
        assert!(matches!(
            builder.intern_batch([
                leaf(6, RuntimePlanTypeProjection::Option(identity(7))),
                leaf(7, RuntimePlanTypeProjection::Option(identity(6))),
            ]),
            Err(RuntimePlanTypeTableError::Cycle { .. })
        ));
        assert!(matches!(
            builder.intern_batch([
                leaf(8, RuntimePlanTypeProjection::Bool),
                leaf(9, RuntimePlanTypeProjection::String),
                leaf(10, RuntimePlanTypeProjection::Unit),
            ]),
            Err(RuntimePlanTypeTableError::IdentityExhausted)
        ));

        let next = builder
            .intern(leaf(11, RuntimePlanTypeProjection::Bool))
            .expect("failed batches committed nothing");
        assert_eq!(retained_id.get(), NonZeroU32::MIN);
        assert_eq!(next.get(), NonZeroU32::new(2).unwrap());
    }

    #[test]
    fn operational_class_is_derived_from_projection() {
        let mut builder = RuntimePlanTypeTableBuilder::new();
        let ids = builder
            .intern_batch([
                leaf(11, RuntimePlanTypeProjection::Range(identity(12))),
                leaf(12, RuntimePlanTypeProjection::Unit),
            ])
            .expect("range graph");
        let table = builder.finish().expect("range graph");

        assert_eq!(
            table.class(ids[0]),
            Ok(RuntimePlanTypeClass::Operational(
                RuntimeOperationalType::Range
            ))
        );
    }

    #[test]
    fn overdeep_shared_graph_is_rejected_before_any_row_commits() {
        let mut seeds = Vec::new();
        for index in 0..=MAX_RUNTIME_PLAN_TYPE_DEPTH {
            let marker = u8::try_from(index + 20).expect("test marker fits");
            let projection = if index == MAX_RUNTIME_PLAN_TYPE_DEPTH {
                RuntimePlanTypeProjection::Unit
            } else {
                let child = u8::try_from(index + 21).expect("test child marker fits");
                RuntimePlanTypeProjection::Option(identity(child))
            };
            seeds.push(leaf(marker, projection));
        }
        let mut builder = RuntimePlanTypeTableBuilder::new();
        assert!(matches!(
            builder.intern_batch(seeds),
            Err(RuntimePlanTypeTableError::NestingTooDeep { .. })
        ));

        let first = builder
            .intern(leaf(90, RuntimePlanTypeProjection::Unit))
            .expect("overdeep batch committed nothing");
        assert_eq!(first.get(), NonZeroU32::MIN);
    }
}
