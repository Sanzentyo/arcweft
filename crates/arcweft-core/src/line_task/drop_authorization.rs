//! Transaction-local evidence for exact displaced graphs and explicit drops.

use std::collections::{BTreeMap, BTreeSet};

use super::LineRuntimeError;
use crate::{effect::RuntimeDropPolicy, runtime_id::RuntimeLineHandleToken, value::RuntimeValue};

/// A boundary policy is issued by an explicit drop/scope exit. Assignment
/// instead authorizes only the exact graph returned by its place replacement.
/// Neither form is live-value storage or a declaration membership index.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RuntimeHandleDropAuthorization {
    boundary: Option<RuntimeDropPolicy>,
    displaced: BTreeMap<RuntimeLineHandleToken, RuntimeDropPolicy>,
}

impl RuntimeHandleDropAuthorization {
    pub(crate) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata_collection(self.displaced.len())?;
        for token in self.displaced.keys() {
            admission.validate_metadata(token)?;
        }
        Ok(())
    }

    pub(crate) fn has_displaced_values(&self) -> bool {
        !self.displaced.is_empty()
    }

    pub(crate) fn at_boundary(policy: Option<RuntimeDropPolicy>) -> Self {
        Self {
            boundary: policy,
            displaced: BTreeMap::new(),
        }
    }

    pub(crate) fn authorize_displaced(
        &mut self,
        value: &RuntimeValue,
    ) -> Result<(), LineRuntimeError> {
        let handles = value
            .affine_line_handles()
            .map_err(|_| LineRuntimeError::InvalidHandlePayload)?;
        self.authorize_handles(handles)
    }

    pub(crate) fn authorize_handles(
        &mut self,
        handles: Vec<crate::value::ownership::RuntimeAffineLineHandle>,
    ) -> Result<(), LineRuntimeError> {
        self.authorize_tokens(handles.into_iter().map(|handle| handle.token().clone()))
    }

    pub(crate) fn authorize_tokens(
        &mut self,
        tokens: impl IntoIterator<Item = RuntimeLineHandleToken>,
    ) -> Result<(), LineRuntimeError> {
        self.authorize_tokens_with_policy(tokens, RuntimeDropPolicy::Default)
    }

    pub(crate) fn authorize_tokens_with_policy(
        &mut self,
        tokens: impl IntoIterator<Item = RuntimeLineHandleToken>,
        policy: RuntimeDropPolicy,
    ) -> Result<(), LineRuntimeError> {
        let mut observed = BTreeSet::new();
        for token in tokens {
            if self.displaced.contains_key(&token) || !observed.insert(token) {
                return Err(LineRuntimeError::DuplicateHandleOccurrence);
            }
        }
        self.displaced
            .extend(observed.into_iter().map(|token| (token, policy)));
        Ok(())
    }

    pub(crate) fn set_boundary(
        &mut self,
        policy: Option<RuntimeDropPolicy>,
    ) -> Result<(), LineRuntimeError> {
        if self.boundary.is_some() && policy.is_some() && self.boundary != policy {
            return Err(LineRuntimeError::InvalidActivationOperation);
        }
        self.boundary = policy.or(self.boundary);
        Ok(())
    }

    pub(crate) fn validate_removed(
        &self,
        mut before: impl FnMut(&RuntimeLineHandleToken) -> bool,
        mut after: impl FnMut(&RuntimeLineHandleToken) -> bool,
    ) -> Result<(), LineRuntimeError> {
        if self
            .displaced
            .keys()
            .any(|token| !before(token) || after(token))
        {
            return Err(LineRuntimeError::UnjournaledHandleDrop);
        }
        Ok(())
    }

    pub(crate) fn policy_for(&self, token: &RuntimeLineHandleToken) -> Option<RuntimeDropPolicy> {
        self.displaced.get(token).copied().or(self.boundary)
    }

    pub(crate) fn displaced_tokens(&self) -> impl Iterator<Item = &RuntimeLineHandleToken> {
        self.displaced.keys()
    }

    pub(crate) fn for_tokens(&self, includes: impl Fn(&RuntimeLineHandleToken) -> bool) -> Self {
        Self {
            boundary: self.boundary,
            displaced: self
                .displaced
                .iter()
                .filter(|(token, _)| includes(token))
                .map(|(token, policy)| (token.clone(), *policy))
                .collect(),
        }
    }
}
