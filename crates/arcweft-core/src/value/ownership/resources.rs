//! One borrowed custody graph, preserving physical storage paths.

use super::{
    RuntimeAffineLineHandle, RuntimeAffineLineHandleError, RuntimeValuePath,
    RuntimeValuePathSegment, RuntimeValueResourceGraph,
};
use crate::value::{
    RuntimeAgentKind, RuntimeIteratorKind, RuntimeOpaqueValueClass, RuntimePredicateKind,
    RuntimePredicateView, RuntimeSeq, RuntimeSequenceView, RuntimeTupleView, RuntimeValueView,
    awbc_save::AwbcRuntimeSeqSnapshot,
};

impl RuntimeValueView<'_> {
    /// The enclosing owner admits resources before collecting this metadata.
    /// This traversal constructs no live value or executable capability.
    pub(crate) fn affine_line_handles(
        self,
    ) -> Result<Vec<RuntimeAffineLineHandle>, RuntimeAffineLineHandleError> {
        let mut graph = RuntimeValueResourceGraph::default();
        self.collect_resources(&RuntimeValuePath::root(), &mut graph)?;
        Ok(graph.line_handles)
    }

    pub(super) fn collect_resources(
        self,
        path: &RuntimeValuePath,
        graph: &mut RuntimeValueResourceGraph,
    ) -> Result<(), RuntimeAffineLineHandleError> {
        match self {
            Self::Scalar(_) | Self::DenseNumeric(_) | Self::Range(_) => Ok(()),
            Self::Tuple(values) => {
                collect_tuple(values, path, RuntimeValuePathSegment::TupleElement, graph)
            }
            Self::Record(fields) => {
                for index in 0..fields.len() {
                    let (field, _, value) = fields.get(index).expect("record owns each field");
                    value.collect_resources(
                        &path.child(RuntimeValuePathSegment::RecordField(field))?,
                        graph,
                    )?;
                }
                Ok(())
            }
            Self::Sequence(sequence) => sequence.collect_resources(path, graph),
            Self::NominalRecord(record) => {
                for (index, value) in record.fields().iter().enumerate() {
                    let field =
                        crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(index)?;
                    value.collect_resources(
                        &path.child(RuntimeValuePathSegment::NominalRecordField(field))?,
                        graph,
                    )?;
                }
                Ok(())
            }
            Self::Opaque(opaque) => match opaque.value_class() {
                RuntimeOpaqueValueClass::AffineHandle(kind) => {
                    graph.line_handles.push(RuntimeAffineLineHandle {
                        kind,
                        token: crate::runtime_id::RuntimeLineHandleToken::try_decode_view(
                            opaque.payload(),
                        )?,
                        path: path.clone(),
                    });
                    Ok(())
                }
                RuntimeOpaqueValueClass::Plain => opaque
                    .payload()
                    .collect_resources(&path.child(RuntimeValuePathSegment::OpaquePayload)?, graph),
            },
            Self::Iterator(iterator) => iterator.collect_custody(path, graph),
            Self::Variant(variant) => match variant.payload() {
                Some(value) => value.collect_resources(
                    &path.child(RuntimeValuePathSegment::VariantPayload)?,
                    graph,
                ),
                None => Ok(()),
            },
            Self::Reduction(reduction) => reduction.collect_custody(path, graph),
            Self::Agent(agent) => match agent.kind() {
                RuntimeAgentKind::Predicate(predicate) => {
                    collect_predicate(predicate, path, &mut 0, graph)
                }
                RuntimeAgentKind::ActionTarget(_)
                | RuntimeAgentKind::CaptureTarget(_)
                | RuntimeAgentKind::DebugStatePath(_)
                | RuntimeAgentKind::ObservationFieldPath(_)
                | RuntimeAgentKind::Probe(_)
                | RuntimeAgentKind::Diagnostics
                | RuntimeAgentKind::ViewportPoint { .. }
                | RuntimeAgentKind::BinaryData(_)
                | RuntimeAgentKind::DataShape(_) => Ok(()),
            },
            Self::Need(need) => {
                if graph.owned_need.is_none() && need.requires_producer_custody() {
                    graph.owned_need = Some((need.correlation(), path.clone()));
                }
                for (index, value) in need.request_values().enumerate() {
                    let index = u32::try_from(index)
                        .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
                    value.collect_resources(
                        &path.child(RuntimeValuePathSegment::NeedRequestArgument(index))?,
                        graph,
                    )?;
                }
                Ok(())
            }
            Self::Callable(callable) => collect_tuple(
                callable.retained(),
                path,
                RuntimeValuePathSegment::CallableRetained,
                graph,
            ),
        }
    }
}

impl crate::value::RuntimeIteratorView<'_> {
    fn collect_custody(
        self,
        path: &RuntimeValuePath,
        graph: &mut RuntimeValueResourceGraph,
    ) -> Result<(), RuntimeAffineLineHandleError> {
        match self.kind() {
            RuntimeIteratorKind::Values(values) => {
                for (offset, value) in values.iter().enumerate() {
                    let offset = u64::try_from(offset)
                        .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
                    value.collect_resources(
                        &path.child(RuntimeValuePathSegment::IteratorRemainder(offset))?,
                        graph,
                    )?;
                }
                Ok(())
            }
            RuntimeIteratorKind::Range(_) => Ok(()),
            RuntimeIteratorKind::Witness(witness) => witness.state().collect_resources(
                &path.child(RuntimeValuePathSegment::IteratorWitnessState)?,
                graph,
            ),
        }
    }
}

impl crate::value::RuntimeReductionView<'_> {
    fn collect_custody(
        self,
        path: &RuntimeValuePath,
        graph: &mut RuntimeValueResourceGraph,
    ) -> Result<(), RuntimeAffineLineHandleError> {
        self.state()
            .collect_resources(&path.child(RuntimeValuePathSegment::ReductionState)?, graph)?;
        for index in 0..self.commands().len() {
            let command = self
                .commands()
                .get(index)
                .expect("reduction owns each command");
            let index = u32::try_from(index)
                .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
            command.payload().collect_resources(
                &path.child(RuntimeValuePathSegment::ReductionCommandPayload(index))?,
                graph,
            )?;
        }
        Ok(())
    }
}

impl RuntimeSequenceView<'_> {
    fn collect_resources(
        self,
        path: &RuntimeValuePath,
        graph: &mut RuntimeValueResourceGraph,
    ) -> Result<(), RuntimeAffineLineHandleError> {
        match self {
            Self::Bytes(_)
            | Self::Live(RuntimeSeq::Dense(_))
            | Self::Snapshot(AwbcRuntimeSeqSnapshot::Dense(_)) => Ok(()),
            Self::Live(RuntimeSeq::TupleColumns(columns)) => {
                for (index, column) in columns.columns().iter().enumerate() {
                    collect_column(Self::Live(column), index, path, graph)?;
                }
                Ok(())
            }
            Self::Snapshot(AwbcRuntimeSeqSnapshot::TupleColumns { columns, .. }) => {
                for (index, column) in columns.iter().enumerate() {
                    collect_column(Self::Snapshot(column), index, path, graph)?;
                }
                Ok(())
            }
            Self::Live(RuntimeSeq::RecordColumns(records)) => {
                for field in records.fields() {
                    Self::Live(field.values()).collect_resources(
                        &path.child(RuntimeValuePathSegment::RecordColumn(field.field()))?,
                        graph,
                    )?;
                }
                Ok(())
            }
            Self::Snapshot(AwbcRuntimeSeqSnapshot::RecordColumns { fields, .. }) => {
                for field in fields {
                    Self::Snapshot(&field.values).collect_resources(
                        &path.child(RuntimeValuePathSegment::RecordColumn(field.field))?,
                        graph,
                    )?;
                }
                Ok(())
            }
            Self::Live(RuntimeSeq::Values(_))
            | Self::Views(_)
            | Self::Snapshot(AwbcRuntimeSeqSnapshot::Values(_))
            | Self::DialogueBindings(_)
            | Self::DialogueEffects(_) => {
                for index in 0..self.len()? {
                    let value = self
                        .value_view(index)
                        .expect("admitted storage owns each logical row");
                    let index = u64::try_from(index)
                        .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
                    value.collect_resources(
                        &path.child(RuntimeValuePathSegment::SequenceElement(index))?,
                        graph,
                    )?;
                }
                Ok(())
            }
        }
    }
}

fn collect_column(
    column: RuntimeSequenceView<'_>,
    index: usize,
    path: &RuntimeValuePath,
    graph: &mut RuntimeValueResourceGraph,
) -> Result<(), RuntimeAffineLineHandleError> {
    let index = u32::try_from(index)
        .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
    column.collect_resources(
        &path.child(RuntimeValuePathSegment::TupleColumn(index))?,
        graph,
    )
}

fn collect_tuple(
    values: RuntimeTupleView<'_>,
    path: &RuntimeValuePath,
    segment: impl Fn(u32) -> RuntimeValuePathSegment,
    graph: &mut RuntimeValueResourceGraph,
) -> Result<(), RuntimeAffineLineHandleError> {
    for (index, value) in values.iter().enumerate() {
        let index = u32::try_from(index)
            .map_err(|_| RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
        value.collect_resources(&path.child(segment(index))?, graph)?;
    }
    Ok(())
}

fn collect_predicate(
    predicate: RuntimePredicateView<'_>,
    path: &RuntimeValuePath,
    ordinal: &mut u32,
    graph: &mut RuntimeValueResourceGraph,
) -> Result<(), RuntimeAffineLineHandleError> {
    match predicate.kind() {
        RuntimePredicateKind::Compare { value, .. } => {
            let index = *ordinal;
            *ordinal = ordinal
                .checked_add(1)
                .ok_or(RuntimeAffineLineHandleError::StructuralOrdinalOverflow)?;
            value.view().collect_resources(
                &path.child(RuntimeValuePathSegment::AgentEmbeddedValue(index))?,
                graph,
            )
        }
        RuntimePredicateKind::All(predicates) | RuntimePredicateKind::Any(predicates) => {
            for index in 0..predicates.len() {
                collect_predicate(
                    predicates.get(index).expect("predicate list owns each row"),
                    path,
                    ordinal,
                    graph,
                )?;
            }
            Ok(())
        }
        RuntimePredicateKind::Not(predicate) => collect_predicate(predicate, path, ordinal, graph),
        RuntimePredicateKind::Exists { .. }
        | RuntimePredicateKind::ActionEnabled { .. }
        | RuntimePredicateKind::DiagnosticsHasError => Ok(()),
    }
}
