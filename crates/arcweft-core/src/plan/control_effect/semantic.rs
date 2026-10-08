//! One C graph pass for construction and borrowed task-image preparation.
//! Definitions stay on their actual owner; completed table caches are never
//! inputs. The common task sealer supplies its source-order C roots and meter.

use super::{
    CONTROL_EFFECT_DOMAIN, ControlEffectContractDigest, RuntimeControlEffectContract,
    RuntimeControlEffectContractError, RuntimeControlEffectContractId,
    RuntimeControlEffectContractTable, RuntimePlanTypeTable, RuntimeTaskPlanSealLimits,
};
use crate::task::semantic::TaskSemanticMeter;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
enum Rows<'a> {
    Construction(&'a [RuntimeControlEffectContract]),
    Admitted(&'a RuntimeControlEffectContractTable),
}
impl<'a> Rows<'a> {
    fn len(self) -> usize {
        match self {
            Self::Construction(rows) => rows.len(),
            Self::Admitted(table) => table.len(),
        }
    }
    fn get(self, index: usize) -> Option<&'a RuntimeControlEffectContract> {
        match self {
            Self::Construction(rows) => rows.get(index),
            Self::Admitted(table) => table.rows.get(index).map(|row| &row.contract),
        }
    }
}

#[derive(Clone, Copy)]
enum State {
    Visiting,
    Done(ControlEffectContractDigest),
}
enum Visit {
    Enter(usize),
    Children { index: usize, next: usize },
}

/// Sparse source-coordinate memo, confined to the preflighted reachable graph.
/// It borrows row definitions and uses the caller's one sticky meter.
pub(crate) struct RuntimeControlEffectSemanticPass<'a> {
    rows: Rows<'a>,
    types: &'a RuntimePlanTypeTable,
    admitted: BTreeSet<usize>,
    state: BTreeMap<usize, State>,
}

/// Count-only owner before common preflight. It has no completion method.
/// Success consumes it into the sole C semantic pass without copying rows.
pub(crate) struct RuntimeControlEffectPreflight<'a> {
    pass: RuntimeControlEffectSemanticPass<'a>,
}

impl<'a> RuntimeControlEffectPreflight<'a> {
    /// Count-only preparation for the common image. Checks are called in the
    /// global field order; no C proof or completed-table cache is read here.
    pub(crate) fn new(
        table: &'a RuntimeControlEffectContractTable,
        types: &'a RuntimePlanTypeTable,
        roots: impl IntoIterator<Item = RuntimeControlEffectContractId>,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, RuntimeControlEffectContractError> {
        meter.status()?;
        meter.checked_count_sum(table.len(), 0)?;
        let rows = Rows::Admitted(table);
        let admitted = RuntimeControlEffectSemanticPass::reachable(
            rows,
            roots.into_iter().map(RuntimeControlEffectContractId::index),
        );
        Ok(Self {
            pass: RuntimeControlEffectSemanticPass {
                rows,
                types,
                admitted,
                state: BTreeMap::new(),
            },
        })
    }

    pub(crate) fn check_children(
        &self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), RuntimeControlEffectContractError> {
        meter.status()?;
        RuntimeControlEffectSemanticPass::preflight_children(
            self.pass.rows,
            &self.pass.admitted,
            limits,
        )
        .inspect_err(|_| meter.reject_owner())
    }

    pub(crate) fn check_effects(
        &self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), RuntimeControlEffectContractError> {
        meter.status()?;
        RuntimeControlEffectSemanticPass::preflight_effects(
            self.pass.rows,
            &self.pass.admitted,
            limits,
        )
        .inspect_err(|_| meter.reject_owner())
    }

    pub(crate) fn known_transcript_bytes(
        &self,
        meter: &mut TaskSemanticMeter,
    ) -> Result<u64, RuntimeControlEffectContractError> {
        meter.status()?;
        RuntimeControlEffectSemanticPass::known_bytes(self.pass.rows, &self.pass.admitted)
            .inspect_err(|_| meter.reject_owner())
    }

    pub(crate) fn finish(
        self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeControlEffectSemanticPass<'a>, RuntimeControlEffectContractError> {
        RuntimeControlEffectSemanticPass::preflight(
            self.pass.rows,
            &self.pass.admitted,
            limits,
            meter,
        )
        .inspect_err(|_| meter.reject_owner())?;
        Ok(self.pass)
    }
}

impl<'a> RuntimeControlEffectSemanticPass<'a> {
    pub(super) fn from_rows(
        rows: &'a [RuntimeControlEffectContract],
        types: &'a RuntimePlanTypeTable,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, RuntimeControlEffectContractError> {
        Self::new(
            Rows::Construction(rows),
            types,
            0..rows.len(),
            limits,
            meter,
        )
    }
    pub(crate) fn from_table(
        table: &'a RuntimeControlEffectContractTable,
        types: &'a RuntimePlanTypeTable,
        roots: &[RuntimeControlEffectContractId],
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, RuntimeControlEffectContractError> {
        Self::new(
            Rows::Admitted(table),
            types,
            roots.iter().map(|id| id.index()),
            limits,
            meter,
        )
    }
    fn new(
        rows: Rows<'a>,
        types: &'a RuntimePlanTypeTable,
        roots: impl IntoIterator<Item = usize>,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, RuntimeControlEffectContractError> {
        meter.status()?;
        meter.checked_count_sum(rows.len(), 0)?;
        let admitted = Self::reachable(rows, roots);
        Self::preflight(rows, &admitted, limits, meter).inspect_err(|_| meter.reject_owner())?;
        Ok(Self {
            rows,
            types,
            admitted,
            state: BTreeMap::new(),
        })
    }
    /// Counting does not complete semantic children or reject bad references
    /// or cycles ahead of the owned limit fields. Missing definitions add no
    /// rows; completion still rejects their exact source-order reference.
    fn reachable(rows: Rows<'_>, roots: impl IntoIterator<Item = usize>) -> BTreeSet<usize> {
        let mut visited = BTreeSet::new();
        for root in roots {
            let mut stack = vec![Visit::Enter(root)];
            while let Some(visit) = stack.pop() {
                match visit {
                    Visit::Enter(index) => {
                        if rows.get(index).is_none() {
                            continue;
                        }
                        if visited.insert(index) {
                            stack.push(Visit::Children { index, next: 0 });
                        }
                    }
                    Visit::Children { index, next } => {
                        let row = rows.get(index).expect("resolved reachability row");
                        if let Some(child) = row.children().get(next) {
                            stack.push(Visit::Children {
                                index,
                                next: next + 1,
                            });
                            stack.push(Visit::Enter(child.index()));
                        }
                    }
                }
            }
        }
        visited
    }
    pub(crate) fn complete(
        &mut self,
        id: RuntimeControlEffectContractId,
        meter: &mut TaskSemanticMeter,
    ) -> Result<ControlEffectContractDigest, RuntimeControlEffectContractError> {
        meter.status()?;
        if !self.admitted.contains(&id.index()) {
            meter.reject_owner();
            return Err(RuntimeControlEffectContractError::UnknownChild { index: id.index() });
        }
        self.complete_root(id.index(), meter)
            .inspect_err(|_| meter.reject_owner())
    }
    fn complete_root(
        &mut self,
        root: usize,
        meter: &mut TaskSemanticMeter,
    ) -> Result<ControlEffectContractDigest, RuntimeControlEffectContractError> {
        let mut stack = vec![Visit::Enter(root)];
        while let Some(visit) = stack.pop() {
            meter.status()?;
            match visit {
                Visit::Enter(index) => match self.state.get(&index) {
                    Some(State::Done(_)) => {}
                    Some(State::Visiting) => {
                        return Err(RuntimeControlEffectContractError::Cycle { index });
                    }
                    None => {
                        if self.rows.get(index).is_none() {
                            return Err(RuntimeControlEffectContractError::UnknownChild { index });
                        }
                        meter.charge_work(1)?;
                        self.state.insert(index, State::Visiting);
                        stack.push(Visit::Children { index, next: 0 });
                    }
                },
                Visit::Children { index, next } => {
                    let row = self
                        .rows
                        .get(index)
                        .expect("preflighted reachable contract");
                    if let Some(child) = row.children().get(next) {
                        meter.charge_work(1)?; // enter the next source-order child
                        stack.push(Visit::Children {
                            index,
                            next: next + 1,
                        });
                        stack.push(Visit::Enter(child.index()));
                    } else {
                        let digest = row.semantic_digest(
                            self.types,
                            |child| match self.state.get(&child.index()) {
                                Some(State::Done(digest)) => Some(*digest),
                                Some(State::Visiting) | None => None,
                            },
                            meter,
                        )?;
                        self.state.insert(index, State::Done(digest));
                    }
                }
            }
        }
        match self.state.get(&root) {
            Some(State::Done(digest)) => Ok(*digest),
            Some(State::Visiting) | None => Err(RuntimeControlEffectContractError::UnsealedChild),
        }
    }

    /// Counts only the admitted union, in canonical source coordinate order.
    /// Child counts precede effect counts and the exact known byte lower bound.
    fn preflight(
        rows: Rows<'_>,
        admitted: &BTreeSet<usize>,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), RuntimeControlEffectContractError> {
        Self::preflight_children(rows, admitted, limits)?;
        Self::preflight_effects(rows, admitted, limits)?;
        meter.preflight_bytes(Self::known_bytes(rows, admitted)?)?;
        Ok(())
    }

    fn preflight_children(
        rows: Rows<'_>,
        admitted: &BTreeSet<usize>,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<(), RuntimeControlEffectContractError> {
        let row = |index| rows.get(index).expect("reachable rows were resolved");
        for &index in admitted {
            let actual = u32::try_from(row(index).children().len())
                .map_err(|_| RuntimeControlEffectContractError::ArithmeticOverflow)?;
            if actual > limits.max_children_per_row {
                return Err(RuntimeControlEffectContractError::ChildrenLimit {
                    index,
                    actual,
                    maximum: limits.max_children_per_row,
                });
            }
        }
        Ok(())
    }

    fn preflight_effects(
        rows: Rows<'_>,
        admitted: &BTreeSet<usize>,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<(), RuntimeControlEffectContractError> {
        let row = |index| rows.get(index).expect("reachable rows were resolved");
        let mut effect_rows = 0_u32;
        for &index in admitted {
            let count = u32::try_from(row(index).effects().len())
                .map_err(|_| RuntimeControlEffectContractError::ArithmeticOverflow)?;
            effect_rows = effect_rows
                .checked_add(count)
                .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
        }
        if effect_rows > limits.max_control_effect_rows {
            return Err(RuntimeControlEffectContractError::EffectRowsLimit {
                actual: effect_rows,
                maximum: limits.max_control_effect_rows,
            });
        }
        Ok(())
    }

    fn known_bytes(
        rows: Rows<'_>,
        admitted: &BTreeSet<usize>,
    ) -> Result<u64, RuntimeControlEffectContractError> {
        let row = |index| rows.get(index).expect("reachable rows were resolved");
        let mut bytes = 0_u64;
        for &index in admitted {
            let row = row(index);
            let fixed = u64::try_from(CONTROL_EFFECT_DOMAIN.len())
                .ok()
                .and_then(|count| count.checked_add(9))
                .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
            bytes = bytes
                .checked_add(fixed)
                .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
            for effect in row.effects() {
                let inputs = u32::try_from(effect.inputs.len())
                    .map_err(|_| RuntimeControlEffectContractError::ArithmeticOverflow)?;
                let payloads = u64::from(inputs)
                    .checked_add(u64::from(effect.identity.is_some()))
                    .and_then(|count| count.checked_add(u64::from(effect.output.is_some())))
                    .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
                let row_bytes = payloads
                    .checked_mul(32)
                    .and_then(|count| count.checked_add(15))
                    .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
                bytes = bytes
                    .checked_add(row_bytes)
                    .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
            }
            let children = u64::try_from(row.children().len())
                .ok()
                .and_then(|count| count.checked_mul(36))
                .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
            bytes = bytes
                .checked_add(children)
                .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests;
