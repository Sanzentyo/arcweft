//! Checked Need-producer argument admission composer.
//!
//! This module joins current HIR/call facts, stable semantic coordinates, and
//! the ownership classifier. It deliberately does not own a second type
//! classifier or expose caller-constructed admission rows.

use arcweft_lang_hir::{
    expr::{HirCallArgument, HirExprKind},
    identity::ExprId,
    project::HirExecutableProjectView,
    symbol::{CallableDeclarationKey, ProjectSymbolTable},
};
use thiserror::Error;

use crate::{
    callable::{
        CallCalleeClassificationFact, CallPoison, CallTargetFact, CheckedCallArgumentSlotSource,
    },
    env::RegisteredSemanticWorld,
    final_analysis::{
        CheckedExpression, CheckedExpressionResolution, CheckedValueResolution,
        FinalSemanticAnalysis, SemanticTranscriptError, StableCheckedValueCoordinate,
        TranscriptHasher, accepted_declaration_id, checked_expression_path, write_len,
        write_value_coordinate,
    },
    ownership::{
        CheckedOwnershipCertificate, CheckedOwnershipError, CheckedOwnershipLimits,
        OwnershipEvidenceDigest, RetainedValueDisposition, classify_checked_producer_arguments,
    },
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
        declaration: &CallableDeclarationKey,
        call: ExprId,
        limits: CheckedOwnershipLimits,
    ) -> Result<CheckedNeedProducerAdmission, CheckedNeedProducerAdmissionError> {
        let values =
            self.checked_producer_argument_values(project, symbols, declaration, call, limits)?;
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
        declaration: &CallableDeclarationKey,
        call: ExprId,
        limits: CheckedOwnershipLimits,
    ) -> Result<Vec<(StableCheckedValueCoordinate, &'a TypeKind)>, CheckedNeedProducerAdmissionError>
    {
        self.validate_generation(project, symbols)
            .map_err(SemanticTranscriptError::from)?;
        let module = project
            .modules()
            .find_map(|(_, module)| {
                (module.module_id() == call.module()).then_some(module.as_ref())
            })
            .ok_or(SemanticTranscriptError::MissingExpression)?;
        let expression = module
            .resolve_expr(call)
            .map_err(|_| SemanticTranscriptError::MissingExpression)?;
        let HirExprKind::Call(authored) = expression.kind() else {
            return Err(CheckedNeedProducerAdmissionError::NotSelectedCall);
        };
        if u64::try_from(authored.arguments().len()).unwrap_or(u64::MAX)
            > limits.max_producer_arguments
        {
            return Err(CheckedNeedProducerAdmissionError::WorkLimit);
        }
        if !matches!(
            self.expression(call).map(CheckedExpression::resolution),
            Some(CheckedExpressionResolution::Call)
        ) {
            return Err(CheckedNeedProducerAdmissionError::NotSelectedCall);
        }
        let facts = self
            .call(call)
            .ok_or(CheckedNeedProducerAdmissionError::NotSelectedCall)?;
        if !matches!(facts.target(), CallTargetFact::Selected { .. })
            || facts.poison() != CallPoison::Clean
        {
            return Err(CheckedNeedProducerAdmissionError::NotSelectedCall);
        }
        self.checked_callable_join(call)
            .map_err(|_| SemanticTranscriptError::MissingCallableJoin)?;
        if facts.function_value_type().is_some()
            || matches!(
                facts.callee(),
                Some(CallCalleeClassificationFact::Value { expression })
                    if !matches!(
                        self.expression(expression).map(CheckedExpression::resolution),
                        Some(CheckedExpressionResolution::Value(
                            CheckedValueResolution::ProjectCallable(_)
                        ))
                    )
            )
        {
            return Err(CheckedNeedProducerAdmissionError::UnsupportedCapture);
        }
        if authored.arguments().len() != facts.arguments().len() {
            return Err(CheckedNeedProducerAdmissionError::UnsupportedArgumentInventory);
        }
        let paths = project
            .declaration_semantic_paths(symbols, declaration)
            .map_err(|_| SemanticTranscriptError::MissingIdentity)?;
        let declaration = accepted_declaration_id(self, declaration)?;
        let mut values = Vec::with_capacity(authored.arguments().len());
        for (ordinal, (authored, fact)) in authored
            .arguments()
            .iter()
            .zip(facts.arguments())
            .enumerate()
        {
            if matches!(authored, HirCallArgument::Spread { .. })
                || usize::from(fact.argument().get()) != ordinal
                || fact.poison() != CallPoison::Clean
            {
                return Err(CheckedNeedProducerAdmissionError::UnsupportedArgumentInventory);
            }
            let [slot] = fact.slots() else {
                return Err(CheckedNeedProducerAdmissionError::UnsupportedArgumentInventory);
            };
            let CheckedCallArgumentSlotSource::Expression(source) = slot.source() else {
                return Err(CheckedNeedProducerAdmissionError::UnsupportedArgumentInventory);
            };
            let checked = self
                .expression(source)
                .ok_or(SemanticTranscriptError::MissingExpression)?;
            if source != authored.value()
                || slot.poison() != CallPoison::Clean
                || slot.inferred() != Some(checked.ty())
            {
                return Err(CheckedNeedProducerAdmissionError::UnsupportedArgumentInventory);
            }
            let path = checked_expression_path(self, &paths, declaration, source)?;
            values.push((
                StableCheckedValueCoordinate::Expression { declaration, path },
                checked.ty(),
            ));
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
        write_value_coordinate(&mut hasher, argument.coordinate());
        hasher.update(argument.ty().as_bytes());
        hasher.update(&[argument.disposition().semantic_tag()]);
    }
    hasher.update(evidence.as_bytes());
    Ok(CheckedNeedProducerAdmissionDigest::from_bytes(
        hasher.finalize()?,
    ))
}
