//! Borrowed defining-order nominal and variant headers.

use super::super::{
    RuntimeNominalRecordValue, RuntimeVariantValue,
    awbc_save::{AwbcRuntimeNominalRecordSnapshot, AwbcRuntimeValueSnapshot},
};
use super::{RuntimeTupleView, RuntimeValueSource, RuntimeValueView};
use crate::{
    entry::{RuntimeNominalTypeId, TypeLayoutHash, schema::value_encoding::ValueAdmissionOrigin},
    pattern::{RuntimeBuiltinVariantCaseIdentity, RuntimeSemanticTypeId, RuntimeVariantIdentity},
    program_types::RuntimeFunctionEffectInstantiation,
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeNominalRecordView<'a> {
    Live(&'a RuntimeNominalRecordValue),
    Snapshot(&'a AwbcRuntimeNominalRecordSnapshot),
}

impl<'a> RuntimeNominalRecordView<'a> {
    pub(crate) fn type_id(self) -> &'a RuntimeNominalTypeId {
        match self {
            Self::Live(value) => value.type_id(),
            Self::Snapshot(value) => &value.type_id,
        }
    }
    pub(crate) fn semantic_identity(self) -> RuntimeSemanticTypeId {
        match self {
            Self::Live(value) => value.semantic_identity(),
            Self::Snapshot(value) => value.semantic_identity,
        }
    }
    pub(crate) fn layout(self) -> TypeLayoutHash {
        match self {
            Self::Live(value) => value.layout(),
            Self::Snapshot(value) => value.layout,
        }
    }
    pub(crate) fn type_instantiation(self) -> Option<&'a RuntimeFunctionEffectInstantiation> {
        match self {
            Self::Live(value) => value.type_instantiation(),
            Self::Snapshot(value) => value.type_instantiation.as_ref(),
        }
    }
    pub(crate) fn fields(self) -> RuntimeTupleView<'a> {
        match self {
            Self::Live(value) => RuntimeTupleView::Values(value.fields()),
            Self::Snapshot(value) => RuntimeTupleView::NominalFields(&value.fields),
        }
    }
    pub(crate) fn field_id(self, ordinal: usize) -> Option<super::super::RuntimeRecordFieldId> {
        match self {
            Self::Live(value) => value.field_id(ordinal),
            Self::Snapshot(value) => value.fields.get(ordinal).map(|field| field.field),
        }
    }
    pub(super) const fn live_value(self) -> Option<&'a RuntimeNominalRecordValue> {
        match self {
            Self::Live(value) => Some(value),
            Self::Snapshot(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeVariantView<'a> {
    owner: &'a RuntimeVariantIdentity,
    ordinal: u32,
    name: &'a str,
    type_instantiation: Option<&'a RuntimeFunctionEffectInstantiation>,
    payload: Option<super::RuntimeValueSource<'a>>,
    live: Option<&'a RuntimeVariantValue>,
}

impl<'a> RuntimeVariantView<'a> {
    /// Projects a canonical one-field builtin case over a prospective live payload.
    pub(crate) fn with_live_builtin_payload<R>(
        case: RuntimeBuiltinVariantCaseIdentity,
        payload: RuntimeValueView<'_>,
        visit: impl FnOnce(RuntimeValueView<'_>) -> R,
    ) -> R {
        let fields = [payload];
        let tuple = RuntimeValueView::Tuple(RuntimeTupleView::Views(&fields));
        let owner = RuntimeVariantIdentity::Builtin(case.owner());
        let (ordinal, schema) = case
            .owner()
            .resolve_case(case)
            .expect("builtin variant case");
        debug_assert_eq!(schema.payload_arity(), Some(1));
        let payload = RuntimeValueSource::Borrowed {
            value: &tuple,
            origin: ValueAdmissionOrigin::Live,
        };
        visit(RuntimeValueView::Variant(RuntimeVariantView::borrowed(
            &owner,
            ordinal,
            schema.name(),
            Some(payload),
        )))
    }

    /// Borrowed construction preview. Payload provenance is retained and no
    /// owning carrier or Copy capability is issued by this projection.
    pub(crate) const fn borrowed(
        owner: &'a RuntimeVariantIdentity,
        ordinal: u32,
        name: &'a str,
        payload: Option<super::RuntimeValueSource<'a>>,
    ) -> Self {
        Self {
            owner,
            ordinal,
            name,
            type_instantiation: None,
            payload,
            live: None,
        }
    }

    pub(super) fn live(value: &'a RuntimeVariantValue) -> Self {
        Self {
            owner: value.owner(),
            ordinal: value.ordinal(),
            name: value.name(),
            type_instantiation: value.type_instantiation(),
            payload: value.payload().map(super::RuntimeValueSource::Live),
            live: Some(value),
        }
    }
    pub(super) fn snapshot(
        owner: &'a RuntimeVariantIdentity,
        ordinal: u32,
        name: &'a str,
        type_instantiation: Option<&'a RuntimeFunctionEffectInstantiation>,
        payload: Option<&'a AwbcRuntimeValueSnapshot>,
    ) -> Self {
        Self {
            owner,
            ordinal,
            name,
            type_instantiation,
            payload: payload.map(super::RuntimeValueSource::Snapshot),
            live: None,
        }
    }
    pub(crate) const fn owner(self) -> &'a RuntimeVariantIdentity {
        self.owner
    }
    pub(crate) const fn ordinal(self) -> u32 {
        self.ordinal
    }
    pub(crate) const fn name(self) -> &'a str {
        self.name
    }
    pub(crate) const fn type_instantiation(self) -> Option<&'a RuntimeFunctionEffectInstantiation> {
        self.type_instantiation
    }
    pub(crate) fn payload(self) -> Option<RuntimeValueView<'a>> {
        self.payload.map(super::RuntimeValueSource::view)
    }
    pub(super) const fn live_value(self) -> Option<&'a RuntimeVariantValue> {
        self.live
    }
}
