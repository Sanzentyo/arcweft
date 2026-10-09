//! Borrowed pure execution capabilities retain their admitted body's owner.

use std::ops::Deref;
use std::sync::Arc;

use crate::pattern::RuntimePatternKind;
use crate::plan::{
    RuntimeCallableParameter, RuntimeFunctionInputBinding, RuntimeFunctionInputSource,
    RuntimeFunctionSite, RuntimePlan, RuntimePureHelper, RuntimePureHelperId, RuntimePureInputType,
    RuntimePureOutputType,
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

/// A capability resolved through one exact immutable plan. Its expression and
/// ABI views borrow the original admitted rows, including every whole formal,
/// even when a body discards a parameter or never reads it.
#[derive(Clone, Copy, Debug)]
pub struct RuntimePureFunctionRef<'a> {
    plan: &'a Arc<RuntimePlan>,
    authority: RuntimePureFunctionAuthority<'a>,
    view: RuntimePureFunctionView<'a>,
}

/// Read-only physical projection used by pure backends. This view borrows the
/// admitted body and full input rows and cannot issue an execution capability.
#[derive(Clone, Copy, Debug)]
pub struct RuntimePureFunctionView<'a> {
    pub id: RuntimePureFunctionId,
    pub name: &'a str,
    pub inputs: RuntimePureFunctionInputs<'a>,
    pub output_type: RuntimePureOutputType,
    pub expr: &'a RuntimeExpr,
    pub scalar_eval_supported: bool,
}

impl<'a> RuntimePureFunctionRef<'a> {
    /// Synthetic recipes and effect-free ordinary expression definitions are
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
                Self::resolve(plan, site).expect("admitted deterministic expression function")
            });
        recipes.chain(functions)
    }

    pub fn resolve(
        plan: &'a Arc<RuntimePlan>,
        id: impl Into<RuntimePureFunctionId>,
    ) -> Result<Self, RuntimeEvalError> {
        let id = id.into();
        let (authority, name, expr, output_type, scalar_eval_supported) = match id {
            RuntimePureFunctionId::Recipe(id) => {
                let helper = super::resolve_pure_helper(plan, id)?;
                super::validate_pure_helper_contract(plan, helper)?;
                (
                    RuntimePureFunctionAuthority::Recipe(helper),
                    helper.name.as_str(),
                    &helper.expr,
                    helper.output_type,
                    helper.scalar_eval_supported,
                )
            }
            RuntimePureFunctionId::Function(site) => {
                let function = plan
                    .function_sites()
                    .get(site)
                    .ok_or(RuntimeFunctionApplyError::UnknownStructuredSite { site })?;
                let expr = function.body().expression().ok_or_else(|| {
                    RuntimeEvalError::UnsupportedPure {
                        name: "structured.function".to_owned(),
                        reason: "a control-transfer body uses the complete function-call runtime"
                            .to_owned(),
                    }
                })?;
                if !function.invocation_effects().is_empty() {
                    return Err(RuntimeEvalError::UnsupportedPure {
                        name: "structured.function".to_owned(),
                        reason: "a deterministic backend requires an admitted empty invocation effect row".to_owned(),
                    });
                }
                let output = super::pure_scalar_projection(plan, function.result())
                    .unwrap_or(RuntimePureOutputType::Value);
                let authority = RuntimePureFunctionAuthority::Function(function);
                let inputs = RuntimePureFunctionInputs { authority, plan };
                let scalar = output != RuntimePureOutputType::Value
                    && inputs.iter().all(|input| {
                        input.abi() != RuntimePureInputType::Value && input.admits_scalar_binding()
                    });
                (authority, "structured.function", expr, output, scalar)
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
                expr,
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
