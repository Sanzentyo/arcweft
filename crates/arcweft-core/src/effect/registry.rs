//! Program-bound replay data for a typed lifetime-registry update.
//!
//! The static key is a selector, never a lease on an active scope. The driver
//! still resolves its actual destination and authorizes the write before
//! publishing this data. This carrier grants no line-handle or Need custody.

use arcweft_id::{EffectId, LifetimeRegistryKey};

use crate::entry::schema::value_encoding::ValueAdmissionOrigin;
use crate::pattern::RuntimeSemanticTypeId;
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::task::RuntimeProgramOwner;
use crate::value::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeBorrowedValue,
    RuntimeRetainedValueAdmission, RuntimeValue,
};

mod journal;
mod scope;
pub(crate) use journal::{RuntimeRegistryUpdateJournal, RuntimeRegistryUpdateJournalError};
pub(crate) use scope::{RuntimeRegistryOwner, RuntimeRegistryWriteSource, RuntimeScopeRegistry};

pub(crate) use scope::RuntimePreparedRegistryWrite;

/// Replayable update data admitted by the exact executable that produced it.
///
/// Values are deeply unrestricted and replay requires fresh admission. There is
/// no unchecked packet `Clone`. Active scope identity, event order, host
/// persistence and adapter cleanup remain obligations of the publishing driver.
///
/// ```compile_fail
/// use arcweft_core::effect::RuntimeRegistryStateUpdate;
/// fn duplicate(packet: &RuntimeRegistryStateUpdate) {
///     let _: RuntimeRegistryStateUpdate = packet.clone();
/// }
/// ```
#[derive(Debug)]
pub struct RuntimeRegistryStateUpdate {
    program: RuntimeProgramOwner,
    row: RuntimeRegistryValue,
}

/// Inert, version-one data inside the owning frame or an emitted update.
/// Restoring it requires validation under that frame's original executable.
#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeRegistryValue {
    key: LifetimeRegistryKey,
    expected: RuntimeSemanticTypeId,
    binding: Option<RuntimeFunctionEffectInstantiation>,
    value: AwbcRuntimeValueSnapshot,
}

impl RuntimeRegistryStateUpdate {
    /// Admits the whole borrowed key, original type and value before copying
    /// any payload. Multiple updates may share one retained admission bank.
    /// Affine values require their actual owning ledger and cannot enter this
    /// replay-data boundary through a borrowed copy.
    pub fn try_copy(
        key: &LifetimeRegistryKey,
        expected: RuntimeSemanticTypeId,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        value: &RuntimeValue,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        Self::prepare(key, expected, binding, admission)?.capture(value)
    }

    pub(crate) fn prepare<'source, 'budget>(
        key: &'source LifetimeRegistryKey,
        expected: RuntimeSemanticTypeId,
        binding: Option<&'source RuntimeFunctionEffectInstantiation>,
        admission: &'budget mut RuntimeRetainedValueAdmission,
    ) -> Result<PreparedRegistryValue<'source, 'budget>, AwbcRuntimeValueSnapshotError> {
        Self::admit_header(
            key,
            expected,
            binding,
            ValueAdmissionOrigin::Live,
            admission,
        )?;
        Ok(PreparedRegistryValue {
            key,
            expected,
            binding,
            admission,
        })
    }

    #[must_use]
    pub fn key(&self) -> &LifetimeRegistryKey {
        &self.row.key
    }

    #[must_use]
    pub const fn value_type(&self) -> RuntimeSemanticTypeId {
        self.row.expected
    }

    /// Read-only inert data, without a live-value or affine-copy entitlement.
    #[must_use]
    pub fn value(&self) -> RuntimeBorrowedValue<'_> {
        (&self.row.value).into()
    }

    #[must_use]
    pub fn required_effect(&self) -> EffectId {
        EffectId::state_write(self.key())
    }

    /// Materializes replay data only under its original executable and retained
    /// invocation. A lookalike program with equal semantic IDs is insufficient.
    /// Admission shares the caller's bank with all other applied updates.
    pub fn materialize(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        admission.status()?;
        let program =
            admission
                .program_owner()
                .ok_or(AwbcRuntimeValueSnapshotError::ProgramOwnerRequired {
                    expected: self.row.expected,
                });
        let result = match program {
            Ok(program) if program.same_program(&self.program) => Ok(()),
            Ok(_) => Err(AwbcRuntimeValueSnapshotError::ProgramOwnerMismatch {
                expected: self.row.expected,
            }),
            Err(error) => Err(error),
        };
        result.inspect_err(|error| admission.retain_failure(error.clone()))?;
        self.row.admit_retained(admission)?;
        self.row
            .value
            .clone()
            .into_admitted_runtime_value(&self.program)
            .inspect_err(|error| admission.retain_failure(error.clone()))
    }

    /// Version-one header: scope name, ordered key names, original semantic
    /// type, optional invocation context and ordered declaration effect rows.
    /// Strings and counts use the existing canonical grammar. The executable
    /// lease is supplied by the enclosing original-program context, not bytes.
    fn admit_header(
        key: &LifetimeRegistryKey,
        expected: RuntimeSemanticTypeId,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        origin: ValueAdmissionOrigin,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        admission.admit_composite(origin, |_, resources| {
            resources.descriptor_string(key.scope().as_str())?;
            resources.descriptor_count(key.segments().len())?;
            for segment in key.segments() {
                resources.descriptor_string(segment.as_str())?;
            }
            resources.descriptor_fixed(expected.as_bytes())?;
            resources.descriptor_fixed(&[u8::from(binding.is_some())])?;
            if let Some(binding) = binding {
                resources.function_instantiation(
                    binding,
                    crate::entry::schema::value_budget::ResourceByteKind::Value,
                )?;
            }
            Ok(())
        })
    }
}

pub(crate) struct PreparedRegistryValue<'source, 'budget> {
    key: &'source LifetimeRegistryKey,
    expected: RuntimeSemanticTypeId,
    binding: Option<&'source RuntimeFunctionEffectInstantiation>,
    admission: &'budget mut RuntimeRetainedValueAdmission,
}

impl PreparedRegistryValue<'_, '_> {
    pub(crate) fn capture(
        self,
        value: &RuntimeValue,
    ) -> Result<RuntimeRegistryStateUpdate, AwbcRuntimeValueSnapshotError> {
        let Self {
            key,
            expected,
            binding,
            admission,
        } = self;
        admission.validate_typed_in_invocation(value, expected, binding)?;
        let program = admission
            .program_owner()
            .ok_or(AwbcRuntimeValueSnapshotError::ProgramOwnerRequired { expected })?;
        let value = AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, program)
            .inspect_err(|error| admission.retain_failure(error.clone()))?;
        Ok(RuntimeRegistryStateUpdate {
            program: admission
                .program_owner()
                .expect("typed admission authenticated the original executable")
                .clone(),
            row: RuntimeRegistryValue {
                key: key.clone(),
                expected,
                binding: binding.cloned(),
                value,
            },
        })
    }
}

impl RuntimeRegistryValue {
    fn admit_retained(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        RuntimeRegistryStateUpdate::admit_header(
            &self.key,
            self.expected,
            self.binding.as_ref(),
            ValueAdmissionOrigin::InertSnapshot,
            admission,
        )?;
        admission.validate_typed_snapshot_in_invocation(
            &self.value,
            self.expected,
            self.binding.as_ref(),
        )
    }
}

#[cfg(test)]
mod tests;
