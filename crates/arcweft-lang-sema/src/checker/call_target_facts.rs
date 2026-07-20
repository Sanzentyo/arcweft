//! Focused retention and caller-owned control for checker-produced call facts.
#![allow(
    dead_code,
    reason = "the caller-owned focused path is consumed by the following native query cut"
)]

use std::sync::atomic::AtomicBool;

use arcweft_source::SourceSpan;

use crate::callable::{
    CallTargetFactError, CallTargetFactMode, CallTargetFacts, PRODUCTION_CALLABLE_LIMITS,
    ResolveCallError, ResolverWork, SemanticSignatureError,
};

static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CallTargetFactReport {
    pub(super) mode: CallTargetFactMode,
    pub(super) fact: Option<CallTargetFacts>,
    pub(super) error: Option<CallTargetFactError>,
}

impl Default for CallTargetFactReport {
    fn default() -> Self {
        Self {
            mode: CallTargetFactMode::Disabled,
            fact: None,
            error: None,
        }
    }
}

pub(super) struct CallTargetFactRecorder {
    mode: CallTargetFactMode,
    fact: Option<CallTargetFacts>,
    error: Option<CallTargetFactError>,
}

impl CallTargetFactRecorder {
    pub(super) fn new(mode: CallTargetFactMode) -> Self {
        Self {
            mode,
            fact: None,
            error: None,
        }
    }

    pub(super) fn wants(&self, call_span: Option<&SourceSpan>) -> bool {
        match &self.mode {
            CallTargetFactMode::Disabled => false,
            CallTargetFactMode::Focused { call } => call_span == Some(call),
        }
    }

    pub(super) fn record(&mut self, facts: CallTargetFacts) {
        if !self.wants(Some(facts.call_span())) || self.error.is_some() {
            return;
        }
        if let CallTargetFactMode::Focused { call } = &self.mode
            && self.fact.is_some()
        {
            self.error = Some(CallTargetFactError::FocusedTargetDuplicate { call: call.clone() });
            return;
        }
        self.fact = Some(facts);
    }

    pub(super) fn record_unavailable(&mut self, call: &SourceSpan, reason: SemanticSignatureError) {
        if self.wants(Some(call)) && self.error.is_none() {
            self.error = Some(CallTargetFactError::Unavailable {
                call: call.clone(),
                reason,
            });
        }
    }

    pub(super) fn record_resolve_error(
        &mut self,
        call: Option<&SourceSpan>,
        reason: ResolveCallError,
    ) {
        if self.wants(call)
            && self.error.is_none()
            && let Some(call) = call
        {
            self.error = Some(CallTargetFactError::Resolve {
                call: call.clone(),
                reason: Box::new(reason),
            });
        }
    }

    pub(super) fn finish(self) -> CallTargetFactReport {
        CallTargetFactReport {
            mode: self.mode,
            fact: self.fact,
            error: self.error,
        }
    }
}

pub(super) struct CallResolverControl<'a> {
    cancellation: &'a AtomicBool,
    work: ResolverWorkOwner<'a>,
}

enum ResolverWorkOwner<'a> {
    Ordinary(ResolverWork),
    Caller(&'a mut ResolverWork),
}

impl CallResolverControl<'static> {
    pub(super) fn ordinary() -> Self {
        Self {
            cancellation: &NEVER_CANCELLED,
            work: ResolverWorkOwner::Ordinary(ResolverWork::new(
                PRODUCTION_CALLABLE_LIMITS.max_query_work(),
            )),
        }
    }
}

impl<'a> CallResolverControl<'a> {
    pub(super) fn caller_owned(cancellation: &'a AtomicBool, work: &'a mut ResolverWork) -> Self {
        Self {
            cancellation,
            work: ResolverWorkOwner::Caller(work),
        }
    }

    pub(super) fn parts(&mut self) -> (&AtomicBool, &mut ResolverWork) {
        let work = match &mut self.work {
            ResolverWorkOwner::Ordinary(work) => {
                work.reset();
                work
            }
            ResolverWorkOwner::Caller(work) => work,
        };
        (self.cancellation, work)
    }
}

#[cfg(test)]
mod tests {
    use super::{CallResolverControl, CallTargetFactMode, CallTargetFactRecorder};
    use crate::callable::PRODUCTION_CALLABLE_LIMITS;

    #[test]
    fn disabled_recorder_never_requests_or_retains_fact_storage() {
        let recorder = CallTargetFactRecorder::new(CallTargetFactMode::Disabled);
        assert!(!recorder.wants(None));

        let report = recorder.finish();
        assert_eq!(report.mode, CallTargetFactMode::Disabled);
        assert!(report.fact.is_none());
        assert!(report.error.is_none());
    }

    #[test]
    fn ordinary_control_reuses_one_production_bounded_work_counter_per_call() {
        let mut control = CallResolverControl::ordinary();
        let (_, work) = control.parts();
        assert_eq!(work.limit(), PRODUCTION_CALLABLE_LIMITS.max_query_work());
        work.charge(1).expect("one work unit");
        assert_eq!(work.consumed(), 1);

        let (_, work) = control.parts();
        assert_eq!(work.limit(), PRODUCTION_CALLABLE_LIMITS.max_query_work());
        assert_eq!(work.consumed(), 0);
    }
}
