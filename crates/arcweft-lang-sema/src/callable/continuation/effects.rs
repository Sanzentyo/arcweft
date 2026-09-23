//! Graph-owned inference prerequisites for prepared function-value results.

use super::{
    Arc, BTreeMap, BTreeSet, CallConstraintInvariant, CheckedCallSite, PreparedCallGraphIssuer,
};
use crate::{
    callable::{
        CallableCandidateId, CallableGroupIndex, CallableResultSchema,
        CallableSignatureSchemaDigest, CheckedCallableId, PreparedResolvedCallable,
    },
    effect_row::EffectRow,
};

/// A projection request belongs to one live graph generation and is never
/// reconstructed from a declaration name or a function-shaped placeholder.
#[derive(Clone)]
pub(crate) struct PreparedCallResultRef {
    issuer: Arc<PreparedCallGraphIssuer>,
    request: u64,
}

impl std::fmt::Debug for PreparedCallResultRef {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PreparedCallResultRef(..)")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreparedEffectRequest {
    site: CheckedCallSite,
    candidate: CallableCandidateId,
    schema: CallableSignatureSchemaDigest,
    checked: CheckedCallableId,
    group: CallableGroupIndex,
    row: Option<EffectRow>,
    result_schema: Option<CallableResultSchema>,
}

/// The inferred preparation row has exactly one owner. Candidate graph
/// deltas move its evidence together with the body facts that established it;
/// the checked catalog receives its independently validated row at finalization.
#[derive(Default)]
pub(crate) struct PreparedCallableEffectRows {
    next_request: u64,
    requests: BTreeMap<u64, PreparedEffectRequest>,
}

pub(super) struct PreparedEffectDelta {
    baseline: BTreeSet<u64>,
    requests: BTreeMap<u64, PreparedEffectRequest>,
}

/// Borrowed view of one live or extracted graph transaction. It does not
/// copy rows out of their affine owner while the selected application seals.
#[derive(Clone, Copy)]
pub(crate) struct PreparedCallableEffectView<'a> {
    live: &'a PreparedCallableEffectRows,
    extracted: Option<&'a PreparedEffectDelta>,
}

impl<'a> PreparedCallableEffectView<'a> {
    pub(crate) fn row(self, checked: &CheckedCallableId) -> Option<&'a EffectRow> {
        self.extracted
            .and_then(|delta| {
                delta.requests.values().find_map(|request| {
                    (&request.checked == checked)
                        .then_some(request.row.as_ref())
                        .flatten()
                })
            })
            .or_else(|| self.live.row(checked))
    }

    pub(crate) fn result_schema(
        self,
        checked: &CheckedCallableId,
    ) -> Option<&'a CallableResultSchema> {
        self.extracted
            .and_then(|delta| {
                delta.requests.values().find_map(|request| {
                    (&request.checked == checked)
                        .then_some(request.result_schema.as_ref())
                        .flatten()
                })
            })
            .or_else(|| self.live.result_schema(checked))
    }
}

impl PreparedEffectDelta {
    pub(super) fn replay_eq(&self, other: &Self) -> bool {
        self.baseline == other.baseline && self.requests.values().eq(other.requests.values())
    }
}

impl PreparedCallableEffectRows {
    pub(crate) const fn view(&self) -> PreparedCallableEffectView<'_> {
        PreparedCallableEffectView {
            live: self,
            extracted: None,
        }
    }

    pub(super) fn extracted_view<'a>(
        &'a self,
        delta: &'a PreparedEffectDelta,
    ) -> Result<PreparedCallableEffectView<'a>, CallConstraintInvariant> {
        self.validate_restore(delta)?;
        Ok(PreparedCallableEffectView {
            live: self,
            extracted: Some(delta),
        })
    }
    pub(crate) fn row(&self, checked: &CheckedCallableId) -> Option<&EffectRow> {
        self.requests.values().find_map(|request| {
            (&request.checked == checked)
                .then_some(request.row.as_ref())
                .flatten()
        })
    }

    pub(crate) fn result_schema(
        &self,
        checked: &CheckedCallableId,
    ) -> Option<&CallableResultSchema> {
        self.requests.values().find_map(|request| {
            (&request.checked == checked)
                .then_some(request.result_schema.as_ref())
                .flatten()
        })
    }

    pub(crate) fn rows(&self) -> impl Iterator<Item = (&CheckedCallableId, &EffectRow)> {
        self.requests
            .values()
            .filter_map(|request| request.row.as_ref().map(|row| (&request.checked, row)))
    }

    pub(super) fn is_ready(&self) -> bool {
        self.requests.values().all(|request| request.row.is_some())
    }

    pub(super) fn snapshot(&self) -> BTreeSet<u64> {
        self.requests.keys().copied().collect()
    }

    pub(super) fn request(
        &mut self,
        issuer: &Arc<PreparedCallGraphIssuer>,
        site: CheckedCallSite,
        candidate: &PreparedResolvedCallable,
    ) -> Result<PreparedCallResultRef, CallConstraintInvariant> {
        let checked = candidate
            .checked()
            .ok_or(CallConstraintInvariant::CheckedCallableAuthorityMismatch)?;
        if !matches!((candidate.schema().effects(), checked.declaration()),
            (crate::callable::CallableEffectSchema::Project { declaration }, crate::callable::CheckedCallableDeclaration::Project(actual)) if declaration == actual)
        {
            return Err(CallConstraintInvariant::CheckedCallableAuthorityMismatch);
        }
        if self
            .requests
            .values()
            .any(|request| &request.checked == checked)
        {
            return Err(CallConstraintInvariant::PendingCallableEffectProjection {
                checked: Box::new(checked.clone()),
                group: candidate.call_group(),
            });
        }
        let request = self.next_request;
        self.next_request = request
            .checked_add(1)
            .ok_or(CallConstraintInvariant::InvalidPreparedDependencyOrder)?;
        self.requests.insert(
            request,
            PreparedEffectRequest {
                site,
                candidate: candidate.id().clone(),
                schema: candidate.schema().semantic_digest(),
                checked: checked.clone(),
                group: candidate.call_group(),
                row: None,
                result_schema: None,
            },
        );
        Ok(PreparedCallResultRef {
            issuer: Arc::clone(issuer),
            request,
        })
    }

    pub(super) fn complete(
        &mut self,
        issuer: &Arc<PreparedCallGraphIssuer>,
        baseline: &BTreeSet<u64>,
        reference: &PreparedCallResultRef,
        checked: &CheckedCallableId,
        row: EffectRow,
        result_schema: Option<CallableResultSchema>,
    ) -> Result<(), CallConstraintInvariant> {
        if !Arc::ptr_eq(issuer, &reference.issuer) {
            return Err(CallConstraintInvariant::ForeignPreparedIssuer);
        }
        if baseline.contains(&reference.request) || !row.is_known() {
            return Err(CallConstraintInvariant::InvalidPreparedNodeState);
        }
        let request = self
            .requests
            .get_mut(&reference.request)
            .ok_or(CallConstraintInvariant::MissingOrStalePreparedNode)?;
        if &request.checked != checked || request.row.is_some() || request.result_schema.is_some() {
            return Err(CallConstraintInvariant::CheckedCallableAuthorityMismatch);
        }
        request.row = Some(row);
        request.result_schema = result_schema;
        Ok(())
    }

    pub(super) fn rollback(&mut self, baseline: &BTreeSet<u64>) {
        self.requests.retain(|id, _| baseline.contains(id));
    }

    pub(super) fn extract(&mut self, baseline: BTreeSet<u64>) -> PreparedEffectDelta {
        let ids = self
            .requests
            .keys()
            .filter(|id| !baseline.contains(id))
            .copied()
            .collect::<Vec<_>>();
        let requests = ids
            .into_iter()
            .filter_map(|id| self.requests.remove(&id).map(|request| (id, request)))
            .collect();
        PreparedEffectDelta { baseline, requests }
    }

    pub(super) fn validate_restore(
        &self,
        delta: &PreparedEffectDelta,
    ) -> Result<(), CallConstraintInvariant> {
        if delta
            .baseline
            .iter()
            .any(|id| !self.requests.contains_key(id))
            || delta
                .requests
                .keys()
                .any(|id| self.requests.contains_key(id) || delta.baseline.contains(id))
        {
            return Err(CallConstraintInvariant::MissingOrStalePreparedNode);
        }
        Ok(())
    }

    pub(super) fn restore(&mut self, delta: PreparedEffectDelta) {
        self.requests.extend(delta.requests);
    }

    pub(super) fn clear(&mut self) {
        self.requests.clear();
    }
}
