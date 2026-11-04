//! One borrowed proof aligns declared operands with the live remaining formals.

use super::*;

enum DeclaredBinding {
    Value(usize),
    Optional(Option<usize>),
    EmptyRest,
}

pub(crate) struct RuntimeCallableDeclaredGroupProjection {
    bindings: Vec<DeclaredBinding>,
    sources: usize,
}

impl RuntimeCallableDeclaredGroupProjection {
    /// Called only after all borrowed values and their ownership projection pass.
    pub(crate) fn materialize(self, values: Vec<RuntimeValue>) -> Vec<RuntimeValue> {
        assert_eq!(values.len(), self.sources);
        let mut values = values.into_iter().map(Some).collect::<Vec<_>>();
        let mut take = |index: usize| {
            values[index]
                .take()
                .expect("unique declared binding source")
        };
        let result = self
            .bindings
            .into_iter()
            .map(|binding| match binding {
                DeclaredBinding::Value(index) => take(index),
                DeclaredBinding::Optional(Some(index)) => RuntimeValue::option_some(take(index)),
                DeclaredBinding::Optional(None) => RuntimeValue::option_none(),
                DeclaredBinding::EmptyRest => crate::value::runtime_sequence_values(Vec::new()),
            })
            .collect();
        assert!(values.iter().all(Option::is_none));
        result
    }
}

impl RuntimeCallableValue {
    pub(crate) fn inspect_declared_group(
        &self,
        expected: RuntimeCallableStateId,
        group: u32,
        parameters: &[u32],
        values: &[RuntimeCallableMaterializedArgument<'_>],
    ) -> Result<RuntimeCallableDeclaredGroupProjection, RuntimeCallableValueError> {
        let invalid = || RuntimeCallableValueError::InputProjection { state: self.state };
        let kinds = match &self.owner {
            RuntimeProgramOwner::Plan(plan) => {
                let base = plan.callable_states().get(expected).ok_or_else(invalid)?;
                let actual = plan.callable_states().get(self.state).ok_or_else(invalid)?;
                self.declared_inputs(base, actual, group)?
            }
            RuntimeProgramOwner::Awbc(program) => {
                let base = program
                    .callable_states
                    .get(expected.index())
                    .ok_or_else(invalid)?;
                let actual = program
                    .callable_states
                    .get(self.state.index())
                    .ok_or_else(invalid)?;
                self.declared_inputs(base, actual, group)?
            }
        };
        if parameters.len() != values.len() || parameters.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid());
        }
        let mut bindings = Vec::with_capacity(kinds.len());
        let mut inspected = Vec::with_capacity(kinds.len());
        let mut used = 0;
        for (parameter, kind) in kinds {
            let source = parameters.binary_search(&parameter).ok();
            used += usize::from(source.is_some());
            match (kind, source.map(|index| values[index])) {
                (
                    RuntimeCallableParameterKind::Fixed,
                    Some(value @ RuntimeCallableMaterializedArgument::Fixed(_)),
                )
                | (
                    RuntimeCallableParameterKind::Rest,
                    Some(value @ RuntimeCallableMaterializedArgument::Rest(_)),
                ) => {
                    bindings.push(DeclaredBinding::Value(source.expect("present source")));
                    inspected.push(value);
                }
                (
                    RuntimeCallableParameterKind::Optional,
                    Some(RuntimeCallableMaterializedArgument::Fixed(value)),
                ) => {
                    bindings.push(DeclaredBinding::Optional(source));
                    inspected.push(RuntimeCallableMaterializedArgument::Optional(Some(value)));
                }
                (RuntimeCallableParameterKind::Optional, None) => {
                    bindings.push(DeclaredBinding::Optional(None));
                    inspected.push(RuntimeCallableMaterializedArgument::Optional(None));
                }
                (RuntimeCallableParameterKind::Rest, None) => {
                    bindings.push(DeclaredBinding::EmptyRest);
                    inspected.push(RuntimeCallableMaterializedArgument::Rest(&[]));
                }
                _ => return Err(invalid()),
            }
        }
        if used != values.len() {
            return Err(invalid());
        }
        self.inspect_group_materialization(&inspected, None)?;
        Ok(RuntimeCallableDeclaredGroupProjection {
            bindings,
            sources: values.len(),
        })
    }

    fn declared_inputs<T, F>(
        &self,
        base: &RuntimeCallableStateDefinition<T, F>,
        actual: &RuntimeCallableStateDefinition<T, F>,
        group: u32,
    ) -> Result<Vec<(u32, RuntimeCallableParameterKind)>, RuntimeCallableValueError> {
        if matches!(
            base.position,
            crate::plan::RuntimeCallablePosition::WithinGroup { .. }
        ) || base.origin != actual.origin
            || base.active_group() != Some(group)
            || actual.active_group() != Some(group)
            || !matches!(base.attached, RuntimeCallableAttachedContract::None)
            || !matches!(actual.attached, RuntimeCallableAttachedContract::None)
        {
            return Err(RuntimeCallableValueError::InputProjection { state: self.state });
        }
        Ok(actual
            .parameters
            .iter()
            .map(|row| (row.coordinate.parameter, row.kind))
            .collect())
    }
}
