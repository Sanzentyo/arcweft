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

impl<'value, V> FrameValueContext<'value, V> {
    /// Register types and bindings belong to this same original frame.
    pub(super) fn validate_registers(
        &self,
        copy_proofs: &[(super::AwbcRegisterId, crate::awbc::verify::CopyProof)],
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&'value V) -> RuntimeValueView<'value>,
        path: &str,
    ) -> Result<(), crate::program_types::RuntimeProgramTypeError> {
        for (index, (slot, storage)) in self.layout.slots.iter().zip(self.registers).enumerate() {
            let proof = copy_proofs
                .binary_search_by_key(&index, |(register, _)| register.index())
                .ok()
                .map(|index| &copy_proofs[index].1);
            self.program.runtime_types.get(slot.ty.index()).ok_or(
                crate::awbc::value_admission::AwbcValueAdmissionError::UnknownType { ty: slot.ty },
            )?;
            storage
                .validate_program_type(
                    proof.map_or(
                        crate::value::ownership::RuntimeValueOwnershipRequirement::Owned,
                        crate::awbc::verify::CopyProof::ownership_requirement,
                    ),
                    crate::program_types::RuntimeProgramValueType::Awbc {
                        program: self.program,
                        ty: slot.ty,
                    },
                    self.binding,
                    resources,
                    &view,
                )
                .map_err(|error| error.within_value_path(&format!("{path}.registers[{index}]")))?;
            if let Some(proof) = proof
                && !matches!(proof, crate::awbc::verify::CopyProof::Copyable)
            {
                proof
                    .validate_storage(storage, &resources.work(), &view, 0)
                    .map_err(|source| {
                        crate::program_types::RuntimeProgramTypeError::from(
                            crate::awbc::value_admission::AwbcValueAdmissionError::Value {
                                ty: slot.ty,
                                source,
                            },
                        )
                        .within_value_path(&format!("{path}.registers[{index}]"))
                    })?;
            }
        }
        Ok(())
    }
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
    pub(crate) fn take_program_fanout_inputs(
        &mut self,
        program: &super::super::product_step::AwbcProductProgram,
        task: super::AwbcTaskPlanId,
        source_register: super::AwbcRegisterId,
        binding: Option<super::AwbcPatternId>,
    ) -> Result<super::FiberProgramFanoutState, FiberStateError> {
        use crate::awbc::schema::{
            AwbcProgramFanoutInput, AwbcProgramTaskKernel, AwbcTaskPlanExecution,
        };
        let original = program
            .original_task_source()
            .ok_or(FiberStateError::InvalidFrame)?;
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            original.program_owner(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let limits = crate::plan::RuntimeTaskPlanSealLimits::default();
        let source_error = |error: crate::plan::task_source::RuntimeAwbcProgramSourceError| {
            FiberStateError::Snapshot(crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: error.to_string(),
            })
        };
        let source = original
            .awbc_program_source(task, limits, &mut admission.task_source_meter(limits)?)
            .map_err(source_error)?;
        let row = program
            .task_plans
            .get(task.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let AwbcTaskPlanExecution::Program(body) = &row.execution else {
            return Err(FiberStateError::InvalidFrame);
        };
        let AwbcProgramTaskKernel::Fanout {
            worker,
            source: source_position,
            limit,
            ..
        } = &body.body
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        if body.kernel != self.function || source_register.0 != *source_position {
            return Err(FiberStateError::InvalidFrame);
        }
        super::validate_frame_layout(
            program,
            self.function,
            self.layout,
            self.type_instantiation.as_deref(),
            self.registers.len(),
            &admission.work(),
        )?;
        let worker_source = original
            .awbc_program_source(*worker, limits, &mut admission.task_source_meter(limits)?)
            .map_err(source_error)?;
        let values = (0..body.capture_modes.len())
            .map(|position| {
                self.register(super::AwbcRegisterId(
                    u32::try_from(position).map_err(|_| FiberStateError::InvalidFrame)?,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let checked = source
            .template_producer_spec(
                original,
                crate::task::RuntimeTaskTemplateArguments::ProgramTask { captures: &values },
                self.type_instantiation.as_deref(),
                &mut admission,
            )
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        admission.validate_metadata(&checked.source().assertion())?;
        let worker_template = worker_source
            .request_endpoint()
            .program_template()
            .ok_or(FiberStateError::InvalidFrame)?;
        admission.validate_metadata_string(&worker_template.producer().debug_label)?;
        let RuntimeValue::UInt(crate::value::RuntimeUInt::U32(limit_value)) =
            self.register(super::AwbcRegisterId(*limit))?
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        let limit_value = *limit_value;
        if limit_value == 0 {
            return Err(FiberStateError::InvalidFrame);
        }
        let RuntimeValue::Seq(sequence) = self.register(source_register)? else {
            return Err(FiberStateError::InvalidFrame);
        };
        let count = sequence.len();
        u32::try_from(count).map_err(|_| FiberStateError::InvalidFrame)?;
        admission.validate_metadata_collection(count)?;
        admission.validate_metadata_collection(body.capture_modes.len())?;
        let results = vec![None; count];
        let RuntimeValue::Seq(sequence) = self.take_register(source_register)? else {
            unreachable!("owned sequence was preflighted before any input movement")
        };
        Ok(super::FiberProgramFanoutState {
            plan: task,
            binding,
            limit: limit_value,
            items: sequence.into_values().into_iter().map(Some).collect(),
            next_index: 0,
            in_flight: Vec::new(),
            results,
        })
    }
}

impl super::FiberFrame {
    /// Admits the complete `AwaitMany` source and capture batch before preparing
    /// any capture copy or consuming the original sequence register.
    pub(in crate::awbc) fn take_await_many_inputs(
        &mut self,
        program: &crate::awbc::product_step::AwbcProductProgram,
        plan: super::AwbcTaskPlanId,
        source: super::AwbcRegisterId,
    ) -> Result<(Vec<RuntimeValue>, crate::value::RuntimeSeq), FiberStateError> {
        let original = program
            .original_task_source()
            .ok_or(FiberStateError::InvalidFrame)?;
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            original.program_owner(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let limits = crate::plan::RuntimeTaskPlanSealLimits::default();
        let mut meter = admission.task_source_meter(limits)?;
        let producer_source = original
            .awbc_template_source(plan, limits, &mut meter)
            .map_err(|error| {
                FiberStateError::Snapshot(crate::value::AwbcRuntimeValueSnapshotError::Message {
                    message: error.to_string(),
                })
            })?;
        let layout = super::validate_frame_layout(
            program,
            self.function,
            self.layout,
            self.type_instantiation.as_deref(),
            self.registers.len(),
            &admission.work(),
        )?;
        let plan = program
            .task_plans
            .get(plan.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let host = plan.host().ok_or(FiberStateError::InvalidFrame)?;
        let crate::awbc::schema::AwbcHostTaskPlanKind::AwaitMany {
            captures,
            request_function,
            ..
        } = &host.kind
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        admission.validate_metadata_collection(captures.len())?;
        let signature = program
            .functions
            .get(request_function.index())
            .and_then(|f| program.signatures.get(f.signature.index()))
            .ok_or(FiberStateError::InvalidFrame)?;
        if captures.len() != signature.params.len() {
            return Err(FiberStateError::InvalidFrame);
        }
        let values = captures
            .iter()
            .map(|register| self.register(*register))
            .collect::<Result<Vec<_>, _>>()?;
        let value = self.register(source)?;
        producer_source
            .template_producer_spec(
                original,
                crate::task::RuntimeTaskTemplateArguments::AwaitManyBase {
                    captures: &values,
                    source: crate::value::RuntimeValueSource::Live(value),
                },
                self.type_instantiation.as_deref(),
                &mut admission,
            )
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if !value.ownership().permits_copy() {
            return Err(FiberStateError::InvalidFrame);
        }
        admission.admit_composite(
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live,
            |_, resources| {
                for ((register, expected), value) in
                    captures.iter().zip(&signature.params).zip(&values)
                {
                    for ty in [layout.slots[register.index()].ty, *expected] {
                        crate::program_types::RuntimeProgramValueType::Awbc { program, ty }
                            .validate_admitted_view(
                                value.view(),
                                self.type_instantiation.as_deref(),
                                resources,
                            )
                            .map_err(|error| {
                                crate::value::AwbcRuntimeValueSnapshotError::ProgramType(Box::new(
                                    error,
                                ))
                            })?;
                    }
                }
                crate::program_types::RuntimeProgramValueType::Awbc {
                    program,
                    ty: layout.slots[source.index()].ty,
                }
                .validate_admitted_view(value.view(), self.type_instantiation.as_deref(), resources)
                .map_err(|error| {
                    crate::value::AwbcRuntimeValueSnapshotError::ProgramType(Box::new(error))
                })
            },
        )?;
        let RuntimeValue::Seq(sequence) = value else {
            return Err(FiberStateError::InvalidFrame);
        };
        u32::try_from(sequence.len()).map_err(|_| FiberStateError::InvalidFrame)?;
        let captured = captures
            .iter()
            .map(|register| {
                self.register(*register)
                    .expect("admitted capture register")
                    .live_view()
                    .copy_unrestricted()
                    .expect("admitted recursive Copy capture")
            })
            .collect();
        let RuntimeValue::Seq(sequence) = self
            .take_register(source)
            .expect("admitted source register")
        else {
            unreachable!("admitted sequence source");
        };
        Ok((captured, sequence))
    }

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

impl<V> FrameValueContext<'_, V> {
    pub(super) fn validate_await_many_site(
        &self,
        expected_plan: super::AwbcTaskPlanId,
        expected_binding: Option<super::AwbcPatternId>,
        cursor: super::FiberCursor,
        resume: super::FiberResumeTarget,
        resources: &mut ValueResourceAdmission,
    ) -> Result<(), FiberStateError> {
        resources
            .work()
            .charge(0)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        let super::FiberResumeTarget::Declared(resume) = resume else {
            return Err(FiberStateError::InvalidFrame);
        };
        let point = self
            .program
            .resume_points
            .get(resume.index())
            .ok_or(FiberStateError::UnknownResumePoint(resume.0))?;
        let function = self
            .program
            .functions
            .get(self.function.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        if point.function != self.function
            || point.frame_layout != function.frame_layout
            || cursor.function != self.function
        {
            return Err(FiberStateError::InvalidFrame);
        }
        resources
            .work()
            .charge(0)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        let block = self
            .program
            .blocks
            .get(cursor.block.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let crate::awbc::schema::AwbcTerminator::AwaitMany {
            plan,
            binding,
            resume: declared_resume,
            ..
        } = block.terminator
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        if block.owner != self.function
            || cursor.instruction_offset != block.instructions.len
            || plan != expected_plan
            || binding != expected_binding
            || declared_resume != resume
        {
            return Err(FiberStateError::InvalidFrame);
        }
        Ok(())
    }
}
