//! One logical-value work allowance, including opaque internal payloads.

use std::{cell::RefCell, rc::Rc};

use crate::value::{RuntimeEntityReference, RuntimeScalarView, RuntimeValueView};

use super::{RuntimeSchemaError, RuntimeSchemaLimits};

pub(crate) struct ValueBudget {
    limits: RuntimeSchemaLimits,
    nodes: usize,
}

/// One admission allowance shared by resource, value, type and effect checks.
/// Cloning this private handle grants access to the same counter, never a fresh
/// quota. It exists only during validation and carries no runtime capability.
#[derive(Clone)]
pub(crate) struct ValidationWork(Rc<RefCell<ValidationWorkState>>);

pub(crate) struct ValidationWorkState {
    limits: RuntimeSchemaLimits,
    consumed: u64,
    failure: Option<RuntimeSchemaError>,
}

impl ValidationWork {
    /// Transfers the one counter only after all temporary validator borrows end.
    /// Failure returns the unchanged handle, so aliases cannot fork an allowance.
    pub(crate) fn try_into_state(self) -> Result<ValidationWorkState, Self> {
        Rc::try_unwrap(self.0)
            .map(RefCell::into_inner)
            .map_err(Self)
    }

    pub(crate) fn from_state(state: ValidationWorkState) -> Self {
        Self(Rc::new(RefCell::new(state)))
    }

    pub(crate) fn new(limits: RuntimeSchemaLimits) -> Self {
        Self(Rc::new(RefCell::new(ValidationWorkState {
            limits,
            consumed: 0,
            failure: None,
        })))
    }

    pub(crate) fn limits(&self) -> RuntimeSchemaLimits {
        self.0.borrow().limits
    }

    #[cfg(test)]
    pub(crate) fn consumed(&self) -> u64 {
        self.0.borrow().consumed
    }

    /// Retains the first hard resource failure across failed Choice branches
    /// and relation APIs whose internal result carries only admissibility.
    pub(crate) fn failure(&self) -> Option<RuntimeSchemaError> {
        self.0.borrow().failure.clone()
    }

    /// Keeps resource errors from metadata admission when an internal matcher
    /// can report only admissibility. A prior hard failure remains final.
    pub(crate) fn retain_failure(&self, error: RuntimeSchemaError) {
        self.0.borrow_mut().failure.get_or_insert(error);
    }

    pub(crate) fn charge(&mut self, depth: usize) -> Result<(), RuntimeSchemaError> {
        self.charge_atoms(1, depth)
    }

    /// A bounded semantic atom may account for several units without opening
    /// another counter or walking an attacker-controlled count to reject it.
    pub(crate) fn charge_atoms(
        &mut self,
        count: u64,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        let mut state = self.0.borrow_mut();
        if let Some(error) = &state.failure {
            return Err(error.clone());
        }
        let result = if !state.limits.permits_depth(depth) {
            Err(RuntimeSchemaError::ValidationDepth {
                path: "$".to_owned(),
                limit: state.limits.max_depth,
            })
        } else if let Some(consumed) = state.consumed.checked_add(count) {
            state.consumed = consumed;
            if consumed > state.limits.max_validation_work {
                Err(RuntimeSchemaError::ValidationWork {
                    path: "$".to_owned(),
                    limit: state.limits.max_validation_work,
                    consumed,
                })
            } else {
                Ok(())
            }
        } else {
            Err(RuntimeSchemaError::ValidationWork {
                path: "$".to_owned(),
                limit: state.limits.max_validation_work,
                consumed: state.consumed,
            })
        };
        if let Err(error) = &result {
            state.failure = Some(error.clone());
        }
        result
    }

    pub(crate) fn collection(&self, count: usize) -> Result<(), RuntimeSchemaError> {
        let mut state = self.0.borrow_mut();
        if let Some(error) = &state.failure {
            return Err(error.clone());
        }
        if state.limits.permits_sequence_items(count) {
            Ok(())
        } else {
            let error = RuntimeSchemaError::BudgetExceeded {
                budget: "sequence_items",
            };
            state.failure = Some(error.clone());
            Err(error)
        }
    }
}

impl ValueBudget {
    pub(crate) const fn new(limits: RuntimeSchemaLimits) -> Self {
        Self { limits, nodes: 0 }
    }

    pub(crate) fn node(&mut self, depth: usize) -> Result<(), RuntimeSchemaError> {
        if !self.limits.permits_depth(depth) {
            return Err(RuntimeSchemaError::BudgetExceeded { budget: "depth" });
        }
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or(RuntimeSchemaError::BudgetExceeded { budget: "nodes" })?;
        if !self.limits.permits_nodes(self.nodes) {
            return Err(RuntimeSchemaError::BudgetExceeded { budget: "nodes" });
        }
        Ok(())
    }

    pub(crate) fn collection(&self, length: usize) -> Result<(), RuntimeSchemaError> {
        if !self.limits.permits_sequence_items(length) {
            return Err(RuntimeSchemaError::BudgetExceeded {
                budget: "sequence_items",
            });
        }
        Ok(())
    }

    pub(crate) fn string(&self, text: &str) -> Result<(), RuntimeSchemaError> {
        self.string_length(text.len())
    }

    fn string_length(&self, length: usize) -> Result<(), RuntimeSchemaError> {
        if !self.limits.permits_string_bytes(length) {
            return Err(RuntimeSchemaError::BudgetExceeded {
                budget: "string_bytes",
            });
        }
        Ok(())
    }

    fn runtime_path(
        &self,
        path: &crate::runtime_id::RuntimeIdPath,
    ) -> Result<(), RuntimeSchemaError> {
        let length = path
            .canonical_len()
            .ok_or(RuntimeSchemaError::BudgetExceeded {
                budget: "string_bytes",
            })?;
        self.string_length(length)
    }

    pub(super) fn value(
        &mut self,
        value: RuntimeValueView<'_>,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        self.node(depth)?;
        self.shape(value)
    }

    pub(crate) fn shape(&self, value: RuntimeValueView<'_>) -> Result<(), RuntimeSchemaError> {
        match value {
            RuntimeValueView::Scalar(RuntimeScalarView::String(text)) => self.string(text)?,
            RuntimeValueView::Scalar(RuntimeScalarView::Progress { label, .. }) => {
                if let Some(label) = label {
                    self.string(label)?;
                }
            }
            RuntimeValueView::Scalar(RuntimeScalarView::EntityRef(reference)) => match reference {
                RuntimeEntityReference::StructuralFlow(flow) => {
                    self.runtime_path(flow.path())?;
                    self.string(flow.public_label_ref().as_str())?;
                }
                RuntimeEntityReference::ImportedProject(entity) => {
                    self.string(entity.public_id().as_str())?;
                    if let Some(flow) = entity.flow() {
                        self.runtime_path(flow.path())?;
                        self.string(flow.public_label_ref().as_str())?;
                    }
                    if let Some(line) = entity.line() {
                        self.runtime_path(line.path())?;
                    }
                }
                RuntimeEntityReference::Project { public_id, .. } => {
                    self.string(public_id.as_str())?
                }
                RuntimeEntityReference::CharacterLook { character, look } => {
                    self.string(character.as_str())?;
                    self.string(look.as_str())?;
                }
                RuntimeEntityReference::DialogueLine(line) => {
                    self.runtime_path(line.path())?;
                }
            },
            RuntimeValueView::DenseNumeric(value) => {
                self.collection(value.rank())?;
                self.collection(value.len())?;
            }
            RuntimeValueView::Tuple(tuple) => self.collection(tuple.len())?,
            RuntimeValueView::Record(record) => {
                self.collection(record.len())?;
                for index in 0..record.len() {
                    let (_, name, _) = record
                        .get(index)
                        .expect("logical record view has every indexed field");
                    self.string(name)?;
                }
            }
            RuntimeValueView::Sequence(sequence) => self.collection(sequence.len()?)?,
            RuntimeValueView::NominalRecord(record) => {
                self.string(record.type_id().as_str())?;
                self.collection(record.fields().len())?;
            }
            RuntimeValueView::Opaque(value) => self.string(value.producer().as_str())?,
            RuntimeValueView::Reduction(value) => {
                self.string(value.owner().producer().as_str())?;
                self.collection(value.commands().len())?;
                for command in value.commands().iter() {
                    self.string(command.constructor().as_str())?;
                    self.string(command.target().as_str())?;
                }
            }
            RuntimeValueView::Variant(variant) => {
                if let crate::pattern::RuntimeVariantIdentity::Nominal { nominal, .. } =
                    variant.owner()
                {
                    self.string(nominal.as_str())?;
                }
                self.string(variant.name())?;
            }
            RuntimeValueView::Agent(_) => {
                // Agent strings and structural predicate nodes are checked by
                // the same iterative visitor that emits their canonical atoms.
            }
            RuntimeValueView::Scalar(_)
            | RuntimeValueView::Range(_)
            | RuntimeValueView::Iterator(_)
            | RuntimeValueView::Need(_)
            | RuntimeValueView::Callable(_) => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod source_metadata_tests {
    use super::*;
    use crate::entry::schema::value_encoding::{ValueAdmissionOrigin, ValueResourceAdmission};
    use crate::plan::FlowRuntimeId;
    use crate::value::{AwbcRuntimeValueSnapshot, RuntimeEntityReference, RuntimeValue};

    #[test]
    fn entity_resource_preflight_bounds_retained_flow_labels_in_live_and_inert_graphs() {
        let limits = RuntimeSchemaLimits {
            max_string_bytes: 4,
            ..RuntimeSchemaLimits::engine_default()
        };
        for label in ["text", "texts"] {
            let reference = RuntimeEntityReference::StructuralFlow(
                FlowRuntimeId::from_runtime_contract("main", label).unwrap(),
            );
            let live = RuntimeValue::EntityRef(reference.clone());
            let inert = AwbcRuntimeValueSnapshot::EntityRef(reference);
            for (origin, view) in [
                (ValueAdmissionOrigin::Live, live.view()),
                (ValueAdmissionOrigin::InertSnapshot, inert.view()),
            ] {
                let mut resources = ValueResourceAdmission::snapshot(limits, origin);
                let result = resources.validate_view(view, 0, None);
                if label == "text" {
                    result.unwrap();
                } else {
                    let expected = RuntimeSchemaError::BudgetExceeded {
                        budget: "string_bytes",
                    };
                    assert_eq!(result, Err(expected.clone()));
                    assert_eq!(resources.work().failure(), Some(expected.clone()));
                    assert_eq!(
                        resources.validate_view(RuntimeValue::Unit.view(), 0, None),
                        Err(expected)
                    );
                }
            }
        }
    }
}
