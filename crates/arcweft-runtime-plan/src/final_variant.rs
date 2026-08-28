//! Exact synthetic variant projection shared by expression and Flow lowering.

use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    RuntimeExprSeed, RuntimeExprSeedKind, RuntimeLocalSeedId, RuntimePatternSeed,
    RuntimePatternSeedKind,
};
use thiserror::Error;

use crate::semantic_facts::{RuntimeNormalizedType, RuntimeNormalizedVariantSelectionError};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum RuntimeNormalizedVariantSeedError {
    #[error(transparent)]
    Selection(#[from] RuntimeNormalizedVariantSelectionError),
    #[error(
        "normalized variant type {owner:?} case {ordinal} payload presence does not match its raw value"
    )]
    PayloadPresence {
        owner: RuntimeSemanticTypeId,
        ordinal: u32,
    },
    #[error(
        "normalized variant type {owner:?} case {ordinal} expects raw payload {expected:?}, found {actual:?}"
    )]
    PayloadType {
        owner: RuntimeSemanticTypeId,
        ordinal: u32,
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },
    #[error(
        "normalized variant type {owner:?} case {ordinal} has inconsistent selected payload evidence"
    )]
    InconsistentSelection {
        owner: RuntimeSemanticTypeId,
        ordinal: u32,
    },
}

/// Constructs one selected variant while preserving its exact structural
/// payload. The caller supplies only the raw unary value; this boundary emits
/// the declaration-owned outer tuple before the variant node.
pub(crate) fn normalized_variant_expression_seed(
    owner: &RuntimeNormalizedType,
    ordinal: u32,
    raw_payload: Option<RuntimeExprSeed>,
) -> Result<RuntimeExprSeed, RuntimeNormalizedVariantSeedError> {
    let selection = owner.variant_selection(ordinal)?;
    let raw_type = selection.single_payload_item()?;
    let payload = match (selection.payload(), raw_type, raw_payload) {
        (Some(payload), Some(expected), Some(raw_payload)) => {
            if raw_payload.ty() != expected.identity() {
                return Err(RuntimeNormalizedVariantSeedError::PayloadType {
                    owner: selection.owner().identity(),
                    ordinal: selection.ordinal(),
                    expected: expected.identity(),
                    actual: raw_payload.ty(),
                });
            }
            Some(Box::new(RuntimeExprSeed::new(
                payload.identity(),
                RuntimeExprSeedKind::Tuple(Box::new([raw_payload])),
            )))
        }
        (None, None, None) => None,
        (Some(_), Some(_), None) | (None, None, Some(_)) => {
            return Err(RuntimeNormalizedVariantSeedError::PayloadPresence {
                owner: selection.owner().identity(),
                ordinal: selection.ordinal(),
            });
        }
        (Some(_), None, _) | (None, Some(_), _) => {
            return Err(RuntimeNormalizedVariantSeedError::InconsistentSelection {
                owner: selection.owner().identity(),
                ordinal: selection.ordinal(),
            });
        }
    };
    Ok(RuntimeExprSeed::new(
        selection.owner().identity(),
        RuntimeExprSeedKind::Variant {
            ordinal: selection.ordinal(),
            payload,
        },
    ))
}

/// Constructs one selected variant pattern whose optional binding targets the
/// raw unary item nested inside the exact structural tuple payload.
pub(crate) fn normalized_variant_binding_pattern_seed(
    owner: &RuntimeNormalizedType,
    ordinal: u32,
    binding: Option<RuntimeLocalSeedId>,
) -> Result<RuntimePatternSeed, RuntimeNormalizedVariantSeedError> {
    let selection = owner.variant_selection(ordinal)?;
    let raw_type = selection.single_payload_item()?;
    let payload = match (selection.payload(), raw_type, binding) {
        (Some(payload), Some(raw_type), Some(local)) => {
            let binding = RuntimePatternSeed::new(
                raw_type.identity(),
                RuntimePatternSeedKind::Bind {
                    local,
                    mutable: false,
                },
            );
            Some(Box::new(RuntimePatternSeed::new(
                payload.identity(),
                RuntimePatternSeedKind::Tuple(Box::new([binding])),
            )))
        }
        (None, None, None) => None,
        (Some(_), Some(_), None) | (None, None, Some(_)) => {
            return Err(RuntimeNormalizedVariantSeedError::PayloadPresence {
                owner: selection.owner().identity(),
                ordinal: selection.ordinal(),
            });
        }
        (Some(_), None, _) | (None, Some(_), _) => {
            return Err(RuntimeNormalizedVariantSeedError::InconsistentSelection {
                owner: selection.owner().identity(),
                ordinal: selection.ordinal(),
            });
        }
    };
    Ok(RuntimePatternSeed::new(
        selection.owner().identity(),
        RuntimePatternSeedKind::Variant {
            ordinal: selection.ordinal(),
            payload,
        },
    ))
}
