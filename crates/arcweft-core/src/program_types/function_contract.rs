//! Scoped callable relations and immutable declaration effect solutions.

use crate::awbc::schema::{AwbcProgram, AwbcRuntimeTypeShape, AwbcTypeId};
use crate::{
    effect_row::{DecisionControl, DecisionWork, EffectPredicate},
    plan::{
        RuntimeArrayLength, RuntimeBoundConstReference, RuntimeBoundEffectReference,
        RuntimeBoundTypeReference, RuntimeTypeBinder, RuntimeTypeScope,
    },
    value::RuntimeValue,
};
use std::collections::{BTreeMap, BTreeSet};
mod authority;
use crate::pattern::RuntimeSemanticTypeId;
use authority::FunctionTypeAuthority;
pub(crate) use authority::RuntimeValueTypeContext;

impl crate::value::RuntimePlaceStorage<RuntimeValue> {
    /// Checks initialized descendants and partial record headers against the
    /// same selected executable and immutable frame binding as complete values.
    pub(crate) fn matches_program_type(
        &self,
        program: super::RuntimeProgramTypes<'_>,
        expected: RuntimeSemanticTypeId,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
    ) -> bool {
        match program {
            super::RuntimeProgramTypes::Plan(plan) => plan
                .by_semantic(expected)
                .is_some_and(|ty| self.matches_authority_type(plan, ty, binding, 0)),
            super::RuntimeProgramTypes::Awbc(program) => program
                .by_semantic(expected)
                .is_some_and(|ty| self.matches_authority_type(program, ty, binding, 0)),
        }
    }

    fn matches_authority_type<A: FunctionTypeAuthority>(
        &self,
        program: &A,
        expected: A::Type,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        depth: usize,
    ) -> bool {
        if depth > crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH
            || program.scope(expected).is_none()
        {
            return false;
        }
        if let Some(value) = self.as_ref() {
            return match binding {
                Some(binding) => binding.value_matches(program, expected, value),
                None => {
                    program
                        .scope(expected)
                        .is_some_and(RuntimeTypeScope::is_root)
                        && program.value_matches(expected, value.view(), depth)
                }
            };
        }
        if self.is_vacant() {
            return true;
        }
        let Some((header, children)) = self.record_parts() else {
            return false;
        };
        !children.is_empty()
            && children.iter().enumerate().all(|(ordinal, child)| {
                program
                    .record_field(expected, header, children.len(), ordinal)
                    .is_some_and(|ty| child.matches_authority_type(program, ty, binding, depth + 1))
            })
    }
}

/// One immutable solution of a function frame's declaration effect scope.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "EffectInstantiationWire", into = "EffectInstantiationWire")]
pub struct RuntimeFunctionEffectInstantiation {
    context: RuntimeSemanticTypeId,
    effects: Box<[crate::effect_row::EffectSet]>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectInstantiationWire {
    context: RuntimeSemanticTypeId,
    effects: Box<[Box<[String]>]>,
}
impl From<RuntimeFunctionEffectInstantiation> for EffectInstantiationWire {
    fn from(value: RuntimeFunctionEffectInstantiation) -> Self {
        Self {
            context: value.context,
            effects: value
                .effects
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|effect| effect.as_str().to_owned())
                        .collect()
                })
                .collect(),
        }
    }
}
impl TryFrom<EffectInstantiationWire> for RuntimeFunctionEffectInstantiation {
    type Error = &'static str;
    fn try_from(value: EffectInstantiationWire) -> Result<Self, Self::Error> {
        let effects = value
            .effects
            .into_vec()
            .into_iter()
            .map(|row| {
                if row.windows(2).any(|pair| pair[0] >= pair[1]) {
                    return Err("effect instantiation rows must be sorted and unique");
                }
                row.into_vec()
                    .into_iter()
                    .map(|label| {
                        arcweft_id::EffectId::parse(label)
                            .map_err(|_| "invalid effect instantiation label")
                    })
                    .collect::<Result<crate::effect_row::EffectSet, _>>()
            })
            .collect::<Result<Box<[_]>, _>>()?;
        Ok(Self {
            context: value.context,
            effects,
        })
    }
}

impl RuntimeFunctionEffectInstantiation {
    fn admits_origin<A: FunctionTypeAuthority>(
        program: &A,
        ty: A::Type,
        origin: Option<&Self>,
    ) -> bool {
        let Some(scope) = program.scope(ty) else {
            return false;
        };
        if scope.is_root() {
            origin.is_none()
        } else {
            origin
                .and_then(|origin| origin.matcher(program))
                .is_some_and(|matcher| &matcher.parameters.scope == scope)
        }
    }
    pub(crate) fn with_value_relation<A: FunctionTypeAuthority, R>(
        program: &A,
        binding: Option<&Self>,
        operation: impl FnOnce(&mut CallableValueRelation<'_, '_, A>) -> R,
    ) -> Option<R> {
        let mut matcher = match binding {
            Some(binding) => binding.matcher(program)?,
            None => ParameterMatcher::root(program),
        };
        let environment = matcher.parameters.clone();
        let result = operation(&mut CallableValueRelation {
            matcher: &mut matcher,
            environment,
            choices: Vec::new(),
        });
        matcher.accepted().then_some(result)
    }
    fn matcher<'program, A: FunctionTypeAuthority>(
        &self,
        program: &'program A,
    ) -> Option<ParameterMatcher<'program, A>> {
        let context = program.by_semantic(self.context)?;
        let (_, mut matcher) = ParameterMatcher::for_function(program, context, None)?;
        let count = matcher
            .parameters
            .scope
            .binders()
            .first()
            .map_or(0, |binder| binder.effects());
        if usize::try_from(count).ok()? != self.effects.len() {
            return None;
        }
        matcher.replacements = self
            .effects
            .iter()
            .enumerate()
            .map(|(slot, effects)| {
                Some((
                    ContractVariable::Parameter(u32::try_from(slot).ok()?),
                    crate::effect_row::EffectFormula::literal(effects.clone(), None),
                ))
            })
            .collect::<Option<_>>()?;
        matcher.predicate = matcher
            .predicate
            .substitute(&matcher.replacements, &mut matcher.work)
            .ok()?;
        if !matcher.predicate.is_unconstrained() {
            return None;
        }
        Some(matcher)
    }
    pub(crate) fn context(&self) -> RuntimeSemanticTypeId {
        self.context
    }
    pub(crate) fn is_valid<A: FunctionTypeAuthority>(&self, program: &A) -> bool {
        self.matcher(program).is_some()
    }
    pub(crate) fn value_matches<A: FunctionTypeAuthority>(
        &self,
        program: &A,
        ty: A::Type,
        value: &RuntimeValue,
    ) -> bool {
        self.value_view_matches(program, ty, value.view())
    }
    pub(crate) fn value_view_matches<A: FunctionTypeAuthority>(
        &self,
        program: &A,
        ty: A::Type,
        value: crate::value::RuntimeValueView<'_>,
    ) -> bool {
        let Some(mut matcher) = self.matcher(program) else {
            return false;
        };
        let environment = matcher.parameters.clone();
        matcher.value_view(ty, value, 0, &environment).is_ok() && matcher.accepted()
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ContractVariable {
    Parameter(u32),
    Source(u32),
    Local { binder: u32, slot: u32 },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BinderOwner {
    Parameters,
    Sources,
    Local(u32),
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct ContractEnvironment {
    scope: RuntimeTypeScope,
    owners: Vec<BinderOwner>,
}
impl ContractEnvironment {
    fn root() -> Self {
        Self {
            scope: RuntimeTypeScope::root(),
            owners: Vec::new(),
        }
    }
    fn enter(&self, binder: RuntimeTypeBinder, owner: BinderOwner) -> Result<Self, ()> {
        let mut next = self.clone();
        next.scope = self.scope.enter(binder).map_err(|_| ())?;
        if !binder.is_empty() {
            next.owners.push(owner);
        }
        Ok(next)
    }
    fn at_scope(&self, scope: &RuntimeTypeScope) -> Result<Self, ()> {
        if scope.is_root() {
            Ok(Self::root())
        } else if scope == &self.scope {
            Ok(self.clone())
        } else {
            Err(())
        }
    }
    fn variable(
        &self,
        reference: &RuntimeBoundEffectReference,
        bindable: Option<&BTreeSet<u32>>,
    ) -> Result<ContractVariable, ()> {
        self.scope.validate_effect(*reference).map_err(|_| ())?;
        Ok(match self.owner(reference.depth())? {
            BinderOwner::Parameters
                if bindable.is_none_or(|slots| slots.contains(&reference.slot())) =>
            {
                ContractVariable::Parameter(reference.slot())
            }
            BinderOwner::Parameters | BinderOwner::Sources => {
                ContractVariable::Source(reference.slot())
            }
            BinderOwner::Local(binder) => ContractVariable::Local {
                binder,
                slot: reference.slot(),
            },
        })
    }
    fn owner(&self, depth: u32) -> Result<BinderOwner, ()> {
        let index = self
            .owners
            .len()
            .checked_sub(
                usize::try_from(depth)
                    .map_err(|_| ())?
                    .checked_add(1)
                    .ok_or(())?,
            )
            .ok_or(())?;
        self.owners.get(index).copied().ok_or(())
    }
    fn type_variable(
        &self,
        reference: RuntimeBoundTypeReference,
    ) -> Result<(BinderOwner, u16), ()> {
        self.scope.validate_type(reference).map_err(|_| ())?;
        Ok((self.owner(reference.depth())?, reference.slot()))
    }
    fn const_variable(
        &self,
        reference: RuntimeBoundConstReference,
    ) -> Result<(BinderOwner, u16), ()> {
        self.scope
            .validate_length(RuntimeArrayLength::Bound(reference))
            .map_err(|_| ())?;
        Ok((self.owner(reference.depth())?, reference.slot()))
    }
}
struct ContractWork(u64);
impl DecisionControl for ContractWork {
    type Error = ();
    fn charge(&mut self, _: DecisionWork) -> Result<(), ()> {
        self.0 = self.0.checked_sub(1).ok_or(())?;
        Ok(())
    }
}
struct ParameterMatcher<'a, A: FunctionTypeAuthority> {
    program: &'a A,
    predicate: EffectPredicate<ContractVariable>,
    source_predicate: EffectPredicate<ContractVariable>,
    parameters: ContractEnvironment,
    bindable: Option<BTreeSet<u32>>,
    locals: BTreeSet<ContractVariable>,
    sources: BTreeSet<ContractVariable>,
    next_binder: u32,
    work: ContractWork,
    replacements: BTreeMap<ContractVariable, crate::effect_row::EffectFormula<ContractVariable>>,
}

struct ValueChoiceRelation {
    before: EffectPredicate<ContractVariable>,
    selected: Option<EffectPredicate<ContractVariable>>,
}
pub(crate) struct CallableValueRelation<'m, 'p, A: FunctionTypeAuthority> {
    matcher: &'m mut ParameterMatcher<'p, A>,
    environment: ContractEnvironment,
    choices: Vec<ValueChoiceRelation>,
}
impl<A: FunctionTypeAuthority> RuntimeValueTypeContext<A::Type>
    for CallableValueRelation<'_, '_, A>
{
    fn permits_scope(&self, scope: &RuntimeTypeScope) -> bool {
        self.environment.at_scope(scope).is_ok()
    }
    fn callable(&mut self, expected: A::Type, actual: A::Type) -> bool {
        self.matcher
            .types(
                expected,
                actual,
                0,
                &self.environment,
                &ContractEnvironment::root(),
            )
            .is_ok()
            && !self.matcher.predicate.is_impossible()
    }
    fn nominal(
        &mut self,
        expected: A::Type,
        actual: A::Type,
        origin: Option<&RuntimeFunctionEffectInstantiation>,
    ) -> bool {
        let Some(scope) = self.matcher.program.scope(actual) else {
            return false;
        };
        let source = if scope.is_root() {
            if origin.is_some() {
                return false;
            }
            ContractEnvironment::root()
        } else {
            let Some(origin) = origin else {
                return false;
            };
            let Some(source) = self.matcher.origin_environment(origin) else {
                return false;
            };
            if &source.scope != scope {
                return false;
            }
            source
        };
        self.matcher
            .types(expected, actual, 0, &self.environment, &source)
            .is_ok()
            && !self.matcher.predicate.is_impossible()
    }
    fn begin_choice(&mut self) {
        self.choices.push(ValueChoiceRelation {
            before: self.matcher.predicate.clone(),
            selected: None,
        });
    }
    fn begin_alternative(&mut self) {
        self.matcher.predicate = self
            .choices
            .last()
            .expect("active value Choice")
            .before
            .clone();
    }
    fn finish_alternative(&mut self, accepted: bool) {
        let choice = self.choices.last_mut().expect("active value Choice");
        if accepted && choice.selected.is_none() {
            choice.selected = Some(self.matcher.predicate.clone());
        }
        self.matcher.predicate = choice.before.clone();
    }
    fn finish_choice(&mut self) {
        let choice = self.choices.pop().expect("active value Choice");
        self.matcher.predicate = choice.selected.unwrap_or(choice.before);
    }
}
impl AwbcProgram {
    pub(crate) fn instantiate_function_effects(
        &self,
        context: AwbcTypeId,
        inputs: &[&RuntimeValue],
    ) -> Option<RuntimeFunctionEffectInstantiation> {
        ParameterMatcher::instantiate(self, context, inputs)
    }
}

impl super::RuntimeProgramTypes<'_> {
    pub(crate) fn type_origin_is_valid(
        self,
        ty: RuntimeSemanticTypeId,
        origin: Option<&RuntimeFunctionEffectInstantiation>,
    ) -> bool {
        match self {
            Self::Plan(plan) => plan.by_semantic(ty).is_some_and(|ty| {
                RuntimeFunctionEffectInstantiation::admits_origin(plan, ty, origin)
            }),
            Self::Awbc(program) => program.by_semantic(ty).is_some_and(|ty| {
                RuntimeFunctionEffectInstantiation::admits_origin(program, ty, origin)
            }),
        }
    }
    pub(crate) fn instantiate_function_effects(
        &self,
        context: RuntimeSemanticTypeId,
        inputs: &[&RuntimeValue],
    ) -> Option<RuntimeFunctionEffectInstantiation> {
        match self {
            Self::Plan(plan) => {
                ParameterMatcher::instantiate(*plan, plan.by_semantic(context)?, inputs)
            }
            Self::Awbc(program) => {
                ParameterMatcher::instantiate(*program, program.by_semantic(context)?, inputs)
            }
        }
    }
}

impl<A: FunctionTypeAuthority> ParameterMatcher<'_, A> {
    fn instantiate(
        program: &A,
        context: A::Type,
        inputs: &[&RuntimeValue],
    ) -> Option<RuntimeFunctionEffectInstantiation> {
        let (parameters, mut matcher) = ParameterMatcher::for_function(program, context, None)?;
        if parameters.len() != inputs.len() {
            return None;
        }
        let environment = matcher.parameters.clone();
        for (expected, value) in parameters.iter().zip(inputs) {
            matcher.value(*expected, value, 0, &environment).ok()?;
        }
        let count = environment
            .scope
            .binders()
            .first()
            .map_or(0, |binder| binder.effects());
        let variables = (0..count).map(ContractVariable::Parameter).collect();
        let predicate = matcher
            .predicate
            .universally_quantified(&matcher.locals, &mut matcher.work)
            .ok()?;
        let completion = predicate.complete(&variables, &mut matcher.work).ok()?;
        if !completion.admissibility.is_unconstrained() {
            return None;
        }
        let least = completion.least?;
        let effects = (0..count)
            .map(|slot| {
                least
                    .get(&ContractVariable::Parameter(slot))?
                    .closed(&mut matcher.work)
                    .ok()?
            })
            .collect::<Option<Box<[_]>>>()?;
        Some(RuntimeFunctionEffectInstantiation {
            context: program.semantic(context)?,
            effects,
        })
    }
}
impl AwbcProgram {
    /// Jointly binds concrete supplied types; declaration-scoped rows require
    /// their owning source contract through the separate default boundary.
    pub fn parameter_contract_accepts_types(
        &self,
        contract: AwbcTypeId,
        inputs: impl IntoIterator<Item = (usize, AwbcTypeId)>,
    ) -> bool {
        let Some((parameters, mut matcher)) = ParameterMatcher::new(self, contract, None) else {
            return false;
        };
        let mut seen = BTreeSet::new();
        for (ordinal, actual) in inputs {
            if self
                .runtime_types
                .get(actual.index())
                .is_none_or(|row| !row.scope().is_root())
            {
                return false;
            }
            let Some(expected) = parameters.get(ordinal) else {
                return false;
            };
            let environment = matcher.parameters.clone();
            if !seen.insert(ordinal)
                || matcher
                    .types(
                        *expected,
                        actual,
                        0,
                        &environment,
                        &ContractEnvironment::root(),
                    )
                    .is_err()
            {
                return false;
            }
        }
        matcher.accepted()
    }
    pub fn parameter_contract_accepts_values<'value>(
        &self,
        contract: AwbcTypeId,
        inputs: impl IntoIterator<Item = (usize, &'value RuntimeValue)>,
    ) -> bool {
        let Some((parameters, mut matcher)) = ParameterMatcher::new(self, contract, None) else {
            return false;
        };
        let mut seen = BTreeSet::new();
        for (ordinal, value) in inputs {
            let Some(expected) = parameters.get(ordinal) else {
                return false;
            };
            let environment = matcher.parameters.clone();
            if !seen.insert(ordinal) || matcher.value(*expected, value, 0, &environment).is_err() {
                return false;
            }
        }
        matcher.accepted()
    }
    /// Checks a default for every admitted source frame, allowing only the
    /// destination's unshared input rows to be chosen for that invocation.
    pub fn parameter_contract_accepts_default(
        &self,
        contract: AwbcTypeId,
        ordinal: usize,
        source_contract: Option<AwbcTypeId>,
        actual: AwbcTypeId,
    ) -> bool {
        let Some(row) = self.runtime_types.get(contract.index()) else {
            return false;
        };
        let AwbcRuntimeTypeShape::Function {
            contract: header,
            parameters,
            ..
        } = row.shape()
        else {
            return false;
        };
        let Some(expected) = parameters.get(ordinal) else {
            return false;
        };
        let Ok(environment) =
            ContractEnvironment::root().enter(header.binder(), BinderOwner::Parameters)
        else {
            return false;
        };
        let mut work =
            ContractWork(crate::entry::RuntimeSchemaLimits::engine_default().max_validation_work);
        let Ok(mut bindable) = collect_parameter_effects(self, *expected, &environment, &mut work)
        else {
            return false;
        };
        for earlier in parameters.iter().take(ordinal) {
            let Ok(shared) = collect_parameter_effects(self, *earlier, &environment, &mut work)
            else {
                return false;
            };
            bindable.retain(|slot| !shared.contains(slot));
        }
        let Some((_, mut matcher)) = ParameterMatcher::new(self, contract, Some(bindable)) else {
            return false;
        };
        let spent =
            crate::entry::RuntimeSchemaLimits::engine_default().max_validation_work - work.0;
        let Some(remaining) = matcher.work.0.checked_sub(spent) else {
            return false;
        };
        matcher.work.0 = remaining;
        let source = match source_contract {
            None => ContractEnvironment::root(),
            Some(source) => {
                let Some(row) = self.runtime_types.get(source.index()) else {
                    return false;
                };
                let AwbcRuntimeTypeShape::Function {
                    contract: source,
                    result,
                    ..
                } = row.shape()
                else {
                    return false;
                };
                if !row.scope().is_root()
                    || *result != actual
                    || (!source.binder().is_empty() && source.binder() != header.binder())
                    || source.invocation() != &crate::effect_row::EffectFormula::empty()
                {
                    return false;
                }
                let Ok(environment) =
                    ContractEnvironment::root().enter(source.binder(), BinderOwner::Sources)
                else {
                    return false;
                };
                let Ok(predicate) = source
                    .predicate()
                    .map_references(&mut matcher.work, &mut |reference, _| {
                        environment.variable(reference, None)
                    })
                else {
                    return false;
                };
                matcher.observe(&predicate);
                matcher.source_predicate = predicate;
                environment
            }
        };
        let environment = matcher.parameters.clone();
        matcher
            .types(*expected, actual, 0, &environment, &source)
            .is_ok()
            && matcher.accepted()
    }
}
impl<'a, A: FunctionTypeAuthority> ParameterMatcher<'a, A> {
    fn root(program: &'a A) -> Self {
        Self {
            program,
            predicate: EffectPredicate::unconstrained(),
            source_predicate: EffectPredicate::unconstrained(),
            parameters: ContractEnvironment::root(),
            bindable: None,
            locals: BTreeSet::new(),
            sources: BTreeSet::new(),
            next_binder: 0,
            work: ContractWork(
                crate::entry::RuntimeSchemaLimits::engine_default().max_validation_work,
            ),
            replacements: BTreeMap::new(),
        }
    }
    fn origin_environment(
        &mut self,
        origin: &RuntimeFunctionEffectInstantiation,
    ) -> Option<ContractEnvironment> {
        let source = origin.matcher(self.program)?;
        let spent = crate::entry::RuntimeSchemaLimits::engine_default()
            .max_validation_work
            .checked_sub(source.work.0)?;
        self.work.0 = self.work.0.checked_sub(spent)?;
        let binder = self.next_binder;
        self.next_binder = binder.checked_add(1)?;
        let environment = ContractEnvironment {
            scope: source.parameters.scope,
            owners: source
                .parameters
                .owners
                .iter()
                .map(|_| BinderOwner::Local(binder))
                .collect(),
        };
        for (slot, effects) in origin.effects.iter().enumerate() {
            self.replacements.insert(
                ContractVariable::Local {
                    binder,
                    slot: u32::try_from(slot).ok()?,
                },
                crate::effect_row::EffectFormula::literal(effects.clone(), None),
            );
        }
        Some(environment)
    }
    fn new(
        program: &'a A,
        contract: A::Type,
        bindable: Option<BTreeSet<u32>>,
    ) -> Option<(&'a [A::Type], Self)> {
        let (header, _, result) = program.function(contract)?;
        if header.binder().effects() == 0
            || header.invocation() != &crate::effect_row::EffectFormula::empty()
            || !program.is_unit(result)
        {
            return None;
        }
        Self::for_function(program, contract, bindable)
    }
    fn for_function(
        program: &'a A,
        contract: A::Type,
        bindable: Option<BTreeSet<u32>>,
    ) -> Option<(&'a [A::Type], Self)> {
        let scope = program.scope(contract)?;
        let (contract, parameters, _) = program.function(contract)?;
        if !scope.is_root()
            || contract.binder().types() != 0
            || contract.binder().const_lengths() != 0
            || contract.invocation().variables().next().is_some()
        {
            return None;
        }
        let parameters_environment = ContractEnvironment::root()
            .enter(contract.binder(), BinderOwner::Parameters)
            .ok()?;
        let mut work =
            ContractWork(crate::entry::RuntimeSchemaLimits::engine_default().max_validation_work);
        let predicate = contract
            .predicate()
            .map_references(&mut work, &mut |reference, _| {
                parameters_environment.variable(reference, bindable.as_ref())
            })
            .ok()?;
        let mut matcher = Self {
            program,
            predicate,
            source_predicate: EffectPredicate::unconstrained(),
            parameters: parameters_environment,
            bindable,
            locals: BTreeSet::new(),
            sources: BTreeSet::new(),
            next_binder: 0,
            work,
            replacements: BTreeMap::new(),
        };
        matcher.observe(&matcher.predicate.clone());
        Some((parameters, matcher))
    }
    fn observe(&mut self, predicate: &EffectPredicate<ContractVariable>) {
        for variable in predicate.variables() {
            match variable {
                ContractVariable::Local { .. } => {
                    self.locals.insert(variable.clone());
                }
                ContractVariable::Source(_) => {
                    self.sources.insert(variable.clone());
                }
                ContractVariable::Parameter(_) => {}
            }
        }
    }
    fn accepted(mut self) -> bool {
        let Ok(predicate) = self
            .predicate
            .universally_quantified(&self.locals, &mut self.work)
        else {
            return false;
        };
        let parameters = predicate
            .variables()
            .filter(|v| matches!(v, ContractVariable::Parameter(_)))
            .cloned()
            .collect();
        let Ok(predicate) = predicate.project(&parameters, &mut self.work) else {
            return false;
        };
        let Ok(predicate) = self.source_predicate.implies(&predicate, &mut self.work) else {
            return false;
        };
        predicate
            .universally_quantified(&self.sources, &mut self.work)
            .is_ok_and(|p| p.is_unconstrained())
    }
    fn enter(&mut self, depth: usize) -> Result<(), ()> {
        if depth > 64 {
            return Err(());
        }
        self.work.charge(DecisionWork::Visit)
    }
    fn types(
        &mut self,
        expected: A::Type,
        actual: A::Type,
        depth: usize,
        expected_environment: &ContractEnvironment,
        actual_environment: &ContractEnvironment,
    ) -> Result<(), ()> {
        self.enter(depth)?;
        let expected_scope = self.program.scope(expected).ok_or(())?;
        let actual_scope = self.program.scope(actual).ok_or(())?;
        let expected_environment = expected_environment.at_scope(expected_scope)?;
        let actual_environment = actual_environment.at_scope(actual_scope)?;
        if expected == actual
            && (expected_scope.is_root() || expected_environment == actual_environment)
        {
            return Ok(());
        }
        match (
            self.program.bound_type(expected),
            self.program.bound_type(actual),
        ) {
            (Some(expected), Some(actual)) => {
                return if expected_environment.type_variable(expected)?
                    == actual_environment.type_variable(actual)?
                {
                    Ok(())
                } else {
                    Err(())
                };
            }
            (None, None) => {}
            _ => return Err(()),
        }
        match (self.program.array(expected), self.program.array(actual)) {
            (Some((expected_length, expected_item)), Some((actual_length, actual_item))) => {
                let same_length = match (expected_length, actual_length) {
                    (
                        RuntimeArrayLength::Constant(expected),
                        RuntimeArrayLength::Constant(actual),
                    ) => expected == actual,
                    (RuntimeArrayLength::Bound(expected), RuntimeArrayLength::Bound(actual)) => {
                        expected_environment.const_variable(expected)?
                            == actual_environment.const_variable(actual)?
                    }
                    _ => false,
                };
                if !same_length {
                    return Err(());
                }
                return self.types(
                    expected_item,
                    actual_item,
                    depth + 1,
                    &expected_environment,
                    &actual_environment,
                );
            }
            (None, None) => {}
            _ => return Err(()),
        }
        if let Some(arguments) = self.program.nominal_arguments(expected, actual) {
            let (expected, actual) = arguments?;
            if expected.len() != actual.len() {
                return Err(());
            }
            for (expected, actual) in expected.iter().zip(actual) {
                self.types(
                    *expected,
                    *actual,
                    depth + 1,
                    &expected_environment,
                    &actual_environment,
                )?;
                self.types(
                    *actual,
                    *expected,
                    depth + 1,
                    &actual_environment,
                    &expected_environment,
                )?;
            }
            return Ok(());
        }
        match (
            self.program.function(expected),
            self.program.function(actual),
        ) {
            (
                Some((expected_contract, expected_parameters, expected_result)),
                Some((actual_contract, actual_parameters, actual_result)),
            ) => {
                if expected_contract.binder() != actual_contract.binder()
                    || expected_parameters.len() != actual_parameters.len()
                {
                    return Err(());
                }
                let binder = self.next_binder;
                self.next_binder = binder.checked_add(1).ok_or(())?;
                let expected_environment = expected_environment
                    .enter(expected_contract.binder(), BinderOwner::Local(binder))?;
                let actual_environment = actual_environment
                    .enter(actual_contract.binder(), BinderOwner::Local(binder))?;
                for (expected, actual) in expected_parameters.iter().zip(actual_parameters) {
                    self.types(
                        *actual,
                        *expected,
                        depth + 1,
                        &actual_environment,
                        &expected_environment,
                    )?;
                }
                self.types(
                    expected_result,
                    actual_result,
                    depth + 1,
                    &expected_environment,
                    &actual_environment,
                )?;
                let expected = expected_contract
                    .invocation()
                    .map_references(&mut self.work, &mut |reference, _| {
                        expected_environment.variable(reference, self.bindable.as_ref())
                    })?
                    .substitute(&self.replacements, &mut self.work)?;
                let actual = actual_contract
                    .invocation()
                    .map_references(&mut self.work, &mut |reference, _| {
                        actual_environment.variable(reference, self.bindable.as_ref())
                    })?
                    .substitute(&self.replacements, &mut self.work)?;
                let relation = actual.subset(&expected, &mut self.work)?;
                let expected_predicate = expected_contract
                    .predicate()
                    .map_references(&mut self.work, &mut |reference, _| {
                        expected_environment.variable(reference, self.bindable.as_ref())
                    })?
                    .substitute(&self.replacements, &mut self.work)?;
                let actual_predicate = actual_contract
                    .predicate()
                    .map_references(&mut self.work, &mut |reference, _| {
                        actual_environment.variable(reference, self.bindable.as_ref())
                    })?
                    .substitute(&self.replacements, &mut self.work)?;
                let consequence = actual_predicate.and(&relation, &mut self.work)?;
                let relation = expected_predicate.implies(&consequence, &mut self.work)?;
                self.observe(&relation);
                self.predicate = self.predicate.and(&relation, &mut self.work)?;
                Ok(())
            }
            _ => {
                let program = self.program;
                if let Some(result) =
                    program.relate_children(expected, actual, &mut |expected, actual| {
                        self.types(
                            expected,
                            actual,
                            depth + 1,
                            &expected_environment,
                            &actual_environment,
                        )
                    })
                {
                    return result;
                }
                if (expected_scope.is_root() && actual_scope.is_root()
                    || expected_environment == actual_environment)
                    && self.program.compatible(expected, actual)
                {
                    Ok(())
                } else {
                    Err(())
                }
            }
        }
    }
    fn value(
        &mut self,
        expected: A::Type,
        value: &RuntimeValue,
        depth: usize,
        environment: &ContractEnvironment,
    ) -> Result<(), ()> {
        self.value_view(expected, value.view(), depth, environment)
    }
    fn value_view(
        &mut self,
        expected: A::Type,
        value: crate::value::RuntimeValueView<'_>,
        depth: usize,
        environment: &ContractEnvironment,
    ) -> Result<(), ()> {
        self.enter(depth)?;
        let scope = self.program.scope(expected).ok_or(())?;
        let environment = environment.at_scope(scope)?;
        let program = self.program;
        let mut relation = CallableValueRelation {
            matcher: self,
            environment,
            choices: Vec::new(),
        };
        if program.value_relation(expected, value, &mut relation) {
            Ok(())
        } else {
            Err(())
        }
    }
}
fn collect_parameter_effects(
    program: &AwbcProgram,
    ty: AwbcTypeId,
    environment: &ContractEnvironment,
    work: &mut ContractWork,
) -> Result<BTreeSet<u32>, ()> {
    let mut result = BTreeSet::new();
    let mut pending = vec![(ty, environment.clone(), 0usize)];
    while let Some((ty, environment, depth)) = pending.pop() {
        if depth > 64 {
            return Err(());
        }
        work.charge(DecisionWork::Visit)?;
        let row = program.runtime_types.get(ty.index()).ok_or(())?;
        let mut environment = environment.at_scope(row.scope())?;
        if let AwbcRuntimeTypeShape::Function { contract, .. } = row.shape() {
            environment = environment.enter(contract.binder(), BinderOwner::Local(0))?;
            for reference in contract
                .invocation()
                .variables()
                .chain(contract.predicate().variables())
            {
                if let ContractVariable::Parameter(slot) = environment.variable(reference, None)? {
                    result.insert(slot);
                }
            }
        }
        row.shape().visit_structural_type_refs(&mut |child| {
            pending.push((child, environment.clone(), depth + 1))
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
