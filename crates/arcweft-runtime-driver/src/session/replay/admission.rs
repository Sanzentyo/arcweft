//! Borrowed retained-graph admission shared by replay recording and execution.

use super::model::{
    RecordedExternalOutcome, RecordedExternalOutcomeResultV1, RecordedRootOutcomeV1,
    RecordedRootTransitionV1, RootReplayTraceV1,
};
use arcweft_core::{
    entry::RuntimeValueDigest,
    pattern::RuntimeSemanticTypeId,
    root::RootTransitionOutcome,
    value::{AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission},
};

impl RecordedRootTransitionV1 {
    pub(super) fn admit_retained(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        event_type: RuntimeSemanticTypeId,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata(&(self.sequence, self.event_digest, &self.outcome))?;
        admission.validate_typed_snapshot(&self.event, event_type)
    }
}

impl RecordedExternalOutcome {
    pub(super) fn admit_retained(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata(&(self.position, &self.request, self.root_event_sequence))?;
        match &self.outcome {
            RecordedExternalOutcomeResultV1::Success(value) => admission.validate_snapshot(value),
            RecordedExternalOutcomeResultV1::Failure { kind, message } => {
                admission.validate_metadata(&(kind, message))
            }
        }
    }
}

impl RootReplayTraceV1 {
    pub(super) fn admit_retained(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        event_type: RuntimeSemanticTypeId,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata(&(&self.identity, self.initializer_state_digest))?;
        admission.validate_metadata_collection(self.transitions.len())?;
        admission.validate_metadata_collection(self.external_outcomes.len())?;
        for transition in &self.transitions {
            transition.admit_retained(admission, event_type)?;
        }
        for outcome in &self.external_outcomes {
            outcome.admit_retained(admission)?;
        }
        Ok(())
    }
}

impl<'a> From<&'a RootTransitionOutcome> for RecordedRootOutcomeV1<&'a [RuntimeValueDigest]> {
    fn from(value: &'a RootTransitionOutcome) -> Self {
        match value {
            RootTransitionOutcome::Committed {
                state_digest,
                command_digests,
                ..
            } => Self::Committed {
                state_digest: *state_digest,
                command_digests,
            },
            RootTransitionOutcome::Rejected { error_digest, .. } => Self::Rejected {
                error_digest: *error_digest,
            },
            RootTransitionOutcome::Trapped { failure_digest, .. } => Self::Trapped {
                failure_digest: *failure_digest,
            },
        }
    }
}

impl RecordedRootOutcomeV1<&[RuntimeValueDigest]> {
    /// The enclosing recorder admits every borrowed outcome before this copy.
    pub(super) fn capture_admitted(self) -> RecordedRootOutcomeV1 {
        match self {
            Self::Committed {
                state_digest,
                command_digests,
            } => RecordedRootOutcomeV1::Committed {
                state_digest,
                command_digests: command_digests.to_vec(),
            },
            Self::Rejected { error_digest } => RecordedRootOutcomeV1::Rejected { error_digest },
            Self::Trapped { failure_digest } => RecordedRootOutcomeV1::Trapped { failure_digest },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{
        RecordedExternalOutcomePositionV1, RecordedHostCallErrorKindV1, RuntimeHostCallId,
    };
    use arcweft_core::{entry::RuntimeSchemaLimits, root::TransitionSequence};

    #[test]
    fn borrowed_transition_outcomes_share_the_owned_wire_grammar() {
        let digest = RuntimeValueDigest::from_bytes([7; 32]);
        for outcome in [
            RootTransitionOutcome::Committed {
                sequence: TransitionSequence::ZERO,
                state_digest: digest,
                command_digests: vec![digest; 2],
            },
            RootTransitionOutcome::Rejected {
                sequence: TransitionSequence::ZERO,
                code: "rejected".into(),
                message: "the diagnostic is not a retained replay field".into(),
                error_digest: digest,
            },
            RootTransitionOutcome::Trapped {
                sequence: TransitionSequence::ZERO,
                message: "the diagnostic is not a retained replay field".into(),
                failure_digest: digest,
            },
        ] {
            let borrowed = RecordedRootOutcomeV1::from(&outcome);
            let mut admission =
                RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits::engine_default());
            admission.validate_metadata(&borrowed).unwrap();
            let wire = serde_json::to_value(&borrowed).unwrap();
            let captured = borrowed.capture_admitted();
            assert_eq!(wire, serde_json::to_value(&captured).unwrap());
            let decoded: RecordedRootOutcomeV1 = serde_json::from_value(wire).unwrap();
            assert_eq!(decoded, captured);
        }
    }

    #[test]
    fn external_failure_metadata_uses_one_sticky_allowance_without_value_nodes() {
        let outcome = RecordedExternalOutcome {
            position: RecordedExternalOutcomePositionV1::BeforeTransition(TransitionSequence::ZERO),
            request: RuntimeHostCallId("request.first".into()),
            outcome: RecordedExternalOutcomeResultV1::Failure {
                kind: RecordedHostCallErrorKindV1::Failed,
                message: "retained external failure".into(),
            },
            root_event_sequence: None,
        };
        let mut admission = RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits {
            max_nodes: 0,
            max_encoded_bytes: 0,
            max_metadata_bytes: 100,
            ..RuntimeSchemaLimits::engine_default()
        });
        outcome.admit_retained(&mut admission).unwrap();
        let failure = outcome.admit_retained(&mut admission).unwrap_err();
        assert_eq!(
            failure,
            AwbcRuntimeValueSnapshotError::Resource(
                arcweft_core::entry::RuntimeSchemaError::BudgetExceeded {
                    budget: "metadata_bytes"
                }
            )
        );
        assert_eq!(admission.validate_metadata(&()), Err(failure));
    }
}
