use std::collections::{BTreeMap, BTreeSet};

use arcweft_core::time::LogicalDuration;
use arcweft_lang_hir::expr::HirExprKind;
use arcweft_lang_hir::identity::{ExprId, HirModuleId, StmtId};
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_hir::stmt::HirStmtKind;

use super::{
    RuntimeNormalizedType, RuntimeResolvedCall, RuntimeResolvedCallOperandSource,
    RuntimeSemanticFactsError, RuntimeSequenceKind, RuntimeTypeShape, resolve_expr, resolve_stmt,
    validate_normalized_type,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl RuntimeLogLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEvaluatedEffectOperandFact {
    source: RuntimeResolvedCallOperandSource,
    ty: RuntimeNormalizedType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEffectFieldFact {
    name: String,
    operand: RuntimeEvaluatedEffectOperandFact,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeDropFadeFact {
    Constant(LogicalDuration),
    Operand(RuntimeEvaluatedEffectOperandFact),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeDropPolicyFact {
    Default,
    Cancel,
    Stop { fade: RuntimeDropFadeFact },
    Finish,
    Release,
    Detach,
}

impl RuntimeEffectFieldFact {
    pub fn new(name: impl Into<String>, operand: RuntimeEvaluatedEffectOperandFact) -> Self {
        Self {
            name: name.into(),
            operand,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn operand(&self) -> &RuntimeEvaluatedEffectOperandFact {
        &self.operand
    }
}

impl RuntimeEvaluatedEffectOperandFact {
    pub const fn new(source: RuntimeResolvedCallOperandSource, ty: RuntimeNormalizedType) -> Self {
        Self { source, ty }
    }

    pub const fn source(&self) -> RuntimeResolvedCallOperandSource {
        self.source
    }

    pub const fn ty(&self) -> &RuntimeNormalizedType {
        &self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeEvaluatedEffect {
    Log {
        level: RuntimeLogLevel,
        message: RuntimeEvaluatedEffectOperandFact,
        fields: Box<[RuntimeEffectFieldFact]>,
    },
    SignalWrite {
        target: RuntimeEvaluatedEffectOperandFact,
        value: RuntimeEvaluatedEffectOperandFact,
    },
    MetricWrite {
        target: RuntimeEvaluatedEffectOperandFact,
        value: RuntimeEvaluatedEffectOperandFact,
    },
    EmitEvent {
        event: RuntimeEvaluatedEffectOperandFact,
        fields: Box<[RuntimeEffectFieldFact]>,
    },
    Panic {
        message: RuntimeEvaluatedEffectOperandFact,
    },
    Fail {
        message: RuntimeEvaluatedEffectOperandFact,
    },
    Bail {
        message: RuntimeEvaluatedEffectOperandFact,
    },
    Ensure {
        condition: RuntimeEvaluatedEffectOperandFact,
        message: RuntimeEvaluatedEffectOperandFact,
    },
    Drop {
        target: RuntimeEvaluatedEffectOperandFact,
        policy: RuntimeDropPolicyFact,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEvaluatedEffectFact {
    application: ExprId,
    effect: RuntimeEvaluatedEffect,
}

impl RuntimeEvaluatedEffectFact {
    pub const fn new(application: ExprId, effect: RuntimeEvaluatedEffect) -> Self {
        Self {
            application,
            effect,
        }
    }

    pub const fn application(&self) -> ExprId {
        self.application
    }

    pub const fn effect(&self) -> &RuntimeEvaluatedEffect {
        &self.effect
    }
}

impl RuntimeEvaluatedEffect {
    pub(super) fn visit_operand_types<'a>(
        &'a self,
        visit: &mut impl FnMut(&'a RuntimeNormalizedType),
    ) {
        match self {
            Self::Log {
                message, fields, ..
            } => {
                visit(message.ty());
                fields.iter().for_each(|field| visit(field.operand().ty()));
            }
            Self::SignalWrite { target, value } | Self::MetricWrite { target, value } => {
                visit(target.ty());
                visit(value.ty());
            }
            Self::EmitEvent { event, fields } => {
                visit(event.ty());
                fields.iter().for_each(|field| visit(field.operand().ty()));
            }
            Self::Panic { message } | Self::Fail { message } | Self::Bail { message } => {
                visit(message.ty());
            }
            Self::Ensure { condition, message } => {
                visit(condition.ty());
                visit(message.ty());
            }
            Self::Drop { target, policy } => {
                visit(target.ty());
                if let RuntimeDropPolicyFact::Stop {
                    fade: RuntimeDropFadeFact::Operand(fade),
                } = policy
                {
                    visit(fade.ty());
                }
            }
        }
    }
}

pub(super) fn validate_evaluated_effect(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expression_types: &BTreeMap<ExprId, RuntimeNormalizedType>,
    calls: &BTreeMap<ExprId, RuntimeResolvedCall>,
    statement: StmtId,
    fact: &RuntimeEvaluatedEffectFact,
) -> Result<(), RuntimeSemanticFactsError> {
    let HirStmtKind::Expression { expression } = resolve_stmt(modules, statement)? else {
        return Err(RuntimeSemanticFactsError::InvalidEvaluatedEffectFact { statement });
    };
    if fact.application() != *expression
        && !matches!(
            resolve_expr(modules, *expression),
            Ok(HirExprKind::Pipe(pipe)) if pipe.right() == fact.application()
        )
    {
        return Err(RuntimeSemanticFactsError::InvalidEvaluatedEffectFact { statement });
    }
    validate_evaluated_effect_operation(
        modules,
        expression_types,
        calls,
        fact.application(),
        fact.effect(),
    )
    .then_some(())
    .ok_or(RuntimeSemanticFactsError::InvalidEvaluatedEffectFact { statement })
}

pub(super) fn validate_evaluated_effect_operation(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expression_types: &BTreeMap<ExprId, RuntimeNormalizedType>,
    calls: &BTreeMap<ExprId, RuntimeResolvedCall>,
    application: ExprId,
    effect: &RuntimeEvaluatedEffect,
) -> bool {
    if !matches!(resolve_expr(modules, application), Ok(HirExprKind::Call(_)))
        || calls.contains_key(&application)
        || !validate_effect_operands(modules, expression_types, effect)
    {
        return false;
    }
    match effect {
        RuntimeEvaluatedEffect::Log { fields, .. }
        | RuntimeEvaluatedEffect::EmitEvent { fields, .. } => effect_fields_are_valid(fields),
        _ => true,
    }
}

fn validate_effect_operands(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expression_types: &BTreeMap<ExprId, RuntimeNormalizedType>,
    effect: &RuntimeEvaluatedEffect,
) -> bool {
    let valid = |operand: &RuntimeEvaluatedEffectOperandFact| {
        validate_evaluated_effect_operand(modules, expression_types, operand)
    };
    match effect {
        RuntimeEvaluatedEffect::Log {
            message, fields, ..
        }
        | RuntimeEvaluatedEffect::EmitEvent {
            event: message,
            fields,
        } => valid(message) && fields.iter().all(|field| valid(field.operand())),
        RuntimeEvaluatedEffect::SignalWrite { target, value }
        | RuntimeEvaluatedEffect::MetricWrite { target, value } => valid(target) && valid(value),
        RuntimeEvaluatedEffect::Panic { message }
        | RuntimeEvaluatedEffect::Fail { message }
        | RuntimeEvaluatedEffect::Bail { message } => valid(message),
        RuntimeEvaluatedEffect::Ensure { condition, message } => {
            valid(condition)
                && matches!(condition.ty().shape(), RuntimeTypeShape::Bool)
                && valid(message)
        }
        RuntimeEvaluatedEffect::Drop { target, policy } => {
            let fade_valid = match policy {
                RuntimeDropPolicyFact::Stop {
                    fade: RuntimeDropFadeFact::Operand(fade),
                } => valid(fade) && matches!(fade.ty().shape(), RuntimeTypeShape::Duration),
                RuntimeDropPolicyFact::Default
                | RuntimeDropPolicyFact::Cancel
                | RuntimeDropPolicyFact::Stop {
                    fade: RuntimeDropFadeFact::Constant(_),
                }
                | RuntimeDropPolicyFact::Finish
                | RuntimeDropPolicyFact::Release
                | RuntimeDropPolicyFact::Detach => true,
            };
            valid(target) && fade_valid
        }
    }
}

fn validate_evaluated_effect_operand(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expression_types: &BTreeMap<ExprId, RuntimeNormalizedType>,
    operand: &RuntimeEvaluatedEffectOperandFact,
) -> bool {
    if validate_normalized_type(modules, operand.ty()).is_err() {
        return false;
    }
    match operand.source() {
        RuntimeResolvedCallOperandSource::Expression(expression) => {
            resolve_expr(modules, expression).is_ok()
                && expression_types.get(&expression) == Some(operand.ty())
        }
        RuntimeResolvedCallOperandSource::CompactNumericElement { sequence, ordinal } => {
            let Some(sequence_ty) = expression_types.get(&sequence) else {
                return false;
            };
            let RuntimeTypeShape::Sequence {
                kind: RuntimeSequenceKind::Vec,
                item,
            } = sequence_ty.shape()
            else {
                return false;
            };
            if item.as_ref() != operand.ty() {
                return false;
            }
            let Ok(HirExprKind::NumericBracketSequence(sequence)) = resolve_expr(modules, sequence)
            else {
                return false;
            };
            usize::try_from(ordinal).is_ok_and(|ordinal| ordinal < sequence.elements().len())
        }
    }
}

fn effect_fields_are_valid(fields: &[RuntimeEffectFieldFact]) -> bool {
    let mut names = BTreeSet::new();
    fields
        .iter()
        .all(|field| !field.name().is_empty() && names.insert(field.name()))
}
