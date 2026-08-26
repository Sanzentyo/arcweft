//! Recursive admission from semantic seeds into the sole executable carriers.

use std::collections::BTreeSet;

use crate::audio::RuntimeAudioCommand;
use crate::effect::{LineEffectRequest, RuntimeEffectExpr, RuntimeEffectFieldExpr};
use crate::entry::TypeLayoutHash;
use crate::pattern::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimePattern,
    RuntimePatternBindingCoordinate, RuntimePatternBindingPath, RuntimePatternBindingStep,
    RuntimePatternKind, RuntimePatternRest, RuntimeRecordPatternField, RuntimeVariantIdentity,
};
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};
use crate::stream::{StreamMatchArm, StreamOp, StreamPlan};
use crate::task::{
    AwaitManyTarget, AwaitTarget, HostTaskRequestTemplate, NamedHostArg,
    RuntimeHostArgumentTemplate,
};
use crate::value::{
    RuntimeAgentConstructor, RuntimeAgentExpr, RuntimeAgentFieldOwner, RuntimeAgentFieldResult,
    RuntimeBinaryOp, RuntimeCallArgument, RuntimeCallArgumentMode, RuntimeExpr, RuntimeExprKind,
    RuntimeExprMatchArm, RuntimeFieldProjection, RuntimeNominalRecordExpr, RuntimeRange,
    RuntimeRecordFieldId, RuntimeRecordFieldIdError, RuntimeReductionProducer,
    RuntimeSignedIntWidth, RuntimeUnaryOp, RuntimeUnsignedIntWidth, RuntimeValue,
};

use super::super::{
    ChoiceRuntimeOption, FlowOp, RuntimeAgentOperationalType, RuntimeAgentTypeProjection,
    RuntimeBuiltinIteratorEvidence, RuntimeBuiltinIteratorFamily, RuntimeHostCallTarget,
    RuntimeIteratorEvidence, RuntimeIteratorWitnessEvidence, RuntimeIteratorWitnessExecutable,
    RuntimeMatchArm, RuntimePlanSequenceKind, RuntimePlanTypeProjection, RuntimePureInputType,
    RuntimePureOutputType, RuntimeReceiverMode,
};
use super::{
    RuntimeAgentExprSeed, RuntimeAudioCommandSeed, RuntimeBuiltinIteratorEvidenceSeed,
    RuntimeCallArgumentSeed, RuntimeChoiceOptionSeed, RuntimeEvaluatedEffectSeed,
    RuntimeExprMatchArmSeed, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFieldProjectionSeed,
    RuntimeFlowMatchArmSeed, RuntimeFlowOpSeed, RuntimeHostArgumentSeed, RuntimeHostCallTargetSeed,
    RuntimeHostTaskRequestTemplateSeed, RuntimeIteratorEvidenceSeed,
    RuntimeIteratorWitnessEvidenceSeed, RuntimeIteratorWitnessExecutableSeed,
    RuntimeLineEffectSeed, RuntimeLocalSeedId, RuntimeNominalRecordFieldSeed,
    RuntimePatternRestSeed, RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlanBuildError,
    RuntimePlanBuilder, RuntimeRecordFieldSeedId, RuntimeStreamMatchArmSeed, RuntimeStreamOpSeed,
    RuntimeStreamPlanSeed,
};

impl RuntimePlanBuilder {
    fn lower_pattern_seed(
        &self,
        seed: RuntimePatternSeed,
    ) -> Result<RuntimePattern, RuntimePlanBuildError> {
        self.lower_pattern(seed, &mut PatternAdmission::default(), &mut Vec::new())
    }

    #[cfg(test)]
    pub(crate) fn lower_pattern_seed_for_test(
        &self,
        seed: RuntimePatternSeed,
    ) -> Result<RuntimePattern, RuntimePlanBuildError> {
        self.lower_pattern_seed(seed)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the match is the exhaustive expression-admission authority"
    )]
    pub(super) fn lower_expression(
        &self,
        seed: RuntimeExprSeed,
    ) -> Result<RuntimeExpr, RuntimePlanBuildError> {
        let (semantic_ty, kind) = seed.into_parts();
        let ty = self.resolve_seed_type("expression", semantic_ty)?;
        let kind = match kind {
            RuntimeExprSeedKind::Value(value) => {
                self.validate_plan_value("expression constant", ty, &value)?;
                RuntimeExprKind::Value(value)
            }
            RuntimeExprSeedKind::Agent(agent) => {
                RuntimeExprKind::Agent(self.lower_agent_expression(ty, agent)?)
            }
            RuntimeExprSeedKind::Local(local) => {
                let (local, local_ty) = self.resolve_local(&local)?;
                require_same("local expression", ty, local_ty)?;
                RuntimeExprKind::Local(local)
            }
            RuntimeExprSeedKind::EntityRef(entity) => {
                self.require_projection("entity-reference expression", ty, |projection| {
                    matches!(projection, RuntimePlanTypeProjection::EntityReference)
                })?;
                RuntimeExprKind::EntityRef(entity)
            }
            RuntimeExprSeedKind::Let {
                binding,
                expr,
                body,
            } => {
                let (binding, binding_ty) = self.resolve_local(&binding)?;
                let expr = self.lower_expression(*expr)?;
                let body = self.lower_expression(*body)?;
                require_same("let binding", binding_ty, expr.ty())?;
                require_same("let body", ty, body.ty())?;
                RuntimeExprKind::Let {
                    binding,
                    expr: Box::new(expr),
                    body: Box::new(body),
                }
            }
            RuntimeExprSeedKind::Tuple(items) => {
                let expected = match self.projection(ty)? {
                    RuntimePlanTypeProjection::Tuple(items) => items.as_ref(),
                    _ => return invalid_projection("tuple expression", ty),
                };
                if expected.len() != items.len() {
                    return invalid_projection("tuple expression arity", ty);
                }
                let mut lowered = Vec::with_capacity(items.len());
                for (item, expected) in items.into_vec().into_iter().zip(expected) {
                    let item = self.lower_expression(item)?;
                    require_same("tuple element", *expected, item.ty())?;
                    lowered.push(item);
                }
                RuntimeExprKind::Tuple(lowered)
            }
            RuntimeExprSeedKind::BracketSeq(items) => {
                let (item_ty, fixed_len) = self.sequence_projection(ty, "sequence expression")?;
                if let Some(expected) = fixed_len {
                    validate_sequence_length(expected, items.len())?;
                }
                let mut lowered = Vec::with_capacity(items.len());
                for item in items {
                    let item = self.lower_expression(item)?;
                    require_same("sequence element", item_ty, item.ty())?;
                    lowered.push(item);
                }
                RuntimeExprKind::BracketSeq(lowered)
            }
            RuntimeExprSeedKind::RepeatSeq { value, len } => {
                let (item_ty, fixed_len) = self.sequence_projection(ty, "repeat expression")?;
                if let Some(expected) = fixed_len {
                    validate_sequence_length(expected, len)?;
                }
                let value = self.lower_expression(*value)?;
                require_same("repeat element", item_ty, value.ty())?;
                RuntimeExprKind::RepeatSeq {
                    value: Box::new(value),
                    len,
                }
            }
            RuntimeExprSeedKind::Range {
                start,
                end,
                inclusive,
            } => {
                if start.is_none() && end.is_none() {
                    return Err(RuntimePlanBuildError::EmptyRangeExpression);
                }
                let item_ty = match self.projection(ty)? {
                    RuntimePlanTypeProjection::Range(item) => *item,
                    _ => return invalid_projection("range expression", ty),
                };
                let start = self.lower_optional_expression(start)?;
                let end = self.lower_optional_expression(end)?;
                for bound in start.iter().chain(end.iter()) {
                    require_same("range bound", item_ty, bound.ty())?;
                }
                RuntimeExprKind::Range {
                    start: start.map(Box::new),
                    end: end.map(Box::new),
                    inclusive,
                }
            }
            RuntimeExprSeedKind::NominalRecord(fields) => {
                RuntimeExprKind::NominalRecord(self.lower_nominal_record(ty, fields)?)
            }
            RuntimeExprSeedKind::Variant { ordinal, payload } => {
                let payload = self.lower_optional_expression(payload)?;
                self.validate_variant_payload(ty, ordinal, payload.as_ref().map(RuntimeExpr::ty))?;
                RuntimeExprKind::Variant {
                    ordinal,
                    payload: payload.map(Box::new),
                }
            }
            RuntimeExprSeedKind::Field { target, field } => {
                let target = self.lower_expression(*target)?;
                let field = self.lower_field_projection(ty, target.ty(), &field)?;
                RuntimeExprKind::Field {
                    target: Box::new(target),
                    field,
                }
            }
            RuntimeExprSeedKind::ProjectTuple { target, ordinal } => {
                let target = self.lower_expression(*target)?;
                let ordinal = usize::try_from(ordinal).map_err(|_| {
                    RuntimePlanBuildError::InvalidTypeProjection {
                        context: "tuple projection ordinal",
                        ty: target.ty(),
                    }
                })?;
                let expected = match self.projection(target.ty())? {
                    RuntimePlanTypeProjection::Tuple(items) => items.get(ordinal).copied(),
                    _ => None,
                }
                .ok_or(RuntimePlanBuildError::InvalidTypeProjection {
                    context: "tuple projection",
                    ty: target.ty(),
                })?;
                require_same("tuple projection result", expected, ty)?;
                RuntimeExprKind::ProjectTuple {
                    target: Box::new(target),
                    ordinal,
                }
            }
            RuntimeExprSeedKind::ProjectRecord { target, field } => {
                let target = self.lower_expression(*target)?;
                let (field, field_ty) = self.resolve_record_field(target.ty(), field)?;
                require_same("record projection result", field_ty, ty)?;
                RuntimeExprKind::ProjectRecord {
                    target: Box::new(target),
                    ordinal: usize::try_from(field.zero_based()).map_err(|_| {
                        RuntimePlanBuildError::RecordFieldIdentity(
                            RuntimeRecordFieldIdError::OrdinalOverflow,
                        )
                    })?,
                }
            }
            RuntimeExprSeedKind::AssignNominalField {
                base,
                owner,
                field,
                expr,
                body,
            } => {
                let (base, base_ty) = base
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignLocalSeed)?;
                let owner = self.resolve_seed_type("assigned record owner", owner)?;
                require_same("assigned record target", owner, base_ty)?;
                let (field, field_ty) = self.resolve_record_field(owner, field)?;
                let expr = self.lower_expression(*expr)?;
                let body = self.lower_expression(*body)?;
                require_same("assigned record field", field_ty, expr.ty())?;
                require_same("assignment body", ty, body.ty())?;
                RuntimeExprKind::AssignNominalField {
                    base,
                    field,
                    expr: Box::new(expr),
                    body: Box::new(body),
                }
            }
            RuntimeExprSeedKind::Call { callee, args } => RuntimeExprKind::Call {
                callee,
                args: self.lower_call_arguments(args)?,
            },
            RuntimeExprSeedKind::Function(site) => {
                let (site, parameters, result) = site
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
                match self.projection(ty)? {
                    RuntimePlanTypeProjection::Function {
                        parameters: expected,
                        result: expected_result,
                    } if expected.as_ref() == parameters && *expected_result == result => {}
                    _ => return invalid_projection("function expression", ty),
                }
                RuntimeExprKind::Function(site)
            }
            RuntimeExprSeedKind::Apply { callee, args } => {
                let callee = self.lower_expression(*callee)?;
                let (parameters, result) = match self.projection(callee.ty())? {
                    RuntimePlanTypeProjection::Function { parameters, result } => {
                        (parameters.clone(), *result)
                    }
                    _ => return invalid_projection("function application callee", callee.ty()),
                };
                let args = self.lower_call_arguments(args)?;
                let actual = self.expanded_argument_types(&args)?;
                if parameters.as_ref() != actual.as_slice() {
                    return invalid_projection("function application arguments", callee.ty());
                }
                require_same("function application result", result, ty)?;
                RuntimeExprKind::Apply {
                    callee: Box::new(callee),
                    args,
                }
            }
            RuntimeExprSeedKind::TraitCall {
                callable,
                receiver,
                args,
            } => {
                let (callable, receiver_mode, parameters, result) = callable
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignTraitMethodSeed)?;
                if self.trait_methods.get(callable.0).is_none() {
                    return Err(RuntimePlanBuildError::ForeignTraitMethodSeed);
                }
                let Some((&receiver_ty, argument_types)) = parameters.split_first() else {
                    return Err(RuntimePlanBuildError::MissingTraitMethodReceiver);
                };
                let receiver = self.lower_expression(*receiver)?;
                require_same("trait-call receiver", receiver_ty, receiver.ty())?;
                let args = self.lower_call_arguments(args)?;
                let actual = self.expanded_argument_types(&args)?;
                if argument_types != actual.as_slice() {
                    return invalid_projection("trait-call arguments", ty);
                }
                require_same("trait-call result", result, ty)?;
                RuntimeExprKind::TraitCall {
                    callable,
                    receiver: Box::new(receiver),
                    receiver_mode,
                    args,
                }
            }
            RuntimeExprSeedKind::PureCall { helper, args } => {
                let (helper, parameters, result) = helper
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignPureHelperSeed)?;
                if self.pure_helpers.get(helper.0).is_none() {
                    return Err(RuntimePlanBuildError::ForeignPureHelperSeed);
                }
                let args = self.lower_call_arguments(args)?;
                let actual = self.expanded_argument_types(&args)?;
                if parameters != actual.as_slice() {
                    return invalid_projection("pure-call arguments", ty);
                }
                require_same("pure-call result", result, ty)?;
                RuntimeExprKind::PureCall { helper, args }
            }
            RuntimeExprSeedKind::Map {
                source,
                param,
                body,
            } => {
                let source = self.lower_expression(*source)?;
                let (source_item, _) =
                    self.sequence_projection(source.ty(), "map source expression")?;
                let (param, param_ty) = self.resolve_local(&param)?;
                require_same("map parameter", source_item, param_ty)?;
                let body = self.lower_expression(*body)?;
                let (result_item, _) = self.sequence_projection(ty, "map result expression")?;
                require_same("map result element", result_item, body.ty())?;
                RuntimeExprKind::Map {
                    source: Box::new(source),
                    param,
                    body: Box::new(body),
                }
            }
            RuntimeExprSeedKind::Filter {
                source,
                param,
                body,
            } => {
                let source = self.lower_expression(*source)?;
                let (source_item, _) =
                    self.sequence_projection(source.ty(), "filter source expression")?;
                let (param, param_ty) = self.resolve_local(&param)?;
                require_same("filter parameter", source_item, param_ty)?;
                let body = self.lower_expression(*body)?;
                self.require_bool("filter predicate", body.ty())?;
                require_same("filter result", source.ty(), ty)?;
                RuntimeExprKind::Filter {
                    source: Box::new(source),
                    param,
                    body: Box::new(body),
                }
            }
            RuntimeExprSeedKind::Sum { source } => {
                let source = self.lower_expression(*source)?;
                let (item, _) = self.sequence_projection(source.ty(), "sum source expression")?;
                self.require_numeric("sum element", item)?;
                require_same("sum result", item, ty)?;
                RuntimeExprKind::Sum {
                    source: Box::new(source),
                }
            }
            RuntimeExprSeedKind::Unary { op, expr } => {
                let expr = self.lower_expression(*expr)?;
                self.validate_unary(op, expr.ty(), ty)?;
                RuntimeExprKind::Unary {
                    op,
                    expr: Box::new(expr),
                }
            }
            RuntimeExprSeedKind::Binary { lhs, op, rhs } => {
                let lhs = self.lower_expression(*lhs)?;
                let rhs = self.lower_expression(*rhs)?;
                self.validate_binary(op, lhs.ty(), rhs.ty(), ty)?;
                RuntimeExprKind::Binary {
                    lhs: Box::new(lhs),
                    op,
                    rhs: Box::new(rhs),
                }
            }
            RuntimeExprSeedKind::If {
                condition,
                then_expr,
                else_expr,
            } => {
                let condition = self.lower_expression(*condition)?;
                let then_expr = self.lower_expression(*then_expr)?;
                let else_expr = self.lower_expression(*else_expr)?;
                self.require_bool("if condition", condition.ty())?;
                require_same("if then branch", ty, then_expr.ty())?;
                require_same("if else branch", ty, else_expr.ty())?;
                RuntimeExprKind::If {
                    condition: Box::new(condition),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                }
            }
            RuntimeExprSeedKind::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let expr = self.lower_expression(*expr)?;
                require_same("if-let scrutinee", pattern.ty(), expr.ty())?;
                let guard = self.lower_optional_expression(guard)?;
                if let Some(guard) = &guard {
                    self.require_bool("if-let guard", guard.ty())?;
                }
                let then_expr = self.lower_expression(*then_expr)?;
                let else_expr = self.lower_expression(*else_expr)?;
                require_same("if-let then branch", ty, then_expr.ty())?;
                require_same("if-let else branch", ty, else_expr.ty())?;
                RuntimeExprKind::IfLet {
                    pattern,
                    expr: Box::new(expr),
                    guard: guard.map(Box::new),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                }
            }
            RuntimeExprSeedKind::Match { scrutinee, arms } => {
                let scrutinee = self.lower_expression(*scrutinee)?;
                let mut lowered = Vec::with_capacity(arms.len());
                for arm in arms {
                    lowered.push(self.lower_match_arm(scrutinee.ty(), ty, arm)?);
                }
                RuntimeExprKind::Match {
                    scrutinee: Box::new(scrutinee),
                    arms: lowered,
                }
            }
            RuntimeExprSeedKind::ReductionUnchanged { state } => {
                let state = self.lower_expression(*state)?;
                let state_ty = match self.projection(state.ty())? {
                    RuntimePlanTypeProjection::Reference(inner) => *inner,
                    _ => state.ty(),
                };
                match self.projection(ty)? {
                    RuntimePlanTypeProjection::Opaque {
                        producer,
                        admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                        value_class: crate::value::RuntimeOpaqueValueClass::Plain,
                        persistence: crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
                        arguments,
                    } if RuntimeReductionProducer::accepts(producer)
                        && arguments.as_ref() == [state_ty] => {}
                    _ => return Err(RuntimePlanBuildError::InvalidReductionUnchanged { ty }),
                }
                RuntimeExprKind::ReductionUnchanged {
                    state: Box::new(state),
                }
            }
        };
        Ok(RuntimeExpr::from_admitted_parts(ty, kind))
    }

    fn lower_optional_expression(
        &self,
        seed: Option<Box<RuntimeExprSeed>>,
    ) -> Result<Option<RuntimeExpr>, RuntimePlanBuildError> {
        seed.map(|seed| self.lower_expression(*seed)).transpose()
    }

    fn lower_match_arm(
        &self,
        scrutinee_ty: RuntimePlanTypeId,
        result_ty: RuntimePlanTypeId,
        arm: RuntimeExprMatchArmSeed,
    ) -> Result<RuntimeExprMatchArm, RuntimePlanBuildError> {
        let (pattern, guard, value) = arm.into_parts();
        let pattern = self.lower_pattern_seed(pattern)?;
        require_same("match arm pattern", scrutinee_ty, pattern.ty())?;
        let guard = guard
            .map(|guard| self.lower_expression(guard))
            .transpose()?;
        if let Some(guard) = &guard {
            self.require_bool("match arm guard", guard.ty())?;
        }
        let value = self.lower_expression(value)?;
        require_same("match arm value", result_ty, value.ty())?;
        Ok(RuntimeExprMatchArm::from_admitted_parts(
            pattern, guard, value,
        ))
    }
}

#[derive(Default)]
struct PatternAdmission {
    bindings: BTreeSet<RuntimeLocalDeclarationId>,
}

fn require_same(
    context: &'static str,
    expected: RuntimePlanTypeId,
    actual: RuntimePlanTypeId,
) -> Result<(), RuntimePlanBuildError> {
    if expected == actual {
        Ok(())
    } else {
        Err(RuntimePlanBuildError::TypeMismatch {
            context,
            expected,
            actual,
        })
    }
}

fn invalid_projection<T>(
    context: &'static str,
    ty: RuntimePlanTypeId,
) -> Result<T, RuntimePlanBuildError> {
    Err(RuntimePlanBuildError::InvalidTypeProjection { context, ty })
}

fn validate_sequence_length(expected: u64, actual: usize) -> Result<(), RuntimePlanBuildError> {
    if usize::try_from(expected).ok() == Some(actual) {
        Ok(())
    } else {
        Err(RuntimePlanBuildError::SequenceLengthMismatch { expected, actual })
    }
}

impl RuntimePlanBuilder {
    pub(super) fn resolve_seed_type(
        &self,
        context: &'static str,
        semantic_identity: crate::pattern::RuntimeSemanticTypeId,
    ) -> Result<RuntimePlanTypeId, RuntimePlanBuildError> {
        self.types.id_for_semantic(semantic_identity).ok_or(
            RuntimePlanBuildError::UnknownSeedType {
                context,
                semantic_identity,
            },
        )
    }

    fn projection(
        &self,
        ty: RuntimePlanTypeId,
    ) -> Result<&RuntimePlanTypeProjection<RuntimePlanTypeId>, RuntimePlanBuildError> {
        self.types
            .get(ty)
            .map(super::super::RuntimePlanTypeDeclaration::projection)
            .ok_or(RuntimePlanBuildError::InvalidTypeProjection {
                context: "plan-local type identity",
                ty,
            })
    }

    fn require_projection(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
        accepts: impl FnOnce(&RuntimePlanTypeProjection<RuntimePlanTypeId>) -> bool,
    ) -> Result<(), RuntimePlanBuildError> {
        if accepts(self.projection(ty)?) {
            Ok(())
        } else {
            invalid_projection(context, ty)
        }
    }

    fn resolve_local(
        &self,
        local: &RuntimeLocalSeedId,
    ) -> Result<(RuntimeLocalDeclarationId, RuntimePlanTypeId), RuntimePlanBuildError> {
        let (local, ty) = local
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignLocalSeed)?;
        if !self.locals.contains(local) {
            return Err(RuntimePlanBuildError::UnknownFunctionLocal { local });
        }
        Ok((local, ty))
    }

    fn sequence_projection(
        &self,
        ty: RuntimePlanTypeId,
        context: &'static str,
    ) -> Result<(RuntimePlanTypeId, Option<u64>), RuntimePlanBuildError> {
        match self.projection(ty)? {
            RuntimePlanTypeProjection::Sequence { item, .. } => Ok((*item, None)),
            RuntimePlanTypeProjection::Array { item, length } => Ok((*item, Some(*length))),
            _ => invalid_projection(context, ty),
        }
    }

    fn lower_nominal_record(
        &self,
        owner: RuntimePlanTypeId,
        fields: Box<[RuntimeNominalRecordFieldSeed]>,
    ) -> Result<RuntimeNominalRecordExpr, RuntimePlanBuildError> {
        let domain = self
            .nominal_record_domains
            .get(owner)
            .ok_or(RuntimePlanBuildError::UnknownNominalRecordDomain { owner })?;
        let mut seen = BTreeSet::new();
        let mut lowered = Vec::with_capacity(fields.len());
        for field in fields {
            let (field, value) = field.into_parts();
            let (field, expected) = self.resolve_record_field(owner, field)?;
            if !seen.insert(field) {
                return Err(RuntimePlanBuildError::DuplicateRecordField { owner, field });
            }
            let value = self.lower_expression(value)?;
            require_same("nominal record field", expected, value.ty())?;
            lowered.push((field, value));
        }
        for ordinal in 0..domain.fields().len() {
            let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)?;
            if !seen.contains(&field) {
                return Err(RuntimePlanBuildError::MissingRecordField { owner, field });
            }
        }
        Ok(RuntimeNominalRecordExpr::from_admitted_parts(lowered))
    }

    fn resolve_record_field(
        &self,
        owner: RuntimePlanTypeId,
        field: RuntimeRecordFieldSeedId,
    ) -> Result<(RuntimeRecordFieldId, RuntimePlanTypeId), RuntimePlanBuildError> {
        let domain = self
            .nominal_record_domains
            .get(owner)
            .ok_or(RuntimePlanBuildError::UnknownNominalRecordDomain { owner })?;
        let ordinal = usize::try_from(field.zero_based()).map_err(|_| {
            RuntimePlanBuildError::RecordFieldIdentity(RuntimeRecordFieldIdError::OrdinalOverflow)
        })?;
        let admitted = RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)?;
        let field_ty = domain
            .fields()
            .get(ordinal)
            .map(super::super::RuntimeNominalRecordDomainField::ty)
            .ok_or(RuntimePlanBuildError::UnknownRecordField {
                owner,
                ordinal: field.zero_based(),
            })?;
        Ok((admitted, field_ty))
    }

    fn lower_field_projection(
        &self,
        result_ty: RuntimePlanTypeId,
        target_ty: RuntimePlanTypeId,
        field: &RuntimeFieldProjectionSeed,
    ) -> Result<RuntimeFieldProjection, RuntimePlanBuildError> {
        match field {
            RuntimeFieldProjectionSeed::Nominal { owner, field } => {
                let owner = self.resolve_seed_type("nominal field owner", *owner)?;
                require_same("nominal field target", owner, target_ty)?;
                let (field, field_ty) = self.resolve_record_field(owner, *field)?;
                require_same("nominal field result", field_ty, result_ty)?;
                Ok(RuntimeFieldProjection::Nominal(field))
            }
            RuntimeFieldProjectionSeed::OpaqueRecord {
                owner,
                producer,
                field,
                field_type,
            } => {
                let semantic_owner = *owner;
                let owner = self.resolve_seed_type("opaque record field owner", semantic_owner)?;
                require_same("opaque record field target", owner, target_ty)?;
                let expected_result =
                    self.resolve_seed_type("opaque record field result", *field_type)?;
                require_same("opaque record field result", expected_result, result_ty)?;
                let opaque_owner = match self.projection(target_ty)? {
                    RuntimePlanTypeProjection::Opaque {
                        producer: accepted,
                        admission,
                        value_class,
                        persistence,
                        ..
                    } if accepted == producer
                        && *admission == RuntimeOpaqueTypeAdmission::ExactIdentity =>
                    {
                        RuntimeOpaqueTypeOwner::with_admission(
                            accepted.clone(),
                            semantic_owner,
                            *admission,
                            *value_class,
                            *persistence,
                        )
                    }
                    _ => return invalid_projection("opaque record field target", target_ty),
                };
                let ordinal = usize::try_from(field.zero_based()).map_err(|_| {
                    RuntimePlanBuildError::RecordFieldIdentity(
                        RuntimeRecordFieldIdError::OrdinalOverflow,
                    )
                })?;
                Ok(RuntimeFieldProjection::OpaqueRecord {
                    owner: opaque_owner,
                    field: RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)?,
                })
            }
            RuntimeFieldProjectionSeed::Agent(field) => {
                if !self.agent_field_owner_matches(target_ty, field.owner())? {
                    return invalid_projection("Agent field owner", target_ty);
                }
                if !self.agent_field_result_matches(result_ty, field.result())? {
                    return invalid_projection("Agent field result", result_ty);
                }
                Ok(RuntimeFieldProjection::Agent(*field))
            }
            RuntimeFieldProjectionSeed::EntityReference(field) => {
                self.require_projection(
                    "entity-reference field target",
                    target_ty,
                    |projection| matches!(projection, RuntimePlanTypeProjection::EntityReference),
                )?;
                if !self.is_string(result_ty)? {
                    return invalid_projection("entity-reference field result", result_ty);
                }
                Ok(RuntimeFieldProjection::EntityReference(*field))
            }
            RuntimeFieldProjectionSeed::Progress(field) => {
                self.require_projection("Progress field target", target_ty, |projection| {
                    matches!(projection, RuntimePlanTypeProjection::Progress)
                })?;
                let result_matches = match (field, self.projection(result_ty)?) {
                    (crate::value::RuntimeProgressField::Ratio, RuntimePlanTypeProjection::F32) => {
                        true
                    }
                    (
                        crate::value::RuntimeProgressField::Label,
                        RuntimePlanTypeProjection::Option(item),
                    ) => self.is_string(*item)?,
                    _ => false,
                };
                if !result_matches {
                    return invalid_projection("Progress field result", result_ty);
                }
                Ok(RuntimeFieldProjection::Progress(*field))
            }
        }
    }

    fn validate_variant_payload(
        &self,
        owner: RuntimePlanTypeId,
        ordinal: u32,
        actual: Option<RuntimePlanTypeId>,
    ) -> Result<(), RuntimePlanBuildError> {
        let expected = self.variant_payload_type(owner, ordinal)?;
        if expected == actual {
            Ok(())
        } else {
            Err(RuntimePlanBuildError::VariantPayloadMismatch {
                owner,
                ordinal,
                expected,
                actual,
            })
        }
    }

    fn variant_payload_type(
        &self,
        owner: RuntimePlanTypeId,
        ordinal: u32,
    ) -> Result<Option<RuntimePlanTypeId>, RuntimePlanBuildError> {
        if let Some(domain) = self.variant_domains.get(owner) {
            return domain
                .case(ordinal)
                .map(super::super::RuntimeVariantCase::payload)
                .ok_or(RuntimePlanBuildError::UnknownVariantCase { owner, ordinal });
        }
        match (self.projection(owner)?, ordinal) {
            (RuntimePlanTypeProjection::Option(item), 0) => Ok(Some(*item)),
            (RuntimePlanTypeProjection::Option(_), 1) => Ok(None),
            (RuntimePlanTypeProjection::Result { value, .. }, 0) => Ok(Some(*value)),
            (RuntimePlanTypeProjection::Result { error, .. }, 1) => Ok(Some(*error)),
            _ => Err(RuntimePlanBuildError::UnknownVariantCase { owner, ordinal }),
        }
    }

    fn lower_call_arguments(
        &self,
        args: Box<[RuntimeCallArgumentSeed]>,
    ) -> Result<Vec<RuntimeCallArgument>, RuntimePlanBuildError> {
        args.into_vec()
            .into_iter()
            .map(|argument| {
                let (value, mode) = argument.into_parts();
                let value = self.lower_expression(value)?;
                if mode == RuntimeCallArgumentMode::Spread {
                    self.require_spreadable(value.ty())?;
                }
                Ok(RuntimeCallArgument::from_admitted_parts(value, mode))
            })
            .collect()
    }

    fn require_spreadable(&self, ty: RuntimePlanTypeId) -> Result<(), RuntimePlanBuildError> {
        if matches!(
            self.projection(ty)?,
            RuntimePlanTypeProjection::Tuple(_)
                | RuntimePlanTypeProjection::Sequence { .. }
                | RuntimePlanTypeProjection::Array { .. }
        ) {
            Ok(())
        } else {
            Err(RuntimePlanBuildError::IndeterminateSpreadArgument { ty })
        }
    }

    fn expanded_argument_types(
        &self,
        args: &[RuntimeCallArgument],
    ) -> Result<Vec<RuntimePlanTypeId>, RuntimePlanBuildError> {
        let mut result = Vec::new();
        for argument in args {
            match argument.mode() {
                RuntimeCallArgumentMode::Value => result.push(argument.value().ty()),
                RuntimeCallArgumentMode::Spread => match self.projection(argument.value().ty())? {
                    RuntimePlanTypeProjection::Tuple(items) => result.extend(items.iter().copied()),
                    RuntimePlanTypeProjection::Array { item, length } => {
                        let count = usize::try_from(*length).map_err(|_| {
                            RuntimePlanBuildError::IndeterminateSpreadArgument {
                                ty: argument.value().ty(),
                            }
                        })?;
                        result.try_reserve(count).map_err(|_| {
                            RuntimePlanBuildError::IndeterminateSpreadArgument {
                                ty: argument.value().ty(),
                            }
                        })?;
                        result.extend(std::iter::repeat_n(*item, count));
                    }
                    _ => {
                        return Err(RuntimePlanBuildError::IndeterminateSpreadArgument {
                            ty: argument.value().ty(),
                        });
                    }
                },
            }
        }
        Ok(result)
    }

    fn validate_unary(
        &self,
        op: RuntimeUnaryOp,
        operand: RuntimePlanTypeId,
        result: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        match op {
            RuntimeUnaryOp::Not => {
                self.require_bool("logical-not operand", operand)?;
                self.require_bool("logical-not result", result)
            }
            RuntimeUnaryOp::Neg => {
                self.require_negatable("numeric-negation operand", operand)?;
                require_same("numeric-negation result", operand, result)
            }
        }
    }

    fn validate_binary(
        &self,
        op: RuntimeBinaryOp,
        lhs: RuntimePlanTypeId,
        rhs: RuntimePlanTypeId,
        result: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        match op {
            RuntimeBinaryOp::Eq | RuntimeBinaryOp::Ne => {
                require_same("equality operands", lhs, rhs)?;
                self.require_bool("equality result", result)
            }
            RuntimeBinaryOp::Lt
            | RuntimeBinaryOp::Le
            | RuntimeBinaryOp::Gt
            | RuntimeBinaryOp::Ge => {
                require_same("comparison operands", lhs, rhs)?;
                self.require_comparable("comparison operand", lhs)?;
                self.require_bool("comparison result", result)
            }
            RuntimeBinaryOp::Add
            | RuntimeBinaryOp::Sub
            | RuntimeBinaryOp::Mul
            | RuntimeBinaryOp::Div => {
                require_same("arithmetic operands", lhs, rhs)?;
                self.require_numeric("arithmetic operand", lhs)?;
                require_same("arithmetic result", lhs, result)
            }
            RuntimeBinaryOp::And | RuntimeBinaryOp::Or => {
                self.require_bool("logical lhs", lhs)?;
                self.require_bool("logical rhs", rhs)?;
                self.require_bool("logical result", result)
            }
        }
    }

    pub(super) fn require_bool(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        if self.is_bool(ty)? {
            Ok(())
        } else {
            invalid_projection(context, ty)
        }
    }

    pub(super) fn validate_callable_input_abi(
        &self,
        context: &'static str,
        inputs: &[(RuntimeLocalDeclarationId, RuntimePlanTypeId)],
        abi: &[RuntimePureInputType],
    ) -> Result<(), RuntimePlanBuildError> {
        if inputs.len() != abi.len() {
            return Err(RuntimePlanBuildError::CallableAbiArity {
                context,
                expected: inputs.len(),
                actual: abi.len(),
            });
        }
        for (index, ((_, ty), abi)) in inputs.iter().zip(abi).enumerate() {
            if !self.scalar_input_abi_matches(*ty, *abi)? {
                return Err(RuntimePlanBuildError::CallableInputAbi {
                    context,
                    index,
                    ty: *ty,
                });
            }
        }
        Ok(())
    }

    pub(super) fn validate_callable_output_abi(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
        abi: RuntimePureOutputType,
    ) -> Result<(), RuntimePlanBuildError> {
        if self.scalar_output_abi_matches(ty, abi)? {
            Ok(())
        } else {
            Err(RuntimePlanBuildError::CallableOutputAbi { context, ty })
        }
    }

    fn scalar_input_abi_matches(
        &self,
        ty: RuntimePlanTypeId,
        abi: RuntimePureInputType,
    ) -> Result<bool, RuntimePlanBuildError> {
        let projection = self.projection(ty)?;
        Ok(match abi {
            RuntimePureInputType::I8 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I8)
            ),
            RuntimePureInputType::I16 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I16)
            ),
            RuntimePureInputType::I32 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I32)
            ),
            RuntimePureInputType::I64 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64)
            ),
            RuntimePureInputType::I128 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I128)
            ),
            RuntimePureInputType::ISize => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::ISize)
            ),
            RuntimePureInputType::U8 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U8)
            ),
            RuntimePureInputType::U16 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U16)
            ),
            RuntimePureInputType::U32 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U32)
            ),
            RuntimePureInputType::U64 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U64)
            ),
            RuntimePureInputType::U128 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U128)
            ),
            RuntimePureInputType::USize => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::USize)
            ),
            RuntimePureInputType::F32 => matches!(projection, RuntimePlanTypeProjection::F32),
            RuntimePureInputType::F64 => matches!(projection, RuntimePlanTypeProjection::F64),
            RuntimePureInputType::Value => true,
        })
    }

    fn scalar_output_abi_matches(
        &self,
        ty: RuntimePlanTypeId,
        abi: RuntimePureOutputType,
    ) -> Result<bool, RuntimePlanBuildError> {
        if abi == RuntimePureOutputType::Value {
            return Ok(true);
        }
        let projection = self.projection(ty)?;
        Ok(match abi {
            RuntimePureOutputType::Bool => matches!(projection, RuntimePlanTypeProjection::Bool),
            RuntimePureOutputType::I8 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I8)
            ),
            RuntimePureOutputType::I16 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I16)
            ),
            RuntimePureOutputType::I32 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I32)
            ),
            RuntimePureOutputType::I64 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64)
            ),
            RuntimePureOutputType::I128 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I128)
            ),
            RuntimePureOutputType::ISize => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::ISize)
            ),
            RuntimePureOutputType::U8 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U8)
            ),
            RuntimePureOutputType::U16 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U16)
            ),
            RuntimePureOutputType::U32 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U32)
            ),
            RuntimePureOutputType::U64 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U64)
            ),
            RuntimePureOutputType::U128 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U128)
            ),
            RuntimePureOutputType::USize => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::USize)
            ),
            RuntimePureOutputType::F32 => matches!(projection, RuntimePlanTypeProjection::F32),
            RuntimePureOutputType::F64 => matches!(projection, RuntimePlanTypeProjection::F64),
            RuntimePureOutputType::Value => true,
        })
    }

    fn require_numeric(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        if matches!(
            self.projection(ty)?,
            RuntimePlanTypeProjection::Signed(_)
                | RuntimePlanTypeProjection::Unsigned(_)
                | RuntimePlanTypeProjection::F32
                | RuntimePlanTypeProjection::F64
        ) {
            Ok(())
        } else {
            invalid_projection(context, ty)
        }
    }

    fn require_comparable(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        if matches!(
            self.projection(ty)?,
            RuntimePlanTypeProjection::Signed(_)
                | RuntimePlanTypeProjection::Unsigned(_)
                | RuntimePlanTypeProjection::F32
                | RuntimePlanTypeProjection::F64
        ) {
            Ok(())
        } else {
            invalid_projection(context, ty)
        }
    }

    fn require_negatable(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        if matches!(
            self.projection(ty)?,
            RuntimePlanTypeProjection::Signed(_)
                | RuntimePlanTypeProjection::F32
                | RuntimePlanTypeProjection::F64
        ) {
            Ok(())
        } else {
            invalid_projection(context, ty)
        }
    }

    fn is_bool(&self, ty: RuntimePlanTypeId) -> Result<bool, RuntimePlanBuildError> {
        Ok(matches!(
            self.projection(ty)?,
            RuntimePlanTypeProjection::Bool
        ))
    }

    fn is_string(&self, ty: RuntimePlanTypeId) -> Result<bool, RuntimePlanBuildError> {
        Ok(matches!(
            self.projection(ty)?,
            RuntimePlanTypeProjection::String
        ))
    }

    fn is_u32(&self, ty: RuntimePlanTypeId) -> Result<bool, RuntimePlanBuildError> {
        Ok(matches!(
            self.projection(ty)?,
            RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U32)
        ))
    }

    fn agent_field_owner_matches(
        &self,
        ty: RuntimePlanTypeId,
        owner: RuntimeAgentFieldOwner,
    ) -> Result<bool, RuntimePlanBuildError> {
        Ok(match (owner, self.projection(ty)?) {
            (RuntimeAgentFieldOwner::Agent(owner), RuntimePlanTypeProjection::Agent(target)) => {
                target.operational_type() == owner
            }
            (RuntimeAgentFieldOwner::Reference, RuntimePlanTypeProjection::Reference(_)) => true,
            _ => false,
        })
    }

    fn agent_field_result_matches(
        &self,
        ty: RuntimePlanTypeId,
        result: RuntimeAgentFieldResult,
    ) -> Result<bool, RuntimePlanBuildError> {
        Ok(match (result, self.projection(ty)?) {
            (RuntimeAgentFieldResult::Bool, RuntimePlanTypeProjection::Bool)
            | (RuntimeAgentFieldResult::String, RuntimePlanTypeProjection::String)
            | (
                RuntimeAgentFieldResult::U32,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U32),
            )
            | (
                RuntimeAgentFieldResult::U64,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U64),
            ) => true,
            (
                RuntimeAgentFieldResult::Agent(expected),
                RuntimePlanTypeProjection::Agent(actual),
            ) => actual.operational_type() == expected,
            (
                RuntimeAgentFieldResult::VecAgent(expected),
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Vec,
                    item,
                },
            ) => matches!(
                self.projection(*item)?,
                RuntimePlanTypeProjection::Agent(actual) if actual.operational_type() == expected
            ),
            (
                RuntimeAgentFieldResult::AgentValueMap,
                RuntimePlanTypeProjection::Map { key, value },
            ) => matches!(
                (self.projection(*key)?, self.projection(*value)?),
                (
                    RuntimePlanTypeProjection::Agent(key),
                    RuntimePlanTypeProjection::Agent(value),
                ) if key.operational_type() == RuntimeAgentOperationalType::AgentValue
                    && value.operational_type() == RuntimeAgentOperationalType::AgentValue
            ),
            _ => false,
        })
    }
}

impl RuntimePlanBuilder {
    fn lower_agent_expression(
        &self,
        result_ty: RuntimePlanTypeId,
        seed: RuntimeAgentExprSeed,
    ) -> Result<RuntimeAgentExpr, RuntimePlanBuildError> {
        let (constructor, choice, operand_seeds) = match seed {
            RuntimeAgentExprSeed::ChoiceAction { choice } => (
                RuntimeAgentConstructor::ChoiceAction,
                Some(choice),
                Vec::new(),
            ),
            RuntimeAgentExprSeed::CaptureViewport => {
                (RuntimeAgentConstructor::CaptureViewport, None, Vec::new())
            }
            RuntimeAgentExprSeed::CaptureLayer { target } => {
                (RuntimeAgentConstructor::CaptureLayer, None, vec![*target])
            }
            RuntimeAgentExprSeed::CaptureObject { target } => {
                (RuntimeAgentConstructor::CaptureObject, None, vec![*target])
            }
            RuntimeAgentExprSeed::StatePath { path } => {
                (RuntimeAgentConstructor::StatePath, None, vec![*path])
            }
            RuntimeAgentExprSeed::ObservationPath { path } => {
                (RuntimeAgentConstructor::ObservationPath, None, vec![*path])
            }
            RuntimeAgentExprSeed::ProbeSignal { target } => {
                (RuntimeAgentConstructor::ProbeSignal, None, vec![*target])
            }
            RuntimeAgentExprSeed::ProbeMetric { target } => {
                (RuntimeAgentConstructor::ProbeMetric, None, vec![*target])
            }
            RuntimeAgentExprSeed::ProbeState { path } => {
                (RuntimeAgentConstructor::ProbeState, None, vec![*path])
            }
            RuntimeAgentExprSeed::ProbeObservation { path } => {
                (RuntimeAgentConstructor::ProbeObservation, None, vec![*path])
            }
            RuntimeAgentExprSeed::Diagnostics => {
                (RuntimeAgentConstructor::Diagnostics, None, Vec::new())
            }
            RuntimeAgentExprSeed::PredicateExists { probe } => {
                (RuntimeAgentConstructor::PredicateExists, None, vec![*probe])
            }
            RuntimeAgentExprSeed::PredicateActionEnabled { target } => (
                RuntimeAgentConstructor::PredicateActionEnabled,
                None,
                vec![*target],
            ),
            RuntimeAgentExprSeed::PredicateDiagnosticsHasError { diagnostics } => (
                RuntimeAgentConstructor::PredicateDiagnosticsHasError,
                None,
                vec![*diagnostics],
            ),
            RuntimeAgentExprSeed::PredicateAll { predicates } => (
                RuntimeAgentConstructor::PredicateAll,
                None,
                predicates.into_vec(),
            ),
            RuntimeAgentExprSeed::PredicateAny { predicates } => (
                RuntimeAgentConstructor::PredicateAny,
                None,
                predicates.into_vec(),
            ),
            RuntimeAgentExprSeed::PredicateNot { predicate } => (
                RuntimeAgentConstructor::PredicateNot,
                None,
                vec![*predicate],
            ),
            RuntimeAgentExprSeed::PredicateCompare { probe, op, value } => {
                let constructor = match op {
                    crate::value::RuntimeAgentCompareOp::Eq => RuntimeAgentConstructor::PredicateEq,
                    crate::value::RuntimeAgentCompareOp::NotEq => {
                        RuntimeAgentConstructor::PredicateNotEq
                    }
                    crate::value::RuntimeAgentCompareOp::Greater => {
                        RuntimeAgentConstructor::PredicateGreater
                    }
                    crate::value::RuntimeAgentCompareOp::GreaterOrEqual => {
                        RuntimeAgentConstructor::PredicateGreaterOrEqual
                    }
                    crate::value::RuntimeAgentCompareOp::Less => {
                        RuntimeAgentConstructor::PredicateLess
                    }
                    crate::value::RuntimeAgentCompareOp::LessOrEqual => {
                        RuntimeAgentConstructor::PredicateLessOrEqual
                    }
                };
                (constructor, None, vec![*probe, *value])
            }
            RuntimeAgentExprSeed::ViewportPoint { x, y } => {
                (RuntimeAgentConstructor::ViewportPoint, None, vec![*x, *y])
            }
        };
        let operands = operand_seeds
            .into_iter()
            .map(|operand| self.lower_expression(operand))
            .collect::<Result<Vec<_>, _>>()?;
        self.validate_agent_result(constructor, result_ty)?;
        self.validate_agent_operands(constructor, &operands)?;
        RuntimeAgentExpr::try_from_admitted_constructor(constructor, choice, operands)
            .map_err(|_| RuntimePlanBuildError::InvalidAgentExpression { constructor })
    }

    fn validate_agent_result(
        &self,
        constructor: RuntimeAgentConstructor,
        result_ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        let valid = match self.projection(result_ty)? {
            RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::Probe(_)) => {
                constructor.result_type() == RuntimeAgentOperationalType::Probe
            }
            RuntimePlanTypeProjection::Agent(agent) => {
                agent.operational_type() == constructor.result_type()
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(RuntimePlanBuildError::InvalidAgentResultType {
                constructor,
                actual: result_ty,
            })
        }
    }

    fn validate_agent_operands(
        &self,
        constructor: RuntimeAgentConstructor,
        operands: &[RuntimeExpr],
    ) -> Result<(), RuntimePlanBuildError> {
        use RuntimeAgentConstructor as Constructor;

        if !constructor.accepts_operand_count(operands.len()) {
            return Err(RuntimePlanBuildError::InvalidAgentExpression { constructor });
        }
        match constructor {
            Constructor::ChoiceAction | Constructor::CaptureViewport | Constructor::Diagnostics => {
            }
            Constructor::CaptureLayer
            | Constructor::CaptureObject
            | Constructor::ProbeSignal
            | Constructor::ProbeMetric => {
                self.require_agent_operand(constructor, 0, operands[0].ty(), |projection| {
                    matches!(
                        projection,
                        RuntimePlanTypeProjection::String
                            | RuntimePlanTypeProjection::EntityReference
                    )
                })?;
            }
            Constructor::StatePath | Constructor::ObservationPath => {
                self.require_agent_operand(constructor, 0, operands[0].ty(), |projection| {
                    matches!(projection, RuntimePlanTypeProjection::String)
                })?;
            }
            Constructor::ProbeState => {
                self.require_agent_type(
                    constructor,
                    0,
                    operands[0].ty(),
                    RuntimeAgentOperationalType::DebugStatePath,
                )?;
            }
            Constructor::ProbeObservation => {
                self.require_agent_type(
                    constructor,
                    0,
                    operands[0].ty(),
                    RuntimeAgentOperationalType::ObservationFieldPath,
                )?;
            }
            Constructor::PredicateExists => self.require_first_agent_type(
                constructor,
                operands,
                RuntimeAgentOperationalType::Probe,
            )?,
            Constructor::PredicateActionEnabled => self.require_first_agent_type(
                constructor,
                operands,
                RuntimeAgentOperationalType::ActionTarget,
            )?,
            Constructor::PredicateDiagnosticsHasError => self.require_first_agent_type(
                constructor,
                operands,
                RuntimeAgentOperationalType::Diagnostics,
            )?,
            Constructor::PredicateNot => self.require_first_agent_type(
                constructor,
                operands,
                RuntimeAgentOperationalType::Predicate,
            )?,
            Constructor::PredicateAll | Constructor::PredicateAny => {
                for (index, operand) in operands.iter().enumerate() {
                    self.require_agent_type(
                        constructor,
                        index,
                        operand.ty(),
                        RuntimeAgentOperationalType::Predicate,
                    )?;
                }
            }
            Constructor::PredicateEq
            | Constructor::PredicateNotEq
            | Constructor::PredicateGreater
            | Constructor::PredicateGreaterOrEqual
            | Constructor::PredicateLess
            | Constructor::PredicateLessOrEqual => {
                self.validate_agent_comparison_operands(constructor, operands)?;
            }
            Constructor::ViewportPoint => {
                for (index, operand) in operands.iter().enumerate() {
                    if !self.is_u32(operand.ty())? {
                        return Err(RuntimePlanBuildError::InvalidAgentOperandType {
                            constructor,
                            operand: index,
                            actual: operand.ty(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_agent_comparison_operands(
        &self,
        constructor: RuntimeAgentConstructor,
        operands: &[RuntimeExpr],
    ) -> Result<(), RuntimePlanBuildError> {
        let probe_item = match self.projection(operands[0].ty())? {
            RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::Probe(item)) => *item,
            _ => {
                return Err(RuntimePlanBuildError::InvalidAgentOperandType {
                    constructor,
                    operand: 0,
                    actual: operands[0].ty(),
                });
            }
        };
        require_same("Agent comparison value", probe_item, operands[1].ty())
    }

    fn require_first_agent_type(
        &self,
        constructor: RuntimeAgentConstructor,
        operands: &[RuntimeExpr],
        expected: RuntimeAgentOperationalType,
    ) -> Result<(), RuntimePlanBuildError> {
        self.require_agent_type(constructor, 0, operands[0].ty(), expected)
    }

    fn require_agent_operand(
        &self,
        constructor: RuntimeAgentConstructor,
        operand: usize,
        actual: RuntimePlanTypeId,
        accepts: impl FnOnce(&RuntimePlanTypeProjection<RuntimePlanTypeId>) -> bool,
    ) -> Result<(), RuntimePlanBuildError> {
        if accepts(self.projection(actual)?) {
            Ok(())
        } else {
            Err(RuntimePlanBuildError::InvalidAgentOperandType {
                constructor,
                operand,
                actual,
            })
        }
    }

    fn require_agent_type(
        &self,
        constructor: RuntimeAgentConstructor,
        operand: usize,
        actual: RuntimePlanTypeId,
        expected: RuntimeAgentOperationalType,
    ) -> Result<(), RuntimePlanBuildError> {
        self.require_agent_operand(constructor, operand, actual, |projection| {
            matches!(projection, RuntimePlanTypeProjection::Agent(agent) if agent.operational_type() == expected)
        })
    }
}

impl RuntimePlanBuilder {
    #[allow(
        clippy::too_many_lines,
        reason = "the match is the exhaustive pattern-admission authority"
    )]
    fn lower_pattern(
        &self,
        seed: RuntimePatternSeed,
        admission: &mut PatternAdmission,
        path: &mut Vec<RuntimePatternBindingStep>,
    ) -> Result<RuntimePattern, RuntimePlanBuildError> {
        let (semantic_ty, kind) = seed.into_parts();
        let ty = self.resolve_seed_type("pattern", semantic_ty)?;
        let kind = match kind {
            RuntimePatternSeedKind::Bind { mutable, local } => RuntimePatternKind::Bind {
                mutable,
                binding: self.lower_pattern_binding(&local, ty, admission, path)?,
            },
            RuntimePatternSeedKind::Discard => RuntimePatternKind::Discard,
            RuntimePatternSeedKind::Literal(value) => {
                self.validate_plan_value("pattern literal", ty, &value)?;
                RuntimePatternKind::Literal(value)
            }
            RuntimePatternSeedKind::Entity(entity) => {
                self.require_projection("entity-reference pattern", ty, |projection| {
                    matches!(projection, RuntimePlanTypeProjection::EntityReference)
                })?;
                RuntimePatternKind::Entity(entity)
            }
            RuntimePatternSeedKind::Tuple(items) => {
                let expected = match self.projection(ty)? {
                    RuntimePlanTypeProjection::Tuple(items) => items.as_ref(),
                    _ => return invalid_projection("tuple pattern", ty),
                };
                if expected.len() != items.len() {
                    return invalid_projection("tuple pattern arity", ty);
                }
                let mut lowered = Vec::with_capacity(items.len());
                for (index, (item, expected)) in
                    items.into_vec().into_iter().zip(expected).enumerate()
                {
                    let step = u32::try_from(index).map_err(|_| {
                        RuntimePlanBuildError::InvalidTypeProjection {
                            context: "tuple pattern ordinal",
                            ty,
                        }
                    })?;
                    path.push(RuntimePatternBindingStep::TupleElement(step));
                    let item = self.lower_pattern(item, admission, path);
                    path.pop();
                    let item = item?;
                    require_same("tuple pattern element", *expected, item.ty())?;
                    lowered.push(item);
                }
                RuntimePatternKind::Tuple(lowered.into_boxed_slice())
            }
            RuntimePatternSeedKind::Record { fields, rest } => {
                let domain = self
                    .nominal_record_domains
                    .get(ty)
                    .ok_or(RuntimePlanBuildError::UnknownNominalRecordDomain { owner: ty })?;
                let field_count = domain.fields().len();
                let exact = matches!(&rest, RuntimePatternRestSeed::Exact);
                let mut admitted_fields = BTreeSet::new();
                let mut lowered = Vec::with_capacity(fields.len());
                for (pattern_ordinal, field) in fields.into_vec().into_iter().enumerate() {
                    let (field, pattern) = field.into_parts();
                    let (field, field_ty) = self.resolve_record_field(ty, field)?;
                    if !admitted_fields.insert(field) {
                        return Err(RuntimePlanBuildError::DuplicateRecordField {
                            owner: ty,
                            field,
                        });
                    }
                    let step = u32::try_from(pattern_ordinal).map_err(|_| {
                        RuntimePlanBuildError::InvalidTypeProjection {
                            context: "record pattern ordinal",
                            ty,
                        }
                    })?;
                    path.push(RuntimePatternBindingStep::RecordField(step));
                    let pattern = self.lower_pattern(pattern, admission, path);
                    path.pop();
                    let pattern = pattern?;
                    require_same("record pattern field", field_ty, pattern.ty())?;
                    lowered.push(RuntimeRecordPatternField::from_admitted_parts(
                        field, pattern,
                    ));
                }
                if exact && admitted_fields.len() != field_count {
                    for ordinal in 0..field_count {
                        let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)?;
                        if !admitted_fields.contains(&field) {
                            return Err(RuntimePlanBuildError::MissingRecordField {
                                owner: ty,
                                field,
                            });
                        }
                    }
                }
                let rest = self.lower_pattern_rest(
                    rest,
                    ty,
                    RuntimePatternBindingStep::RecordRest,
                    admission,
                    path,
                )?;
                RuntimePatternKind::Record {
                    fields: lowered.into_boxed_slice(),
                    rest,
                }
            }
            RuntimePatternSeedKind::Sequence { items, rest } => {
                let (item_ty, fixed_len) = self.sequence_projection(ty, "sequence pattern")?;
                if fixed_len.is_some() && matches!(&rest, RuntimePatternRestSeed::Bind(_)) {
                    return invalid_projection("array rest-binding pattern", ty);
                }
                if let Some(expected) = fixed_len {
                    let actual = items.len();
                    if matches!(&rest, RuntimePatternRestSeed::Exact) {
                        validate_sequence_length(expected, actual)?;
                    } else if u64::try_from(actual).map_or(true, |actual| actual > expected) {
                        return Err(RuntimePlanBuildError::SequenceLengthMismatch {
                            expected,
                            actual,
                        });
                    }
                }
                let mut lowered = Vec::with_capacity(items.len());
                for (index, item) in items.into_vec().into_iter().enumerate() {
                    let step = u32::try_from(index).map_err(|_| {
                        RuntimePlanBuildError::InvalidTypeProjection {
                            context: "sequence pattern ordinal",
                            ty,
                        }
                    })?;
                    path.push(RuntimePatternBindingStep::SequenceElement(step));
                    let item = self.lower_pattern(item, admission, path);
                    path.pop();
                    let item = item?;
                    require_same("sequence pattern element", item_ty, item.ty())?;
                    lowered.push(item);
                }
                let rest = self.lower_pattern_rest(
                    rest,
                    ty,
                    RuntimePatternBindingStep::SequenceRest,
                    admission,
                    path,
                )?;
                RuntimePatternKind::Sequence {
                    items: lowered.into_boxed_slice(),
                    rest,
                }
            }
            RuntimePatternSeedKind::Variant { ordinal, payload } => {
                let expected = self.variant_payload_type(ty, ordinal)?;
                let payload = if let Some(payload) = payload {
                    path.push(RuntimePatternBindingStep::VariantPayload);
                    let payload = self.lower_pattern(*payload, admission, path);
                    path.pop();
                    Some(payload?)
                } else {
                    None
                };
                let actual = payload.as_ref().map(RuntimePattern::ty);
                if expected != actual {
                    return Err(RuntimePlanBuildError::VariantPayloadMismatch {
                        owner: ty,
                        ordinal,
                        expected,
                        actual,
                    });
                }
                RuntimePatternKind::Variant {
                    ordinal,
                    payload: payload.map(Box::new),
                }
            }
            RuntimePatternSeedKind::Whole { local, pattern } => {
                let binding = self.lower_pattern_binding(&local, ty, admission, path)?;
                let pattern = self.lower_pattern(*pattern, admission, path)?;
                require_same("whole pattern child", ty, pattern.ty())?;
                RuntimePatternKind::Whole {
                    binding,
                    pattern: Box::new(pattern),
                }
            }
            RuntimePatternSeedKind::Typed { local } => RuntimePatternKind::Typed {
                binding: self.lower_pattern_binding(&local, ty, admission, path)?,
            },
        };
        Ok(RuntimePattern::from_admitted_parts(ty, kind))
    }

    fn lower_pattern_rest(
        &self,
        rest: RuntimePatternRestSeed,
        whole_ty: RuntimePlanTypeId,
        step: RuntimePatternBindingStep,
        admission: &mut PatternAdmission,
        path: &mut Vec<RuntimePatternBindingStep>,
    ) -> Result<RuntimePatternRest, RuntimePlanBuildError> {
        match rest {
            RuntimePatternRestSeed::Exact => Ok(RuntimePatternRest::Exact),
            RuntimePatternRestSeed::Ignore => Ok(RuntimePatternRest::Ignore),
            RuntimePatternRestSeed::Bind(local) => {
                path.push(step);
                let binding = self.lower_pattern_binding(&local, whole_ty, admission, path);
                path.pop();
                Ok(RuntimePatternRest::Bind(binding?))
            }
        }
    }

    fn lower_pattern_binding(
        &self,
        seed: &RuntimeLocalSeedId,
        expected_ty: RuntimePlanTypeId,
        admission: &mut PatternAdmission,
        path: &[RuntimePatternBindingStep],
    ) -> Result<RuntimePatternBindingCoordinate, RuntimePlanBuildError> {
        let (local, actual_ty) = self.resolve_local(seed)?;
        require_same("pattern binding local", expected_ty, actual_ty)?;
        if !admission.bindings.insert(local) {
            return Err(RuntimePlanBuildError::DuplicatePatternBinding { local });
        }
        let path = if path.is_empty() {
            RuntimePatternBindingPath::whole()
        } else {
            RuntimePatternBindingPath::try_from_steps(path.iter().copied())?
        };
        Ok(RuntimePatternBindingCoordinate::from_admitted_parts(
            local, path,
        ))
    }
}

impl RuntimePlanBuilder {
    pub(super) fn validate_function_body_locals(
        &self,
        body: &RuntimeExpr,
        params: &[RuntimeLocalDeclarationId],
        captures: &[RuntimeLocalDeclarationId],
    ) -> Result<(), RuntimePlanBuildError> {
        let scope = params
            .iter()
            .chain(captures)
            .copied()
            .collect::<BTreeSet<_>>();
        let mut used = BTreeSet::new();
        self.validate_expression_locals(body, &scope, &mut used)?;
        for capture in captures {
            if !used.contains(capture) {
                return Err(RuntimePlanBuildError::UnusedFunctionCapture { local: *capture });
            }
        }
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "lexical validation exhaustively traverses the closed expression algebra"
    )]
    fn validate_expression_locals(
        &self,
        expr: &RuntimeExpr,
        scope: &BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        match expr.kind() {
            RuntimeExprKind::Value(_) | RuntimeExprKind::EntityRef(_) => Ok(()),
            RuntimeExprKind::Agent(agent) => {
                for operand in agent.operands() {
                    self.validate_expression_locals(operand, scope, used)?;
                }
                Ok(())
            }
            RuntimeExprKind::Local(local) => {
                require_local_in_scope(*local, scope)?;
                used.insert(*local);
                Ok(())
            }
            RuntimeExprKind::Let {
                binding,
                expr,
                body,
            } => {
                self.validate_expression_locals(expr, scope, used)?;
                let nested = extend_scope(scope, [*binding])?;
                self.validate_expression_locals(body, &nested, used)
            }
            RuntimeExprKind::Tuple(items) | RuntimeExprKind::BracketSeq(items) => {
                self.validate_expression_slice_locals(items, scope, used)
            }
            RuntimeExprKind::RepeatSeq { value, .. }
            | RuntimeExprKind::Field { target: value, .. }
            | RuntimeExprKind::ProjectTuple { target: value, .. }
            | RuntimeExprKind::ProjectRecord { target: value, .. }
            | RuntimeExprKind::Sum { source: value }
            | RuntimeExprKind::Unary { expr: value, .. }
            | RuntimeExprKind::ReductionUnchanged { state: value } => {
                self.validate_expression_locals(value, scope, used)
            }
            RuntimeExprKind::Range { start, end, .. } => {
                for bound in start.iter().chain(end.iter()) {
                    self.validate_expression_locals(bound, scope, used)?;
                }
                Ok(())
            }
            RuntimeExprKind::NominalRecord(record) => {
                for initializer in record.initializers() {
                    self.validate_expression_locals(initializer.value(), scope, used)?;
                }
                Ok(())
            }
            RuntimeExprKind::Variant { payload, .. } => {
                if let Some(payload) = payload {
                    self.validate_expression_locals(payload, scope, used)?;
                }
                Ok(())
            }
            RuntimeExprKind::AssignNominalField {
                base, expr, body, ..
            } => {
                require_local_in_scope(*base, scope)?;
                used.insert(*base);
                self.validate_expression_locals(expr, scope, used)?;
                self.validate_expression_locals(body, scope, used)
            }
            RuntimeExprKind::Call { args, .. } | RuntimeExprKind::PureCall { args, .. } => {
                self.validate_argument_locals(args, scope, used)
            }
            RuntimeExprKind::Function(site) => {
                let index = usize::try_from(site.get().get() - 1)
                    .map_err(|_| RuntimePlanBuildError::UnknownFunctionSite { site: *site })?;
                let site = self
                    .function_sites
                    .get(index)
                    .ok_or(RuntimePlanBuildError::UnknownFunctionSite { site: *site })?;
                for capture in &site.captures {
                    require_local_in_scope(*capture, scope)?;
                    used.insert(*capture);
                }
                Ok(())
            }
            RuntimeExprKind::Apply { callee, args } => {
                self.validate_expression_locals(callee, scope, used)?;
                self.validate_argument_locals(args, scope, used)
            }
            RuntimeExprKind::TraitCall { receiver, args, .. } => {
                self.validate_expression_locals(receiver, scope, used)?;
                self.validate_argument_locals(args, scope, used)
            }
            RuntimeExprKind::Map {
                source,
                param,
                body,
            }
            | RuntimeExprKind::Filter {
                source,
                param,
                body,
            } => {
                self.validate_expression_locals(source, scope, used)?;
                let nested = extend_scope(scope, [*param])?;
                self.validate_expression_locals(body, &nested, used)
            }
            RuntimeExprKind::Binary { lhs, rhs, .. } => {
                self.validate_expression_locals(lhs, scope, used)?;
                self.validate_expression_locals(rhs, scope, used)
            }
            RuntimeExprKind::If {
                condition,
                then_expr,
                else_expr,
            } => {
                self.validate_expression_locals(condition, scope, used)?;
                self.validate_expression_locals(then_expr, scope, used)?;
                self.validate_expression_locals(else_expr, scope, used)
            }
            RuntimeExprKind::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => {
                self.validate_expression_locals(expr, scope, used)?;
                let nested = extend_scope(scope, pattern_binding_locals(pattern))?;
                if let Some(guard) = guard {
                    self.validate_expression_locals(guard, &nested, used)?;
                }
                self.validate_expression_locals(then_expr, &nested, used)?;
                self.validate_expression_locals(else_expr, scope, used)
            }
            RuntimeExprKind::Match { scrutinee, arms } => {
                self.validate_expression_locals(scrutinee, scope, used)?;
                for arm in arms {
                    let nested = extend_scope(scope, pattern_binding_locals(arm.pattern()))?;
                    if let Some(guard) = arm.guard() {
                        self.validate_expression_locals(guard, &nested, used)?;
                    }
                    self.validate_expression_locals(arm.value(), &nested, used)?;
                }
                Ok(())
            }
        }
    }

    fn validate_expression_slice_locals(
        &self,
        expressions: &[RuntimeExpr],
        scope: &BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        for expression in expressions {
            self.validate_expression_locals(expression, scope, used)?;
        }
        Ok(())
    }

    fn validate_argument_locals(
        &self,
        arguments: &[RuntimeCallArgument],
        scope: &BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        for argument in arguments {
            self.validate_expression_locals(argument.value(), scope, used)?;
        }
        Ok(())
    }

    pub(super) fn lower_stream_plan_seed(
        &self,
        seed: RuntimeStreamPlanSeed,
    ) -> Result<StreamPlan, RuntimePlanBuildError> {
        let item_ty = self.resolve_seed_type("stream item type", seed.item_ty)?;
        let error_ty = self.resolve_seed_type("stream error type", seed.error_ty)?;
        let ops = self.lower_stream_ops(seed.ops, item_ty)?;
        let mut scope = BTreeSet::new();
        self.validate_stream_ops_locals(&ops, &mut scope)?;
        Ok(StreamPlan::from_admitted_parts(
            seed.id, item_ty, error_ty, ops,
        ))
    }

    fn lower_stream_ops(
        &self,
        seeds: Vec<RuntimeStreamOpSeed>,
        item_ty: RuntimePlanTypeId,
    ) -> Result<Vec<StreamOp>, RuntimePlanBuildError> {
        seeds
            .into_iter()
            .map(|seed| self.lower_stream_op(seed, item_ty))
            .collect()
    }

    fn lower_stream_op(
        &self,
        seed: RuntimeStreamOpSeed,
        item_ty: RuntimePlanTypeId,
    ) -> Result<StreamOp, RuntimePlanBuildError> {
        Ok(match seed {
            RuntimeStreamOpSeed::Let { pattern, expr } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let expr = self.lower_expression(expr)?;
                require_same("stream let pattern", pattern.ty(), expr.ty())?;
                StreamOp::Let { pattern, expr }
            }
            RuntimeStreamOpSeed::ForNext {
                pattern,
                source,
                body,
            } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let source = self.lower_expression(source)?;
                let (source_item, _) =
                    self.stream_projection(source.ty(), "stream for-next source expression")?;
                require_same("stream for-next pattern", source_item, pattern.ty())?;
                StreamOp::ForNext {
                    pattern,
                    source,
                    body: self.lower_stream_ops(body, item_ty)?,
                }
            }
            RuntimeStreamOpSeed::Yield { expr } => {
                let expr = self.lower_expression(expr)?;
                require_same("stream yield expression", item_ty, expr.ty())?;
                StreamOp::Yield { expr }
            }
            RuntimeStreamOpSeed::If {
                condition,
                then_ops,
                else_ops,
            } => {
                let condition = self.lower_expression(condition)?;
                self.require_bool("stream if condition", condition.ty())?;
                StreamOp::If {
                    condition,
                    then_ops: self.lower_stream_ops(then_ops, item_ty)?,
                    else_ops: self.lower_stream_ops(else_ops, item_ty)?,
                }
            }
            RuntimeStreamOpSeed::Match { scrutinee, arms } => {
                let scrutinee = self.lower_expression(scrutinee)?;
                StreamOp::Match {
                    scrutinee: scrutinee.clone(),
                    arms: arms
                        .into_iter()
                        .map(|arm| self.lower_stream_match_arm(arm, scrutinee.ty(), item_ty))
                        .collect::<Result<_, _>>()?,
                }
            }
            RuntimeStreamOpSeed::Close { source } => {
                let source = self.lower_expression(source)?;
                self.stream_projection(source.ty(), "stream close source expression")?;
                StreamOp::Close { source }
            }
            RuntimeStreamOpSeed::Return => StreamOp::Return,
        })
    }

    fn lower_stream_match_arm(
        &self,
        arm: RuntimeStreamMatchArmSeed,
        scrutinee_ty: RuntimePlanTypeId,
        item_ty: RuntimePlanTypeId,
    ) -> Result<StreamMatchArm, RuntimePlanBuildError> {
        let pattern = self.lower_pattern_seed(arm.pattern)?;
        require_same("stream match pattern", scrutinee_ty, pattern.ty())?;
        let guard = arm
            .guard
            .map(|guard| self.lower_expression(guard))
            .transpose()?;
        if let Some(guard) = &guard {
            self.require_bool("stream match guard", guard.ty())?;
        }
        Ok(StreamMatchArm {
            pattern,
            guard,
            ops: self.lower_stream_ops(arm.ops, item_ty)?,
        })
    }

    fn validate_stream_ops_locals(
        &self,
        ops: &[StreamOp],
        scope: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        let mut used = BTreeSet::new();
        self.validate_stream_ops_locals_inner(ops, scope, &mut used)
    }

    fn validate_stream_ops_locals_inner(
        &self,
        ops: &[StreamOp],
        scope: &mut BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        for op in ops {
            match op {
                StreamOp::Let { pattern, expr } => {
                    self.validate_expression_locals(expr, scope, used)?;
                    *scope = extend_scope(scope, pattern_binding_locals(pattern))?;
                }
                StreamOp::ForNext {
                    pattern,
                    source,
                    body,
                } => {
                    self.validate_expression_locals(source, scope, used)?;
                    let mut nested = extend_scope(scope, pattern_binding_locals(pattern))?;
                    self.validate_stream_ops_locals_inner(body, &mut nested, used)?;
                }
                StreamOp::Yield { expr } | StreamOp::Close { source: expr } => {
                    self.validate_expression_locals(expr, scope, used)?;
                }
                StreamOp::If {
                    condition,
                    then_ops,
                    else_ops,
                } => {
                    self.validate_expression_locals(condition, scope, used)?;
                    let mut then_scope = scope.clone();
                    self.validate_stream_ops_locals_inner(then_ops, &mut then_scope, used)?;
                    let mut else_scope = scope.clone();
                    self.validate_stream_ops_locals_inner(else_ops, &mut else_scope, used)?;
                }
                StreamOp::Match { scrutinee, arms } => {
                    self.validate_expression_locals(scrutinee, scope, used)?;
                    for arm in arms {
                        let mut arm_scope =
                            extend_scope(scope, pattern_binding_locals(&arm.pattern))?;
                        if let Some(guard) = &arm.guard {
                            self.validate_expression_locals(guard, &arm_scope, used)?;
                        }
                        self.validate_stream_ops_locals_inner(&arm.ops, &mut arm_scope, used)?;
                    }
                }
                StreamOp::Return => {}
            }
        }
        Ok(())
    }

    fn stream_projection(
        &self,
        ty: RuntimePlanTypeId,
        context: &'static str,
    ) -> Result<(RuntimePlanTypeId, RuntimePlanTypeId), RuntimePlanBuildError> {
        match self.projection(ty)? {
            RuntimePlanTypeProjection::Stream { item, error } => Ok((*item, *error)),
            _ => invalid_projection(context, ty),
        }
    }
}

impl RuntimePlanBuilder {
    pub(super) fn lower_flow_ops(
        &self,
        seeds: Vec<RuntimeFlowOpSeed>,
    ) -> Result<Vec<FlowOp>, RuntimePlanBuildError> {
        seeds
            .into_iter()
            .map(|seed| self.lower_flow_op(seed))
            .collect()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the match is the exhaustive canonical flow admission authority"
    )]
    fn lower_flow_op(&self, seed: RuntimeFlowOpSeed) -> Result<FlowOp, RuntimePlanBuildError> {
        Ok(match seed {
            RuntimeFlowOpSeed::Let { pattern, expr } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let expr = self.lower_expression(expr)?;
                require_same("flow let pattern", pattern.ty(), expr.ty())?;
                FlowOp::Let { pattern, expr }
            }
            RuntimeFlowOpSeed::LetElse {
                pattern,
                expr,
                else_ops,
            } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let expr = self.lower_expression(expr)?;
                require_same("flow let-else pattern", pattern.ty(), expr.ty())?;
                FlowOp::LetElse {
                    pattern,
                    expr,
                    else_ops: self.lower_flow_ops(else_ops)?,
                }
            }
            RuntimeFlowOpSeed::AssignNominalField {
                base,
                owner,
                field,
                value,
            } => {
                let (base, base_ty) = base
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignLocalSeed)?;
                let owner = self.resolve_seed_type("flow assignment owner", owner)?;
                require_same("flow assignment base", owner, base_ty)?;
                let (field, field_ty) = self.resolve_record_field(owner, field)?;
                let value = self.lower_expression(value)?;
                require_same("flow assignment value", field_ty, value.ty())?;
                FlowOp::AssignNominalField { base, field, value }
            }
            RuntimeFlowOpSeed::Dialogue { content } => {
                let content = content
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignDialogueContentSeed)?;
                if self.dialogue_content.get(content).is_none() {
                    return Err(RuntimePlanBuildError::ForeignDialogueContentSeed);
                }
                FlowOp::Dialogue { content }
            }
            RuntimeFlowOpSeed::Choice { id, options } => FlowOp::Choice {
                id,
                options: options
                    .into_iter()
                    .map(|option| self.lower_choice_option(option))
                    .collect::<Result<_, _>>()?,
            },
            RuntimeFlowOpSeed::Await {
                binding,
                target,
                observers,
            } => FlowOp::Await {
                binding: binding
                    .map(|binding| self.lower_pattern_seed(binding))
                    .transpose()?,
                target: AwaitTarget {
                    need: target.need,
                    task: target.task,
                    outcome: target.outcome,
                    request: self.lower_host_task_request(target.request)?,
                },
                observers: observers
                    .into_iter()
                    .map(|observer| {
                        Ok(crate::plan::RuntimeAwaitPendingObserver {
                            pattern: self.lower_pattern_seed(observer.pattern)?,
                            ops: observer
                                .ops
                                .into_iter()
                                .map(|op| self.lower_flow_op(op))
                                .collect::<Result<_, _>>()?,
                        })
                    })
                    .collect::<Result<_, RuntimePlanBuildError>>()?,
            },
            RuntimeFlowOpSeed::AwaitMany {
                binding,
                target,
                pending,
            } => {
                if target.limit == 0 {
                    return Err(RuntimePlanBuildError::ZeroAwaitManyLimit);
                }
                let source = self.lower_expression(target.source)?;
                let item_ty = self.await_many_item_type(source.ty())?;
                let (item_binding, binding_ty) = target
                    .item_binding
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignLocalSeed)?;
                require_same("AwaitMany item binding", item_ty, binding_ty)?;
                FlowOp::AwaitMany {
                    binding: binding
                        .map(|binding| self.lower_pattern_seed(binding))
                        .transpose()?,
                    target: AwaitManyTarget {
                        need: target.need,
                        task: target.task,
                        outcome: target.outcome,
                        source,
                        item_binding,
                        limit: target.limit,
                        request: self.lower_host_task_request(target.request)?,
                    },
                    pending: self.lower_line_effects(pending)?,
                }
            }
            RuntimeFlowOpSeed::HostCall { binding, target } => FlowOp::HostCall {
                binding: binding
                    .map(|binding| self.lower_pattern_seed(binding))
                    .transpose()?,
                target: self.lower_host_call_target(target)?,
            },
            RuntimeFlowOpSeed::If {
                condition,
                then_ops,
                else_ops,
            } => {
                let condition = self.lower_expression(condition)?;
                self.require_bool("flow if condition", condition.ty())?;
                FlowOp::If {
                    condition,
                    then_ops: self.lower_flow_ops(then_ops)?,
                    else_ops: self.lower_flow_ops(else_ops)?,
                }
            }
            RuntimeFlowOpSeed::IfLet {
                pattern,
                expr,
                guard,
                then_ops,
                else_ops,
            } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let expr = self.lower_expression(expr)?;
                require_same("flow if-let pattern", pattern.ty(), expr.ty())?;
                let guard = guard
                    .map(|guard| self.lower_expression(guard))
                    .transpose()?;
                if let Some(guard) = &guard {
                    self.require_bool("flow if-let guard", guard.ty())?;
                }
                FlowOp::IfLet {
                    pattern,
                    expr,
                    guard,
                    then_ops: self.lower_flow_ops(then_ops)?,
                    else_ops: self.lower_flow_ops(else_ops)?,
                }
            }
            RuntimeFlowOpSeed::Match { scrutinee, arms } => {
                let scrutinee = self.lower_expression(scrutinee)?;
                let arms = arms
                    .into_iter()
                    .map(|arm| self.lower_flow_match_arm(arm, scrutinee.ty()))
                    .collect::<Result<_, _>>()?;
                FlowOp::Match { scrutinee, arms }
            }
            RuntimeFlowOpSeed::Loop { result, body } => FlowOp::Loop {
                result: result
                    .map(|result| self.lower_pattern_seed(result))
                    .transpose()?,
                body: self.lower_flow_ops(body)?,
            },
            RuntimeFlowOpSeed::While { condition, body } => {
                let condition = self.lower_expression(condition)?;
                self.require_bool("flow while condition", condition.ty())?;
                FlowOp::While {
                    condition,
                    body: self.lower_flow_ops(body)?,
                }
            }
            RuntimeFlowOpSeed::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let expr = self.lower_expression(expr)?;
                require_same("flow while-let pattern", pattern.ty(), expr.ty())?;
                let guard = guard
                    .map(|guard| self.lower_expression(guard))
                    .transpose()?;
                if let Some(guard) = &guard {
                    self.require_bool("flow while-let guard", guard.ty())?;
                }
                FlowOp::WhileLet {
                    pattern,
                    expr,
                    guard,
                    body: self.lower_flow_ops(body)?,
                }
            }
            RuntimeFlowOpSeed::For {
                pattern,
                source,
                evidence,
                body,
            } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let source = self.lower_expression(source)?;
                let (item_ty, evidence) = self.lower_iterator_evidence(source.ty(), evidence)?;
                require_same("flow for pattern", item_ty, pattern.ty())?;
                FlowOp::For {
                    pattern,
                    source,
                    evidence,
                    body: self.lower_flow_ops(body)?,
                }
            }
            RuntimeFlowOpSeed::Thread { name, body } => FlowOp::Thread {
                name,
                body: self.lower_flow_ops(body)?,
            },
            RuntimeFlowOpSeed::Scope(ops) => FlowOp::Scope(self.lower_flow_ops(ops)?),
            RuntimeFlowOpSeed::Break(value) => FlowOp::Break(
                value
                    .map(|value| self.lower_expression(value))
                    .transpose()?,
            ),
            RuntimeFlowOpSeed::Continue => FlowOp::Continue,
            RuntimeFlowOpSeed::Goto(target) => FlowOp::Goto(target),
            RuntimeFlowOpSeed::GotoExpr(target) => FlowOp::GotoExpr(self.lower_expression(target)?),
            RuntimeFlowOpSeed::Return(value) => FlowOp::Return(value),
            RuntimeFlowOpSeed::ReturnExpr(value) => {
                FlowOp::ReturnExpr(self.lower_expression(value)?)
            }
            RuntimeFlowOpSeed::Effect(effect) => FlowOp::Effect(self.lower_line_effect(effect)?),
            RuntimeFlowOpSeed::EvaluatedEffect(effect) => {
                FlowOp::EvaluatedEffect(self.lower_evaluated_effect(effect)?)
            }
            RuntimeFlowOpSeed::RegisterCleanup { key, effect } => FlowOp::RegisterCleanup {
                key,
                effect: self.lower_line_effect(effect)?,
            },
            RuntimeFlowOpSeed::CancelCleanup { key } => FlowOp::CancelCleanup { key },
            RuntimeFlowOpSeed::EnterScope => FlowOp::EnterScope,
            RuntimeFlowOpSeed::ExitScope => FlowOp::ExitScope,
            RuntimeFlowOpSeed::Noop => FlowOp::Noop,
        })
    }

    fn lower_flow_match_arm(
        &self,
        arm: RuntimeFlowMatchArmSeed,
        scrutinee_ty: RuntimePlanTypeId,
    ) -> Result<RuntimeMatchArm, RuntimePlanBuildError> {
        let pattern = self.lower_pattern_seed(arm.pattern)?;
        require_same("flow match pattern", scrutinee_ty, pattern.ty())?;
        let guard = arm
            .guard
            .map(|guard| self.lower_expression(guard))
            .transpose()?;
        if let Some(guard) = &guard {
            self.require_bool("flow match guard", guard.ty())?;
        }
        Ok(RuntimeMatchArm {
            pattern,
            guard,
            ops: self.lower_flow_ops(arm.ops)?,
        })
    }

    fn lower_iterator_evidence(
        &self,
        source: RuntimePlanTypeId,
        evidence: RuntimeIteratorEvidenceSeed,
    ) -> Result<(RuntimePlanTypeId, RuntimeIteratorEvidence), RuntimePlanBuildError> {
        match evidence {
            RuntimeIteratorEvidenceSeed::Builtin(evidence) => {
                self.lower_builtin_iterator(source, &evidence)
            }
            RuntimeIteratorEvidenceSeed::Witness(evidence) => {
                self.lower_iterator_witness(source, evidence)
            }
        }
    }

    fn builtin_iterator_item_type(
        &self,
        source: RuntimePlanTypeId,
        evidence: RuntimeBuiltinIteratorFamily,
    ) -> Result<RuntimePlanTypeId, RuntimePlanBuildError> {
        let projection = self.projection(source)?;
        match (evidence, projection) {
            (RuntimeBuiltinIteratorFamily::Range, RuntimePlanTypeProjection::Range(item))
            | (
                RuntimeBuiltinIteratorFamily::Stream,
                RuntimePlanTypeProjection::Stream { item, .. },
            )
            | (
                RuntimeBuiltinIteratorFamily::Array,
                RuntimePlanTypeProjection::Array { item, .. },
            )
            | (
                RuntimeBuiltinIteratorFamily::Seq,
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Seq,
                    item,
                },
            )
            | (
                RuntimeBuiltinIteratorFamily::Vec,
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Vec,
                    item,
                },
            )
            | (
                RuntimeBuiltinIteratorFamily::Slice,
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Slice,
                    item,
                },
            ) => Ok(*item),
            (
                RuntimeBuiltinIteratorFamily::TupleHomogeneous,
                RuntimePlanTypeProjection::Tuple(items),
            ) => {
                let Some(first) = items.first().copied() else {
                    return invalid_projection("empty homogeneous tuple iterator", source);
                };
                if items.iter().all(|item| *item == first) {
                    Ok(first)
                } else {
                    invalid_projection("heterogeneous tuple iterator", source)
                }
            }
            _ => invalid_projection("flow for iterator evidence", source),
        }
    }

    fn lower_builtin_iterator(
        &self,
        source: RuntimePlanTypeId,
        evidence: &RuntimeBuiltinIteratorEvidenceSeed,
    ) -> Result<(RuntimePlanTypeId, RuntimeIteratorEvidence), RuntimePlanBuildError> {
        let item = self.resolve_seed_type("builtin iterator item", evidence.item)?;
        let iterator = self.resolve_seed_type("builtin iterator state", evidence.iterator)?;
        let next_value =
            self.resolve_seed_type("builtin iterator next value", evidence.next_value)?;
        let step = self.resolve_seed_type("builtin iterator step", evidence.step)?;
        require_same(
            "builtin iterator source item",
            item,
            self.builtin_iterator_item_type(source, evidence.family)?,
        )?;
        match self.projection(iterator)? {
            RuntimePlanTypeProjection::Iterator(actual) if *actual == item => {}
            _ => return invalid_projection("builtin iterator state", iterator),
        }
        match self.projection(next_value)? {
            RuntimePlanTypeProjection::Option(actual) if *actual == item => {}
            _ => return invalid_projection("builtin iterator next value", next_value),
        }
        match self.projection(step)? {
            RuntimePlanTypeProjection::Tuple(items) if items.as_ref() == [iterator, next_value] => {
            }
            _ => return invalid_projection("builtin iterator step", step),
        }
        Ok((
            item,
            RuntimeIteratorEvidence::builtin(RuntimeBuiltinIteratorEvidence {
                family: evidence.family,
                item,
                iterator,
                next_value,
                step,
            }),
        ))
    }

    fn await_many_item_type(
        &self,
        source: RuntimePlanTypeId,
    ) -> Result<RuntimePlanTypeId, RuntimePlanBuildError> {
        match self.projection(source)? {
            RuntimePlanTypeProjection::Sequence { item, .. }
            | RuntimePlanTypeProjection::Array { item, .. } => Ok(*item),
            RuntimePlanTypeProjection::Tuple(items) => {
                let Some(first) = items.first().copied() else {
                    return invalid_projection("empty AwaitMany tuple source", source);
                };
                if items.iter().all(|item| *item == first) {
                    Ok(first)
                } else {
                    invalid_projection("heterogeneous AwaitMany tuple source", source)
                }
            }
            _ => invalid_projection("AwaitMany source", source),
        }
    }

    fn lower_iterator_witness(
        &self,
        source: RuntimePlanTypeId,
        evidence: RuntimeIteratorWitnessEvidenceSeed,
    ) -> Result<(RuntimePlanTypeId, RuntimeIteratorEvidence), RuntimePlanBuildError> {
        let item = self.resolve_seed_type("iterator witness item", evidence.item)?;
        let iterator = self.resolve_seed_type("iterator witness state", evidence.iterator)?;
        let executable = match evidence.executable {
            RuntimeIteratorWitnessExecutableSeed::TraitCalls { into_iter, next } => {
                let into_iter = self.resolve_iterator_method(
                    &into_iter,
                    RuntimeReceiverMode::Owned,
                    &[source],
                    iterator,
                    "IntoIterator::into_iter",
                )?;
                let next = self.resolve_iterator_next(&next, iterator, item)?;
                RuntimeIteratorWitnessExecutable::TraitCalls { into_iter, next }
            }
            RuntimeIteratorWitnessExecutableSeed::IdentityIntoIterator { next } => {
                require_same("identity iterator source", source, iterator)?;
                let next = self.resolve_iterator_next(&next, iterator, item)?;
                RuntimeIteratorWitnessExecutable::IdentityIntoIterator { next }
            }
        };
        Ok((
            item,
            RuntimeIteratorEvidence::Witness(RuntimeIteratorWitnessEvidence {
                item,
                iterator,
                executable,
            }),
        ))
    }

    fn resolve_iterator_next(
        &self,
        method: &super::RuntimeTraitMethodSeedId,
        iterator: RuntimePlanTypeId,
        item: RuntimePlanTypeId,
    ) -> Result<super::super::RuntimeTraitMethodId, RuntimePlanBuildError> {
        let (method, receiver, parameters, result) = method
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignTraitMethodSeed)?;
        if receiver != RuntimeReceiverMode::MutRef || parameters != [iterator] {
            return Err(RuntimePlanBuildError::InvalidIteratorWitness {
                context: "Iterator::next receiver/parameters",
            });
        }
        match self.projection(result)? {
            RuntimePlanTypeProjection::Option(actual) if *actual == item => Ok(method),
            _ => Err(RuntimePlanBuildError::InvalidIteratorWitness {
                context: "Iterator::next result",
            }),
        }
    }

    fn resolve_iterator_method(
        &self,
        method: &super::RuntimeTraitMethodSeedId,
        expected_receiver: RuntimeReceiverMode,
        expected_parameters: &[RuntimePlanTypeId],
        expected_result: RuntimePlanTypeId,
        context: &'static str,
    ) -> Result<super::super::RuntimeTraitMethodId, RuntimePlanBuildError> {
        let (method, receiver, parameters, result) = method
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignTraitMethodSeed)?;
        if receiver != expected_receiver
            || parameters != expected_parameters
            || result != expected_result
        {
            return Err(RuntimePlanBuildError::InvalidIteratorWitness { context });
        }
        Ok(method)
    }

    fn lower_choice_option(
        &self,
        option: RuntimeChoiceOptionSeed,
    ) -> Result<ChoiceRuntimeOption, RuntimePlanBuildError> {
        Ok(ChoiceRuntimeOption {
            id: option.id,
            label: option.label,
            target: option.target,
            out: option.out,
            effects: self.lower_line_effects(option.effects)?,
        })
    }

    fn lower_host_call_target(
        &self,
        target: RuntimeHostCallTargetSeed,
    ) -> Result<RuntimeHostCallTarget, RuntimePlanBuildError> {
        Ok(RuntimeHostCallTarget {
            public_id: target.public_id,
            capability: target.capability,
            operation: target.operation,
            contract: target.contract,
            args: target
                .args
                .into_iter()
                .map(|arg| self.lower_host_argument(arg))
                .collect::<Result<_, _>>()?,
            result: self.resolve_seed_type("host call result", target.result)?,
            mode: target.mode,
            deterministic: target.deterministic,
        })
    }

    fn lower_host_task_request(
        &self,
        request: RuntimeHostTaskRequestTemplateSeed,
    ) -> Result<HostTaskRequestTemplate, RuntimePlanBuildError> {
        Ok(HostTaskRequestTemplate {
            capability: request.capability,
            operation: request.operation,
            args: request
                .args
                .into_iter()
                .map(|arg| {
                    Ok(match arg {
                        RuntimeHostArgumentSeed::Positional(value) => {
                            RuntimeHostArgumentTemplate::Positional(self.lower_expression(value)?)
                        }
                        RuntimeHostArgumentSeed::Named(argument) => {
                            RuntimeHostArgumentTemplate::Named(NamedHostArg {
                                name: argument.name,
                                value: self.lower_expression(argument.value)?,
                            })
                        }
                        RuntimeHostArgumentSeed::Spread(value) => {
                            RuntimeHostArgumentTemplate::Spread(self.lower_expression(value)?)
                        }
                    })
                })
                .collect::<Result<_, RuntimePlanBuildError>>()?,
        })
    }

    fn lower_host_argument(
        &self,
        argument: RuntimeHostArgumentSeed,
    ) -> Result<RuntimeHostArgumentTemplate, RuntimePlanBuildError> {
        Ok(match argument {
            RuntimeHostArgumentSeed::Positional(value) => {
                RuntimeHostArgumentTemplate::Positional(self.lower_expression(value)?)
            }
            RuntimeHostArgumentSeed::Named(argument) => {
                RuntimeHostArgumentTemplate::Named(NamedHostArg {
                    name: argument.name,
                    value: self.lower_expression(argument.value)?,
                })
            }
            RuntimeHostArgumentSeed::Spread(value) => {
                RuntimeHostArgumentTemplate::Spread(self.lower_expression(value)?)
            }
        })
    }

    fn lower_line_effects(
        &self,
        effects: Vec<RuntimeLineEffectSeed>,
    ) -> Result<Vec<LineEffectRequest>, RuntimePlanBuildError> {
        effects
            .into_iter()
            .map(|effect| self.lower_line_effect(effect))
            .collect()
    }

    fn lower_line_effect(
        &self,
        effect: RuntimeLineEffectSeed,
    ) -> Result<LineEffectRequest, RuntimePlanBuildError> {
        match effect {
            RuntimeLineEffectSeed::Static(LineEffectRequest::Audio(_)) => {
                Err(RuntimePlanBuildError::RawExpressionCarrier {
                    context: "static line effect Audio",
                })
            }
            RuntimeLineEffectSeed::Static(effect) => Ok(effect),
            RuntimeLineEffectSeed::Audio(command) => Ok(LineEffectRequest::Audio(Box::new(
                self.lower_audio_command(*command)?,
            ))),
        }
    }

    fn lower_audio_command(
        &self,
        command: RuntimeAudioCommandSeed,
    ) -> Result<RuntimeAudioCommand, RuntimePlanBuildError> {
        Ok(match command {
            play @ RuntimeAudioCommandSeed::Play { .. } => self.lower_audio_play(play)?,
            RuntimeAudioCommandSeed::Stop {
                voice,
                fade_out_millis,
            } => RuntimeAudioCommand::Stop {
                voice: self.lower_expression(voice)?,
                fade_out_millis: self.lower_expression(fade_out_millis)?,
            },
            RuntimeAudioCommandSeed::StopAll { fade_out_millis } => RuntimeAudioCommand::StopAll {
                fade_out_millis: self.lower_expression(fade_out_millis)?,
            },
            RuntimeAudioCommandSeed::SetVoiceGain {
                voice,
                gain_db_milli,
                transition_millis,
            } => RuntimeAudioCommand::SetVoiceGain {
                voice: self.lower_expression(voice)?,
                gain_db_milli: self.lower_expression(gain_db_milli)?,
                transition_millis: self.lower_expression(transition_millis)?,
            },
            RuntimeAudioCommandSeed::SetVoicePan {
                voice,
                pan_milli,
                transition_millis,
            } => RuntimeAudioCommand::SetVoicePan {
                voice: self.lower_expression(voice)?,
                pan_milli: self.lower_expression(pan_milli)?,
                transition_millis: self.lower_expression(transition_millis)?,
            },
            RuntimeAudioCommandSeed::SetBusGain {
                bus,
                gain_db_milli,
                transition_millis,
            } => RuntimeAudioCommand::SetBusGain {
                bus: self.lower_expression(bus)?,
                gain_db_milli: self.lower_expression(gain_db_milli)?,
                transition_millis: self.lower_expression(transition_millis)?,
            },
            RuntimeAudioCommandSeed::SetBusMute { bus, muted } => RuntimeAudioCommand::SetBusMute {
                bus: self.lower_expression(bus)?,
                muted: self.lower_expression(muted)?,
            },
            RuntimeAudioCommandSeed::SetEffectEnabled {
                bus,
                effect,
                enabled,
            } => RuntimeAudioCommand::SetEffectEnabled {
                bus: self.lower_expression(bus)?,
                effect: self.lower_expression(effect)?,
                enabled: self.lower_expression(enabled)?,
            },
            RuntimeAudioCommandSeed::SetEffectParameter {
                bus,
                effect,
                parameter,
                value,
                transition_millis,
            } => RuntimeAudioCommand::SetEffectParameter {
                bus: self.lower_expression(bus)?,
                effect: self.lower_expression(effect)?,
                parameter,
                value: self.lower_expression(value)?,
                transition_millis: self.lower_expression(transition_millis)?,
            },
            RuntimeAudioCommandSeed::ApplySnapshot {
                snapshot,
                transition_millis,
            } => RuntimeAudioCommand::ApplySnapshot {
                snapshot: self.lower_expression(snapshot)?,
                transition_millis: self.lower_expression(transition_millis)?,
            },
            RuntimeAudioCommandSeed::RequestMicrophone {
                capture,
                constraints,
            } => RuntimeAudioCommand::RequestMicrophone {
                capture: self.lower_expression(capture)?,
                constraints,
            },
            RuntimeAudioCommandSeed::StopMicrophone { capture } => {
                RuntimeAudioCommand::StopMicrophone {
                    capture: self.lower_expression(capture)?,
                }
            }
            RuntimeAudioCommandSeed::SetCaptureMonitor {
                capture,
                bus,
                gain_db_milli,
            } => self.lower_audio_capture_monitor(capture, bus, gain_db_milli)?,
        })
    }

    fn lower_audio_play(
        &self,
        play: RuntimeAudioCommandSeed,
    ) -> Result<RuntimeAudioCommand, RuntimePlanBuildError> {
        let RuntimeAudioCommandSeed::Play {
            voice,
            resource,
            bus,
            gain_db_milli,
            pan_milli,
            loop_mode,
            start_frame,
            fade_in_millis,
        } = play
        else {
            return Err(RuntimePlanBuildError::RawExpressionCarrier {
                context: "non-Play seed passed to Play admission",
            });
        };
        Ok(RuntimeAudioCommand::Play {
            voice: self.lower_expression(voice)?,
            resource: self.lower_expression(resource)?,
            bus: self.lower_expression(bus)?,
            gain_db_milli: self.lower_expression(gain_db_milli)?,
            pan_milli: self.lower_expression(pan_milli)?,
            loop_mode,
            start_frame: self.lower_expression(start_frame)?,
            fade_in_millis: self.lower_expression(fade_in_millis)?,
        })
    }

    fn lower_audio_capture_monitor(
        &self,
        capture: RuntimeExprSeed,
        bus: Option<RuntimeExprSeed>,
        gain_db_milli: RuntimeExprSeed,
    ) -> Result<RuntimeAudioCommand, RuntimePlanBuildError> {
        Ok(RuntimeAudioCommand::SetCaptureMonitor {
            capture: self.lower_expression(capture)?,
            bus: bus.map(|bus| self.lower_expression(bus)).transpose()?,
            gain_db_milli: self.lower_expression(gain_db_milli)?,
        })
    }

    fn lower_effect_fields(
        &self,
        fields: Vec<super::RuntimeEffectFieldSeed>,
    ) -> Result<Vec<RuntimeEffectFieldExpr>, RuntimePlanBuildError> {
        fields
            .into_iter()
            .map(|field| {
                Ok(RuntimeEffectFieldExpr {
                    name: field.name,
                    value: self.lower_expression(field.value)?,
                })
            })
            .collect()
    }

    fn lower_evaluated_effect(
        &self,
        effect: RuntimeEvaluatedEffectSeed,
    ) -> Result<RuntimeEffectExpr, RuntimePlanBuildError> {
        Ok(match effect {
            RuntimeEvaluatedEffectSeed::Log {
                level,
                message,
                fields,
            } => RuntimeEffectExpr::Log {
                level,
                message: self.lower_expression(message)?,
                fields: self.lower_effect_fields(fields)?,
            },
            RuntimeEvaluatedEffectSeed::SignalWrite { target, value } => {
                RuntimeEffectExpr::SignalWrite {
                    target: self.lower_expression(target)?,
                    value: self.lower_expression(value)?,
                }
            }
            RuntimeEvaluatedEffectSeed::MetricWrite { target, value } => {
                RuntimeEffectExpr::MetricWrite {
                    target: self.lower_expression(target)?,
                    value: self.lower_expression(value)?,
                }
            }
            RuntimeEvaluatedEffectSeed::EmitEvent { event, fields } => {
                RuntimeEffectExpr::EmitEvent {
                    event: self.lower_expression(event)?,
                    fields: self.lower_effect_fields(fields)?,
                }
            }
            RuntimeEvaluatedEffectSeed::Panic(message) => {
                RuntimeEffectExpr::Panic(self.lower_expression(message)?)
            }
            RuntimeEvaluatedEffectSeed::Fail(message) => {
                RuntimeEffectExpr::Fail(self.lower_expression(message)?)
            }
            RuntimeEvaluatedEffectSeed::Bail(message) => {
                RuntimeEffectExpr::Bail(self.lower_expression(message)?)
            }
            RuntimeEvaluatedEffectSeed::Ensure { condition, message } => {
                let condition = self.lower_expression(condition)?;
                self.require_bool("ensure condition", condition.ty())?;
                RuntimeEffectExpr::Ensure {
                    condition,
                    message: self.lower_expression(message)?,
                }
            }
            RuntimeEvaluatedEffectSeed::Assert {
                guard,
                condition,
                message,
                profile,
            } => {
                let condition = self.lower_expression(condition)?;
                self.require_bool("assert condition", condition.ty())?;
                RuntimeEffectExpr::Assert {
                    guard,
                    condition,
                    message,
                    profile,
                }
            }
        })
    }

    pub(super) fn validate_flow_operation_locals(
        &self,
        ops: &[FlowOp],
        scope: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        let mut used = BTreeSet::new();
        self.validate_flow_operation_locals_inner(ops, scope, &mut used)
    }

    pub(super) fn validate_line_task_actions_locals(
        &self,
        actions: &[&[FlowOp]],
        captures: &BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<BTreeSet<RuntimeLocalDeclarationId>, RuntimePlanBuildError> {
        let mut used = BTreeSet::new();
        for action in actions {
            let mut scope = captures.clone();
            self.validate_flow_operation_locals_inner(action, &mut scope, &mut used)?;
        }
        Ok(used)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "lexical validation exhaustively rejects runtime-only continuation operations"
    )]
    fn validate_flow_operation_locals_inner(
        &self,
        ops: &[FlowOp],
        scope: &mut BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        for op in ops {
            match op {
                FlowOp::Let { pattern, expr } => {
                    self.validate_expression_locals(expr, scope, used)?;
                    *scope = extend_scope(scope, pattern_binding_locals(pattern))?;
                }
                FlowOp::LetElse {
                    pattern,
                    expr,
                    else_ops,
                } => {
                    self.validate_expression_locals(expr, scope, used)?;
                    let mut else_scope = scope.clone();
                    self.validate_flow_operation_locals_inner(else_ops, &mut else_scope, used)?;
                    *scope = extend_scope(scope, pattern_binding_locals(pattern))?;
                }
                FlowOp::AssignNominalField { base, value, .. } => {
                    require_local_in_scope(*base, scope)?;
                    used.insert(*base);
                    self.validate_expression_locals(value, scope, used)?;
                }
                FlowOp::Choice { options, .. } => {
                    for option in options {
                        self.validate_line_effect_locals(&option.effects, scope, used)?;
                    }
                }
                FlowOp::Await {
                    binding,
                    target,
                    observers,
                } => {
                    self.validate_task_request_locals(&target.request, scope, used)?;
                    for observer in observers {
                        let mut observer_scope =
                            extend_scope(scope, pattern_binding_locals(&observer.pattern))?;
                        self.validate_flow_operation_locals_inner(
                            &observer.ops,
                            &mut observer_scope,
                            used,
                        )?;
                    }
                    if let Some(binding) = binding {
                        *scope = extend_scope(scope, pattern_binding_locals(binding))?;
                    }
                }
                FlowOp::AwaitMany {
                    binding,
                    target,
                    pending,
                } => {
                    self.validate_expression_locals(&target.source, scope, used)?;
                    let item_scope = extend_scope(scope, [target.item_binding])?;
                    self.validate_task_request_locals(&target.request, &item_scope, used)?;
                    self.validate_line_effect_locals(pending, scope, used)?;
                    if let Some(binding) = binding {
                        *scope = extend_scope(scope, pattern_binding_locals(binding))?;
                    }
                }
                FlowOp::HostCall { binding, target } => {
                    self.validate_host_argument_locals(&target.args, scope, used)?;
                    if let Some(binding) = binding {
                        *scope = extend_scope(scope, pattern_binding_locals(binding))?;
                    }
                }
                FlowOp::If {
                    condition,
                    then_ops,
                    else_ops,
                } => {
                    self.validate_expression_locals(condition, scope, used)?;
                    let mut then_scope = scope.clone();
                    self.validate_flow_operation_locals_inner(then_ops, &mut then_scope, used)?;
                    let mut else_scope = scope.clone();
                    self.validate_flow_operation_locals_inner(else_ops, &mut else_scope, used)?;
                }
                FlowOp::IfLet {
                    pattern,
                    expr,
                    guard,
                    then_ops,
                    else_ops,
                } => {
                    self.validate_expression_locals(expr, scope, used)?;
                    let mut then_scope = extend_scope(scope, pattern_binding_locals(pattern))?;
                    if let Some(guard) = guard {
                        self.validate_expression_locals(guard, &then_scope, used)?;
                    }
                    self.validate_flow_operation_locals_inner(then_ops, &mut then_scope, used)?;
                    let mut else_scope = scope.clone();
                    self.validate_flow_operation_locals_inner(else_ops, &mut else_scope, used)?;
                }
                FlowOp::Match { scrutinee, arms } => {
                    self.validate_expression_locals(scrutinee, scope, used)?;
                    for arm in arms {
                        let mut arm_scope =
                            extend_scope(scope, pattern_binding_locals(&arm.pattern))?;
                        if let Some(guard) = &arm.guard {
                            self.validate_expression_locals(guard, &arm_scope, used)?;
                        }
                        self.validate_flow_operation_locals_inner(&arm.ops, &mut arm_scope, used)?;
                    }
                }
                FlowOp::Loop { result, body } => {
                    let mut nested = scope.clone();
                    self.validate_flow_operation_locals_inner(body, &mut nested, used)?;
                    if let Some(result) = result {
                        *scope = extend_scope(scope, pattern_binding_locals(result))?;
                    }
                }
                FlowOp::Thread { body, .. } | FlowOp::Scope(body) => {
                    let mut nested = scope.clone();
                    self.validate_flow_operation_locals_inner(body, &mut nested, used)?;
                }
                FlowOp::While { condition, body } => {
                    self.validate_expression_locals(condition, scope, used)?;
                    let mut nested = scope.clone();
                    self.validate_flow_operation_locals_inner(body, &mut nested, used)?;
                }
                FlowOp::WhileLet {
                    pattern,
                    expr,
                    guard,
                    body,
                } => {
                    self.validate_expression_locals(expr, scope, used)?;
                    let mut nested = extend_scope(scope, pattern_binding_locals(pattern))?;
                    if let Some(guard) = guard {
                        self.validate_expression_locals(guard, &nested, used)?;
                    }
                    self.validate_flow_operation_locals_inner(body, &mut nested, used)?;
                }
                FlowOp::For {
                    pattern,
                    source,
                    body,
                    ..
                } => {
                    self.validate_expression_locals(source, scope, used)?;
                    let mut nested = extend_scope(scope, pattern_binding_locals(pattern))?;
                    self.validate_flow_operation_locals_inner(body, &mut nested, used)?;
                }
                FlowOp::Break(value) => {
                    if let Some(value) = value {
                        self.validate_expression_locals(value, scope, used)?;
                    }
                }
                FlowOp::GotoExpr(target) | FlowOp::ReturnExpr(target) => {
                    self.validate_expression_locals(target, scope, used)?;
                }
                FlowOp::Effect(effect) | FlowOp::RegisterCleanup { effect, .. } => {
                    self.validate_line_effect_locals(std::slice::from_ref(effect), scope, used)?;
                }
                FlowOp::EvaluatedEffect(effect) => {
                    for expression in effect.argument_exprs() {
                        self.validate_expression_locals(expression, scope, used)?;
                    }
                }
                FlowOp::Continue
                | FlowOp::Goto(_)
                | FlowOp::Return(_)
                | FlowOp::CancelCleanup { .. }
                | FlowOp::EnterScope
                | FlowOp::ExitScope
                | FlowOp::Noop => {}
                FlowOp::Bind(_) => {
                    return Err(RuntimePlanBuildError::NonCanonicalFlowOperation {
                        operation: "Bind",
                    });
                }
                FlowOp::Dialogue { content, .. } => {
                    if self.dialogue_content.get(*content).is_none() {
                        return Err(RuntimePlanBuildError::ForeignDialogueContentSeed);
                    }
                }
                FlowOp::LoopNext { .. } => {
                    return Err(RuntimePlanBuildError::NonCanonicalFlowOperation {
                        operation: "LoopNext",
                    });
                }
                FlowOp::WhileNext { .. } => {
                    return Err(RuntimePlanBuildError::NonCanonicalFlowOperation {
                        operation: "WhileNext",
                    });
                }
                FlowOp::WhileLetNext { .. } => {
                    return Err(RuntimePlanBuildError::NonCanonicalFlowOperation {
                        operation: "WhileLetNext",
                    });
                }
                FlowOp::ForNext { .. } => {
                    return Err(RuntimePlanBuildError::NonCanonicalFlowOperation {
                        operation: "ForNext",
                    });
                }
                FlowOp::LetScope { .. } => {
                    return Err(RuntimePlanBuildError::NonCanonicalFlowOperation {
                        operation: "LetScope",
                    });
                }
                FlowOp::ExitScopeBind { .. } => {
                    return Err(RuntimePlanBuildError::NonCanonicalFlowOperation {
                        operation: "ExitScopeBind",
                    });
                }
                FlowOp::CompleteAwaitObserver => {
                    return Err(RuntimePlanBuildError::NonCanonicalFlowOperation {
                        operation: "CompleteAwaitObserver",
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_task_request_locals(
        &self,
        request: &HostTaskRequestTemplate,
        scope: &BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        self.validate_host_argument_locals(&request.args, scope, used)
    }

    fn validate_host_argument_locals(
        &self,
        arguments: &[RuntimeHostArgumentTemplate],
        scope: &BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        for argument in arguments {
            let value = match argument {
                RuntimeHostArgumentTemplate::Positional(value)
                | RuntimeHostArgumentTemplate::Spread(value) => value,
                RuntimeHostArgumentTemplate::Named(argument) => &argument.value,
            };
            self.validate_expression_locals(value, scope, used)?;
        }
        Ok(())
    }

    fn validate_line_effect_locals(
        &self,
        effects: &[LineEffectRequest],
        scope: &BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        for effect in effects {
            if let LineEffectRequest::Audio(command) = effect {
                for expression in audio_command_expressions(command) {
                    self.validate_expression_locals(expression, scope, used)?;
                }
            }
        }
        Ok(())
    }
}

fn audio_command_expressions(command: &RuntimeAudioCommand) -> Vec<&RuntimeExpr> {
    match command {
        RuntimeAudioCommand::Play {
            voice,
            resource,
            bus,
            gain_db_milli,
            pan_milli,
            start_frame,
            fade_in_millis,
            ..
        } => vec![
            voice,
            resource,
            bus,
            gain_db_milli,
            pan_milli,
            start_frame,
            fade_in_millis,
        ],
        RuntimeAudioCommand::Stop {
            voice,
            fade_out_millis,
        } => vec![voice, fade_out_millis],
        RuntimeAudioCommand::StopAll { fade_out_millis } => vec![fade_out_millis],
        RuntimeAudioCommand::SetVoiceGain {
            voice,
            gain_db_milli,
            transition_millis,
        } => vec![voice, gain_db_milli, transition_millis],
        RuntimeAudioCommand::SetVoicePan {
            voice,
            pan_milli,
            transition_millis,
        } => vec![voice, pan_milli, transition_millis],
        RuntimeAudioCommand::SetBusGain {
            bus,
            gain_db_milli,
            transition_millis,
        } => vec![bus, gain_db_milli, transition_millis],
        RuntimeAudioCommand::SetBusMute { bus, muted } => vec![bus, muted],
        RuntimeAudioCommand::SetEffectEnabled {
            bus,
            effect,
            enabled,
        } => vec![bus, effect, enabled],
        RuntimeAudioCommand::SetEffectParameter {
            bus,
            effect,
            value,
            transition_millis,
            ..
        } => vec![bus, effect, value, transition_millis],
        RuntimeAudioCommand::ApplySnapshot {
            snapshot,
            transition_millis,
        } => vec![snapshot, transition_millis],
        RuntimeAudioCommand::RequestMicrophone { capture, .. }
        | RuntimeAudioCommand::StopMicrophone { capture } => vec![capture],
        RuntimeAudioCommand::SetCaptureMonitor {
            capture,
            bus,
            gain_db_milli,
        } => std::iter::once(capture)
            .chain(bus.iter())
            .chain(std::iter::once(gain_db_milli))
            .collect(),
    }
}

fn require_local_in_scope(
    local: RuntimeLocalDeclarationId,
    scope: &BTreeSet<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimePlanBuildError> {
    if scope.contains(&local) {
        Ok(())
    } else {
        Err(RuntimePlanBuildError::UnreachableFunctionLocal { local })
    }
}

fn extend_scope(
    scope: &BTreeSet<RuntimeLocalDeclarationId>,
    locals: impl IntoIterator<Item = RuntimeLocalDeclarationId>,
) -> Result<BTreeSet<RuntimeLocalDeclarationId>, RuntimePlanBuildError> {
    let mut nested = scope.clone();
    for local in locals {
        if !nested.insert(local) {
            return Err(RuntimePlanBuildError::DuplicateFunctionLocal { local });
        }
    }
    Ok(nested)
}

fn pattern_binding_locals(pattern: &RuntimePattern) -> Vec<RuntimeLocalDeclarationId> {
    let mut locals = Vec::new();
    collect_pattern_binding_locals(pattern, &mut locals);
    locals
}

fn collect_pattern_binding_locals(
    pattern: &RuntimePattern,
    locals: &mut Vec<RuntimeLocalDeclarationId>,
) {
    match pattern.kind() {
        RuntimePatternKind::Bind { binding, .. } | RuntimePatternKind::Typed { binding } => {
            locals.push(binding.local());
        }
        RuntimePatternKind::Discard
        | RuntimePatternKind::Literal(_)
        | RuntimePatternKind::Entity(_) => {}
        RuntimePatternKind::Tuple(items) => {
            for item in items {
                collect_pattern_binding_locals(item, locals);
            }
        }
        RuntimePatternKind::Record { fields, rest } => {
            for field in fields {
                collect_pattern_binding_locals(field.pattern(), locals);
            }
            if let Some(binding) = rest.binding() {
                locals.push(binding.local());
            }
        }
        RuntimePatternKind::Sequence { items, rest } => {
            for item in items {
                collect_pattern_binding_locals(item, locals);
            }
            if let Some(binding) = rest.binding() {
                locals.push(binding.local());
            }
        }
        RuntimePatternKind::Variant { payload, .. } => {
            if let Some(payload) = payload {
                collect_pattern_binding_locals(payload, locals);
            }
        }
        RuntimePatternKind::Whole { binding, pattern } => {
            locals.push(binding.local());
            collect_pattern_binding_locals(pattern, locals);
        }
    }
}

fn bytes_value_matches(value: &RuntimeValue) -> bool {
    let RuntimeValue::Seq(sequence) = value else {
        return false;
    };
    sequence.clone().into_values().iter().all(|value| {
        matches!(
            value,
            RuntimeValue::UInt(value) if value.width() == RuntimeUnsignedIntWidth::U8
        )
    })
}

impl RuntimePlanBuilder {
    fn validate_plan_value(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
        value: &RuntimeValue,
    ) -> Result<(), RuntimePlanBuildError> {
        if value.contains_function() {
            return Err(RuntimePlanBuildError::FunctionValueInPlan { context });
        }
        if value.contains_nonconstant_opaque() {
            return Err(RuntimePlanBuildError::NonConstantOpaqueValueInPlan { context });
        }
        if self.value_matches_type(ty, value, 0)? {
            Ok(())
        } else {
            Err(RuntimePlanBuildError::InvalidValueType { context, ty })
        }
    }

    fn value_matches_type(
        &self,
        ty: RuntimePlanTypeId,
        value: &RuntimeValue,
        depth: usize,
    ) -> Result<bool, RuntimePlanBuildError> {
        if depth > crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH {
            return Ok(false);
        }
        let declaration =
            self.types
                .get(ty)
                .ok_or(RuntimePlanBuildError::InvalidTypeProjection {
                    context: "literal plan type",
                    ty,
                })?;
        self.value_matches_projection(ty, declaration.projection(), value, depth)
    }

    fn value_matches_projection(
        &self,
        ty: RuntimePlanTypeId,
        projection: &RuntimePlanTypeProjection<RuntimePlanTypeId>,
        value: &RuntimeValue,
        depth: usize,
    ) -> Result<bool, RuntimePlanBuildError> {
        Ok(match projection {
            RuntimePlanTypeProjection::Unit => matches!(value, RuntimeValue::Unit),
            RuntimePlanTypeProjection::Bool => matches!(value, RuntimeValue::Bool(_)),
            RuntimePlanTypeProjection::Signed(width) => {
                matches!(value, RuntimeValue::Int(value) if *width == value.width())
            }
            RuntimePlanTypeProjection::Unsigned(width) => {
                matches!(value, RuntimeValue::UInt(value) if *width == value.width())
            }
            RuntimePlanTypeProjection::F32 => matches!(value, RuntimeValue::F32(_)),
            RuntimePlanTypeProjection::F64 => matches!(value, RuntimeValue::F64(_)),
            RuntimePlanTypeProjection::String => matches!(value, RuntimeValue::String(_)),
            RuntimePlanTypeProjection::Char => matches!(value, RuntimeValue::Char(_)),
            RuntimePlanTypeProjection::Bytes => bytes_value_matches(value),
            RuntimePlanTypeProjection::Duration => matches!(value, RuntimeValue::Duration(_)),
            RuntimePlanTypeProjection::Progress => matches!(value, RuntimeValue::Progress(_)),
            RuntimePlanTypeProjection::EntityReference => {
                matches!(value, RuntimeValue::EntityRef(_))
            }
            RuntimePlanTypeProjection::Range(item) => match value {
                RuntimeValue::Range(range) => self.range_matches_item(*item, range)?,
                _ => false,
            },
            RuntimePlanTypeProjection::Iterator(item) => match value {
                RuntimeValue::Iterator(iterator) => {
                    self.iterator_matches_item(*item, iterator, depth + 1)?
                }
                _ => false,
            },
            RuntimePlanTypeProjection::Sequence { item, .. }
            | RuntimePlanTypeProjection::Array { item, .. } => {
                self.sequence_value_matches(projection, *item, value, depth + 1)?
            }
            RuntimePlanTypeProjection::Tuple(items) => {
                self.tuple_value_matches(items, value, depth + 1)?
            }
            RuntimePlanTypeProjection::Choice(alternatives) => {
                self.choice_value_matches(alternatives, value, depth + 1)?
            }
            RuntimePlanTypeProjection::Result { .. } | RuntimePlanTypeProjection::Option(_) => {
                matches!(value, RuntimeValue::Variant { .. })
                    && self.variant_value_matches(ty, value, depth + 1)?
            }
            RuntimePlanTypeProjection::ProjectNominal {
                nominal, layout, ..
            } => self.project_nominal_value_matches(ty, nominal, *layout, value, depth + 1)?,
            projection @ RuntimePlanTypeProjection::Opaque { .. } => {
                self.opaque_value_matches(ty, projection, value, depth + 1)?
            }
            RuntimePlanTypeProjection::Agent(expected) => match value {
                RuntimeValue::Agent(value) => {
                    !matches!(expected, RuntimeAgentTypeProjection::Probe(_))
                        && expected.operational_type() == value.operational_type()
                }
                _ => false,
            },
            RuntimePlanTypeProjection::Never
            | RuntimePlanTypeProjection::Map { .. }
            | RuntimePlanTypeProjection::Need(_)
            | RuntimePlanTypeProjection::Stream { .. }
            | RuntimePlanTypeProjection::ThreadHandle(_)
            | RuntimePlanTypeProjection::Shared(_)
            | RuntimePlanTypeProjection::Reference(_)
            | RuntimePlanTypeProjection::Function { .. } => false,
        })
    }

    fn sequence_value_matches(
        &self,
        projection: &RuntimePlanTypeProjection<RuntimePlanTypeId>,
        item: RuntimePlanTypeId,
        value: &RuntimeValue,
        depth: usize,
    ) -> Result<bool, RuntimePlanBuildError> {
        let RuntimeValue::Seq(sequence) = value else {
            return Ok(false);
        };
        let values = sequence.clone().into_values();
        if let RuntimePlanTypeProjection::Array { length, .. } = projection
            && u64::try_from(values.len()).ok() != Some(*length)
        {
            return Ok(false);
        }
        self.values_match_type(item, &values, depth)
    }

    fn tuple_value_matches(
        &self,
        items: &[RuntimePlanTypeId],
        value: &RuntimeValue,
        depth: usize,
    ) -> Result<bool, RuntimePlanBuildError> {
        let RuntimeValue::Tuple(values) = value else {
            return Ok(false);
        };
        if items.len() != values.len() {
            return Ok(false);
        }
        for (item, value) in items.iter().zip(values) {
            if !self.value_matches_type(*item, value, depth)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn choice_value_matches(
        &self,
        alternatives: &[RuntimePlanTypeId],
        value: &RuntimeValue,
        depth: usize,
    ) -> Result<bool, RuntimePlanBuildError> {
        for alternative in alternatives {
            if self.value_matches_type(*alternative, value, depth)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn project_nominal_value_matches(
        &self,
        ty: RuntimePlanTypeId,
        nominal: &crate::entry::RuntimeNominalTypeId,
        layout: TypeLayoutHash,
        value: &RuntimeValue,
        depth: usize,
    ) -> Result<bool, RuntimePlanBuildError> {
        if self.variant_domains.get(ty).is_some() {
            return match value {
                RuntimeValue::Variant { .. } => self.variant_value_matches(ty, value, depth),
                _ => Ok(false),
            };
        }
        let RuntimeValue::NominalRecord(record) = value else {
            return Ok(false);
        };
        let Some(domain) = self.nominal_record_domains.get(ty) else {
            return Ok(false);
        };
        Ok(nominal == record.type_id()
            && layout == record.layout()
            && domain.fields().len() == record.fields().len()
            && self.nominal_fields_match(domain, record.fields(), depth)?)
    }

    fn opaque_value_matches(
        &self,
        ty: RuntimePlanTypeId,
        projection: &RuntimePlanTypeProjection<RuntimePlanTypeId>,
        value: &RuntimeValue,
        depth: usize,
    ) -> Result<bool, RuntimePlanBuildError> {
        let RuntimePlanTypeProjection::Opaque {
            producer,
            admission,
            value_class,
            persistence,
            arguments,
        } = projection
        else {
            unreachable!("opaque value matching receives one opaque projection")
        };
        let semantic_identity = self
            .types
            .get(ty)
            .ok_or(RuntimePlanBuildError::InvalidTypeProjection {
                context: "opaque literal type",
                ty,
            })?
            .semantic_identity();
        if self.variant_domains.get(ty).is_some() {
            return match value {
                RuntimeValue::Variant { .. } => self.variant_value_matches(ty, value, depth),
                _ => Ok(false),
            };
        }
        match value {
            RuntimeValue::Reduction(value) => Ok(*admission
                == RuntimeOpaqueTypeAdmission::ExactIdentity
                && RuntimeReductionProducer::accepts(producer)
                && value.owner().producer() == producer
                && value.owner().admission() == RuntimeOpaqueTypeAdmission::ExactIdentity
                && value.owner().semantic_identity() == semantic_identity
                && value.owner().value_class() == *value_class
                && value.owner().persistence() == *persistence
                && match arguments.as_ref() {
                    [state] => self.value_matches_type(*state, value.state(), depth)?,
                    _ => false,
                }),
            RuntimeValue::Opaque(value) => Ok(producer == value.producer()
                && *value_class == value.value_class()
                && *persistence == value.persistence()
                && match *admission {
                    RuntimeOpaqueTypeAdmission::ExactIdentity => {
                        semantic_identity == value.semantic_identity()
                    }
                    RuntimeOpaqueTypeAdmission::ProducerWide => true,
                }),
            _ => Ok(false),
        }
    }

    fn range_matches_item(
        &self,
        item: RuntimePlanTypeId,
        range: &RuntimeRange,
    ) -> Result<bool, RuntimePlanBuildError> {
        Ok(match (self.projection(item)?, range) {
            (RuntimePlanTypeProjection::Signed(expected), RuntimeRange::Int { start, end, .. }) => {
                (start.is_some() || end.is_some())
                    && start
                        .iter()
                        .chain(end.iter())
                        .all(|value| value.width() == *expected)
            }
            (
                RuntimePlanTypeProjection::Unsigned(expected),
                RuntimeRange::UInt { start, end, .. },
            ) => {
                (start.is_some() || end.is_some())
                    && start
                        .iter()
                        .chain(end.iter())
                        .all(|value| value.width() == *expected)
            }
            _ => false,
        })
    }

    fn values_match_type(
        &self,
        item: RuntimePlanTypeId,
        values: &[RuntimeValue],
        depth: usize,
    ) -> Result<bool, RuntimePlanBuildError> {
        for value in values {
            if !self.value_matches_type(item, value, depth)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn nominal_fields_match(
        &self,
        domain: &super::super::RuntimeNominalRecordDomain,
        values: &[RuntimeValue],
        depth: usize,
    ) -> Result<bool, RuntimePlanBuildError> {
        for (field, value) in domain.fields().iter().zip(values) {
            if !self.value_matches_type(field.ty(), value, depth)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn iterator_matches_item(
        &self,
        item: RuntimePlanTypeId,
        iterator: &crate::value::RuntimeIterator,
        depth: usize,
    ) -> Result<bool, RuntimePlanBuildError> {
        match iterator {
            crate::value::RuntimeIterator::Values { items, .. } => {
                for value in items {
                    if !self.value_matches_type(item, value, depth)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            crate::value::RuntimeIterator::Range(range) => {
                Ok(match (self.projection(item)?, range) {
                    (
                        RuntimePlanTypeProjection::Signed(expected),
                        crate::value::RuntimeRangeIterator::Int { width, .. },
                    ) => expected == width,
                    (
                        RuntimePlanTypeProjection::Unsigned(expected),
                        crate::value::RuntimeRangeIterator::UInt { width, .. },
                    ) => expected == width,
                    _ => false,
                })
            }
            crate::value::RuntimeIterator::Witness { .. } => Ok(false),
        }
    }

    fn variant_value_matches(
        &self,
        owner: RuntimePlanTypeId,
        value: &RuntimeValue,
        depth: usize,
    ) -> Result<bool, RuntimePlanBuildError> {
        let RuntimeValue::Variant {
            owner: actual_owner,
            ordinal,
            name,
            payload,
        } = value
        else {
            return Ok(false);
        };
        let declaration =
            self.types
                .get(owner)
                .ok_or(RuntimePlanBuildError::InvalidTypeProjection {
                    context: "variant value owner",
                    ty: owner,
                })?;
        let owner_matches = match (declaration.projection(), actual_owner) {
            (RuntimePlanTypeProjection::Option(_), RuntimeVariantIdentity::Option)
            | (RuntimePlanTypeProjection::Result { .. }, RuntimeVariantIdentity::Result) => true,
            (
                RuntimePlanTypeProjection::ProjectNominal { .. }
                | RuntimePlanTypeProjection::Opaque { .. },
                RuntimeVariantIdentity::Nominal {
                    nominal: actual,
                    semantic_identity,
                },
            ) => {
                self.variant_domains
                    .get(owner)
                    .is_some_and(|domain| domain.nominal() == actual)
                    && declaration.semantic_identity() == *semantic_identity
            }
            _ => false,
        };
        if !owner_matches {
            return Ok(false);
        }
        let (expected_name, expected_payload) = self.variant_case_metadata(owner, *ordinal)?;
        if expected_name != name || expected_payload.is_some() != payload.is_some() {
            return Ok(false);
        }
        match (expected_payload, payload) {
            (Some(expected), Some(payload)) => self.value_matches_type(expected, payload, depth),
            (None, None) => Ok(true),
            _ => Ok(false),
        }
    }

    fn variant_case_metadata(
        &self,
        owner: RuntimePlanTypeId,
        ordinal: u32,
    ) -> Result<(&str, Option<RuntimePlanTypeId>), RuntimePlanBuildError> {
        if let Some(domain) = self.variant_domains.get(owner) {
            let case = domain
                .case(ordinal)
                .ok_or(RuntimePlanBuildError::UnknownVariantCase { owner, ordinal })?;
            return Ok((case.name(), case.payload()));
        }
        match (self.projection(owner)?, ordinal) {
            (RuntimePlanTypeProjection::Option(item), 0) => Ok(("Some", Some(*item))),
            (RuntimePlanTypeProjection::Option(_), 1) => Ok(("None", None)),
            (RuntimePlanTypeProjection::Result { value, .. }, 0) => Ok(("Ok", Some(*value))),
            (RuntimePlanTypeProjection::Result { error, .. }, 1) => Ok(("Err", Some(*error))),
            _ => Err(RuntimePlanBuildError::UnknownVariantCase { owner, ordinal }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{RuntimeNominalTypeId, TypeLayoutHash};
    use crate::pattern::{
        RuntimeOpaqueTypeProducerId, RuntimePatternBindingPathError, RuntimeSemanticTypeId,
    };
    use crate::plan::{
        RuntimeLocalDeclarationSeed, RuntimeNominalRecordDomainFieldSeed,
        RuntimeNominalRecordDomainSeed, RuntimePlanTypeSeed, RuntimeRecordPatternFieldSeed,
        RuntimeVariantCaseSeed, RuntimeVariantDomainSeed,
    };
    use crate::value::{RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass};

    fn identity(marker: u8) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes([marker; 32])
    }

    fn nominal(label: &str) -> RuntimeNominalTypeId {
        RuntimeNominalTypeId::try_new(label).expect("test nominal")
    }

    #[test]
    fn opaque_record_projection_retains_the_complete_exact_owner() {
        let producer =
            RuntimeOpaqueTypeProducerId::try_new("fixture.dialogue-view").expect("test producer");
        let semantic_owner = identity(91);
        let field_type = identity(92);
        let mut builder = RuntimePlanBuilder::new();
        let admission = builder
            .admit_semantic_batch(
                [
                    RuntimePlanTypeSeed::new(
                        semantic_owner,
                        RuntimePlanTypeProjection::Opaque {
                            producer: producer.clone(),
                            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                            value_class: RuntimeOpaqueValueClass::AffineHandle(
                                RuntimeHandleKind::StageActor,
                            ),
                            persistence: RuntimeOpaquePersistence::SnapshotOnly,
                            arguments: Box::new([]),
                        },
                    ),
                    RuntimePlanTypeSeed::new(field_type, RuntimePlanTypeProjection::String),
                ],
                [RuntimeLocalDeclarationSeed::new(semantic_owner)],
                [],
                [],
            )
            .expect("opaque record graph");
        let expression = builder
            .lower_expression(RuntimeExprSeed::new(
                field_type,
                RuntimeExprSeedKind::Field {
                    target: Box::new(RuntimeExprSeed::new(
                        semantic_owner,
                        RuntimeExprSeedKind::Local(admission.local_ids()[0].clone()),
                    )),
                    field: RuntimeFieldProjectionSeed::OpaqueRecord {
                        owner: semantic_owner,
                        producer: producer.clone(),
                        field: RuntimeRecordFieldSeedId::from_zero_based(0),
                        field_type,
                    },
                },
            ))
            .expect("opaque record projection");

        let RuntimeExprKind::Field {
            field: RuntimeFieldProjection::OpaqueRecord { owner, field },
            ..
        } = expression.kind()
        else {
            panic!("opaque record field expression");
        };
        assert_eq!(field.zero_based(), 0);
        assert_eq!(owner.producer(), &producer);
        assert_eq!(owner.semantic_identity(), semantic_owner);
        assert_eq!(owner.admission(), RuntimeOpaqueTypeAdmission::ExactIdentity);
        assert_eq!(
            owner.value_class(),
            RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::StageActor)
        );
        assert_eq!(owner.persistence(), RuntimeOpaquePersistence::SnapshotOnly);
    }

    #[test]
    fn record_binding_path_uses_retained_pattern_order_not_domain_ordinal() {
        let mut builder = RuntimePlanBuilder::new();
        let admission = builder
            .admit_semantic_batch(
                [
                    RuntimePlanTypeSeed::new(
                        identity(1),
                        RuntimePlanTypeProjection::ProjectNominal {
                            nominal: nominal("game.Pair"),
                            layout: TypeLayoutHash::from_bytes([4; 32]),
                            arguments: Box::new([]),
                        },
                    ),
                    RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::Bool),
                ],
                [RuntimeLocalDeclarationSeed::new(identity(2))],
                [RuntimeNominalRecordDomainSeed::new(
                    identity(1),
                    [
                        RuntimeNominalRecordDomainFieldSeed::new("first", identity(2)),
                        RuntimeNominalRecordDomainFieldSeed::new("second", identity(2)),
                    ],
                )],
                [],
            )
            .expect("record graph");
        let pattern = RuntimePatternSeed::new(
            identity(1),
            RuntimePatternSeedKind::Record {
                fields: Box::new([RuntimeRecordPatternFieldSeed::new(
                    RuntimeRecordFieldSeedId::from_zero_based(1),
                    RuntimePatternSeed::new(
                        identity(2),
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: admission.local_ids()[0].clone(),
                        },
                    ),
                )]),
                rest: RuntimePatternRestSeed::Ignore,
            },
        );

        let admitted = builder
            .lower_pattern_seed(pattern)
            .expect("admitted record pattern");
        let RuntimePatternKind::Record { fields, .. } = admitted.kind() else {
            panic!("record pattern kind");
        };
        assert_eq!(fields[0].field().zero_based(), 1);
        let RuntimePatternKind::Bind { binding, .. } = fields[0].pattern().kind() else {
            panic!("field bind pattern");
        };
        assert_eq!(
            binding.path().steps(),
            [RuntimePatternBindingStep::RecordField(0)]
        );
    }

    #[test]
    fn binding_path_accepts_depth_64_and_rejects_depth_65() {
        let mut builder = RuntimePlanBuilder::new();
        let owner = nominal("game.Recursive");
        let admission = builder
            .admit_semantic_batch(
                [RuntimePlanTypeSeed::new(
                    identity(1),
                    RuntimePlanTypeProjection::ProjectNominal {
                        nominal: owner.clone(),
                        layout: TypeLayoutHash::from_bytes([8; 32]),
                        arguments: Box::new([]),
                    },
                )],
                [RuntimeLocalDeclarationSeed::new(identity(1))],
                [],
                [RuntimeVariantDomainSeed::new(
                    identity(1),
                    owner,
                    [RuntimeVariantCaseSeed::new("Next", Some(identity(1)))],
                )],
            )
            .expect("recursive variant domain");
        let local = admission.local_ids()[0].clone();

        assert!(
            builder
                .lower_pattern_seed(nested_variant(64, local.clone()))
                .is_ok()
        );
        assert_eq!(
            builder.lower_pattern_seed(nested_variant(65, local)),
            Err(RuntimePlanBuildError::PatternBindingPath(
                RuntimePatternBindingPathError::TooDeep {
                    actual: 65,
                    maximum: 64,
                }
            ))
        );
    }

    fn nested_variant(depth: usize, local: RuntimeLocalSeedId) -> RuntimePatternSeed {
        let mut pattern = RuntimePatternSeed::new(
            identity(1),
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local,
            },
        );
        for _ in 0..depth {
            pattern = RuntimePatternSeed::new(
                identity(1),
                RuntimePatternSeedKind::Variant {
                    ordinal: 0,
                    payload: Some(Box::new(pattern)),
                },
            );
        }
        pattern
    }
}
