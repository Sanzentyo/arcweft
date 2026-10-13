//! Borrowed original frame authority for retained-input admission.

use super::{
    AwbcFunctionId, AwbcProgram, FiberFormatAttemptState, FiberFormatState, FiberScope,
    FiberScopeCleanup, FiberStateError,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::value::{RuntimePlaceStorage, RuntimeValue, RuntimeValueView};

pub(super) struct FrameValueContext<'a, V> {
    pub(super) program: &'a AwbcProgram,
    pub(super) function: AwbcFunctionId,
    pub(super) layout: &'a crate::awbc::schema::AwbcFrameLayout,
    pub(super) binding: Option<&'a RuntimeFunctionEffectInstantiation>,
    pub(super) registers: &'a [RuntimePlaceStorage<V>],
    pub(super) scope_count: usize,
}

impl<V> FrameValueContext<'_, V> {
    pub(super) fn validate_scopes(
        &self,
        root_cleanups: &[FiberScopeCleanup<V>],
        scopes: &[FiberScope<V>],
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_> + Copy,
    ) -> Result<(), FiberStateError> {
        for count in [root_cleanups.len(), scopes.len()] {
            resources
                .work()
                .collection(count)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        }
        for cleanup in root_cleanups {
            cleanup.validate_inputs(self, resources, view)?;
        }
        let mut ids = std::collections::BTreeSet::new();
        for (ordinal, scope) in scopes.iter().enumerate() {
            scope.validate_inputs(
                self,
                ordinal,
                ordinal.checked_sub(1).map(|index| scopes[index].id),
                &mut ids,
                resources,
                view,
            )?;
        }
        // A retained packet must belong to the lexical scope at its original
        // reached instruction, including after later scopes have been entered.
        self.program
            .verify_scope_stacks(
                root_cleanups
                    .iter()
                    .map(|cleanup| (cleanup.source, &scopes[..0]))
                    .chain(scopes.iter().enumerate().flat_map(|(ordinal, scope)| {
                        scope
                            .defers
                            .iter()
                            .map(move |deferred| (deferred.source, &scopes[..=ordinal]))
                            .chain(
                                scope
                                    .cleanups
                                    .iter()
                                    .map(move |cleanup| (cleanup.source, &scopes[..=ordinal])),
                            )
                    })),
            )
            .map_err(|_| FiberStateError::InvalidFrame)?;
        Ok(())
    }

    pub(super) fn validate_formats(
        &self,
        format: Option<&FiberFormatState<V>>,
        attempts: &[FiberFormatAttemptState<V>],
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_> + Copy,
    ) -> Result<(), FiberStateError> {
        resources
            .work()
            .collection(attempts.len())
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if let Some(format) = format {
            format.validate_inputs(self, resources, view)?;
        }
        for attempt in attempts {
            attempt.validate_inputs(self, resources, view)?;
        }
        Ok(())
    }
}

impl super::FiberFrame {
    pub(in crate::awbc) fn cancel_cleanup_by_key(
        &mut self,
        program: &AwbcProgram,
        key: &str,
    ) -> Result<(), FiberStateError> {
        let mut resources =
            ValueResourceAdmission::live(crate::entry::RuntimeSchemaLimits::engine_default());
        let context = FrameValueContext {
            program,
            function: self.function,
            layout: super::validate_frame_layout(
                program,
                self.function,
                self.layout,
                self.type_instantiation.as_deref(),
                self.registers.len(),
                &resources.work(),
            )?,
            binding: self.type_instantiation.as_deref(),
            registers: &self.registers,
            scope_count: self.scopes.len(),
        };
        context.validate_scopes(
            &self.root_cleanups,
            &self.scopes,
            &mut resources,
            RuntimeValue::view,
        )?;
        // Every key is resolved before mutation, so the following retains
        // cannot fail after removing an earlier registration.
        let root = self
            .root_cleanups
            .iter()
            .map(|cleanup| cleanup.key(program).map(|candidate| candidate != key))
            .collect::<Result<Vec<_>, _>>()?;
        let scopes = self
            .scopes
            .iter()
            .map(|scope| {
                scope
                    .cleanups
                    .iter()
                    .map(|cleanup| cleanup.key(program).map(|candidate| candidate != key))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut root = root.into_iter();
        self.root_cleanups
            .retain(|_| root.next().expect("admitted root cleanup inventory"));
        for (scope, keep) in self.scopes.iter_mut().zip(scopes) {
            let mut keep = keep.into_iter();
            scope
                .cleanups
                .retain(|_| keep.next().expect("admitted lexical cleanup inventory"));
        }
        Ok(())
    }
    /// Transfers one instruction's input sequence after the complete original
    /// layout/value/Copy admission. Repeated Copy registers supply prepared
    /// copies and one final move; an affine register can occur only once.
    pub(in crate::awbc) fn take_register_values(
        &mut self,
        program: &AwbcProgram,
        registers: &[super::AwbcRegisterId],
    ) -> Result<Vec<RuntimeValue>, FiberStateError> {
        enum Operand {
            Copy(RuntimeValue),
            Move(super::AwbcRegisterId),
        }
        let mut resources =
            ValueResourceAdmission::live(crate::entry::RuntimeSchemaLimits::engine_default());
        resources
            .work()
            .collection(registers.len())
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        let layout = super::validate_frame_layout(
            program,
            self.function,
            self.layout,
            self.type_instantiation.as_deref(),
            self.registers.len(),
            &resources.work(),
        )?;
        let mut occurrences = std::collections::BTreeMap::new();
        for register in registers {
            let slot = layout
                .slots
                .get(register.index())
                .ok_or(FiberStateError::InvalidFrame)?;
            let value = self.register(*register)?;
            crate::program_types::RuntimeProgramValueType::Awbc {
                program,
                ty: slot.ty,
            }
            .validate_view(
                value.view(),
                self.type_instantiation.as_deref(),
                &mut resources,
            )
            .map_err(|error| {
                FiberStateError::Snapshot(crate::value::AwbcRuntimeValueSnapshotError::ProgramType(
                    Box::new(error),
                ))
            })?;
            let count = occurrences.entry(*register).or_insert(0usize);
            *count += 1;
            if *count > 1 && !value.ownership().permits_copy() {
                return Err(FiberStateError::Snapshot(
                    crate::value::AwbcRuntimeValueSnapshotError::AffineValue,
                ));
            }
        }
        // Prepare all copies while every source is still present. A failed
        // Copy proof cannot follow an earlier register move.
        let mut prepared = Vec::with_capacity(registers.len());
        for register in registers {
            let remaining = occurrences
                .get_mut(register)
                .expect("admitted input has an occurrence");
            *remaining -= 1;
            prepared.push(if *remaining == 0 {
                Operand::Move(*register)
            } else {
                Operand::Copy(
                    self.register(*register)?
                        .live_view()
                        .copy_unrestricted()
                        .ok_or(FiberStateError::Snapshot(
                            crate::value::AwbcRuntimeValueSnapshotError::AffineValue,
                        ))?,
                )
            });
        }
        // Each admitted source has exactly one Move. The exclusive frame
        // borrow prevents changes between admission, preparation and commit.
        Ok(prepared
            .into_iter()
            .map(|operand| match operand {
                Operand::Copy(value) => value,
                Operand::Move(register) => self
                    .take_register(register)
                    .expect("admitted source is present at its unique transfer commit"),
            })
            .collect())
    }
}
