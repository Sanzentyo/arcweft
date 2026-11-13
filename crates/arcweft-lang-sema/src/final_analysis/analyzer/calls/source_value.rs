//! Actual source terms retain their checked or staged value owner.

use super::*;

pub(super) struct AnalyzerObservedSourceValue {
    origin: AnalyzerObservedSourceOrigin,
    original: crate::types::constraints::OriginalProducerContext,
    value: ObservedValue,
}

enum ObservedValue {
    Expression(crate::final_analysis::PreparedExpressionFact),
    Prepared(AnalyzerPreparedSourceActual),
    #[cfg(test)]
    Fixture(TypeKind),
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
        }))
    }

    pub(super) fn prepared(
        origin: AnalyzerObservedSourceOrigin,
        original: crate::types::constraints::OriginalProducerContext,
        value: AnalyzerPreparedSourceActual,
    ) -> Option<Arc<Self>> {
        value.validates(origin.source().local()).then(|| {
            Arc::new(Self {
                origin,
                original,
                value: ObservedValue::Prepared(value),
            })
        })
    }

    pub(super) fn actual(&self) -> &TypeKind {
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

    #[cfg(test)]
    pub(super) fn fixture(source: AnalyzerCallConstraintSourceId, value: TypeKind) -> Arc<Self> {
        Arc::new(Self {
            origin: AnalyzerObservedSourceOrigin::Check(source),
            original: fixture_original_context(),
            value: ObservedValue::Fixture(value),
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
    fn value_type(&self) -> &TypeKind {
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
