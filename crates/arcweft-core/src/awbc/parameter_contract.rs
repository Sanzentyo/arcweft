//! Joint binding of declaration-owned generic effect input scopes.

use super::schema::{AwbcProgram, AwbcRuntimeTypeShape, AwbcTypeId};
use crate::{
    effect_row::{DecisionControl, DecisionWork, EffectPredicate},
    plan::RuntimeBoundEffectReference,
    value::RuntimeValue,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ContractVariable {
    Parameter(u32),
    Local { binder: u32, depth: u32, slot: u32 },
}

struct ContractWork(u64);
impl DecisionControl for ContractWork {
    type Error = ();
    fn charge(&mut self, _: DecisionWork) -> Result<(), ()> {
        self.0 = self.0.checked_sub(1).ok_or(())?;
        Ok(())
    }
}

struct ParameterMatcher<'a> {
    program: &'a AwbcProgram,
    predicate: EffectPredicate<ContractVariable>,
    locals: BTreeSet<ContractVariable>,
    next_binder: u32,
    work: ContractWork,
}

impl AwbcProgram {
    /// Checks all supplied input types together under one declaration binder.
    pub fn parameter_contract_accepts_types(
        &self,
        contract: AwbcTypeId,
        inputs: impl IntoIterator<Item = (usize, AwbcTypeId)>,
    ) -> bool {
        let Some((parameters, mut matcher)) = ParameterMatcher::new(self, contract) else {
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
            if !seen.insert(ordinal) || matcher.types(*expected, actual, 0, true, false).is_err() {
                return false;
            }
        }
        matcher.accepted()
    }

    /// Checks live input values under the same joint generic contract as codecs.
    pub fn parameter_contract_accepts_values<'value>(
        &self,
        contract: AwbcTypeId,
        inputs: impl IntoIterator<Item = (usize, &'value RuntimeValue)>,
    ) -> bool {
        let Some((parameters, mut matcher)) = ParameterMatcher::new(self, contract) else {
            return false;
        };
        let mut seen = BTreeSet::new();
        for (ordinal, value) in inputs {
            let Some(expected) = parameters.get(ordinal) else {
                return false;
            };
            if !seen.insert(ordinal) || matcher.value(*expected, value, 0).is_err() {
                return false;
            }
        }
        matcher.accepted()
    }
}

impl<'a> ParameterMatcher<'a> {
    fn new(program: &'a AwbcProgram, contract: AwbcTypeId) -> Option<(&'a [AwbcTypeId], Self)> {
        let row = program.runtime_types.get(contract.index())?;
        let AwbcRuntimeTypeShape::Function {
            contract,
            parameters,
            result,
        } = row.shape()
        else {
            return None;
        };
        if !row.scope().binders().is_empty()
            || contract.binder().types() != 0
            || contract.binder().const_lengths() != 0
            || contract.binder().effects() == 0
            || contract.invocation() != &crate::effect_row::EffectFormula::empty()
            || !matches!(
                program.runtime_types.get(result.index())?.shape(),
                AwbcRuntimeTypeShape::Unit
            )
        {
            return None;
        }
        let mut work =
            ContractWork(crate::entry::RuntimeSchemaLimits::engine_default().max_validation_work);
        let predicate = contract
            .predicate()
            .map_references(&mut work, &mut |reference, _| {
                if reference.depth() != 0 || reference.slot() >= contract.binder().effects() {
                    return Err(());
                }
                Ok(ContractVariable::Parameter(reference.slot()))
            })
            .ok()?;
        Some((
            parameters,
            Self {
                program,
                predicate,
                locals: BTreeSet::new(),
                next_binder: 0,
                work,
            },
        ))
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
            .filter(|variable| matches!(variable, ContractVariable::Parameter(_)))
            .cloned()
            .collect();
        predicate
            .project(&parameters, &mut self.work)
            .is_ok_and(|predicate| predicate.is_unconstrained())
    }

    fn enter(&mut self, depth: usize) -> Result<(), ()> {
        if depth > 64 {
            return Err(());
        }
        self.work.charge(DecisionWork::Visit)
    }

    fn types(
        &mut self,
        expected: AwbcTypeId,
        actual: AwbcTypeId,
        depth: usize,
        expected_parameter: bool,
        actual_parameter: bool,
    ) -> Result<(), ()> {
        self.enter(depth)?;
        let expected_row = self.program.runtime_types.get(expected.index()).ok_or(())?;
        let actual_row = self.program.runtime_types.get(actual.index()).ok_or(())?;
        let expected_parameter = expected_parameter && !expected_row.scope().is_root();
        let actual_parameter = actual_parameter && !actual_row.scope().is_root();
        if expected == actual {
            return Ok(());
        }
        match (expected_row.shape(), actual_row.shape()) {
            (
                AwbcRuntimeTypeShape::Function {
                    contract: expected_contract,
                    parameters: expected_parameters,
                    result: expected_result,
                },
                AwbcRuntimeTypeShape::Function {
                    contract: actual_contract,
                    parameters: actual_parameters,
                    result: actual_result,
                },
            ) => {
                if expected_contract.binder() != actual_contract.binder()
                    || !expected_contract.binder().is_empty()
                    || expected_parameters.len() != actual_parameters.len()
                {
                    return Err(());
                }
                for (expected, actual) in expected_parameters.iter().zip(actual_parameters) {
                    self.types(
                        *actual,
                        *expected,
                        depth + 1,
                        actual_parameter,
                        expected_parameter,
                    )?;
                }
                self.types(
                    *expected_result,
                    *actual_result,
                    depth + 1,
                    expected_parameter,
                    actual_parameter,
                )?;
                let binder = self.next_binder;
                self.next_binder = binder.checked_add(1).ok_or(())?;
                let expected_scope = expected_row.scope().binders().len()
                    + usize::from(!expected_contract.binder().is_empty());
                let actual_scope = actual_row.scope().binders().len()
                    + usize::from(!actual_contract.binder().is_empty());
                let map = |reference: &RuntimeBoundEffectReference,
                           root: bool,
                           scope: usize|
                 -> Result<ContractVariable, ()> {
                    let depth = usize::try_from(reference.depth()).map_err(|_| ())?;
                    if depth >= scope {
                        return Err(());
                    }
                    if root && depth + 1 == scope {
                        Ok(ContractVariable::Parameter(reference.slot()))
                    } else {
                        Ok(ContractVariable::Local {
                            binder,
                            depth: reference.depth(),
                            slot: reference.slot(),
                        })
                    }
                };
                let expected = expected_contract
                    .invocation()
                    .map_references(&mut self.work, &mut |reference, _| {
                        map(reference, expected_parameter, expected_scope)
                    })?;
                let actual = actual_contract
                    .invocation()
                    .map_references(&mut self.work, &mut |reference, _| {
                        map(reference, actual_parameter, actual_scope)
                    })?;
                for variable in expected.variables().chain(actual.variables()) {
                    if matches!(variable, ContractVariable::Local { .. }) {
                        self.locals.insert(variable.clone());
                    }
                }
                let relation = actual.subset(&expected, &mut self.work)?;
                self.predicate = self.predicate.and(&relation, &mut self.work)?;
                if expected_contract.predicate() != actual_contract.predicate() {
                    return Err(());
                }
                Ok(())
            }
            (AwbcRuntimeTypeShape::Tuple(expected), AwbcRuntimeTypeShape::Tuple(actual))
                if expected.len() == actual.len() =>
            {
                for (expected, actual) in expected.iter().zip(actual) {
                    self.types(
                        *expected,
                        *actual,
                        depth + 1,
                        expected_parameter,
                        actual_parameter,
                    )?;
                }
                Ok(())
            }
            (
                AwbcRuntimeTypeShape::BoundType(expected),
                AwbcRuntimeTypeShape::BoundType(actual),
            ) if expected == actual => Ok(()),
            _ if self.program.types_compatible(expected, actual) => Ok(()),
            _ => Err(()),
        }
    }

    fn value(
        &mut self,
        expected: AwbcTypeId,
        value: &RuntimeValue,
        depth: usize,
    ) -> Result<(), ()> {
        self.enter(depth)?;
        let row = self.program.runtime_types.get(expected.index()).ok_or(())?;
        match (row.shape(), value) {
            (AwbcRuntimeTypeShape::Function { .. }, RuntimeValue::Callable(value)) => {
                if !matches!(value.owner(), crate::task::RuntimeProgramOwner::Awbc(owner) if std::ptr::eq(owner.as_ref(), self.program))
                    || value.validate_retained().is_err()
                {
                    return Err(());
                }
                let actual = value.function_type().map_err(|_| ())?;
                let actual = self
                    .program
                    .runtime_types
                    .iter()
                    .position(|ty| ty.semantic_identity() == actual)
                    .and_then(|index| u32::try_from(index).ok())
                    .map(AwbcTypeId)
                    .ok_or(())?;
                self.types(expected, actual, depth + 1, true, false)
            }
            (AwbcRuntimeTypeShape::Tuple(types), RuntimeValue::Tuple(values))
                if types.len() == values.len() =>
            {
                for (ty, value) in types.iter().zip(values) {
                    self.value(*ty, value, depth + 1)?;
                }
                Ok(())
            }
            _ if self.program.value_matches_type(value, expected) => Ok(()),
            _ => Err(()),
        }
    }
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
    }
}
