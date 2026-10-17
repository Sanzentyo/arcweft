//! Declaration lookup without an application or an evaluated namespace value.

use super::{
    CallCalleeClassificationFact, CallResolverAuthority, CallResolverContext, CallResolverRequest,
    CallableLimits, CallablePath, ExprId, HirExprKind, HirModule, HirPath, HirPathRoot,
    HirPathValue, HirSelectedMember, PrepareFinalCallCalleeError, PreparedCallCallee,
    PreparedFreeCallContext, PreparedFreeCallScope, ResolveCallError, ResolverWork,
};

/// An original, bounded final-HIR reference and its canonical lookup path.
/// It carries no arguments, application group, or independently copied schema.
pub(crate) struct PreparedCallableReference<'hir> {
    module: &'hir HirModule,
    expression: ExprId,
    path: HirPath,
    callable: CallablePath,
    scope: PreparedFreeCallScope,
}

impl PreparedCallableReference<'_> {
    pub(crate) const fn callable_path(&self) -> &CallablePath {
        &self.callable
    }

    pub(crate) const fn path(&self) -> &HirPath {
        &self.path
    }
}

impl<'hir> CallResolverAuthority<'hir> {
    /// Retains the actual HIR path/select chain. Value-first lookup remains
    /// with the Analyzer before this reference enters the shared resolver.
    pub(crate) fn prepare_callable_reference(
        self,
        expression: ExprId,
        limits: &CallableLimits,
    ) -> Result<Option<PreparedCallableReference<'hir>>, PrepareFinalCallCalleeError> {
        let invalid = || PrepareFinalCallCalleeError::InvalidValuePath { expression };
        let mut current = expression;
        let mut members = Vec::new();
        let mut path = loop {
            let node = self
                .validate_expression(current, limits)
                .map_err(|_| invalid())?;
            match node.kind() {
                HirExprKind::Path(HirPathValue::Resolved(path)) => {
                    if path
                        .segments()
                        .len()
                        .checked_add(members.len())
                        .is_none_or(|count| count > limits.max_path_segments())
                    {
                        return Err(invalid());
                    }
                    break path.clone();
                }
                HirExprKind::Select(select) => {
                    let HirSelectedMember::Name(member) = select.member() else {
                        return Err(PrepareFinalCallCalleeError::RecoveredCallee);
                    };
                    if members.len() >= limits.max_path_segments() {
                        return Err(invalid());
                    }
                    members.push(member);
                    current = select.target();
                }
                _ => return Ok(None),
            }
        };
        for member in members.into_iter().rev() {
            path = path.with_terminal_member(member);
        }
        let scope = match path.root() {
            HirPathRoot::ImplicitCrate => PreparedFreeCallScope::Implicit,
            HirPathRoot::Crate | HirPathRoot::SelfModule | HirPathRoot::Super { .. } => {
                PreparedFreeCallScope::ExplicitProject
            }
        };
        let callable = super::preparation::callable_path_from_hir(expression, &path, limits)?;
        Ok(Some(PreparedCallableReference {
            module: self.module(),
            expression,
            path,
            callable,
            scope,
        }))
    }
}

impl<'a> CallResolverRequest<'a> {
    /// A reference query borrows the same checked catalog transaction as a
    /// call, but never manufactures a nullary call or consumes an input group.
    pub(crate) fn try_new_reference(
        reference: &'a PreparedCallableReference<'_>,
        context: &CallResolverContext<'a>,
        work: &'a mut ResolverWork,
    ) -> Result<Self, ResolveCallError> {
        if context.cancellation.load(super::Ordering::Acquire) {
            return Err(ResolveCallError::Cancelled);
        }
        if reference.expression != context.expression
            || !std::ptr::eq(reference.module, context.authority.module())
            || reference.path.segments().len() > context.limits.max_path_segments()
        {
            return Err(ResolveCallError::InvalidResolvedCallable);
        }
        context
            .authority
            .validate_expression(context.expression, context.limits)?;
        Ok(Self {
            callee: PreparedCallCallee::Free {
                path: &reference.callable,
                project: None,
                scope: reference.scope,
                context: PreparedFreeCallContext::Ordinary,
            },
            authority: context.authority,
            checked: context.checked,
            presentation_character_owner: context.presentation_character_owner,
            call: None,
            classification: CallCalleeClassificationFact::Value {
                expression: reference.expression,
            },
            cancellation: context.cancellation,
            prepared_continuations: context.prepared_continuations,
            work,
            limits: context.limits,
            implicit_extension_receiver: None,
        })
    }
}
