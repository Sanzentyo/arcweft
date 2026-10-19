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
    /// Validates the complete capture grammar and actual source/item types on
    /// the caller's existing bank. Borrowed views hash the existing tuple ABI;
    /// no argument value, request definition or lease is copied before success.
    #[expect(
        clippy::too_many_lines,
        reason = "one closed Thread/AwaitMany input algebra preserves preflight, ownership, type and canonical argument order"
    )]
    pub(crate) fn template_producer_spec<V: RuntimeStoredValue>(
        &self,
        program: &RuntimeTaskSourceProgram,
        arguments: RuntimeTaskTemplateArguments<'_, V>,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<NeedProducerSpec<RuntimeTaskSource>, RuntimeTaskProducerSourceError> {
        admission.status()?;
        let captures = arguments.captures();
        admission.validate_metadata_collection(captures.len())?;
        let Some(RuntimeBodyTaskSource::Producer(producer)) = self.request_endpoint().task_source()
        else {
            return Err(RuntimeTaskProducerSourceError::RequestKind);
        };
        if !admission
            .program_owner()
            .is_some_and(|owner| program.owns_source_for(self, owner))
        {
            return Err(RuntimeTaskProducerSourceError::OriginalProgram);
        }
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
            producer.payload_type,
            &producer.class,
            producer.priority,
            &producer.cancel_scope,
            producer.policy,
            &producer.request.capability,
            &producer.request.operation,
        ))?;
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
            admission.validate_typed_borrowed_in_invocation(
                value.borrowed_value(),
                identity,
                binding,
            )?;
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
                admission.validate_owned_typed_borrowed_in_invocation(
                    source.borrowed_value(),
                    ty,
                    binding,
                )?;
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
                admission.validate_owned_typed_borrowed_in_invocation(
                    item.borrowed_value(),
                    identity,
                    binding,
                )?;
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
            payload_type: producer.payload_type,
            arguments: digest,
        })
    }
}
