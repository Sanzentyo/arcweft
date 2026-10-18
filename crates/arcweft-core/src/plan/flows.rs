//! Source-ordered Flow definitions and invocation schemas with one immutable
//! lookup authority. Dynamic target labels are operational selector bindings.

use super::{FlowRuntimeId, RuntimeFlow, RuntimeFlowSchema, RuntimeFlowTargetError};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Positions {
    first: usize,
    count: usize,
}

impl Positions {
    fn find<'a, K: Ord + 'a>(
        index: &[usize],
        target: &K,
        key: impl Fn(usize) -> &'a K,
    ) -> Option<Self> {
        let start = index.partition_point(|&ordinal| key(ordinal) < target);
        let end = index.partition_point(|&ordinal| key(ordinal) <= target);
        (start < end).then(|| Self {
            first: index[start],
            count: end - start,
        })
    }
    fn index<'a, K: Ord + 'a>(count: usize, key: impl Fn(usize) -> &'a K) -> Vec<usize> {
        let mut index = (0..count).collect::<Vec<_>>();
        // The source ordinal breaks ties, retaining the established first-row
        // precedence even when structural admission will reject duplicates.
        index.sort_unstable_by(|&left, &right| key(left).cmp(key(right)).then(left.cmp(&right)));
        index
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RuntimeFlowTable {
    rows: Vec<RuntimeFlow>,
    schemas: Vec<RuntimeFlowSchema>,
    by_identity: Vec<usize>,
    schema_by_identity: Vec<usize>,
    by_selector: Vec<usize>,
}

impl RuntimeFlowTable {
    pub(super) fn from_rows(rows: Vec<RuntimeFlow>, schemas: Vec<RuntimeFlowSchema>) -> Self {
        let by_identity = Positions::index(rows.len(), |ordinal| &rows[ordinal].id);
        let schema_by_identity = Positions::index(schemas.len(), |ordinal| &schemas[ordinal].flow);
        let by_selector =
            Positions::index(rows.len(), |ordinal| rows[ordinal].id.public_label_ref());
        Self {
            rows,
            schemas,
            by_identity,
            schema_by_identity,
            by_selector,
        }
    }

    /// Exact index buffers, without copied identity/selector payloads or an
    /// opaque tree-allocation estimate. Sorting uses no additional heap buffer.
    pub(in crate::plan) fn source_index_storage_bytes(
        rows: usize,
        schemas: usize,
    ) -> Option<usize> {
        rows.checked_mul(2)?
            .checked_add(schemas)?
            .checked_mul(size_of::<usize>())
    }

    pub(super) fn as_slice(&self) -> &[RuntimeFlow] {
        &self.rows
    }
    pub(super) fn schemas(&self) -> &[RuntimeFlowSchema] {
        &self.schemas
    }

    pub(crate) fn position(&self, identity: &FlowRuntimeId) -> Option<usize> {
        Positions::find(&self.by_identity, identity, |ordinal| {
            &self.rows[ordinal].id
        })
        .filter(|position| position.count == 1)
        .map(|position| position.first)
    }

    pub(crate) fn flow(&self, identity: &FlowRuntimeId) -> Option<&RuntimeFlow> {
        self.position(identity).map(|ordinal| &self.rows[ordinal])
    }

    pub(crate) fn schema(&self, identity: &FlowRuntimeId) -> Option<&RuntimeFlowSchema> {
        Positions::find(&self.schema_by_identity, identity, |ordinal| {
            &self.schemas[ordinal].flow
        })
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
        if let Some(position) = Positions::find(&self.by_identity, &projected, |ordinal| {
            &self.rows[ordinal].id
        }) {
            return Ok(self.rows[position.first].id.clone());
        }
        match Positions::find(&self.by_selector, projected.public_label_ref(), |ordinal| {
            self.rows[ordinal].id.public_label_ref()
        }) {
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
