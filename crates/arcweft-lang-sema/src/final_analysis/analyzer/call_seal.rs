//! One-shot prepared-call graph detachment and final C-seal staging.
//!
//! This module is the analyzer half of the phase boundary.  It consumes the
//! complete graph, detaches every prepared candidate into the one callable
//! definition arena, and retains only opaque graph keys until dependency-first
//! final application sealing replaces them with checked continuations.

use std::{collections::BTreeMap, sync::Arc};

use arcweft_lang_hir::identity::{ExprId, LocalId};

use crate::{
    callable::{
        CallConstraintInvariant, CallableParameterAdmission, CallableReceiverMode,
        CheckedCallApplicationSite, CheckedCallArgumentPassing, CheckedCallArgumentSlotSource,
        CheckedCallCalleeExecution, CheckedCallExecutionArgumentSeal,
        CheckedCallExecutionProjectionSeal, CheckedCallExecutionSlotSeal,
        CheckedCallExecutionSource, CheckedCallOperandDestination, CheckedCallReceiverProjection,
        CheckedCallSemanticOperandSeal, CheckedCallSemanticOperandSource,
        CheckedCallSemanticSelection, CheckedCallSite, CheckedCaptureSignatureSeal,
        DetachedPreparedCallableApplication, MappedCallArgumentPassing,
        PreparedCallGraphSealAuthority, PreparedCallGraphSealNodeKey, PreparedCallGraphSealPayload,
        PreparedCallableDefinitionKey, PreparedFunctionValueOriginIdentity,
        PreparedResolvedCallableDefinitionBatch, PreparedResolvedCallableDetachArena,
        PreparedResolvedCallableIdentity, ResolvedCallable, ResolvedCallableBase,
        ResolvedCallableBaseInstantiation, ResolvedCallableBaseSeal,
        ResolvedCallableStableIdentitySeal, ResolvedCallableState,
    },
    final_analysis::{
        CheckedExpression, FinalCallSealFailure, FinalCallSealLocation, FinalSemanticAnalysisError,
        PreparedExpressionFact,
    },
    semantic_coordinate::SemanticCoordinateIndex,
    types::TypeKind,
};

use super::calls::{
    AnalyzerDetachedCandidateRecord, AnalyzerDetachedConsideredCandidate,
    AnalyzerDetachedUnselectedCall, AnalyzerDetachedUnselectedOutcome, AnalyzerPreparedCallGraph,
    AnalyzerPreparedCalleeExpression, AnalyzerPreparedExpressionResolution, final_call_effects,
    final_callable_effects,
};

pub(super) struct DetachedAnalyzerCallGraph {
    pub(super) authority: PreparedCallGraphSealAuthority,
    pub(super) definitions: PreparedResolvedCallableDefinitionBatch,
    pub(super) nodes: Box<[DetachedAnalyzerCallNode]>,
}

pub(super) struct DetachedAnalyzerCallNode {
    pub(super) key: PreparedCallGraphSealNodeKey,
    pub(super) site: CheckedCallSite,
    pub(super) dependencies: Box<[PreparedCallGraphSealNodeKey]>,
    pub(super) payload: DetachedAnalyzerCallPayload,
}

pub(super) enum DetachedAnalyzerCallPayload {
    SelectedValue {
        selected: DetachedAnalyzerSelectedCall,
        result: crate::callable::CallableResultSchema,
    },
    SelectedContinuation {
        selected: DetachedAnalyzerSelectedCall,
        result: crate::types::TypeKind,
    },
    Unselected(AnalyzerDetachedUnselectedCall),
}

pub(super) struct DetachedAnalyzerSelectedCall {
    pub(super) application: DetachedPreparedCallableApplication,
    pub(super) record: AnalyzerDetachedCandidateRecord,
}

pub(super) fn detach_prepared_call_graph(
    graph: AnalyzerPreparedCallGraph,
) -> Result<DetachedAnalyzerCallGraph, CallConstraintInvariant> {
    let (authority, nodes) = graph.into_seal_nodes()?;
    let mut arena = PreparedResolvedCallableDetachArena::new();
    let mut detached = Vec::with_capacity(nodes.len());
    for node in nodes {
        let (key, site, dependencies, payload) = node.into_parts();
        let payload = match payload {
            PreparedCallGraphSealPayload::SelectedValue { prefix, result } => {
                let (application, record) = prefix.into_parts();
                let application = application.detach(&mut arena)?;
                let record = record.into_parts().detach(&mut arena)?;
                DetachedAnalyzerCallPayload::SelectedValue {
                    selected: DetachedAnalyzerSelectedCall {
                        application,
                        record,
                    },
                    result,
                }
            }
            PreparedCallGraphSealPayload::SelectedContinuation { prefix } => {
                let (application, record) = prefix.into_parts();
                let result = application.function_type()?;
                let application = application.detach(&mut arena)?;
                let record = record.into_parts().detach(&mut arena)?;
                DetachedAnalyzerCallPayload::SelectedContinuation {
                    selected: DetachedAnalyzerSelectedCall {
                        application,
                        record,
                    },
                    result,
                }
            }
            PreparedCallGraphSealPayload::Unselected(value) => {
                DetachedAnalyzerCallPayload::Unselected(value.detach(&mut arena)?)
            }
        };
        detached.push(DetachedAnalyzerCallNode {
            key,
            site,
            dependencies,
            payload,
        });
    }
    let definitions = arena.finish()?;
    Ok(DetachedAnalyzerCallGraph {
        authority,
        definitions,
        nodes: detached.into_boxed_slice(),
    })
}

fn seal_resolved_callable_base(
    location: FinalCallSealLocation,
    key: PreparedCallableDefinitionKey,
    definitions: &mut PreparedResolvedCallableDefinitionBatch,
    sealed: &mut BTreeMap<PreparedCallableDefinitionKey, Arc<ResolvedCallableBase>>,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    locals: &BTreeMap<LocalId, TypeKind>,
) -> Result<Arc<ResolvedCallableBase>, FinalSemanticAnalysisError> {
    if let Some(base) = sealed.get(&key) {
        return Ok(Arc::clone(base));
    }
    let stable = {
        let definition = definitions
            .get(key)
            .map_err(|error| final_call_seal_error(location, error))?;
        stable_identity_seal(location, definition, coordinates, expressions, locals)?
    };
    let definition = definitions
        .take(key)
        .map_err(|error| final_call_seal_error(location, error))?;
    let base = ResolvedCallableBase::seal(ResolvedCallableBaseSeal { definition, stable })
        .map_err(|error| final_call_seal_error(location, error))?;
    sealed.insert(key, Arc::clone(&base));
    Ok(base)
}

#[allow(clippy::too_many_arguments)]
fn seal_resolved_callable(
    location: FinalCallSealLocation,
    detached: crate::callable::DetachedPreparedResolvedCallable,
    node_dependencies: &[PreparedCallGraphSealNodeKey],
    authority: &PreparedCallGraphSealAuthority,
    definitions: &mut PreparedResolvedCallableDefinitionBatch,
    bases: &mut BTreeMap<PreparedCallableDefinitionKey, Arc<ResolvedCallableBase>>,
    applications: &BTreeMap<PreparedCallGraphSealNodeKey, crate::callable::CheckedCallApplication>,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    locals: &BTreeMap<LocalId, TypeKind>,
) -> Result<Arc<ResolvedCallable>, FinalSemanticAnalysisError> {
    let base = seal_resolved_callable_base(
        location,
        detached.definition(),
        definitions,
        bases,
        coordinates,
        expressions,
        locals,
    )?;
    match detached {
        crate::callable::DetachedPreparedResolvedCallable::Base { .. } => {
            Ok(ResolvedCallable::from_base(base))
        }
        crate::callable::DetachedPreparedResolvedCallable::PreparedContinuation {
            reference,
            current_group,
            function_type,
            ..
        } => {
            let dependency = authority
                .resolve_reference(&reference)
                .map_err(|error| final_call_seal_error(location, error))?;
            if node_dependencies.binary_search(&dependency).is_err() {
                return Err(final_call_seal_error(
                    location,
                    CallConstraintInvariant::InvalidPreparedDependencyOrder,
                ));
            }
            let continuation = applications
                .get(&dependency)
                .and_then(|application| match application.result() {
                    crate::callable::CheckedCallResult::Continuation(continuation) => {
                        Some(Arc::clone(continuation))
                    }
                    crate::callable::CheckedCallResult::Value(_) => None,
                    crate::callable::CheckedCallResult::ContentEmission(_) => None,
                })
                .ok_or_else(|| {
                    final_call_seal_error(
                        location,
                        CallConstraintInvariant::MissingOrStalePreparedNode,
                    )
                })?;
            ResolvedCallable::try_from_continuation(
                base,
                continuation,
                current_group,
                &function_type,
            )
            .map_err(|error| final_call_seal_error(location, error))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn seal_selected_inventory(
    location: FinalCallSealLocation,
    selected: crate::callable::DetachedPreparedResolvedCallable,
    considered: Box<[AnalyzerDetachedConsideredCandidate]>,
    node_dependencies: &[PreparedCallGraphSealNodeKey],
    authority: &PreparedCallGraphSealAuthority,
    definitions: &mut PreparedResolvedCallableDefinitionBatch,
    bases: &mut BTreeMap<PreparedCallableDefinitionKey, Arc<ResolvedCallableBase>>,
    applications: &BTreeMap<PreparedCallGraphSealNodeKey, crate::callable::CheckedCallApplication>,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    locals: &BTreeMap<LocalId, TypeKind>,
    limits: &crate::callable::CallableLimits,
) -> Result<crate::callable::CheckedCandidateInventory, FinalSemanticAnalysisError> {
    let selected = seal_resolved_callable(
        location,
        selected,
        node_dependencies,
        authority,
        definitions,
        bases,
        applications,
        coordinates,
        expressions,
        locals,
    )?;
    let mut selected_index = None;
    let mut candidates = Vec::with_capacity(considered.len());
    for candidate in considered {
        match candidate {
            AnalyzerDetachedConsideredCandidate::Selected => {
                if selected_index.replace(candidates.len()).is_some() {
                    return Err(final_call_seal_error(
                        location,
                        CallConstraintInvariant::PreparedBaseMismatch,
                    ));
                }
                candidates.push(Arc::clone(&selected));
            }
            AnalyzerDetachedConsideredCandidate::Other(candidate) => {
                candidates.push(seal_resolved_callable(
                    location,
                    candidate,
                    node_dependencies,
                    authority,
                    definitions,
                    bases,
                    applications,
                    coordinates,
                    expressions,
                    locals,
                )?);
            }
        }
    }
    let selected_index = selected_index.ok_or_else(|| {
        final_call_seal_error(location, CallConstraintInvariant::PreparedBaseMismatch)
    })?;
    let selected_index = crate::callable::PreparedCandidateIndex::try_from_usize(selected_index)
        .map_err(|error| final_call_seal_error(location, error))?;
    crate::callable::CheckedCandidateInventory::seal(candidates, selected_index, limits)
        .map_err(|error| final_call_seal_error(location, error))
}

fn checked_execution_source(
    location: FinalCallSealLocation,
    source: crate::callable::CheckedCallArgumentSlotSource,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
) -> Result<CheckedCallExecutionSource, FinalSemanticAnalysisError> {
    let evidence = coordinates
        .expression_evidence(source.owner())
        .map_err(|_| {
            final_call_seal_error(
                location,
                CallConstraintInvariant::MissingCheckedExpressionCoordinate {
                    owner: source.owner(),
                },
            )
        })?;
    CheckedCallExecutionSource::seal(source, evidence)
        .map_err(|error| final_call_seal_error(location, error))
}

fn checked_callee_execution(
    location: FinalCallSealLocation,
    record: &AnalyzerDetachedCandidateRecord,
    selected: &ResolvedCallable,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
) -> Result<CheckedCallCalleeExecution, FinalSemanticAnalysisError> {
    let requires_value_callee = selected.requires_value_callee();
    if record.callee_inputs.is_function_value() != requires_value_callee {
        return Err(final_call_seal_error(
            location,
            CallConstraintInvariant::PreparedCallSiteMismatch,
        ));
    }
    if !requires_value_callee {
        return Ok(CheckedCallCalleeExecution::Direct);
    }
    let source = record
        .callee_expression
        .semantic_expression()
        .ok_or_else(|| {
            final_call_seal_error(location, CallConstraintInvariant::PreparedCallSiteMismatch)
        })?;
    if record.callee_expression.callable_type_projection() != Some(source) {
        return Err(final_call_seal_error(
            location,
            CallConstraintInvariant::PreparedCallSiteMismatch,
        ));
    }
    let source = checked_execution_source(
        location,
        crate::callable::CheckedCallArgumentSlotSource::Expression(source),
        coordinates,
    )?;
    Ok(CheckedCallCalleeExecution::Value { source })
}

fn checked_receiver_projection(
    location: FinalCallSealLocation,
    record: &AnalyzerDetachedCandidateRecord,
    selected: &ResolvedCallable,
    solution: &crate::callable::FrozenCallTypeSolution,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
) -> Result<(CheckedCallReceiverProjection, u32), FinalSemanticAnalysisError> {
    if matches!(selected.state(), ResolvedCallableState::Continuation(_)) {
        return Ok((CheckedCallReceiverProjection::None, 0));
    }
    match (selected.instantiation(), &record.callee_inputs) {
        (
            ResolvedCallableBaseInstantiation::None
            | ResolvedCallableBaseInstantiation::EnumConstructor
            | ResolvedCallableBaseInstantiation::Result { .. }
            | ResolvedCallableBaseInstantiation::Option
            | ResolvedCallableBaseInstantiation::Character { .. },
            _,
        ) => Ok((CheckedCallReceiverProjection::None, 0)),
        (
            ResolvedCallableBaseInstantiation::Receiver { receiver },
            crate::callable::PreparedCallCalleeConstraintInputs::ValueReceiver { source, actual },
        ) => {
            checked_value_receiver_source(
                location,
                record,
                solution,
                *source,
                actual,
                receiver,
                expressions,
            )?;
            Ok((
                CheckedCallReceiverProjection::Operand {
                    mode: CallableReceiverMode::Value {
                        receiver: receiver.clone(),
                    },
                    ty: receiver.clone(),
                    source: checked_execution_source(
                        location,
                        crate::callable::CheckedCallArgumentSlotSource::Expression(*source),
                        coordinates,
                    )?,
                    abi_position: 0,
                },
                1,
            ))
        }
        (
            ResolvedCallableBaseInstantiation::TypeReceiver { receiver },
            crate::callable::PreparedCallCalleeConstraintInputs::AssociatedType { actual },
        ) if receiver.receiver() == actual => Ok((
            CheckedCallReceiverProjection::SemanticOnly {
                mode: CallableReceiverMode::Type {
                    receiver: actual.clone(),
                },
                ty: actual.clone(),
            },
            0,
        )),
        (
            ResolvedCallableBaseInstantiation::Extension {
                receiver,
                group,
                parameter,
            },
            crate::callable::PreparedCallCalleeConstraintInputs::ValueReceiver { source, actual },
        ) => {
            checked_value_receiver_source(
                location,
                record,
                solution,
                *source,
                actual,
                receiver,
                expressions,
            )?;
            Ok((
                CheckedCallReceiverProjection::Operand {
                    mode: CallableReceiverMode::Extension {
                        receiver: receiver.clone(),
                        group: *group,
                        parameter: *parameter,
                    },
                    ty: receiver.clone(),
                    source: checked_execution_source(
                        location,
                        crate::callable::CheckedCallArgumentSlotSource::Expression(*source),
                        coordinates,
                    )?,
                    abi_position: 0,
                },
                1,
            ))
        }
        _ => Err(final_call_seal_error(
            location,
            CallConstraintInvariant::PreparedBaseMismatch,
        )),
    }
}

fn checked_value_receiver_source(
    location: FinalCallSealLocation,
    record: &AnalyzerDetachedCandidateRecord,
    solution: &crate::callable::FrozenCallTypeSolution,
    source: ExprId,
    prepared_actual: &TypeKind,
    resolved_receiver: &TypeKind,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
) -> Result<(), FinalSemanticAnalysisError> {
    let source_identity = super::calls::AnalyzerCallConstraintSource::Receiver { source };
    let mut rows = record
        .closed_sources
        .iter()
        .filter(|closed| closed.source() == source_identity);
    let closed = rows.next().ok_or_else(|| {
        final_call_seal_error(location, CallConstraintInvariant::MalformedMapperSeal)
    })?;
    if rows.next().is_some()
        || closed.actual() != prepared_actual
        || !closed.prepared_source_projection().is_scalar()
        || closed.source_projection() != &crate::types::CheckedConstraintSourceProjection::Scalar
        || closed.final_expected() != Some(resolved_receiver)
        || solution
            .complete_value(prepared_actual)
            .map_err(|error| final_call_seal_error(location, error))?
            != *resolved_receiver
        || expressions
            .get(&source)
            .is_none_or(|checked| checked.value_type() != Some(prepared_actual))
    {
        return Err(final_call_seal_error(
            location,
            CallConstraintInvariant::PreparedBaseMismatch,
        ));
    }
    let selection = checked_semantic_selection(location, closed)?;
    if selection
        .alternative()
        .is_none_or(|alternative| alternative.get() != 0)
        || selection.evidence()
            != Some(&crate::callable::CheckedSemanticValueEvidence::NoVariantCase)
    {
        return Err(final_call_seal_error(
            location,
            CallConstraintInvariant::MalformedMapperSeal,
        ));
    }
    Ok(())
}

fn checked_passing(passing: MappedCallArgumentPassing) -> CheckedCallArgumentPassing {
    match passing {
        MappedCallArgumentPassing::Positional => CheckedCallArgumentPassing::Positional,
        MappedCallArgumentPassing::Named => CheckedCallArgumentPassing::Named,
        MappedCallArgumentPassing::Spread => CheckedCallArgumentPassing::Spread,
    }
}

fn checked_semantic_selection(
    location: FinalCallSealLocation,
    closed: &crate::types::constraints::ClosedConstraintProbe<
        super::calls::AnalyzerCallConstraintDomain,
    >,
) -> Result<CheckedCallSemanticSelection, FinalSemanticAnalysisError> {
    let alternative = closed
        .selection()
        .alternative()
        .map(crate::callable::CallableParameterAlternativeIndex::from_checked_ordinal);
    match (alternative, closed.selection().evidence()) {
        (None, None) => Ok(CheckedCallSemanticSelection::Unchecked),
        (Some(alternative), Some(evidence)) => Ok(CheckedCallSemanticSelection::Checked {
            alternative,
            evidence: evidence.clone(),
        }),
        (None, Some(_)) | (Some(_), None) => Err(final_call_seal_error(
            location,
            CallConstraintInvariant::MalformedMapperSeal,
        )),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the C sealer validates one prepared actual against its exact schema row and frozen solution"
)]
fn validate_checked_parameter_actual(
    location: FinalCallSealLocation,
    selected: &ResolvedCallable,
    solution: &crate::callable::FrozenCallTypeSolution,
    coordinate: crate::callable::CallableParameterCoordinate,
    source_projection: &crate::types::CheckedConstraintSourceProjection,
    selection: &CheckedCallSemanticSelection,
    actual: &TypeKind,
    prepared_actual: &TypeKind,
) -> Result<(), FinalSemanticAnalysisError> {
    let parameter = selected
        .schema()
        .group(coordinate.group())
        .and_then(|group| group.parameter(coordinate.parameter()))
        .ok_or_else(|| {
            final_call_seal_error(location, CallConstraintInvariant::MalformedSchemaInventory)
        })?;
    let sealed_actual = match parameter.admission() {
        CallableParameterAdmission::Checked { rule, .. } => {
            let alternative = selection.alternative().ok_or_else(|| {
                final_call_seal_error(location, CallConstraintInvariant::MalformedMapperSeal)
            })?;
            let alternative = rule
                .alternative(alternative.get() as usize)
                .ok_or_else(|| {
                    final_call_seal_error(location, CallConstraintInvariant::MalformedMapperSeal)
                })?;
            selected
                .base()
                .issue_parameter_effect_projection(
                    coordinate,
                    alternative.expected(),
                    source_projection,
                )
                .and_then(|token| token.seal_actual(actual, solution))
                .map_err(|error| final_call_seal_error(location, error))?
        }
        CallableParameterAdmission::UncheckedSupply => actual.clone(),
        CallableParameterAdmission::Semantic(
            crate::callable::CallableSemanticAdmission::CompileTimeScalar(admission),
        ) => {
            if actual != admission.value_type()
                || !matches!(
                    selection,
                    CheckedCallSemanticSelection::Checked { alternative, evidence }
                        if alternative.get() == 0
                            && evidence == &crate::callable::CheckedSemanticValueEvidence::NoVariantCase
                )
                || source_projection != &crate::types::CheckedConstraintSourceProjection::Scalar
            {
                return Err(final_call_seal_error(
                    location,
                    CallConstraintInvariant::MalformedMapperSeal,
                ));
            }
            actual.clone()
        }
        CallableParameterAdmission::Semantic(
            crate::callable::CallableSemanticAdmission::TextProxyNominal,
        ) => {
            return Err(final_call_seal_error(
                location,
                CallConstraintInvariant::MalformedMapperSeal,
            ));
        }
    };
    if &sealed_actual != prepared_actual {
        return Err(final_call_seal_error(
            location,
            CallConstraintInvariant::PreparedEffectInstantiationMismatch,
        ));
    }
    Ok(())
}

fn checked_execution_projection(
    location: FinalCallSealLocation,
    record: &AnalyzerDetachedCandidateRecord,
    selected: &ResolvedCallable,
    solution: &crate::callable::FrozenCallTypeSolution,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    site: &CheckedCallApplicationSite,
) -> Result<CheckedCallExecutionProjectionSeal, FinalSemanticAnalysisError> {
    let (receiver, mut abi_position) = checked_receiver_projection(
        location,
        record,
        selected,
        solution,
        coordinates,
        expressions,
    )?;
    let mapping = record.inputs.mapping();
    let dialogue_application_coordinate = mapping
        .dialogue_application()
        .map(|application| {
            coordinates
                .expression_evidence(application)
                .map(|evidence| {
                    crate::semantic_coordinate::StableCheckedValueCoordinate::Expression(
                        evidence.into_coordinate(),
                    )
                })
                .map_err(|_| {
                    final_call_seal_error(
                        location,
                        CallConstraintInvariant::MissingCheckedExpressionCoordinate {
                            owner: application,
                        },
                    )
                })
        })
        .transpose()?;
    let mut arguments = Vec::with_capacity(mapping.arguments().len());
    let mut semantic_operands = Vec::new();
    {
        semantic_operands.reserve(mapping.dialogue_application_metadata().len());
        for (argument_index, argument) in mapping.arguments().iter().enumerate() {
            let argument_ordinal =
                arcweft_lang_hir::expr::HirCallArgumentOrdinal::try_from_usize(argument_index)
                    .map_err(|_| {
                        final_call_seal_error(
                            location,
                            CallConstraintInvariant::MalformedMapperSeal,
                        )
                    })?;
            let mut slots = Vec::with_capacity(argument.slots().len());
            for slot in argument.slots() {
                let metadata = mapping.dialogue_application_metadata().iter().find(|row| {
                    row.argument() == argument_ordinal
                        && slot.source() == CheckedCallArgumentSlotSource::Expression(row.source())
                });
                let dialogue_patch_coordinate = slot.coordinate().filter(|coordinate| {
                    selected
                        .schema()
                        .group(coordinate.group())
                        .and_then(|group| group.parameter(coordinate.parameter()))
                        .is_some_and(|parameter| {
                            matches!(
                                parameter.consumer(),
                                crate::callable::CallableParameterConsumer::DialoguePatch(_)
                            )
                        })
                });
                let text_proxy_type_coordinate = slot.coordinate().filter(|coordinate| {
                    selected
                        .schema()
                        .group(coordinate.group())
                        .and_then(|group| group.parameter(coordinate.parameter()))
                        .is_some_and(|parameter| {
                            matches!(
                                parameter.consumer(),
                                crate::callable::CallableParameterConsumer::Content(
                                    crate::callable::CallableContentParameterConsumer::ObjectType
                                )
                            )
                        })
                });
                let source_identity = if let Some(metadata) = metadata {
                    super::calls::AnalyzerCallConstraintSource::DialogueApplicationMetadata {
                        argument: argument_ordinal,
                        slot: slot.slot(),
                        source: metadata.source(),
                        coordinate: metadata.coordinate(),
                    }
                } else if let Some(coordinate) = dialogue_patch_coordinate {
                    super::calls::AnalyzerCallConstraintSource::DialoguePatch {
                        argument: argument_ordinal,
                        slot: slot.slot(),
                        source: slot.source(),
                        coordinate,
                        physical_kind:
                            crate::final_analysis::PhysicalArgumentEvaluationKind::Authored,
                    }
                } else if let Some(coordinate) = text_proxy_type_coordinate {
                    let CheckedCallArgumentSlotSource::Expression(source) = slot.source() else {
                        return Err(final_call_seal_error(
                            location,
                            CallConstraintInvariant::MalformedMapperSeal,
                        ));
                    };
                    super::calls::AnalyzerCallConstraintSource::TextProxyObjectOperand {
                        argument: argument_ordinal,
                        source,
                        coordinate,
                    }
                } else {
                    super::calls::AnalyzerCallConstraintSource::Argument {
                        argument: argument_ordinal,
                        slot: slot.slot(),
                        source: slot.source(),
                        physical_kind:
                            crate::final_analysis::PhysicalArgumentEvaluationKind::Authored,
                    }
                };
                let closed = if metadata.is_some() || text_proxy_type_coordinate.is_some() {
                    record
                        .closed_sources
                        .iter()
                        .find(|closed| closed.source() == source_identity)
                } else {
                    record
                        .closed_sources
                        .iter()
                        .find(|closed| closed.source().same_argument_identity(source_identity))
                }
                .ok_or_else(|| {
                    final_call_seal_error(location, CallConstraintInvariant::MalformedMapperSeal)
                })?;
                let destination = match (slot.coordinate(), slot.open_argument()) {
                    (Some(coordinate), None) => {
                        CheckedCallOperandDestination::Parameter(coordinate)
                    }
                    (None, Some(open)) => CheckedCallOperandDestination::Open(open.clone()),
                    _ => {
                        return Err(final_call_seal_error(
                            location,
                            CallConstraintInvariant::MalformedMapperSeal,
                        ));
                    }
                };
                let selection = checked_semantic_selection(location, closed)?;
                let source_projection = closed.source_projection().clone();
                let inferred = solution
                    .complete_value(closed.actual())
                    .map_err(|error| final_call_seal_error(location, error))?;
                let expected = closed
                    .final_expected()
                    .map(|expected| solution.complete_value(expected))
                    .transpose()
                    .map_err(|error| final_call_seal_error(location, error))?;
                // The Object `type` slot is consumed by the ordinary mapper,
                // but is a semantic-only schema parameter: it has no runtime
                // ABI slot. Its RHS must still be the exact typed TypeValue
                // fact published by the owner transaction. The semantic
                // operand projection below validates the corresponding
                // closed nominal at the same schema coordinate.
                if let Some(coordinate) = text_proxy_type_coordinate {
                    let CheckedCallArgumentSlotSource::Expression(expression) = slot.source()
                    else {
                        return Err(final_call_seal_error(
                            location,
                            CallConstraintInvariant::MalformedMapperSeal,
                        ));
                    };
                    let checked = expressions.get(&expression).ok_or_else(|| {
                        final_call_seal_error(
                            location,
                            CallConstraintInvariant::MissingCheckedExpressionCoordinate {
                                owner: expression,
                            },
                        )
                    })?;
                    let value = checked.project_nominal_type_value().ok_or_else(|| {
                        final_call_seal_error(
                            location,
                            CallConstraintInvariant::MalformedMapperSeal,
                        )
                    })?;
                    let value_type = TypeKind::MetaType(Box::new(value.nominal().ty()));
                    let Some(checked_type) = checked.value_type() else {
                        return Err(final_call_seal_error(
                            location,
                            CallConstraintInvariant::MalformedMapperSeal,
                        ));
                    };
                    if checked_type != &value_type
                        || value.nominal().ty() != *closed.actual()
                        || destination != CheckedCallOperandDestination::Parameter(coordinate)
                    {
                        return Err(final_call_seal_error(
                            location,
                            CallConstraintInvariant::MalformedMapperSeal,
                        ));
                    }
                    // The source was consumed by the ordinary mapper above,
                    // while its checked semantic identity is emitted below;
                    // do not assign it a physical ABI position.
                    continue;
                }
                if let CheckedCallArgumentSlotSource::Expression(expression) = slot.source() {
                    let checked = expressions.get(&expression).ok_or_else(|| {
                        final_call_seal_error(
                            location,
                            CallConstraintInvariant::MissingCheckedExpressionCoordinate {
                                owner: expression,
                            },
                        )
                    })?;
                    let actual = checked.value_type().ok_or_else(|| {
                        final_call_seal_error(
                            location,
                            CallConstraintInvariant::MalformedMapperSeal,
                        )
                    })?;
                    if let CheckedCallOperandDestination::Parameter(coordinate) = &destination {
                        if record.consumer.runtime_parameter(*coordinate).is_none() {
                            validate_checked_parameter_actual(
                                location,
                                selected,
                                solution,
                                *coordinate,
                                &source_projection,
                                &selection,
                                actual,
                                closed.actual(),
                            )?;
                        }
                    } else if actual != closed.actual() {
                        return Err(final_call_seal_error(
                            location,
                            CallConstraintInvariant::PreparedEffectInstantiationMismatch,
                        ));
                    }
                    if let Some(metadata) = metadata {
                        let CheckedCallOperandDestination::Parameter(coordinate) = destination
                        else {
                            return Err(final_call_seal_error(
                                location,
                                CallConstraintInvariant::MalformedMapperSeal,
                            ));
                        };
                        let parameter = selected
                            .schema()
                            .group(coordinate.group())
                            .and_then(|group| group.parameter(coordinate.parameter()))
                            .ok_or_else(|| {
                                final_call_seal_error(
                                    location,
                                    CallConstraintInvariant::MalformedSchemaInventory,
                                )
                            })?;
                        let source = match (metadata.evidence(), checked.checked_resolution(), parameter.consumer()) {
                            (
                                crate::callable::PreparedDialogueApplicationMetadataEvidence::Id(id),
                                Some(crate::final_analysis::CheckedExpressionResolution::DialogueLineCoordinate(checked_id)),
                                crate::callable::CallableParameterConsumer::DialogueApplicationMetadata(
                                    crate::callable::DialogueApplicationMetadataCoordinate::Id,
                                ),
                            ) if id == checked_id => {
                                CheckedCallSemanticOperandSource::DialogueApplicationId {
                                    argument: argument_ordinal,
                                    source: metadata.source(),
                                    application: dialogue_application_coordinate.clone().ok_or_else(|| {
                                        final_call_seal_error(
                                            location,
                                            CallConstraintInvariant::MalformedMapperSeal,
                                        )
                                    })?,
                                    id: id.clone(),
                                }
                            }
                            (
                                crate::callable::PreparedDialogueApplicationMetadataEvidence::TextKey(key),
                                Some(crate::final_analysis::CheckedExpressionResolution::DialogueTextKeyCoordinate(checked_key)),
                                crate::callable::CallableParameterConsumer::DialogueApplicationMetadata(
                                    crate::callable::DialogueApplicationMetadataCoordinate::TextKey,
                                ),
                            ) if key == checked_key => {
                                CheckedCallSemanticOperandSource::DialogueApplicationTextKey {
                                    argument: argument_ordinal,
                                    source: metadata.source(),
                                    application: dialogue_application_coordinate.clone().ok_or_else(|| {
                                        final_call_seal_error(
                                            location,
                                            CallConstraintInvariant::MalformedMapperSeal,
                                        )
                                    })?,
                                    key: key.clone(),
                                }
                            }
                            _ => {
                                return Err(final_call_seal_error(
                                    location,
                                    CallConstraintInvariant::MalformedMapperSeal,
                                ));
                            }
                        };
                        semantic_operands.push(CheckedCallSemanticOperandSeal {
                            source,
                            destination: coordinate,
                            source_projection,
                            selection,
                            inferred,
                            expected,
                        });
                        continue;
                    }
                }
                abi_position = abi_position
                    .checked_add(1)
                    .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
                slots.push(CheckedCallExecutionSlotSeal {
                    slot: slot.slot(),
                    source: checked_execution_source(location, slot.source(), coordinates)?,
                    destination,
                    source_projection,
                    selection,
                    inferred,
                    expected,
                });
            }
            arguments.push(CheckedCallExecutionArgumentSeal {
                argument: argument_ordinal,
                passing: checked_passing(argument.passing()),
                slots: slots.into_boxed_slice(),
            });
        }
    }
    semantic_operands.reserve(record.inputs.semantic_operands().len());
    for operand in record.inputs.semantic_operands() {
        let source_identity = match (operand.owner(), operand.role()) {
            (
                crate::callable::PreparedCallSemanticOperandOwner::DialogueApplication,
                crate::callable::PreparedCallSemanticOperandRole::DialogueTarget
                | crate::callable::PreparedCallSemanticOperandRole::DialogueContent
                | crate::callable::PreparedCallSemanticOperandRole::DialogueLinePlan,
            ) => super::calls::AnalyzerCallConstraintSource::DialogueApplicationOperand {
                owner: operand.owner(),
                source: operand.source(),
                role: operand.role(),
                coordinate: operand.coordinate(),
            },
            (
                crate::callable::PreparedCallSemanticOperandOwner::TextProxyObject,
                crate::callable::PreparedCallSemanticOperandRole::TextProxyNominalDiscriminator,
            ) => super::calls::AnalyzerCallConstraintSource::TextProxyObjectOperand {
                argument: operand.argument().ok_or_else(|| {
                    final_call_seal_error(location, CallConstraintInvariant::MalformedMapperSeal)
                })?,
                source: operand.source(),
                coordinate: operand.coordinate(),
            },
            _ => {
                return Err(final_call_seal_error(
                    location,
                    CallConstraintInvariant::MalformedMapperSeal,
                ));
            }
        };
        let closed = record
            .closed_sources
            .iter()
            .find(|closed| closed.source() == source_identity)
            .ok_or_else(|| {
                final_call_seal_error(location, CallConstraintInvariant::MalformedMapperSeal)
            })?;
        let selection = checked_semantic_selection(location, closed)?;
        let source_projection = closed.source_projection().clone();
        let inferred = solution
            .complete_value(closed.actual())
            .map_err(|error| final_call_seal_error(location, error))?;
        let expected = closed
            .final_expected()
            .map(|expected| solution.complete_value(expected))
            .transpose()
            .map_err(|error| final_call_seal_error(location, error))?;
        let actual = match (operand.owner(), operand.role()) {
            (
                crate::callable::PreparedCallSemanticOperandOwner::DialogueApplication,
                crate::callable::PreparedCallSemanticOperandRole::DialogueTarget,
            ) => expressions
                .get(&operand.source())
                .ok_or_else(|| {
                    final_call_seal_error(
                        location,
                        CallConstraintInvariant::MissingCheckedExpressionCoordinate {
                            owner: operand.source(),
                        },
                    )
                })?
                .value_type()
                .ok_or_else(|| {
                    final_call_seal_error(location, CallConstraintInvariant::MalformedMapperSeal)
                })?,
            (
                crate::callable::PreparedCallSemanticOperandOwner::DialogueApplication,
                crate::callable::PreparedCallSemanticOperandRole::DialogueContent
                | crate::callable::PreparedCallSemanticOperandRole::DialogueLinePlan,
            ) => {
                if operand.source() != site.raw().expression() {
                    return Err(final_call_seal_error(
                        location,
                        CallConstraintInvariant::MalformedMapperSeal,
                    ));
                }
                operand.actual()
            }
            (
                crate::callable::PreparedCallSemanticOperandOwner::TextProxyObject,
                crate::callable::PreparedCallSemanticOperandRole::TextProxyNominalDiscriminator,
            ) => operand.actual(),
            _ => {
                return Err(final_call_seal_error(
                    location,
                    CallConstraintInvariant::MalformedMapperSeal,
                ));
            }
        };
        if matches!(
            (operand.owner(), operand.role()),
            (
                crate::callable::PreparedCallSemanticOperandOwner::TextProxyObject,
                crate::callable::PreparedCallSemanticOperandRole::TextProxyNominalDiscriminator,
            )
        ) {
            // The source TypeValue was checked against this closed source in
            // the mapper projection above. Keep only the affine join here;
            // admission, coordinate, selection, and expected/inferred
            // validation belong to the checked execution projection owner.
            if actual != closed.actual() {
                return Err(final_call_seal_error(
                    location,
                    CallConstraintInvariant::MalformedMapperSeal,
                ));
            }
        } else {
            if record
                .consumer
                .runtime_parameter(operand.coordinate())
                .is_none()
            {
                validate_checked_parameter_actual(
                    location,
                    selected,
                    solution,
                    operand.coordinate(),
                    &source_projection,
                    &selection,
                    actual,
                    closed.actual(),
                )?;
            }
        }
        let source = match (operand.owner(), operand.role()) {
            (
                crate::callable::PreparedCallSemanticOperandOwner::DialogueApplication,
                crate::callable::PreparedCallSemanticOperandRole::DialogueTarget,
            ) => CheckedCallSemanticOperandSource::DialogueTarget(checked_execution_source(
                location,
                CheckedCallArgumentSlotSource::Expression(operand.source()),
                coordinates,
            )?),
            (
                crate::callable::PreparedCallSemanticOperandOwner::DialogueApplication,
                crate::callable::PreparedCallSemanticOperandRole::DialogueContent,
            ) => CheckedCallSemanticOperandSource::DialogueContent {
                application: site.coordinate().clone(),
            },
            (
                crate::callable::PreparedCallSemanticOperandOwner::DialogueApplication,
                crate::callable::PreparedCallSemanticOperandRole::DialogueLinePlan,
            ) => CheckedCallSemanticOperandSource::DialogueLinePlan {
                application: site.coordinate().clone(),
            },
            (
                crate::callable::PreparedCallSemanticOperandOwner::TextProxyObject,
                crate::callable::PreparedCallSemanticOperandRole::TextProxyNominalDiscriminator,
            ) => CheckedCallSemanticOperandSource::TextProxyObject {
                argument: operand.argument().ok_or_else(|| {
                    final_call_seal_error(location, CallConstraintInvariant::MalformedMapperSeal)
                })?,
                source: checked_execution_source(
                    location,
                    CheckedCallArgumentSlotSource::Expression(operand.source()),
                    coordinates,
                )?,
            },
            _ => {
                return Err(final_call_seal_error(
                    location,
                    CallConstraintInvariant::MalformedMapperSeal,
                ));
            }
        };
        semantic_operands.push(CheckedCallSemanticOperandSeal {
            source,
            destination: operand.coordinate(),
            source_projection,
            selection,
            inferred,
            expected,
        });
    }
    let attached_parameter = selected
        .schema()
        .attached_content()
        .filter(|parameter| parameter.group() == selected.call_group());
    let mut attached_content_abi_type = None;
    let attached_content = match (attached_parameter, record.inputs.attached_content()) {
        (None, None) => None,
        (
            Some(parameter),
            Some(crate::callable::PreparedCallAttachedContentOperand::Present { source }),
        ) if parameter.execution()
            == crate::callable::CallableAttachedContentExecution::Structural =>
        {
            Some(
                crate::callable::CheckedCallAttachedContentOperand::StructuralPresent {
                    source: crate::callable::CheckedCallAttachedContentSource::seal(source, site)
                        .map_err(|error| final_call_seal_error(location, error))?,
                },
            )
        }
        (Some(parameter), prepared)
            if parameter.execution()
                == crate::callable::CallableAttachedContentExecution::RuntimeContent =>
        {
            let content = selected
                .schema()
                .result_schema()
                .value_type()
                .map(|result| solution.instantiate_template(result))
                .transpose()
                .map_err(|error| final_call_seal_error(location, error))?
                .ok_or_else(|| {
                    final_call_seal_error(location, CallConstraintInvariant::MalformedMapperSeal)
                })?;
            attached_content_abi_type = Some(match parameter.presence() {
                crate::callable::CallableParameterPresence::Required => content,
                crate::callable::CallableParameterPresence::Optional
                | crate::callable::CallableParameterPresence::Defaulted => {
                    TypeKind::Option(Box::new(content))
                }
            });
            let current_abi_position = abi_position;
            abi_position
                .checked_add(1)
                .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            Some(match prepared {
                Some(crate::callable::PreparedCallAttachedContentOperand::Omitted) => {
                    crate::callable::CheckedCallAttachedContentOperand::RuntimeOmitted {
                        abi_position: current_abi_position,
                    }
                }
                Some(crate::callable::PreparedCallAttachedContentOperand::Present { source }) => {
                    crate::callable::CheckedCallAttachedContentOperand::RuntimePresent {
                        source: crate::callable::CheckedCallAttachedContentSource::seal(
                            source, site,
                        )
                        .map_err(|error| final_call_seal_error(location, error))?,
                        abi_position: current_abi_position,
                    }
                }
                None => {
                    return Err(final_call_seal_error(
                        location,
                        CallConstraintInvariant::MalformedMapperSeal,
                    ));
                }
            })
        }
        _ => {
            return Err(final_call_seal_error(
                location,
                CallConstraintInvariant::MalformedMapperSeal,
            ));
        }
    };
    Ok(CheckedCallExecutionProjectionSeal {
        receiver,
        arguments: arguments.into_boxed_slice(),
        semantic_operands: semantic_operands.into_boxed_slice(),
        attached_content,
        attached_content_abi_type,
    })
}

struct SealedSelectedCall {
    application: crate::callable::CheckedCallApplication,
    expression_resolution: AnalyzerPreparedExpressionResolution,
    callee_expression: AnalyzerPreparedCalleeExpression,
    enclosing_callable: Option<arcweft_lang_hir::symbol::CallableDeclarationKey>,
    accounting: crate::callable::CallResolverAccountingReport,
}

impl SealedSelectedCall {
    fn callee_update(
        &self,
        expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
        checked_callables: &crate::callable::CheckedCallableCatalog,
    ) -> Result<Option<(ExprId, PreparedExpressionFact)>, FinalSemanticAnalysisError> {
        let Some(callee) = self.callee_expression.semantic_expression() else {
            return Ok(None);
        };
        let previous = expressions
            .get(&callee)
            .cloned()
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: callee })?;
        if matches!(
            self.callee_expression,
            AnalyzerPreparedCalleeExpression::RetainExisting { .. }
        ) {
            let selected = self.application.core().candidates().selected();
            let crate::callable::CallableCandidateId::EnumVariant(candidate) = selected.id() else {
                return Ok(None);
            };
            let PreparedExpressionFact::Variant(prepared) = previous else {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            };
            let ResolvedCallableBaseInstantiation::EnumConstructor = selected.instantiation()
            else {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            };
            let expected = selected
                .schema()
                .value_type()
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            if prepared.owner().ty() != *expected
                || prepared.selected_ordinal() != candidate.case()
                || expected.semantic_identity_digest()? != candidate.owner()
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            let updated = prepared.try_map_types(&mut |ty| {
                self.application
                    .core()
                    .solution()
                    .instantiate_template(ty)
                    .map_err(|error| {
                        final_call_seal_error(
                            FinalCallSealLocation::Site(self.application.core().site()),
                            error,
                        )
                    })
            })?;
            if updated.shell().value_type() != self.application.result().value_type() {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            return Ok(Some((callee, PreparedExpressionFact::from(updated))));
        }
        let ty = selected_callable_type(&self.application, checked_callables)?;
        let updated = match previous {
            PreparedExpressionFact::Method(prepared) => PreparedExpressionFact::from(
                prepared
                    .with_type(ty)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
            ),
            PreparedExpressionFact::Complete(previous) => CheckedExpression::value(
                ty,
                previous.required_type_selection(callee)?,
                previous.effects().clone(),
                previous.resolution().clone(),
            )
            .into(),
            PreparedExpressionFact::CompileTimeScalar(prepared) => {
                let (shell, scalar, original) = prepared.into_parts();
                let PreparedExpressionFact::Complete(previous) = original else {
                    return Err(FinalSemanticAnalysisError::UnsealedPreparedC2Owner);
                };
                PreparedExpressionFact::from(
                    crate::final_analysis::PreparedCompileTimeScalarExpression::try_new(
                        shell,
                        scalar,
                        CheckedExpression::value(
                            ty,
                            previous.required_type_selection(callee)?,
                            previous.effects().clone(),
                            previous.resolution().clone(),
                        )
                        .into(),
                    )
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
                )
            }
            PreparedExpressionFact::OwnerBound(_)
            | PreparedExpressionFact::DialogueApplication(_)
            | PreparedExpressionFact::ContentApplication(_)
            | PreparedExpressionFact::Entry(_)
            | PreparedExpressionFact::Variant(_)
            | PreparedExpressionFact::ProjectField(_)
            | PreparedExpressionFact::ProjectRecord(_)
            | PreparedExpressionFact::ProjectNominalTypeValue(_) => {
                return Err(FinalSemanticAnalysisError::UnsealedPreparedC2Owner);
            }
        };
        Ok(Some((callee, updated)))
    }
}

#[allow(clippy::too_many_arguments)]
fn seal_selected_call(
    location: FinalCallSealLocation,
    site: CheckedCallSite,
    selected: DetachedAnalyzerSelectedCall,
    expected_result: crate::callable::CheckedCallResultSeal,
    node_dependencies: &[PreparedCallGraphSealNodeKey],
    authority: &PreparedCallGraphSealAuthority,
    definitions: &mut PreparedResolvedCallableDefinitionBatch,
    bases: &mut BTreeMap<PreparedCallableDefinitionKey, Arc<ResolvedCallableBase>>,
    applications: &BTreeMap<PreparedCallGraphSealNodeKey, crate::callable::CheckedCallApplication>,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    locals: &BTreeMap<LocalId, TypeKind>,
    checked_callables: &crate::callable::CheckedCallableCatalog,
    limits: &crate::callable::CallableLimits,
) -> Result<SealedSelectedCall, FinalSemanticAnalysisError> {
    let DetachedAnalyzerSelectedCall {
        application,
        mut record,
    } = selected;
    if site.expression() != record.expression {
        return Err(final_call_seal_error(
            location,
            CallConstraintInvariant::PreparedCallSiteMismatch,
        ));
    }
    let (selected, solution_seed) = application.into_parts();
    let considered = core::mem::take(&mut record.inventory);
    let inventory = seal_selected_inventory(
        location,
        selected,
        considered,
        node_dependencies,
        authority,
        definitions,
        bases,
        applications,
        coordinates,
        expressions,
        locals,
        limits,
    )?;
    let selected = inventory.selected();
    let solution = crate::callable::FrozenCallTypeSolution::seal(solution_seed, selected.base())
        .map_err(|error| final_call_seal_error(location, error))?;
    let callee = checked_callee_execution(location, &record, selected, coordinates)?;
    let site = CheckedCallApplicationSite::seal(
        site,
        coordinates
            .expression_evidence(site.expression())
            .map_err(|_| {
                final_call_seal_error(
                    location,
                    CallConstraintInvariant::MissingCheckedExpressionCoordinate {
                        owner: site.expression(),
                    },
                )
            })?,
    )
    .map_err(|error| final_call_seal_error(location, error))?;
    let execution = checked_execution_projection(
        location,
        &record,
        selected,
        &solution,
        coordinates,
        expressions,
        &site,
    )?;
    let effects = final_call_effects(selected, solution.completed_group(), checked_callables)?;
    let consumer = record
        .consumer
        .checked_seal(selected)
        .map_err(|error| final_call_seal_error(location, error))?;
    let core = crate::callable::CheckedCallApplicationCore::seal(
        crate::callable::CheckedCallApplicationCoreSeal {
            site,
            current_group: solution.completed_group(),
            candidates: inventory,
            consumer,
            solution,
            callee,
            execution,
            effects,
        },
    )
    .map_err(|error| final_call_seal_error(location, error))?;
    let application = crate::callable::CheckedCallApplication::seal(core, expected_result)
        .map_err(|error| final_call_seal_error(location, error))?;
    Ok(SealedSelectedCall {
        application,
        expression_resolution: record.expression_resolution,
        callee_expression: record.callee_expression,
        enclosing_callable: record.enclosing_callable,
        accounting: record.accounting,
    })
}

struct SealedUnselectedCall {
    outcome: crate::callable::CallAnalysisOutcome,
    enclosing_callable: Option<arcweft_lang_hir::symbol::CallableDeclarationKey>,
    accounting: crate::callable::CallResolverAccountingReport,
}

#[allow(clippy::too_many_arguments)]
fn seal_unselected_candidates(
    location: FinalCallSealLocation,
    candidates: Box<[crate::callable::DetachedPreparedResolvedCallable]>,
    node_dependencies: &[PreparedCallGraphSealNodeKey],
    authority: &PreparedCallGraphSealAuthority,
    definitions: &mut PreparedResolvedCallableDefinitionBatch,
    bases: &mut BTreeMap<PreparedCallableDefinitionKey, Arc<ResolvedCallableBase>>,
    applications: &BTreeMap<PreparedCallGraphSealNodeKey, crate::callable::CheckedCallApplication>,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    locals: &BTreeMap<LocalId, TypeKind>,
) -> Result<Vec<Arc<ResolvedCallable>>, FinalSemanticAnalysisError> {
    candidates
        .into_vec()
        .into_iter()
        .map(|candidate| {
            seal_resolved_callable(
                location,
                candidate,
                node_dependencies,
                authority,
                definitions,
                bases,
                applications,
                coordinates,
                expressions,
                locals,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn seal_unselected_call(
    location: FinalCallSealLocation,
    site: CheckedCallSite,
    value: AnalyzerDetachedUnselectedCall,
    node_dependencies: &[PreparedCallGraphSealNodeKey],
    authority: &PreparedCallGraphSealAuthority,
    definitions: &mut PreparedResolvedCallableDefinitionBatch,
    bases: &mut BTreeMap<PreparedCallableDefinitionKey, Arc<ResolvedCallableBase>>,
    applications: &BTreeMap<PreparedCallGraphSealNodeKey, crate::callable::CheckedCallApplication>,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    locals: &BTreeMap<LocalId, TypeKind>,
    limits: &crate::callable::CallableLimits,
) -> Result<SealedUnselectedCall, FinalSemanticAnalysisError> {
    let AnalyzerDetachedUnselectedCall {
        enclosing_callable,
        outcome: prepared_outcome,
        accounting,
        selected_expression_inventory,
    } = value;
    if !matches!(site, CheckedCallSite::HirCall(_)) {
        return Err(final_call_seal_error(
            location,
            CallConstraintInvariant::PreparedCallSiteMismatch,
        ));
    }
    for source in selected_expression_inventory
        .arguments()
        .iter()
        .copied()
        .filter(|argument| argument.semantic_owner().is_none())
        .map(arcweft_lang_hir::project::HirSelectedCallArgument::expression)
        .chain(selected_expression_inventory.callee())
    {
        coordinates.expression_evidence(source).map_err(|_| {
            final_call_seal_error(location, CallConstraintInvariant::PreparedCallSiteMismatch)
        })?;
    }
    let outcome = match prepared_outcome {
        AnalyzerDetachedUnselectedOutcome::Ambiguous {
            callee,
            considered,
            tied,
        } => {
            let considered = seal_unselected_candidates(
                location,
                considered,
                node_dependencies,
                authority,
                definitions,
                bases,
                applications,
                coordinates,
                expressions,
                locals,
            )?;
            let mut candidates = Vec::with_capacity(tied.len());
            for tied in tied {
                let mut matches = considered
                    .iter()
                    .filter(|candidate| candidate.id() == &tied);
                let candidate = matches.next().cloned().ok_or_else(|| {
                    final_call_seal_error(location, CallConstraintInvariant::PreparedBaseMismatch)
                })?;
                if matches.next().is_some() {
                    return Err(final_call_seal_error(
                        location,
                        CallConstraintInvariant::PreparedBaseMismatch,
                    ));
                }
                candidates.push(candidate);
            }
            crate::callable::CallAnalysisOutcome::Ambiguous(
                crate::callable::CheckedAmbiguousCallEvidence::seal(
                    site, callee, candidates, considered, limits,
                )
                .map_err(|_| {
                    final_call_seal_error(
                        location,
                        CallConstraintInvariant::InvalidPreparedNodeState,
                    )
                })?,
            )
        }
        AnalyzerDetachedUnselectedOutcome::Rejected { callee, candidates } => {
            let candidates = seal_unselected_candidates(
                location,
                candidates,
                node_dependencies,
                authority,
                definitions,
                bases,
                applications,
                coordinates,
                expressions,
                locals,
            )?;
            crate::callable::CallAnalysisOutcome::Rejected(
                crate::callable::CheckedRejectedCallEvidence::seal(
                    site, callee, candidates, limits,
                )
                .map_err(|_| {
                    final_call_seal_error(
                        location,
                        CallConstraintInvariant::InvalidPreparedNodeState,
                    )
                })?,
            )
        }
        AnalyzerDetachedUnselectedOutcome::NonCallable { callee, source, ty } => {
            crate::callable::CallAnalysisOutcome::NonCallable(
                crate::callable::CheckedNonCallableEvidence::seal(site, callee, source, ty)
                    .map_err(|_| {
                        final_call_seal_error(
                            location,
                            CallConstraintInvariant::InvalidPreparedNodeState,
                        )
                    })?,
            )
        }
        AnalyzerDetachedUnselectedOutcome::Missing { callee, kind } => {
            crate::callable::CallAnalysisOutcome::Missing(
                crate::callable::CheckedMissingCallEvidence::seal(site, callee, kind).map_err(
                    |_| {
                        final_call_seal_error(
                            location,
                            CallConstraintInvariant::InvalidPreparedNodeState,
                        )
                    },
                )?,
            )
        }
    };
    Ok(SealedUnselectedCall {
        outcome,
        enclosing_callable,
        accounting,
    })
}

struct PendingFinalCall {
    owner: ExprId,
    facts: crate::callable::CallTargetFacts,
    selected: Option<PendingSelectedExpressionUpdate>,
}

struct PendingSelectedExpressionUpdate {
    resolution: AnalyzerPreparedExpressionResolution,
    result: crate::callable::CallableResultSchema,
    effects: crate::effects::EffectSet,
    callee: Option<(ExprId, PreparedExpressionFact)>,
}

fn checked_call_result_schema(
    application: &crate::callable::CheckedCallApplication,
) -> Result<crate::callable::CallableResultSchema, FinalSemanticAnalysisError> {
    match application.result() {
        crate::callable::CheckedCallResult::Value(value) => {
            Ok(crate::callable::CallableResultSchema::Value(value.clone()))
        }
        crate::callable::CheckedCallResult::ContentEmission(operation) => Ok(
            crate::callable::CallableResultSchema::ContentEmission(*operation),
        ),
        crate::callable::CheckedCallResult::Continuation(continuation) => Ok(
            crate::callable::CallableResultSchema::Value(continuation.function_type().clone()),
        ),
    }
}

fn selected_callable_type(
    application: &crate::callable::CheckedCallApplication,
    checked_callables: &crate::callable::CheckedCallableCatalog,
) -> Result<TypeKind, FinalSemanticAnalysisError> {
    let selected = application.core().candidates().selected();
    if let ResolvedCallableState::Continuation(continuation) = selected.state() {
        return Ok(continuation.function_type().clone());
    }
    let effects = final_callable_effects(selected, checked_callables)?;
    let declared = selected
        .base()
        .callable_type_with_terminal_effects(&effects)
        .map_err(|error| {
            final_call_seal_error(
                FinalCallSealLocation::Site(application.core().site()),
                error,
            )
        })?;
    application
        .core()
        .solution()
        .instantiate_result(&declared)
        .map_err(|error| {
            final_call_seal_error(
                FinalCallSealLocation::Site(application.core().site()),
                error,
            )
        })
}

impl super::Analyzer<'_, '_, '_> {
    pub(super) fn finalize_call_facts(
        &mut self,
        checked_callables: &crate::callable::CheckedCallableCatalog,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let graph = self
            .facts
            .take_prepared_calls()
            .map_err(FinalSemanticAnalysisError::from)?;
        let DetachedAnalyzerCallGraph {
            authority,
            mut definitions,
            nodes,
        } = detach_prepared_call_graph(graph)
            .map_err(|error| final_call_seal_error(FinalCallSealLocation::Graph, error))?;
        let mut bases = BTreeMap::new();
        let mut applications = BTreeMap::new();
        let mut pending = Vec::with_capacity(nodes.len());
        {
            let expressions = self.facts.expressions();
            let locals = self.facts.locals();
            for node in nodes.into_vec() {
                let DetachedAnalyzerCallNode {
                    key,
                    site,
                    dependencies,
                    payload,
                } = node;
                self.control.check()?;
                let owner = site.expression();
                match payload {
                    DetachedAnalyzerCallPayload::SelectedValue { selected, result } => {
                        let expected_result = match &result {
                            crate::callable::CallableResultSchema::Value(value) => {
                                crate::callable::CheckedCallResultSeal::Value {
                                    prepared: value.clone(),
                                }
                            }
                            crate::callable::CallableResultSchema::ContentEmission(operation) => {
                                crate::callable::CheckedCallResultSeal::ContentEmission {
                                    operation: *operation,
                                }
                            }
                        };
                        let sealed = seal_selected_call(
                            FinalCallSealLocation::Site(site),
                            site,
                            selected,
                            expected_result,
                            &dependencies,
                            &authority,
                            &mut definitions,
                            &mut bases,
                            &applications,
                            coordinates,
                            expressions,
                            locals,
                            checked_callables,
                            &self.catalogs.callable_limits,
                        )?;
                        let callee = sealed.callee_update(expressions, checked_callables)?;
                        let update = PendingSelectedExpressionUpdate {
                            resolution: sealed.expression_resolution,
                            result: checked_call_result_schema(&sealed.application)?,
                            effects: sealed.application.core().effects().concrete().clone(),
                            callee,
                        };
                        applications.insert(key, sealed.application.clone());
                        let facts = crate::callable::CallTargetFacts::try_new(
                            crate::callable::CallTargetFactsInput {
                                enclosing_callable: sealed.enclosing_callable,
                                outcome: crate::callable::CallAnalysisOutcome::Selected(
                                    sealed.application,
                                ),
                                accounting: sealed.accounting,
                            },
                            self.module(owner.module())?,
                            &self.catalogs.callable_limits,
                        )
                        .map_err(|error| {
                            FinalSemanticAnalysisError::CallFactsSeal {
                                owner,
                                error: Box::new(error),
                            }
                        })?;
                        pending.push(PendingFinalCall {
                            owner,
                            facts,
                            selected: Some(update),
                        });
                    }
                    DetachedAnalyzerCallPayload::SelectedContinuation { selected, result } => {
                        let sealed = seal_selected_call(
                            FinalCallSealLocation::Site(site),
                            site,
                            selected,
                            crate::callable::CheckedCallResultSeal::Continuation {
                                prepared: result,
                            },
                            &dependencies,
                            &authority,
                            &mut definitions,
                            &mut bases,
                            &applications,
                            coordinates,
                            expressions,
                            locals,
                            checked_callables,
                            &self.catalogs.callable_limits,
                        )?;
                        let callee = sealed.callee_update(expressions, checked_callables)?;
                        let update = PendingSelectedExpressionUpdate {
                            resolution: sealed.expression_resolution,
                            result: checked_call_result_schema(&sealed.application)?,
                            effects: sealed.application.core().effects().concrete().clone(),
                            callee,
                        };
                        applications.insert(key, sealed.application.clone());
                        let facts = crate::callable::CallTargetFacts::try_new(
                            crate::callable::CallTargetFactsInput {
                                enclosing_callable: sealed.enclosing_callable,
                                outcome: crate::callable::CallAnalysisOutcome::Selected(
                                    sealed.application,
                                ),
                                accounting: sealed.accounting,
                            },
                            self.module(owner.module())?,
                            &self.catalogs.callable_limits,
                        )
                        .map_err(|error| {
                            FinalSemanticAnalysisError::CallFactsSeal {
                                owner,
                                error: Box::new(error),
                            }
                        })?;
                        pending.push(PendingFinalCall {
                            owner,
                            facts,
                            selected: Some(update),
                        });
                    }
                    DetachedAnalyzerCallPayload::Unselected(value) => {
                        let sealed = seal_unselected_call(
                            FinalCallSealLocation::Site(site),
                            site,
                            value,
                            &dependencies,
                            &authority,
                            &mut definitions,
                            &mut bases,
                            &applications,
                            coordinates,
                            expressions,
                            locals,
                            &self.catalogs.callable_limits,
                        )?;
                        let facts = crate::callable::CallTargetFacts::try_new(
                            crate::callable::CallTargetFactsInput {
                                enclosing_callable: sealed.enclosing_callable,
                                outcome: sealed.outcome,
                                accounting: sealed.accounting,
                            },
                            self.module(owner.module())?,
                            &self.catalogs.callable_limits,
                        )
                        .map_err(|error| {
                            FinalSemanticAnalysisError::CallFactsSeal {
                                owner,
                                error: Box::new(error),
                            }
                        })?;
                        pending.push(PendingFinalCall {
                            owner,
                            facts,
                            selected: None,
                        });
                    }
                }
            }
        }
        definitions
            .finish()
            .map_err(|error| final_call_seal_error(FinalCallSealLocation::Graph, error))?;
        for pending in pending {
            self.control.check()?;
            if let Some(update) = pending.selected {
                if let Some((callee, updated)) = update.callee {
                    self.facts
                        .replace_existing_expression(callee, updated)
                        .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                }
                let previous = self
                    .facts
                    .expressions()
                    .get(&pending.owner)
                    .cloned()
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: pending.owner,
                    })?;
                let updated = match update.resolution {
                    AnalyzerPreparedExpressionResolution::Complete(resolution) => {
                        let crate::callable::CallableResultSchema::Value(result) = update.result
                        else {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                        };
                        match previous {
                            PreparedExpressionFact::CompileTimeScalar(prepared) => {
                                let (shell, scalar, original) = prepared.into_parts();
                                let original = CheckedExpression::value(
                                    result,
                                    original.required_type_selection(pending.owner)?,
                                    update.effects,
                                    resolution,
                                );
                                PreparedExpressionFact::from(
                                    crate::final_analysis::PreparedCompileTimeScalarExpression::try_new(
                                        shell,
                                        scalar,
                                        original.into(),
                                    )
                                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
                                )
                            }
                            previous => CheckedExpression::value(
                                result,
                                previous.required_type_selection(pending.owner)?,
                                update.effects,
                                resolution,
                            )
                            .into(),
                        }
                    }
                    AnalyzerPreparedExpressionResolution::DialogueApplication => {
                        let PreparedExpressionFact::DialogueApplication(prepared) = previous else {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                        };
                        let crate::callable::CallableResultSchema::Value(result) = &update.result
                        else {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                        };
                        if prepared.shell().value_type() != Some(result) {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                        }
                        PreparedExpressionFact::DialogueApplication(prepared)
                    }
                    AnalyzerPreparedExpressionResolution::ContentApplication => {
                        let PreparedExpressionFact::ContentApplication(prepared) = previous else {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                        };
                        match (&update.result, prepared.shell().result()) {
                            (
                                crate::callable::CallableResultSchema::Value(result),
                                super::super::prepared::PreparedExpressionResult::Value(value),
                            ) if value.ty() == result => {}
                            (
                                crate::callable::CallableResultSchema::ContentEmission(operation),
                                super::super::prepared::PreparedExpressionResult::NonValue(
                                    super::super::prepared::PreparedNonValueExpressionResult::ContentEmission(
                                        actual,
                                    ),
                                ),
                            ) if operation == actual => {}
                            _ => return Err(FinalSemanticAnalysisError::WrongPayloadFamily),
                        }
                        PreparedExpressionFact::ContentApplication(prepared)
                    }
                };
                self.facts
                    .replace_existing_expression(pending.owner, updated)
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            }
            if self
                .facts
                .publish_final_call_fact(pending.owner, pending.facts)
                .map_err(FinalSemanticAnalysisError::from)?
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
        Ok(())
    }
}

fn stable_identity_seal(
    location: FinalCallSealLocation,
    definition: &crate::callable::PreparedResolvedCallableDefinitionSealInput,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    locals: &BTreeMap<LocalId, TypeKind>,
) -> Result<ResolvedCallableStableIdentitySeal, FinalSemanticAnalysisError> {
    let effects = || {
        definition
            .schema()
            .effects()
            .fixed_row()
            .cloned()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)
    };
    match definition.identity() {
        PreparedResolvedCallableIdentity::Catalog(_) => {
            Ok(ResolvedCallableStableIdentitySeal::Catalog)
        }
        PreparedResolvedCallableIdentity::Language(_) => {
            Ok(ResolvedCallableStableIdentitySeal::Language)
        }
        PreparedResolvedCallableIdentity::Lexical { local } => {
            let binding = coordinates.binding_evidence(*local).map_err(|_| {
                final_call_seal_error(
                    location,
                    CallConstraintInvariant::MissingCheckedBindingCoordinate { owner: *local },
                )
            })?;
            Ok(ResolvedCallableStableIdentitySeal::Lexical {
                binding,
                effects: effects()?,
            })
        }
        PreparedResolvedCallableIdentity::FunctionValue {
            producer: PreparedFunctionValueOriginIdentity::IndependentExpression { producer },
            ordinal,
            captures,
        } => {
            let expression = coordinates.expression_evidence(*producer).map_err(|_| {
                final_call_seal_error(
                    location,
                    CallConstraintInvariant::MissingCheckedExpressionCoordinate {
                        owner: *producer,
                    },
                )
            })?;
            let function_type = expressions
                .get(producer)
                .and_then(PreparedExpressionFact::value_type)
                .filter(|ty| matches!(ty, TypeKind::Function { .. }))
                .cloned()
                .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                    owner: *producer,
                })?;
            let captures = captures
                .iter()
                .map(|capture| {
                    let local = capture.local();
                    Ok::<CheckedCaptureSignatureSeal, FinalSemanticAnalysisError>(
                        CheckedCaptureSignatureSeal {
                            binding: coordinates.binding_evidence(local).map_err(|_| {
                                final_call_seal_error(
                                    location,
                                    CallConstraintInvariant::MissingCheckedBindingCoordinate {
                                        owner: local,
                                    },
                                )
                            })?,
                            mode: capture.mode(),
                            ty: locals.get(&local).cloned().ok_or(
                                FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local },
                            )?,
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(ResolvedCallableStableIdentitySeal::FunctionValue {
                expression,
                ordinal: *ordinal,
                function_type,
                effects: effects()?,
                captures,
            })
        }
    }
}

fn final_call_seal_error(
    location: FinalCallSealLocation,
    failure: CallConstraintInvariant,
) -> FinalSemanticAnalysisError {
    FinalSemanticAnalysisError::CallSeal(FinalCallSealFailure::new(location, failure))
}
