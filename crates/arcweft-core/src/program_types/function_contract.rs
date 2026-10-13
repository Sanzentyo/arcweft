//! Scoped callable relations and immutable declaration effect solutions.

use crate::awbc::schema::{AwbcProgram, AwbcRuntimeTypeShape, AwbcTypeId};
use crate::{
    effect_row::{DecisionControl, DecisionWork, EffectPredicate},
    entry::schema::value_budget::ValidationWork,
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

impl super::RuntimeProgramValueType<'_> {
    pub(crate) fn validate_view(
        self,
        value: crate::value::RuntimeValueView<'_>,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        resources: &mut crate::entry::schema::value_encoding::ValueResourceAdmission,
    ) -> Result<(), super::RuntimeProgramTypeError> {
        resources.validate_view(value, 0, None)?;
        match self {
            Self::Plan { program, ty } => {
                Self::validate_authority_value(program, ty, value, binding, resources)
            }
            Self::Awbc { program, ty } => {
                Self::validate_authority_value(program, ty, value, binding, resources)
            }
        }
    }

    /// Selects a builtin case's original unary field row while checking the
    /// source and tuple scopes against the caller's immutable binding.
    pub(crate) fn builtin_case_payload(
        self,
        case: crate::pattern::RuntimeBuiltinVariantCaseIdentity,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        resources: &crate::entry::schema::value_encoding::ValueResourceAdmission,
    ) -> Result<Option<Self>, super::RuntimeProgramTypeError> {
        match self {
            Self::Plan { program, ty } => {
                Self::validate_builtin_payload(program, ty, case, binding, resources)
                    .map(|payload| payload.map(|ty| Self::Plan { program, ty }))
            }
            Self::Awbc { program, ty } => {
                Self::validate_builtin_payload(program, ty, case, binding, resources)
                    .map(|payload| payload.map(|ty| Self::Awbc { program, ty }))
            }
        }
    }

    fn validate_builtin_payload<A: FunctionTypeAuthority>(
        program: &A,
        expected: A::Type,
        case: crate::pattern::RuntimeBuiltinVariantCaseIdentity,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        resources: &crate::entry::schema::value_encoding::ValueResourceAdmission,
    ) -> Result<Option<A::Type>, super::RuntimeProgramTypeError> {
        let invalid = || A::invalid_builtin_payload(expected);
        RuntimeFunctionEffectInstantiation::with_value_relation(
            program,
            resources.origin(),
            binding,
            &resources.work(),
            |context| {
                let mut work = resources.work();
                work.charge(0)?;
                if program
                    .scope(expected)
                    .is_none_or(|scope| !context.permits_scope(scope))
                {
                    return Err(invalid());
                }
                let payload = program.builtin_case_payload(expected, case)?;
                if let Some((tuple, field)) = payload {
                    work.charge(0)?;
                    if program
                        .scope(tuple)
                        .is_none_or(|scope| !context.permits_scope(scope))
                    {
                        return Err(invalid());
                    }
                    Ok(Some(field))
                } else {
                    Ok(None)
                }
            },
        )?
        .ok_or_else(invalid)
    }

    fn validate_authority_value<A: FunctionTypeAuthority>(
        program: &A,
        expected: A::Type,
        value: crate::value::RuntimeValueView<'_>,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        resources: &crate::entry::schema::value_encoding::ValueResourceAdmission,
    ) -> Result<(), super::RuntimeProgramTypeError> {
        RuntimeFunctionEffectInstantiation::with_value_relation(
            program,
            resources.origin(),
            binding,
            &resources.work(),
            |context| program.value_relation(expected, value, context),
        )?
        .ok_or_else(|| {
            A::value_error(
                expected,
                crate::entry::RuntimeSchemaError::Type {
                    path: "$".to_owned(),
                    expected: "original value type binding",
                    actual: "invalid binding",
                },
            )
        })
    }
}

impl crate::value::RuntimePlaceStorage<RuntimeValue> {
    /// Checks initialized descendants and partial record headers against the
    /// same selected executable and immutable frame binding as complete values.
    pub(crate) fn matches_program_type(
        &self,
        expected: super::RuntimeProgramValueType<'_>,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
    ) -> bool {
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::live(
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        self.validate_program_type(expected, binding, &mut resources, RuntimeValue::view)
            .is_ok()
    }
}

impl<T> crate::value::RuntimePlaceStorage<T> {
    /// Admits complete and partial storage while it is still borrowed. The
    /// destination binding comes from the original frame or declaration, never
    /// from the candidate's retained nominal provenance.
    pub(crate) fn validate_program_type<'value>(
        &'value self,
        expected: super::RuntimeProgramValueType<'_>,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        resources: &mut crate::entry::schema::value_encoding::ValueResourceAdmission,
        view: impl Fn(&'value T) -> crate::value::RuntimeValueView<'value>,
    ) -> Result<(), super::RuntimeProgramTypeError> {
        self.preflight_resources(resources, &view, 0)?;
        match expected {
            super::RuntimeProgramValueType::Plan { program, ty } => {
                self.validate_authority_type(program, ty, binding, resources, &view)
            }
            super::RuntimeProgramValueType::Awbc { program, ty } => {
                self.validate_authority_type(program, ty, binding, resources, &view)
            }
        }
    }

    fn preflight_resources<'value>(
        &'value self,
        resources: &mut crate::entry::schema::value_encoding::ValueResourceAdmission,
        view: &impl Fn(&'value T) -> crate::value::RuntimeValueView<'value>,
        depth: usize,
    ) -> Result<(), crate::entry::RuntimeSchemaError> {
        let result = if let Some(value) = self.as_ref() {
            resources.validate_view(view(value), depth, None)
        } else if let Some((header, children)) = self.record_parts() {
            resources
                .partial_record(header, children.len(), depth)
                .and_then(|()| {
                    children
                        .iter()
                        .enumerate()
                        .try_for_each(|(ordinal, child)| {
                            child
                                .preflight_resources(resources, view, depth + 1)
                                .map_err(|error| error.within_value_path(&format!("$[{ordinal}]")))
                        })
                })
        } else {
            resources.vacant(depth)
        };
        if let Err(error) = &result {
            resources.work().retain_failure(error.clone());
        }
        result
    }

    fn validate_authority_type<'value, A: FunctionTypeAuthority>(
        &'value self,
        program: &A,
        expected: A::Type,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        resources: &crate::entry::schema::value_encoding::ValueResourceAdmission,
        view: &impl Fn(&'value T) -> crate::value::RuntimeValueView<'value>,
    ) -> Result<(), super::RuntimeProgramTypeError> {
        RuntimeFunctionEffectInstantiation::with_value_relation(
            program,
            resources.origin(),
            binding,
            &resources.work(),
            |context| self.validate_place_type(program, expected, context, view, 0),
        )?
        .ok_or_else(|| {
            A::value_error(
                expected,
                crate::entry::RuntimeSchemaError::Type {
                    path: "$".to_owned(),
                    expected: "original place type binding",
                    actual: "invalid binding",
                },
            )
        })
    }

    fn validate_place_type<'value, A: FunctionTypeAuthority>(
        &'value self,
        program: &A,
        expected: A::Type,
        context: &mut CallableValueRelation<'_, '_, A>,
        view: &impl Fn(&'value T) -> crate::value::RuntimeValueView<'value>,
        depth: usize,
    ) -> Result<(), super::RuntimeProgramTypeError> {
        context.validation_work().charge(depth)?;
        let mismatch = || {
            A::value_error(
                expected,
                crate::entry::RuntimeSchemaError::Type {
                    path: "$".to_owned(),
                    expected: "original typed place",
                    actual: "partial or vacant place",
                },
            )
        };
        if !program
            .scope(expected)
            .is_some_and(|scope| context.permits_scope(scope))
        {
            return Err(mismatch());
        }
        if let Some(value) = self.as_ref() {
            return program.value_relation(expected, view(value), context);
        }
        if self.is_vacant() {
            return Ok(());
        }
        let Some((header, children)) = self.record_parts() else {
            return Err(mismatch());
        };
        if let crate::value::RuntimePlaceRecordHeader::Nominal {
            semantic_identity,
            type_instantiation,
            ..
        } = header
        {
            let actual = program
                .by_semantic(*semantic_identity)
                .ok_or_else(mismatch)?;
            if !context.nominal(expected, actual, type_instantiation.as_deref()) {
                return Err(context
                    .validation_work()
                    .failure()
                    .map_or_else(mismatch, Into::into));
            }
        }
        if children.is_empty() {
            return Err(mismatch());
        }
        children
            .iter()
            .enumerate()
            .try_for_each(|(ordinal, child)| {
                let ty = program
                    .record_field(expected, header, children.len(), ordinal)
                    .ok_or_else(mismatch)?;
                child
                    .validate_place_type(program, ty, context, view, depth + 1)
                    .map_err(|error| error.within_value_path(&format!("$[{ordinal}]")))
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
    /// Charges the retained declaration solution before matching or copying its
    /// rows. Metadata contributes work and depth, not logical payload nodes.
    pub(crate) fn validate_resource_limits(
        &self,
        depth: usize,
        budget: &crate::entry::schema::value_budget::ValueBudget,
        work: &mut crate::entry::schema::value_budget::ValidationWork,
    ) -> Result<(), crate::entry::RuntimeSchemaError> {
        work.charge(depth)?;
        budget.collection(self.effects.len())?;
        for row in &self.effects {
            work.charge(depth + 1)?;
            budget.collection(row.len())?;
            for effect in row.iter() {
                work.charge(depth + 2)?;
                budget.string(effect.as_str())?;
            }
        }
        Ok(())
    }

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
    pub(crate) fn with_value_relation<A: FunctionTypeAuthority, R, E>(
        program: &A,
        origin: crate::entry::schema::value_encoding::ValueAdmissionOrigin,
        binding: Option<&Self>,
        work: &ValidationWork,
        operation: impl FnOnce(&mut CallableValueRelation<'_, '_, A>) -> Result<R, E>,
    ) -> Result<Option<R>, E>
    where
        E: From<crate::entry::RuntimeSchemaError>,
    {
        let matcher = match binding {
            Some(binding) => binding.matcher_with_work(program, work.clone()),
            None => Some(ParameterMatcher::root(program, work.clone())),
        };
        let Some(mut matcher) = matcher else {
            return work.failure().map_or(Ok(None), |error| Err(error.into()));
        };
        let environment = matcher.parameters.clone();
        let result = operation(&mut CallableValueRelation {
            matcher: &mut matcher,
            environment,
            choices: Vec::new(),
            origin,
        })?;
        let accepted = matcher.accepted();
        if let Some(error) = work.failure() {
            return Err(error.into());
        }
        Ok(accepted.then_some(result))
    }
    fn matcher<'program, A: FunctionTypeAuthority>(
        &self,
        program: &'program A,
    ) -> Option<ParameterMatcher<'program, A>> {
        self.matcher_with_work(
            program,
            ValidationWork::new(crate::entry::RuntimeSchemaLimits::engine_default()),
        )
    }
    fn matcher_with_work<'program, A: FunctionTypeAuthority>(
        &self,
        program: &'program A,
        mut work: ValidationWork,
    ) -> Option<ParameterMatcher<'program, A>> {
        // Bound retained metadata before copying effect rows into formulas or
        // resolving the selected program's declaration scope. Source matchers
        // use the same allowance as their destination, including this pass.
        if let Err(error) = self.validate_resource_limits(
            0,
            &crate::entry::schema::value_budget::ValueBudget::new(work.limits()),
            &mut work,
        ) {
            work.retain_failure(error);
            return None;
        }
        let context = program.by_semantic(self.context)?;
        let (_, mut matcher) =
            ParameterMatcher::for_function_with_work(program, context, None, work)?;
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
    pub(crate) fn is_valid_with_work<A: FunctionTypeAuthority>(
        &self,
        program: &A,
        work: &ValidationWork,
    ) -> bool {
        self.matcher_with_work(program, work.clone()).is_some()
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
impl DecisionControl for ValidationWork {
    type Error = ();
    fn charge(&mut self, _: DecisionWork) -> Result<(), ()> {
        self.charge(0).map_err(|_| ())
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
    work: ValidationWork,
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
    origin: crate::entry::schema::value_encoding::ValueAdmissionOrigin,
}
impl<A: FunctionTypeAuthority> RuntimeValueTypeContext<A::Type>
    for CallableValueRelation<'_, '_, A>
{
    fn validation_work(&self) -> ValidationWork {
        self.matcher.work.clone()
    }
    fn admission_origin(&self) -> crate::entry::schema::value_encoding::ValueAdmissionOrigin {
        self.origin
    }
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
    fn finish_choice(&mut self, admitted: bool) {
        let choice = self.choices.pop().expect("active value Choice");
        self.matcher.predicate = if admitted {
            choice.selected.unwrap_or(choice.before)
        } else {
            choice.before
        };
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
        let mut work = ValidationWork::new(crate::entry::RuntimeSchemaLimits::engine_default());
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
        let Some((_, mut matcher)) =
            ParameterMatcher::new_with_work(self, contract, Some(bindable), work)
        else {
            return false;
        };
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
    fn root(program: &'a A, work: ValidationWork) -> Self {
        Self {
            program,
            predicate: EffectPredicate::unconstrained(),
            source_predicate: EffectPredicate::unconstrained(),
            parameters: ContractEnvironment::root(),
            bindable: None,
            locals: BTreeSet::new(),
            sources: BTreeSet::new(),
            next_binder: 0,
            work,
            replacements: BTreeMap::new(),
        }
    }
    fn origin_environment(
        &mut self,
        origin: &RuntimeFunctionEffectInstantiation,
    ) -> Option<ContractEnvironment> {
        let source = origin.matcher_with_work(self.program, self.work.clone())?;
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
        Self::new_with_work(
            program,
            contract,
            bindable,
            ValidationWork::new(crate::entry::RuntimeSchemaLimits::engine_default()),
        )
    }
    fn new_with_work(
        program: &'a A,
        contract: A::Type,
        bindable: Option<BTreeSet<u32>>,
        work: ValidationWork,
    ) -> Option<(&'a [A::Type], Self)> {
        let (header, _, result) = program.function(contract)?;
        if header.binder().effects() == 0
            || header.invocation() != &crate::effect_row::EffectFormula::empty()
            || !program.is_unit(result)
        {
            return None;
        }
        Self::for_function_with_work(program, contract, bindable, work)
    }
    fn for_function(
        program: &'a A,
        contract: A::Type,
        bindable: Option<BTreeSet<u32>>,
    ) -> Option<(&'a [A::Type], Self)> {
        Self::for_function_with_work(
            program,
            contract,
            bindable,
            ValidationWork::new(crate::entry::RuntimeSchemaLimits::engine_default()),
        )
    }
    fn for_function_with_work(
        program: &'a A,
        contract: A::Type,
        bindable: Option<BTreeSet<u32>>,
        mut work: ValidationWork,
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
        self.work.charge(depth).map_err(|_| ())
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
            origin: crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live,
        };
        if program
            .value_relation(expected, value, &mut relation)
            .is_ok()
        {
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
    work: &mut ValidationWork,
) -> Result<BTreeSet<u32>, ()> {
    let mut result = BTreeSet::new();
    let mut pending = vec![(ty, environment.clone(), 0usize)];
    while let Some((ty, environment, depth)) = pending.pop() {
        if depth > 64 {
            return Err(());
        }
        work.charge(depth).map_err(|_| ())?;
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
