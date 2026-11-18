//! Actual source terms retain their checked or staged value owner.

use super::*;
use crate::final_analysis::accounting::PhysicalSourceType;
use std::sync::OnceLock;

pub(in crate::final_analysis::analyzer) struct AnalyzerObservedSourceValue {
    origin: AnalyzerObservedSourceOrigin,
    original: crate::types::constraints::OriginalProducerContext,
    value: ObservedValue,
    physical: OnceLock<PhysicalSourceType>,
}

enum ObservedValue {
    Expression(crate::final_analysis::PreparedExpressionFact),
    Prepared(Arc<AnalyzerPreparedSourceActual>),
    #[cfg(test)]
    Fixture(TypeKind<crate::types::PreparedEffects>),
}

pub(super) enum AnalyzerObservedSourceOrigin {
    Probe(crate::types::constraints::ConstraintSourceReceipt<AnalyzerCallConstraintDomain>),
    Check(AnalyzerCallConstraintSourceId),
}

impl AnalyzerObservedSourceOrigin {
    fn source(&self) -> AnalyzerCallConstraintSourceId {
        match self {
            Self::Probe(receipt) => receipt.source(),
            Self::Check(source) => *source,
        }
    }
}

impl AnalyzerObservedSourceValue {
    pub(super) fn expression(
        origin: AnalyzerObservedSourceOrigin,
        original: crate::types::constraints::OriginalProducerContext,
        value: crate::final_analysis::PreparedExpressionFact,
    ) -> Option<Arc<Self>> {
        value.value_type()?;
        Some(Arc::new(Self {
            origin,
            original,
            value: ObservedValue::Expression(value),
            physical: OnceLock::new(),
        }))
    }

    pub(super) fn prepared(
        origin: AnalyzerObservedSourceOrigin,
        original: crate::types::constraints::OriginalProducerContext,
        value: Arc<AnalyzerPreparedSourceActual>,
    ) -> Option<Arc<Self>> {
        value.validates(origin.source().local()).then(|| {
            Arc::new(Self {
                origin,
                original,
                value: ObservedValue::Prepared(value),
                physical: OnceLock::new(),
            })
        })
    }

    pub(super) fn actual(&self) -> &TypeKind<crate::types::PreparedEffects> {
        match &self.value {
            // The private constructor rejects non-value expression facts.
            ObservedValue::Expression(value) => {
                value.value_type().expect("retained checked value fact")
            }
            ObservedValue::Prepared(value) => value.actual(),
            #[cfg(test)]
            ObservedValue::Fixture(value) => value,
        }
    }

    pub(super) fn expression_fact(&self) -> Option<&crate::final_analysis::PreparedExpressionFact> {
        match &self.value {
            ObservedValue::Expression(fact) => Some(fact),
            ObservedValue::Prepared(_) => None,
            #[cfg(test)]
            ObservedValue::Fixture(_) => None,
        }
    }

    pub(super) fn record_physical_type(
        &self,
        attempt: &PhysicalCallAttemptId,
        value_owner: ExprId,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
        work: &mut AnalyzerPreparationWork<'_>,
    ) -> Result<PhysicalSourceType, FinalSemanticAnalysisError> {
        work.visit_binding()?;
        let prepared = self
            .actual()
            .retain_effects(self.original.occurrence_scope(), graph, work)
            .map_err(|error| {
                FinalSemanticAnalysisError::source_prepared_type_retention(
                    value_owner.into(),
                    error,
                )
            })?;
        let source = PhysicalSourceType::new(attempt.root(), prepared);
        self.physical.set(source.clone()).map_err(|_| {
            crate::final_analysis::CandidateFactTransactionViolation::PhysicalCandidateTypeObservationConflict {
                owner: attempt.owner(),
            }
        })?;
        Ok(source)
    }

    pub(super) fn physical_type(&self) -> Option<&PhysicalSourceType> {
        self.physical.get()
    }

    pub(super) fn is_probed_source(&self, source: AnalyzerCallConstraintSourceId) -> bool {
        matches!(&self.origin, AnalyzerObservedSourceOrigin::Probe(receipt) if receipt.source() == source)
    }

    pub(super) fn staged_value(&self) -> Option<&Arc<AnalyzerPreparedSourceActual>> {
        match &self.value {
            ObservedValue::Prepared(value) => Some(value),
            ObservedValue::Expression(_) => None,
            #[cfg(test)]
            ObservedValue::Fixture(_) => None,
        }
    }

    #[cfg(test)]
    pub(super) fn fixture(
        source: AnalyzerCallConstraintSourceId,
        value: TypeKind<crate::types::PreparedEffects>,
    ) -> Arc<Self> {
        Arc::new(Self {
            origin: AnalyzerObservedSourceOrigin::Check(source),
            original: fixture_original_context(),
            value: ObservedValue::Fixture(value),
            physical: OnceLock::new(),
        })
    }
}

impl crate::types::constraints::ConstraintSourceValueOwner<AnalyzerCallConstraintDomain>
    for AnalyzerObservedSourceValue
{
    fn source(&self) -> AnalyzerCallConstraintSourceId {
        self.origin.source()
    }
    fn original_context(&self) -> &crate::types::constraints::OriginalProducerContext {
        &self.original
    }
    fn probe_receipt(
        &self,
    ) -> Option<&crate::types::constraints::ConstraintSourceReceipt<AnalyzerCallConstraintDomain>>
    {
        match &self.origin {
            AnalyzerObservedSourceOrigin::Probe(receipt) => Some(receipt),
            AnalyzerObservedSourceOrigin::Check(_) => None,
        }
    }
    fn value_type(&self) -> &TypeKind<crate::types::PreparedEffects> {
        self.actual()
    }
}

#[cfg(test)]
pub(super) fn fixture_original_context() -> crate::types::constraints::OriginalProducerContext {
    use crate::types::constraints::context::{
        LocalConstraintAccounting, TypeConstraintContext, TypeConstraintLimits,
    };
    let cancellation = std::sync::atomic::AtomicBool::new(false);
    let mut context = TypeConstraintContext::<_, AnalyzerCallConstraintDomain>::with_accounting(
        LocalConstraintAccounting::new(
            TypeConstraintLimits::new(4096, 4096, 128, 128),
            &cancellation,
        ),
    );
    context.fixture_original_context(None).unwrap()
}
