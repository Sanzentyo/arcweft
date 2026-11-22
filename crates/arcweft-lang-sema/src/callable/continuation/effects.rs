//! Graph-owned inference prerequisites for prepared function-value results.

use super::{
    Arc, BTreeMap, BTreeSet, CallConstraintInvariant, CheckedCallSite, PreparedCallGraphIssuer,
};
use crate::{
    callable::{
        CallableCandidateId, CallableGroupIndex, CallableResultSchema,
        CallableSignatureSchemaDigest, CheckedCallableId, PreparedResolvedCallable,
    },
    effect_row::{DecisionControl, DecisionWork, EffectRow},
};

/// The actual producer that needs a declaration's latent callable type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedCallableEffectProjectionSite {
    CallResult(CheckedCallSite),
    CallableValue(arcweft_lang_hir::identity::ExprId),
}

impl From<CheckedCallSite> for PreparedCallableEffectProjectionSite {
    fn from(site: CheckedCallSite) -> Self {
        Self::CallResult(site)
    }
}

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
    site: PreparedCallableEffectProjectionSite,
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
    requests: BTreeMap<u64, LiveEffectRequest>,
    writes: Vec<u64>,
}

struct LiveEffectRequest {
    frame: u64,
    request: PreparedEffectRequest,
}

/// The graph issues increasing frame IDs. A restored request joins the
/// receiving frame regardless of when it was originally issued.
pub(super) struct PreparedEffectRequestCheckpoint {
    frame: u64,
    journal_start: usize,
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
    pub(super) fn has_writes(&self) -> bool {
        !self.requests.is_empty()
    }

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
        self.requests
            .values()
            .map(|entry| &entry.request)
            .find_map(|request| {
                (&request.checked == checked)
                    .then_some(request.row.as_ref())
                    .flatten()
            })
    }

    pub(crate) fn result_schema(
        &self,
        checked: &CheckedCallableId,
    ) -> Option<&CallableResultSchema> {
        self.requests
            .values()
            .map(|entry| &entry.request)
            .find_map(|request| {
                (&request.checked == checked)
                    .then_some(request.result_schema.as_ref())
                    .flatten()
            })
    }

    pub(crate) fn rows(&self) -> impl Iterator<Item = (&CheckedCallableId, &EffectRow)> {
        self.requests
            .values()
            .map(|entry| &entry.request)
            .filter_map(|request| request.row.as_ref().map(|row| (&request.checked, row)))
    }

    pub(super) fn is_ready(&self) -> bool {
        self.requests
            .values()
            .all(|entry| entry.request.row.is_some())
    }

    pub(super) fn checkpoint(&self, frame: u64) -> PreparedEffectRequestCheckpoint {
        PreparedEffectRequestCheckpoint {
            frame,
            journal_start: self.writes.len(),
        }
    }

    pub(super) fn preflight_extract<C: DecisionControl>(
        &self,
        checkpoint: &PreparedEffectRequestCheckpoint,
        control: &mut C,
    ) -> Result<BTreeSet<u64>, C::Error> {
        let mut baseline = BTreeSet::new();
        for (&id, entry) in &self.requests {
            control.charge(DecisionWork::Visit)?;
            if entry.frame < checkpoint.frame {
                control.charge(DecisionWork::Node)?;
                baseline.insert(id);
            }
        }
        for _ in &self.writes[checkpoint.journal_start..] {
            control.charge(DecisionWork::Node)?;
        }
        Ok(baseline)
    }

    pub(super) fn request<C: DecisionControl>(
        &mut self,
        issuer: &Arc<PreparedCallGraphIssuer>,
        checkpoint: &PreparedEffectRequestCheckpoint,
        site: PreparedCallableEffectProjectionSite,
        candidate: &PreparedResolvedCallable,
        control: &mut C,
    ) -> Result<PreparedCallResultRef, super::PreparedCallGraphMutationFailure<C::Error>> {
        use super::PreparedCallGraphMutationFailure::Control;
        let checked = candidate
            .checked()
            .ok_or(CallConstraintInvariant::CheckedCallableAuthorityMismatch)?;
        if !matches!((candidate.schema().effects(), checked.declaration()),
            (crate::callable::CallableEffectSchema::Project { declaration }, crate::callable::CheckedCallableDeclaration::Project(actual)) if declaration == actual)
        {
            return Err(CallConstraintInvariant::CheckedCallableAuthorityMismatch.into());
        }
        for entry in self.requests.values() {
            control.charge(DecisionWork::Visit).map_err(Control)?;
            if &entry.request.checked == checked {
                return Err(CallConstraintInvariant::PendingCallableEffectProjection {
                    checked: Box::new(checked.clone()),
                    group: candidate.call_group(),
                }
                .into());
            }
        }
        let request = self.next_request;
        let next_request = request
            .checked_add(1)
            .ok_or(CallConstraintInvariant::InvalidPreparedDependencyOrder)?;
        // Admit the live entry and the journal record before either changes.
        control.charge(DecisionWork::Node).map_err(Control)?;
        control.charge(DecisionWork::Node).map_err(Control)?;
        self.next_request = next_request;
        self.insert(
            checkpoint,
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
        checkpoint: &PreparedEffectRequestCheckpoint,
        reference: &PreparedCallResultRef,
        checked: &CheckedCallableId,
        row: EffectRow,
        result_schema: Option<CallableResultSchema>,
    ) -> Result<(), CallConstraintInvariant> {
        if !Arc::ptr_eq(issuer, &reference.issuer) {
            return Err(CallConstraintInvariant::ForeignPreparedIssuer);
        }
        if !row.is_known() {
            return Err(CallConstraintInvariant::InvalidPreparedNodeState);
        }
        let entry = self
            .requests
            .get_mut(&reference.request)
            .ok_or(CallConstraintInvariant::MissingOrStalePreparedNode)?;
        if entry.frame < checkpoint.frame {
            return Err(CallConstraintInvariant::InvalidPreparedNodeState);
        }
        let request = &mut entry.request;
        if &request.checked != checked || request.row.is_some() || request.result_schema.is_some() {
            return Err(CallConstraintInvariant::CheckedCallableAuthorityMismatch);
        }
        request.row = Some(row);
        request.result_schema = result_schema;
        Ok(())
    }

    pub(super) fn rollback(&mut self, checkpoint: PreparedEffectRequestCheckpoint) {
        for id in self.writes.drain(checkpoint.journal_start..) {
            self.requests.remove(&id).expect("journaled effect request");
        }
    }

    pub(super) fn extract(
        &mut self,
        checkpoint: PreparedEffectRequestCheckpoint,
        baseline: BTreeSet<u64>,
    ) -> PreparedEffectDelta {
        let requests = self
            .writes
            .drain(checkpoint.journal_start..)
            .map(|id| {
                (
                    id,
                    self.requests
                        .remove(&id)
                        .expect("journaled effect request")
                        .request,
                )
            })
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

    pub(super) fn validate_restore_batch<'a, C: DecisionControl>(
        &self,
        deltas: impl Iterator<Item = &'a PreparedEffectDelta> + Clone,
        control: &mut C,
    ) -> Result<(), super::PreparedCallGraphMutationFailure<C::Error>> {
        use super::PreparedCallGraphMutationFailure::Control;

        let mut requests = BTreeSet::new();
        let mut callables = BTreeSet::new();
        for delta in deltas.clone() {
            for (&id, request) in &delta.requests {
                control.charge(DecisionWork::Visit).map_err(Control)?;
                // Two scratch indexes, one live entry, and one journal record.
                for _ in 0..4 {
                    control.charge(DecisionWork::Node).map_err(Control)?;
                }
                if self.requests.contains_key(&id)
                    || delta.baseline.contains(&id)
                    || !requests.insert(id)
                {
                    return Err(CallConstraintInvariant::MissingOrStalePreparedNode.into());
                }
                if !callables.insert(&request.checked) {
                    return Err(CallConstraintInvariant::PendingCallableEffectProjection {
                        checked: Box::new(request.checked.clone()),
                        group: request.group,
                    }
                    .into());
                }
            }
        }
        for id in deltas.flat_map(|delta| &delta.baseline) {
            control.charge(DecisionWork::Visit).map_err(Control)?;
            if !self.requests.contains_key(id) && !requests.contains(id) {
                return Err(CallConstraintInvariant::MissingOrStalePreparedNode.into());
            }
        }
        if !callables.is_empty() {
            for entry in self.requests.values() {
                control.charge(DecisionWork::Visit).map_err(Control)?;
                let request = &entry.request;
                if callables.contains(&request.checked) {
                    return Err(CallConstraintInvariant::PendingCallableEffectProjection {
                        checked: Box::new(request.checked.clone()),
                        group: request.group,
                    }
                    .into());
                }
            }
        }
        Ok(())
    }

    pub(super) fn restore(
        &mut self,
        checkpoint: &PreparedEffectRequestCheckpoint,
        delta: PreparedEffectDelta,
    ) {
        for (id, request) in delta.requests {
            self.insert(checkpoint, id, request);
        }
    }

    fn insert(
        &mut self,
        checkpoint: &PreparedEffectRequestCheckpoint,
        id: u64,
        request: PreparedEffectRequest,
    ) {
        self.requests.insert(
            id,
            LiveEffectRequest {
                frame: checkpoint.frame,
                request,
            },
        );
        self.writes.push(id);
    }

    pub(super) fn commit_root(&mut self) {
        self.writes.clear();
    }

    pub(super) fn clear(&mut self) {
        self.requests.clear();
        self.writes.clear();
    }
}
