//! Current-checkout migration evidence for the closed callable-family inventory.

use super::CallableFamily;
use crate::callable::CallableValidator;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MigrationAuthorityClass {
    RejectingSchema,
    IntentionallyUnchecked,
    PendingAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MigrationCompletionDisposition {
    Credited,
    PendingAuthority,
    PendingRemoval,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MigrationFamilyEvidence {
    current: MigrationAuthorityClass,
    final_completion: MigrationCompletionDisposition,
}

impl MigrationFamilyEvidence {
    const fn new(
        current: MigrationAuthorityClass,
        final_completion: MigrationCompletionDisposition,
    ) -> Self {
        Self {
            current,
            final_completion,
        }
    }

    pub(crate) const fn current(self) -> MigrationAuthorityClass {
        self.current
    }

    pub(crate) const fn final_completion(self) -> MigrationCompletionDisposition {
        self.final_completion
    }
}

impl CallableFamily {
    /// Classifies the current production authority and its final-completion credit.
    ///
    /// Historical pre-switch phases remain implementation/VCS evidence. This
    /// exhaustive match always describes the actual checkout and therefore
    /// cannot manufacture a hypothetical final family inventory.
    pub(crate) const fn migration_evidence(self) -> MigrationFamilyEvidence {
        use MigrationAuthorityClass::{IntentionallyUnchecked, PendingAuthority, RejectingSchema};
        use MigrationCompletionDisposition::{
            Credited, PendingAuthority as CompletionPendingAuthority, PendingRemoval,
        };

        match self {
            Self::Fx
            | Self::EnumConstructor
            | Self::ResultConstructor
            | Self::OptionConstructor
            | Self::Builtin
            | Self::Agent
            | Self::Presentation
            | Self::Project
            | Self::Environment
            | Self::Lexical
            | Self::FunctionValue
            | Self::CollectionMethod
            | Self::PresentationHandleMethod
            | Self::IntegerMethod
            | Self::DomainMethod
            | Self::TraitMethod
            | Self::DataLast
            | Self::StageMethod => MigrationFamilyEvidence::new(RejectingSchema, Credited),
            Self::CapacityMethod | Self::Drop | Self::Promotion => {
                MigrationFamilyEvidence::new(IntentionallyUnchecked, Credited)
            }
            Self::Speaker => MigrationFamilyEvidence::new(IntentionallyUnchecked, PendingRemoval),
            Self::Dialogue => {
                MigrationFamilyEvidence::new(PendingAuthority, CompletionPendingAuthority)
            }
        }
    }

    /// Checks that an observed candidate schema still belongs to this family.
    pub(crate) fn migration_validator_matches(self, validator: &CallableValidator) -> bool {
        match self {
            Self::Fx => matches!(validator, CallableValidator::Fx(_)),
            Self::EnumConstructor => matches!(validator, CallableValidator::EnumConstructor(_)),
            Self::ResultConstructor => {
                matches!(validator, CallableValidator::ResultConstructor(_))
            }
            Self::OptionConstructor => {
                matches!(validator, CallableValidator::OptionConstructor(_))
            }
            Self::Builtin => matches!(
                validator,
                CallableValidator::Builtin(_) | CallableValidator::ReductionConstructor(_)
            ),
            Self::Agent => matches!(validator, CallableValidator::Agent(_)),
            Self::Presentation => matches!(validator, CallableValidator::Presentation(_)),
            Self::Dialogue => matches!(validator, CallableValidator::Dialogue(_)),
            Self::Project
            | Self::Environment
            | Self::Lexical
            | Self::FunctionValue
            | Self::DataLast => matches!(validator, CallableValidator::Ordinary),
            Self::CollectionMethod => matches!(validator, CallableValidator::Collection(_)),
            Self::PresentationHandleMethod => {
                matches!(validator, CallableValidator::PresentationHandle(_))
            }
            Self::IntegerMethod => matches!(validator, CallableValidator::Integer(_)),
            Self::DomainMethod => matches!(validator, CallableValidator::Domain(_)),
            Self::TraitMethod => matches!(validator, CallableValidator::Trait(_)),
            Self::CapacityMethod => matches!(validator, CallableValidator::Capacity(_)),
            Self::StageMethod => matches!(validator, CallableValidator::Stage(_)),
            Self::Drop => matches!(validator, CallableValidator::Drop),
            Self::Promotion => matches!(validator, CallableValidator::Promotion(_)),
            Self::Speaker => matches!(validator, CallableValidator::Speaker),
        }
    }
}
