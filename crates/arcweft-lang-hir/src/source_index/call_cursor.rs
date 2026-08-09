//! Active Call slots projected from the committed final-HIR source manifest.

use arcweft_source::SourceDocumentIdentity;

use crate::expr::{
    HirCallArgumentListTerminator, HirCallArgumentOrdinal, HirCallExpr, HirCallTypeApplication,
    HirCallTypeApplicationTerminator, HirCallTypeArgumentOrdinal, HirExprKind,
};
use crate::identity::ExprId;
use crate::module::HirModule;

use super::{
    HirCallTypeApplicationSourceRole, HirExprSourceRole, HirSourcePresence, HirSourceQuery,
    HirSourceQueryError, HirSourceSite,
};

impl HirModule {
    /// Returns the zero-based ordinary Call argument slot selected by `cursor`.
    ///
    /// The opening token is outside. The byte boundary immediately after it is
    /// slot zero, each separator starts the following slot, and a trailing
    /// separator selects the one-past slot represented by `arguments().len()`.
    /// Closed and recovered lists both include their interior end boundary.
    /// No source text or detached syntax reader participates in this query.
    ///
    /// # Panics
    ///
    /// Panics only if an accepted call manifest violates its validated ordinal
    /// conversion or ordering invariant.
    pub fn call_active_argument_slot(
        &self,
        expected_source: &SourceDocumentIdentity,
        owner: ExprId,
        cursor: usize,
    ) -> Result<Option<usize>, HirSourceQueryError> {
        let open = required_call_site(
            self,
            expected_source,
            owner,
            HirExprSourceRole::CallArgumentListOpen,
        )?;
        let call = resolved_call(self, owner);
        let content_end = match call.terminator() {
            HirCallArgumentListTerminator::Closed => site_start(required_call_site(
                self,
                expected_source,
                owner,
                HirExprSourceRole::CallArgumentListClose,
            )?),
            HirCallArgumentListTerminator::RecoveredMissing => site_start(required_call_site(
                self,
                expected_source,
                owner,
                HirExprSourceRole::CallArgumentListRecoveryEnd,
            )?),
        };
        if cursor < site_end(open) || cursor > content_end {
            return Ok(None);
        }

        let mut active = 0usize;
        for following in 1..call.arguments().len() {
            let ordinal = HirCallArgumentOrdinal::try_from_usize(following)
                .expect("published Call argument count fits its HIR ordinal");
            let separator = required_call_site(
                self,
                expected_source,
                owner,
                HirExprSourceRole::CallArgumentSeparator { following: ordinal },
            )?;
            if site_start(separator) <= cursor {
                active = following;
            }
        }
        if !call.arguments().is_empty()
            && optional_call_site(
                self,
                expected_source,
                owner,
                HirExprSourceRole::CallArgumentTrailingSeparator,
            )?
            .is_some_and(|separator| site_start(separator) <= cursor)
        {
            active = call.arguments().len();
        }
        Ok(Some(active))
    }

    /// Returns the zero-based explicit Call type-argument slot selected by `cursor`.
    ///
    /// This projection is independent from the ordinary argument list and from
    /// generic arguments retained by an associated receiver. A trailing type
    /// separator selects the one-past slot represented by the explicit type
    /// argument count.
    ///
    /// # Panics
    ///
    /// Panics only if an accepted call manifest violates its validated ordinal
    /// conversion or ordering invariant.
    pub fn call_active_type_argument_slot(
        &self,
        expected_source: &SourceDocumentIdentity,
        owner: ExprId,
        cursor: usize,
    ) -> Result<Option<usize>, HirSourceQueryError> {
        // This required Call role validates the typed owner and exact source
        // identity even when the Call has no explicit type application.
        let _ = required_call_site(
            self,
            expected_source,
            owner,
            HirExprSourceRole::CallArgumentListOpen,
        )?;
        let call = resolved_call(self, owner);
        let HirCallTypeApplication::Present {
            arguments,
            terminator,
            ..
        } = call.explicit_type_application()
        else {
            return Ok(None);
        };
        let type_role = |role| HirExprSourceRole::CallTypeApplication(role);
        let open = required_call_site(
            self,
            expected_source,
            owner,
            type_role(HirCallTypeApplicationSourceRole::OpenAngle),
        )?;
        let content_end = match terminator {
            HirCallTypeApplicationTerminator::Closed
            | HirCallTypeApplicationTerminator::InvalidPresent => site_start(required_call_site(
                self,
                expected_source,
                owner,
                type_role(HirCallTypeApplicationSourceRole::CloseAngle),
            )?),
            HirCallTypeApplicationTerminator::RecoveredMissing => site_start(required_call_site(
                self,
                expected_source,
                owner,
                type_role(HirCallTypeApplicationSourceRole::RecoveryEnd),
            )?),
        };
        if cursor < site_end(open) || cursor > content_end {
            return Ok(None);
        }

        let mut active = 0usize;
        for following in 1..arguments.len() {
            let ordinal = HirCallTypeArgumentOrdinal::try_from_usize(following)
                .expect("published Call type-argument count fits its HIR ordinal");
            let separator = required_call_site(
                self,
                expected_source,
                owner,
                type_role(HirCallTypeApplicationSourceRole::Separator { following: ordinal }),
            )?;
            if site_start(separator) <= cursor {
                active = following;
            }
        }
        if !arguments.is_empty()
            && optional_call_site(
                self,
                expected_source,
                owner,
                type_role(HirCallTypeApplicationSourceRole::TrailingSeparator),
            )?
            .is_some_and(|separator| site_start(separator) <= cursor)
        {
            active = arguments.len();
        }
        Ok(Some(active))
    }
}

fn resolved_call(module: &HirModule, owner: ExprId) -> &HirCallExpr {
    let expression = module
        .resolve_expr(owner)
        .expect("successful Call source query validated the expression owner");
    let HirExprKind::Call(call) = expression.kind() else {
        unreachable!("Call source role was admitted by a non-Call expression")
    };
    call
}

fn required_call_site<'module>(
    module: &'module HirModule,
    expected_source: &SourceDocumentIdentity,
    owner: ExprId,
    role: HirExprSourceRole,
) -> Result<&'module HirSourceSite, HirSourceQueryError> {
    let lookup = module.source_site(expected_source, HirSourceQuery::Expr { owner, role })?;
    match lookup.presence() {
        HirSourcePresence::Present(site) => Ok(site),
        HirSourcePresence::AbsentOptional => {
            unreachable!("required committed Call source role resolved as optional-absent")
        }
    }
}

fn optional_call_site<'module>(
    module: &'module HirModule,
    expected_source: &SourceDocumentIdentity,
    owner: ExprId,
    role: HirExprSourceRole,
) -> Result<Option<&'module HirSourceSite>, HirSourceQueryError> {
    let lookup = module.source_site(expected_source, HirSourceQuery::Expr { owner, role })?;
    Ok(match lookup.presence() {
        HirSourcePresence::Present(site) => Some(site),
        HirSourcePresence::AbsentOptional => None,
    })
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
