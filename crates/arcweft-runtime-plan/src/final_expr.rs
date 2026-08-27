//! Runtime-expression seed projection from accepted final HIR.

use std::collections::BTreeMap;

use arcweft_core::entry::RuntimeCallableId;
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    RuntimeAgentExprSeed, RuntimeCallArgumentSeed, RuntimeExprMatchArmSeed, RuntimeExprSeed,
    RuntimeExprSeedKind, RuntimeFieldProjectionSeed, RuntimeFlowOpSeed, RuntimeFunctionSiteSeedId,
    RuntimeHostArgumentSeed, RuntimeHostCallTargetSeed, RuntimeLocalSeedId,
    RuntimeNominalRecordFieldSeed, RuntimePatternSeed, RuntimePatternSeedKind,
    RuntimePureHelperSeedId, RuntimeRecordFieldSeedId, RuntimeTraitMethodSeedId,
};
use arcweft_core::task::NamedHostArg;
use arcweft_core::value::{
    RuntimeAgentCompareOp, RuntimeBinaryOp, RuntimeCallArgumentMode, RuntimeCallTarget,
    RuntimeUnaryOp, RuntimeValue,
};
use arcweft_lang_hir::expr::{HirBinaryOp, HirExprKind, HirUnaryOp};
use arcweft_lang_hir::identity::{ExprId, LocalId, StmtId};
use arcweft_lang_hir::item::HirFunctionBody;
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_hir::stmt::HirStmtKind;
use arcweft_lang_hir::symbol::ImplMethodDeclarationId;

use crate::agent::RuntimeAgentIntrinsic;
use crate::final_pattern::{FinalPatternLowerer, project_entity_reference};
use crate::flow::TryLocalSeeds;
use crate::semantic_facts::{
    RuntimeNormalizedType, RuntimePlanSemanticFacts, RuntimeRecordExpressionSource,
    RuntimeReductionConstructor, RuntimeResolvedCall, RuntimeResolvedCallDispatch,
    RuntimeResolvedCallOperand, RuntimeResolvedCallOperandBinding,
    RuntimeResolvedCallOperandOrigin, RuntimeResolvedCallOperandProjection,
    RuntimeResolvedCallOperandSource, RuntimeResolvedSelect, RuntimeResolvedStaticCallTarget,
    RuntimeResolvedValue, RuntimeResolvedVariant, RuntimeTryBoundaryOwner, RuntimeTryCarrierFact,
    RuntimeTypeShape,
};

pub(crate) struct FinalExprLowerer<'hir> {
    module: &'hir HirModule,
    facts: &'hir RuntimePlanSemanticFacts,
    locals: &'hir BTreeMap<LocalId, RuntimeLocalSeedId>,
    pure_helpers: &'hir BTreeMap<RuntimeCallableId, RuntimePureHelperSeedId>,
    trait_methods: &'hir BTreeMap<ImplMethodDeclarationId, RuntimeTraitMethodSeedId>,
    function_sites: &'hir BTreeMap<ExprId, RuntimeFunctionSiteSeedId>,
    pipe_locals: &'hir BTreeMap<ExprId, RuntimeLocalSeedId>,
    try_locals: &'hir BTreeMap<ExprId, TryLocalSeeds>,
    overrides: BTreeMap<ExprId, RuntimeExprSeed>,
}

#[derive(Clone)]
enum PureTryContinuation {
    Return,
    Try {
        owner: ExprId,
        outer: Box<Self>,
    },
    Compose {
        owner: ExprId,
        child: ExprId,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
        outer: Box<Self>,
    },
    LetBlock {
        binding: RuntimeLocalSeedId,
        statements: Box<[StmtId]>,
        tail: ExprId,
        outer: Box<Self>,
    },
    AssignBlock {
        statement: StmtId,
        statements: Box<[StmtId]>,
        tail: ExprId,
        outer: Box<Self>,
    },
    WrapCarrier {
        owner: ExprId,
        outer: Box<Self>,
    },
    IfCondition {
        then_branch: ExprId,
        else_branch: ExprId,
        outer: Box<Self>,
    },
    MatchScrutinee {
        owner: ExprId,
        outer: Box<Self>,
    },
    IfLetScrutinee {
        owner: ExprId,
        outer: Box<Self>,
    },
    ShortCircuit {
        operator: HirBinaryOp,
        right: ExprId,
        outer: Box<Self>,
    },
    PipeLeft {
        owner: ExprId,
        outer: Box<Self>,
    },
    WrapSuccess {
        boundary: RuntimeSemanticTypeId,
    },
}

impl PureTryContinuation {
    fn after_carrier(self, target: ExprId) -> Option<Self> {
        match self {
            Self::WrapCarrier { owner, outer } if owner == target => Some(*outer),
            Self::Try { outer, .. }
            | Self::Compose { outer, .. }
            | Self::LetBlock { outer, .. }
            | Self::AssignBlock { outer, .. }
            | Self::WrapCarrier { outer, .. }
            | Self::IfCondition { outer, .. }
            | Self::MatchScrutinee { outer, .. }
            | Self::IfLetScrutinee { outer, .. }
            | Self::ShortCircuit { outer, .. }
            | Self::PipeLeft { outer, .. } => outer.after_carrier(target),
            Self::Return | Self::WrapSuccess { .. } => None,
        }
    }
}

impl<'hir> FinalExprLowerer<'hir> {
    pub(crate) const fn new(
        module: &'hir HirModule,
        facts: &'hir RuntimePlanSemanticFacts,
        locals: &'hir BTreeMap<LocalId, RuntimeLocalSeedId>,
        pure_helpers: &'hir BTreeMap<RuntimeCallableId, RuntimePureHelperSeedId>,
        trait_methods: &'hir BTreeMap<ImplMethodDeclarationId, RuntimeTraitMethodSeedId>,
        function_sites: &'hir BTreeMap<ExprId, RuntimeFunctionSiteSeedId>,
        control_locals: (
            &'hir BTreeMap<ExprId, RuntimeLocalSeedId>,
            &'hir BTreeMap<ExprId, TryLocalSeeds>,
        ),
    ) -> Self {
        let (pipe_locals, try_locals) = control_locals;
        Self {
            module,
            facts,
            locals,
            pure_helpers,
            trait_methods,
            function_sites,
            pipe_locals,
            try_locals,
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
        _call: &arcweft_lang_hir::expr::HirCallExpr,
    ) -> Result<Option<RuntimeHostCallTargetSeed>, String> {
        let call = self
            .facts
            .call(id)
            .ok_or_else(|| format!("checked call fact is missing for expression {id:?}"))?;
        let RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Host(host)) =
            call.dispatch()
        else {
            return Ok(None);
        };
        let args = self
            .lower_call_operands(id, call)?
            .into_iter()
            .zip(call.operands())
            .map(|((value, _), operand)| {
                if matches!(operand.origin(), RuntimeResolvedCallOperandOrigin::Receiver) {
                    return Err(format!(
                        "host call {id:?} cannot project a receiver argument"
                    ));
                }
                match (operand.projection(), operand.binding()) {
                    (
                        RuntimeResolvedCallOperandProjection::Scalar,
                        RuntimeResolvedCallOperandBinding::Positional,
                    ) => Ok(RuntimeHostArgumentSeed::Positional(value)),
                    (
                        RuntimeResolvedCallOperandProjection::Scalar,
                        RuntimeResolvedCallOperandBinding::Named(name),
                    ) => Ok(RuntimeHostArgumentSeed::Named(NamedHostArg {
                        name: name.clone(),
                        value,
                    })),
                    (
                        RuntimeResolvedCallOperandProjection::SpreadContainer(_),
                        RuntimeResolvedCallOperandBinding::Positional,
                    ) => Ok(RuntimeHostArgumentSeed::Spread(value)),
                    _ => Err(format!(
                        "host call {id:?} has an unsupported operand binding/projection"
                    )),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(RuntimeHostCallTargetSeed {
            public_id: host.public_id().to_owned(),
            capability: host.capability().to_owned(),
            operation: host.operation().to_owned(),
            contract: host.contract(),
            args,
            result: self.expression_type(id)?,
            mode: host.mode(),
            deterministic: host.deterministic(),
        }))
    }

    pub(crate) fn lower(&self, id: ExprId) -> Result<RuntimeExprSeed, String> {
        if let Some(value) = self.overrides.get(&id) {
            return Ok(value.clone());
        }
        if self.facts.implicit_callable(id).is_some() {
            return Ok(RuntimeExprSeed::new(
                self.expression_type(id)?,
                RuntimeExprSeedKind::Function(self.function_sites.get(&id).cloned().ok_or_else(
                    || format!("builder-issued function site seed is missing for {id:?}"),
                )?),
            ));
        }
        self.lower_body(id)
    }

    pub(crate) fn lower_function_site_body(
        &self,
        site_owner: ExprId,
        body: ExprId,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<RuntimeExprSeed, String> {
        let lowerer = self.clone_with_overrides(overrides);
        if let Some(tried) = lowerer
            .facts
            .tried(body)
            .filter(|tried| tried.boundary() == RuntimeTryBoundaryOwner::FunctionSite(site_owner))
        {
            lowerer.lower_with_try_continuation(
                body,
                PureTryContinuation::WrapSuccess {
                    boundary: tried.boundary_type().identity(),
                },
            )
        } else if lowerer.contains_executable_try(body)? {
            lowerer.lower_with_try_continuation(body, PureTryContinuation::Return)
        } else {
            lowerer.lower_body(body)
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one closed final-HIR expression projection"
    )]
    fn lower_body(&self, id: ExprId) -> Result<RuntimeExprSeed, String> {
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
            HirExprKind::ShortVariant(_)
                if matches!(
                    self.facts.value(id),
                    Some(RuntimeResolvedValue::CharacterLook { .. })
                ) =>
            {
                RuntimeExprSeedKind::EntityRef(self.entity_reference(id)?)
            }
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
            HirExprKind::RecordLiteral(_) | HirExprKind::Record(_) => {
                RuntimeExprSeedKind::NominalRecord(
                    self.lower_nominal_fields(id)?.into_boxed_slice(),
                )
            }
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
            HirExprKind::Pipe(pipe) => {
                let binding = self.pipe_locals.get(&id).cloned().ok_or_else(|| {
                    format!("builder-issued once-only pipe local is missing for {id:?}")
                })?;
                let value = RuntimeExprSeed::new(
                    self.expression_type(pipe.left())?,
                    RuntimeExprSeedKind::Local(binding.clone()),
                );
                let overrides = self
                    .facts
                    .pipe(id)
                    .ok_or_else(|| format!("checked pipe fact is missing for {id:?}"))?
                    .placeholders()
                    .iter()
                    .map(|placeholder| (*placeholder, value.clone()))
                    .collect();
                let body = self.clone_with_overrides(overrides).lower(pipe.right())?;
                return Ok(RuntimeExprSeed::new(
                    self.expression_type(id)?,
                    RuntimeExprSeedKind::Let {
                        binding,
                        expr: Box::new(self.lower(pipe.left())?),
                        body: Box::new(body),
                    },
                ));
            }
            HirExprKind::Placeholder(_)
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

    fn clone_with_overrides(&self, overrides: BTreeMap<ExprId, RuntimeExprSeed>) -> Self {
        let mut merged = self.overrides.clone();
        merged.extend(overrides);
        Self {
            module: self.module,
            facts: self.facts,
            locals: self.locals,
            pure_helpers: self.pure_helpers,
            trait_methods: self.trait_methods,
            function_sites: self.function_sites,
            pipe_locals: self.pipe_locals,
            try_locals: self.try_locals,
            overrides: merged,
        }
    }

    fn lower_try_continuation(
        &self,
        owner: ExprId,
        value: RuntimeExprSeed,
        outer: PureTryContinuation,
    ) -> Result<RuntimeExprSeed, String> {
        let tried = self
            .facts
            .tried(owner)
            .ok_or_else(|| format!("checked Try fact is missing for {owner:?}"))?;
        let locals = self
            .try_locals
            .get(&owner)
            .ok_or_else(|| format!("admitted Try locals are missing for {owner:?}"))?;
        let success_value = RuntimeExprSeed::new(
            tried.carrier().success().identity(),
            RuntimeExprSeedKind::Local(locals.success.clone()),
        );
        let failure_continuation = match tried.boundary() {
            RuntimeTryBoundaryOwner::CarrierBlock(boundary) => Some(
                outer
                    .clone()
                    .after_carrier(boundary)
                    .ok_or_else(|| format!("Try {owner:?} has no active carrier continuation"))?,
            ),
            RuntimeTryBoundaryOwner::Infallible
            | RuntimeTryBoundaryOwner::FunctionSite(_)
            | RuntimeTryBoundaryOwner::Callable(_) => None,
        };
        let success = self.apply_try_continuation(success_value, outer)?;
        let success_pattern = RuntimePatternSeed::new(
            tried.carrier_type().identity(),
            RuntimePatternSeedKind::Variant {
                ordinal: 0,
                payload: Some(Box::new(RuntimePatternSeed::new(
                    tried.carrier().success().identity(),
                    RuntimePatternSeedKind::Bind {
                        local: locals.success.clone(),
                        mutable: false,
                    },
                ))),
            },
        );
        let (failure_pattern, failure_payload) = match tried.carrier() {
            RuntimeTryCarrierFact::Result { residual, .. } => {
                let local = locals.residual.clone().ok_or_else(|| {
                    format!("Result Try {owner:?} has no admitted residual local")
                })?;
                (
                    RuntimePatternSeed::new(
                        tried.carrier_type().identity(),
                        RuntimePatternSeedKind::Variant {
                            ordinal: 1,
                            payload: Some(Box::new(RuntimePatternSeed::new(
                                residual.identity(),
                                RuntimePatternSeedKind::Bind {
                                    local: local.clone(),
                                    mutable: false,
                                },
                            ))),
                        },
                    ),
                    Some(Box::new(RuntimeExprSeed::new(
                        residual.identity(),
                        RuntimeExprSeedKind::Local(local),
                    ))),
                )
            }
            RuntimeTryCarrierFact::Option { .. } => (
                RuntimePatternSeed::new(
                    tried.carrier_type().identity(),
                    RuntimePatternSeedKind::Variant {
                        ordinal: 1,
                        payload: None,
                    },
                ),
                None,
            ),
        };
        let failure = RuntimeExprSeed::new(
            tried.boundary_type().identity(),
            RuntimeExprSeedKind::Variant {
                ordinal: 1,
                payload: failure_payload,
            },
        );
        let failure = match failure_continuation {
            Some(continuation) => self.apply_try_continuation(failure, continuation)?,
            None => failure,
        };
        if failure.ty() != success.ty() {
            return Err(format!(
                "Try {owner:?} branches do not produce one continuation type"
            ));
        }
        Ok(RuntimeExprSeed::new(
            success.ty(),
            RuntimeExprSeedKind::Match {
                scrutinee: Box::new(value),
                arms: vec![
                    RuntimeExprMatchArmSeed::new(success_pattern, None, success),
                    RuntimeExprMatchArmSeed::new(failure_pattern, None, failure),
                ]
                .into_boxed_slice(),
            },
        ))
    }

    pub(crate) fn lower_function_body(
        &self,
        body: &HirFunctionBody,
    ) -> Result<RuntimeExprSeed, String> {
        match body {
            HirFunctionBody::Block {
                statements, tail, ..
            } => self.lower_function_block(statements, *tail, PureTryContinuation::Return),
            HirFunctionBody::Error(expression) => Err(format!(
                "recovered ordinary-function body {expression:?} cannot enter runtime lowering"
            )),
        }
    }

    fn lower_function_block(
        &self,
        statements: &[StmtId],
        tail: ExprId,
        continuation: PureTryContinuation,
    ) -> Result<RuntimeExprSeed, String> {
        let Some((statement_id, remaining)) = statements.split_first() else {
            return self.lower_with_try_continuation(tail, continuation);
        };
        let statement = self.module.resolve_stmt(*statement_id).map_err(|error| {
            format!("cannot resolve function statement {statement_id:?}: {error}")
        })?;
        if statement.is_poisoned() {
            return Err("recovered function statement is not executable".to_owned());
        }
        match statement.kind() {
            HirStmtKind::Let {
                pattern,
                initializer,
                ..
            } => self.lower_with_try_continuation(
                *initializer,
                PureTryContinuation::LetBlock {
                    binding: self.simple_binding(*pattern)?,
                    statements: remaining.into(),
                    tail,
                    outer: Box::new(continuation),
                },
            ),
            HirStmtKind::Assign { value, .. } => self.lower_with_try_continuation(
                *value,
                PureTryContinuation::AssignBlock {
                    statement: *statement_id,
                    statements: remaining.into(),
                    tail,
                    outer: Box::new(continuation),
                },
            ),
            other => Err(format!(
                "statement {other:?} cannot be embedded in a pure runtime expression block"
            )),
        }
    }

    fn contains_executable_try(&self, owner: ExprId) -> Result<bool, String> {
        if self.facts.implicit_callable(owner).is_some() {
            return Ok(false);
        }
        if self.facts.tried(owner).is_some() {
            return Ok(true);
        }
        let expression = self.resolve_expression(owner)?;
        if matches!(expression.kind(), HirExprKind::Closure(_)) {
            return Ok(false);
        }
        match expression.kind() {
            HirExprKind::Block(block) => {
                return self.block_contains_executable_try(block.statements(), block.tail());
            }
            HirExprKind::ComputationBlock(block) => {
                return self.block_contains_executable_try(block.statements(), block.tail());
            }
            HirExprKind::NamedBlock(block) => {
                return self.block_contains_executable_try(block.statements(), block.tail());
            }
            _ => {}
        }
        for child in expression.kind().direct_expression_children() {
            if self.contains_executable_try(child)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn block_contains_executable_try(
        &self,
        statements: &[StmtId],
        tail: ExprId,
    ) -> Result<bool, String> {
        for statement in statements {
            let statement = self.module.resolve_stmt(*statement).map_err(|error| {
                format!("cannot resolve pure block statement {statement:?}: {error}")
            })?;
            let child = match statement.kind() {
                HirStmtKind::Let { initializer, .. } => *initializer,
                HirStmtKind::Assign { value, .. } => *value,
                _ => continue,
            };
            if self.contains_executable_try(child)? {
                return Ok(true);
            }
        }
        self.contains_executable_try(tail)
    }

    fn lower_with_try_continuation(
        &self,
        owner: ExprId,
        continuation: PureTryContinuation,
    ) -> Result<RuntimeExprSeed, String> {
        let expression = self.resolve_expression(owner)?;
        if let HirExprKind::Try(operation) = expression.kind() {
            return self.lower_with_try_continuation(
                operation.operand(),
                PureTryContinuation::Try {
                    owner,
                    outer: Box::new(continuation),
                },
            );
        }
        if let Some(lowered) =
            self.lower_pipe_try_continuation(owner, expression.kind(), continuation.clone())?
        {
            return Ok(lowered);
        }
        if let Some(lowered) =
            self.lower_block_try_continuation(owner, expression.kind(), continuation.clone())
        {
            return lowered;
        }
        if let HirExprKind::If(branch) = expression.kind() {
            return self.lower_if_continuation(owner, branch, continuation);
        }
        if let HirExprKind::Match(matched) = expression.kind() {
            if self.contains_executable_try(matched.scrutinee())? {
                return self.lower_with_try_continuation(
                    matched.scrutinee(),
                    PureTryContinuation::MatchScrutinee {
                        owner,
                        outer: Box::new(continuation),
                    },
                );
            }
            return self.lower_match_continuation(
                matched,
                self.lower(matched.scrutinee())?,
                &continuation,
            );
        }
        if let HirExprKind::IfLet(branch) = expression.kind() {
            if self.contains_executable_try(branch.scrutinee())? {
                return self.lower_with_try_continuation(
                    branch.scrutinee(),
                    PureTryContinuation::IfLetScrutinee {
                        owner,
                        outer: Box::new(continuation),
                    },
                );
            }
            return self.lower_if_let_continuation(
                branch,
                self.lower(branch.scrutinee())?,
                continuation,
            );
        }
        if let HirExprKind::Binary(binary) = expression.kind()
            && matches!(
                binary.operator(),
                HirBinaryOp::And | HirBinaryOp::Or | HirBinaryOp::Implies
            )
        {
            if self.contains_executable_try(binary.left())? {
                return self.lower_with_try_continuation(
                    binary.left(),
                    PureTryContinuation::ShortCircuit {
                        operator: binary.operator(),
                        right: binary.right(),
                        outer: Box::new(continuation),
                    },
                );
            }
            return self.lower_short_circuit(
                self.lower(binary.left())?,
                binary.operator(),
                binary.right(),
                continuation,
            );
        }
        for child in expression.kind().direct_expression_children() {
            if !self.overrides.contains_key(&child) && self.contains_executable_try(child)? {
                return self.lower_with_try_continuation(
                    child,
                    PureTryContinuation::Compose {
                        owner,
                        child,
                        overrides: self.overrides.clone(),
                        outer: Box::new(continuation),
                    },
                );
            }
        }
        self.apply_try_continuation(self.lower(owner)?, continuation)
    }

    fn lower_block_try_continuation(
        &self,
        owner: ExprId,
        kind: &HirExprKind,
        continuation: PureTryContinuation,
    ) -> Option<Result<RuntimeExprSeed, String>> {
        match kind {
            HirExprKind::Block(block) => {
                Some(self.lower_function_block(block.statements(), block.tail(), continuation))
            }
            HirExprKind::NamedBlock(block) => {
                Some(self.lower_function_block(block.statements(), block.tail(), continuation))
            }
            HirExprKind::ComputationBlock(block)
                if matches!(
                    block.kind(),
                    arcweft_lang_hir::expr::HirComputationBlockKind::Result
                        | arcweft_lang_hir::expr::HirComputationBlockKind::Option
                ) =>
            {
                Some(self.lower_function_block(
                    block.statements(),
                    block.tail(),
                    PureTryContinuation::WrapCarrier {
                        owner,
                        outer: Box::new(continuation),
                    },
                ))
            }
            _ => None,
        }
    }

    fn lower_pipe_try_continuation(
        &self,
        owner: ExprId,
        kind: &HirExprKind,
        continuation: PureTryContinuation,
    ) -> Result<Option<RuntimeExprSeed>, String> {
        let HirExprKind::Pipe(pipe) = kind else {
            return Ok(None);
        };
        if self.contains_executable_try(pipe.left())? {
            return self
                .lower_with_try_continuation(
                    pipe.left(),
                    PureTryContinuation::PipeLeft {
                        owner,
                        outer: Box::new(continuation),
                    },
                )
                .map(Some);
        }
        self.lower_pipe_continuation(owner, pipe, self.lower(pipe.left())?, continuation)
            .map(Some)
    }

    fn apply_try_continuation(
        &self,
        value: RuntimeExprSeed,
        continuation: PureTryContinuation,
    ) -> Result<RuntimeExprSeed, String> {
        match continuation {
            PureTryContinuation::Return => Ok(value),
            PureTryContinuation::Compose {
                owner,
                child,
                mut overrides,
                outer,
            } => {
                overrides.insert(child, value);
                self.clone_with_overrides(overrides)
                    .lower_with_try_continuation(owner, *outer)
            }
            PureTryContinuation::Try { owner, outer } => {
                self.lower_try_continuation(owner, value, *outer)
            }
            PureTryContinuation::LetBlock {
                binding,
                statements,
                tail,
                outer,
            } => {
                let body = self.lower_function_block(&statements, tail, *outer)?;
                Ok(RuntimeExprSeed::new(
                    body.ty(),
                    RuntimeExprSeedKind::Let {
                        binding,
                        expr: Box::new(value),
                        body: Box::new(body),
                    },
                ))
            }
            PureTryContinuation::AssignBlock {
                statement,
                statements,
                tail,
                outer,
            } => {
                let body = self.lower_function_block(&statements, tail, *outer)?;
                self.lower_assignment_value(statement, value, body)
            }
            PureTryContinuation::WrapCarrier { owner, outer } => {
                let boundary = self
                    .facts
                    .expression_type(owner)
                    .ok_or_else(|| format!("carrier block {owner:?} has no checked result type"))?;
                let wrapped = wrap_success(boundary.identity(), value);
                self.apply_try_continuation(wrapped, *outer)
            }
            PureTryContinuation::IfCondition {
                then_branch,
                else_branch,
                outer,
            } => {
                let then_expr = self.lower_with_try_continuation(then_branch, (*outer).clone())?;
                let else_expr = self.lower_with_try_continuation(else_branch, *outer)?;
                if then_expr.ty() != else_expr.ty() {
                    return Err("If Try branches do not produce one continuation type".to_owned());
                }
                Ok(RuntimeExprSeed::new(
                    then_expr.ty(),
                    RuntimeExprSeedKind::If {
                        condition: Box::new(value),
                        then_expr: Box::new(then_expr),
                        else_expr: Box::new(else_expr),
                    },
                ))
            }
            PureTryContinuation::MatchScrutinee { owner, outer } => {
                let expression = self
                    .module
                    .resolve_expr(owner)
                    .map_err(|error| format!("cannot resolve Match {owner:?}: {error}"))?;
                let HirExprKind::Match(matched) = expression.kind() else {
                    return Err(format!("Match continuation owner {owner:?} changed family"));
                };
                self.lower_match_continuation(matched, value, &outer)
            }
            PureTryContinuation::IfLetScrutinee { owner, outer } => {
                let expression = self
                    .module
                    .resolve_expr(owner)
                    .map_err(|error| format!("cannot resolve IfLet {owner:?}: {error}"))?;
                let HirExprKind::IfLet(branch) = expression.kind() else {
                    return Err(format!("IfLet continuation owner {owner:?} changed family"));
                };
                self.lower_if_let_continuation(branch, value, *outer)
            }
            PureTryContinuation::ShortCircuit {
                operator,
                right,
                outer,
            } => self.lower_short_circuit(value, operator, right, *outer),
            PureTryContinuation::PipeLeft { owner, outer } => {
                self.apply_pipe_left_continuation(owner, value, *outer)
            }
            PureTryContinuation::WrapSuccess { boundary } => Ok(wrap_success(boundary, value)),
        }
    }

    fn lower_pipe_continuation(
        &self,
        owner: ExprId,
        pipe: &arcweft_lang_hir::expr::HirPipeExpr,
        left: RuntimeExprSeed,
        continuation: PureTryContinuation,
    ) -> Result<RuntimeExprSeed, String> {
        let binding = self.pipe_locals.get(&owner).cloned().ok_or_else(|| {
            format!("builder-issued once-only pipe local is missing for {owner:?}")
        })?;
        let local = RuntimeExprSeed::new(
            self.expression_type(pipe.left())?,
            RuntimeExprSeedKind::Local(binding.clone()),
        );
        let overrides = self
            .facts
            .pipe(owner)
            .ok_or_else(|| format!("checked pipe fact is missing for {owner:?}"))?
            .placeholders()
            .iter()
            .map(|placeholder| (*placeholder, local.clone()))
            .collect();
        let body = self
            .clone_with_overrides(overrides)
            .lower_with_try_continuation(pipe.right(), continuation)?;
        Ok(RuntimeExprSeed::new(
            body.ty(),
            RuntimeExprSeedKind::Let {
                binding,
                expr: Box::new(left),
                body: Box::new(body),
            },
        ))
    }

    fn apply_pipe_left_continuation(
        &self,
        owner: ExprId,
        value: RuntimeExprSeed,
        continuation: PureTryContinuation,
    ) -> Result<RuntimeExprSeed, String> {
        let expression = self.resolve_expression(owner)?;
        let HirExprKind::Pipe(pipe) = expression.kind() else {
            return Err(format!("Pipe continuation owner {owner:?} changed family"));
        };
        self.lower_pipe_continuation(owner, pipe, value, continuation)
    }

    fn lower_if_continuation(
        &self,
        owner: ExprId,
        branch: &arcweft_lang_hir::expr::HirIfExpr,
        continuation: PureTryContinuation,
    ) -> Result<RuntimeExprSeed, String> {
        if self.contains_executable_try(branch.condition())? {
            return self.lower_with_try_continuation(
                branch.condition(),
                PureTryContinuation::IfCondition {
                    then_branch: branch.then_branch(),
                    else_branch: branch.else_branch(),
                    outer: Box::new(continuation),
                },
            );
        }
        let condition = self.lower(branch.condition())?;
        let then_expr =
            self.lower_with_try_continuation(branch.then_branch(), continuation.clone())?;
        let else_expr = self.lower_with_try_continuation(branch.else_branch(), continuation)?;
        if then_expr.ty() != else_expr.ty() {
            return Err(format!("If expression {owner:?} Try branches disagree"));
        }
        Ok(RuntimeExprSeed::new(
            then_expr.ty(),
            RuntimeExprSeedKind::If {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            },
        ))
    }

    fn lower_short_circuit(
        &self,
        condition: RuntimeExprSeed,
        operator: HirBinaryOp,
        right: ExprId,
        continuation: PureTryContinuation,
    ) -> Result<RuntimeExprSeed, String> {
        let bool_type = self
            .facts
            .expression_type(right)
            .ok_or_else(|| format!("short-circuit right operand {right:?} has no type"))?;
        let literal = |value| {
            RuntimeExprSeed::new(
                bool_type.identity(),
                RuntimeExprSeedKind::Value(RuntimeValue::Bool(value)),
            )
        };
        let evaluated = self.lower_with_try_continuation(right, continuation.clone())?;
        let skipped = self.apply_try_continuation(
            literal(matches!(operator, HirBinaryOp::Or | HirBinaryOp::Implies)),
            continuation,
        )?;
        let (then_expr, else_expr) = match operator {
            HirBinaryOp::And | HirBinaryOp::Implies => (evaluated, skipped),
            HirBinaryOp::Or => (skipped, evaluated),
            _ => return Err("non-short-circuit operator reached short-circuit lowering".to_owned()),
        };
        if then_expr.ty() != else_expr.ty() {
            return Err("short-circuit Try branches disagree".to_owned());
        }
        Ok(RuntimeExprSeed::new(
            then_expr.ty(),
            RuntimeExprSeedKind::If {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            },
        ))
    }

    fn lower_match_continuation(
        &self,
        matched: &arcweft_lang_hir::expr::HirMatchExpr,
        scrutinee: RuntimeExprSeed,
        continuation: &PureTryContinuation,
    ) -> Result<RuntimeExprSeed, String> {
        let mut arms = Vec::with_capacity(matched.arms().len());
        let mut ty = None;
        for arm in matched.arms() {
            if let Some(guard) = arm.guard()
                && self.contains_executable_try(guard)?
            {
                return Err("Try in a pure Match guard requires guard-local CPS".to_owned());
            }
            let value = self.lower_with_try_continuation(arm.value(), continuation.clone())?;
            if ty.is_some_and(|ty| ty != value.ty()) {
                return Err("pure Match Try arms do not produce one continuation type".to_owned());
            }
            ty = Some(value.ty());
            arms.push(RuntimeExprMatchArmSeed::new(
                self.pattern().lower(arm.pattern())?,
                arm.guard().map(|guard| self.lower(guard)).transpose()?,
                value,
            ));
        }
        let ty = ty.ok_or_else(|| "pure Match has no checked arms".to_owned())?;
        Ok(RuntimeExprSeed::new(
            ty,
            RuntimeExprSeedKind::Match {
                scrutinee: Box::new(scrutinee),
                arms: arms.into_boxed_slice(),
            },
        ))
    }

    fn lower_if_let_continuation(
        &self,
        branch: &arcweft_lang_hir::expr::HirIfLetExpr,
        scrutinee: RuntimeExprSeed,
        continuation: PureTryContinuation,
    ) -> Result<RuntimeExprSeed, String> {
        if let Some(guard) = branch.guard()
            && self.contains_executable_try(guard)?
        {
            return Err("Try in a pure IfLet guard requires guard-local CPS".to_owned());
        }
        let then_expr =
            self.lower_with_try_continuation(branch.then_branch(), continuation.clone())?;
        let else_expr = self.lower_with_try_continuation(branch.else_branch(), continuation)?;
        if then_expr.ty() != else_expr.ty() {
            return Err("pure IfLet Try branches do not produce one continuation type".to_owned());
        }
        Ok(RuntimeExprSeed::new(
            then_expr.ty(),
            RuntimeExprSeedKind::IfLet {
                pattern: self.pattern().lower(branch.pattern())?,
                expr: Box::new(scrutinee),
                guard: branch
                    .guard()
                    .map(|guard| self.lower(guard))
                    .transpose()?
                    .map(Box::new),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            },
        ))
    }

    pub(crate) fn lower_assignment(
        &self,
        statement: StmtId,
        value: ExprId,
        body: RuntimeExprSeed,
    ) -> Result<RuntimeExprSeed, String> {
        self.lower_assignment_value(statement, self.lower(value)?, body)
    }

    fn lower_assignment_value(
        &self,
        statement: StmtId,
        value: RuntimeExprSeed,
        body: RuntimeExprSeed,
    ) -> Result<RuntimeExprSeed, String> {
        let (local, owner, field) = self.assignment_parts(statement)?;
        Ok(RuntimeExprSeed::new(
            body.ty(),
            RuntimeExprSeedKind::AssignNominalField {
                base: self.local(local)?,
                owner,
                field,
                expr: Box::new(value),
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
            RuntimeRecordFieldSeedId::from_zero_based(assignment.field().zero_based()),
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
            | RuntimeResolvedValue::CharacterLook { .. }
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
        let operands = selected.operands();
        let lowered = self.lower_call_operands(id, selected)?;
        let values = lowered
            .iter()
            .map(|(value, mode)| (value.clone(), *mode))
            .collect::<Vec<_>>();
        let arguments = values
            .iter()
            .map(|(value, mode)| RuntimeCallArgumentSeed::new(value.clone(), *mode))
            .collect::<Vec<_>>();
        match selected.dispatch() {
            RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Intrinsic(
                intrinsic,
            )) => Ok(RuntimeExprSeedKind::Call {
                callee: RuntimeCallTarget::intrinsic(*intrinsic),
                args: arguments.into_boxed_slice(),
            }),
            RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Agent(
                intrinsic,
            )) => self.lower_agent_intrinsic(id, *intrinsic, &values, operands),
            RuntimeResolvedCallDispatch::Static(
                RuntimeResolvedStaticCallTarget::AgentProbeComparison(operation),
            ) => self.lower_agent_compare(id, *operation, &values),
            RuntimeResolvedCallDispatch::Static(
                RuntimeResolvedStaticCallTarget::AgentDiagnosticsHasError,
            ) => Ok(RuntimeExprSeedKind::Agent(
                RuntimeAgentExprSeed::PredicateDiagnosticsHasError {
                    diagnostics: Box::new(exact_one(id, &self.scalar_values(id, &values)?)?),
                },
            )),
            RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Declaration(
                callable,
            )) => Ok(RuntimeExprSeedKind::PureCall {
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
            RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Variant(
                variant,
            )) => {
                self.validate_variant_call_payload(id, operands, variant)?;
                let scalar_values = self.scalar_values(id, &values)?;
                let payload = match scalar_values.len() {
                    0 => None,
                    1 => Some(Box::new(scalar_values[0].clone())),
                    _ => Some(Box::new(RuntimeExprSeed::new(
                        self.expression_type(id)?,
                        RuntimeExprSeedKind::Tuple(scalar_values.into_boxed_slice()),
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
            RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Reduction(
                RuntimeReductionConstructor::Unchanged,
            )) => Ok(RuntimeExprSeedKind::ReductionUnchanged {
                state: Box::new(exact_one(id, &self.scalar_values(id, &values)?)?),
            }),
            RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Registered(
                registered,
            )) => Ok(RuntimeExprSeedKind::Call {
                callee: RuntimeCallTarget::callable(registered.clone()),
                args: arguments.into_boxed_slice(),
            }),
            RuntimeResolvedCallDispatch::Value { callee } => {
                if call.callee().value_expression() != Some(*callee) {
                    return Err(format!(
                        "value callee projection for {id:?} does not match final HIR"
                    ));
                }
                Ok(RuntimeExprSeedKind::Apply {
                    callee: Box::new(self.lower(*callee)?),
                    args: arguments.into_boxed_slice(),
                })
            }
            RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::TraitMethod {
                method,
                ..
            }) => {
                let receiver_index = operands
                    .iter()
                    .position(|operand| {
                        matches!(operand.origin(), RuntimeResolvedCallOperandOrigin::Receiver)
                    })
                    .ok_or_else(|| format!("trait call {id:?} has no receiver projection"))?;
                let scalar_values = self.scalar_values(id, &values)?;
                let receiver = scalar_values
                    .get(receiver_index)
                    .cloned()
                    .ok_or_else(|| format!("trait call {id:?} receiver is not a scalar operand"))?;
                let args = scalar_values
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, value)| (index != receiver_index).then_some(value))
                    .map(|value| {
                        RuntimeCallArgumentSeed::new(value, RuntimeCallArgumentMode::Value)
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                Ok(RuntimeExprSeedKind::TraitCall {
                    callable: self.trait_methods.get(method).cloned().ok_or_else(|| {
                        format!("builder-issued trait-method seed is missing for {method:?}")
                    })?,
                    receiver: Box::new(receiver),
                    args,
                })
            }
            RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Host(_)) => Err(
                format!("host call {id:?} is effectful and cannot enter pure expression lowering"),
            ),
            RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Line(_)) => Err(
                format!("line capability call {id:?} must be consumed by typed line-plan lowering"),
            ),
        }
    }

    fn lower_call_operands(
        &self,
        _id: ExprId,
        call: &RuntimeResolvedCall,
    ) -> Result<Vec<(RuntimeExprSeed, RuntimeCallArgumentMode)>, String> {
        call.operands()
            .iter()
            .map(|operand| {
                let value = match operand.source() {
                    RuntimeResolvedCallOperandSource::Expression(source) => {
                        let value = self.lower(source)?;
                        if value.ty() != operand.ty().identity() {
                            return Err(format!(
                                "call operand source {source:?} has type {:?}, expected {:?}",
                                value.ty(),
                                operand.ty().identity()
                            ));
                        }
                        value
                    }
                    RuntimeResolvedCallOperandSource::CompactNumericElement {
                        sequence,
                        ordinal,
                    } => {
                        let literal = self.facts.expression_literal(sequence).ok_or_else(|| {
                            format!(
                                "compact call operand sequence {sequence:?} has no checked literal"
                            )
                        })?;
                        let RuntimeValue::Seq(values) = literal else {
                            return Err(format!(
                                "compact call operand sequence {sequence:?} is not a checked sequence"
                            ));
                        };
                        let ordinal = usize::try_from(ordinal).map_err(|_| {
                            format!("compact call operand ordinal {ordinal} does not fit usize")
                        })?;
                        if ordinal >= values.len() {
                            return Err(format!(
                                "compact call operand ordinal {ordinal} is out of range for {sequence:?}"
                            ));
                        }
                        RuntimeExprSeed::new(operand.ty().identity(), RuntimeExprSeedKind::Value(values.value_at(ordinal)))
                    }
                };
                let mode = match operand.projection() {
                    RuntimeResolvedCallOperandProjection::Scalar => RuntimeCallArgumentMode::Value,
                    RuntimeResolvedCallOperandProjection::SpreadContainer(_) => {
                        RuntimeCallArgumentMode::Spread
                    }
                };
                Ok((value, mode))
            })
            .collect()
    }

    fn scalar_values(
        &self,
        id: ExprId,
        values: &[(RuntimeExprSeed, RuntimeCallArgumentMode)],
    ) -> Result<Vec<RuntimeExprSeed>, String> {
        if values
            .iter()
            .any(|(_, mode)| *mode != RuntimeCallArgumentMode::Value)
        {
            return Err(format!(
                "typed call {id:?} requires non-spread runtime operands"
            ));
        }
        Ok(values.iter().map(|(value, _)| value.clone()).collect())
    }

    /*
     * Call operand lowering is intentionally driven only by the sealed
     * runtime carrier above.  The old authored-ordinal recovery helpers were
     * deleted so runtime projection cannot re-search HIR arguments.
     */

    fn lower_select(&self, id: ExprId, target_id: ExprId) -> Result<RuntimeExprSeedKind, String> {
        let target = Box::new(self.lower(target_id)?);
        match self
            .facts
            .select(id)
            .ok_or_else(|| format!("checked member fact is missing for expression {id:?}"))?
        {
            RuntimeResolvedSelect::Method => Err(format!(
                "bound method at {id:?} cannot execute outside its checked Call"
            )),
            RuntimeResolvedSelect::AgentField { field } => Ok(RuntimeExprSeedKind::Field {
                target,
                field: RuntimeFieldProjectionSeed::Agent(*field),
            }),
            RuntimeResolvedSelect::ProgressField { field } => Ok(RuntimeExprSeedKind::Field {
                target,
                field: RuntimeFieldProjectionSeed::Progress(*field),
            }),
            RuntimeResolvedSelect::Field { owner, field } => Ok(RuntimeExprSeedKind::Field {
                target,
                field: RuntimeFieldProjectionSeed::Nominal {
                    owner: *owner,
                    field: RuntimeRecordFieldSeedId::from_zero_based(field.zero_based()),
                },
            }),
            RuntimeResolvedSelect::OpaqueRecord {
                owner,
                producer,
                field,
                field_type,
            } => Ok(RuntimeExprSeedKind::Field {
                target,
                field: RuntimeFieldProjectionSeed::OpaqueRecord {
                    owner: *owner,
                    producer: producer.clone(),
                    field: RuntimeRecordFieldSeedId::from_zero_based(field.zero_based()),
                    field_type: *field_type,
                },
            }),
        }
    }

    fn lower_nominal_fields(
        &self,
        id: ExprId,
    ) -> Result<Vec<RuntimeNominalRecordFieldSeed>, String> {
        let record = self.facts.nominal_record(id).ok_or_else(|| {
            format!(
                "nominal record expression {id:?} requires a typed runtime nominal-expression owner"
            )
        })?;
        record
            .fields()
            .iter()
            .map(|field| {
                let value = match field.source() {
                    RuntimeRecordExpressionSource::Expression(value) => self.lower(value)?,
                    RuntimeRecordExpressionSource::Binding(local) => RuntimeExprSeed::new(
                        self.local_type(local)?,
                        RuntimeExprSeedKind::Local(self.local(local)?),
                    ),
                };
                Ok(RuntimeNominalRecordFieldSeed::new(
                    RuntimeRecordFieldSeedId::from_zero_based(field.field().zero_based()),
                    value,
                ))
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
        intrinsic: RuntimeAgentIntrinsic,
        values: &[(RuntimeExprSeed, RuntimeCallArgumentMode)],
        operands: &[RuntimeResolvedCallOperand],
    ) -> Result<RuntimeExprSeedKind, String> {
        if let Some(operation) = intrinsic.host_operation() {
            return Err(format!(
                "Agent host call {operation} at {id:?} cannot enter pure expression lowering"
            ));
        }
        let values = self.scalar_values(id, values)?;
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
                choice: self.choice_target(operands, id)?,
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
        op: RuntimeAgentCompareOp,
        values: &[(RuntimeExprSeed, RuntimeCallArgumentMode)],
    ) -> Result<RuntimeExprSeedKind, String> {
        let mut values = self.scalar_values(id, values)?;
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
        operands: &[RuntimeResolvedCallOperand],
        id: ExprId,
    ) -> Result<arcweft_core::entry::RuntimeCommandTargetId, String> {
        let operand = operands
            .first()
            .ok_or_else(|| format!("choice_action at {id:?} has no accepted operand"))?;
        let RuntimeResolvedCallOperandSource::Expression(expression) = operand.source() else {
            return Err(format!(
                "choice_action at {id:?} requires an expression operand"
            ));
        };
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
            Some(RuntimeResolvedValue::CharacterLook { character, look }) => {
                Ok(arcweft_core::value::RuntimeEntityReference::CharacterLook {
                    character: character.clone(),
                    look: look.clone(),
                })
            }
            _ => Err(format!(
                "Agent semantic identity at {id:?} is not an exact accepted entity"
            )),
        }
    }

    fn validate_variant_call_payload(
        &self,
        id: ExprId,
        operands: &[RuntimeResolvedCallOperand],
        variant: &RuntimeResolvedVariant,
    ) -> Result<(), String> {
        let types = operands
            .iter()
            .map(|operand| {
                if matches!(operand.origin(), RuntimeResolvedCallOperandOrigin::Receiver) {
                    return Err(format!(
                        "variant constructor {id:?} has an invalid receiver"
                    ));
                }
                if !matches!(
                    operand.projection(),
                    RuntimeResolvedCallOperandProjection::Scalar
                ) {
                    return Err(format!(
                        "variant constructor {id:?} cannot consume a spread container"
                    ));
                }
                Ok(operand.ty())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let payload = variant
            .selected_payload_type()
            .map_err(|error| error.to_string())?;
        (payload.is_some() == !operands.is_empty()
            && variant_payload_accepts_argument_types(payload, &types))
        .then_some(())
        .ok_or_else(|| {
            format!(
                "variant constructor at {id:?} does not match its selected normalized payload type"
            )
        })
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

    fn resolve_expression(
        &self,
        owner: ExprId,
    ) -> Result<&arcweft_lang_hir::expr::HirExpr, String> {
        self.module
            .resolve_expr(owner)
            .map_err(|error| format!("cannot resolve expression {owner:?}: {error}"))
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

fn wrap_success(boundary: RuntimeSemanticTypeId, value: RuntimeExprSeed) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        boundary,
        RuntimeExprSeedKind::Variant {
            ordinal: 0,
            payload: Some(Box::new(value)),
        },
    )
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
