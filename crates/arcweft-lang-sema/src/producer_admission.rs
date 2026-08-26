//! Checked Need-producer argument admission composer.
//!
//! This module joins current HIR/call facts, stable semantic coordinates, and
//! the ownership classifier. It deliberately does not own a second type
//! classifier or expose caller-constructed admission rows.

use arcweft_lang_hir::{
    identity::ExprId, project::HirExecutableProjectView, symbol::ProjectSymbolTable,
};
use thiserror::Error;

use crate::{
    callable::{
        CheckedCallArgumentPassing, CheckedCallArgumentSlotSource, CheckedCallCalleeExecution,
        CheckedCallRuntimeOperand, CheckedCallRuntimeOperandOrder,
    },
    env::RegisteredSemanticWorld,
    final_analysis::{FinalSemanticAnalysis, SemanticTranscriptError, TranscriptHasher, write_len},
    ownership::{
        CheckedOwnershipCertificate, CheckedOwnershipError, CheckedOwnershipLimits,
        OwnershipEvidenceDigest, RetainedValueDisposition, classify_checked_producer_arguments,
    },
    semantic_coordinate::StableCheckedValueCoordinate,
    types::{SemanticTypeDigest, TypeKind},
};

/// Stable digest proving that the exact source-ordered Need producer values
/// are retainable. Producer identity and task identity deliberately do not
/// participate in this admission digest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedNeedProducerAdmissionDigest([u8; 32]);

impl CheckedNeedProducerAdmissionDigest {
    const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// One source-ordered producer argument admitted from exact checked call
/// facts. Construction remains private to this composer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProducerArgumentAdmission {
    coordinate: StableCheckedValueCoordinate,
    ty: SemanticTypeDigest,
    disposition: RetainedValueDisposition,
}

impl CheckedProducerArgumentAdmission {
    pub const fn coordinate(&self) -> &StableCheckedValueCoordinate {
        &self.coordinate
    }

    pub const fn ty(&self) -> SemanticTypeDigest {
        self.ty
    }

    pub const fn disposition(&self) -> RetainedValueDisposition {
        self.disposition
    }
}

/// Transactional semantic admission for the exact arguments of one selected
/// producer call. Producer contract, task plan, runtime values, and task
/// identity are deliberately outside this certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedNeedProducerAdmission {
    arguments: Box<[CheckedProducerArgumentAdmission]>,
    ownership: CheckedOwnershipCertificate,
    digest: CheckedNeedProducerAdmissionDigest,
}

impl CheckedNeedProducerAdmission {
    pub fn arguments(&self) -> &[CheckedProducerArgumentAdmission] {
        &self.arguments
    }

    pub const fn ownership(&self) -> CheckedOwnershipCertificate {
        self.ownership
    }

    pub const fn digest(&self) -> CheckedNeedProducerAdmissionDigest {
        self.digest
    }
}

/// Failure to derive an exact producer-argument certificate from current
/// checked call and HIR authorities.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CheckedNeedProducerAdmissionError {
    #[error(transparent)]
    Semantic(#[from] SemanticTranscriptError),
    #[error(transparent)]
    Ownership(#[from] CheckedOwnershipError),
    #[error("producer admission requires one exact selected call")]
    NotSelectedCall,
    #[error(
        "producer call retains a receiver, function value, or capture not admitted by this cut"
    )]
    UnsupportedCapture,
    #[error("producer call argument inventory is not one exact source expression per argument")]
    UnsupportedArgumentInventory,
    #[error("producer admission work limit exceeded")]
    WorkLimit,
}

impl FinalSemanticAnalysis {
    /// Derives the exact source-ordered semantic retention certificate for a
    /// direct selected producer call.
    ///
    /// Calls with a receiver/function-value capture, spreads, compact numeric
    /// slots, recovery, or a value requiring a live Need/Function certificate
    /// fail closed in this cut.
    pub fn checked_need_producer_admission_for_call(
        &self,
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        call: ExprId,
        limits: CheckedOwnershipLimits,
    ) -> Result<CheckedNeedProducerAdmission, CheckedNeedProducerAdmissionError> {
        let values = self.checked_producer_argument_values(project, symbols, call, limits)?;
        let types = values.iter().map(|(_, ty)| *ty).collect::<Vec<_>>();
        let (dispositions, ownership) =
            classify_checked_producer_arguments(self, world, &types, limits)?;
        let arguments = values
            .into_iter()
            .zip(dispositions)
            .map(
                |((coordinate, ty), disposition)| CheckedProducerArgumentAdmission {
                    coordinate,
                    ty: ty.semantic_identity_digest(),
                    disposition,
                },
            )
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let digest = need_producer_admission_digest(&arguments, ownership.evidence())?;
        Ok(CheckedNeedProducerAdmission {
            arguments,
            ownership,
            digest,
        })
    }

    fn checked_producer_argument_values<'a>(
        &'a self,
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        call: ExprId,
        limits: CheckedOwnershipLimits,
    ) -> Result<Vec<(StableCheckedValueCoordinate, &'a TypeKind)>, CheckedNeedProducerAdmissionError>
    {
        self.validate_generation(project, symbols)
            .map_err(SemanticTranscriptError::from)?;
        let facts = self
            .call(call)
            .ok_or(CheckedNeedProducerAdmissionError::NotSelectedCall)?;
        let Some(application) = facts.selected_application() else {
            return Err(CheckedNeedProducerAdmissionError::NotSelectedCall);
        };
        let core = application.core();
        self.checked_callable_join(call)
            .map_err(|_| SemanticTranscriptError::MissingCallableJoin)?;
        if !matches!(core.callee(), CheckedCallCalleeExecution::Direct) {
            return Err(CheckedNeedProducerAdmissionError::UnsupportedCapture);
        }
        let execution = core.execution();
        let operands = execution.ordered_runtime_operands(CheckedCallRuntimeOperandOrder::Source);
        if u64::try_from(operands.len()).unwrap_or(u64::MAX) > limits.max_producer_arguments {
            return Err(CheckedNeedProducerAdmissionError::WorkLimit);
        }
        if operands
            .iter()
            .any(|operand| matches!(operand, CheckedCallRuntimeOperand::Receiver { .. }))
        {
            return Err(CheckedNeedProducerAdmissionError::UnsupportedCapture);
        }
        if operands.len() != execution.arguments().len() {
            return Err(CheckedNeedProducerAdmissionError::UnsupportedArgumentInventory);
        }
        let mut values = Vec::with_capacity(operands.len());
        for (ordinal, operand) in operands.iter().copied().enumerate() {
            let CheckedCallRuntimeOperand::Argument {
                argument,
                passing,
                slot,
            } = operand
            else {
                return Err(CheckedNeedProducerAdmissionError::UnsupportedCapture);
            };
            if passing == CheckedCallArgumentPassing::Spread
                || usize::from(argument.get()) != ordinal
                || slot.slot().get() != 0
            {
                return Err(CheckedNeedProducerAdmissionError::UnsupportedArgumentInventory);
            }
            let CheckedCallArgumentSlotSource::Expression(_) = slot.source().raw() else {
                return Err(CheckedNeedProducerAdmissionError::UnsupportedArgumentInventory);
            };
            // The C sealer already proved the raw expression type against the
            // checked-base effect projection and frozen solution. Retention
            // classification consumes that final execution type, not the raw
            // annotation/inference carrier.
            values.push((slot.source().coordinate().clone(), slot.inferred()));
        }
        Ok(values)
    }
}

fn need_producer_admission_digest(
    arguments: &[CheckedProducerArgumentAdmission],
    evidence: OwnershipEvidenceDigest,
) -> Result<CheckedNeedProducerAdmissionDigest, SemanticTranscriptError> {
    let mut used = 0;
    let mut hasher = TranscriptHasher::new(&mut used, u64::MAX);
    hasher.update(b"arcweft.lang.need-producer-admission.v1\0");
    write_len(&mut hasher, arguments.len());
    for argument in arguments {
        hasher.update(&argument.coordinate().canonical_bytes());
        hasher.update(argument.ty().as_bytes());
        hasher.update(&[argument.disposition().semantic_tag()]);
    }
    hasher.update(evidence.as_bytes());
    Ok(CheckedNeedProducerAdmissionDigest::from_bytes(
        hasher.finalize()?,
    ))
}
