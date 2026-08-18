//! Runtime-expression seed projection from accepted final HIR.

use std::collections::BTreeMap;

use arcweft_core::entry::RuntimeCallableId;
use arcweft_core::plan::{
    RuntimeAgentExprSeed, RuntimeCallArgumentSeed, RuntimeExprMatchArmSeed, RuntimeExprSeed,
    RuntimeExprSeedKind, RuntimeFieldProjectionSeed, RuntimeFlowOpSeed, RuntimeFunctionSiteSeedId,
    RuntimeHostArgumentSeed, RuntimeHostCallTargetSeed, RuntimeLocalSeedId,
    RuntimeNominalRecordFieldSeed, RuntimePureHelperSeedId, RuntimeRecordFieldSeedId,
    RuntimeTraitMethodSeedId,
};
use arcweft_core::task::NamedHostArg;
use arcweft_core::value::{
    RuntimeAgentCompareOp, RuntimeBinaryOp, RuntimeCallArgumentMode, RuntimeCallTarget,
    RuntimeUnaryOp, RuntimeValue,
};
use arcweft_lang_hir::expr::{
    HirBinaryOp, HirCallArgument, HirExprKind, HirRecordField, HirUnaryOp,
};
use arcweft_lang_hir::identity::{ExprId, LocalId, StmtId};
use arcweft_lang_hir::item::HirFunctionBody;
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_hir::stmt::HirStmtKind;
use arcweft_lang_hir::symbol::ImplMethodDeclarationId;

use crate::agent::RuntimeAgentIntrinsic;
use crate::final_pattern::{FinalPatternLowerer, project_entity_reference};
use crate::semantic_facts::{
    RuntimeNormalizedType, RuntimePlanSemanticFacts, RuntimeReductionConstructor,
    RuntimeResolvedCallArgument, RuntimeResolvedCallTarget, RuntimeResolvedHostArgumentPassing,
    RuntimeResolvedSelect, RuntimeResolvedValue, RuntimeResolvedVariant, RuntimeTypeShape,
};

pub(crate) struct FinalExprLowerer<'hir> {
    module: &'hir HirModule,
    facts: &'hir RuntimePlanSemanticFacts,
    locals: &'hir BTreeMap<LocalId, RuntimeLocalSeedId>,
    pure_helpers: &'hir BTreeMap<RuntimeCallableId, RuntimePureHelperSeedId>,
    trait_methods: &'hir BTreeMap<ImplMethodDeclarationId, RuntimeTraitMethodSeedId>,
    function_sites: &'hir BTreeMap<ExprId, RuntimeFunctionSiteSeedId>,
    overrides: BTreeMap<ExprId, RuntimeExprSeed>,
}

impl<'hir> FinalExprLowerer<'hir> {
    pub(crate) const fn new(
        module: &'hir HirModule,
        facts: &'hir RuntimePlanSemanticFacts,
        locals: &'hir BTreeMap<LocalId, RuntimeLocalSeedId>,
        pure_helpers: &'hir BTreeMap<RuntimeCallableId, RuntimePureHelperSeedId>,
        trait_methods: &'hir BTreeMap<ImplMethodDeclarationId, RuntimeTraitMethodSeedId>,
        function_sites: &'hir BTreeMap<ExprId, RuntimeFunctionSiteSeedId>,
    ) -> Self {
        Self {
            module,
            facts,
            locals,
            pure_helpers,
            trait_methods,
            function_sites,
            overrides: BTreeMap::new(),
        }
    }

    pub(crate) fn with_overrides(mut self, overrides: BTreeMap<ExprId, RuntimeExprSeed>) -> Self {
        self.overrides = overrides;
        self
    }

    pub(crate) fn lower_host_call_target(
        &self,
        id: ExprId,
        call: &arcweft_lang_hir::expr::HirCallExpr,
    ) -> Result<Option<RuntimeHostCallTargetSeed>, String> {
        let selected = self
            .facts
            .call(id)
            .ok_or_else(|| format!("checked call fact is missing for expression {id:?}"))?;
        let RuntimeResolvedCallTarget::Host(host) = selected.target() else {
            return Ok(None);
        };
        let args = selected
            .arguments()
            .iter()
            .map(|argument| match argument {
                RuntimeResolvedCallArgument::Authored { passing, .. } => {
                    let (value, _) = self.resolved_argument(call, id, argument)?;
                    Ok(match passing {
                        RuntimeResolvedHostArgumentPassing::Positional => {
                            RuntimeHostArgumentSeed::Positional(value)
                        }
                        RuntimeResolvedHostArgumentPassing::Named(name) => {
                            RuntimeHostArgumentSeed::Named(NamedHostArg {
                                name: name.clone(),
                                value,
                            })
                        }
                        RuntimeResolvedHostArgumentPassing::Spread => {
                            RuntimeHostArgumentSeed::Spread(value)
                        }
                    })
                }
                RuntimeResolvedCallArgument::Receiver => Err(format!(
                    "host call {id:?} cannot project a receiver argument"
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(RuntimeHostCallTargetSeed {
            public_id: host.public_id().to_owned(),
            capability: host.capability().to_owned(),
            operation: host.operation().to_owned(),
            args,
            mode: host.mode(),
            deterministic: host.deterministic(),
        }))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one closed final-HIR expression projection"
    )]
    pub(crate) fn lower(&self, id: ExprId) -> Result<RuntimeExprSeed, String> {
        if let Some(value) = self.overrides.get(&id) {
            return Ok(value.clone());
        }
        let expression = self
            .module
            .resolve_expr(id)
            .map_err(|error| format!("cannot resolve final-HIR expression {id:?}: {error}"))?;
        if expression.is_poisoned() {
            return Err(format!(
                "final-HIR expression {id:?} contains recovery and is not executable"
            ));
        }
        let kind = match expression.kind() {
            HirExprKind::Unit => RuntimeExprSeedKind::Value(RuntimeValue::Unit),
            HirExprKind::Literal(_) | HirExprKind::NumericBracketSequence(_) => {
                RuntimeExprSeedKind::Value(self.facts.expression_literal(id).cloned().ok_or_else(
                    || format!("checked literal fact is missing for expression {id:?}"),
                )?)
            }
            HirExprKind::EntityReference(_) => {
                RuntimeExprSeedKind::EntityRef(self.entity_reference(id)?)
            }
            HirExprKind::Path(_) if self.facts.expression_variant(id).is_some() => {
                self.lower_unit_variant(id)?
            }
            HirExprKind::Path(_) => self.lower_path(id)?,
            HirExprKind::ShortVariant(_) => self.lower_unit_variant(id)?,
            HirExprKind::Tuple(tuple) => RuntimeExprSeedKind::Tuple(
                tuple
                    .elements()
                    .iter()
                    .map(|element| self.lower(*element))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            ),
            HirExprKind::BracketSequence(sequence) => RuntimeExprSeedKind::BracketSeq(
                sequence
                    .elements()
                    .iter()
                    .map(|element| self.lower(*element))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            ),
            HirExprKind::ArrayRepeat(repeat) => RuntimeExprSeedKind::RepeatSeq {
                value: Box::new(self.lower(repeat.value())?),
                len: self.lower_constant_length(repeat.length())?,
            },
            HirExprKind::Call(call) => self.lower_call(id, call)?,
            HirExprKind::Select(select) => self.lower_select(id, select.target())?,
            HirExprKind::Range(range) => RuntimeExprSeedKind::Range {
                start: range
                    .start()
                    .map(|value| self.lower(value))
                    .transpose()?
                    .map(Box::new),
                end: range
                    .end()
                    .map(|value| self.lower(value))
                    .transpose()?
                    .map(Box::new),
                inclusive: range.inclusive(),
            },
            HirExprKind::RecordLiteral(_) => {
                return Err(format!(
                    "structural record expression {id:?} has no closed runtime seed variant"
                ));
            }
            HirExprKind::Record(record) => RuntimeExprSeedKind::NominalRecord(
                self.lower_nominal_fields(id, record.fields())?
                    .into_boxed_slice(),
            ),
            HirExprKind::Binary(binary) => RuntimeExprSeedKind::Binary {
                lhs: Box::new(self.lower(binary.left())?),
                op: runtime_binary(binary.operator()).ok_or_else(|| {
                    format!(
                        "binary operator {:?} at {id:?} has no runtime expression representation",
                        binary.operator()
                    )
                })?,
                rhs: Box::new(self.lower(binary.right())?),
            },
            HirExprKind::Unary(unary) => RuntimeExprSeedKind::Unary {
                op: match unary.operator() {
                    HirUnaryOp::Not => RuntimeUnaryOp::Not,
                    HirUnaryOp::Negate => RuntimeUnaryOp::Neg,
                },
                expr: Box::new(self.lower(unary.operand())?),
            },
            HirExprKind::Closure(_) => {
                RuntimeExprSeedKind::Function(self.function_sites.get(&id).cloned().ok_or_else(
                    || format!("builder-issued function site seed is missing for closure {id:?}"),
                )?)
            }
            HirExprKind::Block(block) => {
                return self.lower_block(id, block.statements(), block.tail());
            }
            HirExprKind::ComputationBlock(block) => {
                return self.lower_block(id, block.statements(), block.tail());
            }
            HirExprKind::NamedBlock(block) => {
                return self.lower_block(id, block.statements(), block.tail());
            }
            HirExprKind::If(branch) => RuntimeExprSeedKind::If {
                condition: Box::new(self.lower(branch.condition())?),
                then_expr: Box::new(self.lower(branch.then_branch())?),
                else_expr: Box::new(self.lower(branch.else_branch())?),
            },
            HirExprKind::IfLet(branch) => RuntimeExprSeedKind::IfLet {
                pattern: self.pattern().lower(branch.pattern())?,
                expr: Box::new(self.lower(branch.scrutinee())?),
                guard: branch
                    .guard()
                    .map(|guard| self.lower(guard))
                    .transpose()?
                    .map(Box::new),
                then_expr: Box::new(self.lower(branch.then_branch())?),
                else_expr: Box::new(self.lower(branch.else_branch())?),
            },
            HirExprKind::Match(matched) => RuntimeExprSeedKind::Match {
                scrutinee: Box::new(self.lower(matched.scrutinee())?),
                arms: matched
                    .arms()
                    .iter()
                    .map(|arm| {
                        Ok(RuntimeExprMatchArmSeed::new(
                            self.pattern().lower(arm.pattern())?,
                            arm.guard().map(|guard| self.lower(guard)).transpose()?,
                            self.lower(arm.value())?,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?
                    .into_boxed_slice(),
            },
            HirExprKind::PostfixBracket(_) => {
                return self.lower(self.facts.postfix_candidate(id).ok_or_else(|| {
                    format!("checked postfix candidate is missing for expression {id:?}")
                })?);
            }
            HirExprKind::Index(index) => RuntimeExprSeedKind::Call {
                callee: RuntimeCallTarget::intrinsic(
                    arcweft_core::value::RuntimeIntrinsic::CoreIndex,
                ),
                args: vec![
                    RuntimeCallArgumentSeed::new(
                        self.lower(index.target())?,
                        RuntimeCallArgumentMode::Value,
                    ),
                    RuntimeCallArgumentSeed::new(
                        self.lower(index.index())?,
                        RuntimeCallArgumentMode::Value,
                    ),
                ]
                .into_boxed_slice(),
            },
            HirExprKind::Placeholder(_)
            | HirExprKind::Pipe(_)
            | HirExprKind::Try(_)
            | HirExprKind::Await(_)
            | HirExprKind::Loop(_)
            | HirExprKind::Thread(_)
            | HirExprKind::Choice(_)
            | HirExprKind::Borrow(_)
            | HirExprKind::Dereference(_)
            | HirExprKind::DialogueContentApplication(_)
            | HirExprKind::Error(_)
            | HirExprKind::ForSynthetic(_)
            | HirExprKind::LifetimePath(_) => {
                return Err(format!(
                    "final-HIR expression family {:?} at {id:?} is not a pure runtime expression",
                    expression.kind()
                ));
            }
        };
        Ok(RuntimeExprSeed::new(self.expression_type(id)?, kind))
    }

    pub(crate) fn lower_function_body(
        &self,
        body: &HirFunctionBody,
    ) -> Result<RuntimeExprSeed, String> {
        match body {
            HirFunctionBody::Block {
                statements, tail, ..
            } => self.lower_block(*tail, statements, *tail),
            HirFunctionBody::Error(expression) => Err(format!(
                "recovered ordinary-function body {expression:?} cannot enter runtime lowering"
            )),
        }
    }

    pub(crate) fn lower_assignment(
        &self,
        statement: StmtId,
        value: ExprId,
        body: RuntimeExprSeed,
    ) -> Result<RuntimeExprSeed, String> {
        let (local, owner, field) = self.assignment_parts(statement)?;
        Ok(RuntimeExprSeed::new(
            body.ty(),
            RuntimeExprSeedKind::AssignNominalField {
                base: self.local(local)?,
                owner,
                field,
                expr: Box::new(self.lower(value)?),
                body: Box::new(body),
            },
        ))
    }

    pub(crate) fn lower_flow_assignment(
        &self,
        statement: StmtId,
        value: ExprId,
    ) -> Result<RuntimeFlowOpSeed, String> {
        let (local, owner, field) = self.assignment_parts(statement)?;
        Ok(RuntimeFlowOpSeed::AssignNominalField {
            base: self.local(local)?,
            owner,
            field,
            value: self.lower(value)?,
        })
    }

    fn assignment_parts(
        &self,
        statement: StmtId,
    ) -> Result<
        (
            LocalId,
            arcweft_core::pattern::RuntimeSemanticTypeId,
            RuntimeRecordFieldSeedId,
        ),
        String,
    > {
        let assignment = self.facts.assignment(statement).ok_or_else(|| {
            format!("checked assignment fact is missing for statement {statement:?}")
        })?;
        Ok((
            assignment.base(),
            assignment.nominal().identity(),
            RuntimeRecordFieldSeedId::from_zero_based(assignment.field_ordinal()),
        ))
    }

    fn lower_path(&self, id: ExprId) -> Result<RuntimeExprSeedKind, String> {
        match self
            .facts
            .value(id)
            .ok_or_else(|| format!("checked value fact is missing for expression {id:?}"))?
        {
            RuntimeResolvedValue::Local(local) => {
                Ok(RuntimeExprSeedKind::Local(self.local(*local)?))
            }
            RuntimeResolvedValue::Constant(value) => Ok(RuntimeExprSeedKind::Value(value.clone())),
            RuntimeResolvedValue::Intrinsic(_)
            | RuntimeResolvedValue::ProjectCallable(_)
            | RuntimeResolvedValue::ProjectItem(_)
            | RuntimeResolvedValue::DialogueLine(_)
            | RuntimeResolvedValue::Registered(_) => Err(format!(
                "resolved callable/project value at {id:?} requires a builder-issued runtime function-value identity"
            )),
        }
    }

    fn lower_unit_variant(&self, id: ExprId) -> Result<RuntimeExprSeedKind, String> {
        let selected = self
            .facts
            .expression_variant(id)
            .ok_or_else(|| format!("checked variant fact is missing for expression {id:?}"))?;
        if selected
            .selected_payload_type()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Err(format!(
                "payload-bearing variant at {id:?} was selected without a payload"
            ));
        }
        Ok(RuntimeExprSeedKind::Variant {
            ordinal: selected
                .checked_selection()
                .map_err(|error| error.to_string())?
                .ordinal(),
            payload: None,
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the closed call target projection remains together"
    )]
    fn lower_call(
        &self,
        id: ExprId,
        call: &arcweft_lang_hir::expr::HirCallExpr,
    ) -> Result<RuntimeExprSeedKind, String> {
        let selected = self
            .facts
            .call(id)
            .ok_or_else(|| format!("checked call fact is missing for expression {id:?}"))?;
        let arguments = selected
            .arguments()
            .iter()
            .map(|argument| self.lower_call_argument(call, id, argument))
            .collect::<Result<Vec<_>, _>>()?;
        match selected.target() {
            RuntimeResolvedCallTarget::Intrinsic(intrinsic) => Ok(RuntimeExprSeedKind::Call {
                callee: RuntimeCallTarget::intrinsic(*intrinsic),
                args: arguments.into_boxed_slice(),
            }),
            RuntimeResolvedCallTarget::Agent(intrinsic) => {
                self.lower_agent_intrinsic(id, call, *intrinsic)
            }
            RuntimeResolvedCallTarget::AgentProbeComparison(operation) => {
                self.lower_agent_compare(id, call, *operation)
            }
            RuntimeResolvedCallTarget::AgentDiagnosticsHasError => Ok(RuntimeExprSeedKind::Agent(
                RuntimeAgentExprSeed::PredicateDiagnosticsHasError {
                    diagnostics: Box::new(
                        self.value_arguments(call, id, 1)?
                            .pop()
                            .expect("one argument"),
                    ),
                },
            )),
            RuntimeResolvedCallTarget::Declaration(callable) => Ok(RuntimeExprSeedKind::PureCall {
                helper: self
                    .pure_helpers
                    .get(callable.runtime())
                    .cloned()
                    .ok_or_else(|| {
                        format!(
                            "builder-issued pure-helper seed is missing for callable {:?}",
                            callable.runtime()
                        )
                    })?,
                args: arguments.into_boxed_slice(),
            }),
            RuntimeResolvedCallTarget::Variant(variant) => {
                self.validate_variant_call_payload(id, call, selected.arguments(), variant)?;
                let payload = match arguments.len() {
                    0 => None,
                    1 => Some(Box::new(
                        self.value_arguments(call, id, 1)?
                            .pop()
                            .expect("one argument"),
                    )),
                    _ => Some(Box::new(RuntimeExprSeed::new(
                        self.expression_type(id)?,
                        RuntimeExprSeedKind::Tuple(
                            self.value_arguments(call, id, arguments.len())?
                                .into_boxed_slice(),
                        ),
                    ))),
                };
                Ok(RuntimeExprSeedKind::Variant {
                    ordinal: variant
                        .checked_selection()
                        .map_err(|error| error.to_string())?
                        .ordinal(),
                    payload,
                })
            }
            RuntimeResolvedCallTarget::Reduction(RuntimeReductionConstructor::Unchanged) => {
                Ok(RuntimeExprSeedKind::ReductionUnchanged {
                    state: Box::new(
                        self.value_arguments(call, id, 1)?
                            .pop()
                            .expect("one argument"),
                    ),
                })
            }
            RuntimeResolvedCallTarget::Registered(registered) => Ok(RuntimeExprSeedKind::Call {
                callee: RuntimeCallTarget::callable(registered.clone()),
                args: arguments.into_boxed_slice(),
            }),
            RuntimeResolvedCallTarget::FunctionValue => Ok(RuntimeExprSeedKind::Apply {
                callee: Box::new(self.lower(call.callee().value_expression().ok_or_else(
                    || format!("function-value call {id:?} has no final-HIR value callee"),
                )?)?),
                args: arguments.into_boxed_slice(),
            }),
            RuntimeResolvedCallTarget::TraitMethod { method, .. } => {
                let receiver_index = selected
                    .arguments()
                    .iter()
                    .position(|argument| matches!(argument, RuntimeResolvedCallArgument::Receiver))
                    .ok_or_else(|| format!("trait call {id:?} has no receiver projection"))?;
                let mut values = self.value_arguments(call, id, arguments.len())?;
                let receiver = values.remove(receiver_index);
                Ok(RuntimeExprSeedKind::TraitCall {
                    callable: self.trait_methods.get(method).cloned().ok_or_else(|| {
                        format!("builder-issued trait-method seed is missing for {method:?}")
                    })?,
                    receiver: Box::new(receiver),
                    args: values
                        .into_iter()
                        .map(|value| {
                            RuntimeCallArgumentSeed::new(value, RuntimeCallArgumentMode::Value)
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                })
            }
            RuntimeResolvedCallTarget::Host(_) => Err(format!(
                "host call {id:?} is effectful and cannot enter pure expression lowering"
            )),
        }
    }

    fn lower_call_argument(
        &self,
        call: &arcweft_lang_hir::expr::HirCallExpr,
        id: ExprId,
        resolved: &RuntimeResolvedCallArgument,
    ) -> Result<RuntimeCallArgumentSeed, String> {
        let (value, mode) = self.resolved_argument(call, id, resolved)?;
        Ok(RuntimeCallArgumentSeed::new(value, mode))
    }

    fn resolved_argument(
        &self,
        call: &arcweft_lang_hir::expr::HirCallExpr,
        id: ExprId,
        resolved: &RuntimeResolvedCallArgument,
    ) -> Result<(RuntimeExprSeed, RuntimeCallArgumentMode), String> {
        match resolved {
            RuntimeResolvedCallArgument::Authored { ordinal, .. } => {
                let ordinal = usize::try_from(*ordinal)
                    .map_err(|_| format!("call argument ordinal {ordinal} does not fit usize"))?;
                let argument = call.arguments().get(ordinal).ok_or_else(|| {
                    format!("call argument ordinal {ordinal} is absent at {id:?}")
                })?;
                Ok((
                    self.lower(argument.value())?,
                    if matches!(argument, HirCallArgument::Spread { .. }) {
                        RuntimeCallArgumentMode::Spread
                    } else {
                        RuntimeCallArgumentMode::Value
                    },
                ))
            }
            RuntimeResolvedCallArgument::Receiver => Ok((
                self.lower_call_receiver(call, id)?,
                RuntimeCallArgumentMode::Value,
            )),
        }
    }

    fn value_arguments(
        &self,
        call: &arcweft_lang_hir::expr::HirCallExpr,
        id: ExprId,
        expected: usize,
    ) -> Result<Vec<RuntimeExprSeed>, String> {
        let selected = self
            .facts
            .call(id)
            .ok_or_else(|| format!("checked call fact is missing for expression {id:?}"))?;
        let values = selected
            .arguments()
            .iter()
            .map(|resolved| self.resolved_argument(call, id, resolved))
            .collect::<Result<Vec<_>, _>>()?;
        if values.len() != expected
            || values
                .iter()
                .any(|(_, mode)| *mode != RuntimeCallArgumentMode::Value)
        {
            return Err(format!(
                "typed call {id:?} requires {expected} non-spread arguments"
            ));
        }
        Ok(values.into_iter().map(|(value, _)| value).collect())
    }

    fn lower_select(&self, id: ExprId, target_id: ExprId) -> Result<RuntimeExprSeedKind, String> {
        let target = Box::new(self.lower(target_id)?);
        match self
            .facts
            .select(id)
            .ok_or_else(|| format!("checked member fact is missing for expression {id:?}"))?
        {
            RuntimeResolvedSelect::Method { .. } => Err(format!(
                "bound method at {id:?} cannot execute outside its checked Call"
            )),
            RuntimeResolvedSelect::AgentField { field } => Ok(RuntimeExprSeedKind::Field {
                target,
                field: RuntimeFieldProjectionSeed::Agent(*field),
            }),
            RuntimeResolvedSelect::Field {
                nominal: Some(nominal),
                ordinal: Some(ordinal),
                ..
            } => Ok(RuntimeExprSeedKind::Field {
                target,
                field: RuntimeFieldProjectionSeed::Nominal {
                    owner: nominal.identity(),
                    field: RuntimeRecordFieldSeedId::from_zero_based(*ordinal),
                },
            }),
            RuntimeResolvedSelect::Field {
                nominal: Some(_),
                ordinal: None,
                ..
            } => Err(format!(
                "nominal field selection {id:?} has no accepted defining-order coordinate"
            )),
            RuntimeResolvedSelect::Field { nominal: None, .. } => Err(format!(
                "field selection {id:?} has no closed Agent/entity field coordinate in semantic facts"
            )),
            RuntimeResolvedSelect::TupleElement { ordinal } => {
                Ok(RuntimeExprSeedKind::ProjectTuple {
                    target,
                    ordinal: *ordinal,
                })
            }
            RuntimeResolvedSelect::RecordElement {
                nominal: Some(_),
                ordinal,
                ..
            } => Ok(RuntimeExprSeedKind::ProjectRecord {
                target,
                field: RuntimeRecordFieldSeedId::from_zero_based(*ordinal),
            }),
            RuntimeResolvedSelect::RecordElement { nominal: None, .. } => Err(format!(
                "record selection {id:?} has no accepted nominal field coordinate"
            )),
        }
    }

    fn lower_nominal_fields(
        &self,
        id: ExprId,
        fields: &[HirRecordField],
    ) -> Result<Vec<RuntimeNominalRecordFieldSeed>, String> {
        let nominal = self.facts.nominal_record(id).ok_or_else(|| {
            format!(
                "nominal record expression {id:?} requires a typed runtime nominal-expression owner"
            )
        })?;
        fields
            .iter()
            .map(|field| match field {
                HirRecordField::Explicit { name, value } => Ok(RuntimeNominalRecordFieldSeed::new(
                    Self::nominal_record_field(nominal, name.as_str(), id)?,
                    self.lower(*value)?,
                )),
                HirRecordField::Shorthand { name, local } => {
                    Ok(RuntimeNominalRecordFieldSeed::new(
                        Self::nominal_record_field(nominal, name.as_str(), id)?,
                        RuntimeExprSeed::new(
                            self.local_type(*local)?,
                            RuntimeExprSeedKind::Local(self.local(*local)?),
                        ),
                    ))
                }
                HirRecordField::Invalid { .. } => {
                    Err(format!("record expression {id:?} has an invalid field"))
                }
            })
            .collect()
    }

    fn lower_block(
        &self,
        owner: ExprId,
        statements: &[StmtId],
        tail: ExprId,
    ) -> Result<RuntimeExprSeed, String> {
        let body = statements
            .iter()
            .rev()
            .try_fold(self.lower(tail)?, |body, statement| {
                let statement_id = *statement;
                let statement = self.module.resolve_stmt(statement_id).map_err(|error| {
                    format!("cannot resolve block statement {statement_id:?}: {error}")
                })?;
                if statement.is_poisoned() {
                    return Err("recovered block statement is not executable".to_owned());
                }
                match statement.kind() {
                    HirStmtKind::Let {
                        pattern,
                        initializer,
                        ..
                    } => Ok(RuntimeExprSeed::new(
                        body.ty(),
                        RuntimeExprSeedKind::Let {
                            binding: self.simple_binding(*pattern)?,
                            expr: Box::new(self.lower(*initializer)?),
                            body: Box::new(body),
                        },
                    )),
                    HirStmtKind::Assign { value, .. } => {
                        self.lower_assignment(statement_id, *value, body)
                    }
                    other => Err(format!(
                        "statement {other:?} cannot be embedded in a pure runtime expression block"
                    )),
                }
            })?;
        Ok(RuntimeExprSeed::new(
            self.expression_type(owner)?,
            body.kind().clone(),
        ))
    }

    fn lower_agent_intrinsic(
        &self,
        id: ExprId,
        call: &arcweft_lang_hir::expr::HirCallExpr,
        intrinsic: RuntimeAgentIntrinsic,
    ) -> Result<RuntimeExprSeedKind, String> {
        if let Some(operation) = intrinsic.host_operation() {
            return Err(format!(
                "Agent host call {operation} at {id:?} cannot enter pure expression lowering"
            ));
        }
        let values = self.value_arguments(
            call,
            id,
            self.facts
                .call(id)
                .expect("selected call")
                .arguments()
                .len(),
        )?;
        let one = |values: &Vec<RuntimeExprSeed>| exact_one(id, values);
        Ok(RuntimeExprSeedKind::Agent(match intrinsic {
            RuntimeAgentIntrinsic::StatePath => RuntimeAgentExprSeed::StatePath {
                path: Box::new(one(&values)?),
            },
            RuntimeAgentIntrinsic::ObservationPath => RuntimeAgentExprSeed::ObservationPath {
                path: Box::new(one(&values)?),
            },
            RuntimeAgentIntrinsic::Viewport => {
                require_count(id, intrinsic, &values, 0)?;
                RuntimeAgentExprSeed::CaptureViewport
            }
            RuntimeAgentIntrinsic::Layer => RuntimeAgentExprSeed::CaptureLayer {
                target: Box::new(one(&values)?),
            },
            RuntimeAgentIntrinsic::Object => RuntimeAgentExprSeed::CaptureObject {
                target: Box::new(one(&values)?),
            },
            RuntimeAgentIntrinsic::Signal => RuntimeAgentExprSeed::ProbeSignal {
                target: Box::new(one(&values)?),
            },
            RuntimeAgentIntrinsic::Metric => RuntimeAgentExprSeed::ProbeMetric {
                target: Box::new(one(&values)?),
            },
            RuntimeAgentIntrinsic::State => RuntimeAgentExprSeed::ProbeState {
                path: Box::new(one(&values)?),
            },
            RuntimeAgentIntrinsic::Observation => RuntimeAgentExprSeed::ProbeObservation {
                path: Box::new(one(&values)?),
            },
            RuntimeAgentIntrinsic::Diagnostics => {
                require_count(id, intrinsic, &values, 0)?;
                RuntimeAgentExprSeed::Diagnostics
            }
            RuntimeAgentIntrinsic::Exists => RuntimeAgentExprSeed::PredicateExists {
                probe: Box::new(one(&values)?),
            },
            RuntimeAgentIntrinsic::ActionEnabled => RuntimeAgentExprSeed::PredicateActionEnabled {
                target: Box::new(one(&values)?),
            },
            RuntimeAgentIntrinsic::All => RuntimeAgentExprSeed::PredicateAll {
                predicates: values.into_boxed_slice(),
            },
            RuntimeAgentIntrinsic::Any => RuntimeAgentExprSeed::PredicateAny {
                predicates: values.into_boxed_slice(),
            },
            RuntimeAgentIntrinsic::Not => RuntimeAgentExprSeed::PredicateNot {
                predicate: Box::new(one(&values)?),
            },
            RuntimeAgentIntrinsic::ChoiceAction => RuntimeAgentExprSeed::ChoiceAction {
                choice: self.choice_target(call, id)?,
            },
            RuntimeAgentIntrinsic::ViewportPoint => {
                let mut values = values;
                if values.len() != 2 {
                    return Err(format!(
                        "typed Agent intrinsic {intrinsic:?} at {id:?} has {} runtime arguments instead of 2",
                        values.len()
                    ));
                }
                RuntimeAgentExprSeed::ViewportPoint {
                    x: Box::new(values.remove(0)),
                    y: Box::new(values.remove(0)),
                }
            }
            RuntimeAgentIntrinsic::Observe
            | RuntimeAgentIntrinsic::Expect
            | RuntimeAgentIntrinsic::Deny
            | RuntimeAgentIntrinsic::Checkpoint
            | RuntimeAgentIntrinsic::Note
            | RuntimeAgentIntrinsic::Attach
            | RuntimeAgentIntrinsic::Capture
            | RuntimeAgentIntrinsic::ReadResource
            | RuntimeAgentIntrinsic::EntityMeta
            | RuntimeAgentIntrinsic::ProjectNeighbors
            | RuntimeAgentIntrinsic::Wait
            | RuntimeAgentIntrinsic::AdvanceText
            | RuntimeAgentIntrinsic::PointerClick
            | RuntimeAgentIntrinsic::Invoke
            | RuntimeAgentIntrinsic::RagQuery => {
                unreachable!("host operations return before deterministic value lowering")
            }
        }))
    }

    fn lower_agent_compare(
        &self,
        id: ExprId,
        call: &arcweft_lang_hir::expr::HirCallExpr,
        op: RuntimeAgentCompareOp,
    ) -> Result<RuntimeExprSeedKind, String> {
        let mut values = self.value_arguments(call, id, 2)?;
        Ok(RuntimeExprSeedKind::Agent(
            RuntimeAgentExprSeed::PredicateCompare {
                probe: Box::new(values.remove(0)),
                op,
                value: Box::new(values.remove(0)),
            },
        ))
    }

    fn choice_target(
        &self,
        call: &arcweft_lang_hir::expr::HirCallExpr,
        id: ExprId,
    ) -> Result<arcweft_core::entry::RuntimeCommandTargetId, String> {
        let expression = call
            .arguments()
            .first()
            .ok_or_else(|| format!("choice_action at {id:?} has no authored argument"))?
            .value();
        arcweft_core::entry::RuntimeCommandTargetId::try_new(
            self.entity_reference(expression)?.runtime_label(),
        )
        .map_err(|error| format!("choice_action at {id:?} has invalid target identity: {error}"))
    }

    fn entity_reference(
        &self,
        id: ExprId,
    ) -> Result<arcweft_core::value::RuntimeEntityReference, String> {
        match self.facts.value(id) {
            Some(RuntimeResolvedValue::ProjectItem(item)) => Ok(project_entity_reference(item)),
            Some(RuntimeResolvedValue::DialogueLine(line)) => Ok(
                arcweft_core::value::RuntimeEntityReference::DialogueLine(line.clone()),
            ),
            _ => Err(format!(
                "Agent semantic identity at {id:?} is not an exact accepted entity"
            )),
        }
    }

    fn lower_call_receiver(
        &self,
        call: &arcweft_lang_hir::expr::HirCallExpr,
        id: ExprId,
    ) -> Result<RuntimeExprSeed, String> {
        self.lower(
            self.module
                .resolve_call_value_receiver(call)
                .map_err(|error| format!("cannot resolve call receiver at {id:?}: {error}"))?
                .ok_or_else(|| format!("call {id:?} has no receiver-bearing value callee"))?,
        )
    }

    fn validate_variant_call_payload(
        &self,
        id: ExprId,
        call: &arcweft_lang_hir::expr::HirCallExpr,
        arguments: &[RuntimeResolvedCallArgument],
        variant: &RuntimeResolvedVariant,
    ) -> Result<(), String> {
        let types = arguments
            .iter()
            .map(|argument| match argument {
                RuntimeResolvedCallArgument::Authored { ordinal, .. } => {
                    let ordinal = usize::try_from(*ordinal).map_err(|_| {
                        format!("call argument ordinal {ordinal} does not fit usize")
                    })?;
                    let value = call
                        .arguments()
                        .get(ordinal)
                        .ok_or_else(|| {
                            format!("call argument ordinal {ordinal} is absent at {id:?}")
                        })?
                        .value();
                    self.facts.expression_type(value).ok_or_else(|| {
                        format!(
                            "accepted type is missing for variant constructor argument {value:?}"
                        )
                    })
                }
                RuntimeResolvedCallArgument::Receiver => Err(format!(
                    "variant constructor {id:?} has an invalid receiver"
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let payload = variant
            .selected_payload_type()
            .map_err(|error| error.to_string())?;
        (payload.is_some() != arguments.is_empty()
            && variant_payload_accepts_argument_types(payload, &types))
        .then_some(())
        .ok_or_else(|| {
            format!(
                "variant constructor at {id:?} does not match its selected normalized payload type"
            )
        })
    }

    fn nominal_record_field(
        nominal: &crate::semantic_facts::RuntimeResolvedNominalRecord,
        name: &str,
        owner: ExprId,
    ) -> Result<RuntimeRecordFieldSeedId, String> {
        nominal
            .layout()
            .field_by_name(name)
            .map(|(field, _)| RuntimeRecordFieldSeedId::from_zero_based(field.zero_based()))
            .ok_or_else(|| format!("accepted nominal record {owner:?} lacks field {name:?}"))
    }

    fn lower_constant_length(&self, id: ExprId) -> Result<usize, String> {
        match self
            .facts
            .expression_literal(id)
            .ok_or_else(|| format!("checked array length fact is missing for {id:?}"))?
        {
            RuntimeValue::UInt(value) => usize::try_from(value.as_u128())
                .map_err(|_| format!("array length at {id:?} does not fit usize")),
            RuntimeValue::Int(value) if value.as_i128() >= 0 => usize::try_from(value.as_i128())
                .map_err(|_| format!("array length at {id:?} does not fit usize")),
            _ => Err(format!(
                "array length at {id:?} is not a checked non-negative integer"
            )),
        }
    }

    fn simple_binding(
        &self,
        id: arcweft_lang_hir::identity::PatternId,
    ) -> Result<RuntimeLocalSeedId, String> {
        let pattern = self
            .module
            .resolve_pattern(id)
            .map_err(|error| format!("cannot resolve final-HIR binding pattern {id:?}: {error}"))?;
        let local = match pattern.kind() {
            arcweft_lang_hir::pattern::HirPatternKind::Binding(
                arcweft_lang_hir::pattern::HirPatternBinding::Bound { local, .. },
            )
            | arcweft_lang_hir::pattern::HirPatternKind::MutableBinding(
                arcweft_lang_hir::pattern::HirPatternBinding::Bound { local, .. },
            )
            | arcweft_lang_hir::pattern::HirPatternKind::TypedBinding {
                binding: arcweft_lang_hir::pattern::HirPatternBinding::Bound { local, .. },
                ..
            } => *local,
            _ => return Err(format!("pattern {id:?} is not a single runtime binding")),
        };
        self.local(local)
    }

    fn expression_type(
        &self,
        id: ExprId,
    ) -> Result<arcweft_core::pattern::RuntimeSemanticTypeId, String> {
        self.facts
            .expression_type(id)
            .map(RuntimeNormalizedType::identity)
            .ok_or_else(|| format!("accepted type is missing for expression {id:?}"))
    }
    fn local_type(
        &self,
        id: LocalId,
    ) -> Result<arcweft_core::pattern::RuntimeSemanticTypeId, String> {
        self.facts
            .local_type(id)
            .map(RuntimeNormalizedType::identity)
            .ok_or_else(|| format!("accepted type is missing for local {id:?}"))
    }
    fn local(&self, id: LocalId) -> Result<RuntimeLocalSeedId, String> {
        self.locals.get(&id).cloned().ok_or_else(|| {
            format!("runtime local seed handle is missing for accepted local {id:?}")
        })
    }
    fn pattern(&self) -> FinalPatternLowerer<'hir> {
        FinalPatternLowerer::new(self.module, self.facts, self.locals)
    }
}

fn exact_one(id: ExprId, values: &[RuntimeExprSeed]) -> Result<RuntimeExprSeed, String> {
    if values.len() == 1 {
        Ok(values[0].clone())
    } else {
        Err(format!(
            "typed Agent intrinsic at {id:?} has {} runtime arguments instead of one",
            values.len()
        ))
    }
}
fn require_count(
    id: ExprId,
    intrinsic: RuntimeAgentIntrinsic,
    values: &[RuntimeExprSeed],
    expected: usize,
) -> Result<(), String> {
    (values.len() == expected).then_some(()).ok_or_else(|| format!("typed Agent intrinsic {intrinsic:?} at {id:?} has {} runtime arguments instead of {expected}", values.len()))
}
fn variant_payload_accepts_argument_types(
    payload: Option<&RuntimeNormalizedType>,
    arguments: &[&RuntimeNormalizedType],
) -> bool {
    match arguments {
        [] => payload.is_none(),
        [argument] => payload == Some(*argument),
        arguments => {
            matches!(payload.map(RuntimeNormalizedType::shape), Some(RuntimeTypeShape::Tuple(items)) if items.len() == arguments.len() && items.iter().zip(arguments).all(|(expected, actual)| expected == *actual))
        }
    }
}
fn runtime_binary(operator: HirBinaryOp) -> Option<RuntimeBinaryOp> {
    Some(match operator {
        HirBinaryOp::Or => RuntimeBinaryOp::Or,
        HirBinaryOp::And => RuntimeBinaryOp::And,
        HirBinaryOp::Equal => RuntimeBinaryOp::Eq,
        HirBinaryOp::NotEqual => RuntimeBinaryOp::Ne,
        HirBinaryOp::GreaterOrEqual => RuntimeBinaryOp::Ge,
        HirBinaryOp::LessOrEqual => RuntimeBinaryOp::Le,
        HirBinaryOp::Greater => RuntimeBinaryOp::Gt,
        HirBinaryOp::Less => RuntimeBinaryOp::Lt,
        HirBinaryOp::Add => RuntimeBinaryOp::Add,
        HirBinaryOp::Subtract => RuntimeBinaryOp::Sub,
        HirBinaryOp::Multiply => RuntimeBinaryOp::Mul,
        HirBinaryOp::Divide => RuntimeBinaryOp::Div,
        HirBinaryOp::Implies | HirBinaryOp::In | HirBinaryOp::Merge | HirBinaryOp::Remainder => {
            return None;
        }
    })
}
