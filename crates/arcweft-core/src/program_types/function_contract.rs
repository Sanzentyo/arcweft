//! Joint effect binding and declaration-scoped default relations.

use crate::awbc::schema::{AwbcProgram, AwbcRuntimeTypeShape, AwbcTypeId};
use crate::{
    effect_row::{DecisionControl, DecisionWork, EffectPredicate},
    plan::{RuntimeBoundEffectReference, RuntimeTypeBinder, RuntimeTypeScope},
    value::RuntimeValue,
};
use std::collections::{BTreeMap, BTreeSet};
mod authority;
use crate::pattern::RuntimeSemanticTypeId;
use authority::FunctionTypeAuthority;

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
enum EffectOwner {
    Parameters,
    Sources,
    Local(u32),
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct EffectEnvironment {
    scope: RuntimeTypeScope,
    owners: Vec<EffectOwner>,
}
impl EffectEnvironment {
    fn root() -> Self {
        Self {
            scope: RuntimeTypeScope::root(),
            owners: Vec::new(),
        }
    }
    fn enter(&self, binder: RuntimeTypeBinder, owner: EffectOwner) -> Result<Self, ()> {
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
        let index = self
            .owners
            .len()
            .checked_sub(usize::try_from(reference.depth()).map_err(|_| ())? + 1)
            .ok_or(())?;
        Ok(match self.owners.get(index).ok_or(())? {
            EffectOwner::Parameters
                if bindable.is_none_or(|slots| slots.contains(&reference.slot())) =>
            {
                ContractVariable::Parameter(reference.slot())
            }
            EffectOwner::Parameters | EffectOwner::Sources => {
                ContractVariable::Source(reference.slot())
            }
            EffectOwner::Local(binder) => ContractVariable::Local {
                binder: *binder,
                slot: reference.slot(),
            },
        })
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
    parameters: EffectEnvironment,
    bindable: Option<BTreeSet<u32>>,
    locals: BTreeSet<ContractVariable>,
    sources: BTreeSet<ContractVariable>,
    next_binder: u32,
    work: ContractWork,
    replacements: BTreeMap<ContractVariable, crate::effect_row::EffectFormula<ContractVariable>>,
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
                        &EffectEnvironment::root(),
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
            EffectEnvironment::root().enter(header.binder(), EffectOwner::Parameters)
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
            None => EffectEnvironment::root(),
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
                    EffectEnvironment::root().enter(source.binder(), EffectOwner::Sources)
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
        let parameters_environment = EffectEnvironment::root()
            .enter(contract.binder(), EffectOwner::Parameters)
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
        expected_environment: &EffectEnvironment,
        actual_environment: &EffectEnvironment,
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
            self.program.function(expected),
            self.program.function(actual),
        ) {
            (
                Some((expected_contract, expected_parameters, expected_result)),
                Some((actual_contract, actual_parameters, actual_result)),
            ) => {
                if expected_contract.binder() != actual_contract.binder()
                    || expected_contract.binder().types() != 0
                    || expected_contract.binder().const_lengths() != 0
                    || expected_parameters.len() != actual_parameters.len()
                {
                    return Err(());
                }
                let binder = self.next_binder;
                self.next_binder = binder.checked_add(1).ok_or(())?;
                let expected_environment = expected_environment
                    .enter(expected_contract.binder(), EffectOwner::Local(binder))?;
                let actual_environment = actual_environment
                    .enter(actual_contract.binder(), EffectOwner::Local(binder))?;
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
                if let (Some(expected), Some(actual)) =
                    (self.program.tuple(expected), self.program.tuple(actual))
                {
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
                    }
                    return Ok(());
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
        environment: &EffectEnvironment,
    ) -> Result<(), ()> {
        self.value_view(expected, value.view(), depth, environment)
    }
    fn value_view(
        &mut self,
        expected: A::Type,
        value: crate::value::RuntimeValueView<'_>,
        depth: usize,
        environment: &EffectEnvironment,
    ) -> Result<(), ()> {
        self.enter(depth)?;
        let scope = self.program.scope(expected).ok_or(())?;
        let environment = environment.at_scope(scope)?;
        match (self.program.function(expected), value) {
            (
                Some(_),
                crate::value::RuntimeValueView::RuntimeOnly(RuntimeValue::Callable(value)),
            ) => {
                let actual = self.program.callable_type(value).ok_or(())?;
                self.types(
                    expected,
                    actual,
                    depth + 1,
                    &environment,
                    &EffectEnvironment::root(),
                )
            }
            (_, crate::value::RuntimeValueView::Tuple(values))
                if self.program.tuple(expected).is_some() =>
            {
                let types = self.program.tuple(expected).ok_or(())?;
                if types.len() != values.len() {
                    return Err(());
                }
                for (index, ty) in types.iter().enumerate() {
                    self.value_view(*ty, values.get(index).ok_or(())?, depth + 1, &environment)?;
                }
                Ok(())
            }
            _ if scope.is_root() && self.program.value_matches(expected, value, depth) => Ok(()),
            _ => Err(()),
        }
    }
}
fn collect_parameter_effects(
    program: &AwbcProgram,
    ty: AwbcTypeId,
    environment: &EffectEnvironment,
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
            environment = environment.enter(contract.binder(), EffectOwner::Local(0))?;
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
mod tests {
    use super::*;
    use crate::{
        awbc::schema::AwbcRuntimeType,
        effect_row::EffectFormula,
        pattern::RuntimeSemanticTypeId,
        plan::{RuntimeFunctionTypeContract, RuntimeTypeBinder, RuntimeTypeScope},
    };

    #[test]
    fn scoped_default_preserves_shared_source_rows_across_callback_variance() {
        let binder = RuntimeTypeBinder::new(0, 0, 2);
        let scope = RuntimeTypeScope::root().enter(binder).unwrap();
        let row = |marker, shape| {
            AwbcRuntimeType::new(RuntimeSemanticTypeId::from_bytes([marker; 32]), shape)
        };
        let function = |parameters, result, effects| AwbcRuntimeTypeShape::Function {
            contract: RuntimeFunctionTypeContract::new(
                RuntimeTypeBinder::EMPTY,
                EffectPredicate::unconstrained(),
                effects,
            ),
            parameters,
            result,
        };
        let header = |parameters, result| AwbcRuntimeTypeShape::Function {
            contract: RuntimeFunctionTypeContract::new(
                binder,
                EffectPredicate::unconstrained(),
                EffectFormula::empty(),
            ),
            parameters,
            result,
        };
        let mut program = AwbcProgram::default();
        program.runtime_types = vec![
            row(1, AwbcRuntimeTypeShape::Unit),
            row(
                2,
                function(
                    vec![],
                    AwbcTypeId(0),
                    EffectFormula::literal(
                        Default::default(),
                        Some(scope.bound_effect(0, 0).unwrap()),
                    ),
                ),
            )
            .with_scope(scope.clone()),
            row(
                3,
                function(
                    vec![],
                    AwbcTypeId(0),
                    EffectFormula::literal(
                        Default::default(),
                        Some(scope.bound_effect(0, 1).unwrap()),
                    ),
                ),
            )
            .with_scope(scope.clone()),
            row(
                4,
                function(vec![AwbcTypeId(1)], AwbcTypeId(0), EffectFormula::empty()),
            )
            .with_scope(scope.clone()),
            row(
                5,
                function(vec![AwbcTypeId(2)], AwbcTypeId(0), EffectFormula::empty()),
            )
            .with_scope(scope.clone()),
            row(
                6,
                AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(1), AwbcTypeId(3)]),
            )
            .with_scope(scope.clone()),
            row(
                7,
                AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(2), AwbcTypeId(4)]),
            )
            .with_scope(scope),
            row(8, header(vec![AwbcTypeId(1), AwbcTypeId(6)], AwbcTypeId(0))),
            row(9, header(vec![AwbcTypeId(1)], AwbcTypeId(5))),
            row(
                10,
                function(
                    vec![],
                    AwbcTypeId(0),
                    EffectFormula::literal(
                        crate::effect_row::EffectSet::from_labels(["io.read"]).unwrap(),
                        None,
                    ),
                ),
            ),
            row(
                11,
                header(vec![AwbcTypeId(1), AwbcTypeId(1)], AwbcTypeId(0)),
            ),
        ];
        // Both tuple members refer to the same source row. The destination
        // can choose that row after the source is fixed, in both variances.
        assert!(program.parameter_contract_accepts_default(
            AwbcTypeId(7),
            1,
            Some(AwbcTypeId(8)),
            AwbcTypeId(5)
        ));
        assert!(!program.parameter_contract_accepts_default(AwbcTypeId(7), 1, None, AwbcTypeId(5)));
        // A row already owned by an earlier input is rigid. An IO callback
        // cannot be a default for every possible earlier row, including empty.
        assert!(!program.parameter_contract_accepts_default(
            AwbcTypeId(10),
            1,
            None,
            AwbcTypeId(9)
        ));
    }

    #[test]
    fn shared_effect_bindings_reject_jointly_incompatible_callback_variance() {
        let binder = RuntimeTypeBinder::new(0, 0, 1);
        let scope = RuntimeTypeScope::root().enter(binder).unwrap();
        let reference = scope.bound_effect(0, 0).unwrap();
        let function = |parameters, effects| AwbcRuntimeTypeShape::Function {
            contract: RuntimeFunctionTypeContract::new(
                RuntimeTypeBinder::EMPTY,
                EffectPredicate::unconstrained(),
                effects,
            ),
            parameters,
            result: AwbcTypeId(0),
        };
        let row = |marker, shape| {
            AwbcRuntimeType::new(RuntimeSemanticTypeId::from_bytes([marker; 32]), shape)
        };
        let mut program = AwbcProgram::default();
        program.runtime_types = vec![
            row(1, AwbcRuntimeTypeShape::Unit),
            row(
                2,
                function(
                    vec![],
                    EffectFormula::literal(Default::default(), Some(reference)),
                ),
            )
            .with_scope(scope.clone()),
            row(3, function(vec![], EffectFormula::empty())),
            row(4, function(vec![AwbcTypeId(1)], EffectFormula::empty())).with_scope(scope),
            row(5, function(vec![AwbcTypeId(2)], EffectFormula::empty())),
            row(
                6,
                function(
                    vec![],
                    EffectFormula::literal(
                        crate::effect_row::EffectSet::from_labels(["io.read"]).unwrap(),
                        None,
                    ),
                ),
            ),
            row(
                7,
                AwbcRuntimeTypeShape::Function {
                    contract: RuntimeFunctionTypeContract::new(
                        binder,
                        EffectPredicate::unconstrained(),
                        EffectFormula::empty(),
                    ),
                    parameters: vec![AwbcTypeId(3), AwbcTypeId(1)],
                    result: AwbcTypeId(0),
                },
            ),
        ];
        assert!(program.parameter_contract_accepts_types(AwbcTypeId(6), [(0, AwbcTypeId(4))]));
        assert!(program.parameter_contract_accepts_types(AwbcTypeId(6), [(1, AwbcTypeId(5))]));
        assert!(!program.parameter_contract_accepts_types(
            AwbcTypeId(6),
            [(0, AwbcTypeId(4)), (1, AwbcTypeId(5))]
        ));
        assert!(program.parameter_contract_accepts_types(
            AwbcTypeId(6),
            [(0, AwbcTypeId(4)), (1, AwbcTypeId(2))]
        ));
        assert!(!program.parameter_contract_accepts_types(
            AwbcTypeId(6),
            [(0, AwbcTypeId(4)), (0, AwbcTypeId(4))]
        ));
        // An input is an instantiated value, never a declaration-scoped type.
        assert!(!program.parameter_contract_accepts_types(AwbcTypeId(6), [(1, AwbcTypeId(1))]));
        assert!(!program.parameter_contract_accepts_types(AwbcTypeId(2), []));

        let fixed = |effects| RuntimeFunctionEffectInstantiation {
            context: program.runtime_types[6].semantic_identity(),
            effects: vec![effects].into_boxed_slice(),
        };
        let relates = |binding: &RuntimeFunctionEffectInstantiation, expected, actual| {
            let mut matcher = binding.matcher(&program).unwrap();
            let environment = matcher.parameters.clone();
            matcher
                .types(
                    expected,
                    actual,
                    0,
                    &environment,
                    &EffectEnvironment::root(),
                )
                .is_ok()
                && matcher.accepted()
        };
        let empty = fixed(Default::default());
        let io = fixed(crate::effect_row::EffectSet::from_labels(["io.read"]).unwrap());
        assert!(relates(&empty, AwbcTypeId(1), AwbcTypeId(2)));
        assert!(!relates(&empty, AwbcTypeId(1), AwbcTypeId(5)));
        assert!(relates(&io, AwbcTypeId(1), AwbcTypeId(5)));
        assert!(relates(&empty, AwbcTypeId(3), AwbcTypeId(4)));
        assert!(!relates(&io, AwbcTypeId(3), AwbcTypeId(4)));
        let encoded = serde_json::to_value(&io).unwrap();
        assert_eq!(
            serde_json::from_value::<RuntimeFunctionEffectInstantiation>(encoded.clone()).unwrap(),
            io
        );
        let mut duplicate = encoded;
        duplicate["effects"][0] = serde_json::json!(["io.read", "io.read"]);
        assert!(serde_json::from_value::<RuntimeFunctionEffectInstantiation>(duplicate).is_err());
        assert!(
            !RuntimeFunctionEffectInstantiation {
                context: program.runtime_types[6].semantic_identity(),
                effects: Box::new([]),
            }
            .is_valid(&program)
        );
    }
}
