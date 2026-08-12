//! Runtime-expression projection from final arena HIR and checked semantic facts.

use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeCallTarget, RuntimeExpr, RuntimeExprMatchArm, RuntimeFieldExpr,
    RuntimeUnaryOp, RuntimeValue,
};
use arcweft_lang_hir::expr::{
    HirBinaryOp, HirCallArgument, HirExprKind, HirRecordField, HirUnaryOp,
};
use arcweft_lang_hir::identity::{ExprId, LocalId, StmtId};
use arcweft_lang_hir::item::HirFunctionBody;
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_hir::stmt::HirStmtKind;

use crate::agent::RuntimeAgentIntrinsic;
use crate::final_pattern::FinalPatternLowerer;
use crate::semantic_facts::{
    RuntimePlanSemanticFacts, RuntimeReductionConstructor, RuntimeResolvedCallArgument,
    RuntimeResolvedCallTarget, RuntimeResolvedSelect, RuntimeResolvedValue,
};

pub(crate) struct FinalExprLowerer<'hir> {
    module: &'hir HirModule,
    facts: &'hir RuntimePlanSemanticFacts,
}

impl<'hir> FinalExprLowerer<'hir> {
    pub(crate) const fn new(
        module: &'hir HirModule,
        facts: &'hir RuntimePlanSemanticFacts,
    ) -> Self {
        Self { module, facts }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the exhaustive final-HIR expression projection is kept in one match so every executable and rejected family remains visibly closed"
    )]
    pub(crate) fn lower(&self, id: ExprId) -> Result<RuntimeExpr, String> {
        let expression = self
            .module
            .resolve_expr(id)
            .map_err(|error| format!("cannot resolve final-HIR expression {id:?}: {error}"))?;
        if expression.is_poisoned() {
            return Err(format!(
                "final-HIR expression {id:?} contains recovery and is not executable"
            ));
        }
        match expression.kind() {
            HirExprKind::Unit => Ok(RuntimeExpr::Value(RuntimeValue::Unit)),
            HirExprKind::Literal(_) | HirExprKind::NumericBracketSequence(_) => self
                .facts
                .expression_literal(id)
                .cloned()
                .map(RuntimeExpr::Value)
                .ok_or_else(|| format!("checked literal fact is missing for expression {id:?}")),
            HirExprKind::EntityReference(_) => match self.facts.value(id) {
                Some(RuntimeResolvedValue::ProjectItem(item)) => {
                    Ok(RuntimeExpr::EntityRef(item.public_id().as_str().to_owned()))
                }
                Some(RuntimeResolvedValue::DialogueLine(line)) => {
                    Ok(RuntimeExpr::EntityRef(line.canonical_label()))
                }
                Some(_) => Err(format!(
                    "checked value fact for entity expression {id:?} has the wrong family"
                )),
                None => Err(format!(
                    "checked project-item fact is missing for entity expression {id:?}"
                )),
            },
            HirExprKind::Path(_) => {
                if self.facts.expression_variant(id).is_some() {
                    self.lower_unit_variant(id)
                } else {
                    self.lower_path(id)
                }
            }
            HirExprKind::ShortVariant(_) => self.lower_unit_variant(id),
            HirExprKind::Tuple(tuple) => tuple
                .elements()
                .iter()
                .map(|element| self.lower(*element))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeExpr::Tuple),
            HirExprKind::BracketSequence(sequence) => sequence
                .elements()
                .iter()
                .map(|element| self.lower(*element))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeExpr::BracketSeq),
            HirExprKind::ArrayRepeat(repeat) => {
                let length = self.lower_constant_length(repeat.length())?;
                Ok(RuntimeExpr::RepeatSeq {
                    value: Box::new(self.lower(repeat.value())?),
                    len: length,
                })
            }
            HirExprKind::Call(call) => self.lower_call(id, call),
            HirExprKind::Select(select) => {
                let target = Box::new(self.lower(select.target())?);
                let selected = self.facts.select(id).ok_or_else(|| {
                    format!("checked member fact is missing for expression {id:?}")
                })?;
                match selected {
                    RuntimeResolvedSelect::Method { name } => Err(format!(
                        "bound method {} at {id:?} cannot execute outside its checked Call",
                        name.as_str()
                    )),
                    RuntimeResolvedSelect::Field { name, .. } => Ok(RuntimeExpr::Field {
                        target,
                        field: name.as_str().to_owned(),
                    }),
                    RuntimeResolvedSelect::TupleElement { ordinal } => {
                        Ok(RuntimeExpr::ProjectTuple {
                            target,
                            ordinal: usize::try_from(*ordinal).map_err(|_| {
                                format!("tuple ordinal {ordinal} does not fit usize")
                            })?,
                        })
                    }
                    RuntimeResolvedSelect::RecordElement { ordinal, .. } => {
                        Ok(RuntimeExpr::ProjectRecord {
                            target,
                            ordinal: usize::try_from(*ordinal).map_err(|_| {
                                format!("record ordinal {ordinal} does not fit usize")
                            })?,
                        })
                    }
                }
            }
            HirExprKind::Range(range) => Ok(RuntimeExpr::Range {
                start: range
                    .start()
                    .map(|start| self.lower(start))
                    .transpose()?
                    .map(Box::new),
                end: range
                    .end()
                    .map(|end| self.lower(end))
                    .transpose()?
                    .map(Box::new),
                inclusive: range.inclusive(),
            }),
            HirExprKind::RecordLiteral(record) => self
                .lower_record_fields(id, record.fields())
                .map(RuntimeExpr::Record),
            HirExprKind::Record(record) => {
                let _ = self
                    .facts
                    .nominal_record(id)
                    .ok_or_else(|| {
                        format!(
                            "nominal record expression {id:?} requires a typed runtime nominal-expression owner"
                        )
                    })?;
                self.lower_record_fields(id, record.fields())
                    .map(RuntimeExpr::Record)
            }
            HirExprKind::Binary(binary) => {
                let op = runtime_binary(binary.operator()).ok_or_else(|| {
                    format!(
                        "binary operator {:?} at {id:?} has no runtime expression representation",
                        binary.operator()
                    )
                })?;
                Ok(RuntimeExpr::Binary {
                    lhs: Box::new(self.lower(binary.left())?),
                    op,
                    rhs: Box::new(self.lower(binary.right())?),
                })
            }
            HirExprKind::Unary(unary) => Ok(RuntimeExpr::Unary {
                op: match unary.operator() {
                    HirUnaryOp::Not => RuntimeUnaryOp::Not,
                    HirUnaryOp::Negate => RuntimeUnaryOp::Neg,
                },
                expr: Box::new(self.lower(unary.operand())?),
            }),
            HirExprKind::Closure(closure) => {
                let mut parameters = Vec::with_capacity(closure.parameters().len());
                for parameter in closure.parameters() {
                    let pattern = FinalPatternLowerer::new(self.module, self.facts)
                        .lower(parameter.pattern())?;
                    parameters.push(simple_binding(pattern, parameter.pattern())?);
                }
                Ok(RuntimeExpr::Function {
                    params: parameters,
                    body: Box::new(self.lower(closure.body())?),
                })
            }
            HirExprKind::Block(block) => self.lower_block(block.statements(), block.tail()),
            HirExprKind::ComputationBlock(block) => {
                self.lower_block(block.statements(), block.tail())
            }
            HirExprKind::NamedBlock(block) => self.lower_block(block.statements(), block.tail()),
            HirExprKind::If(branch) => Ok(RuntimeExpr::If {
                condition: Box::new(self.lower(branch.condition())?),
                then_expr: Box::new(self.lower(branch.then_branch())?),
                else_expr: Box::new(self.lower(branch.else_branch())?),
            }),
            HirExprKind::IfLet(branch) => Ok(RuntimeExpr::IfLet {
                pattern: FinalPatternLowerer::new(self.module, self.facts)
                    .lower(branch.pattern())?,
                expr: Box::new(self.lower(branch.scrutinee())?),
                guard: branch
                    .guard()
                    .map(|guard| self.lower(guard))
                    .transpose()?
                    .map(Box::new),
                then_expr: Box::new(self.lower(branch.then_branch())?),
                else_expr: Box::new(self.lower(branch.else_branch())?),
            }),
            HirExprKind::Match(matched) => {
                let arms = matched
                    .arms()
                    .iter()
                    .map(|arm| {
                        Ok(RuntimeExprMatchArm {
                            pattern: FinalPatternLowerer::new(self.module, self.facts)
                                .lower(arm.pattern())?,
                            guard: arm.guard().map(|guard| self.lower(guard)).transpose()?,
                            value: self.lower(arm.value())?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(RuntimeExpr::Match {
                    scrutinee: Box::new(self.lower(matched.scrutinee())?),
                    arms,
                })
            }
            HirExprKind::PostfixBracket(_) => {
                let candidate = self.facts.postfix_candidate(id).ok_or_else(|| {
                    format!("checked postfix candidate is missing for expression {id:?}")
                })?;
                self.lower(candidate)
            }
            HirExprKind::Placeholder(_)
            | HirExprKind::Index(_)
            | HirExprKind::Pipe(_)
            | HirExprKind::Try(_)
            | HirExprKind::Await(_)
            | HirExprKind::Thread(_)
            | HirExprKind::Choice(_)
            | HirExprKind::Borrow(_)
            | HirExprKind::Dereference(_)
            | HirExprKind::DialogueContentApplication(_)
            | HirExprKind::Error(_)
            | HirExprKind::ForSynthetic(_)
            | HirExprKind::LifetimePath(_) => Err(format!(
                "final-HIR expression family {:?} at {id:?} is not a pure runtime expression",
                expression.kind()
            )),
        }
    }

    /// Lowers the exact final-HIR body owned by one admitted ordinary function.
    pub(crate) fn lower_function_body(
        &self,
        body: &HirFunctionBody,
    ) -> Result<RuntimeExpr, String> {
        match body {
            HirFunctionBody::Block {
                statements, tail, ..
            } => self.lower_block(statements, *tail),
            HirFunctionBody::Error(expression) => Err(format!(
                "recovered ordinary-function body {expression:?} cannot enter runtime lowering"
            )),
        }
    }

    /// Lowers one checked named-field assignment and continues with the
    /// caller-owned expression. Both pure method blocks and Flow statement
    /// lowering consume this exact projection.
    pub(crate) fn lower_assignment(
        &self,
        target: ExprId,
        value: ExprId,
        body: RuntimeExpr,
    ) -> Result<RuntimeExpr, String> {
        let target_expression = self
            .module
            .resolve_expr(target)
            .map_err(|error| format!("cannot resolve assignment target {target:?}: {error}"))?;
        let HirExprKind::Select(select) = target_expression.kind() else {
            return Err(format!(
                "assignment target {target:?} is not a typed field selection"
            ));
        };
        let RuntimeResolvedSelect::Field { name, .. } = self
            .facts
            .select(target)
            .ok_or_else(|| format!("assignment target {target:?} has no checked field fact"))?
        else {
            return Err(format!(
                "assignment target {target:?} is not a checked named field"
            ));
        };
        Ok(RuntimeExpr::AssignField {
            target: Box::new(self.lower(select.target())?),
            field: name.as_str().to_owned(),
            expr: Box::new(self.lower(value)?),
            body: Box::new(body),
        })
    }

    fn lower_path(&self, id: ExprId) -> Result<RuntimeExpr, String> {
        match self
            .facts
            .value(id)
            .ok_or_else(|| format!("checked value fact is missing for expression {id:?}"))?
        {
            RuntimeResolvedValue::Local(local) => Ok(RuntimeExpr::Local(self.local_name(*local)?)),
            RuntimeResolvedValue::Constant(value) => Ok(RuntimeExpr::Value(value.clone())),
            RuntimeResolvedValue::Intrinsic(_)
            | RuntimeResolvedValue::ProjectCallable(_)
            | RuntimeResolvedValue::ProjectItem(_)
            | RuntimeResolvedValue::DialogueLine(_)
            | RuntimeResolvedValue::Registered(_) => Err(format!(
                "resolved callable/project value at {id:?} requires a typed runtime function-value identity"
            )),
        }
    }

    fn lower_unit_variant(&self, id: ExprId) -> Result<RuntimeExpr, String> {
        let selected = self
            .facts
            .expression_variant(id)
            .ok_or_else(|| format!("checked variant fact is missing for expression {id:?}"))?;
        let selection = selected
            .checked_selection()
            .map_err(|error| error.to_string())?;
        if selection.payload().is_some() {
            return Err(format!(
                "payload-bearing variant `{}` at {id:?} was selected without a payload",
                selection.name()
            ));
        }
        Ok(RuntimeExpr::Variant {
            owner: selection.owner().clone(),
            ordinal: selection.ordinal(),
            name: selection.name().to_owned(),
            payload: None,
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one checked call projection must keep argument ownership and every closed runtime dispatch target visibly aligned"
    )]
    fn lower_call(
        &self,
        id: ExprId,
        call: &arcweft_lang_hir::expr::HirCallExpr,
    ) -> Result<RuntimeExpr, String> {
        let selected = self
            .facts
            .call(id)
            .ok_or_else(|| format!("checked call fact is missing for expression {id:?}"))?;
        let arguments = selected
            .arguments()
            .iter()
            .map(|argument| match argument {
                RuntimeResolvedCallArgument::Authored { ordinal } => {
                    let ordinal = usize::try_from(*ordinal).map_err(|_| {
                        format!("call argument ordinal {ordinal} does not fit usize")
                    })?;
                    let argument = call.arguments().get(ordinal).ok_or_else(|| {
                        format!("call argument ordinal {ordinal} is absent at {id:?}")
                    })?;
                    let value = self.lower(argument.value())?;
                    Ok(if matches!(argument, HirCallArgument::Spread { .. }) {
                        RuntimeExpr::SpreadArg(Box::new(value))
                    } else {
                        value
                    })
                }
                RuntimeResolvedCallArgument::Receiver => self.lower_call_receiver(call, id),
            })
            .collect::<Result<Vec<_>, String>>()?;

        match selected.target() {
            RuntimeResolvedCallTarget::Intrinsic(intrinsic) => Ok(RuntimeExpr::Call {
                callee: RuntimeCallTarget::intrinsic(*intrinsic),
                args: arguments,
            }),
            RuntimeResolvedCallTarget::Agent(intrinsic) => {
                self.lower_agent_intrinsic(id, call, *intrinsic, arguments)
            }
            RuntimeResolvedCallTarget::AgentProbeComparison(operation) => {
                lower_agent_probe_comparison(id, *operation, &arguments)
            }
            RuntimeResolvedCallTarget::Declaration(callable) => Ok(RuntimeExpr::Call {
                callee: RuntimeCallTarget::callable(callable.runtime().clone()),
                args: arguments,
            }),
            RuntimeResolvedCallTarget::Variant(variant) => {
                let selection = variant
                    .checked_selection()
                    .map_err(|error| error.to_string())?;
                let payload = match arguments.len() {
                    0 => None,
                    1 => Some(Box::new(
                        arguments
                            .into_iter()
                            .next()
                            .expect("one constructor argument was observed"),
                    )),
                    _ => Some(Box::new(RuntimeExpr::Tuple(arguments))),
                };
                if selection.payload().is_some() != payload.is_some() {
                    return Err(format!(
                        "variant constructor `{}` at {id:?} has incompatible payload presence",
                        selection.name()
                    ));
                }
                Ok(RuntimeExpr::Variant {
                    owner: selection.owner().clone(),
                    ordinal: selection.ordinal(),
                    name: selection.name().to_owned(),
                    payload,
                })
            }
            RuntimeResolvedCallTarget::Reduction(RuntimeReductionConstructor::Unchanged) => {
                let [state] = arguments.as_slice() else {
                    return Err(format!(
                        "Reduction.unchanged call {id:?} has {} runtime arguments instead of one",
                        arguments.len()
                    ));
                };
                Ok(RuntimeExpr::Record(vec![
                    RuntimeFieldExpr {
                        name: "state".to_owned(),
                        value: state.clone(),
                    },
                    RuntimeFieldExpr {
                        name: "commands".to_owned(),
                        value: RuntimeExpr::BracketSeq(Vec::new()),
                    },
                ]))
            }
            RuntimeResolvedCallTarget::Registered(registered) => Ok(RuntimeExpr::Call {
                callee: RuntimeCallTarget::callable(registered.clone()),
                args: arguments,
            }),
            RuntimeResolvedCallTarget::FunctionValue => {
                let callee = call.callee().value_expression().ok_or_else(|| {
                    format!("function-value call {id:?} has no final-HIR value callee")
                })?;
                Ok(RuntimeExpr::Apply {
                    callee: Box::new(self.lower(callee)?),
                    args: arguments,
                })
            }
            RuntimeResolvedCallTarget::TraitMethod { method, receiver } => {
                let receiver_index = selected
                    .arguments()
                    .iter()
                    .position(|argument| matches!(argument, RuntimeResolvedCallArgument::Receiver))
                    .ok_or_else(|| format!("trait call {id:?} has no receiver projection"))?;
                let mut arguments = arguments;
                let receiver_value = arguments.remove(receiver_index);
                Ok(RuntimeExpr::TraitCall {
                    callable: *method,
                    receiver: Box::new(receiver_value),
                    receiver_mode: *receiver,
                    args: arguments,
                })
            }
            RuntimeResolvedCallTarget::Host { .. } => Err(format!(
                "host call {id:?} is effectful and cannot enter pure expression lowering"
            )),
        }
    }

    fn lower_agent_intrinsic(
        &self,
        id: ExprId,
        call: &arcweft_lang_hir::expr::HirCallExpr,
        intrinsic: RuntimeAgentIntrinsic,
        arguments: Vec<RuntimeExpr>,
    ) -> Result<RuntimeExpr, String> {
        if let Some(operation) = intrinsic.host_operation() {
            return Err(format!(
                "Agent host call {operation} at {id:?} cannot enter pure expression lowering"
            ));
        }
        match intrinsic {
            RuntimeAgentIntrinsic::StatePath | RuntimeAgentIntrinsic::ObservationPath => {
                require_agent_argument_count(id, intrinsic, &arguments, 1)?;
                Ok(arguments[0].clone())
            }
            RuntimeAgentIntrinsic::Viewport
            | RuntimeAgentIntrinsic::Layer
            | RuntimeAgentIntrinsic::Object
            | RuntimeAgentIntrinsic::ViewportPoint => lower_agent_target(id, intrinsic, &arguments),
            RuntimeAgentIntrinsic::Signal
            | RuntimeAgentIntrinsic::Metric
            | RuntimeAgentIntrinsic::State
            | RuntimeAgentIntrinsic::Observation
            | RuntimeAgentIntrinsic::Diagnostics => lower_agent_probe(id, intrinsic, &arguments),
            RuntimeAgentIntrinsic::Exists
            | RuntimeAgentIntrinsic::ActionEnabled
            | RuntimeAgentIntrinsic::All
            | RuntimeAgentIntrinsic::Any
            | RuntimeAgentIntrinsic::Not => lower_agent_predicate(id, intrinsic, arguments),
            RuntimeAgentIntrinsic::ChoiceAction => {
                require_agent_argument_count(id, intrinsic, &arguments, 1)?;
                let authored = call
                    .arguments()
                    .first()
                    .ok_or_else(|| format!("choice_action at {id:?} has no authored argument"))?
                    .value();
                let target = self.static_entity_label(authored)?;
                Ok(RuntimeExpr::Record(vec![
                    agent_field(
                        "id",
                        agent_string(&format!("action.select_choice.{target}")),
                    ),
                    agent_field("target", agent_string(&target)),
                    agent_field("action", agent_string("select_choice")),
                    agent_field("kind", agent_string("semantic")),
                    agent_field("enabled", RuntimeExpr::Value(RuntimeValue::Bool(true))),
                ]))
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
                unreachable!("Agent host operations returned before deterministic value lowering")
            }
        }
    }

    fn static_entity_label(&self, expression: ExprId) -> Result<String, String> {
        match self.facts.value(expression) {
            Some(RuntimeResolvedValue::ProjectItem(item)) => {
                Ok(item.public_id().as_str().to_owned())
            }
            Some(RuntimeResolvedValue::DialogueLine(line)) => Ok(line.canonical_label()),
            _ => Err(format!(
                "Agent semantic identity at {expression:?} is not an exact accepted entity"
            )),
        }
    }

    fn lower_call_receiver(
        &self,
        call: &arcweft_lang_hir::expr::HirCallExpr,
        id: ExprId,
    ) -> Result<RuntimeExpr, String> {
        let callee = call
            .callee()
            .value_expression()
            .ok_or_else(|| format!("call {id:?} has no receiver-bearing value callee"))?;
        let expression = self
            .module
            .resolve_expr(callee)
            .map_err(|error| format!("cannot resolve call receiver {callee:?}: {error}"))?;
        if let HirExprKind::Select(select) = expression.kind() {
            self.lower(select.target())
        } else {
            self.lower(callee)
        }
    }

    fn lower_record_fields(
        &self,
        owner: ExprId,
        fields: &[HirRecordField],
    ) -> Result<Vec<RuntimeFieldExpr>, String> {
        fields
            .iter()
            .map(|field| match field {
                HirRecordField::Explicit { name, value } => Ok(RuntimeFieldExpr {
                    name: name.as_str().to_owned(),
                    value: self.lower(*value)?,
                }),
                HirRecordField::Shorthand { name, local } => Ok(RuntimeFieldExpr {
                    name: name.as_str().to_owned(),
                    value: RuntimeExpr::Local(self.local_name(*local)?),
                }),
                HirRecordField::Invalid { .. } => {
                    Err(format!("record expression {owner:?} has an invalid field"))
                }
            })
            .collect()
    }

    fn lower_block(&self, statements: &[StmtId], tail: ExprId) -> Result<RuntimeExpr, String> {
        statements
            .iter()
            .rev()
            .try_fold(self.lower(tail)?, |body, statement| {
                let statement = self.module.resolve_stmt(*statement).map_err(|error| {
                    format!("cannot resolve block statement {statement:?}: {error}")
                })?;
                if statement.is_poisoned() {
                    return Err("recovered block statement is not executable".to_owned());
                }
                match statement.kind() {
                    HirStmtKind::Let {
                        pattern,
                        initializer,
                        ..
                    } => {
                        let pattern_id = *pattern;
                        let pattern =
                            FinalPatternLowerer::new(self.module, self.facts).lower(pattern_id)?;
                        Ok(RuntimeExpr::Let {
                            name: simple_binding(pattern, pattern_id)?,
                            expr: Box::new(self.lower(*initializer)?),
                            body: Box::new(body),
                        })
                    }
                    HirStmtKind::Assign { target, value } => {
                        self.lower_assignment(*target, *value, body)
                    }
                    other => Err(format!(
                        "statement {other:?} cannot be embedded in a pure runtime expression block"
                    )),
                }
            })
    }

    fn lower_constant_length(&self, id: ExprId) -> Result<usize, String> {
        let value = self
            .facts
            .expression_literal(id)
            .ok_or_else(|| format!("checked array length fact is missing for {id:?}"))?;
        match value {
            RuntimeValue::UInt(value) => usize::try_from(value.as_u128())
                .map_err(|_| format!("array length at {id:?} does not fit usize")),
            RuntimeValue::Int(value) if value.as_i128() >= 0 => usize::try_from(value.as_i128())
                .map_err(|_| format!("array length at {id:?} does not fit usize")),
            _ => Err(format!(
                "array length at {id:?} is not a checked non-negative integer"
            )),
        }
    }

    fn local_name(&self, local: LocalId) -> Result<String, String> {
        self.module
            .resolve_local(local)
            .map(|local| local.name().as_str().to_owned())
            .map_err(|error| format!("cannot resolve final-HIR local {local:?}: {error}"))
    }
}

fn lower_agent_target(
    id: ExprId,
    intrinsic: RuntimeAgentIntrinsic,
    arguments: &[RuntimeExpr],
) -> Result<RuntimeExpr, String> {
    match intrinsic {
        RuntimeAgentIntrinsic::Viewport => {
            require_agent_argument_count(id, intrinsic, arguments, 0)?;
            Ok(RuntimeExpr::Record(vec![agent_field(
                "kind",
                agent_string("viewport"),
            )]))
        }
        RuntimeAgentIntrinsic::Layer | RuntimeAgentIntrinsic::Object => {
            require_agent_argument_count(id, intrinsic, arguments, 1)?;
            let kind = if intrinsic == RuntimeAgentIntrinsic::Layer {
                "layer"
            } else {
                "object"
            };
            Ok(RuntimeExpr::Record(vec![
                agent_field("kind", agent_string(kind)),
                agent_field("target", arguments[0].clone()),
            ]))
        }
        RuntimeAgentIntrinsic::ViewportPoint => {
            require_agent_argument_count(id, intrinsic, arguments, 2)?;
            Ok(RuntimeExpr::Record(vec![
                agent_field("x", arguments[0].clone()),
                agent_field("y", arguments[1].clone()),
            ]))
        }
        _ => unreachable!("target constructor dispatcher owns only Agent target intrinsics"),
    }
}

fn lower_agent_probe(
    id: ExprId,
    intrinsic: RuntimeAgentIntrinsic,
    arguments: &[RuntimeExpr],
) -> Result<RuntimeExpr, String> {
    match intrinsic {
        RuntimeAgentIntrinsic::Signal | RuntimeAgentIntrinsic::Metric => {
            require_agent_argument_count(id, intrinsic, arguments, 1)?;
            let kind = if intrinsic == RuntimeAgentIntrinsic::Signal {
                "signal"
            } else {
                "metric"
            };
            Ok(RuntimeExpr::Record(vec![
                agent_field("kind", agent_string(kind)),
                agent_field("target", arguments[0].clone()),
            ]))
        }
        RuntimeAgentIntrinsic::State | RuntimeAgentIntrinsic::Observation => {
            require_agent_argument_count(id, intrinsic, arguments, 1)?;
            let kind = if intrinsic == RuntimeAgentIntrinsic::State {
                "state"
            } else {
                "observation"
            };
            Ok(RuntimeExpr::Record(vec![
                agent_field("kind", agent_string(kind)),
                agent_field("path", arguments[0].clone()),
            ]))
        }
        RuntimeAgentIntrinsic::Diagnostics => {
            require_agent_argument_count(id, intrinsic, arguments, 0)?;
            Ok(RuntimeExpr::Record(vec![agent_field(
                "kind",
                agent_string("diagnostics"),
            )]))
        }
        _ => unreachable!("probe constructor dispatcher owns only Agent probe intrinsics"),
    }
}

fn lower_agent_predicate(
    id: ExprId,
    intrinsic: RuntimeAgentIntrinsic,
    arguments: Vec<RuntimeExpr>,
) -> Result<RuntimeExpr, String> {
    match intrinsic {
        RuntimeAgentIntrinsic::Exists | RuntimeAgentIntrinsic::Not => {
            require_agent_argument_count(id, intrinsic, &arguments, 1)?;
            let (kind, value) = if intrinsic == RuntimeAgentIntrinsic::Exists {
                ("exists", "probe")
            } else {
                ("not", "predicate")
            };
            Ok(RuntimeExpr::Record(vec![
                agent_field("kind", agent_string(kind)),
                agent_field(value, arguments[0].clone()),
            ]))
        }
        RuntimeAgentIntrinsic::ActionEnabled => {
            require_agent_argument_count(id, intrinsic, &arguments, 1)?;
            Ok(RuntimeExpr::Record(vec![
                agent_field("kind", agent_string("action_enabled")),
                agent_field(
                    "target",
                    RuntimeExpr::Field {
                        target: Box::new(arguments[0].clone()),
                        field: "target".to_owned(),
                    },
                ),
            ]))
        }
        RuntimeAgentIntrinsic::All | RuntimeAgentIntrinsic::Any => Ok(RuntimeExpr::Record(vec![
            agent_field(
                "kind",
                agent_string(if intrinsic == RuntimeAgentIntrinsic::All {
                    "all"
                } else {
                    "any"
                }),
            ),
            agent_field("predicates", RuntimeExpr::Tuple(arguments)),
        ])),
        _ => unreachable!("predicate dispatcher owns only Agent predicate intrinsics"),
    }
}

fn lower_agent_probe_comparison(
    id: ExprId,
    operation: crate::agent::RuntimeAgentProbeComparison,
    arguments: &[RuntimeExpr],
) -> Result<RuntimeExpr, String> {
    if arguments.len() != 2 {
        return Err(format!(
            "typed Agent probe comparison {operation:?} at {id:?} has {} runtime arguments instead of 2",
            arguments.len()
        ));
    }
    Ok(RuntimeExpr::Record(vec![
        agent_field("kind", agent_string("compare")),
        agent_field("probe", arguments[0].clone()),
        agent_field("op", agent_string(operation.operation())),
        agent_field("value", arguments[1].clone()),
    ]))
}

fn require_agent_argument_count(
    id: ExprId,
    intrinsic: RuntimeAgentIntrinsic,
    arguments: &[RuntimeExpr],
    expected: usize,
) -> Result<(), String> {
    (arguments.len() == expected).then_some(()).ok_or_else(|| {
        format!(
            "typed Agent intrinsic {intrinsic:?} at {id:?} has {} runtime arguments instead of {expected}",
            arguments.len()
        )
    })
}

fn agent_field(name: &str, value: RuntimeExpr) -> RuntimeFieldExpr {
    RuntimeFieldExpr {
        name: name.to_owned(),
        value,
    }
}

fn agent_string(value: &str) -> RuntimeExpr {
    RuntimeExpr::Value(RuntimeValue::String(value.to_owned()))
}

fn simple_binding(
    pattern: arcweft_core::pattern::RuntimePattern,
    owner: impl std::fmt::Debug,
) -> Result<String, String> {
    match pattern {
        arcweft_core::pattern::RuntimePattern::Ident(name)
        | arcweft_core::pattern::RuntimePattern::MutIdent(name)
        | arcweft_core::pattern::RuntimePattern::Typed { name, .. } => Ok(name),
        _ => Err(format!("pattern {owner:?} is not a single runtime binding")),
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
