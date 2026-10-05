//! Checked Need-producer argument admission composer.
//!
//! This module joins current HIR/call facts, stable semantic coordinates, and
//! the ownership classifier. It deliberately does not own a second type
//! classifier or expose caller-constructed admission rows.

use arcweft_lang_hir::{
    identity::ExprId, project::HirAnalysisProjectView, symbol::ProjectSymbolTable,
};
use thiserror::Error;

use crate::{
    callable::{
        CheckedCallArgumentPassing, CheckedCallArgumentSlotSource, CheckedCallCalleeExecution,
        CheckedCallRuntimeOperand,
    },
    env::RegisteredSemanticWorld,
    final_analysis::{
        CheckedTranscriptByteBudget, FinalSemanticAnalysis, TranscriptHasher, TranscriptWriteError,
        write_len,
    },
    ownership::{
        CheckedOwnershipCertificate, CheckedOwnershipError, CheckedOwnershipLimits,
        OwnershipEvidenceDigest, RetainedValueDisposition, classify_checked_producer_arguments,
    },
    semantic_coordinate::StableCheckedValueCoordinate,
    types::{GenericScopeError, SemanticTypeDigest, TypeKind},
};
use arcweft_core::task::NeedProducerSiteDigest;

/// Static definition issued from one accepted producer expression. Its site
/// uses the canonical checked path; its plan commits the complete body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExpressionProducerDefinition {
    site: NeedProducerSiteDigest,
    plan: arcweft_core::task::TaskPlanSemanticDigest,
}

impl CheckedExpressionProducerDefinition {
    pub const fn site(&self) -> NeedProducerSiteDigest {
        self.site
    }
    pub const fn plan(&self) -> arcweft_core::task::TaskPlanSemanticDigest {
        self.plan
    }
}

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
    site: NeedProducerSiteDigest,
    arguments: Box<[CheckedProducerArgumentAdmission]>,
    ownership: CheckedOwnershipCertificate,
    digest: CheckedNeedProducerAdmissionDigest,
}

impl CheckedNeedProducerAdmission {
    #[must_use]
    pub const fn site(&self) -> NeedProducerSiteDigest {
        self.site
    }

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
    SemanticTranscript(#[from] crate::final_analysis::CheckedSemanticTranscriptError),
    #[error("internal thread producer requires an accepted Thread expression")]
    NotThread,
    #[error(transparent)]
    GenericScope(#[from] GenericScopeError),
    #[error(transparent)]
    Generation(Box<crate::final_analysis::FinalSemanticAnalysisError>),
    #[error("selected callable join is missing")]
    MissingCallableJoin,
    #[error("producer semantic transcript byte accounting overflow")]
    TranscriptArithmeticOverflow,
    #[error("producer semantic transcript byte limit {limit} exceeded by attempt {attempted}")]
    TranscriptLimitExceeded { limit: u64, attempted: u64 },
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
    #[error("selected Need producer call site has no canonical checked coordinate")]
    SiteEncoding,
}

impl From<crate::final_analysis::FinalSemanticAnalysisError> for CheckedNeedProducerAdmissionError {
    fn from(error: crate::final_analysis::FinalSemanticAnalysisError) -> Self {
        Self::Generation(Box::new(error))
    }
}

impl From<TranscriptWriteError> for CheckedNeedProducerAdmissionError {
    fn from(error: TranscriptWriteError) -> Self {
        match error {
            TranscriptWriteError::ArithmeticOverflow => Self::TranscriptArithmeticOverflow,
            TranscriptWriteError::LimitExceeded { limit, attempted } => {
                Self::TranscriptLimitExceeded { limit, attempted }
            }
        }
    }
}

impl FinalSemanticAnalysis {
    pub fn checked_thread_producer_admission(
        &self,
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        owner: ExprId,
    ) -> Result<CheckedExpressionProducerDefinition, CheckedNeedProducerAdmissionError> {
        self.validate_generation(project, symbols)?;
        let expression = project
            .modules()
            .find_map(|(_, module)| {
                (module.module_id() == owner.module())
                    .then(|| module.resolve_expr(owner).ok())
                    .flatten()
            })
            .ok_or(CheckedNeedProducerAdmissionError::NotThread)?;
        if !matches!(
            expression.kind(),
            arcweft_lang_hir::expr::HirExprKind::Thread(_)
        ) {
            return Err(CheckedNeedProducerAdmissionError::NotThread);
        }
        self.checked_expression_producer_definition(project, symbols, owner)
    }

    pub fn checked_call_producer_definition(
        &self,
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        owner: ExprId,
    ) -> Result<CheckedExpressionProducerDefinition, CheckedNeedProducerAdmissionError> {
        self.validate_generation(project, symbols)?;
        if self
            .call(owner)
            .and_then(|call| call.selected_application())
            .is_none()
        {
            return Err(CheckedNeedProducerAdmissionError::NotSelectedCall);
        }
        self.checked_expression_producer_definition(project, symbols, owner)
    }

    fn checked_expression_producer_definition(
        &self,
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        owner: ExprId,
    ) -> Result<CheckedExpressionProducerDefinition, CheckedNeedProducerAdmissionError> {
        let coordinates = crate::semantic_coordinate::SemanticCoordinateIndex::new(
            self.accepted_root_catalog(),
            self,
        );
        let coordinate = coordinates
            .expression(owner)
            .map_err(|_| CheckedNeedProducerAdmissionError::SiteEncoding)?;
        let bytes = coordinate
            .canonical_bytes()
            .map_err(|_| CheckedNeedProducerAdmissionError::SiteEncoding)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.need.producer-site.v1\0");
        hasher.update(
            &u32::try_from(bytes.len())
                .map_err(|_| CheckedNeedProducerAdmissionError::SiteEncoding)?
                .to_le_bytes(),
        );
        hasher.update(&bytes);
        let plan = self.checked_expression_semantic_digest(
            project,
            symbols,
            owner,
            crate::final_analysis::CheckedMatchLimits::PRODUCTION,
        )?;
        Ok(CheckedExpressionProducerDefinition {
            site: NeedProducerSiteDigest::from_bytes(*hasher.finalize().as_bytes()),
            plan: arcweft_core::task::TaskPlanSemanticDigest::from_bytes(*plan.as_bytes()),
        })
    }

    /// Derives the exact source-ordered semantic retention certificate for a
    /// direct selected producer call.
    ///
    /// Calls with a receiver/function-value capture, spreads, compact numeric
    /// slots, recovery, or a value requiring a live Need/Function certificate
    /// fail closed in this cut.
    pub fn checked_need_producer_admission_for_call(
        &self,
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        call: ExprId,
        limits: CheckedOwnershipLimits,
    ) -> Result<CheckedNeedProducerAdmission, CheckedNeedProducerAdmissionError> {
        let (site, values) =
            self.checked_producer_argument_values(project, symbols, call, limits)?;
        let types = values.iter().map(|(_, ty)| *ty).collect::<Vec<_>>();
        let (dispositions, ownership) =
            classify_checked_producer_arguments(self, world, &types, limits)?;
        let arguments = values
            .into_iter()
            .zip(dispositions)
            .map(|((coordinate, ty), disposition)| {
                Ok::<_, GenericScopeError>(CheckedProducerArgumentAdmission {
                    coordinate,
                    ty: ty.semantic_identity_digest()?,
                    disposition,
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let digest = need_producer_admission_digest(&arguments, ownership.evidence())?;
        Ok(CheckedNeedProducerAdmission {
            site,
            arguments,
            ownership,
            digest,
        })
    }

    fn checked_producer_argument_values<'a>(
        &'a self,
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        call: ExprId,
        limits: CheckedOwnershipLimits,
    ) -> Result<
        (
            NeedProducerSiteDigest,
            Vec<(StableCheckedValueCoordinate, &'a TypeKind)>,
        ),
        CheckedNeedProducerAdmissionError,
    > {
        self.validate_generation(project, symbols)
            .map_err(CheckedNeedProducerAdmissionError::from)?;
        let facts = self
            .call(call)
            .ok_or(CheckedNeedProducerAdmissionError::NotSelectedCall)?;
        let Some(application) = facts.selected_application() else {
            return Err(CheckedNeedProducerAdmissionError::NotSelectedCall);
        };
        let core = application.core();
        let site_bytes = core
            .stable_site()
            .canonical_bytes()
            .map_err(|_| CheckedNeedProducerAdmissionError::SiteEncoding)?;
        let mut site_hasher = blake3::Hasher::new();
        site_hasher.update(b"arcweft.need.producer-site.v1\0");
        site_hasher.update(
            &u32::try_from(site_bytes.len())
                .map_err(|_| CheckedNeedProducerAdmissionError::SiteEncoding)?
                .to_le_bytes(),
        );
        site_hasher.update(&site_bytes);
        let site = NeedProducerSiteDigest::from_bytes(*site_hasher.finalize().as_bytes());
        self.checked_callable_join(call)
            .map_err(|_| CheckedNeedProducerAdmissionError::MissingCallableJoin)?;
        if !matches!(core.callee(), CheckedCallCalleeExecution::Direct) {
            return Err(CheckedNeedProducerAdmissionError::UnsupportedCapture);
        }
        let execution = core.execution();
        let operands = core.runtime_operands();
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
        Ok((site, values))
    }
}

fn need_producer_admission_digest(
    arguments: &[CheckedProducerArgumentAdmission],
    evidence: OwnershipEvidenceDigest,
) -> Result<CheckedNeedProducerAdmissionDigest, CheckedNeedProducerAdmissionError> {
    let mut required = u64::try_from(b"arcweft.lang.need-producer-admission.v1\0".len())
        .map_err(|_| TranscriptWriteError::ArithmeticOverflow)?
        .checked_add(8)
        .ok_or(TranscriptWriteError::ArithmeticOverflow)?;
    for argument in arguments {
        let coordinate = argument
            .coordinate()
            .canonical_bytes()
            .map_err(|_| CheckedNeedProducerAdmissionError::TranscriptArithmeticOverflow)?;
        required = required
            .checked_add(
                u64::try_from(coordinate.len())
                    .map_err(|_| TranscriptWriteError::ArithmeticOverflow)?,
            )
            .and_then(|value| value.checked_add(32 + 1))
            .ok_or(TranscriptWriteError::ArithmeticOverflow)?;
    }
    required = required
        .checked_add(32)
        .ok_or(TranscriptWriteError::ArithmeticOverflow)?;
    let mut budget = CheckedTranscriptByteBudget::exact(required);
    let mut hasher = TranscriptHasher::new(&mut budget);
    hasher.update(b"arcweft.lang.need-producer-admission.v1\0")?;
    write_len(&mut hasher, arguments.len())?;
    for argument in arguments {
        hasher.update(
            &argument
                .coordinate()
                .canonical_bytes()
                .map_err(|_| CheckedNeedProducerAdmissionError::TranscriptArithmeticOverflow)?,
        )?;
        hasher.update(argument.ty().as_bytes())?;
        hasher.update(&[argument.disposition().semantic_tag()])?;
    }
    hasher.update(evidence.as_bytes())?;
    Ok(CheckedNeedProducerAdmissionDigest::from_bytes(
        hasher.finalize(),
    ))
}
