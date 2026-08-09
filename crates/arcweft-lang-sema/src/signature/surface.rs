//! Bounded final-HIR call-surface selection before semantic checking.

use std::cmp::Ordering;

use arcweft_lang_hir::{
    expr::{
        HirCallArgument, HirCallArgumentListTerminator, HirCallCallee, HirCallExpr, HirCallValue,
        HirExprKind, HirRequiredTokenState,
    },
    identity::ExprId,
    module::HirModule,
    source_index::{HirExprSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite},
};
use arcweft_source::{SourceDocument, SourceRange, SourceSpan};

use crate::callable::{ResolveCallError, SignatureQueryWorkMeter, SignatureWorkKind};

use super::{
    SignatureQueryControl, SignatureQueryError, SignatureQueryStep, SignatureQueryStepControl,
    map_signature_accounting_error,
};

pub(super) struct SignatureSurfaceSelection {
    pub(super) site: Option<FocusedCallSite>,
    pub(super) unsupported_surface: bool,
}

/// One exact final-HIR Call selected by a cursor inside its argument list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FocusedCallSite {
    expression: ExprId,
    call: SourceSpan,
    callee: SourceSpan,
    arguments: SourceSpan,
    active_argument: Option<usize>,
    recovery_nodes: usize,
    missing_close_delimiter: bool,
    argument_content: SourceRange,
    open_paren_start: usize,
    byte_offset: Option<usize>,
}

impl FocusedCallSite {
    pub(crate) const fn expression(&self) -> ExprId {
        self.expression
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

    pub(crate) fn compare_focus(&self, current: &Self) -> Ordering {
        if strictly_contains(current.argument_content, self.argument_content) {
            return Ordering::Greater;
        }
        if strictly_contains(self.argument_content, current.argument_content) {
            return Ordering::Less;
        }
        range_len(current.argument_content)
            .cmp(&range_len(self.argument_content))
            .then_with(|| self.open_paren_start.cmp(&current.open_paren_start))
    }
}

pub(super) fn select_signature_surface(
    module: &HirModule,
    document: &SourceDocument,
    byte_offset: usize,
    control: SignatureQueryControl<'_>,
    work: &mut SignatureQueryWorkMeter,
) -> Result<SignatureSurfaceSelection, SignatureQueryError> {
    let mut scanner = SurfaceScanner {
        module,
        document,
        byte_offset,
        control,
        work,
        selected: None,
        unsupported_surface: false,
    };
    scanner.scan_module()?;
    Ok(SignatureSurfaceSelection {
        site: scanner.selected,
        unsupported_surface: scanner.unsupported_surface,
    })
}

struct SurfaceScanner<'a> {
    module: &'a HirModule,
    document: &'a SourceDocument,
    byte_offset: usize,
    control: SignatureQueryControl<'a>,
    work: &'a mut SignatureQueryWorkMeter,
    selected: Option<FocusedCallSite>,
    unsupported_surface: bool,
}

impl SurfaceScanner<'_> {
    fn visit_node(&mut self) -> Result<(), SignatureQueryError> {
        self.control
            .check_signature_query_step(SignatureQueryStep::SurfaceTraversal)
            .map_err(map_control_error)?;
        self.work
            .charge(SignatureWorkKind::NodeVisits, 1)
            .map_err(map_signature_accounting_error)
    }

    fn poll_operation(&self) -> Result<(), SignatureQueryError> {
        self.control
            .check_signature_query_step(SignatureQueryStep::SurfaceTraversal)
            .map_err(map_control_error)
    }

    fn scan_module(&mut self) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        for _ in self.module.items() {
            self.visit_node()?;
        }
        for _ in self.module.scopes() {
            self.visit_node()?;
        }
        for _ in self.module.locals() {
            self.visit_node()?;
        }
        for _ in self.module.statements() {
            self.visit_node()?;
        }
        for _ in self.module.patterns() {
            self.visit_node()?;
        }
        for _ in self.module.types() {
            self.visit_node()?;
        }
        for _ in self.module.captures() {
            self.visit_node()?;
        }
        for (expression_id, expression) in self.module.expressions() {
            self.visit_node()?;
            match expression.kind() {
                HirExprKind::Call(call) => self.scan_call(expression_id, call)?,
                HirExprKind::DialogueContentApplication(_) | HirExprKind::PostfixBracket(_) => {
                    self.mark_unsupported(expression_id)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn scan_call(
        &mut self,
        expression_id: ExprId,
        call: &HirCallExpr,
    ) -> Result<(), SignatureQueryError> {
        self.work
            .charge(SignatureWorkKind::CandidateCalls, 1)
            .map_err(map_signature_accounting_error)?;
        for _ in call.arguments() {
            self.poll_operation()?;
            self.work
                .charge(SignatureWorkKind::Arguments, 1)
                .map_err(map_signature_accounting_error)?;
        }

        let recovery_nodes = call
            .arguments()
            .iter()
            .filter(|argument| argument_is_recovered(argument))
            .count()
            + usize::from(call.terminator() == HirCallArgumentListTerminator::RecoveredMissing);
        for _ in 0..recovery_nodes {
            self.poll_operation()?;
            self.work
                .charge(SignatureWorkKind::RecoveryNodes, 1)
                .map_err(map_signature_accounting_error)?;
        }

        let Some(candidate) = self.focused_call_site(expression_id, call)? else {
            return Ok(());
        };
        self.poll_operation()?;
        self.work
            .charge(SignatureWorkKind::NestedCalls, 1)
            .map_err(map_signature_accounting_error)?;
        match self.selected.as_ref() {
            None => self.selected = Some(candidate),
            Some(current) => match candidate.compare_focus(current) {
                Ordering::Greater => self.selected = Some(candidate),
                Ordering::Less => {}
                Ordering::Equal
                    if candidate.expression() == current.expression()
                        && candidate.arguments() == current.arguments() => {}
                Ordering::Equal => {
                    return Err(super::SignatureSemanticUnavailable::AmbiguousCallRange {
                        document: Box::new(self.document.identity().clone()),
                        byte_offset: self.byte_offset,
                    }
                    .into());
                }
            },
        }
        Ok(())
    }

    fn focused_call_site(
        &self,
        expression_id: ExprId,
        call: &HirCallExpr,
    ) -> Result<Option<FocusedCallSite>, SignatureQueryError> {
        let Some(active_argument) = self
            .module
            .call_active_argument_slot(self.document.identity(), expression_id, self.byte_offset)
            .map_err(|error| super::SignatureSemanticUnavailable::SourceQuery {
                owner: expression_id,
                error: Box::new(error),
            })?
        else {
            return Ok(None);
        };
        let call_span = self.required_span(expression_id, HirExprSourceRole::Whole)?;
        let callee = self.callee_span(expression_id, call.callee())?;
        let open = self.required_span(expression_id, HirExprSourceRole::CallArgumentListOpen)?;
        let (content_end, list_end) = match call.terminator() {
            HirCallArgumentListTerminator::Closed => {
                let close =
                    self.required_span(expression_id, HirExprSourceRole::CallArgumentListClose)?;
                (close.range().start(), close.range().end())
            }
            HirCallArgumentListTerminator::RecoveredMissing => {
                let insertion = self.required_offset(
                    expression_id,
                    HirExprSourceRole::CallArgumentListRecoveryEnd,
                )?;
                (insertion, insertion)
            }
        };
        let content = SourceRange::new(open.range().end(), content_end);
        let arguments = self
            .document
            .span(SourceRange::new(open.range().start(), list_end))
            .map_err(map_span_error)?;
        Ok(Some(FocusedCallSite {
            expression: expression_id,
            call: call_span,
            callee,
            arguments,
            active_argument: Some(active_argument),
            recovery_nodes: call
                .arguments()
                .iter()
                .filter(|argument| argument_is_recovered(argument))
                .count()
                + usize::from(call.terminator() == HirCallArgumentListTerminator::RecoveredMissing),
            missing_close_delimiter: matches!(
                call.terminator(),
                HirCallArgumentListTerminator::RecoveredMissing
            ),
            argument_content: content,
            open_paren_start: open.range().start(),
            byte_offset: Some(self.byte_offset),
        }))
    }

    fn callee_span(
        &self,
        expression_id: ExprId,
        callee: &HirCallCallee,
    ) -> Result<SourceSpan, SignatureQueryError> {
        match callee {
            HirCallCallee::Value { .. } => {
                self.required_span(expression_id, HirExprSourceRole::CallCallee)
            }
            HirCallCallee::UnresolvedDot { .. } | HirCallCallee::Associated { .. } => {
                let receiver =
                    self.required_site(expression_id, HirExprSourceRole::CallAssociatedReceiver)?;
                let member =
                    self.required_site(expression_id, HirExprSourceRole::CallAssociatedMember)?;
                self.document
                    .span(SourceRange::new(site_start(&receiver), site_end(&member)))
                    .map_err(map_span_error)
            }
        }
    }

    fn mark_unsupported(&mut self, expression_id: ExprId) -> Result<(), SignatureQueryError> {
        let span = self.required_span(expression_id, HirExprSourceRole::Whole)?;
        self.unsupported_surface |=
            span.range().start() <= self.byte_offset && self.byte_offset <= span.range().end();
        Ok(())
    }

    fn required_span(
        &self,
        owner: ExprId,
        role: HirExprSourceRole,
    ) -> Result<SourceSpan, SignatureQueryError> {
        match self.required_site(owner, role)? {
            HirSourceSite::Span(span) => Ok(span),
            HirSourceSite::Insertion(insertion) => self
                .document
                .span(SourceRange::new(insertion.offset(), insertion.offset()))
                .map_err(map_span_error),
        }
    }

    fn required_offset(
        &self,
        owner: ExprId,
        role: HirExprSourceRole,
    ) -> Result<usize, SignatureQueryError> {
        Ok(site_start(&self.required_site(owner, role)?))
    }

    fn required_site(
        &self,
        owner: ExprId,
        role: HirExprSourceRole,
    ) -> Result<HirSourceSite, SignatureQueryError> {
        self.optional_site(owner, role)?.ok_or_else(|| {
            super::SignatureSemanticUnavailable::MissingSourceComponent { owner, role }.into()
        })
    }

    fn optional_site(
        &self,
        owner: ExprId,
        role: HirExprSourceRole,
    ) -> Result<Option<HirSourceSite>, SignatureQueryError> {
        let lookup = self
            .module
            .source_site(
                self.document.identity(),
                HirSourceQuery::Expr { owner, role },
            )
            .map_err(|error| super::SignatureSemanticUnavailable::SourceQuery {
                owner,
                error: Box::new(error),
            })?;
        Ok(match lookup.presence() {
            HirSourcePresence::Present(site) => Some(site.clone()),
            HirSourcePresence::AbsentOptional => None,
        })
    }
}

fn argument_is_recovered(argument: &HirCallArgument) -> bool {
    let value_missing = matches!(argument.value_state(), HirCallValue::Missing { .. });
    match argument {
        HirCallArgument::Positional { .. } => value_missing,
        HirCallArgument::Named { name, equals, .. } => {
            name.resolved().is_none() || *equals != HirRequiredTokenState::Present || value_missing
        }
        HirCallArgument::Spread { ellipsis, .. } => {
            *ellipsis != HirRequiredTokenState::Present || value_missing
        }
    }
}

fn site_start(site: &HirSourceSite) -> usize {
    match site {
        HirSourceSite::Span(span) => span.range().start(),
        HirSourceSite::Insertion(insertion) => insertion.offset(),
    }
}

fn site_end(site: &HirSourceSite) -> usize {
    match site {
        HirSourceSite::Span(span) => span.range().end(),
        HirSourceSite::Insertion(insertion) => insertion.offset(),
    }
}

fn range_len(range: SourceRange) -> usize {
    range.end().saturating_sub(range.start())
}

fn strictly_contains(outer: SourceRange, inner: SourceRange) -> bool {
    outer.start() <= inner.start()
        && inner.end() <= outer.end()
        && (outer.start() < inner.start() || inner.end() < outer.end())
}

fn map_control_error(error: ResolveCallError) -> SignatureQueryError {
    match error {
        ResolveCallError::Cancelled => SignatureQueryError::Cancelled,
        ResolveCallError::DeadlineExceeded => SignatureQueryError::DeadlineExceeded,
        error => SignatureQueryError::Resolve(error),
    }
}

fn map_span_error(_: arcweft_source::SourceSpanError) -> SignatureQueryError {
    crate::callable::SemanticSignatureError::InvalidSpan.into()
}
