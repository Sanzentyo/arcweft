//! Normalized type dependencies of executable facts in every semantic scope.
//!
//! A fact's result type is only one dependency. Calls, nominal schemas,
//! captures and nested executable catalogs retain additional owned types that
//! must enter the same transaction type graph before any body is lowered.

use super::{
    RuntimeCheckedCapture, RuntimeClosureCaptureFact, RuntimeClosureInstanceFact,
    RuntimeClosureParameterFact, RuntimeContentFragmentFact, RuntimeDialogueEffectCaptureFact,
    RuntimeDialogueEffectTrigger, RuntimeDialogueValueExpression, RuntimeIteratorFact,
    RuntimeNormalizedType, RuntimeProjectAttachedDefaultCapture, RuntimeProjectCallable,
    RuntimeProjectContinuationAbi, RuntimeProjectFunctionExpressionPayload,
    RuntimeProjectFunctionInstanceFact, RuntimeProjectFunctionInstanceSemanticFacts,
    RuntimeProjectFunctionPatternPayload, RuntimeProjectFunctionStatementPayload,
    RuntimeProjectFunctionTypeProjection, RuntimeRecordExpressionFact, RuntimeRecordPatternFact,
    RuntimeResolvedAttachedContent, RuntimeResolvedCall, RuntimeResolvedCallDispatch,
    RuntimeResolvedCallOperand, RuntimeResolvedHostCallOwner, RuntimeResolvedNominalRecordField,
    RuntimeResolvedStaticCallTarget, RuntimeResolvedValue, RuntimeTryFact,
};

impl RuntimeProjectCallable {
    pub(super) fn append_normalized_types<'a>(
        &'a self,
        roots: &mut Vec<&'a RuntimeNormalizedType>,
    ) {
        if let Some(attached) = self.attached_content_abi() {
            roots.extend([attached.binding_ty(), attached.abi_ty()]);
        }
    }
}

impl RuntimeResolvedValue {
    pub(super) fn append_normalized_types<'a>(
        &'a self,
        roots: &mut Vec<&'a RuntimeNormalizedType>,
    ) {
        match self {
            Self::ProjectCallable(callable) => callable.append_normalized_types(roots),
            Self::Local(_)
            | Self::ProjectItem(_)
            | Self::DialogueLine(_)
            | Self::CharacterLook { .. }
            | Self::Intrinsic(_)
            | Self::Registered(_)
            | Self::Constant(_) => {}
        }
    }
}

impl RuntimeResolvedCall {
    pub(super) fn append_normalized_types<'a>(
        &'a self,
        roots: &mut Vec<&'a RuntimeNormalizedType>,
    ) {
        roots.extend(self.operands().iter().map(RuntimeResolvedCallOperand::ty));
        roots.extend(
            self.attached_content()
                .map(RuntimeResolvedAttachedContent::ty),
        );
        self.dispatch().append_normalized_types(roots);
        if let Some(plan) = self.project_function() {
            for parameter in plan.current_group_materialization() {
                roots.extend([parameter.abi_ty(), parameter.binding_ty()]);
            }
            roots.extend(plan.input().function_type());
            roots.extend(
                plan.input()
                    .continuation_abi()
                    .into_iter()
                    .flat_map(RuntimeProjectContinuationAbi::prefix_types),
            );
            roots.extend(plan.outcome().function_type());
            roots.extend(
                plan.outcome()
                    .continuation_abi()
                    .into_iter()
                    .flat_map(RuntimeProjectContinuationAbi::prefix_types),
            );
            plan.callable().append_normalized_types(roots);
        }
    }
}

impl RuntimeResolvedCallDispatch {
    fn append_normalized_types<'a>(&'a self, roots: &mut Vec<&'a RuntimeNormalizedType>) {
        match self {
            Self::Static(target) => target.append_normalized_types(roots),
            Self::Value { .. } => {}
        }
    }
}

impl RuntimeResolvedStaticCallTarget {
    fn append_normalized_types<'a>(&'a self, roots: &mut Vec<&'a RuntimeNormalizedType>) {
        match self {
            Self::Declaration(callable) => callable.append_normalized_types(roots),
            Self::Variant(variant) => variant.owner().append_normalized_types(roots),
            Self::Host(host) => match host.owner() {
                RuntimeResolvedHostCallOwner::ExternCapability(callable) => {
                    callable.append_normalized_types(roots);
                }
                RuntimeResolvedHostCallOwner::Agent(_) => {}
            },
            Self::Intrinsic(_)
            | Self::Agent(_)
            | Self::AgentProbeComparison(_)
            | Self::AgentDiagnosticsHasError
            | Self::Reduction(_)
            | Self::StandardMap(_)
            | Self::TraitMethod { .. }
            | Self::Line(_)
            | Self::Registered(_) => {}
        }
    }
}

impl RuntimeRecordExpressionFact {
    pub(super) fn append_normalized_types<'a>(
        &'a self,
        roots: &mut Vec<&'a RuntimeNormalizedType>,
    ) {
        roots.extend(
            self.nominal()
                .fields()
                .iter()
                .map(RuntimeResolvedNominalRecordField::ty),
        );
    }
}

impl RuntimeRecordPatternFact {
    pub(super) fn append_normalized_types<'a>(
        &'a self,
        roots: &mut Vec<&'a RuntimeNormalizedType>,
    ) {
        match (self.nominal(), self.structural()) {
            (Some(nominal), None) => roots.extend(
                nominal
                    .fields()
                    .iter()
                    .map(RuntimeResolvedNominalRecordField::ty),
            ),
            (None, Some(structural)) => roots.push(structural),
            _ => unreachable!("record pattern fact has one exact owner"),
        }
    }
}

impl RuntimeTryFact {
    pub(super) fn append_normalized_types<'a>(
        &'a self,
        roots: &mut Vec<&'a RuntimeNormalizedType>,
    ) {
        roots.extend([
            self.carrier_type(),
            self.carrier().success(),
            self.boundary_type(),
        ]);
        roots.extend(self.carrier().residual());
    }
}

impl RuntimeIteratorFact {
    pub(super) fn append_normalized_types<'a>(
        &'a self,
        roots: &mut Vec<&'a RuntimeNormalizedType>,
    ) {
        match self {
            Self::Builtin(builtin) => roots.extend([
                builtin.item(),
                builtin.iterator(),
                builtin.next_value(),
                builtin.step(),
            ]),
            Self::Witness(witness) => roots.extend([witness.item(), witness.iterator()]),
        }
    }
}

impl RuntimeContentFragmentFact {
    pub(super) fn append_normalized_types<'a>(
        &'a self,
        roots: &mut Vec<&'a RuntimeNormalizedType>,
    ) {
        roots.extend(self.values().iter().map(RuntimeDialogueValueExpression::ty));
        for effect in self.effects() {
            roots.extend(
                effect
                    .captures()
                    .iter()
                    .map(RuntimeDialogueEffectCaptureFact::ty),
            );
            if let RuntimeDialogueEffectTrigger::Delay {
                duration_type,
                schedule_handle_type,
                ..
            } = effect.trigger()
            {
                roots.extend([duration_type, schedule_handle_type]);
            }
            effect
                .operation()
                .effect()
                .visit_operand_types(&mut |ty| roots.push(ty));
        }
    }
}

impl RuntimeProjectFunctionInstanceFact {
    pub(super) fn append_normalized_types<'a>(
        &'a self,
        roots: &mut Vec<&'a RuntimeNormalizedType>,
    ) {
        roots.push(self.function_type());
        roots.extend(
            self.parameters()
                .iter()
                .flat_map(|parameter| [parameter.abi_ty(), parameter.binding_ty()]),
        );
        if let Some(default) = self.attached_default() {
            roots.push(default.result());
            roots.extend(
                default
                    .captures()
                    .iter()
                    .map(RuntimeProjectAttachedDefaultCapture::binding_ty),
            );
        }
        self.semantics().append_normalized_types(roots);
    }
}

impl RuntimeClosureInstanceFact {
    pub(super) fn append_normalized_types<'a>(
        &'a self,
        roots: &mut Vec<&'a RuntimeNormalizedType>,
    ) {
        roots.push(self.function_type());
        roots.extend(
            self.parameters()
                .iter()
                .map(RuntimeClosureParameterFact::ty),
        );
        roots.extend(self.captures().iter().map(RuntimeClosureCaptureFact::ty));
        self.semantics().append_normalized_types(roots);
    }
}

impl RuntimeProjectFunctionInstanceSemanticFacts {
    fn append_normalized_types<'a>(&'a self, roots: &mut Vec<&'a RuntimeNormalizedType>) {
        roots.extend(
            self.type_projection()
                .iter()
                .filter_map(RuntimeProjectFunctionTypeProjection::ty),
        );
        roots.extend(self.captures().iter().map(RuntimeCheckedCapture::ty));
        for expression in self.expressions() {
            expression.payload().append_normalized_types(roots);
        }
        for pattern in self.patterns() {
            pattern.payload().append_normalized_types(roots);
        }
        for statement in self.statements() {
            statement.payload().append_normalized_types(roots);
        }
    }
}

impl RuntimeProjectFunctionExpressionPayload {
    fn append_normalized_types<'a>(&'a self, roots: &mut Vec<&'a RuntimeNormalizedType>) {
        match self {
            Self::Value(value) => value.append_normalized_types(roots),
            Self::NominalRecord(record) => record.append_normalized_types(roots),
            Self::Variant(variant) => variant.owner().append_normalized_types(roots),
            Self::Call(call) => call.append_normalized_types(roots),
            Self::Try(tried) => tried.append_normalized_types(roots),
            Self::ImplicitCallable {
                callable, tried, ..
            } => {
                roots.extend([callable.parameter(), callable.result()]);
                if let Some(tried) = tried {
                    tried.append_normalized_types(roots);
                }
            }
            Self::DialogueApplication {
                application,
                fragments,
            } => {
                roots.push(application.line_result());
                for fragment in fragments {
                    fragment.append_normalized_types(roots);
                }
            }
            Self::ContentApplication { fragments } => {
                for fragment in fragments {
                    fragment.append_normalized_types(roots);
                }
            }
            Self::Closure(closure) => closure.append_normalized_types(roots),
            Self::Structural
            | Self::Consumed
            | Self::Literal(_)
            | Self::Select(_)
            | Self::PostfixCandidate(_)
            | Self::Await(_)
            | Self::Choice(_)
            | Self::Pipe(_) => {}
        }
    }
}

impl RuntimeProjectFunctionPatternPayload {
    fn append_normalized_types<'a>(&'a self, roots: &mut Vec<&'a RuntimeNormalizedType>) {
        match self {
            Self::NominalRecord(record) => record.append_normalized_types(roots),
            Self::Variant(variant) => variant.owner().append_normalized_types(roots),
            Self::Structural | Self::Literal(_) | Self::Entity(_) | Self::TypedBinding => {}
        }
    }
}

impl RuntimeProjectFunctionStatementPayload {
    fn append_normalized_types<'a>(&'a self, roots: &mut Vec<&'a RuntimeNormalizedType>) {
        match self {
            Self::Assignment(assignment) => {
                roots.extend([assignment.field_type(), assignment.value_type()]);
            }
            Self::EvaluatedEffect(effect) => effect
                .effect()
                .visit_operand_types(&mut |ty| roots.push(ty)),
            Self::Iteration(iteration) => iteration.append_normalized_types(roots),
            Self::Structural
            | Self::Assertion(_)
            | Self::Defer
            | Self::ControlTransfer
            | Self::Trigger(_)
            | Self::UnsafeAudit
            | Self::Select
            | Self::SourceLocale
            | Self::Scope
            | Self::Include
            | Self::Suspension
            | Self::Yield => {}
        }
    }
}
