//! Original-program admission of one borrowed declaration parameter group.

use super::{AwbcProgram, AwbcTypeId, BTreeSet, ParameterMatcher, RuntimeValue};
use crate::entry::RuntimeSchemaLimits;
use crate::entry::schema::value_encoding::{ValueAdmissionOrigin, ValueResourceAdmission};
use crate::task::RuntimeProgramOwner;
use crate::value::{AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError};
use std::sync::Arc;

/// Failure while admitting a borrowed View declaration parameter group.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeParameterValueAdmissionError {
    #[error("invalid declaration parameter contract {contract:?}")]
    Contract { contract: AwbcTypeId },
    #[error("declaration parameter {ordinal} is absent or repeated")]
    Ordinal { ordinal: usize },
    #[error("declaration parameter {ordinal} failed original-program value admission: {source}")]
    Value {
        ordinal: usize,
        #[source]
        source: Box<AwbcRuntimeValueSnapshotError>,
    },
    #[error("declaration parameter {ordinal} violates its joint type/effect relation")]
    Relation { ordinal: usize },
    #[error("declaration parameter values violate their joint effect predicate")]
    Predicate,
    #[error("declaration parameter group exhausted its admission resources: {source}")]
    Resource {
        #[source]
        source: crate::entry::RuntimeSchemaError,
    },
}

impl AwbcProgram {
    /// Admits the entire supplied group while every value remains borrowed.
    /// Original callable/capture/storage ownership, resource limits, deep Copy,
    /// and the joint parameter relation share one work allowance. This creates
    /// neither a snapshot nor another live value owner.
    pub fn validate_parameter_contract_values<'value>(
        self: &Arc<Self>,
        contract: AwbcTypeId,
        inputs: impl IntoIterator<Item = (usize, &'value RuntimeValue)>,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), RuntimeParameterValueAdmissionError> {
        let mut resources = ValueResourceAdmission::snapshot(limits, ValueAdmissionOrigin::Live);
        let (parameters, mut matcher) =
            ParameterMatcher::new_with_work(self.as_ref(), contract, None, resources.work())
                .ok_or_else(|| {
                    resources.work().failure().map_or(
                        RuntimeParameterValueAdmissionError::Contract { contract },
                        |source| RuntimeParameterValueAdmissionError::Resource { source },
                    )
                })?;
        let owner = RuntimeProgramOwner::Awbc(Arc::clone(self));
        let mut seen = BTreeSet::new();
        for (ordinal, value) in inputs {
            let expected = parameters
                .get(ordinal)
                .filter(|_| seen.insert(ordinal))
                .ok_or(RuntimeParameterValueAdmissionError::Ordinal { ordinal })?;
            let failure = |source| RuntimeParameterValueAdmissionError::Value {
                ordinal,
                source: Box::new(source),
            };
            value
                .view()
                .admit_unrestricted(&mut resources)
                .map_err(failure)?;
            AwbcRuntimeValueSnapshot::validate_admitted_storage_view(
                value.view(),
                Some(&owner),
                &resources,
            )
            .map_err(failure)?;
            let environment = matcher.parameters.clone();
            if matcher.value(*expected, value, 0, &environment).is_err() {
                return Err(resources.work().failure().map_or(
                    RuntimeParameterValueAdmissionError::Relation { ordinal },
                    |error| failure(AwbcRuntimeValueSnapshotError::from(error)),
                ));
            }
        }
        if !matcher.accepted() {
            if let Some(error) = resources.work().failure() {
                return Err(RuntimeParameterValueAdmissionError::Resource { source: error });
            }
            return Err(RuntimeParameterValueAdmissionError::Predicate);
        }
        Ok(())
    }
}
