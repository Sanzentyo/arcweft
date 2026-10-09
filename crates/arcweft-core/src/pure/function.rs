//! Borrowed pure execution capabilities retain their admitted body's owner.

use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::Arc;

use crate::pattern::RuntimePatternKind;
use crate::plan::{
    RuntimeCallableParameter, RuntimeExecutableBody, RuntimeFunctionInputBinding,
    RuntimeFunctionInputSource, RuntimeFunctionSite, RuntimeFunctionSiteBody, RuntimePlan,
    RuntimePureHelper, RuntimePureHelperId, RuntimePureInputType, RuntimePureOutputType,
};
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLocalDeclarationId};
use crate::value::{RuntimeEvalError, RuntimeExpr, RuntimeFunctionApplyError};

/// An admitted source body or an explicitly authored synthetic helper recipe.
/// Neither branch carries a copied expression or a reconstructed signature.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePureFunctionId {
    Recipe(RuntimePureHelperId),
    Function(RuntimeFunctionSiteId),
}

impl From<RuntimePureHelperId> for RuntimePureFunctionId {
    fn from(value: RuntimePureHelperId) -> Self {
        Self::Recipe(value)
    }
}

impl From<RuntimeFunctionSiteId> for RuntimePureFunctionId {
    fn from(value: RuntimeFunctionSiteId) -> Self {
        Self::Function(value)
    }
}

#[derive(Clone, Copy, Debug)]
enum RuntimePureFunctionAuthority<'a> {
    Recipe(&'a RuntimePureHelper),
    Function(&'a RuntimeFunctionSite),
}

/// The original rows remain the input authority. A scalar backend obtains only
/// the physical local alias and representation needed by its compiled frame.
#[derive(Clone, Copy, Debug)]
pub enum RuntimePureFunctionInputRef<'a> {
    Recipe(&'a RuntimeCallableParameter),
    Function {
        plan: &'a RuntimePlan,
        input: &'a RuntimeFunctionInputBinding,
    },
}

impl<'a> RuntimePureFunctionInputRef<'a> {
    /// The complete formal's raw ingress local, before its checked pattern.
    pub fn input_local(self) -> RuntimeLocalDeclarationId {
        match self {
            Self::Recipe(input) => input.local(),
            Self::Function { input, .. } => input.input_local(),
        }
    }

    /// A scalar compiler uses the checked direct binder alias. Destructuring
    /// and constrained patterns remain in the VM's complete function binder.
    pub fn local(self) -> RuntimeLocalDeclarationId {
        match self {
            Self::Recipe(input) => input.local(),
            Self::Function { input, .. } => match input.pattern().kind() {
                RuntimePatternKind::Bind { binding, .. }
                | RuntimePatternKind::Typed { binding } => binding.local(),
                _ => input.input_local(),
            },
        }
    }

    pub fn abi(self) -> RuntimePureInputType {
        match self {
            Self::Recipe(input) => input.abi(),
            Self::Function { plan, input } => plan
                .local_declarations()
                .get(input.input_local())
                .and_then(|local| super::pure_scalar_projection(plan, local.ty()))
                .map_or(RuntimePureInputType::Value, output_as_input),
        }
    }

    pub const fn function_input(self) -> Option<&'a RuntimeFunctionInputBinding> {
        match self {
            Self::Recipe(_) => None,
            Self::Function { input, .. } => Some(input),
        }
    }

    /// Static passing belongs to a formal parameter. A free capture has its
    /// transfer operation on the retained original function input instead.
    pub const fn passing(self) -> Option<crate::plan::RuntimeFunctionParameterPassing> {
        match self {
            Self::Recipe(input) => Some(input.passing()),
            Self::Function { input, .. } => match input.source() {
                RuntimeFunctionInputSource::Parameter { passing, .. }
                | RuntimeFunctionInputSource::CapturedParameter { passing, .. } => Some(passing),
                RuntimeFunctionInputSource::Capture { .. } => None,
            },
        }
    }

    fn admits_scalar_binding(self) -> bool {
        match self {
            Self::Recipe(_) => true,
            Self::Function { input, .. } => matches!(
                input.pattern().kind(),
                RuntimePatternKind::Bind { .. }
                    | RuntimePatternKind::Typed { .. }
                    | RuntimePatternKind::Discard
            ),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimePureFunctionInputs<'a> {
    authority: RuntimePureFunctionAuthority<'a>,
    plan: &'a RuntimePlan,
}

impl<'a> RuntimePureFunctionInputs<'a> {
    pub fn len(self) -> usize {
        match self.authority {
            RuntimePureFunctionAuthority::Recipe(helper) => helper.inputs.len(),
            RuntimePureFunctionAuthority::Function(function) => function.inputs().len(),
        }
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    pub fn get(self, index: usize) -> Option<RuntimePureFunctionInputRef<'a>> {
        match self.authority {
            RuntimePureFunctionAuthority::Recipe(helper) => helper
                .inputs
                .get(index)
                .map(RuntimePureFunctionInputRef::Recipe),
            RuntimePureFunctionAuthority::Function(function) => {
                function
                    .inputs()
                    .get(index)
                    .map(|input| RuntimePureFunctionInputRef::Function {
                        plan: self.plan,
                        input,
                    })
            }
        }
    }

    pub fn first(self) -> Option<RuntimePureFunctionInputRef<'a>> {
        self.get(0)
    }

    pub fn iter(self) -> RuntimePureFunctionInputIter<'a> {
        RuntimePureFunctionInputIter {
            inputs: self,
            front: 0,
            back: self.len(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RuntimePureFunctionInputIter<'a> {
    inputs: RuntimePureFunctionInputs<'a>,
    front: usize,
    back: usize,
}

impl<'a> Iterator for RuntimePureFunctionInputIter<'a> {
    type Item = RuntimePureFunctionInputRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let index = self.front;
        self.front += 1;
        self.inputs.get(index)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.back - self.front;
        (remaining, Some(remaining))
    }
}

impl DoubleEndedIterator for RuntimePureFunctionInputIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        self.inputs.get(self.back)
    }
}

impl ExactSizeIterator for RuntimePureFunctionInputIter<'_> {}

impl<'a> IntoIterator for &RuntimePureFunctionInputs<'a> {
    type Item = RuntimePureFunctionInputRef<'a>;
    type IntoIter = RuntimePureFunctionInputIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// A capability resolved through one exact immutable plan. Its body and
/// ABI views borrow the original admitted rows, including every whole formal,
/// even when a body discards a parameter or never reads it.
#[derive(Clone, Copy, Debug)]
pub struct RuntimePureFunctionRef<'a> {
    plan: &'a Arc<RuntimePlan>,
    authority: RuntimePureFunctionAuthority<'a>,
    view: RuntimePureFunctionView<'a>,
}

/// A physical backend borrows the original admitted body. An executable body
/// retains its checked control operations and return ownership; it is never
/// reconstructed as an expression or copied into a synthetic helper recipe.
#[derive(Clone, Copy, Debug)]
pub enum RuntimePureFunctionBodyRef<'a> {
    Expression(&'a RuntimeExpr),
    Executable(&'a RuntimeExecutableBody),
}

impl<'a> RuntimePureFunctionBodyRef<'a> {
    pub const fn expression(self) -> Option<&'a RuntimeExpr> {
        match self {
            Self::Expression(value) => Some(value),
            Self::Executable(_) => None,
        }
    }

    /// Projects the original body's complete, equally scheduled control paths.
    /// The same lexical target resolver as native/AWBC handles emitted exits,
    /// consumed control scaffolding, typed Unit moves and early-return cleanup.
    pub(crate) fn exact_scheduled_control_ops(
        self,
        function: RuntimePureFunctionRef<'_>,
    ) -> Option<usize> {
        use super::RuntimePureControlBindings;
        use crate::scope::{RuntimeScopeExitTarget, RuntimeScopeFrameKind};
        fn cost(
            function: RuntimePureFunctionRef<'_>,
            ops: &[crate::plan::FlowOp],
            control: &mut RuntimePureControlBindings<'_, BTreeMap<RuntimeLocalDeclarationId, ()>>,
            depth: usize,
        ) -> Option<(usize, bool)> {
            if depth > 128 {
                return None;
            }
            use crate::plan::FlowOp;
            let mut count = 0usize;
            for op in ops {
                count = count.checked_add(1)?;
                match op {
                    FlowOp::Let { pattern, expr } => {
                        if let Some(value) = control.evaluate_unit(expr).ok()? {
                            control.bind_unit(pattern, value).ok()?;
                        } else {
                            if !matches!(
                                pattern.kind(),
                                RuntimePatternKind::Bind { .. }
                                    | RuntimePatternKind::Typed { .. }
                                    | RuntimePatternKind::Discard
                            ) || !scalar_completion_expr_is_total(function.plan(), expr, 0)
                            {
                                return None;
                            }
                            control.consume_numeric_expression(expr).ok()?;
                            if let RuntimePatternKind::Bind { binding, .. }
                            | RuntimePatternKind::Typed { binding } = pattern.kind()
                            {
                                control.numeric_mut().insert(binding.local(), ());
                            }
                        }
                    }
                    FlowOp::EnterScope { .. } => {
                        control.enter_scope(RuntimeScopeFrameKind::EmittedLexical);
                    }
                    FlowOp::ExitScope => control
                        .exit_scope(RuntimeScopeExitTarget::EmittedLexical)
                        .ok()?,
                    FlowOp::ExitScopeBind { pattern, expr } => {
                        let unit = control.evaluate_unit(expr).ok()?;
                        if unit.is_none()
                            && (!scalar_completion_expr_is_total(function.plan(), expr, 0)
                                || !matches!(
                                    pattern.kind(),
                                    RuntimePatternKind::Bind { .. }
                                        | RuntimePatternKind::Typed { .. }
                                        | RuntimePatternKind::Discard
                                ))
                        {
                            return None;
                        }
                        if unit.is_none() {
                            control.consume_numeric_expression(expr).ok()?;
                        }
                        control
                            .exit_scope(RuntimeScopeExitTarget::EmittedLexical)
                            .ok()?;
                        if let Some(value) = unit {
                            control.bind_unit(pattern, value).ok()?;
                        } else if let RuntimePatternKind::Bind { binding, .. }
                        | RuntimePatternKind::Typed { binding } = pattern.kind()
                        {
                            control.numeric_mut().insert(binding.local(), ());
                        }
                    }
                    FlowOp::Noop => {}
                    FlowOp::ReturnExpr(value) => {
                        if !scalar_completion_expr_is_total(function.plan(), value, 0) {
                            return None;
                        }
                        control.consume_numeric_expression(value).ok()?;
                        return Some((count, true));
                    }
                    FlowOp::Scope { body, .. } => {
                        if !body.is_empty() {
                            let scope = control.enter_scope(RuntimeScopeFrameKind::EmittedLexical);
                            count = count.checked_add(1)?;
                            let (nested, returns) = cost(function, body, control, depth + 1)?;
                            count = count.checked_add(nested)?;
                            if returns {
                                return Some((count, true));
                            }
                            if control.contains_scope(scope) {
                                control
                                    .exit_scope(RuntimeScopeExitTarget::Frame(scope))
                                    .ok()?;
                                count = count.checked_add(1)?;
                            }
                        }
                    }
                    FlowOp::If {
                        condition,
                        then_ops,
                        else_ops,
                    } => {
                        if !scalar_completion_expr_is_total(function.plan(), condition, 0) {
                            return None;
                        }
                        control.consume_numeric_expression(condition).ok()?;
                        let branch = |ops: &[FlowOp]| -> Option<(
                            usize,
                            bool,
                            RuntimePureControlBindings<'_, BTreeMap<RuntimeLocalDeclarationId, ()>>,
                        )> {
                            let mut selected = control.clone();
                            let scope = (!ops.is_empty())
                                .then(|| selected.enter_scope(RuntimeScopeFrameKind::Control));
                            let (mut count, returns) =
                                cost(function, ops, &mut selected, depth + 1)?;
                            if let Some(scope) = scope {
                                count = count.checked_add(1)?;
                                if !returns && selected.contains_scope(scope) {
                                    selected
                                        .exit_scope(RuntimeScopeExitTarget::Frame(scope))
                                        .ok()?;
                                    count = count.checked_add(1)?;
                                }
                            }
                            Some((count, returns, selected))
                        };
                        let left = branch(then_ops)?;
                        let right = branch(else_ops)?;
                        if (left.0, left.1) != (right.0, right.1) {
                            return None;
                        }
                        count = count.checked_add(left.0)?;
                        if left.1 {
                            return Some((count, true));
                        }
                        if !left.2.compatible_after_branch(&right.2)
                            || left.2.numeric() != right.2.numeric()
                        {
                            return None;
                        }
                        *control = left.2;
                    }
                    _ => return None,
                }
            }
            Some((count, false))
        }
        match self {
            Self::Expression(_) => Some(0),
            Self::Executable(body) if body.is_effect_free() => {
                let initial = function
                    .inputs
                    .iter()
                    .map(|input| (input.local(), ()))
                    .collect();
                let mut control = RuntimePureControlBindings::new(function, initial);
                let (count, returns) = cost(function, body.ops(), &mut control, 0)?;
                returns.then_some(count)
            }
            Self::Executable(_) => None,
        }
    }
    pub const fn is_executable(self) -> bool {
        matches!(self, Self::Executable(_))
    }
}

/// Read-only physical projection used by pure backends. This view borrows the
/// admitted body and full input rows and cannot issue an execution capability.
#[derive(Clone, Copy, Debug)]
pub struct RuntimePureFunctionView<'a> {
    pub id: RuntimePureFunctionId,
    pub name: &'a str,
    pub inputs: RuntimePureFunctionInputs<'a>,
    pub output_type: RuntimePureOutputType,
    pub body: RuntimePureFunctionBodyRef<'a>,
    pub scalar_eval_supported: bool,
}

impl<'a> RuntimePureFunctionRef<'a> {
    /// Completes only total original numeric control with exactly scheduled cost.
    /// Hidden calls, unequal paths and fallible operators retain native control.
    pub(crate) fn exact_scalar_completion_control_ops(self) -> Option<usize> {
        self.body.exact_scheduled_control_ops(self)
    }

    /// Synthetic recipes and effect-free ordinary definitions are
    /// the eager acceleration roots. Other callable roles remain owned by
    /// their existing invocation or control-transfer boundary.
    ///
    /// # Panics
    ///
    /// Panics while advancing the iterator if a catalogued candidate violates
    /// its admitted pure contract. The immutable plan retains those candidates.
    pub fn eager_candidates(plan: &'a Arc<RuntimePlan>) -> impl Iterator<Item = Self> + 'a {
        let recipes = plan
            .pure_helpers()
            .iter()
            .map(|helper| Self::resolve(plan, helper.id).expect("admitted synthetic pure recipe"));
        let functions = plan
            .function_sites()
            .iter_with_ids()
            .filter(|(_, function)| function.is_eager_pure_candidate())
            .map(|(site, _)| {
                Self::resolve(plan, site).expect("admitted deterministic function body")
            });
        recipes.chain(functions)
    }

    pub fn resolve(
        plan: &'a Arc<RuntimePlan>,
        id: impl Into<RuntimePureFunctionId>,
    ) -> Result<Self, RuntimeEvalError> {
        let id = id.into();
        let (authority, name, body, output_type, scalar_eval_supported) = match id {
            RuntimePureFunctionId::Recipe(id) => {
                let helper = super::resolve_pure_helper(plan, id)?;
                super::validate_pure_helper_contract(plan, helper)?;
                (
                    RuntimePureFunctionAuthority::Recipe(helper),
                    helper.name.as_str(),
                    RuntimePureFunctionBodyRef::Expression(&helper.expr),
                    helper.output_type,
                    helper.scalar_eval_supported,
                )
            }
            RuntimePureFunctionId::Function(site) => {
                let function = plan
                    .function_sites()
                    .get(site)
                    .ok_or(RuntimeFunctionApplyError::UnknownStructuredSite { site })?;
                if !function.invocation_effects().is_empty() || !function.body().is_effect_free() {
                    return Err(RuntimeEvalError::UnsupportedPure {
                        name: "structured.function".to_owned(),
                        reason: "a deterministic backend requires an admitted empty invocation effect row".to_owned(),
                    });
                }
                let output = super::pure_scalar_projection(plan, function.result())
                    .unwrap_or(RuntimePureOutputType::Value);
                let authority = RuntimePureFunctionAuthority::Function(function);
                let body = match function.body() {
                    RuntimeFunctionSiteBody::Expression(expr) => {
                        RuntimePureFunctionBodyRef::Expression(expr)
                    }
                    RuntimeFunctionSiteBody::Executable(body) => {
                        RuntimePureFunctionBodyRef::Executable(body)
                    }
                };
                let inputs = RuntimePureFunctionInputs { authority, plan };
                let scalar = output != RuntimePureOutputType::Value
                    && inputs.iter().all(|input| {
                        input.abi() != RuntimePureInputType::Value && input.admits_scalar_binding()
                    });
                (authority, "structured.function", body, output, scalar)
            }
        };
        Ok(Self {
            plan,
            authority,
            view: RuntimePureFunctionView {
                id,
                name,
                inputs: RuntimePureFunctionInputs { authority, plan },
                output_type,
                body,
                scalar_eval_supported,
            },
        })
    }

    pub const fn plan(self) -> &'a Arc<RuntimePlan> {
        self.plan
    }

    pub const fn id(self) -> RuntimePureFunctionId {
        self.view.id
    }

    /// The expression-only physical subset explicitly declines executable
    /// bodies. General consumers inspect `body` and enter its actual owner.
    pub fn expression(self) -> Result<&'a RuntimeExpr, RuntimeEvalError> {
        self.body
            .expression()
            .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                name: self.name.to_owned(),
                reason: "this expression-only backend requires function control transfer"
                    .to_owned(),
            })
    }

    pub const fn result_type(self) -> crate::runtime_id::RuntimePlanTypeId {
        match self.authority {
            RuntimePureFunctionAuthority::Recipe(helper) => helper.expr.ty(),
            RuntimePureFunctionAuthority::Function(function) => function.result(),
        }
    }

    pub const fn definition(self) -> crate::plan::RuntimeFunctionDefinitionIdentity {
        match self.authority {
            RuntimePureFunctionAuthority::Recipe(helper) => helper.definition,
            RuntimePureFunctionAuthority::Function(function) => function.definition(),
        }
    }

    pub fn supports_scalar_frame(self) -> bool {
        self.inputs
            .iter()
            .all(RuntimePureFunctionInputRef::admits_scalar_binding)
    }

    pub const fn origin(self) -> crate::plan::RuntimePureHelperOrigin {
        match self.authority {
            RuntimePureFunctionAuthority::Recipe(helper) => helper.origin,
            RuntimePureFunctionAuthority::Function(_) => {
                crate::plan::RuntimePureHelperOrigin::Inferred
            }
        }
    }

    /// The original structured declaration keeps static passing, transfer,
    /// whole-formal origins, patterns, ownership and invocation effects.
    pub const fn function_site(self) -> Option<&'a RuntimeFunctionSite> {
        match self.authority {
            RuntimePureFunctionAuthority::Recipe(_) => None,
            RuntimePureFunctionAuthority::Function(function) => Some(function),
        }
    }

    /// Maps the full flat backend frame back to its checked retained/supplied
    /// packets. The VM executes the original binder and type instantiation.
    pub(super) fn split_function_arguments(
        self,
        values: impl IntoIterator<Item = crate::value::RuntimeValue>,
    ) -> Result<
        (
            Vec<crate::value::RuntimeValue>,
            Vec<crate::value::RuntimeValue>,
        ),
        RuntimeEvalError,
    > {
        let function = self.function_site().expect("function capability");
        let mut captures = vec![None; function.capture_inputs().count()];
        let mut arguments = vec![None; function.parameter_inputs().count()];
        let mut values = values.into_iter();
        for input in function.inputs() {
            let value = values
                .next()
                .ok_or_else(|| RuntimeEvalError::TooManyPureArgs {
                    helper: self.name.to_owned(),
                    max: function.inputs().len(),
                    found: function.inputs().len().saturating_sub(1),
                })?;
            let (packet, position) = match input.source() {
                RuntimeFunctionInputSource::Capture { position }
                | RuntimeFunctionInputSource::CapturedParameter { position, .. } => {
                    (&mut captures, position)
                }
                RuntimeFunctionInputSource::Parameter { position, .. } => {
                    (&mut arguments, position)
                }
            };
            let slot = packet.get_mut(position as usize).ok_or_else(|| {
                RuntimeEvalError::UnsupportedPure {
                    name: self.name.to_owned(),
                    reason: "function input packet is not canonical".to_owned(),
                }
            })?;
            if slot.replace(value).is_some() {
                return Err(RuntimeEvalError::UnsupportedPure {
                    name: self.name.to_owned(),
                    reason: "function input packet repeats a whole formal".to_owned(),
                });
            }
        }
        if values.next().is_some() {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: self.name.to_owned(),
                max: function.inputs().len(),
                found: function.inputs().len() + 1,
            });
        }
        let complete = |packet: Vec<Option<crate::value::RuntimeValue>>| {
            packet
                .into_iter()
                .map(|value| {
                    value.ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                        name: self.name.to_owned(),
                        reason: "function input packet omits a whole formal".to_owned(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        };
        Ok((complete(captures)?, complete(arguments)?))
    }
}

impl<'a> Deref for RuntimePureFunctionRef<'a> {
    type Target = RuntimePureFunctionView<'a>;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

fn output_as_input(output: RuntimePureOutputType) -> RuntimePureInputType {
    match output {
        RuntimePureOutputType::I8 => RuntimePureInputType::I8,
        RuntimePureOutputType::I16 => RuntimePureInputType::I16,
        RuntimePureOutputType::I32 => RuntimePureInputType::I32,
        RuntimePureOutputType::I64 => RuntimePureInputType::I64,
        RuntimePureOutputType::I128 => RuntimePureInputType::I128,
        RuntimePureOutputType::ISize => RuntimePureInputType::ISize,
        RuntimePureOutputType::U8 => RuntimePureInputType::U8,
        RuntimePureOutputType::U16 => RuntimePureInputType::U16,
        RuntimePureOutputType::U32 => RuntimePureInputType::U32,
        RuntimePureOutputType::U64 => RuntimePureInputType::U64,
        RuntimePureOutputType::U128 => RuntimePureInputType::U128,
        RuntimePureOutputType::USize => RuntimePureInputType::USize,
        RuntimePureOutputType::F32 => RuntimePureInputType::F32,
        RuntimePureOutputType::F64 => RuntimePureInputType::F64,
        RuntimePureOutputType::Bool | RuntimePureOutputType::Value => RuntimePureInputType::Value,
    }
}

/// A conservative projection of the already admitted numeric expression.
/// This is an eligibility check, never a second evaluator or a body recipe.
fn scalar_completion_expr_is_total(plan: &RuntimePlan, value: &RuntimeExpr, depth: usize) -> bool {
    if depth > 128 {
        return false;
    }
    use crate::plan::RuntimePureOutputType as O;
    use crate::value::{RuntimeBinaryOp, RuntimeExprKind, RuntimeUnaryOp, RuntimeValue};
    let total = |value| scalar_completion_expr_is_total(plan, value, depth + 1);
    match value.kind() {
        RuntimeExprKind::Value(value) => matches!(
            value,
            RuntimeValue::Bool(_)
                | RuntimeValue::Int(_)
                | RuntimeValue::UInt(_)
                | RuntimeValue::F32(_)
                | RuntimeValue::F64(_)
        ),
        RuntimeExprKind::Local(read) => {
            read.fields().is_empty()
                && matches!(
                    super::pure_scalar_projection(plan, value.ty()),
                    Some(
                        O::I8
                            | O::I16
                            | O::I32
                            | O::I64
                            | O::I128
                            | O::ISize
                            | O::U8
                            | O::U16
                            | O::U32
                            | O::U64
                            | O::U128
                            | O::USize
                            | O::F32
                            | O::F64
                            | O::Bool
                    )
                )
        }
        RuntimeExprKind::Let { expr, body, .. } => total(expr) && total(body),
        RuntimeExprKind::Scope { body, .. } => total(body),
        RuntimeExprKind::Unary { op, expr } => {
            let projection = super::pure_scalar_projection(plan, value.ty());
            match op {
                RuntimeUnaryOp::Not => projection == Some(O::Bool) && total(expr),
                RuntimeUnaryOp::Neg => {
                    matches!(
                        projection,
                        Some(
                            O::I8 | O::I16 | O::I32 | O::I64 | O::I128 | O::ISize | O::F32 | O::F64
                        )
                    ) && total(expr)
                }
            }
        }
        RuntimeExprKind::Binary { lhs, op, rhs } => {
            (*op != RuntimeBinaryOp::Div
                || matches!(
                    super::pure_scalar_projection(plan, value.ty()),
                    Some(O::F32 | O::F64)
                ))
                && total(lhs)
                && total(rhs)
        }
        RuntimeExprKind::If {
            condition,
            then_expr,
            else_expr,
        } => total(condition) && total(then_expr) && total(else_expr),
        RuntimeExprKind::Call { callee, args } => {
            callee.as_intrinsic() == Some(crate::value::RuntimeIntrinsic::Add)
                && args.len() == 2
                && args.iter().all(|argument| {
                    argument.mode() == crate::value::RuntimeCallArgumentMode::Value
                        && total(argument.value())
                })
        }
        RuntimeExprKind::Agent(_)
        | RuntimeExprKind::SequencePopFront { .. }
        | RuntimeExprKind::SequencePush { .. }
        | RuntimeExprKind::SequencePopBack { .. }
        | RuntimeExprKind::EntityRef(_)
        | RuntimeExprKind::Tuple(_)
        | RuntimeExprKind::DialogueContent { .. }
        | RuntimeExprKind::FormatContent { .. }
        | RuntimeExprKind::CharacterDialogue { .. }
        | RuntimeExprKind::BracketSeq(_)
        | RuntimeExprKind::RepeatSeq { .. }
        | RuntimeExprKind::Range { .. }
        | RuntimeExprKind::NominalRecord(_)
        | RuntimeExprKind::Variant { .. }
        | RuntimeExprKind::Field { .. }
        | RuntimeExprKind::ProjectTuple { .. }
        | RuntimeExprKind::ProjectRecord { .. }
        | RuntimeExprKind::Assign { .. }
        | RuntimeExprKind::MakeCallable { .. }
        | RuntimeExprKind::SpecializeCallable { .. }
        | RuntimeExprKind::ApplyGroup { .. }
        | RuntimeExprKind::TraitCall { .. }
        | RuntimeExprKind::PureCall { .. }
        | RuntimeExprKind::StandardMap { .. }
        | RuntimeExprKind::Sum { .. }
        | RuntimeExprKind::IfLet { .. }
        | RuntimeExprKind::Match { .. }
        | RuntimeExprKind::ReductionUnchanged { .. } => false,
    }
}
