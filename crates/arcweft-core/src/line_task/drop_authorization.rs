//! Transaction-local evidence for exact displaced graphs and explicit drops.

use std::collections::{BTreeMap, BTreeSet};

use super::LineRuntimeError;
use crate::{effect::RuntimeDropPolicy, runtime_id::RuntimeLineHandleToken, value::RuntimeValue};

/// A boundary policy is issued by an explicit drop/scope exit. Assignment
/// instead authorizes only the exact graph returned by its place replacement.
/// Neither form is live-value storage or a declaration membership index.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeHandleDropAuthorization {
    boundary: Option<RuntimeDropPolicy>,
    #[serde(with = "displaced_rows")]
    displaced: BTreeMap<RuntimeLineHandleToken, RuntimeDropPolicy>,
}

mod displaced_rows {
    use super::*;
    use serde::de::{Error, SeqAccess, Visitor};
    use serde::{Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(
        rows: &BTreeMap<RuntimeLineHandleToken, RuntimeDropPolicy>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(rows)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<RuntimeLineHandleToken, RuntimeDropPolicy>, D::Error> {
        struct Rows;
        impl<'de> Visitor<'de> for Rows {
            type Value = BTreeMap<RuntimeLineHandleToken, RuntimeDropPolicy>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .write_str("a sorted sequence of unique displaced handle tokens and policies")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut input: A) -> Result<Self::Value, A::Error> {
                let mut rows = BTreeMap::new();
                let limit = crate::entry::RuntimeSchemaLimits::engine_default();
                while let Some((token, policy)) = input.next_element::<(_, _)>()? {
                    if !limit.permits_sequence_items(rows.len() + 1) {
                        return Err(A::Error::custom("displaced handle count exceeds its limit"));
                    }
                    if rows
                        .last_key_value()
                        .is_some_and(|(last, _)| last >= &token)
                    {
                        return Err(A::Error::custom(
                            "displaced handle tokens must be sorted and unique",
                        ));
                    }
                    rows.insert(token, policy);
                }
                Ok(rows)
            }
        }
        deserializer.deserialize_seq(Rows)
    }
}

impl RuntimeHandleDropAuthorization {
    pub(crate) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata_collection(self.displaced.len())?;
        admission.validate_metadata(&self.boundary)?;
        for row in &self.displaced {
            admission.validate_metadata(&row)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_id::{
        DialogueActivationId, RuntimeDialogueContentPlanId, RuntimeLineHandleSiteId,
        RuntimePersistentFiberId,
    };
    use std::num::NonZeroU32;

    #[test]
    fn displaced_handle_rows_retain_exact_policies_and_reject_repeated_tokens() {
        let activation = DialogueActivationId::new(
            crate::effect::RuntimeArtifactFingerprint::try_from_bytes([7; 32]).unwrap(),
            RuntimePersistentFiberId::from_allocated(11),
            RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::new(3).unwrap()),
            17,
        );
        let token = RuntimeLineHandleToken::new(
            activation,
            RuntimeLineHandleSiteId::from_zero_based(3),
            23,
        );
        let mut authorization =
            RuntimeHandleDropAuthorization::at_boundary(Some(RuntimeDropPolicy::Release));
        authorization
            .authorize_tokens_with_policy([token.clone()], RuntimeDropPolicy::Cancel)
            .unwrap();
        let mut wire = serde_json::to_value(&authorization).unwrap();
        let decoded: RuntimeHandleDropAuthorization = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(decoded, authorization);
        assert_eq!(decoded.policy_for(&token), Some(RuntimeDropPolicy::Cancel));

        let rows = wire["displaced"].as_array_mut().unwrap();
        rows.push(rows[0].clone());
        let error = serde_json::from_value::<RuntimeHandleDropAuthorization>(wire).unwrap_err();
        assert!(error.to_string().contains("sorted and unique"));
    }
}
