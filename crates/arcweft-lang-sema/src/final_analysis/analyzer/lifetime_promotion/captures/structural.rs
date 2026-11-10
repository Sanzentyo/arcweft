//! Follow the stored sources selected by a checked expression.

use arcweft_lang_hir::{
    expr::{HirComputationBlockExpr, HirComputationBlockKind, HirExprKind},
    identity::ExprId,
    module::HirModule,
    project::{HirControlTransferTarget, HirLoopTargetFamily, HirSemanticBodyOwner},
};

use crate::final_analysis::{
    CheckedExpression, CheckedFieldReceiver, CheckedMatchArmFact, PreparedExpressionFact,
    prepared::{PreparedProjectRecordExpression, PreparedRecordValueSource},
};
use crate::types::{ArrayLength, TypeKind};

use super::{
    Analyzer, BorrowDependencies, CaptureWorklist, CarrierStorage, FinalSemanticAnalysisError,
    Observation, ReceiverBorrowQuery, StorageFrame, StorageProjection, ValueOrigin,
    worklist::PositionalStorage,
};

impl Analyzer<'_, '_, '_> {
    pub(super) fn structural_dependencies(
        &self,
        producer: ExprId,
        origin: ValueOrigin,
        (module, frame): (&HirModule, &StorageFrame),
        work: &mut CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let fact = self
            .storage_expression_fact(producer, frame)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: producer })?;
        if fact.creates_implicit_callable() {
            return Ok(BorrowDependencies::default());
        }
        match fact {
            PreparedExpressionFact::ProjectField(field) => {
                let mut observation = origin.observation;
                for (nominal, ordinal) in field.declaration_fields().rev() {
                    observation = work.project(
                        StorageProjection::Field {
                            nominal: nominal.clone(),
                            ordinal,
                        },
                        observation,
                        receiver,
                    )?;
                }
                return self.stored_source_dependencies(
                    (field.receiver(), observation),
                    origin,
                    (module, frame),
                    work,
                    receiver,
                );
            }
            PreparedExpressionFact::ProjectRecord(record) => {
                return self.record_storage_dependencies(
                    record,
                    origin,
                    (module, frame),
                    work,
                    receiver,
                );
            }
            _ => {}
        }

        let expression = module
            .resolve_expr(producer)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let mut dependencies = BorrowDependencies::default();
        let values = match expression.kind() {
            HirExprKind::Try(tried) => {
                let carrier = fact
                    .try_carrier()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let observation = work.project(
                    StorageProjection::Carrier(CarrierStorage::success(carrier)),
                    origin.observation,
                    receiver,
                )?;
                return work.demand(
                    ValueOrigin {
                        expression: tried.operand(),
                        observation,
                        ..origin
                    },
                    origin,
                    receiver,
                );
            }
            HirExprKind::ComputationBlock(block) if block.kind().is_carrier() => {
                return self.carrier_storage_dependencies(
                    (producer, block),
                    origin,
                    module,
                    work,
                    receiver,
                );
            }
            HirExprKind::Tuple(tuple) => {
                return positional_storage_dependencies(
                    tuple.elements(),
                    PositionalStorage::Tuple,
                    origin,
                    work,
                    receiver,
                );
            }
            HirExprKind::BracketSequence(sequence) => {
                return positional_storage_dependencies(
                    sequence.elements(),
                    PositionalStorage::Sequence,
                    origin,
                    work,
                    receiver,
                );
            }
            HirExprKind::ArrayRepeat(repeat) => {
                let (ordinal, observation) = match work.selected_storage(origin.observation)? {
                    Some((
                        StorageProjection::Element {
                            family: PositionalStorage::Sequence,
                            ordinal,
                        },
                        remaining,
                    ))
                    | Some((StorageProjection::SequenceTail(ordinal), remaining)) => {
                        (*ordinal, remaining)
                    }
                    None if origin.observation == Observation::Stored => (0, Observation::Stored),
                    _ => return Ok(BorrowDependencies::default()),
                };
                if matches!(fact.value_type(), Some(TypeKind::Array { len: ArrayLength::Const(len), .. }) if ordinal as usize >= *len)
                {
                    return Ok(BorrowDependencies::default());
                }
                return work.demand(
                    ValueOrigin {
                        expression: repeat.value(),
                        observation,
                        ..origin
                    },
                    origin,
                    receiver,
                );
            }
            HirExprKind::Block(block) => vec![block.tail()],
            HirExprKind::NamedBlock(block) => vec![block.tail()],
            HirExprKind::If(branches) => vec![branches.else_branch(), branches.then_branch()],
            HirExprKind::IfLet(branches) => vec![branches.else_branch(), branches.then_branch()],
            HirExprKind::Match(_) => {
                let matched = fact
                    .complete()
                    .and_then(CheckedExpression::match_fact)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                receiver.charge(matched.arms().len() as u64)?;
                matched
                    .arms()
                    .iter()
                    .rev()
                    .map(CheckedMatchArmFact::value)
                    .collect()
            }
            HirExprKind::Loop(body) => self
                .checked_body_value_sources(
                    module,
                    body.statements(),
                    body.tail(),
                    |target| {
                        matches!(
                            target,
                            HirControlTransferTarget::Loop {
                                family: HirLoopTargetFamily::LoopExpression, body_owner, ..
                            } if *body_owner == HirSemanticBodyOwner::direct_expression(producer)
                        )
                    },
                    false,
                )
                .map_err(|error| error.into_public(producer))?,
            _ => Vec::new(),
        };
        for expression in values {
            dependencies.include(work.demand(
                ValueOrigin {
                    expression,
                    ..origin
                },
                origin,
                receiver,
            )?);
            if dependencies.is_borrowed() {
                break;
            }
        }
        Ok(dependencies)
    }

    fn carrier_storage_dependencies(
        &self,
        (producer, block): (ExprId, &HirComputationBlockExpr),
        origin: ValueOrigin,
        module: &HirModule,
        work: &mut CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let success = match block.kind() {
            HirComputationBlockKind::Option => CarrierStorage::OptionItem,
            HirComputationBlockKind::Result => CarrierStorage::ResultOk,
            _ => return Err(FinalSemanticAnalysisError::WrongPayloadFamily),
        };
        let normal = work.carrier_payload(origin.observation, success)?;
        let residual = if success == CarrierStorage::ResultOk {
            work.carrier_payload(origin.observation, CarrierStorage::ResultErr)?
        } else {
            None
        };
        let mut dependencies = BorrowDependencies::default();
        if normal.is_none() && residual.is_none() {
            return Ok(dependencies);
        }
        let sources = self
            .checked_carrier_value_sources(module, producer, block)
            .map_err(|error| error.into_public(producer))?;
        if let Some((expression, observation)) = sources.success.zip(normal) {
            dependencies.include(work.demand(
                ValueOrigin {
                    expression,
                    observation,
                    ..origin
                },
                origin,
                receiver,
            )?);
        }
        if let Some(remaining) = residual
            && !sources.residuals.is_empty()
            && !dependencies.is_borrowed()
        {
            let observation = work.project(
                StorageProjection::Carrier(CarrierStorage::ResultErr),
                remaining,
                receiver,
            )?;
            for expression in sources.residuals {
                dependencies.include(work.demand(
                    ValueOrigin {
                        expression,
                        observation,
                        ..origin
                    },
                    origin,
                    receiver,
                )?);
                if dependencies.is_borrowed() {
                    break;
                }
            }
        }
        Ok(dependencies)
    }

    fn record_storage_dependencies(
        &self,
        record: &PreparedProjectRecordExpression,
        origin: ValueOrigin,
        context: (&HirModule, &StorageFrame),
        work: &mut CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let (ordinal, observation) = match work.selected_storage(origin.observation)? {
            Some((StorageProjection::Field { nominal, ordinal }, remaining))
                if nominal == record.nominal().declaration() =>
            {
                (Some(*ordinal), remaining)
            }
            None if origin.observation == Observation::Stored => (None, Observation::Stored),
            _ => return Ok(BorrowDependencies::default()),
        };
        let mut dependencies = BorrowDependencies::default();
        for field in record.fields() {
            receiver.charge(1)?;
            if ordinal.is_some_and(|ordinal| ordinal != field.declaration_ordinal()) {
                continue;
            }
            let source = match field.source() {
                PreparedRecordValueSource::Expression(expression) => {
                    CheckedFieldReceiver::Expression(expression)
                }
                PreparedRecordValueSource::Local(local) => CheckedFieldReceiver::Binding(local),
            };
            dependencies.include(self.stored_source_dependencies(
                (source, observation),
                origin,
                context,
                work,
                receiver,
            )?);
            if dependencies.is_borrowed() {
                break;
            }
        }
        Ok(dependencies)
    }

    fn stored_source_dependencies(
        &self,
        (source, observation): (CheckedFieldReceiver, Observation),
        origin: ValueOrigin,
        (module, frame): (&HirModule, &StorageFrame),
        work: &mut CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        match source {
            CheckedFieldReceiver::Expression(expression) => work.demand(
                ValueOrigin {
                    expression,
                    observation,
                    ..origin
                },
                origin,
                receiver,
            ),
            CheckedFieldReceiver::Binding(local) => self.stored_local_dependencies(
                module,
                origin,
                (local, observation),
                frame,
                work,
                receiver,
            ),
        }
    }
}

fn positional_storage_dependencies(
    elements: &[ExprId],
    family: PositionalStorage,
    origin: ValueOrigin,
    work: &mut CaptureWorklist,
    receiver: &mut ReceiverBorrowQuery<'_>,
) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
    let (elements, observation) = match work.selected_storage(origin.observation)? {
        Some((
            StorageProjection::Element {
                family: selected,
                ordinal,
            },
            remaining,
        )) if *selected == family => (
            elements
                .get(*ordinal as usize)
                .map_or(&[][..], std::slice::from_ref),
            remaining,
        ),
        Some((StorageProjection::SequenceTail(start), remaining))
            if family == PositionalStorage::Sequence =>
        {
            (elements.get(*start as usize..).unwrap_or(&[]), remaining)
        }
        None if origin.observation == Observation::Stored => (elements, Observation::Stored),
        _ => return Ok(BorrowDependencies::default()),
    };
    let mut dependencies = BorrowDependencies::default();
    for &expression in elements {
        dependencies.include(work.demand(
            ValueOrigin {
                expression,
                observation,
                ..origin
            },
            origin,
            receiver,
        )?);
        if dependencies.is_borrowed() {
            break;
        }
    }
    Ok(dependencies)
}
