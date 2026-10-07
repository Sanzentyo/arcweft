//! Source-ordered Flow definitions and invocation schemas with one immutable
//! lookup authority. Dynamic target labels are operational selector bindings.

use super::{FlowRuntimeId, RuntimeFlow, RuntimeFlowSchema, RuntimeFlowTargetError};
use crate::runtime_id::RuntimePublicLabel;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Positions {
    first: usize,
    count: usize,
}

impl Positions {
    fn insert<K: Ord>(index: &mut BTreeMap<K, Self>, key: K, ordinal: usize) {
        index
            .entry(key)
            .and_modify(|position| position.count += 1)
            .or_insert(Self {
                first: ordinal,
                count: 1,
            });
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RuntimeFlowTable {
    rows: Vec<RuntimeFlow>,
    schemas: Vec<RuntimeFlowSchema>,
    by_identity: BTreeMap<FlowRuntimeId, Positions>,
    schema_by_identity: BTreeMap<FlowRuntimeId, Positions>,
    by_selector: BTreeMap<RuntimePublicLabel, Positions>,
}

impl RuntimeFlowTable {
    pub(super) fn from_rows(rows: Vec<RuntimeFlow>, schemas: Vec<RuntimeFlowSchema>) -> Self {
        let mut by_identity = BTreeMap::new();
        let mut schema_by_identity = BTreeMap::new();
        let mut by_selector = BTreeMap::new();
        for (ordinal, row) in rows.iter().enumerate() {
            Positions::insert(&mut by_identity, row.id.clone(), ordinal);
            Positions::insert(&mut by_selector, row.id.public_label(), ordinal);
        }
        for (ordinal, schema) in schemas.iter().enumerate() {
            Positions::insert(&mut schema_by_identity, schema.flow.clone(), ordinal);
        }
        Self {
            rows,
            schemas,
            by_identity,
            schema_by_identity,
            by_selector,
        }
    }

    pub(super) fn as_slice(&self) -> &[RuntimeFlow] {
        &self.rows
    }
    pub(super) fn schemas(&self) -> &[RuntimeFlowSchema] {
        &self.schemas
    }

    pub(crate) fn position(&self, identity: &FlowRuntimeId) -> Option<usize> {
        self.by_identity
            .get(identity)
            .filter(|position| position.count == 1)
            .map(|position| position.first)
    }

    pub(crate) fn flow(&self, identity: &FlowRuntimeId) -> Option<&RuntimeFlow> {
        self.position(identity).map(|ordinal| &self.rows[ordinal])
    }

    pub(crate) fn schema(&self, identity: &FlowRuntimeId) -> Option<&RuntimeFlowSchema> {
        self.schema_by_identity
            .get(identity)
            .filter(|position| position.count == 1)
            .map(|position| &self.schemas[position.first])
    }

    pub(crate) fn resolve_target(
        &self,
        value: &str,
    ) -> Result<FlowRuntimeId, RuntimeFlowTargetError> {
        let projected = FlowRuntimeId::from_runtime_target_value(value)?;
        // Match the accepted exact identity before considering its public
        // selector, preserving the established dynamic target precedence.
        if let Some(position) = self.by_identity.get(&projected) {
            return Ok(self.rows[position.first].id.clone());
        }
        match self.by_selector.get(projected.public_label_ref()) {
            Some(position) if position.count == 1 => Ok(self.rows[position.first].id.clone()),
            Some(position) => Err(RuntimeFlowTargetError::Ambiguous {
                target: value.to_owned(),
                matches: position.count,
            }),
            None => Err(RuntimeFlowTargetError::Missing {
                target: value.to_owned(),
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn into_parts(self) -> (Vec<RuntimeFlow>, Vec<RuntimeFlowSchema>) {
        (self.rows, self.schemas)
    }
}

impl std::ops::Deref for RuntimeFlowTable {
    type Target = [RuntimeFlow];
    fn deref(&self) -> &Self::Target {
        &self.rows
    }
}

impl<'a> IntoIterator for &'a RuntimeFlowTable {
    type Item = &'a RuntimeFlow;
    type IntoIter = std::slice::Iter<'a, RuntimeFlow>;
    fn into_iter(self) -> Self::IntoIter {
        self.rows.iter()
    }
}

#[cfg(test)]
mod tests;
