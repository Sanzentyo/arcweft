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

struct ValidationWorkState {
    limits: RuntimeSchemaLimits,
    consumed: u64,
    failure: Option<RuntimeSchemaError>,
}

impl ValidationWork {
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
        let mut state = self.0.borrow_mut();
        if let Some(error) = &state.failure {
            return Err(error.clone());
        }
        let result = if !state.limits.permits_depth(depth) {
            Err(RuntimeSchemaError::ValidationDepth {
                path: "$".to_owned(),
                limit: state.limits.max_depth,
            })
        } else if let Some(consumed) = state.consumed.checked_add(1) {
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
        if !self.limits.permits_string_bytes(text.len()) {
            return Err(RuntimeSchemaError::BudgetExceeded {
                budget: "string_bytes",
            });
        }
        Ok(())
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
                    self.string(&flow.canonical_label())?
                }
                RuntimeEntityReference::ImportedProject(entity) => {
                    self.string(entity.public_id().as_str())?;
                    if let Some(flow) = entity.flow() {
                        self.string(&flow.canonical_label())?;
                    }
                    if let Some(line) = entity.line() {
                        self.string(&line.canonical_label())?;
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
                    self.string(&line.canonical_label())?
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
