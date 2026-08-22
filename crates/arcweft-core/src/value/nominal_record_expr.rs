//! Checked nominal-record expression carriers.

use super::{RuntimeExpr, RuntimeRecordFieldId};

/// A nominal-record expression whose nominal owner is the enclosing typed
/// expression node.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNominalRecordExpr {
    initializers: Box<[RuntimeNominalRecordFieldExpr]>,
}

/// One authored-order initializer with its accepted defining-layout field ID.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNominalRecordFieldExpr {
    field: RuntimeRecordFieldId,
    value: RuntimeExpr,
}

impl RuntimeNominalRecordExpr {
    pub(crate) fn from_admitted_parts(
        initializers: Vec<(RuntimeRecordFieldId, RuntimeExpr)>,
    ) -> Self {
        Self {
            initializers: initializers
                .into_iter()
                .map(|(field, value)| RuntimeNominalRecordFieldExpr { field, value })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    #[must_use]
    pub fn initializers(&self) -> &[RuntimeNominalRecordFieldExpr] {
        &self.initializers
    }
}

impl RuntimeNominalRecordFieldExpr {
    #[must_use]
    pub const fn field(&self) -> RuntimeRecordFieldId {
        self.field
    }

    #[must_use]
    pub const fn value(&self) -> &RuntimeExpr {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_id::RuntimePlanTypeId;
    use crate::value::{RuntimeExprKind, RuntimeValue};
    use std::num::NonZeroU32;

    fn expr(value: RuntimeValue) -> RuntimeExpr {
        RuntimeExpr::from_admitted_parts(
            RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN),
            RuntimeExprKind::Value(value),
        )
    }

    fn field(ordinal: usize) -> RuntimeRecordFieldId {
        RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap()
    }

    #[test]
    fn admitted_parts_retain_authored_order_and_field_ids() {
        let expression = RuntimeNominalRecordExpr::from_admitted_parts(vec![
            (field(1), expr(RuntimeValue::String("second".to_owned()))),
            (field(0), expr(RuntimeValue::Bool(true))),
        ]);

        assert_eq!(expression.initializers()[0].field().get().get(), 2);
        assert_eq!(expression.initializers()[1].field().get().get(), 1);
    }
}
