//! Borrowed defining-order nominal and variant headers.

use super::super::{
    RuntimeNominalRecordValue, RuntimeVariantValue,
    awbc_save::{AwbcRuntimeNominalRecordSnapshot, AwbcRuntimeValueSnapshot},
};
use super::{RuntimeTupleView, RuntimeValueView};
use crate::{
    entry::{RuntimeNominalTypeId, TypeLayoutHash},
    pattern::{RuntimeSemanticTypeId, RuntimeVariantIdentity},
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
