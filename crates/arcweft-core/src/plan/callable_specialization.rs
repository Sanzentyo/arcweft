//! Checked type substitution and state transitions for reusable callable values.
//!
//! A specialization owns type evidence and references the existing state graph.
//! It contains no values, executable bodies, or second retained-layout model.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

use crate::effect_row::{DecisionControl, DecisionWork, EffectFormula, EffectPredicate, EffectSet};
use crate::runtime_id::RuntimeCallableStateId;

use super::{
    RuntimeArrayLength, RuntimeBoundConstReference, RuntimeBoundEffectReference,
    RuntimeBoundTypeReference, RuntimeCallableAttachedContract, RuntimeCallableDefault,
    RuntimeCallableStateDefinition, RuntimeCallableTransition, RuntimeFunctionTypeContract,
    RuntimePlanTypeProjection, RuntimeTypeScope,
};

/// Simultaneous arguments in the source function binder's namespace order.
/// References in the right-hand sides belong to the target function's scope.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeFunctionSpecializationArguments<T> {
    pub types: Box<[T]>,
    pub const_lengths: Box<[RuntimeArrayLength]>,
    pub effects: Box<[EffectFormula<RuntimeBoundEffectReference>]>,
}

impl<T> RuntimeFunctionSpecializationArguments<T> {
    pub fn try_map<U, E>(
        self,
        mut map: impl FnMut(T) -> Result<U, E>,
    ) -> Result<RuntimeFunctionSpecializationArguments<U>, E> {
        Ok(RuntimeFunctionSpecializationArguments {
            types: self
                .types
                .into_vec()
                .into_iter()
                .map(&mut map)
                .collect::<Result<_, _>>()?,
            const_lengths: self.const_lengths,
            effects: self.effects,
        })
    }
}

/// One exact state selection. Specialization preserves retained value order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCallableSpecializationState<S> {
    pub source: S,
    pub target: S,
}

/// One admitted specialization of a function scheme in the program.
///
/// State rows retain the code provenance of the actual input value. Values of
/// the same function type may have different origins; no type-only code lookup
/// can select a target absent from this checked relation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCallableSpecializationDefinition<T, S> {
    pub source_type: T,
    pub target_type: T,
    pub arguments: RuntimeFunctionSpecializationArguments<T>,
    pub states: Box<[RuntimeCallableSpecializationState<S>]>,
}

impl<T, S> RuntimeCallableSpecializationDefinition<T, S> {
    pub fn try_map<U, R, E>(
        self,
        mut ty: impl FnMut(T) -> Result<U, E>,
        mut state: impl FnMut(S) -> Result<R, E>,
    ) -> Result<RuntimeCallableSpecializationDefinition<U, R>, E> {
        Ok(RuntimeCallableSpecializationDefinition {
            source_type: ty(self.source_type)?,
            target_type: ty(self.target_type)?,
            arguments: self.arguments.try_map(ty)?,
            states: self
                .states
                .into_vec()
                .into_iter()
                .map(|row| {
                    Ok(RuntimeCallableSpecializationState {
                        source: state(row.source)?,
                        target: state(row.target)?,
                    })
                })
                .collect::<Result<_, E>>()?,
        })
    }
}

/// The two executable formats expose the same typed declarations to the
/// specialization checker. Neither format can assert its own relation valid.
pub trait RuntimeCallableSpecializationContext {
    type Type: Copy + Eq + Ord;
    type Function: Clone + Eq;

    fn type_scope(&self, ty: Self::Type) -> Option<&RuntimeTypeScope>;
    fn type_projection(&self, ty: Self::Type) -> Option<RuntimePlanTypeProjection<Self::Type>>;
    fn callable_state(
        &self,
        id: RuntimeCallableStateId,
    ) -> Option<&RuntimeCallableStateDefinition<Self::Type, Self::Function>>;
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeCallableSpecializationError {
    #[error("callable specialization references an unknown type or state")]
    UnknownReference,
    #[error("callable specialization has an invalid root or argument binder")]
    InvalidScope,
    #[error("callable specialization argument arity disagrees with its source binder")]
    ArgumentArity,
    #[error("callable specialization type substitution disagrees with its target")]
    TypeMismatch,
    #[error("callable specialization state correspondence is incomplete or inconsistent")]
    StateMismatch,
    #[error("callable specialization exceeds its validation work limit")]
    WorkLimit,
}

struct SpecializationWork {
    remaining: u64,
}

impl SpecializationWork {
    fn visit(&mut self) -> Result<(), RuntimeCallableSpecializationError> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(RuntimeCallableSpecializationError::WorkLimit)?;
        Ok(())
    }
}

impl DecisionControl for SpecializationWork {
    type Error = RuntimeCallableSpecializationError;

    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        self.visit()
    }
}

#[derive(Clone, Copy)]
enum TypeSide {
    Source {
        nested: u32,
    },
    /// A right-hand side belongs to the target binder. `local` counts binders
    /// introduced inside that side; only its free references need lifting.
    Argument {
        lift: u32,
        local: u32,
    },
}

struct TypeChecker<'a, C: RuntimeCallableSpecializationContext> {
    context: &'a C,
    arguments: &'a RuntimeFunctionSpecializationArguments<C::Type>,
    target_binder: super::RuntimeTypeBinder,
    work: SpecializationWork,
}

impl<C: RuntimeCallableSpecializationContext> TypeChecker<'_, C> {
    fn projection(
        &mut self,
        ty: C::Type,
    ) -> Result<RuntimePlanTypeProjection<C::Type>, RuntimeCallableSpecializationError> {
        self.work.visit()?;
        self.context
            .type_projection(ty)
            .ok_or(RuntimeCallableSpecializationError::UnknownReference)
    }

    fn shifted_type(
        reference: RuntimeBoundTypeReference,
        side: TypeSide,
    ) -> Result<RuntimeBoundTypeReference, RuntimeCallableSpecializationError> {
        let TypeSide::Argument { lift, local } = side else {
            return Ok(reference);
        };
        let depth = if reference.depth() >= local {
            reference.depth().checked_add(lift)
        } else {
            Some(reference.depth())
        }
        .ok_or(RuntimeCallableSpecializationError::InvalidScope)?;
        Ok(RuntimeBoundTypeReference::from_coordinates(
            depth,
            reference.slot(),
        ))
    }

    fn length(
        &mut self,
        value: RuntimeArrayLength,
        side: TypeSide,
    ) -> Result<RuntimeArrayLength, RuntimeCallableSpecializationError> {
        let RuntimeArrayLength::Bound(reference) = value else {
            return Ok(value);
        };
        match side {
            TypeSide::Source { nested } if reference.depth() == nested => {
                let argument = *self
                    .arguments
                    .const_lengths
                    .get(usize::from(reference.slot()))
                    .ok_or(RuntimeCallableSpecializationError::ArgumentArity)?;
                self.length(
                    argument,
                    TypeSide::Argument {
                        lift: nested,
                        local: 0,
                    },
                )
            }
            TypeSide::Source { nested } if reference.depth() < nested => Ok(value),
            TypeSide::Source { .. } => Err(RuntimeCallableSpecializationError::InvalidScope),
            TypeSide::Argument { lift, local } => {
                let depth = if reference.depth() >= local {
                    reference.depth().checked_add(lift)
                } else {
                    Some(reference.depth())
                }
                .ok_or(RuntimeCallableSpecializationError::InvalidScope)?;
                Ok(RuntimeArrayLength::Bound(
                    RuntimeBoundConstReference::from_coordinates(depth, reference.slot()),
                ))
            }
        }
    }

    fn shifted_effect(
        &mut self,
        reference: RuntimeBoundEffectReference,
        side: TypeSide,
    ) -> Result<EffectFormula<RuntimeBoundEffectReference>, RuntimeCallableSpecializationError>
    {
        match side {
            TypeSide::Source { nested } if reference.depth() == nested => {
                let argument = self
                    .arguments
                    .effects
                    .get(reference.slot() as usize)
                    .ok_or(RuntimeCallableSpecializationError::ArgumentArity)?
                    .clone();
                self.effect_formula(
                    &argument,
                    TypeSide::Argument {
                        lift: nested,
                        local: 0,
                    },
                )
            }
            TypeSide::Source { nested } if reference.depth() < nested => {
                Ok(EffectFormula::literal(EffectSet::new(), Some(reference)))
            }
            TypeSide::Source { .. } => Err(RuntimeCallableSpecializationError::InvalidScope),
            TypeSide::Argument { lift, local } => {
                let depth = if reference.depth() >= local {
                    reference.depth().checked_add(lift)
                } else {
                    Some(reference.depth())
                }
                .ok_or(RuntimeCallableSpecializationError::InvalidScope)?;
                Ok(EffectFormula::literal(
                    EffectSet::new(),
                    Some(RuntimeBoundEffectReference::from_coordinates(
                        depth,
                        reference.slot(),
                    )),
                ))
            }
        }
    }

    fn effect_replacements(
        &mut self,
        variables: impl Iterator<Item = RuntimeBoundEffectReference>,
        side: TypeSide,
    ) -> Result<
        BTreeMap<RuntimeBoundEffectReference, EffectFormula<RuntimeBoundEffectReference>>,
        RuntimeCallableSpecializationError,
    > {
        let mut replacements = BTreeMap::new();
        for reference in variables {
            self.work.visit()?;
            if let std::collections::btree_map::Entry::Vacant(entry) = replacements.entry(reference)
            {
                let replacement = self.shifted_effect(reference, side)?;
                entry.insert(replacement);
            }
        }
        Ok(replacements)
    }

    fn effect_formula(
        &mut self,
        formula: &EffectFormula<RuntimeBoundEffectReference>,
        side: TypeSide,
    ) -> Result<EffectFormula<RuntimeBoundEffectReference>, RuntimeCallableSpecializationError>
    {
        let replacements = self.effect_replacements(formula.variables().copied(), side)?;
        formula.substitute(&replacements, &mut self.work)
    }

    fn effect_predicate(
        &mut self,
        predicate: &EffectPredicate<RuntimeBoundEffectReference>,
        side: TypeSide,
    ) -> Result<EffectPredicate<RuntimeBoundEffectReference>, RuntimeCallableSpecializationError>
    {
        let replacements = self.effect_replacements(predicate.variables().copied(), side)?;
        predicate.substitute(&replacements, &mut self.work)
    }

    fn contract(
        &mut self,
        contract: &RuntimeFunctionTypeContract,
        side: TypeSide,
        root: bool,
    ) -> Result<RuntimeFunctionTypeContract, RuntimeCallableSpecializationError> {
        let binder = if root && matches!(side, TypeSide::Source { .. }) {
            self.target_binder
        } else {
            contract.binder()
        };
        let child_side = match side {
            TypeSide::Source { nested } if root => TypeSide::Source { nested },
            TypeSide::Source { nested } => TypeSide::Source {
                nested: nested + u32::from(!contract.binder().is_empty()),
            },
            TypeSide::Argument { lift, local } => TypeSide::Argument {
                lift,
                local: local + u32::from(!contract.binder().is_empty()),
            },
        };
        Ok(RuntimeFunctionTypeContract::new(
            binder,
            self.effect_predicate(contract.predicate(), child_side)?,
            self.effect_formula(contract.invocation(), child_side)?,
        ))
    }

    fn compare(
        &mut self,
        source: C::Type,
        target: C::Type,
        side: TypeSide,
        root: bool,
    ) -> Result<(), RuntimeCallableSpecializationError> {
        let left = self.projection(source)?;
        let right = self.projection(target)?;
        if let RuntimePlanTypeProjection::BoundType(reference) = left {
            match side {
                TypeSide::Source { nested } if reference.depth() == nested => {
                    let argument = *self
                        .arguments
                        .types
                        .get(usize::from(reference.slot()))
                        .ok_or(RuntimeCallableSpecializationError::ArgumentArity)?;
                    return self.compare(
                        argument,
                        target,
                        TypeSide::Argument {
                            lift: nested,
                            local: 0,
                        },
                        false,
                    );
                }
                TypeSide::Source { nested } if reference.depth() < nested => {}
                TypeSide::Source { .. } => {
                    return Err(RuntimeCallableSpecializationError::InvalidScope);
                }
                TypeSide::Argument { .. } => {}
            }
            let shifted = Self::shifted_type(reference, side)?;
            return (right == RuntimePlanTypeProjection::BoundType(shifted))
                .then_some(())
                .ok_or(RuntimeCallableSpecializationError::TypeMismatch);
        }
        let left_children = left.children();
        let right_children = right.children();
        let mut left_shape = left
            .clone()
            .try_map(|_| Ok::<(), RuntimeCallableSpecializationError>(()))?;
        let right_shape = right
            .clone()
            .try_map(|_| Ok::<(), RuntimeCallableSpecializationError>(()))?;
        match &mut left_shape {
            RuntimePlanTypeProjection::Array { length, .. } => {
                *length = self.length(*length, side)?;
            }
            RuntimePlanTypeProjection::Function { contract, .. } => {
                *contract = self.contract(contract, side, root)?;
            }
            _ => {}
        }
        if left_shape != right_shape || left_children.len() != right_children.len() {
            return Err(RuntimeCallableSpecializationError::TypeMismatch);
        }
        let child_side = match (side, &left) {
            (TypeSide::Source { nested }, RuntimePlanTypeProjection::Function { contract, .. })
                if !root && !contract.binder().is_empty() =>
            {
                TypeSide::Source { nested: nested + 1 }
            }
            (
                TypeSide::Argument { lift, local },
                RuntimePlanTypeProjection::Function { contract, .. },
            ) if !contract.binder().is_empty() => TypeSide::Argument {
                lift,
                local: local + 1,
            },
            _ => side,
        };
        for (left_child, right_child) in left_children.iter().zip(right_children.iter()) {
            self.compare(**left_child, **right_child, child_side, false)?;
        }
        Ok(())
    }
}

impl<T: Copy + Eq + Ord> RuntimeCallableSpecializationDefinition<T, RuntimeCallableStateId> {
    pub fn validate<C: RuntimeCallableSpecializationContext<Type = T>>(
        &self,
        context: &C,
        maximum_work: u64,
    ) -> Result<(), RuntimeCallableSpecializationError> {
        self.validate_counted(context, maximum_work).map(|_| ())
    }

    /// Returns work charged by this exact relation so a containing verifier
    /// can enforce a cumulative budget without partitioning it arbitrarily.
    pub fn validate_counted<C: RuntimeCallableSpecializationContext<Type = T>>(
        &self,
        context: &C,
        maximum_work: u64,
    ) -> Result<u64, RuntimeCallableSpecializationError> {
        let (Some(source_scope), Some(target_scope)) = (
            context.type_scope(self.source_type),
            context.type_scope(self.target_type),
        ) else {
            return Err(RuntimeCallableSpecializationError::UnknownReference);
        };
        if !source_scope.is_root() || !target_scope.is_root() {
            return Err(RuntimeCallableSpecializationError::InvalidScope);
        }
        let (
            Some(RuntimePlanTypeProjection::Function {
                contract: source, ..
            }),
            Some(RuntimePlanTypeProjection::Function {
                contract: target, ..
            }),
        ) = (
            context.type_projection(self.source_type),
            context.type_projection(self.target_type),
        )
        else {
            return Err(RuntimeCallableSpecializationError::TypeMismatch);
        };
        let binder = source.binder();
        if self.arguments.types.len() != usize::from(binder.types())
            || self.arguments.const_lengths.len() != usize::from(binder.const_lengths())
            || self.arguments.effects.len() != binder.effects() as usize
        {
            return Err(RuntimeCallableSpecializationError::ArgumentArity);
        }
        let target_scope = RuntimeTypeScope::root()
            .enter(target.binder())
            .map_err(|_| RuntimeCallableSpecializationError::InvalidScope)?;
        for ty in &self.arguments.types {
            let scope = context
                .type_scope(*ty)
                .ok_or(RuntimeCallableSpecializationError::UnknownReference)?;
            if !scope.is_root() && scope != &target_scope {
                return Err(RuntimeCallableSpecializationError::InvalidScope);
            }
        }
        for length in &self.arguments.const_lengths {
            target_scope
                .validate_length(*length)
                .map_err(|_| RuntimeCallableSpecializationError::InvalidScope)?;
        }
        for formula in &self.arguments.effects {
            for reference in formula.variables() {
                target_scope
                    .validate_effect(*reference)
                    .map_err(|_| RuntimeCallableSpecializationError::InvalidScope)?;
            }
        }
        let mut checker = TypeChecker {
            context,
            arguments: &self.arguments,
            target_binder: target.binder(),
            work: SpecializationWork {
                remaining: maximum_work,
            },
        };
        checker.compare(
            self.source_type,
            self.target_type,
            TypeSide::Source { nested: 0 },
            true,
        )?;
        let mut state_map = BTreeMap::new();
        let mut target_states = BTreeSet::new();
        for row in &self.states {
            checker.work.visit()?;
            if state_map.insert(row.source, row.target).is_some()
                || !target_states.insert(row.target)
            {
                return Err(RuntimeCallableSpecializationError::StateMismatch);
            }
        }
        if state_map.is_empty() {
            return Err(RuntimeCallableSpecializationError::StateMismatch);
        }
        let mut anchored = false;
        for row in &self.states {
            let source = context
                .callable_state(row.source)
                .ok_or(RuntimeCallableSpecializationError::UnknownReference)?;
            let target = context
                .callable_state(row.target)
                .ok_or(RuntimeCallableSpecializationError::UnknownReference)?;
            if source.position != target.position
                || source.retained.len() != target.retained.len()
                || source.parameters.len() != target.parameters.len()
                || source.origin != target.origin
            {
                return Err(RuntimeCallableSpecializationError::StateMismatch);
            }
            anchored |= source.function_type == self.source_type
                && target.function_type == self.target_type;
            checker.compare(
                source.function_type,
                target.function_type,
                TypeSide::Source { nested: 0 },
                true,
            )?;
            checker.compare(
                source.result,
                target.result,
                TypeSide::Source { nested: 0 },
                false,
            )?;
            for (left, right) in source.retained.iter().zip(target.retained.iter()) {
                if left.role != right.role {
                    return Err(RuntimeCallableSpecializationError::StateMismatch);
                }
                checker.compare(left.ty, right.ty, TypeSide::Source { nested: 0 }, false)?;
            }
            for (left, right) in source.parameters.iter().zip(target.parameters.iter()) {
                if left.coordinate != right.coordinate || left.kind != right.kind {
                    return Err(RuntimeCallableSpecializationError::StateMismatch);
                }
                checker.compare(
                    left.abi_ty,
                    right.abi_ty,
                    TypeSide::Source { nested: 0 },
                    false,
                )?;
                checker.compare(
                    left.binding_ty,
                    right.binding_ty,
                    TypeSide::Source { nested: 0 },
                    false,
                )?;
            }
            match (&source.attached, &target.attached) {
                (RuntimeCallableAttachedContract::None, RuntimeCallableAttachedContract::None) => {}
                (
                    RuntimeCallableAttachedContract::Required { ty: left },
                    RuntimeCallableAttachedContract::Required { ty: right },
                ) => {
                    checker.compare(*left, *right, TypeSide::Source { nested: 0 }, false)?;
                }
                (
                    RuntimeCallableAttachedContract::Defaulted {
                        ty: left,
                        default: left_default,
                    },
                    RuntimeCallableAttachedContract::Defaulted {
                        ty: right,
                        default: right_default,
                    },
                ) => {
                    checker.compare(*left, *right, TypeSide::Source { nested: 0 }, false)?;
                    match (left_default, right_default) {
                        (
                            RuntimeCallableDefault::RequiresSpecialization,
                            RuntimeCallableDefault::RequiresSpecialization
                            | RuntimeCallableDefault::Body { .. },
                        ) => {}
                        (
                            RuntimeCallableDefault::Body { .. },
                            RuntimeCallableDefault::Body { .. },
                        ) if left_default == right_default => {}
                        _ => return Err(RuntimeCallableSpecializationError::StateMismatch),
                    }
                }
                (
                    RuntimeCallableAttachedContract::Optional {
                        value: lv,
                        binding: lb,
                    },
                    RuntimeCallableAttachedContract::Optional {
                        value: rv,
                        binding: rb,
                    },
                ) => {
                    checker.compare(*lv, *rv, TypeSide::Source { nested: 0 }, false)?;
                    checker.compare(*lb, *rb, TypeSide::Source { nested: 0 }, false)?;
                }
                _ => return Err(RuntimeCallableSpecializationError::StateMismatch),
            }
            match (&source.transition, &target.transition) {
                (
                    RuntimeCallableTransition::RequiresSpecialization,
                    RuntimeCallableTransition::RequiresSpecialization
                    | RuntimeCallableTransition::Retain { .. }
                    | RuntimeCallableTransition::Invoke { .. },
                ) if source.partials.is_empty() => {}
                (
                    RuntimeCallableTransition::Retain {
                        state: source_next,
                        values: source_values,
                    },
                    RuntimeCallableTransition::Retain {
                        state: target_next,
                        values: target_values,
                    },
                ) if state_map.get(source_next) == Some(target_next)
                    && source_values == target_values
                    && source.partials.len() == target.partials.len()
                    && source.partials.iter().zip(target.partials.iter()).all(
                        |(left, right)| {
                            left.parameters == right.parameters
                                && left.values == right.values
                                && state_map.get(&left.state) == Some(&right.state)
                        },
                    ) => {}
                _ => return Err(RuntimeCallableSpecializationError::StateMismatch),
            }
        }
        if !anchored {
            return Err(RuntimeCallableSpecializationError::StateMismatch);
        }
        Ok(maximum_work - checker.work.remaining)
    }
}

impl RuntimeCallableSpecializationContext for super::RuntimePlan {
    type Type = crate::runtime_id::RuntimePlanTypeId;
    type Function = crate::runtime_id::RuntimeFunctionSiteId;

    fn type_scope(&self, ty: Self::Type) -> Option<&RuntimeTypeScope> {
        self.type_table()
            .get(ty)
            .map(super::RuntimePlanTypeDeclaration::scope)
    }

    fn type_projection(&self, ty: Self::Type) -> Option<RuntimePlanTypeProjection<Self::Type>> {
        self.type_table()
            .get(ty)
            .map(super::RuntimePlanTypeDeclaration::projection)
            .cloned()
    }

    fn callable_state(
        &self,
        id: RuntimeCallableStateId,
    ) -> Option<&RuntimeCallableStateDefinition<Self::Type, Self::Function>> {
        self.callable_states().get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{
        RuntimeCallableParameterCoordinate, RuntimeCallableParameterInput,
        RuntimeCallableParameterKind, RuntimeCallablePosition, RuntimeTypeBinder,
    };

    #[derive(Default)]
    struct Context {
        types: BTreeMap<u32, (RuntimeTypeScope, RuntimePlanTypeProjection<u32>)>,
        states: BTreeMap<RuntimeCallableStateId, RuntimeCallableStateDefinition<u32, u32>>,
    }

    impl RuntimeCallableSpecializationContext for Context {
        type Type = u32;
        type Function = u32;

        fn type_scope(&self, ty: u32) -> Option<&RuntimeTypeScope> {
            self.types.get(&ty).map(|(scope, _)| scope)
        }

        fn type_projection(&self, ty: u32) -> Option<RuntimePlanTypeProjection<u32>> {
            self.types
                .get(&ty)
                .map(|(_, projection)| projection.clone())
        }

        fn callable_state(
            &self,
            id: RuntimeCallableStateId,
        ) -> Option<&RuntimeCallableStateDefinition<u32, u32>> {
            self.states.get(&id)
        }
    }

    fn state(
        function_type: u32,
        origin: RuntimeCallableStateId,
        parameter: u32,
        transition: RuntimeCallableTransition<u32>,
    ) -> RuntimeCallableStateDefinition<u32, u32> {
        RuntimeCallableStateDefinition {
            function_type,
            origin,
            position: RuntimeCallablePosition::Unapplied,
            retained: Box::new([]),
            parameters: Box::new([RuntimeCallableParameterInput {
                coordinate: RuntimeCallableParameterCoordinate {
                    group: 0,
                    parameter: 0,
                },
                kind: RuntimeCallableParameterKind::Fixed,
                abi_ty: parameter,
                binding_ty: parameter,
            }]),
            result: parameter,
            attached: RuntimeCallableAttachedContract::None,
            transition,
            partials: Box::new([]),
        }
    }

    #[test]
    fn specialization_lifts_target_binder_under_nested_function_without_capturing_it() {
        let outer = RuntimeTypeBinder::new(1, 0, 0);
        let scope = RuntimeTypeScope::root().enter(outer).unwrap();
        let nested = scope.enter(outer).unwrap();
        let t = RuntimeBoundTypeReference::from_coordinates(1, 0);
        let u = RuntimeBoundTypeReference::from_coordinates(0, 0);
        let source_type = 0;
        let target_type = 1;
        let source_parameter = 2;
        let target_parameter = 3;
        let source_t = 4;
        let target_v = 5;
        let source_u = 6;
        let target_u = 7;
        let target_argument = 8;
        let contract = RuntimeFunctionTypeContract::new(
            outer,
            EffectPredicate::unconstrained(),
            EffectFormula::empty(),
        );
        let mut context = Context::default();
        let mut insert = |id, ty_scope, projection| {
            context.types.insert(id, (ty_scope, projection));
        };
        insert(
            source_type,
            RuntimeTypeScope::root(),
            RuntimePlanTypeProjection::Function {
                contract: contract.clone(),
                parameters: Box::new([source_parameter]),
                result: source_parameter,
            },
        );
        insert(
            target_type,
            RuntimeTypeScope::root(),
            RuntimePlanTypeProjection::Function {
                contract: contract.clone(),
                parameters: Box::new([target_parameter]),
                result: target_parameter,
            },
        );
        insert(
            source_parameter,
            scope.clone(),
            RuntimePlanTypeProjection::Function {
                contract: contract.clone(),
                parameters: Box::new([source_t, source_u]),
                result: source_t,
            },
        );
        insert(
            target_parameter,
            scope.clone(),
            RuntimePlanTypeProjection::Function {
                contract,
                parameters: Box::new([target_v, target_u]),
                result: target_v,
            },
        );
        insert(
            source_t,
            nested.clone(),
            RuntimePlanTypeProjection::BoundType(t),
        );
        insert(
            target_v,
            nested.clone(),
            RuntimePlanTypeProjection::BoundType(t),
        );
        insert(
            source_u,
            nested.clone(),
            RuntimePlanTypeProjection::BoundType(u),
        );
        insert(target_u, nested, RuntimePlanTypeProjection::BoundType(u));
        insert(
            target_argument,
            scope,
            RuntimePlanTypeProjection::BoundType(RuntimeBoundTypeReference::from_coordinates(0, 0)),
        );
        let source_state = RuntimeCallableStateId::for_index(0).unwrap();
        let target_state = RuntimeCallableStateId::for_index(1).unwrap();
        context.states.insert(
            source_state,
            state(
                source_type,
                source_state,
                source_parameter,
                RuntimeCallableTransition::RequiresSpecialization,
            ),
        );
        context.states.insert(
            target_state,
            state(
                target_type,
                source_state,
                target_parameter,
                RuntimeCallableTransition::RequiresSpecialization,
            ),
        );
        let definition = RuntimeCallableSpecializationDefinition {
            source_type,
            target_type,
            arguments: RuntimeFunctionSpecializationArguments {
                types: Box::new([target_argument]),
                const_lengths: Box::new([]),
                effects: Box::new([]),
            },
            states: Box::new([RuntimeCallableSpecializationState {
                source: source_state,
                target: target_state,
            }]),
        };
        assert_eq!(definition.validate(&context, 10_000), Ok(()));
        assert!(definition.validate_counted(&context, 10_000).unwrap() > 0);
        assert_eq!(
            definition.validate(&context, 0),
            Err(RuntimeCallableSpecializationError::WorkLimit)
        );
        context.types.get_mut(&target_v).unwrap().1 = RuntimePlanTypeProjection::BoundType(u);
        assert_eq!(
            definition.validate(&context, 10_000),
            Err(RuntimeCallableSpecializationError::TypeMismatch)
        );
    }

    #[test]
    fn specialization_substitutes_const_and_effect_arguments_together() {
        let binder = RuntimeTypeBinder::new(0, 1, 1);
        let source_scope = RuntimeTypeScope::root().enter(binder).unwrap();
        let effect = RuntimeBoundEffectReference::from_coordinates(0, 0);
        let mut context = Context::default();
        context.types.insert(
            0,
            (
                RuntimeTypeScope::root(),
                RuntimePlanTypeProjection::Function {
                    contract: RuntimeFunctionTypeContract::new(
                        binder,
                        EffectPredicate::unconstrained(),
                        EffectFormula::literal(EffectSet::new(), Some(effect)),
                    ),
                    parameters: Box::new([2]),
                    result: 2,
                },
            ),
        );
        context.types.insert(
            1,
            (
                RuntimeTypeScope::root(),
                RuntimePlanTypeProjection::Function {
                    contract: RuntimeFunctionTypeContract::new(
                        RuntimeTypeBinder::EMPTY,
                        EffectPredicate::unconstrained(),
                        EffectFormula::empty(),
                    ),
                    parameters: Box::new([3]),
                    result: 3,
                },
            ),
        );
        context.types.insert(
            2,
            (
                source_scope,
                RuntimePlanTypeProjection::Array {
                    item: 4,
                    length: RuntimeArrayLength::Bound(
                        RuntimeBoundConstReference::from_coordinates(0, 0),
                    ),
                },
            ),
        );
        context.types.insert(
            3,
            (
                RuntimeTypeScope::root(),
                RuntimePlanTypeProjection::Array {
                    item: 4,
                    length: RuntimeArrayLength::Constant(3),
                },
            ),
        );
        context.types.insert(
            4,
            (RuntimeTypeScope::root(), RuntimePlanTypeProjection::Bool),
        );
        let source = RuntimeCallableStateId::for_index(0).unwrap();
        let target = RuntimeCallableStateId::for_index(1).unwrap();
        context.states.insert(
            source,
            state(
                0,
                source,
                2,
                RuntimeCallableTransition::RequiresSpecialization,
            ),
        );
        context.states.insert(
            target,
            state(
                1,
                source,
                3,
                RuntimeCallableTransition::Invoke {
                    function: 0,
                    captures: Box::new([]),
                    arguments: Box::new([]),
                },
            ),
        );
        let definition = RuntimeCallableSpecializationDefinition {
            source_type: 0,
            target_type: 1,
            arguments: RuntimeFunctionSpecializationArguments {
                types: Box::new([]),
                const_lengths: Box::new([RuntimeArrayLength::Constant(3)]),
                effects: Box::new([EffectFormula::empty()]),
            },
            states: Box::new([RuntimeCallableSpecializationState { source, target }]),
        };
        assert_eq!(definition.validate(&context, 10_000), Ok(()));
        context.types.get_mut(&3).unwrap().1 = RuntimePlanTypeProjection::Array {
            item: 4,
            length: RuntimeArrayLength::Constant(4),
        };
        assert_eq!(
            definition.validate(&context, 10_000),
            Err(RuntimeCallableSpecializationError::TypeMismatch)
        );
    }
}
