//! Type-preserving canonical value transformations.

use super::{RuntimeOpaqueValueError, RuntimeSeq, RuntimeValue, RuntimeValueNestingError};
use crate::entry::RuntimeSchemaError;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeValueNormalizationError {
    #[error(transparent)]
    Nesting(#[from] RuntimeValueNestingError),
    #[error(transparent)]
    Schema(#[from] RuntimeSchemaError),
    #[error(transparent)]
    Opaque(#[from] RuntimeOpaqueValueError),
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeValueMergeError {
    #[error(transparent)]
    Nesting(#[from] RuntimeValueNestingError),
    #[error("sparse value merging requires unrestricted values")]
    Affine,
}

impl RuntimeValue {
    /// Canonicalizes finite floating-point zeros without changing value types or headers.
    /// Sequence storage becomes canonical ordered rows for backend-independent codecs.
    pub fn try_normalize_finite_floats(self) -> Result<Self, RuntimeValueNormalizationError> {
        self.validate_nesting_depth(super::MAX_RUNTIME_VALUE_NESTING_DEPTH)?;
        normalize_runtime_value(self)
    }

    /// Applies a sparse typed value patch while retaining matching container identities.
    pub fn try_merge_sparse_patch(&self, incoming: &Self) -> Result<Self, RuntimeValueMergeError> {
        self.validate_nesting_depth(super::MAX_RUNTIME_VALUE_NESTING_DEPTH)?;
        incoming.validate_nesting_depth(super::MAX_RUNTIME_VALUE_NESTING_DEPTH)?;
        if !self.ownership().permits_copy() || !incoming.ownership().permits_copy() {
            return Err(RuntimeValueMergeError::Affine);
        }
        Ok(merge_runtime_value(self, incoming))
    }
}

fn normalize_runtime_value(
    value: RuntimeValue,
) -> Result<RuntimeValue, RuntimeValueNormalizationError> {
    match value {
        RuntimeValue::F32(value) => {
            if !value.is_finite() {
                return Err(RuntimeSchemaError::NonFinite {
                    path: "$".to_owned(),
                    kind: "f32",
                }
                .into());
            }
            Ok(RuntimeValue::F32(if value == 0.0 { 0.0 } else { value }))
        }
        RuntimeValue::F64(value) => {
            if !value.is_finite() {
                return Err(RuntimeSchemaError::NonFinite {
                    path: "$".to_owned(),
                    kind: "f64",
                }
                .into());
            }
            Ok(RuntimeValue::F64(if value == 0.0 { 0.0 } else { value }))
        }
        RuntimeValue::Tuple(values) => values
            .into_iter()
            .map(normalize_runtime_value)
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeValue::Tuple),
        RuntimeValue::Seq(sequence) => sequence
            .into_values()
            .into_iter()
            .map(normalize_runtime_value)
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeSeq::values)
            .map(RuntimeValue::Seq),
        RuntimeValue::Record(fields) => fields
            .try_map_values(normalize_runtime_value)
            .map(RuntimeValue::Record),
        RuntimeValue::Opaque(value) => {
            let owner = crate::pattern::RuntimeOpaqueTypeOwner::exact_with(
                value.producer().clone(),
                value.semantic_identity(),
                value.value_class(),
                value.persistence(),
            );
            Ok(owner.try_wrap(normalize_runtime_value(value.into_payload())?)?)
        }
        RuntimeValue::NominalRecord(record) => record
            .try_map_fields(normalize_runtime_value)
            .map(RuntimeValue::NominalRecord),
        RuntimeValue::Variant(variant) => variant
            .try_map_payload(normalize_runtime_value)
            .map(RuntimeValue::Variant),
        value => Ok(value),
    }
}

fn merge_runtime_value(base: &RuntimeValue, incoming: &RuntimeValue) -> RuntimeValue {
    if matches!(
        incoming.builtin_variant_case(),
        Some((
            crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionNone,
            None
        ))
    ) {
        return base.clone();
    }
    if let Some((crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(patch))) =
        incoming.builtin_variant_case()
    {
        let value = match base.builtin_variant_case() {
            Some((
                crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionSome,
                Some(current),
            )) => merge_runtime_value(current, patch),
            _ => patch.clone(),
        };
        return RuntimeValue::option_some(value);
    }

    match (base, incoming) {
        (RuntimeValue::Opaque(base), RuntimeValue::Opaque(incoming))
            if base.producer() == incoming.producer()
                && base.semantic_identity() == incoming.semantic_identity()
                && base.value_class() == incoming.value_class()
                && base.persistence() == incoming.persistence() =>
        {
            let owner = crate::pattern::RuntimeOpaqueTypeOwner::exact_with(
                base.producer().clone(),
                base.semantic_identity(),
                base.value_class(),
                base.persistence(),
            );
            let payload = merge_runtime_value(base.payload(), incoming.payload());
            owner
                .try_wrap(payload)
                .expect("merging a validated opaque payload preserves its exact owner")
        }
        (RuntimeValue::Tuple(base), RuntimeValue::Tuple(incoming))
            if base.len() == incoming.len() =>
        {
            RuntimeValue::Tuple(
                base.iter()
                    .zip(incoming)
                    .map(|(base, incoming)| merge_runtime_value(base, incoming))
                    .collect(),
            )
        }
        (RuntimeValue::Record(base), RuntimeValue::Record(incoming))
            if base.len() == incoming.len()
                && base.iter().zip(incoming).all(|(base, incoming)| {
                    base.field() == incoming.field() && base.name() == incoming.name()
                }) =>
        {
            RuntimeValue::try_record(
                base.iter()
                    .zip(incoming)
                    .map(|(base, incoming)| {
                        (
                            base.name().to_owned(),
                            merge_runtime_value(base.value(), incoming.value()),
                        )
                    })
                    .collect(),
            )
            .expect("merging a validated record preserves its unique field layout")
        }
        (RuntimeValue::NominalRecord(base), RuntimeValue::NominalRecord(incoming))
            if base.type_id() == incoming.type_id()
                && base.semantic_identity() == incoming.semantic_identity()
                && base.layout() == incoming.layout()
                && base.fields().len() == incoming.fields().len()
                && base.type_instantiation() == incoming.type_instantiation() =>
        {
            RuntimeValue::NominalRecord(merge_nominal_fields(base, incoming))
        }
        (RuntimeValue::Variant(base), RuntimeValue::Variant(incoming))
            if base.owner() == incoming.owner()
                && base.ordinal() == incoming.ordinal()
                && base.type_instantiation() == incoming.type_instantiation()
                && base.name() == incoming.name()
                && base.payload().is_some()
                && incoming.payload().is_some() =>
        {
            RuntimeValue::Variant(
                incoming
                    .clone()
                    .try_map_payload(|incoming| {
                        Ok::<_, std::convert::Infallible>(merge_runtime_value(
                            base.payload().expect("matched present payload"),
                            &incoming,
                        ))
                    })
                    .expect("infallible value merge"),
            )
        }
        // A leaf Set replaces the prior leaf. Structured containers recurse
        // only when both values carry the same exact structural identity.
        _ => incoming.clone(),
    }
}

fn merge_nominal_fields(
    base: &super::RuntimeNominalRecordValue,
    incoming: &super::RuntimeNominalRecordValue,
) -> super::RuntimeNominalRecordValue {
    let mut base_fields = base.fields().iter();
    incoming.clone().map_fields(|incoming| {
        merge_runtime_value(
            base_fields
                .next()
                .expect("equal defining-order field counts"),
            &incoming,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{
        RuntimeNominalDeclarationId, RuntimeNominalSchemaBody, RuntimeNominalSchemaCase,
        RuntimeNominalSchemaDefinition, RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity,
        RuntimeNominalTypeId, RuntimeSchemaLimits, RuntimeTypeSchema,
    };
    use crate::pattern::{
        RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
    };

    #[test]
    fn nominal_variant_transform_preserves_header_and_rejects_unbound_serde() {
        let semantic = RuntimeSemanticTypeId::from_bytes([37; 32]);
        let graph = RuntimeNominalSchemaGraph::try_new(
            vec![RuntimeNominalSchemaDefinition::new(
                RuntimeNominalDeclarationId::from_bytes(*semantic.as_bytes()),
                RuntimeNominalSchemaIdentity::new(
                    RuntimeNominalTypeId::try_new("test.FloatCase").unwrap(),
                    semantic,
                ),
                vec![],
                RuntimeNominalSchemaBody::Variant {
                    cases: vec![
                        RuntimeNominalSchemaCase::new(
                            0,
                            "Full".to_owned(),
                            Some(RuntimeTypeSchema::F64),
                        ),
                        RuntimeNominalSchemaCase::new(1, "Empty".to_owned(), None),
                    ]
                    .into(),
                },
            )],
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
        let value = graph
            .try_variant_value(
                semantic,
                0,
                Some(RuntimeValue::F64(-0.0)),
                RuntimeSchemaLimits::engine_default(),
            )
            .unwrap();
        let normalized = value.clone().try_normalize_finite_floats().unwrap();
        let RuntimeValue::Variant(original) = &value else {
            unreachable!()
        };
        let super::super::view::RuntimeValueView::Variant(borrowed) = value.view() else {
            unreachable!()
        };
        assert!(std::ptr::eq(borrowed, original));
        let copied = value
            .view()
            .copy_unrestricted()
            .expect("unrestricted borrowed variant");
        assert_eq!(copied, value);
        let RuntimeValue::Variant(normalized_variant) = &normalized else {
            unreachable!()
        };
        assert_eq!(normalized_variant.owner(), original.owner());
        assert_eq!(normalized_variant.ordinal(), original.ordinal());
        assert_eq!(normalized_variant.name(), original.name());
        let Some(RuntimeValue::F64(number)) = normalized_variant.payload() else {
            unreachable!()
        };
        assert_eq!(number.to_bits(), 0.0_f64.to_bits());
        assert!(
            graph
                .accepts_value(semantic, &normalized, RuntimeSchemaLimits::engine_default())
                .is_ok()
        );
        assert!(serde_json::to_value(&value).is_err());
        let wire = serde_json::json!({"Variant": { "owner": original.owner(), "ordinal": original.ordinal(),
            "name": original.name(), "payload": {"F64": -0.0}, "type_instantiation": null }});
        assert!(serde_json::from_value::<RuntimeValue>(wire).is_err());
        assert_eq!(
            value.try_merge_sparse_patch(&normalized).unwrap(),
            normalized
        );
        let empty = graph
            .try_variant_value(semantic, 1, None, RuntimeSchemaLimits::engine_default())
            .unwrap();
        assert_eq!(value.try_merge_sparse_patch(&empty).unwrap(), empty);
        assert!(matches!(
            RuntimeValue::option_some(RuntimeValue::F64(f64::NAN)).try_normalize_finite_floats(),
            Err(RuntimeValueNormalizationError::Schema(
                RuntimeSchemaError::NonFinite { .. }
            ))
        ));
    }

    #[test]
    fn sparse_merge_rejects_nested_affine_values_before_copying() {
        let owner = RuntimeOpaqueTypeOwner::exact_with(
            RuntimeOpaqueTypeProducerId::try_new("test.affine").unwrap(),
            RuntimeSemanticTypeId::from_bytes([38; 32]),
            super::super::RuntimeOpaqueValueClass::AffineHandle(
                super::super::RuntimeHandleKind::Cue,
            ),
            super::super::RuntimeOpaquePersistence::SnapshotOnly,
        );
        let value = RuntimeValue::option_some(owner.try_wrap(RuntimeValue::Unit).unwrap());
        assert!(matches!(
            value.try_merge_sparse_patch(&RuntimeValue::option_none()),
            Err(RuntimeValueMergeError::Affine)
        ));
        assert!(matches!(
            RuntimeValue::Unit.try_merge_sparse_patch(&value),
            Err(RuntimeValueMergeError::Affine)
        ));
        assert!(!value.ownership().permits_copy());
        assert!(value.view().copy_unrestricted().is_none());
    }
}
