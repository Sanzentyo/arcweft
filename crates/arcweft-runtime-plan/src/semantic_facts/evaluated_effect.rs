use std::collections::{BTreeMap, BTreeSet};

use arcweft_core::{
    effect::{RuntimeRegistryDropTarget, RuntimeRegistryReadKind},
    time::LogicalDuration,
};
use arcweft_lang_hir::expr::{HirExprKind, HirPlaceholderKind};
use arcweft_lang_hir::identity::{ExprId, HirModuleId};
use arcweft_lang_hir::module::HirModule;

use super::{
    RuntimeNormalizedType, RuntimeResolvedCall, RuntimeResolvedCallOperandSource,
    RuntimeSemanticFactsError, RuntimeSequenceKind, RuntimeTypeShape, resolve_expr,
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
    EmitAction {
        payload: RuntimeEvaluatedEffectOperandFact,
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
    RegistryDrop {
        address: ExprId,
        argument: ExprId,
        target: RuntimeRegistryDropTarget<RuntimeNormalizedType>,
        policy: RuntimeDropPolicyFact,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEvaluatedEffectFact {
    site_root: ExprId,
    application_site: ExprId,
    result: RuntimeNormalizedType,
    effect: RuntimeEvaluatedEffect,
}

impl RuntimeEvaluatedEffectFact {
    pub const fn new(
        site_root: ExprId,
        application_site: ExprId,
        result: RuntimeNormalizedType,
        effect: RuntimeEvaluatedEffect,
    ) -> Self {
        Self {
            site_root,
            application_site,
            result,
            effect,
        }
    }

    pub const fn site_root(&self) -> ExprId {
        self.site_root
    }

    pub const fn application_site(&self) -> ExprId {
        self.application_site
    }

    /// Exact normalized result type copied from the checked terminal
    /// operation application. This remains available even when the call is a
    /// non-value evaluated-effect carrier.
    pub const fn result(&self) -> &RuntimeNormalizedType {
        &self.result
    }

    pub const fn effect(&self) -> &RuntimeEvaluatedEffect {
        &self.effect
    }
}

impl RuntimeEvaluatedEffect {
    pub(super) fn uses_pipe_address(&self) -> bool {
        matches!(self, Self::RegistryDrop { address, argument, .. } if address != argument)
    }

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
            Self::EmitAction { payload } => visit(payload.ty()),
            Self::Ensure { condition, message } => {
                visit(condition.ty());
                visit(message.ty());
            }
            Self::Drop { target, policy } => {
                visit(target.ty());
                policy.visit_operand_types(visit);
            }
            Self::RegistryDrop { target, policy, .. } => {
                visit(target.item_type());
                policy.visit_operand_types(visit);
            }
        }
    }
}

impl RuntimeDropPolicyFact {
    fn dynamic_fade(&self) -> Option<&RuntimeEvaluatedEffectOperandFact> {
        match self {
            Self::Stop {
                fade: RuntimeDropFadeFact::Operand(fade),
            } => Some(fade),
            _ => None,
        }
    }

    fn visit_operand_types<'a>(&'a self, visit: &mut impl FnMut(&'a RuntimeNormalizedType)) {
        if let Some(fade) = self.dynamic_fade() {
            visit(fade.ty());
        }
    }
}

pub(super) fn validate_evaluated_effect(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expression_types: &super::RuntimeExpressionTypeView<'_>,
    calls: &BTreeMap<ExprId, RuntimeResolvedCall>,
    expression: ExprId,
    fact: &RuntimeEvaluatedEffectFact,
) -> Result<(), RuntimeSemanticFactsError> {
    let expected_result = match fact.effect() {
        RuntimeEvaluatedEffect::Panic { .. }
        | RuntimeEvaluatedEffect::Fail { .. }
        | RuntimeEvaluatedEffect::Bail { .. } => RuntimeTypeShape::Never,
        _ => RuntimeTypeShape::Unit,
    };
    if fact.site_root() != expression
        || fact.result().shape() != &expected_result
        || !validate_evaluated_effect_site(modules, fact)
    {
        return Err(RuntimeSemanticFactsError::InvalidEvaluatedEffectFact { expression });
    }
    validate_evaluated_effect_operation(modules, expression_types, calls, fact)
        .then_some(())
        .ok_or(RuntimeSemanticFactsError::InvalidEvaluatedEffectFact { expression })
}

pub(super) fn validate_evaluated_effect_site(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    fact: &RuntimeEvaluatedEffectFact,
) -> bool {
    validate_evaluated_effect_result(modules, fact.result())
        && (fact.application_site() == fact.site_root()
            || matches!(
                resolve_expr(modules, fact.site_root()),
                Ok(HirExprKind::Pipe(pipe)) if pipe.right() == fact.application_site()
            ))
}

pub(super) fn validate_evaluated_effect_result(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    result: &RuntimeNormalizedType,
) -> bool {
    validate_normalized_type(modules, result).is_ok()
}

pub(super) fn validate_evaluated_effect_operation(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expression_types: &super::RuntimeExpressionTypeView<'_>,
    calls: &BTreeMap<ExprId, RuntimeResolvedCall>,
    fact: &RuntimeEvaluatedEffectFact,
) -> bool {
    let application = fact.application_site();
    let effect = fact.effect();
    let Ok(HirExprKind::Call(call)) = resolve_expr(modules, application) else {
        return false;
    };
    if calls.contains_key(&application)
        || !validate_effect_operands(modules, expression_types, effect)
    {
        return false;
    }
    match effect {
        RuntimeEvaluatedEffect::Log { fields, .. }
        | RuntimeEvaluatedEffect::EmitEvent { fields, .. } => effect_fields_are_valid(fields),
        RuntimeEvaluatedEffect::RegistryDrop {
            address,
            argument,
            target,
            ..
        } => {
            let uses_argument = call
                .arguments()
                .iter()
                .any(|value| value.value() == *argument);
            let same_address = address == argument;
            let pipe_address = matches!(
                resolve_expr(modules, fact.site_root()),
                Ok(HirExprKind::Pipe(pipe)) if pipe.left() == *address && pipe.right() == application
            ) && matches!(
                resolve_expr(modules, *argument),
                Ok(HirExprKind::Placeholder(HirPlaceholderKind::PipeLeft))
            );
            uses_argument
                && (same_address || pipe_address)
                && validate_registry_drop_target(modules, *address, target)
        }
        _ => true,
    }
}

fn validate_effect_operands(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expression_types: &super::RuntimeExpressionTypeView<'_>,
    effect: &RuntimeEvaluatedEffect,
) -> bool {
    let valid = |operand: &RuntimeEvaluatedEffectOperandFact| {
        validate_evaluated_effect_operand(modules, expression_types, operand)
    };
    let fade_valid = |fade: &RuntimeEvaluatedEffectOperandFact| {
        valid(fade) && matches!(fade.ty().shape(), RuntimeTypeShape::Duration)
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
        RuntimeEvaluatedEffect::EmitAction { payload } => valid(payload),
        RuntimeEvaluatedEffect::Ensure { condition, message } => {
            valid(condition)
                && matches!(condition.ty().shape(), RuntimeTypeShape::Bool)
                && valid(message)
        }
        RuntimeEvaluatedEffect::Drop { target, policy } => {
            valid(target) && policy.dynamic_fade().is_none_or(fade_valid)
        }
        RuntimeEvaluatedEffect::RegistryDrop { policy, .. } => {
            policy.dynamic_fade().is_none_or(fade_valid)
        }
    }
}

fn validate_registry_drop_target(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    address: ExprId,
    target: &RuntimeRegistryDropTarget<RuntimeNormalizedType>,
) -> bool {
    let Ok(HirExprKind::LifetimePath(path)) = resolve_expr(modules, address) else {
        return false;
    };
    let Some(path) = path.as_resolved() else {
        return false;
    };
    path.key().as_ref() == target.key()
        && path.optional() == (target.presence() == RuntimeRegistryReadKind::Optional)
        && validate_normalized_type(modules, target.item_type()).is_ok()
}

fn validate_evaluated_effect_operand(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expression_types: &super::RuntimeExpressionTypeView<'_>,
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
