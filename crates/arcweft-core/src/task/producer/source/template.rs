//! Original capture and fanout argument grammar, before retaining a Source.

use super::{
    RuntimeTaskProducerSourceError, RuntimeTaskSource, RuntimeTaskSourceProgram,
    RuntimeTaskSourceRef,
};
use crate::plan::FlowOp;
use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
use crate::plan::body_semantic::function::EndpointKind;
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::task::NeedProducerSpec;
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission, RuntimeStoredValue,
    RuntimeTupleView, RuntimeValueSource, RuntimeValueView,
};

/// These are borrowed logical operands, not an already copied request or an
/// untyped caller-built tuple. Each role is admitted against its original AST.
pub(crate) enum RuntimeTaskTemplateArguments<'value, V> {
    Thread {
        captures: &'value [V],
    },
    AwaitManyBase {
        captures: &'value [V],
        source: RuntimeValueSource<'value>,
    },
    AwaitManyChild {
        captures: &'value [V],
        index: u32,
        item: RuntimeValueSource<'value>,
    },
}

impl<'value, V: RuntimeStoredValue> RuntimeTaskTemplateArguments<'value, V> {
    fn captures(&self) -> &'value [V] {
        match self {
            Self::Thread { captures }
            | Self::AwaitManyBase { captures, .. }
            | Self::AwaitManyChild { captures, .. } => captures,
        }
    }

    fn capture_locals(
        &self,
        target: &crate::task::AwaitManyTarget,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<Vec<crate::runtime_id::RuntimeLocalDeclarationId>, RuntimeTaskProducerSourceError>
    {
        admission.validate_metadata_collection(target.base.request.captures.len())?;
        admission.validate_metadata_collection(target.child.request.captures.len())?;
        let mut locals = Vec::new();
        target.try_visit_captures(|local| {
            admission.admit_composite(V::admission_origin(), |_, resources| {
                let next = locals.len().checked_add(1).ok_or(
                    crate::entry::RuntimeSchemaError::Encoding {
                        message: "capture count overflow".into(),
                    },
                )?;
                resources.work().collection(next)?;
                resources.work().charge(0)?;
                Ok(())
            })?;
            locals.push(local);
            Ok::<(), RuntimeTaskProducerSourceError>(())
        })?;
        if locals.len() != self.captures().len() {
            return Err(RuntimeTaskProducerSourceError::CaptureCount {
                actual: self.captures().len(),
                expected: locals.len(),
            });
        }
        Ok(locals)
    }
}

impl RuntimeTaskSourceRef<'_> {
    /// Checks a retained handle against the original template and admitted
    /// operand identity without cloning a request or constructing a `TaskSpec`.
    pub(crate) fn validate_template_handle(
        &self,
        checked: &NeedProducerSpec<RuntimeTaskSource>,
        handle: crate::value::RuntimeNeedView<'_>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), RuntimeTaskProducerSourceError> {
        admission.status()?;
        let Some(owner) = admission.program_owner() else {
            return Err(RuntimeTaskProducerSourceError::OriginalProgram);
        };
        if !checked.source().program().owns_source_for(self, owner)
            || self.task_plan() != checked.source().task_plan()
        {
            return Err(RuntimeTaskProducerSourceError::OriginalProgram);
        }
        let Some(RuntimeBodyTaskSource::Producer(template)) = self.request_endpoint().task_source()
        else {
            return Err(RuntimeTaskProducerSourceError::RequestKind);
        };
        let definition = &template.definition;
        if handle.producer() != &crate::task::NeedProducerInstance::try_from(checked)?
            || handle.class() != &definition.class
            || handle.priority() != definition.priority
            || handle.cancel_scope() != &definition.cancel_scope
            || handle.policy() != definition.policy
            || handle.outcome() != &definition.outcome()
        {
            return Err(RuntimeTaskProducerSourceError::Specification);
        }
        Ok(())
    }

    /// Materializes only the original template's scheduling metadata after its
    /// complete operand admission. An equal digest from another lease cannot
    /// supply either the source endpoint or a caller-selected task policy.
    pub(crate) fn template_task_spec(
        &self,
        checked: &NeedProducerSpec<RuntimeTaskSource>,
        generation: crate::task::GenerationId,
        request: crate::task::HostTaskRequest,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<crate::task::TaskSpec, RuntimeTaskProducerSourceError> {
        admission.status()?;
        let Some(owner) = admission.program_owner() else {
            return Err(RuntimeTaskProducerSourceError::OriginalProgram);
        };
        if !checked.source().program().owns_source_for(self, owner)
            || self.task_plan() != checked.source().task_plan()
        {
            return Err(RuntimeTaskProducerSourceError::OriginalProgram);
        }
        let Some(RuntimeBodyTaskSource::Producer(template)) = self.request_endpoint().task_source()
        else {
            return Err(RuntimeTaskProducerSourceError::RequestKind);
        };
        let definition = &template.definition;
        admission.validate_metadata_string(&definition.debug_label)?;
        let spec = crate::task::TaskSpec {
            generation,
            producer: crate::task::NeedProducerInstance::try_from(checked)?,
            class: definition.class.clone(),
            priority: definition.priority,
            cancel_scope: definition.cancel_scope.clone(),
            policy: definition.policy,
            outcome: definition.outcome(),
            request: request.into(),
            debug_label: definition.debug_label.clone(),
        };
        spec.validate_outcome()?;
        Ok(spec)
    }

    /// Validates the complete capture grammar and actual source/item types on
    /// the caller's existing bank. Borrowed views hash the existing tuple ABI;
    /// no argument value, request definition or lease is copied before success.
    pub(crate) fn template_producer_spec<V: RuntimeStoredValue>(
        &self,
        program: &RuntimeTaskSourceProgram,
        arguments: RuntimeTaskTemplateArguments<'_, V>,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<NeedProducerSpec<RuntimeTaskSource>, RuntimeTaskProducerSourceError> {
        self.template_producer_spec_with_admission(program, arguments, binding, admission, false)
    }

    /// The enclosing retained grammar has already charged each value and
    /// checked its custody. Original type relations, metadata, binding and
    /// Source identity still run on the very same allowance.
    pub(crate) fn template_producer_spec_after_value_admission<V: RuntimeStoredValue>(
        &self,
        program: &RuntimeTaskSourceProgram,
        arguments: RuntimeTaskTemplateArguments<'_, V>,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<NeedProducerSpec<RuntimeTaskSource>, RuntimeTaskProducerSourceError> {
        self.template_producer_spec_with_admission(program, arguments, binding, admission, true)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one closed Thread/AwaitMany input algebra preserves preflight, ownership, type and canonical argument order"
    )]
    fn template_producer_spec_with_admission<V: RuntimeStoredValue>(
        &self,
        program: &RuntimeTaskSourceProgram,
        arguments: RuntimeTaskTemplateArguments<'_, V>,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        admission: &mut RuntimeRetainedValueAdmission,
        values_admitted: bool,
    ) -> Result<NeedProducerSpec<RuntimeTaskSource>, RuntimeTaskProducerSourceError> {
        admission.status()?;
        let captures = arguments.captures();
        admission.validate_metadata_collection(captures.len())?;
        let task_source = self
            .request_endpoint()
            .task_source()
            .ok_or(RuntimeTaskProducerSourceError::RequestKind)?;
        let producer = task_source
            .producer_definition()
            .ok_or(RuntimeTaskProducerSourceError::RequestKind)?;
        if !admission
            .program_owner()
            .is_some_and(|owner| program.owns_source_for(self, owner))
        {
            return Err(RuntimeTaskProducerSourceError::OriginalProgram);
        }
        self.admit_invocation_binding(program, binding, admission)?;
        let mut locals = Vec::new();
        match (self.request_endpoint().operation(), &arguments) {
            (
                Some(FlowOp::Thread { captures, .. }),
                RuntimeTaskTemplateArguments::Thread { .. },
            ) => {
                admission.validate_metadata_collection(captures.len())?;
                locals.extend_from_slice(captures.as_slice());
            }
            (
                Some(FlowOp::AwaitMany { target, .. }),
                RuntimeTaskTemplateArguments::AwaitManyBase { .. },
            ) if self.request_endpoint().kind() == EndpointKind::AwaitManyBase => {
                locals = arguments.capture_locals(&target, admission)?;
            }
            (
                Some(FlowOp::AwaitMany { target, .. }),
                RuntimeTaskTemplateArguments::AwaitManyChild { .. },
            ) if self.request_endpoint().kind() == EndpointKind::AwaitManyChild => {
                locals = arguments.capture_locals(&target, admission)?;
            }
            _ => return Err(RuntimeTaskProducerSourceError::RequestKind),
        }
        if locals.len() != captures.len() {
            return Err(RuntimeTaskProducerSourceError::CaptureCount {
                actual: captures.len(),
                expected: locals.len(),
            });
        }
        admission.validate_metadata(&(
            producer.family,
            producer.contract,
            producer.producer_site,
            producer.handle_type,
            &producer.class,
            producer.priority,
            &producer.cancel_scope,
            producer.policy,
        ))?;
        if let RuntimeBodyTaskSource::Producer(template) = task_source {
            admission.validate_metadata(&(
                &template.request.capability,
                &template.request.operation,
                &template.definition.debug_label,
            ))?;
        }
        // Inspect only the source container header before any capture value is
        // projected. Children stay borrowed until its complete role preflight.
        let source_view =
            if let RuntimeTaskTemplateArguments::AwaitManyBase { source, .. } = &arguments {
                let view = source.view();
                let count = match view {
                    RuntimeValueView::Tuple(items) => items.len(),
                    RuntimeValueView::Sequence(items) => items.len().map_err(|error| {
                        let error = AwbcRuntimeValueSnapshotError::from(error);
                        admission.retain_failure(error.clone());
                        error
                    })?,
                    _ => return Err(RuntimeTaskProducerSourceError::SourceShape),
                };
                if u32::try_from(count).is_err() {
                    return Err(RuntimeTaskProducerSourceError::SourceCount { actual: count });
                }
                admission.validate_metadata_collection(count)?;
                Some((view, count))
            } else {
                None
            };
        for (local, value) in locals.iter().zip(captures) {
            let ty = program
                .original()
                .local_declarations()
                .get(*local)
                .expect("the common seal admitted the original capture")
                .ty();
            let identity = program
                .original()
                .type_table()
                .get(ty)
                .expect("the common seal admitted the capture type")
                .semantic_identity();
            // Captures enter both thread frames and request kernels by Copy.
            if values_admitted {
                admission.validate_admitted_typed_borrowed_in_invocation(
                    value.borrowed_value(),
                    identity,
                    binding,
                )?;
            } else {
                admission.validate_typed_borrowed_in_invocation(
                    value.borrowed_value(),
                    identity,
                    binding,
                )?;
            }
        }
        let captured: Vec<_> = captures.iter().map(V::borrowed_view).collect();
        let captured = RuntimeValueView::Tuple(RuntimeTupleView::Views(&captured));
        let max_bytes = admission.work().limits().platform_encoded_bytes();
        let digest = match arguments {
            RuntimeTaskTemplateArguments::Thread { .. } => {
                crate::entry::schema::canonical_runtime_value_view_digest(captured, max_bytes)
            }
            RuntimeTaskTemplateArguments::AwaitManyBase { source, .. } => {
                let Some(FlowOp::AwaitMany { target, .. }) = self.request_endpoint().operation()
                else {
                    unreachable!("the complete operand grammar was checked")
                };
                let (view, count) = source_view.expect("the source header was preflighted");
                let ty = program
                    .original()
                    .type_table()
                    .get(target.source.ty())
                    .expect("the common seal admitted the original source container")
                    .semantic_identity();
                // Fanout takes custody of its evaluated source; this grants no Copy.
                if values_admitted {
                    admission.validate_admitted_typed_borrowed_in_invocation(
                        source.borrowed_value(),
                        ty,
                        binding,
                    )?;
                } else {
                    admission.validate_owned_typed_borrowed_in_invocation(
                        source.borrowed_value(),
                        ty,
                        binding,
                    )?;
                }
                let source: Vec<_> = match view {
                    RuntimeValueView::Tuple(items) => items.iter().collect(),
                    RuntimeValueView::Sequence(items) => (0..count)
                        .map(|index| {
                            items
                                .value_view(index)
                                .expect("admitted sequence owns every indexed item")
                        })
                        .collect(),
                    _ => unreachable!("source shape was checked"),
                };
                let fields = [
                    captured,
                    RuntimeValueView::Tuple(RuntimeTupleView::Views(&source)),
                ];
                crate::entry::schema::canonical_runtime_value_view_digest(
                    RuntimeValueView::Tuple(RuntimeTupleView::Views(&fields)),
                    max_bytes,
                )
            }
            RuntimeTaskTemplateArguments::AwaitManyChild { index, item, .. } => {
                let Some(FlowOp::AwaitMany { target, .. }) = self.request_endpoint().operation()
                else {
                    unreachable!("the complete operand grammar was checked")
                };
                admission.validate_metadata(&index)?;
                let ty = program
                    .original()
                    .local_declarations()
                    .get(target.item_binding)
                    .expect("the common seal admitted the original item local")
                    .ty();
                let identity = program
                    .original()
                    .type_table()
                    .get(ty)
                    .expect("the common seal admitted the original item type")
                    .semantic_identity();
                if values_admitted {
                    admission.validate_admitted_typed_borrowed_in_invocation(
                        item.borrowed_value(),
                        identity,
                        binding,
                    )?;
                } else {
                    admission.validate_owned_typed_borrowed_in_invocation(
                        item.borrowed_value(),
                        identity,
                        binding,
                    )?;
                }
                let index = RuntimeValueView::Scalar(crate::value::RuntimeScalarView::UInt(
                    crate::value::RuntimeUInt::U32(index),
                ));
                let fields = [captured, index, item.view()];
                crate::entry::schema::canonical_runtime_value_view_digest(
                    RuntimeValueView::Tuple(RuntimeTupleView::Views(&fields)),
                    max_bytes,
                )
            }
        }
        .map_err(|error| {
            let error = AwbcRuntimeValueSnapshotError::from(error);
            admission.retain_failure(error.clone());
            RuntimeTaskProducerSourceError::Admission(error)
        })?;
        let source = RuntimeTaskSource::from_borrowed(
            self,
            program,
            admission
                .program_owner()
                .ok_or(RuntimeTaskProducerSourceError::OriginalProgram)?,
        )
        .ok_or(RuntimeTaskProducerSourceError::OriginalProgram)?;
        Ok(NeedProducerSpec {
            family: source.task_plan().definition().family(),
            contract: producer.contract,
            plan: source,
            producer_site: producer.producer_site,
            handle_type: producer.handle_type,
            arguments: digest,
            binding: binding.cloned(),
        })
    }
}
