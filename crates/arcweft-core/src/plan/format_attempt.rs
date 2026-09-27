//! Plan-owned manifests for source-ordered, flow-evaluated `fmt` operands.

use crate::runtime_id::{
    RuntimeDialogueContentTemplateId, RuntimeFormatAttemptId, RuntimePlanTypeId,
};
use crate::value::RuntimeFmtParameterId;

/// One admitted formatter operand in its source evaluation order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFormatAttemptOperand {
    parameter: RuntimeFmtParameterId,
    ty: RuntimePlanTypeId,
}

impl RuntimeFormatAttemptOperand {
    pub(crate) const fn new(parameter: RuntimeFmtParameterId, ty: RuntimePlanTypeId) -> Self {
        Self { parameter, ty }
    }

    #[must_use]
    pub const fn parameter(&self) -> RuntimeFmtParameterId {
        self.parameter
    }

    #[must_use]
    pub const fn ty(&self) -> RuntimePlanTypeId {
        self.ty
    }
}

/// Complete checked operand ABI for one flow-evaluated formatter occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFormatAttempt {
    id: RuntimeFormatAttemptId,
    template: RuntimeDialogueContentTemplateId,
    operands: Box<[RuntimeFormatAttemptOperand]>,
}

impl RuntimeFormatAttempt {
    pub(crate) fn new(
        id: RuntimeFormatAttemptId,
        template: RuntimeDialogueContentTemplateId,
        operands: Box<[RuntimeFormatAttemptOperand]>,
    ) -> Self {
        Self {
            id,
            template,
            operands,
        }
    }

    #[must_use]
    pub const fn id(&self) -> RuntimeFormatAttemptId {
        self.id
    }

    #[must_use]
    pub const fn template(&self) -> RuntimeDialogueContentTemplateId {
        self.template
    }

    #[must_use]
    pub const fn operands(&self) -> &[RuntimeFormatAttemptOperand] {
        &self.operands
    }

    #[must_use]
    pub fn operand_index(&self, parameter: RuntimeFmtParameterId) -> Option<usize> {
        self.operands
            .iter()
            .position(|operand| operand.parameter == parameter)
    }
}

/// Dense immutable owner for every formatter attempt referenced by a plan.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeFormatAttemptTable {
    rows: Box<[RuntimeFormatAttempt]>,
}

impl RuntimeFormatAttemptTable {
    pub(crate) fn from_admitted_rows(rows: Box<[RuntimeFormatAttempt]>) -> Self {
        Self { rows }
    }

    #[must_use]
    pub fn get(&self, id: RuntimeFormatAttemptId) -> Option<&RuntimeFormatAttempt> {
        self.rows.get(id.index())
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &RuntimeFormatAttempt> {
        self.rows.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
