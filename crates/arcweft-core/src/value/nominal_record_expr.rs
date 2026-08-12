//! Checked nominal-record expression carriers.

use super::{
    RuntimeExpr, RuntimeNominalRecordLayout, RuntimeRecordFieldId, RuntimeRecordFieldIdError,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;
use thiserror::Error;

/// A nominal-record expression with one accepted layout authority.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeNominalRecordExpr {
    layout: Arc<RuntimeNominalRecordLayout>,
    initializers: Box<[RuntimeNominalRecordFieldExpr]>,
}

/// One authored-order initializer with its accepted defining-layout field ID.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeNominalRecordFieldExpr {
    field: RuntimeRecordFieldId,
    name: String,
    value: RuntimeExpr,
}

/// Failure to admit or revalidate a nominal-record initializer.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeNominalRecordInitializerError {
    #[error(
        "nominal record initializer has {actual} fields, exceeding the {maximum}-field identity space"
    )]
    TooManyFields { actual: usize, maximum: u32 },
    #[error("nominal record initializer contains duplicate field `{name}`")]
    DuplicateName { name: String },
    #[error("nominal record initializer contains unknown field `{name}`")]
    UnknownField { name: String },
    #[error("nominal record initializer is missing field {field:?} (`{name}`)")]
    MissingField {
        field: RuntimeRecordFieldId,
        name: String,
    },
    #[error("nominal record initializer field {ordinal} (`{name}`) has invalid identity")]
    InvalidFieldIdentity {
        ordinal: usize,
        name: String,
        source: RuntimeRecordFieldIdError,
    },
    #[error("nominal record initializer `{name}` carries field {actual:?}, expected {expected:?}")]
    FieldIdentityMismatch {
        name: String,
        expected: RuntimeRecordFieldId,
        actual: RuntimeRecordFieldId,
    },
}

impl RuntimeNominalRecordExpr {
    /// Admits authored-order initializers against the defining-layout authority.
    pub fn try_from_checked_initializers(
        layout: Arc<RuntimeNominalRecordLayout>,
        initializers_in_authored_order: Vec<(String, RuntimeExpr)>,
    ) -> Result<Self, RuntimeNominalRecordInitializerError> {
        if initializers_in_authored_order.len() > u32::MAX as usize {
            return Err(RuntimeNominalRecordInitializerError::TooManyFields {
                actual: initializers_in_authored_order.len(),
                maximum: u32::MAX,
            });
        }

        let mut names = BTreeSet::new();
        let mut initializers = Vec::with_capacity(initializers_in_authored_order.len());
        for (name, value) in initializers_in_authored_order {
            if !names.insert(name.clone()) {
                return Err(RuntimeNominalRecordInitializerError::DuplicateName { name });
            }
            let field = authoritative_field_id(&layout, &name)?;
            initializers.push(RuntimeNominalRecordFieldExpr { field, name, value });
        }
        require_all_layout_fields(&layout, &names)?;

        Ok(Self {
            layout,
            initializers: initializers.into_boxed_slice(),
        })
    }

    /// Revalidates a deserialized carrier before plan publication.
    pub fn validate(&self) -> Result<(), RuntimeNominalRecordInitializerError> {
        if self.initializers.len() > u32::MAX as usize {
            return Err(RuntimeNominalRecordInitializerError::TooManyFields {
                actual: self.initializers.len(),
                maximum: u32::MAX,
            });
        }

        let mut names = BTreeSet::new();
        for initializer in &self.initializers {
            if !names.insert(initializer.name.clone()) {
                return Err(RuntimeNominalRecordInitializerError::DuplicateName {
                    name: initializer.name.clone(),
                });
            }
            let expected = authoritative_field_id(&self.layout, &initializer.name)?;
            if initializer.field != expected {
                return Err(
                    RuntimeNominalRecordInitializerError::FieldIdentityMismatch {
                        name: initializer.name.clone(),
                        expected,
                        actual: initializer.field,
                    },
                );
            }
        }
        require_all_layout_fields(&self.layout, &names)
    }

    #[must_use]
    pub const fn layout(&self) -> &Arc<RuntimeNominalRecordLayout> {
        &self.layout
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
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn value(&self) -> &RuntimeExpr {
        &self.value
    }
}

fn authoritative_field_id(
    layout: &RuntimeNominalRecordLayout,
    name: &str,
) -> Result<RuntimeRecordFieldId, RuntimeNominalRecordInitializerError> {
    let Some((ordinal, _)) = layout
        .fields()
        .iter()
        .enumerate()
        .find(|(_, field)| field.name() == name)
    else {
        return Err(RuntimeNominalRecordInitializerError::UnknownField {
            name: name.to_owned(),
        });
    };
    RuntimeRecordFieldId::from_accepted_zero_based(ordinal).map_err(|source| {
        RuntimeNominalRecordInitializerError::InvalidFieldIdentity {
            ordinal,
            name: name.to_owned(),
            source,
        }
    })
}

fn require_all_layout_fields(
    layout: &RuntimeNominalRecordLayout,
    names: &BTreeSet<String>,
) -> Result<(), RuntimeNominalRecordInitializerError> {
    for (ordinal, field) in layout.fields().iter().enumerate() {
        if names.contains(field.name()) {
            continue;
        }
        let field_id =
            RuntimeRecordFieldId::from_accepted_zero_based(ordinal).map_err(|source| {
                RuntimeNominalRecordInitializerError::InvalidFieldIdentity {
                    ordinal,
                    name: field.name().to_owned(),
                    source,
                }
            })?;
        return Err(RuntimeNominalRecordInitializerError::MissingField {
            field: field_id,
            name: field.name().to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{RuntimeNominalTypeId, TypeLayoutHash};
    use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
    use crate::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan, RuntimePlanError};
    use crate::value::RuntimeValue;

    fn layout() -> Arc<RuntimeNominalRecordLayout> {
        Arc::new(
            RuntimeNominalRecordLayout::try_from_checked_projection(
                RuntimeNominalTypeId::try_new("game.Pair").unwrap(),
                RuntimeSemanticTypeId::from_bytes([7; 32]),
                TypeLayoutHash::from_bytes([9; 32]),
                vec![
                    ("alpha".to_owned(), RuntimeCheckedType::Bool),
                    ("zeta".to_owned(), RuntimeCheckedType::String),
                ],
            )
            .unwrap(),
        )
    }

    #[test]
    fn admission_retains_authored_order_and_assigns_layout_ids() {
        let expression = RuntimeNominalRecordExpr::try_from_checked_initializers(
            layout(),
            vec![
                (
                    "zeta".to_owned(),
                    RuntimeExpr::Value(RuntimeValue::String("second".to_owned())),
                ),
                (
                    "alpha".to_owned(),
                    RuntimeExpr::Value(RuntimeValue::Bool(true)),
                ),
            ],
        )
        .unwrap();

        assert_eq!(expression.initializers()[0].name(), "zeta");
        assert_eq!(expression.initializers()[0].field().get().get(), 2);
        assert_eq!(expression.initializers()[1].name(), "alpha");
        assert_eq!(expression.initializers()[1].field().get().get(), 1);
        expression.validate().unwrap();
    }

    #[test]
    fn admission_rejects_duplicate_unknown_and_missing_names() {
        let duplicate = RuntimeNominalRecordExpr::try_from_checked_initializers(
            layout(),
            vec![
                (
                    "alpha".to_owned(),
                    RuntimeExpr::Value(RuntimeValue::Bool(true)),
                ),
                (
                    "alpha".to_owned(),
                    RuntimeExpr::Value(RuntimeValue::Bool(false)),
                ),
            ],
        );
        assert!(matches!(
            duplicate,
            Err(RuntimeNominalRecordInitializerError::DuplicateName { .. })
        ));

        let unknown = RuntimeNominalRecordExpr::try_from_checked_initializers(
            layout(),
            vec![
                (
                    "alpha".to_owned(),
                    RuntimeExpr::Value(RuntimeValue::Bool(true)),
                ),
                ("other".to_owned(), RuntimeExpr::Value(RuntimeValue::Unit)),
            ],
        );
        assert!(matches!(
            unknown,
            Err(RuntimeNominalRecordInitializerError::UnknownField { .. })
        ));

        let missing = RuntimeNominalRecordExpr::try_from_checked_initializers(
            layout(),
            vec![("zeta".to_owned(), RuntimeExpr::Value(RuntimeValue::Unit))],
        );
        assert!(matches!(
            missing,
            Err(RuntimeNominalRecordInitializerError::MissingField { name, .. })
                if name == "alpha"
        ));
    }

    #[test]
    fn deserialized_field_identity_tampering_is_rejected() {
        let expression = RuntimeNominalRecordExpr::try_from_checked_initializers(
            layout(),
            vec![
                (
                    "alpha".to_owned(),
                    RuntimeExpr::Value(RuntimeValue::Bool(true)),
                ),
                (
                    "zeta".to_owned(),
                    RuntimeExpr::Value(RuntimeValue::String("value".to_owned())),
                ),
            ],
        )
        .unwrap();
        let mut json = serde_json::to_value(expression).unwrap();
        json["initializers"][0]["field"] = serde_json::json!(2);
        let tampered: RuntimeNominalRecordExpr = serde_json::from_value(json).unwrap();
        assert!(matches!(
            tampered.validate(),
            Err(RuntimeNominalRecordInitializerError::FieldIdentityMismatch { .. })
        ));

        let plan = RuntimePlan::new(
            vec![RuntimeFlow {
                id: FlowRuntimeId::canonical("main").unwrap(),
                ops: vec![FlowOp::ReturnExpr(RuntimeExpr::NominalRecord(tampered))],
            }],
            Vec::new(),
        )
        .unwrap();
        assert!(matches!(
            plan.verify(),
            Err(RuntimePlanError::InvalidNominalRecordExpression {
                source: RuntimeNominalRecordInitializerError::FieldIdentityMismatch { .. },
                ..
            })
        ));
    }
}
