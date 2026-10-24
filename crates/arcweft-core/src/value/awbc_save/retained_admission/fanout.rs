//! Failure-atomic ordered fanout construction under the original program bank.

use super::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeFunctionEffectInstantiation,
    RuntimeRetainedValueAdmission, RuntimeSemanticTypeId, RuntimeValue,
    RuntimeValueOwnershipRequirement, RuntimeValueView, ValueAdmissionOrigin,
};
use crate::entry::schema::value_encoding::ValueCollectionKind;
use crate::pattern::{RuntimeBuiltinVariantCaseIdentity as Case, RuntimeVariantIdentity};
use crate::program_types::RuntimeFanoutOutputKind;
use crate::value::{
    LiveRuntimeValueView, RuntimeSequenceView, RuntimeStoredValue, RuntimeTupleView,
    RuntimeValueSource, RuntimeVariantView,
};

/// Exclusive custody keeps every original slot intact until all output and
/// consumer preflights finish. Only live owning carriers can be consumed.
pub(crate) struct PreparedFanout<'a, V> {
    slots: &'a mut [Option<V>],
    kind: RuntimeFanoutOutputKind,
    first_error: Option<usize>,
}

impl RuntimeRetainedValueAdmission {
    pub(crate) fn prepare_fanout<'a, V: RuntimeStoredValue + Into<RuntimeValue>>(
        &mut self,
        aggregate: RuntimeSemanticTypeId,
        child: RuntimeSemanticTypeId,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        slots: &'a mut [Option<V>],
    ) -> Result<PreparedFanout<'a, V>, AwbcRuntimeValueSnapshotError> {
        self.status()?;
        let result = (|| {
            self.validate_metadata_collection(slots.len())?;
            if !matches!(V::admission_origin(), ValueAdmissionOrigin::Live) {
                return Err(invalid("fanout construction requires live result custody"));
            }
            let owner =
                self.owner
                    .clone()
                    .ok_or(AwbcRuntimeValueSnapshotError::ProgramOwnerRequired {
                        expected: aggregate,
                    })?;
            let types = owner.types();
            let kind = types.fanout_output_kind(aggregate, child)?;
            let mut first_error = None;
            for (index, slot) in slots.iter().enumerate() {
                let value = slot
                    .as_ref()
                    .ok_or_else(|| invalid("fanout result is not ready"))?;
                // This only selects the prospective output depth. Every
                // header, binding and payload is checked against its original
                // child type below before any projection or consumption.
                if kind == RuntimeFanoutOutputKind::ResultSequence
                    && let RuntimeValueView::Variant(variant) = value.borrowed_view()
                    && *variant.owner() == RuntimeVariantIdentity::Builtin(Case::ResultErr.owner())
                    && Case::ResultErr
                        .owner()
                        .case_at(variant.ordinal())
                        .is_some_and(|case| case.identity() == Case::ResultErr)
                {
                    first_error.get_or_insert(index);
                }
            }
            let depth =
                usize::from(kind == RuntimeFanoutOutputKind::Sequence || first_error.is_none());
            self.resources.set_origin(ValueAdmissionOrigin::Live);
            for slot in slots.iter() {
                let value = slot
                    .as_ref()
                    .expect("all fanout slots were checked")
                    .borrowed_view();
                value.admit_ownership(
                    RuntimeValueOwnershipRequirement::Owned,
                    &mut self.resources,
                    depth,
                )?;
                types
                    .value_type(child)?
                    .validate_admitted_view(value, binding, &self.resources)?;
                AwbcRuntimeValueSnapshot::validate_admitted_storage_view(
                    value,
                    Some(&owner),
                    &self.resources,
                )?;
            }
            match kind {
                RuntimeFanoutOutputKind::Sequence => self.resources.collection_header(
                    ValueCollectionKind::Sequence,
                    slots.len(),
                    0,
                )?,
                RuntimeFanoutOutputKind::ResultSequence => {
                    self.resources.builtin_variant_header(
                        if first_error.is_some() {
                            Case::ResultErr
                        } else {
                            Case::ResultOk
                        },
                        0,
                    )?;
                    self.resources
                        .collection_header(ValueCollectionKind::Tuple, 1, 1)?;
                    if first_error.is_none() {
                        self.resources.collection_header(
                            ValueCollectionKind::Sequence,
                            slots.len(),
                            2,
                        )?;
                    }
                }
            }
            let prepared = PreparedFanout {
                slots: &mut *slots,
                kind,
                first_error,
            };
            prepared.with_view(|view| {
                types.value_type(aggregate)?.validate_admitted_view(
                    view.inspect(),
                    binding,
                    &self.resources,
                )
            })?;
            Ok((kind, first_error))
        })();
        if let Err(error) = &result {
            self.retain_failure(error.clone());
        }
        result.map(|(kind, first_error)| PreparedFanout {
            slots,
            kind,
            first_error,
        })
    }
}

impl<V: RuntimeStoredValue + Into<RuntimeValue>> PreparedFanout<'_, V> {
    /// Releases the exclusive construction proof without taking any input.
    /// Its admission work remains charged to the original bank.
    pub(crate) fn abort(self) {
        let Self { slots, .. } = self;
        let _ = slots;
    }

    pub(crate) fn with_view<R>(&self, visit: impl FnOnce(LiveRuntimeValueView<'_>) -> R) -> R {
        if let Some(index) = self.first_error {
            let payload = admitted_result_payload(
                self.slots[index]
                    .as_ref()
                    .expect("prepared error remains present")
                    .borrowed_view(),
            );
            return with_result_view(Case::ResultErr, payload, visit);
        }
        let mut values = Vec::with_capacity(self.slots.len());
        for slot in self.slots.iter() {
            let value = slot
                .as_ref()
                .expect("prepared fanout slot remains present")
                .borrowed_view();
            if self.kind == RuntimeFanoutOutputKind::Sequence {
                values.push(value);
            } else {
                values.push(admitted_result_payload(value));
            }
        }
        let sequence = RuntimeValueView::Sequence(RuntimeSequenceView::Views(&values));
        if self.kind == RuntimeFanoutOutputKind::Sequence {
            return visit(LiveRuntimeValueView::construction(sequence));
        }
        with_result_view(Case::ResultOk, sequence, visit)
    }

    /// No validation or value cloning remains after the exclusive preflight.
    pub(crate) fn commit(self) -> RuntimeValue {
        if let Some(index) = self.first_error {
            let value: RuntimeValue = self.slots[index]
                .take()
                .expect("prepared error remains present")
                .into();
            let (_, payload) = value
                .try_into_builtin_variant_case()
                .expect("prepared child is Result");
            for slot in self.slots.iter_mut() {
                drop(slot.take());
            }
            return RuntimeValue::result_err(payload.expect("Result has a payload"));
        }
        let mut values = Vec::with_capacity(self.slots.len());
        for slot in self.slots.iter_mut() {
            let value: RuntimeValue = slot
                .take()
                .expect("prepared fanout slot remains present")
                .into();
            if self.kind == RuntimeFanoutOutputKind::Sequence {
                values.push(value);
            } else {
                let (_, payload) = value
                    .try_into_builtin_variant_case()
                    .expect("prepared child is Result");
                let payload = payload.expect("Result has its unary payload");
                values.push(payload);
            }
        }
        let value = crate::value::runtime_sequence_values(values);
        match self.kind {
            RuntimeFanoutOutputKind::Sequence => value,
            RuntimeFanoutOutputKind::ResultSequence => RuntimeValue::result_ok(value),
        }
    }
}

/// Original type admission has proved this Result's canonical unary payload.
/// A scoped builtin may retain its genuine construction binding; inspecting
/// that admitted value does not impose an extra unscoped-literal restriction.
fn admitted_result_payload(value: RuntimeValueView<'_>) -> RuntimeValueView<'_> {
    let RuntimeValueView::Variant(variant) = value else {
        unreachable!("admitted Result is a variant")
    };
    let Some(RuntimeValueView::Tuple(fields)) = variant.payload() else {
        unreachable!("admitted Result owns its payload tuple")
    };
    fields
        .get(0)
        .expect("admitted Result has one payload field")
}

fn with_result_view<R>(
    case: Case,
    payload: RuntimeValueView<'_>,
    visit: impl FnOnce(LiveRuntimeValueView<'_>) -> R,
) -> R {
    let fields = [payload];
    let tuple = RuntimeValueView::Tuple(RuntimeTupleView::Views(&fields));
    let owner = RuntimeVariantIdentity::Builtin(case.owner());
    let (ordinal, schema) = case
        .owner()
        .resolve_case(case)
        .expect("builtin Result case");
    let payload = RuntimeValueSource::Borrowed {
        value: &tuple,
        origin: ValueAdmissionOrigin::Live,
    };
    visit(LiveRuntimeValueView::construction(
        RuntimeValueView::Variant(RuntimeVariantView::borrowed(
            &owner,
            ordinal,
            schema.name(),
            Some(payload),
        )),
    ))
}

fn invalid(message: &str) -> AwbcRuntimeValueSnapshotError {
    AwbcRuntimeValueSnapshotError::Message {
        message: message.to_owned(),
    }
}
