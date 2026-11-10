//! Deep Copy follows the checked value producer, including results of a known closure.

use std::collections::BTreeSet;

use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, LocalId},
    pattern::{HirPatternBinding, HirPatternKind},
    project::{HirBindingSite, HirExpressionBindingRole},
};

use crate::{
    callable::{
        CheckedCallApplication, CheckedCallArgumentSlotSource, CheckedCallCalleeExecution,
        CheckedCallOperandDestination, CheckedCallResult, CheckedConstraintSourceProjection,
    },
    final_analysis::model::CallableResultSource,
    ownership::CheckedOwnershipLimits,
    types::TypeKind,
};

use super::{
    CheckedExpressionResolution, CheckedLocalCopyEvidence, CheckedLocalUseError,
    CheckedValueResolution, LocalUseChecker,
};

struct Invocation<'a> {
    closure: ExprId,
    application: &'a CheckedCallApplication,
    lexical: Option<&'a Invocation<'a>>,
    caller: Option<&'a Invocation<'a>>,
}

struct ValueCopy<'a, 'program> {
    checker: &'a LocalUseChecker<'program>,
    visiting: &'a mut BTreeSet<LocalId>,
    invocations: BTreeSet<ExprId>,
    remaining: u64,
    depth: u64,
    proven_invocations: u64,
}

impl LocalUseChecker<'_> {
    pub(super) fn binding_is_copy(
        &self,
        local: LocalId,
        visiting: &mut BTreeSet<LocalId>,
    ) -> Result<bool, CheckedLocalUseError> {
        ValueCopy::new(self, visiting).binding(local, None)
    }

    pub(super) fn binding_copy_evidence(
        &self,
        local: LocalId,
        visiting: &mut BTreeSet<LocalId>,
    ) -> Result<Option<CheckedLocalCopyEvidence>, CheckedLocalUseError> {
        ValueCopy::new(self, visiting).binding_evidence(local, None)
    }

    pub(super) fn expression_is_copy(
        &self,
        owner: ExprId,
        visiting: &mut BTreeSet<LocalId>,
    ) -> Result<bool, CheckedLocalUseError> {
        let mut query = ValueCopy::new(self, visiting);
        // This path classifies a synthetic operand with no initializer check.
        Ok(query.expression(owner, None)? && query.proven_invocations == 0)
    }
}

impl<'a, 'program> ValueCopy<'a, 'program> {
    fn new(checker: &'a LocalUseChecker<'program>, visiting: &'a mut BTreeSet<LocalId>) -> Self {
        Self {
            checker,
            visiting,
            invocations: BTreeSet::new(),
            remaining: CheckedOwnershipLimits::PRODUCTION.max_value_certificate_nodes,
            depth: 0,
            proven_invocations: 0,
        }
    }

    fn visit<T>(
        &mut self,
        query: impl FnOnce(&mut Self) -> Result<T, CheckedLocalUseError>,
    ) -> Result<T, CheckedLocalUseError> {
        if self.depth > CheckedOwnershipLimits::PRODUCTION.max_recursion_depth {
            return Err(CheckedLocalUseError::ValueOwnershipWorkLimit);
        }
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(CheckedLocalUseError::ValueOwnershipWorkLimit)?;
        self.depth += 1;
        let result = query(self);
        self.depth -= 1;
        result
    }

    fn binding(
        &mut self,
        local: LocalId,
        frame: Option<&Invocation<'_>>,
    ) -> Result<bool, CheckedLocalUseError> {
        if let Some((invocation, parameter)) = self.formal(local, frame) {
            if let Some(source) = self.whole_argument(local, invocation, parameter)? {
                return self.expression(source, invocation.caller);
            }
            let ty = self
                .checker
                .analysis
                .local(local)
                .ok_or(CheckedLocalUseError::InvalidTopology)?
                .ty();
            return self.checker.type_is_copy(ty);
        }
        if self.checker.copy_requirements.contains_key(&local) {
            return Ok(true);
        }
        let before = self.proven_invocations;
        let result = self.binding_evidence(local, frame);
        if frame.is_none() {
            // A closed, immutable alias inherits its initializer's check.
            self.proven_invocations = before;
        }
        result.map(|evidence| evidence.is_some())
    }

    fn binding_evidence(
        &mut self,
        local: LocalId,
        frame: Option<&Invocation<'_>>,
    ) -> Result<Option<CheckedLocalCopyEvidence>, CheckedLocalUseError> {
        self.visit(|query| query.binding_evidence_inner(local, frame))
    }

    fn binding_evidence_inner(
        &mut self,
        local: LocalId,
        frame: Option<&Invocation<'_>>,
    ) -> Result<Option<CheckedLocalCopyEvidence>, CheckedLocalUseError> {
        let binding = self
            .checker
            .analysis
            .local(local)
            .ok_or(CheckedLocalUseError::InvalidTopology)?;
        let closed = self.checker.closed_type(binding.ty())?;
        let ty = closed.semantic_identity_digest().ok();
        if self.checker.type_is_copy(binding.ty())? {
            return Ok(ty.map(|ty| CheckedLocalCopyEvidence::StructuralType { ty }));
        }
        if !matches!(
            closed,
            TypeKind::Function { .. } | TypeKind::CharacterDialogue(_)
        ) || !self.visiting.insert(local)
        {
            return Ok(None);
        }
        let before = self.proven_invocations;
        let result = (|| match (ty, self.immutable_initializer(local)?) {
            (Some(ty), Some(expression)) if self.expression(expression, frame)? => {
                Ok(Some(if self.proven_invocations != before {
                    CheckedLocalCopyEvidence::InvocationInitializer { ty, expression }
                } else {
                    CheckedLocalCopyEvidence::ImmutableInitializer { ty, expression }
                }))
            }
            _ => Ok(None),
        })();
        self.visiting.remove(&local);
        result
    }

    fn immutable_initializer(
        &self,
        local: LocalId,
    ) -> Result<Option<ExprId>, CheckedLocalUseError> {
        if self
            .checker
            .module
            .resolve_local(local)
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?
            .is_mutable_binding()
        {
            return Ok(None);
        }
        Ok(self
            .checker
            .analysis
            .hir_topology()
            .module(local.module())
            .and_then(|module| module.local_origins().binding(local))
            .and_then(|origin| origin.value()))
    }

    fn expression(
        &mut self,
        owner: ExprId,
        frame: Option<&Invocation<'_>>,
    ) -> Result<bool, CheckedLocalUseError> {
        self.visit(|query| query.expression_inner(owner, frame))
    }

    fn expression_inner(
        &mut self,
        owner: ExprId,
        frame: Option<&Invocation<'_>>,
    ) -> Result<bool, CheckedLocalUseError> {
        let expression = self
            .checker
            .analysis
            .expression(owner)
            .ok_or(CheckedLocalUseError::InvalidTopology)?;
        if let Some(ty) = expression.value_type()
            && self.checker.type_is_copy(ty)?
        {
            return Ok(true);
        }
        let application = self
            .checker
            .analysis
            .call(owner)
            .and_then(|facts| facts.selected_application());
        match expression.resolution() {
            CheckedExpressionResolution::Value(CheckedValueResolution::Local(local)) => {
                return self.binding(*local, frame);
            }
            CheckedExpressionResolution::Value(
                CheckedValueResolution::ProjectCallable(_)
                | CheckedValueResolution::EnvironmentCallable(_),
            ) => return Ok(true),
            CheckedExpressionResolution::Closure(closure) => {
                for capture in closure.captures() {
                    if !self.binding(capture.local(), frame)? {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            CheckedExpressionResolution::CharacterDialogueFactory(_)
            | CheckedExpressionResolution::CharacterDialogueReconfigure(_) => {}
            _ if application.is_some_and(|call| call.core().retains_inputs()) => {}
            _ => {
                if let Some(application) = application
                    && matches!(
                        application.result(),
                        CheckedCallResult::Value(TypeKind::Function { .. })
                    )
                    && let CheckedCallCalleeExecution::Value { source } =
                        application.core().callee()
                    && let CheckedCallArgumentSlotSource::Expression(callee) = source.raw()
                {
                    return self.invoked_result(owner, callee, application, frame);
                }
                return self.branches(owner, |query, branch| query.expression(branch, frame));
            }
        }
        // A direct continuation retains evaluated inputs. Only a value
        // callee also contributes a runtime capture to its Copy footprint.
        let static_continuation = application.is_some_and(|call| {
            call.core().retains_inputs()
                && matches!(call.core().callee(), CheckedCallCalleeExecution::Direct)
        });
        let checker = self.checker;
        let fact = checker
            .analysis
            .checked_expression_edge_fact(owner)
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
        for edge in fact.edges() {
            if static_continuation
                && matches!(
                    edge.role(),
                    crate::semantic_coordinate::CheckedExpressionChildRole::Callee
                )
            {
                continue;
            }
            if !self.expression(edge.child(), frame)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn invoked_result(
        &mut self,
        owner: ExprId,
        callee: ExprId,
        application: &CheckedCallApplication,
        frame: Option<&Invocation<'_>>,
    ) -> Result<bool, CheckedLocalUseError> {
        if !self.invocations.insert(owner) {
            return Ok(false);
        }
        let result = self.callee_result(callee, application, frame, frame);
        self.invocations.remove(&owner);
        if matches!(result, Ok(true)) {
            self.proven_invocations += 1;
        }
        result
    }

    fn callee_result(
        &mut self,
        callee: ExprId,
        application: &CheckedCallApplication,
        lexical: Option<&Invocation<'_>>,
        caller: Option<&Invocation<'_>>,
    ) -> Result<bool, CheckedLocalUseError> {
        self.visit(|query| query.callee_result_inner(callee, application, lexical, caller))
    }

    fn callee_result_inner(
        &mut self,
        callee: ExprId,
        application: &CheckedCallApplication,
        lexical: Option<&Invocation<'_>>,
        caller: Option<&Invocation<'_>>,
    ) -> Result<bool, CheckedLocalUseError> {
        let checked = self
            .checker
            .analysis
            .expression(callee)
            .ok_or(CheckedLocalUseError::InvalidTopology)?;
        match checked.resolution() {
            CheckedExpressionResolution::Closure(closure) => {
                let frame = Invocation {
                    closure: closure.owner(),
                    application,
                    lexical,
                    caller,
                };
                for &source in closure.value_results() {
                    let CallableResultSource::Returned(result) = source else {
                        return Ok(false);
                    };
                    if !self.expression(result, Some(&frame))? {
                        return Ok(false);
                    }
                }
                Ok(!closure.value_results().is_empty())
            }
            CheckedExpressionResolution::Value(CheckedValueResolution::Local(local)) => {
                let local = *local;
                if !self.visiting.insert(local) {
                    return Ok(false);
                }
                let result = (|| {
                    if let Some((invocation, parameter)) = self.formal(local, lexical) {
                        return match self.whole_argument(local, invocation, parameter)? {
                            Some(source) => {
                                self.callee_result(source, application, invocation.caller, caller)
                            }
                            None => Ok(false),
                        };
                    }
                    match self.immutable_initializer(local)? {
                        Some(source) => self.callee_result(source, application, lexical, caller),
                        None => Ok(false),
                    }
                })();
                self.visiting.remove(&local);
                result
            }
            _ => self.branches(callee, |query, branch| {
                query.callee_result(branch, application, lexical, caller)
            }),
        }
    }

    fn branches(
        &mut self,
        owner: ExprId,
        mut value: impl FnMut(&mut Self, ExprId) -> Result<bool, CheckedLocalUseError>,
    ) -> Result<bool, CheckedLocalUseError> {
        let hir = self
            .checker
            .module
            .resolve_expr(owner)
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
        match hir.kind() {
            HirExprKind::If(branch) => {
                Ok(value(self, branch.then_branch())? && value(self, branch.else_branch())?)
            }
            HirExprKind::Block(block) => value(self, block.tail()),
            HirExprKind::NamedBlock(block) => value(self, block.tail()),
            _ => Ok(false),
        }
    }

    fn formal<'frame>(
        &self,
        local: LocalId,
        mut frame: Option<&'frame Invocation<'frame>>,
    ) -> Option<(&'frame Invocation<'frame>, u32)> {
        let origin = self
            .checker
            .analysis
            .hir_topology()
            .module(local.module())?
            .local_origins()
            .binding(local)?;
        let HirBindingSite::Expression {
            expression,
            role: HirExpressionBindingRole::ClosureParameter { parameter },
        } = origin.site()
        else {
            return None;
        };
        while let Some(invocation) = frame {
            if invocation.closure == expression {
                return Some((invocation, parameter));
            }
            frame = invocation.lexical;
        }
        None
    }

    fn whole_argument(
        &self,
        local: LocalId,
        invocation: &Invocation<'_>,
        parameter: u32,
    ) -> Result<Option<ExprId>, CheckedLocalUseError> {
        if self
            .checker
            .module
            .resolve_local(local)
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?
            .is_mutable_binding()
        {
            return Ok(None);
        }
        let closure = self
            .checker
            .module
            .resolve_expr(invocation.closure)
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
        let HirExprKind::Closure(closure) = closure.kind() else {
            return Err(CheckedLocalUseError::InvalidTopology);
        };
        let formal = closure
            .parameters()
            .get(parameter as usize)
            .ok_or(CheckedLocalUseError::InvalidTopology)?;
        let pattern = self
            .checker
            .module
            .resolve_pattern(formal.pattern())
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
        if !matches!(pattern.kind(),
            HirPatternKind::Binding(HirPatternBinding::Bound { local: bound, .. })
            | HirPatternKind::TypedBinding { binding: HirPatternBinding::Bound { local: bound, .. }, .. }
            | HirPatternKind::WholeBinding { binding: HirPatternBinding::Bound { local: bound, .. }, .. }
            if *bound == local)
        {
            return Ok(None);
        }
        let mut operands = invocation
            .application
            .core()
            .execution()
            .arguments()
            .iter()
            .flat_map(|argument| argument.slots())
            .filter(|slot| {
                let CheckedCallOperandDestination::Parameter(coordinate) = slot.destination()
                else {
                    return false;
                };
                coordinate.group() == invocation.application.core().current_group()
                    && coordinate.parameter().get() == parameter as usize
            });
        let (Some(slot), None) = (operands.next(), operands.next()) else {
            return Ok(None);
        };
        Ok(match (slot.source_projection(), slot.source().raw()) {
            (
                CheckedConstraintSourceProjection::Scalar,
                CheckedCallArgumentSlotSource::Expression(source),
            ) => Some(source),
            _ => None,
        })
    }
}
