//! Focused retention and caller-owned control for checker-produced call facts.

use std::sync::atomic::AtomicBool;

use arcweft_lang_syntax::{
    ast::common::TextRange,
    expr::{ArgumentListTerminatorSyntax, CallArgumentRecoverySyntax, CallExpr},
};
use arcweft_source::{SourceDocument, SourceRange, SourceSpan};

use crate::callable::{
    CallTargetFactError, CallTargetFactMode, CallTargetFacts, PRODUCTION_CALLABLE_LIMITS,
    ResolveCallError, ResolverWork, SemanticSignatureError,
};

static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CallTargetFactReport {
    pub(super) mode: CallTargetFactMode,
    pub(super) site: Option<FocusedCallSite>,
    pub(super) unsupported_surface: bool,
    pub(super) fact: Option<CallTargetFacts>,
    pub(super) error: Option<CallTargetFactError>,
}

impl Default for CallTargetFactReport {
    fn default() -> Self {
        Self {
            mode: CallTargetFactMode::Disabled,
            site: None,
            unsupported_surface: false,
            fact: None,
            error: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FocusedCallSite {
    call: SourceSpan,
    callee: SourceSpan,
    arguments: SourceSpan,
    active_argument: Option<usize>,
    recovery_nodes: usize,
    missing_close_delimiter: bool,
    argument_content: TextRange,
    open_paren_start: usize,
}

impl FocusedCallSite {
    pub(crate) const fn call(&self) -> &SourceSpan {
        &self.call
    }

    pub(crate) const fn arguments(&self) -> &SourceSpan {
        &self.arguments
    }

    pub(crate) const fn callee(&self) -> &SourceSpan {
        &self.callee
    }

    pub(crate) const fn active_argument(&self) -> Option<usize> {
        self.active_argument
    }

    pub(crate) const fn recovery_nodes(&self) -> usize {
        self.recovery_nodes
    }

    pub(crate) const fn missing_close_delimiter(&self) -> bool {
        self.missing_close_delimiter
    }
}

pub(super) struct CallTargetFactRecorder {
    mode: CallTargetFactMode,
    site: Option<FocusedCallSite>,
    unsupported_surface: bool,
    fact: Option<CallTargetFacts>,
    error: Option<CallTargetFactError>,
}

impl CallTargetFactRecorder {
    pub(super) fn new(mode: CallTargetFactMode) -> Self {
        Self {
            mode,
            site: None,
            unsupported_surface: false,
            fact: None,
            error: None,
        }
    }

    pub(super) fn observe_call(&mut self, call: &CallExpr, document: &SourceDocument) {
        if self.error.is_some() && !matches!(&self.mode, CallTargetFactMode::Cursor { .. }) {
            return;
        }
        match &self.mode {
            CallTargetFactMode::Disabled => {}
            #[cfg(test)]
            CallTargetFactMode::Focused { call: focused } => {
                if self.site.is_some() || focused.source() != document.identity() {
                    return;
                }
                let Ok(call_span) =
                    document.span(SourceRange::new(call.range().start(), call.range().end()))
                else {
                    return;
                };
                if &call_span == focused
                    && let Some(site) = focused_site(call, document, None)
                {
                    self.site = Some(site);
                }
            }
            CallTargetFactMode::Cursor {
                document: focused_document,
                byte_offset,
            } => {
                if focused_document != document.identity()
                    || call.range().start() > *byte_offset
                    || call.range().end() < *byte_offset
                {
                    return;
                }
                let Some(arguments) = call
                    .parenthesized_syntax()
                    .map(arcweft_lang_syntax::expr::ParenthesizedCallSyntax::argument_list)
                else {
                    self.unsupported_surface |= call.callback_block_syntax().is_some();
                    return;
                };
                if !arguments.contains_signature_cursor(*byte_offset) {
                    self.unsupported_surface |= call.callback_block_syntax().is_some();
                    return;
                }
                let Some(candidate) = focused_site(call, document, Some(*byte_offset)) else {
                    return;
                };
                let Some(current) = self.site.as_ref() else {
                    self.site = Some(candidate);
                    return;
                };
                match compare_call_site(&candidate, current) {
                    std::cmp::Ordering::Greater => {
                        self.site = Some(candidate);
                        self.fact = None;
                        self.error = None;
                    }
                    std::cmp::Ordering::Less => {}
                    std::cmp::Ordering::Equal
                        if candidate.call == current.call
                            && candidate.arguments == current.arguments => {}
                    std::cmp::Ordering::Equal => {
                        self.error = Some(CallTargetFactError::AmbiguousCallRange {
                            document: focused_document.clone(),
                            byte_offset: *byte_offset,
                        });
                    }
                }
            }
        }
    }

    pub(super) fn wants(&self, call_span: Option<&SourceSpan>) -> bool {
        match &self.mode {
            CallTargetFactMode::Disabled => false,
            #[cfg(test)]
            CallTargetFactMode::Focused { call } => call_span == Some(call),
            CallTargetFactMode::Cursor { .. } => {
                self.site.as_ref().map(FocusedCallSite::call) == call_span
            }
        }
    }

    pub(super) fn record(&mut self, facts: CallTargetFacts) {
        if !self.wants(Some(facts.call_span())) || self.error.is_some() {
            return;
        }
        #[cfg(test)]
        {
            if let CallTargetFactMode::Focused { call } = &self.mode
                && self.fact.is_some()
            {
                self.error =
                    Some(CallTargetFactError::FocusedTargetDuplicate { call: call.clone() });
                return;
            }
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
            site: self.site,
            unsupported_surface: self.unsupported_surface,
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
    Caller {
        focused: &'a mut ResolverWork,
        ordinary: ResolverWork,
    },
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
            work: ResolverWorkOwner::Caller {
                focused: work,
                ordinary: ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work()),
            },
        }
    }

    pub(super) fn parts(&mut self, focused: bool) -> (&AtomicBool, &mut ResolverWork) {
        let work = match &mut self.work {
            ResolverWorkOwner::Ordinary(work) => {
                work.reset();
                return (&NEVER_CANCELLED, work);
            }
            ResolverWorkOwner::Caller {
                focused: caller,
                ordinary,
            } if focused => {
                return (self.cancellation, caller);
            }
            ResolverWorkOwner::Caller { ordinary, .. } => {
                ordinary.reset();
                ordinary
            }
        };
        (&NEVER_CANCELLED, work)
    }
}

fn compare_call_site(candidate: &FocusedCallSite, current: &FocusedCallSite) -> std::cmp::Ordering {
    if strictly_contains(current.argument_content, candidate.argument_content) {
        return std::cmp::Ordering::Greater;
    }
    if strictly_contains(candidate.argument_content, current.argument_content) {
        return std::cmp::Ordering::Less;
    }
    range_len(current.argument_content)
        .cmp(&range_len(candidate.argument_content))
        .then_with(|| candidate.open_paren_start.cmp(&current.open_paren_start))
}

fn range_len(range: TextRange) -> usize {
    range.end().saturating_sub(range.start())
}

fn strictly_contains(outer: TextRange, inner: TextRange) -> bool {
    outer.start() <= inner.start()
        && inner.end() <= outer.end()
        && (outer.start() < inner.start() || inner.end() < outer.end())
}

fn focused_site(
    call: &CallExpr,
    document: &SourceDocument,
    byte_offset: Option<usize>,
) -> Option<FocusedCallSite> {
    let syntax = call.parenthesized_syntax()?;
    let arguments = syntax.argument_list();
    let call_span = document
        .span(SourceRange::new(call.range().start(), call.range().end()))
        .ok()?;
    let callee = document
        .span(SourceRange::new(
            call.callee_range().start(),
            call.callee_range().end(),
        ))
        .ok()?;
    let arguments_span = document
        .span(SourceRange::new(
            arguments.range().start(),
            arguments.range().end(),
        ))
        .ok()?;
    let recovered_arguments = arguments
        .arguments()
        .iter()
        .filter(|argument| {
            matches!(
                argument.recovery(),
                CallArgumentRecoverySyntax::Recovered { .. }
            )
        })
        .count();
    let missing_close_delimiter = matches!(
        arguments.terminator(),
        ArgumentListTerminatorSyntax::RecoveredMissing { .. }
    );
    Some(FocusedCallSite {
        call: call_span,
        callee,
        arguments: arguments_span,
        active_argument: byte_offset.and_then(|offset| arguments.active_argument_slot(offset)),
        recovery_nodes: recovered_arguments + usize::from(missing_close_delimiter),
        missing_close_delimiter,
        argument_content: arguments.content_range(),
        open_paren_start: arguments.open_paren().start(),
    })
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
        let (_, work) = control.parts(false);
        assert_eq!(work.limit(), PRODUCTION_CALLABLE_LIMITS.max_query_work());
        work.charge(1).expect("one work unit");
        assert_eq!(work.consumed(), 1);

        let (_, work) = control.parts(false);
        assert_eq!(work.limit(), PRODUCTION_CALLABLE_LIMITS.max_query_work());
        assert_eq!(work.consumed(), 0);
    }

    #[test]
    fn caller_control_charges_only_the_focused_call() {
        use std::sync::atomic::AtomicBool;

        use crate::callable::ResolverWork;

        let cancelled = AtomicBool::new(false);
        let mut caller = ResolverWork::new(7);
        let mut control = CallResolverControl::caller_owned(&cancelled, &mut caller);

        let (_, ordinary) = control.parts(false);
        ordinary.charge(3).expect("ordinary work");
        let (_, ordinary) = control.parts(false);
        assert_eq!(ordinary.consumed(), 0);

        let (_, focused) = control.parts(true);
        focused.charge(2).expect("focused work");
        let (_, focused) = control.parts(true);
        assert_eq!(focused.consumed(), 2);
        assert_eq!(caller.consumed(), 2);
    }
}
