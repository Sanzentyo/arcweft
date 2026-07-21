//! Whole-module and focused retention with caller-owned control for checker-produced call facts.

use std::{
    cell::RefCell,
    collections::{BTreeMap, btree_map::Entry},
    rc::Rc,
    sync::atomic::AtomicBool,
};

use arcweft_lang_syntax::{
    ast::common::TextRange,
    expr::{ArgumentListTerminatorSyntax, CallArgumentRecoverySyntax, CallExpr},
};
use arcweft_source::{SourceDocument, SourceRange, SourceSpan};

use crate::callable::{
    CallTargetFactError, CallTargetFactMode, CallTargetFacts, CheckedCallTarget,
    PRODUCTION_CALLABLE_LIMITS, ResolveCallError, ResolverWork, SemanticSignatureError,
    SignatureAccountingError, SignatureQueryStep, SignatureQueryStepControl,
    SignatureQueryWorkMeter, SignatureWorkKind,
};

static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);

pub(crate) struct SignatureFocusedAnalysis<'a> {
    pub(crate) module: &'a arcweft_lang_hir::model::HirModule,
    pub(crate) registered: &'a crate::registration::RegisteredSemanticWorld,
    pub(crate) site: FocusedCallSite,
    pub(crate) cancellation: &'a AtomicBool,
    pub(crate) work: &'a mut ResolverWork,
    pub(crate) signature_work: &'a mut SignatureQueryWorkMeter,
    pub(crate) signature_control: &'a dyn SignatureQueryStepControl,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CallTargetFactReport {
    pub(super) mode: CallTargetFactMode,
    pub(super) site: Option<FocusedCallSite>,
    pub(super) facts: BTreeMap<crate::checker::TypeExpressionId, CallTargetFacts>,
    pub(super) error: Option<CallTargetFactError>,
}

impl Default for CallTargetFactReport {
    fn default() -> Self {
        Self {
            mode: CallTargetFactMode::Disabled,
            site: None,
            facts: BTreeMap::new(),
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
    byte_offset: Option<usize>,
}

impl FocusedCallSite {
    pub(crate) fn from_call(
        call: &CallExpr,
        document: &SourceDocument,
        byte_offset: usize,
    ) -> Option<Self> {
        focused_site(call, document, Some(byte_offset))
    }

    pub(crate) fn compare_focus(&self, current: &Self) -> std::cmp::Ordering {
        compare_call_site(self, current)
    }

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

    pub(crate) const fn byte_offset(&self) -> Option<usize> {
        self.byte_offset
    }
}

#[derive(Clone)]
pub(super) struct CallTargetFactRecorder {
    mode: CallTargetFactMode,
    site: Option<FocusedCallSite>,
    facts: BTreeMap<crate::checker::TypeExpressionId, CallTargetFacts>,
    error: Option<CallTargetFactError>,
}

impl CallTargetFactRecorder {
    pub(super) fn new(mode: CallTargetFactMode) -> Self {
        Self {
            mode,
            site: None,
            facts: BTreeMap::new(),
            error: None,
        }
    }

    pub(super) fn observe_call(&mut self, call: &CallExpr, document: &SourceDocument) {
        if self.error.is_some() {
            return;
        }
        match &self.mode {
            CallTargetFactMode::Disabled | CallTargetFactMode::All => {}
            CallTargetFactMode::Focused { call: focused, .. } => {
                if focused.source() != document.identity() {
                    return;
                }
                let Ok(call_span) =
                    document.span(SourceRange::new(call.range().start(), call.range().end()))
                else {
                    return;
                };
                if &call_span != focused {
                    return;
                }
                let Some(site) = focused_site(call, document, None) else {
                    return;
                };
                if self.site.is_some() {
                    self.error = Some(CallTargetFactError::FocusedTargetDuplicate {
                        call: focused.clone(),
                    });
                    return;
                }
                self.site = Some(site);
            }
        }
    }

    pub(super) fn wants(&self, call_span: Option<&SourceSpan>) -> bool {
        match &self.mode {
            CallTargetFactMode::Disabled => false,
            CallTargetFactMode::All => call_span.is_some(),
            CallTargetFactMode::Focused { call, .. } => call_span == Some(call),
        }
    }

    pub(super) fn focuses(&self, call_span: Option<&SourceSpan>) -> bool {
        matches!(
            &self.mode,
            CallTargetFactMode::Focused { call, .. } if call_span == Some(call)
        )
    }

    pub(super) fn active_parameter(
        &self,
        checked: &CheckedCallTarget,
    ) -> Option<crate::callable::CallableParameterCoordinate> {
        match &self.mode {
            CallTargetFactMode::Disabled | CallTargetFactMode::All => None,
            CallTargetFactMode::Focused {
                active_argument,
                byte_offset,
                ..
            } => checked.active_parameter(*active_argument, *byte_offset),
        }
    }

    pub(super) fn record(&mut self, facts: CallTargetFacts) {
        if !self.wants(Some(facts.call_span())) || self.error.is_some() {
            return;
        }
        match &self.mode {
            CallTargetFactMode::Disabled => return,
            CallTargetFactMode::Focused { call, .. } if !self.facts.is_empty() => {
                self.error =
                    Some(CallTargetFactError::FocusedTargetDuplicate { call: call.clone() });
                return;
            }
            CallTargetFactMode::Focused { .. } | CallTargetFactMode::All => {}
        }
        let expression = facts.expression();
        match self.facts.entry(expression) {
            Entry::Vacant(entry) => {
                entry.insert(facts);
            }
            Entry::Occupied(_) => {
                self.error = Some(CallTargetFactError::DuplicateExpression { expression });
            }
        }
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

    pub(super) fn record_signature_accounting_error(&mut self, reason: SignatureAccountingError) {
        if self.error.is_none() {
            self.error = Some(CallTargetFactError::SignatureAccounting { reason });
        }
    }

    pub(super) fn terminal_query_error(&self) -> Option<CallTargetFactError> {
        match self.error.as_ref() {
            Some(error @ CallTargetFactError::SignatureAccounting { .. }) => Some(error.clone()),
            Some(error @ CallTargetFactError::Resolve { reason, .. })
                if matches!(
                    reason.as_ref(),
                    ResolveCallError::Cancelled
                        | ResolveCallError::DeadlineExceeded
                        | ResolveCallError::CandidateLimit { .. }
                        | ResolveCallError::Work(_)
                        | ResolveCallError::SignatureLimit(_)
                        | ResolveCallError::SignatureArithmeticOverflow { .. }
                ) =>
            {
                Some(error.clone())
            }
            _ => None,
        }
    }

    pub(super) fn record_terminal_query_error(&mut self, error: CallTargetFactError) {
        self.error = Some(error);
    }

    pub(super) fn restore_selected_nested_facts_from(&mut self, checked: &Self) {
        if self.mode != checked.mode || checked.facts.is_empty() {
            return;
        }
        self.site.clone_from(&checked.site);
        for (expression, fact) in &checked.facts {
            match self.facts.entry(*expression) {
                Entry::Vacant(entry) => {
                    entry.insert(fact.clone());
                }
                Entry::Occupied(entry) if entry.get() == fact => {}
                Entry::Occupied(_) => {
                    self.error = Some(CallTargetFactError::DuplicateExpression {
                        expression: *expression,
                    });
                    return;
                }
            }
        }
        if self.error.is_none() {
            self.error.clone_from(&checked.error);
        }
    }

    pub(super) fn finish(self) -> CallTargetFactReport {
        CallTargetFactReport {
            mode: self.mode,
            site: self.site,
            facts: self.facts,
            error: self.error,
        }
    }
}

#[derive(Clone)]
pub(super) struct CallResolverControl<'a> {
    cancellation: &'a AtomicBool,
    work: ResolverWorkOwner<'a>,
    signature_work: Option<Rc<RefCell<&'a mut SignatureQueryWorkMeter>>>,
    signature_control: Option<&'a dyn SignatureQueryStepControl>,
}

#[derive(Clone)]
enum ResolverWorkOwner<'a> {
    Ordinary(Rc<RefCell<ResolverWork>>),
    Caller {
        focused: Rc<RefCell<&'a mut ResolverWork>>,
        ordinary: Rc<RefCell<ResolverWork>>,
    },
}

#[derive(Clone, Copy)]
pub(super) enum CallableWorkOperation {
    Resolver,
    ArgumentMapping,
    TypeCheck,
}

impl<'a> CallResolverControl<'a> {
    pub(super) fn ordinary() -> Self {
        Self {
            cancellation: &NEVER_CANCELLED,
            work: ResolverWorkOwner::Ordinary(Rc::new(RefCell::new(ResolverWork::new(
                PRODUCTION_CALLABLE_LIMITS.max_query_work(),
            )))),
            signature_work: None,
            signature_control: None,
        }
    }
    pub(super) fn caller_owned(
        cancellation: &'a AtomicBool,
        work: &'a mut ResolverWork,
        signature_work: Option<&'a mut SignatureQueryWorkMeter>,
        signature_control: Option<&'a dyn SignatureQueryStepControl>,
    ) -> Self {
        Self {
            cancellation,
            work: ResolverWorkOwner::Caller {
                focused: Rc::new(RefCell::new(work)),
                ordinary: Rc::new(RefCell::new(ResolverWork::new(
                    PRODUCTION_CALLABLE_LIMITS.max_query_work(),
                ))),
            },
            signature_work: signature_work.map(|work| Rc::new(RefCell::new(work))),
            signature_control,
        }
    }

    pub(super) fn with_parts<T>(
        &mut self,
        focused: bool,
        use_parts: impl FnOnce(
            &AtomicBool,
            &mut ResolverWork,
            Option<&mut SignatureQueryWorkMeter>,
            Option<&dyn SignatureQueryStepControl>,
        ) -> T,
    ) -> T {
        match &self.work {
            ResolverWorkOwner::Ordinary(work) => {
                let mut work = work.borrow_mut();
                work.reset();
                use_parts(&NEVER_CANCELLED, &mut work, None, None)
            }
            ResolverWorkOwner::Caller {
                focused: caller, ..
            } if focused => {
                let mut caller = caller.borrow_mut();
                match &self.signature_work {
                    Some(signature_work) => {
                        let mut signature_work = signature_work.borrow_mut();
                        use_parts(
                            self.cancellation,
                            &mut caller,
                            Some(&mut signature_work),
                            self.signature_control,
                        )
                    }
                    None => use_parts(self.cancellation, &mut caller, None, self.signature_control),
                }
            }
            ResolverWorkOwner::Caller { ordinary, .. } => {
                let mut ordinary = ordinary.borrow_mut();
                ordinary.reset();
                use_parts(&NEVER_CANCELLED, &mut ordinary, None, None)
            }
        }
    }

    pub(super) fn charge_signature(
        &mut self,
        kind: SignatureWorkKind,
        units: u64,
    ) -> Result<(), SignatureAccountingError> {
        let Some(work) = &self.signature_work else {
            return Ok(());
        };
        work.borrow_mut().charge(kind, units)
    }

    pub(super) fn charge_callable_operation(
        &mut self,
        focused: bool,
        operation: CallableWorkOperation,
    ) -> Result<(), crate::callable::CallableQueryLimitError> {
        let charge = |work: &mut ResolverWork| match operation {
            CallableWorkOperation::Resolver => work.charge(1),
            CallableWorkOperation::ArgumentMapping => work.charge_argument_mapping(1),
            CallableWorkOperation::TypeCheck => work.charge_type_check(1),
        };
        match &self.work {
            ResolverWorkOwner::Ordinary(work) => charge(&mut work.borrow_mut()),
            ResolverWorkOwner::Caller {
                focused: caller, ..
            } if focused => charge(&mut caller.borrow_mut()),
            ResolverWorkOwner::Caller { ordinary, .. } => charge(&mut ordinary.borrow_mut()),
        }
    }

    pub(super) fn check_signature_query_step(
        &self,
        step: SignatureQueryStep,
    ) -> Result<(), ResolveCallError> {
        match self.signature_control {
            Some(control) => control.check_signature_query_step(step),
            None if self.cancellation.load(std::sync::atomic::Ordering::Relaxed) => {
                Err(ResolveCallError::Cancelled)
            }
            None => Ok(()),
        }
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
        byte_offset,
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
        assert!(report.facts.is_empty());
        assert!(report.error.is_none());
    }

    #[test]
    fn ordinary_control_reuses_one_production_bounded_work_counter_per_call() {
        let mut control = CallResolverControl::ordinary();
        control.with_parts(false, |_, work, _, _| {
            assert_eq!(work.limit(), PRODUCTION_CALLABLE_LIMITS.max_query_work());
            work.charge(1).expect("one work unit");
            assert_eq!(work.consumed(), 1);
        });

        control.with_parts(false, |_, work, _, _| {
            assert_eq!(work.limit(), PRODUCTION_CALLABLE_LIMITS.max_query_work());
            assert_eq!(work.consumed(), 0);
        });
    }

    #[test]
    fn caller_control_charges_only_the_focused_call() {
        use std::sync::atomic::AtomicBool;

        use crate::callable::ResolverWork;

        let cancelled = AtomicBool::new(false);
        let mut caller = ResolverWork::new(7);
        let mut control = CallResolverControl::caller_owned(&cancelled, &mut caller, None, None);

        control.with_parts(false, |_, ordinary, _, _| {
            ordinary.charge(3).expect("ordinary work");
        });
        control.with_parts(false, |_, ordinary, _, _| {
            assert_eq!(ordinary.consumed(), 0);
        });

        control.with_parts(true, |_, focused, _, _| {
            focused.charge(2).expect("focused work");
        });
        control.with_parts(true, |_, focused, _, _| {
            assert_eq!(focused.consumed(), 2);
        });
        assert_eq!(caller.consumed(), 2);
    }
}
