//! One logical-value work allowance, including opaque internal payloads.

use crate::value::{RuntimeEntityReference, RuntimeScalarView, RuntimeValueView};

use super::{RuntimeSchemaError, RuntimeSchemaLimits};

pub(super) struct ValueBudget {
    limits: RuntimeSchemaLimits,
    nodes: usize,
}

/// One expected-type allowance, shared by every attempted Choice branch.
pub(crate) struct ValidationWork {
    limits: RuntimeSchemaLimits,
    consumed: u64,
}

impl ValidationWork {
    pub(crate) const fn new(limits: RuntimeSchemaLimits) -> Self {
        Self {
            limits,
            consumed: 0,
        }
    }

    pub(crate) const fn limits(&self) -> RuntimeSchemaLimits {
        self.limits
    }

    pub(crate) fn charge(&mut self, depth: usize) -> Result<(), RuntimeSchemaError> {
        if !self.limits.permits_depth(depth) {
            return Err(RuntimeSchemaError::ValidationDepth {
                path: "$".to_owned(),
                limit: self.limits.max_depth,
            });
        }
        self.consumed = self
            .consumed
            .checked_add(1)
            .ok_or(RuntimeSchemaError::ValidationWork {
                path: "$".to_owned(),
                limit: self.limits.max_validation_work,
                consumed: self.consumed,
            })?;
        if self.consumed > self.limits.max_validation_work {
            return Err(RuntimeSchemaError::ValidationWork {
                path: "$".to_owned(),
                limit: self.limits.max_validation_work,
                consumed: self.consumed,
            });
        }
        Ok(())
    }

    pub(crate) fn collection(&self, count: usize) -> Result<(), RuntimeSchemaError> {
        if self.limits.permits_sequence_items(count) {
            Ok(())
        } else {
            Err(RuntimeSchemaError::BudgetExceeded {
                budget: "sequence_items",
            })
        }
    }
}

impl ValueBudget {
    pub(super) const fn new(limits: RuntimeSchemaLimits) -> Self {
        Self { limits, nodes: 0 }
    }

    pub(super) fn node(&mut self, depth: usize) -> Result<(), RuntimeSchemaError> {
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

    pub(super) fn collection(&self, length: usize) -> Result<(), RuntimeSchemaError> {
        if !self.limits.permits_sequence_items(length) {
            return Err(RuntimeSchemaError::BudgetExceeded {
                budget: "sequence_items",
            });
        }
        Ok(())
    }

    pub(super) fn string(&self, text: &str) -> Result<(), RuntimeSchemaError> {
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

    pub(super) fn shape(&self, value: RuntimeValueView<'_>) -> Result<(), RuntimeSchemaError> {
        match value {
            RuntimeValueView::Scalar(RuntimeScalarView::String(text)) => self.string(text)?,
            RuntimeValueView::Scalar(RuntimeScalarView::Progress(progress)) => {
                if let Some(label) = progress.label() {
                    self.string(label)?;
                }
            }
            RuntimeValueView::Scalar(RuntimeScalarView::EntityRef(reference)) => match reference {
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
            RuntimeValueView::Sequence(sequence) => self.collection(sequence.len())?,
            RuntimeValueView::NominalRecord(record) => {
                self.string(record.type_id().as_str())?;
                self.collection(record.fields().len())?;
            }
            RuntimeValueView::Opaque(value) => self.string(value.producer().as_str())?,
            RuntimeValueView::Reduction(value) => {
                self.string(value.owner().producer().as_str())?;
                self.collection(value.commands().len())?;
                for command in value.commands() {
                    self.string(command.constructor().as_str())?;
                    self.string(command.target().as_str())?;
                }
            }
            RuntimeValueView::Variant { owner, name, .. } => {
                if let crate::pattern::RuntimeVariantIdentity::Nominal { nominal, .. } = owner {
                    self.string(nominal.as_str())?;
                }
                self.string(name)?;
            }
            RuntimeValueView::Agent(_) => {
                // Agent strings and structural predicate nodes are checked by
                // the same iterative visitor that emits their canonical atoms.
            }
            RuntimeValueView::Scalar(_) | RuntimeValueView::RuntimeOnly(_) => {}
        }
        Ok(())
    }
}
