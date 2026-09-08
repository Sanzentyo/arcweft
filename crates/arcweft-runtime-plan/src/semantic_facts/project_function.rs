//! Checked project-function continuation and closed-instance runtime facts.
//!
//! Curried project calls retain checked lineage until the terminal application
//! closes every generic parameter. Only that terminal call may name a
//! concrete runtime function instance. Runtime execution therefore never
//! selects an instantiation from a callee value or from the first observed
//! call.

use std::collections::BTreeSet;

use arcweft_core::entry::RuntimeCallableId;
use arcweft_id::{EffectId, runtime_program::RuntimeProjectContinuationLineageId};
use arcweft_lang_hir::{
    identity::{CaptureId, ExprId, ItemId, LocalId, PatternId, ScopeId, StmtId, TypeId},
    item::HirParameterKind,
};
use arcweft_lang_sema::callable::{
    CallableGroupIndex, CallableInstantiationDigest, CheckedClosureId,
};
use arcweft_lang_sema::final_analysis::{
    CheckedExecutableRuntimeExpressionFactFamily, CheckedExecutableRuntimeFactPartition,
    CheckedExecutableRuntimePatternFactFamily, CheckedExecutableRuntimeStatementFactFamily,
};
use thiserror::Error;

use super::{
    RuntimeAssertionAdmission, RuntimeAssignmentFact, RuntimeAwaitFact, RuntimeCheckedCapture,
    RuntimeChoiceFact, RuntimeContentFragmentFact, RuntimeDialogueApplication,
    RuntimeEvaluatedEffectFact, RuntimeImplicitCallableFact, RuntimeIteratorFact,
    RuntimeNormalizedType, RuntimePipeFact, RuntimeProjectCallable, RuntimeProjectItem,
    RuntimeRecordExpressionFact, RuntimeRecordPatternFact, RuntimeResolvedCall,
    RuntimeResolvedSelect, RuntimeResolvedValue, RuntimeResolvedVariant,
    RuntimeScopedExecutableSemanticFactView, RuntimeSequenceKind, RuntimeTriggerAdmission,
    RuntimeTryFact, RuntimeTypeShape,
};
use arcweft_core::value::RuntimeValue;

/// Stable identity of one fully closed ordinary project-function group.
///
/// The callable identity and semantic instantiation digest are owner-issued;
/// the group is the exact checked application group that performs the
/// invocation. No call-site expression ID participates in this identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeProjectFunctionInstanceKey {
    callable: RuntimeCallableId,
    instantiation: CallableInstantiationDigest,
    group: CallableGroupIndex,
}

impl RuntimeProjectFunctionInstanceKey {
    pub const fn new(
        callable: RuntimeCallableId,
        instantiation: CallableInstantiationDigest,
        group: CallableGroupIndex,
    ) -> Self {
        Self {
            callable,
            instantiation,
            group,
        }
    }

    pub const fn callable(&self) -> &RuntimeCallableId {
        &self.callable
    }

    pub const fn instantiation(&self) -> CallableInstantiationDigest {
        self.instantiation
    }

    pub const fn group(&self) -> CallableGroupIndex {
        self.group
    }
}

/// Checked non-call runtime role that reaches one closed ordinary Function
/// instance.
///
/// The role is structural rather than a display label. Together with the
/// exact Entry item owner it lets semantic-fact admission validate the HIR
/// member family while the instance key retains the sole callable identity
/// and closed substitution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeProjectFunctionRootRole {
    EntryInitializer,
    EntryReducer,
    EntryController,
}

/// One checked non-call ingress into the ordinary project-function instance
/// catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectFunctionRootFact {
    entry: ItemId,
    role: RuntimeProjectFunctionRootRole,
    instance: RuntimeProjectFunctionInstanceKey,
}

impl RuntimeProjectFunctionRootFact {
    pub const fn new(
        entry: ItemId,
        role: RuntimeProjectFunctionRootRole,
        instance: RuntimeProjectFunctionInstanceKey,
    ) -> Self {
        Self {
            entry,
            role,
            instance,
        }
    }

    pub const fn entry(&self) -> ItemId {
        self.entry
    }

    pub const fn role(&self) -> RuntimeProjectFunctionRootRole {
        self.role
    }

    pub const fn instance(&self) -> &RuntimeProjectFunctionInstanceKey {
        &self.instance
    }
}

/// Exact normalized ABI carried by one project-function continuation value.
///
/// The stable lineage proves semantic origin; the explicit prefix row lets
/// runtime construction, application, and snapshot restoration validate the
/// opaque stored values without reopening HIR or adopting operand types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectContinuationAbi {
    lineage: RuntimeProjectContinuationLineageId,
    function_type: RuntimeNormalizedType,
    prefix_types: Box<[RuntimeNormalizedType]>,
}

impl RuntimeProjectContinuationAbi {
    pub fn try_new(
        lineage: RuntimeProjectContinuationLineageId,
        function_type: RuntimeNormalizedType,
        prefix_types: Box<[RuntimeNormalizedType]>,
    ) -> Result<Self, RuntimeProjectFunctionFactError> {
        if !matches!(function_type.shape(), RuntimeTypeShape::Function { .. }) {
            return Err(RuntimeProjectFunctionFactError::InvalidFunctionType);
        }
        Ok(Self {
            lineage,
            function_type,
            prefix_types,
        })
    }

    pub const fn lineage(&self) -> RuntimeProjectContinuationLineageId {
        self.lineage
    }

    pub const fn function_type(&self) -> &RuntimeNormalizedType {
        &self.function_type
    }

    pub const fn prefix_types(&self) -> &[RuntimeNormalizedType] {
        &self.prefix_types
    }
}

/// One logical current-group binding and its complete physical
/// materialization recipe. `operand_indices` address the sole source-ordered
/// `RuntimeResolvedCall::operands` row, never ABI positions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectFunctionParameterMaterialization {
    group: CallableGroupIndex,
    parameter: u32,
    kind: HirParameterKind,
    abi_ty: RuntimeNormalizedType,
    binding_ty: RuntimeNormalizedType,
    operand_indices: Box<[u32]>,
}

impl RuntimeProjectFunctionParameterMaterialization {
    pub fn try_new(
        group: CallableGroupIndex,
        parameter: u32,
        kind: HirParameterKind,
        abi_ty: RuntimeNormalizedType,
        binding_ty: RuntimeNormalizedType,
        operand_indices: Box<[u32]>,
    ) -> Result<Self, RuntimeProjectFunctionFactError> {
        let valid_source = match kind {
            HirParameterKind::Fixed | HirParameterKind::ExtensionReceiver => {
                operand_indices.len() == 1
            }
            HirParameterKind::RestPositional => {
                operand_indices.windows(2).all(|pair| pair[0] < pair[1])
            }
        };
        if !valid_source {
            return Err(RuntimeProjectFunctionFactError::InvalidParameterMaterialization);
        }
        if !parameter_binding_type_matches(kind, &abi_ty, &binding_ty) {
            return Err(RuntimeProjectFunctionFactError::InvalidParameterMaterialization);
        }
        Ok(Self {
            group,
            parameter,
            kind,
            abi_ty,
            binding_ty,
            operand_indices,
        })
    }

    pub const fn group(&self) -> CallableGroupIndex {
        self.group
    }

    pub const fn parameter(&self) -> u32 {
        self.parameter
    }

    pub const fn kind(&self) -> HirParameterKind {
        self.kind
    }

    pub const fn abi_ty(&self) -> &RuntimeNormalizedType {
        &self.abi_ty
    }

    pub const fn binding_ty(&self) -> &RuntimeNormalizedType {
        &self.binding_ty
    }

    pub const fn operand_indices(&self) -> &[u32] {
        &self.operand_indices
    }
}

/// Exact checked input consumed by one project-function call site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeProjectFunctionCallInput {
    /// The call names the project declaration directly and has no previously
    /// evaluated prefix.
    Direct,
    /// The call consumes one previously produced continuation. `callee` is a
    /// generation-local HIR join only; `lineage` is the stable semantic
    /// identity checked by the runtime value boundary.
    Continuation {
        callee: ExprId,
        abi: RuntimeProjectContinuationAbi,
    },
}

impl RuntimeProjectFunctionCallInput {
    pub const fn callee(&self) -> Option<ExprId> {
        match self {
            Self::Direct => None,
            Self::Continuation { callee, .. } => Some(*callee),
        }
    }

    pub const fn lineage(&self) -> Option<RuntimeProjectContinuationLineageId> {
        match self {
            Self::Direct => None,
            Self::Continuation { abi, .. } => Some(abi.lineage()),
        }
    }

    pub const fn function_type(&self) -> Option<&RuntimeNormalizedType> {
        match self {
            Self::Direct => None,
            Self::Continuation { abi, .. } => Some(abi.function_type()),
        }
    }

    pub const fn continuation_abi(&self) -> Option<&RuntimeProjectContinuationAbi> {
        match self {
            Self::Direct => None,
            Self::Continuation { abi, .. } => Some(abi),
        }
    }
}

/// Checked result owned by one project-function call site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeProjectFunctionCallOutcome {
    /// The current group appends its operands once and yields another typed
    /// continuation. It does not allocate or invoke a function site.
    Continue {
        abi: RuntimeProjectContinuationAbi,
        next_group: CallableGroupIndex,
    },
    /// The current group closes the call and invokes this exact compiler-
    /// produced function instance.
    Invoke {
        instance: RuntimeProjectFunctionInstanceKey,
    },
}

impl RuntimeProjectFunctionCallOutcome {
    pub const fn lineage(&self) -> Option<RuntimeProjectContinuationLineageId> {
        match self {
            Self::Continue { abi, .. } => Some(abi.lineage()),
            Self::Invoke { .. } => None,
        }
    }

    pub const fn next_group(&self) -> Option<CallableGroupIndex> {
        match self {
            Self::Continue { next_group, .. } => Some(*next_group),
            Self::Invoke { .. } => None,
        }
    }

    pub const fn function_type(&self) -> Option<&RuntimeNormalizedType> {
        match self {
            Self::Continue { abi, .. } => Some(abi.function_type()),
            Self::Invoke { .. } => None,
        }
    }

    pub const fn instance(&self) -> Option<&RuntimeProjectFunctionInstanceKey> {
        match self {
            Self::Continue { .. } => None,
            Self::Invoke { instance } => Some(instance),
        }
    }

    pub const fn continuation_abi(&self) -> Option<&RuntimeProjectContinuationAbi> {
        match self {
            Self::Continue { abi, .. } => Some(abi),
            Self::Invoke { .. } => None,
        }
    }
}

/// One call-fact-owned project-function continuation/invocation contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectFunctionCallPlan {
    callable: RuntimeProjectCallable,
    current_group_materialization: Box<[RuntimeProjectFunctionParameterMaterialization]>,
    input: RuntimeProjectFunctionCallInput,
    outcome: RuntimeProjectFunctionCallOutcome,
}

impl RuntimeProjectFunctionCallPlan {
    pub fn try_new(
        callable: RuntimeProjectCallable,
        completed_group: CallableGroupIndex,
        current_group_materialization: Box<[RuntimeProjectFunctionParameterMaterialization]>,
        input: RuntimeProjectFunctionCallInput,
        outcome: RuntimeProjectFunctionCallOutcome,
    ) -> Result<Self, RuntimeProjectFunctionFactError> {
        if callable.declaration().owner()
            != arcweft_lang_hir::symbol::CallableDeclarationOwner::Function
        {
            return Err(RuntimeProjectFunctionFactError::NotOrdinaryFunction);
        }
        if input
            .function_type()
            .is_some_and(|ty| !matches!(ty.shape(), RuntimeTypeShape::Function { .. }))
            || outcome
                .function_type()
                .is_some_and(|ty| !matches!(ty.shape(), RuntimeTypeShape::Function { .. }))
        {
            return Err(RuntimeProjectFunctionFactError::InvalidFunctionType);
        }
        let input_prefix_types = input
            .continuation_abi()
            .map_or(&[][..], RuntimeProjectContinuationAbi::prefix_types);
        let output_prefix_len = input_prefix_types
            .len()
            .checked_add(current_group_materialization.len())
            .ok_or(RuntimeProjectFunctionFactError::InvalidContinuationAbi)?;
        let mut physical_operands = BTreeSet::new();
        for (expected_parameter, parameter) in current_group_materialization.iter().enumerate() {
            if parameter.group() != completed_group
                || u32::try_from(expected_parameter).ok() != Some(parameter.parameter())
                || parameter
                    .operand_indices()
                    .iter()
                    .any(|index| !physical_operands.insert(*index))
            {
                return Err(RuntimeProjectFunctionFactError::InvalidParameterMaterialization);
            }
        }
        if matches!(input, RuntimeProjectFunctionCallInput::Direct) != (completed_group.get() == 0)
        {
            return Err(RuntimeProjectFunctionFactError::InvalidContinuationAbi);
        }
        match &outcome {
            RuntimeProjectFunctionCallOutcome::Continue { abi, next_group }
                if completed_group.get().checked_add(1) != Some(next_group.get())
                    || abi.prefix_types().len() != output_prefix_len
                    || !abi.prefix_types().starts_with(input_prefix_types)
                    || abi.prefix_types()[input_prefix_types.len()..]
                        .iter()
                        .zip(current_group_materialization.iter())
                        .any(|(actual, materialization)| {
                            actual != materialization.binding_ty()
                        }) =>
            {
                return Err(RuntimeProjectFunctionFactError::InvalidContinuationAbi);
            }
            RuntimeProjectFunctionCallOutcome::Invoke { instance }
                if instance.callable() != callable.runtime()
                    || instance.group() != completed_group =>
            {
                return Err(RuntimeProjectFunctionFactError::InstanceKeyMismatch);
            }
            RuntimeProjectFunctionCallOutcome::Continue { .. }
            | RuntimeProjectFunctionCallOutcome::Invoke { .. } => {}
        }
        Ok(Self {
            callable,
            current_group_materialization,
            input,
            outcome,
        })
    }

    pub const fn callable(&self) -> &RuntimeProjectCallable {
        &self.callable
    }

    pub const fn input(&self) -> &RuntimeProjectFunctionCallInput {
        &self.input
    }

    pub const fn current_group_materialization(
        &self,
    ) -> &[RuntimeProjectFunctionParameterMaterialization] {
        &self.current_group_materialization
    }

    pub const fn outcome(&self) -> &RuntimeProjectFunctionCallOutcome {
        &self.outcome
    }
}

/// Runtime emission selected from the final checked ordinary-function role.
/// Both variants reserve an ordinary `RuntimeFunctionSite`; this enum selects
/// its structured body family and never reintroduces `RuntimePureHelper`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeProjectFunctionExecution {
    ExpressionFunctionSite,
    ExecutableFunctionSite,
}

/// Exact source-ordered ABI row for one ordinary parameter in the invoked
/// prefix/current group chain.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeProjectFunctionParameterSource {
    /// Value already evaluated by a previous group and retained in the
    /// `ProjectContinuation` prefix product.
    ContinuationPrefix { position: u32 },
    /// Logical binding value evaluated by the terminal application being
    /// invoked. This is not a physical call-operand position: a rest
    /// parameter may consume many physical operands and still owns one row.
    CurrentGroup { position: u32 },
}

/// Pattern and local IDs are generation-bound installation joins; the
/// normalized type is the frozen instantiated ABI type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectFunctionParameterAbi {
    group: CallableGroupIndex,
    parameter: u32,
    source: RuntimeProjectFunctionParameterSource,
    pattern: PatternId,
    source_type: TypeId,
    kind: HirParameterKind,
    bindings: Box<[LocalId]>,
    abi_ty: RuntimeNormalizedType,
    binding_ty: RuntimeNormalizedType,
}

impl RuntimeProjectFunctionParameterAbi {
    pub fn new(
        group: CallableGroupIndex,
        parameter: u32,
        source: RuntimeProjectFunctionParameterSource,
        pattern: PatternId,
        source_type: TypeId,
        kind: HirParameterKind,
        bindings: Box<[LocalId]>,
        abi_ty: RuntimeNormalizedType,
        binding_ty: RuntimeNormalizedType,
    ) -> Self {
        Self {
            group,
            parameter,
            source,
            pattern,
            source_type,
            kind,
            bindings,
            abi_ty,
            binding_ty,
        }
    }

    pub const fn group(&self) -> CallableGroupIndex {
        self.group
    }

    pub const fn parameter(&self) -> u32 {
        self.parameter
    }

    pub const fn source(&self) -> RuntimeProjectFunctionParameterSource {
        self.source
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }

    pub const fn source_type(&self) -> TypeId {
        self.source_type
    }

    pub const fn kind(&self) -> HirParameterKind {
        self.kind
    }

    pub const fn bindings(&self) -> &[LocalId] {
        &self.bindings
    }

    pub const fn abi_ty(&self) -> &RuntimeNormalizedType {
        &self.abi_ty
    }

    pub const fn binding_ty(&self) -> &RuntimeNormalizedType {
        &self.binding_ty
    }
}

/// Exact logical parameter source captured by the declaration-owned attached
/// default function. The source is resolved from the pending ProjectCall
/// logical product, never from the caller environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectAttachedDefaultCapture {
    group: CallableGroupIndex,
    parameter: u32,
    source: RuntimeProjectFunctionParameterSource,
    pattern: PatternId,
    pattern_digest: arcweft_lang_sema::final_analysis::CheckedPatternSemanticDigest,
    bindings: Box<[LocalId]>,
    used_locals: Box<[LocalId]>,
    binding_ty: RuntimeNormalizedType,
}

impl RuntimeProjectAttachedDefaultCapture {
    pub fn new(
        group: CallableGroupIndex,
        parameter: u32,
        source: RuntimeProjectFunctionParameterSource,
        pattern: PatternId,
        pattern_digest: arcweft_lang_sema::final_analysis::CheckedPatternSemanticDigest,
        bindings: Box<[LocalId]>,
        used_locals: Box<[LocalId]>,
        binding_ty: RuntimeNormalizedType,
    ) -> Self {
        Self {
            group,
            parameter,
            source,
            pattern,
            pattern_digest,
            bindings,
            used_locals,
            binding_ty,
        }
    }

    pub const fn group(&self) -> CallableGroupIndex {
        self.group
    }

    pub const fn parameter(&self) -> u32 {
        self.parameter
    }

    pub const fn source(&self) -> RuntimeProjectFunctionParameterSource {
        self.source
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }

    pub const fn pattern_digest(
        &self,
    ) -> arcweft_lang_sema::final_analysis::CheckedPatternSemanticDigest {
        self.pattern_digest
    }

    pub const fn bindings(&self) -> &[LocalId] {
        &self.bindings
    }

    pub const fn used_locals(&self) -> &[LocalId] {
        &self.used_locals
    }

    pub const fn binding_ty(&self) -> &RuntimeNormalizedType {
        &self.binding_ty
    }
}

/// Closed executable default function owned atomically by one terminal
/// project-function instance. Its capture sources address that ProjectCall's
/// logical prefix/current values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectAttachedDefaultFunctionFact {
    source: ExprId,
    coordinate: arcweft_lang_sema::semantic_coordinate::StableCheckedValueCoordinate,
    digest: arcweft_lang_sema::callable::CheckedAttachedContentDefaultExpressionDigest,
    result: RuntimeNormalizedType,
    suspension: arcweft_lang_sema::final_analysis::CheckedSuspensionRole,
    control: arcweft_lang_sema::final_analysis::CheckedExecutableControlRole,
    execution: RuntimeProjectFunctionExecution,
    effects: Box<[EffectId]>,
    captures: Box<[RuntimeProjectAttachedDefaultCapture]>,
}

impl RuntimeProjectAttachedDefaultFunctionFact {
    pub fn try_new(
        source: ExprId,
        coordinate: arcweft_lang_sema::semantic_coordinate::StableCheckedValueCoordinate,
        digest: arcweft_lang_sema::callable::CheckedAttachedContentDefaultExpressionDigest,
        result: RuntimeNormalizedType,
        suspension: arcweft_lang_sema::final_analysis::CheckedSuspensionRole,
        control: arcweft_lang_sema::final_analysis::CheckedExecutableControlRole,
        execution: RuntimeProjectFunctionExecution,
        effects: Box<[EffectId]>,
        captures: Box<[RuntimeProjectAttachedDefaultCapture]>,
    ) -> Result<Self, RuntimeProjectFunctionFactError> {
        let expected_execution = match (suspension, effects.is_empty(), control) {
            (
                arcweft_lang_sema::final_analysis::CheckedSuspensionRole::NonSuspending,
                true,
                arcweft_lang_sema::final_analysis::CheckedExecutableControlRole::ExpressionCompatible,
            ) => {
                RuntimeProjectFunctionExecution::ExpressionFunctionSite
            }
            (
                arcweft_lang_sema::final_analysis::CheckedSuspensionRole::NonSuspending,
                false,
                _,
            )
            | (
                arcweft_lang_sema::final_analysis::CheckedSuspensionRole::MaySuspend,
                _,
                _,
            )
            | (
                arcweft_lang_sema::final_analysis::CheckedSuspensionRole::NonSuspending,
                true,
                arcweft_lang_sema::final_analysis::CheckedExecutableControlRole::FlowRequired,
            ) => {
                RuntimeProjectFunctionExecution::ExecutableFunctionSite
            }
        };
        if execution != expected_execution
            || !effects.windows(2).all(|pair| pair[0] < pair[1])
            || captures.windows(2).any(|pair| {
                (pair[0].group(), pair[0].parameter()) >= (pair[1].group(), pair[1].parameter())
            })
            || captures.iter().any(|capture| {
                let bindings = capture.bindings().iter().copied().collect::<BTreeSet<_>>();
                let used = capture
                    .used_locals()
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>();
                bindings.len() != capture.bindings().len()
                    || used.is_empty()
                    || used.len() != capture.used_locals().len()
                    || !used.is_subset(&bindings)
            })
        {
            return Err(RuntimeProjectFunctionFactError::InvalidAttachedDefaultFunction);
        }
        Ok(Self {
            source,
            coordinate,
            digest,
            result,
            suspension,
            control,
            execution,
            effects,
            captures,
        })
    }

    pub const fn source(&self) -> ExprId {
        self.source
    }

    pub const fn coordinate(
        &self,
    ) -> &arcweft_lang_sema::semantic_coordinate::StableCheckedValueCoordinate {
        &self.coordinate
    }

    pub const fn digest(
        &self,
    ) -> arcweft_lang_sema::callable::CheckedAttachedContentDefaultExpressionDigest {
        self.digest
    }

    pub const fn result(&self) -> &RuntimeNormalizedType {
        &self.result
    }

    pub const fn suspension(&self) -> arcweft_lang_sema::final_analysis::CheckedSuspensionRole {
        self.suspension
    }

    pub const fn control(&self) -> arcweft_lang_sema::final_analysis::CheckedExecutableControlRole {
        self.control
    }

    pub const fn execution(&self) -> RuntimeProjectFunctionExecution {
        self.execution
    }

    pub const fn effects(&self) -> &[EffectId] {
        &self.effects
    }

    pub const fn captures(&self) -> &[RuntimeProjectAttachedDefaultCapture] {
        &self.captures
    }
}

/// Exact ordinary-function body selected from final HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectFunctionBody {
    scope: ScopeId,
    statements: Box<[StmtId]>,
    tail: ExprId,
}

impl RuntimeProjectFunctionBody {
    pub const fn new(scope: ScopeId, statements: Box<[StmtId]>, tail: ExprId) -> Self {
        Self {
            scope,
            statements,
            tail,
        }
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub const fn statements(&self) -> &[StmtId] {
        &self.statements
    }

    pub const fn tail(&self) -> ExprId {
        self.tail
    }
}

/// Generation-local HIR owner of one frozen instantiated type projection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeProjectFunctionTypeOwner {
    Expression(ExprId),
    Pattern(PatternId),
    Local(LocalId),
    Type(TypeId),
}

impl RuntimeProjectFunctionTypeOwner {
    pub const fn module(self) -> arcweft_lang_hir::identity::HirModuleId {
        match self {
            Self::Expression(owner) => owner.module(),
            Self::Pattern(owner) => owner.module(),
            Self::Local(owner) => owner.module(),
            Self::Type(owner) => owner.module(),
        }
    }
}

/// Complete substitution-backed semantic disposition used while lowering one
/// closed instance body. Every callable-scope expression owns a row even when
/// its semantic result is not a runtime value; patterns, locals, and source type roots
/// always own a normalized value type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeProjectFunctionTypeProjection {
    Value {
        owner: RuntimeProjectFunctionTypeOwner,
        ty: RuntimeNormalizedType,
    },
    SemanticOnlyExpression {
        owner: ExprId,
    },
}

/// Closed runtime payload selected for one expression in a project-function
/// instance. Every expression in the sealed executable partition owns exactly
/// one variant, including structural and consumed rows, so adding a new
/// checked family cannot silently omit its runtime projection.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeProjectFunctionExpressionPayload {
    Structural,
    Consumed,
    Literal(RuntimeValue),
    Value(RuntimeResolvedValue),
    Select(RuntimeResolvedSelect),
    NominalRecord(RuntimeRecordExpressionFact),
    Variant(RuntimeResolvedVariant),
    Call(RuntimeResolvedCall),
    PostfixCandidate(ExprId),
    Await(RuntimeAwaitFact),
    Choice(RuntimeChoiceFact),
    Try(RuntimeTryFact),
    ImplicitCallable {
        callable: RuntimeImplicitCallableFact,
        tried: Option<RuntimeTryFact>,
        pipe: Option<RuntimePipeFact>,
    },
    Pipe(RuntimePipeFact),
    DialogueApplication {
        application: RuntimeDialogueApplication,
        fragments: Box<[RuntimeContentFragmentFact]>,
    },
    ContentApplication {
        fragments: Box<[RuntimeContentFragmentFact]>,
    },
    Closure(Box<RuntimeClosureInstanceFact>),
}

impl RuntimeProjectFunctionExpressionPayload {
    pub const fn family(&self) -> CheckedExecutableRuntimeExpressionFactFamily {
        match self {
            Self::Structural => CheckedExecutableRuntimeExpressionFactFamily::Structural,
            Self::Consumed => CheckedExecutableRuntimeExpressionFactFamily::Consumed,
            Self::Literal(_) => CheckedExecutableRuntimeExpressionFactFamily::Literal,
            Self::Value(_) => CheckedExecutableRuntimeExpressionFactFamily::Value,
            Self::Select(_) => CheckedExecutableRuntimeExpressionFactFamily::Select,
            Self::NominalRecord(_) => CheckedExecutableRuntimeExpressionFactFamily::NominalRecord,
            Self::Variant(_) => CheckedExecutableRuntimeExpressionFactFamily::Variant,
            Self::Call(_) => CheckedExecutableRuntimeExpressionFactFamily::Call,
            Self::PostfixCandidate(_) => {
                CheckedExecutableRuntimeExpressionFactFamily::PostfixCandidate
            }
            Self::Await(_) => CheckedExecutableRuntimeExpressionFactFamily::Await,
            Self::Choice(_) => CheckedExecutableRuntimeExpressionFactFamily::Choice,
            Self::Try(_) => CheckedExecutableRuntimeExpressionFactFamily::Try,
            Self::ImplicitCallable { .. } => {
                CheckedExecutableRuntimeExpressionFactFamily::ImplicitCallable
            }
            Self::Pipe(_) => CheckedExecutableRuntimeExpressionFactFamily::Pipe,
            Self::DialogueApplication { .. } => {
                CheckedExecutableRuntimeExpressionFactFamily::DialogueApplication
            }
            Self::ContentApplication { .. } => {
                CheckedExecutableRuntimeExpressionFactFamily::ContentApplication
            }
            Self::Closure(_) => CheckedExecutableRuntimeExpressionFactFamily::Closure,
        }
    }
}

/// One source-owner-ordered closed expression row.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProjectFunctionExpressionSemanticFact {
    owner: ExprId,
    children: Box<[ExprId]>,
    payload: RuntimeProjectFunctionExpressionPayload,
}

impl RuntimeProjectFunctionExpressionSemanticFact {
    pub fn new(
        owner: ExprId,
        children: Box<[ExprId]>,
        payload: RuntimeProjectFunctionExpressionPayload,
    ) -> Self {
        Self {
            owner,
            children,
            payload,
        }
    }

    pub const fn owner(&self) -> ExprId {
        self.owner
    }

    pub const fn children(&self) -> &[ExprId] {
        &self.children
    }

    pub const fn payload(&self) -> &RuntimeProjectFunctionExpressionPayload {
        &self.payload
    }
}

/// Closed runtime payload selected for one pattern in a project-function
/// instance.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeProjectFunctionPatternPayload {
    Structural,
    Literal(RuntimeValue),
    Entity(RuntimeProjectItem),
    NominalRecord(RuntimeRecordPatternFact),
    Variant(RuntimeResolvedVariant),
    TypedBinding,
}

impl RuntimeProjectFunctionPatternPayload {
    pub const fn family(&self) -> CheckedExecutableRuntimePatternFactFamily {
        match self {
            Self::Structural => CheckedExecutableRuntimePatternFactFamily::Structural,
            Self::Literal(_) => CheckedExecutableRuntimePatternFactFamily::Literal,
            Self::Entity(_) => CheckedExecutableRuntimePatternFactFamily::Entity,
            Self::NominalRecord(_) => CheckedExecutableRuntimePatternFactFamily::NominalRecord,
            Self::Variant(_) => CheckedExecutableRuntimePatternFactFamily::Variant,
            Self::TypedBinding => CheckedExecutableRuntimePatternFactFamily::TypedBinding,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProjectFunctionPatternSemanticFact {
    owner: PatternId,
    payload: RuntimeProjectFunctionPatternPayload,
}

impl RuntimeProjectFunctionPatternSemanticFact {
    pub const fn new(owner: PatternId, payload: RuntimeProjectFunctionPatternPayload) -> Self {
        Self { owner, payload }
    }

    pub const fn owner(&self) -> PatternId {
        self.owner
    }

    pub const fn payload(&self) -> &RuntimeProjectFunctionPatternPayload {
        &self.payload
    }
}

/// Closed runtime payload selected for one statement in a project-function
/// instance. Payload-free variants remain explicit completeness evidence.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeProjectFunctionStatementPayload {
    Structural,
    Assignment(RuntimeAssignmentFact),
    Assertion(RuntimeAssertionAdmission),
    Defer,
    EvaluatedEffect(RuntimeEvaluatedEffectFact),
    Iteration(RuntimeIteratorFact),
    ControlTransfer,
    Trigger(RuntimeTriggerAdmission),
    UnsafeAudit,
    Select,
    SourceLocale,
    Scope,
    Include,
    Suspension,
    Yield,
}

impl RuntimeProjectFunctionStatementPayload {
    pub const fn family(&self) -> CheckedExecutableRuntimeStatementFactFamily {
        match self {
            Self::Structural => CheckedExecutableRuntimeStatementFactFamily::Structural,
            Self::Assignment(_) => CheckedExecutableRuntimeStatementFactFamily::Assignment,
            Self::Assertion(_) => CheckedExecutableRuntimeStatementFactFamily::Assertion,
            Self::Defer => CheckedExecutableRuntimeStatementFactFamily::Defer,
            Self::EvaluatedEffect(_) => {
                CheckedExecutableRuntimeStatementFactFamily::EvaluatedEffect
            }
            Self::Iteration(_) => CheckedExecutableRuntimeStatementFactFamily::Iteration,
            Self::ControlTransfer => CheckedExecutableRuntimeStatementFactFamily::ControlTransfer,
            Self::Trigger(_) => CheckedExecutableRuntimeStatementFactFamily::Trigger,
            Self::UnsafeAudit => CheckedExecutableRuntimeStatementFactFamily::UnsafeAudit,
            Self::Select => CheckedExecutableRuntimeStatementFactFamily::Select,
            Self::SourceLocale => CheckedExecutableRuntimeStatementFactFamily::SourceLocale,
            Self::Scope => CheckedExecutableRuntimeStatementFactFamily::Scope,
            Self::Include => CheckedExecutableRuntimeStatementFactFamily::Include,
            Self::Suspension => CheckedExecutableRuntimeStatementFactFamily::Suspension,
            Self::Yield => CheckedExecutableRuntimeStatementFactFamily::Yield,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProjectFunctionStatementSemanticFact {
    owner: StmtId,
    payload: RuntimeProjectFunctionStatementPayload,
}

impl RuntimeProjectFunctionStatementSemanticFact {
    pub const fn new(owner: StmtId, payload: RuntimeProjectFunctionStatementPayload) -> Self {
        Self { owner, payload }
    }

    pub const fn owner(&self) -> StmtId {
        self.owner
    }

    pub const fn payload(&self) -> &RuntimeProjectFunctionStatementPayload {
        &self.payload
    }
}

impl RuntimeProjectFunctionTypeProjection {
    pub const fn value(owner: RuntimeProjectFunctionTypeOwner, ty: RuntimeNormalizedType) -> Self {
        Self::Value { owner, ty }
    }

    pub const fn semantic_only_expression(owner: ExprId) -> Self {
        Self::SemanticOnlyExpression { owner }
    }

    pub const fn owner(&self) -> RuntimeProjectFunctionTypeOwner {
        match self {
            Self::Value { owner, .. } => *owner,
            Self::SemanticOnlyExpression { owner } => {
                RuntimeProjectFunctionTypeOwner::Expression(*owner)
            }
        }
    }

    pub const fn ty(&self) -> Option<&RuntimeNormalizedType> {
        match self {
            Self::Value { ty, .. } => Some(ty),
            Self::SemanticOnlyExpression { .. } => None,
        }
    }
}

/// Semantic identity of one explicit closure. The checked identity owns its
/// source/revision; an enclosing project instance, when present, owns the frozen
/// generic environment shared by all closures inside that instance.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeClosureInstanceKey {
    enclosing_instance: Option<RuntimeProjectFunctionInstanceKey>,
    closure: CheckedClosureId,
}

impl RuntimeClosureInstanceKey {
    pub const fn new(
        enclosing_instance: Option<RuntimeProjectFunctionInstanceKey>,
        closure: CheckedClosureId,
    ) -> Self {
        Self {
            enclosing_instance,
            closure,
        }
    }

    pub const fn enclosing_instance(&self) -> Option<&RuntimeProjectFunctionInstanceKey> {
        self.enclosing_instance.as_ref()
    }

    pub const fn closure(&self) -> &CheckedClosureId {
        &self.closure
    }
}

/// One logical closure parameter and its closed binding type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeClosureParameterFact {
    position: u32,
    pattern: PatternId,
    ty: RuntimeNormalizedType,
}

impl RuntimeClosureParameterFact {
    pub const fn new(position: u32, pattern: PatternId, ty: RuntimeNormalizedType) -> Self {
        Self {
            position,
            pattern,
            ty,
        }
    }

    pub const fn position(&self) -> u32 {
        self.position
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }

    pub const fn ty(&self) -> &RuntimeNormalizedType {
        &self.ty
    }
}

/// One source-ordered closure capture. `source` is the outer lexical local
/// evaluated once when constructing the closure value; `capture` is the exact
/// HIR capture row consumed by the nested executable partition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeClosureCaptureFact {
    position: u32,
    capture: CaptureId,
    source: LocalId,
    ty: RuntimeNormalizedType,
}

impl RuntimeClosureCaptureFact {
    pub const fn new(
        position: u32,
        capture: CaptureId,
        source: LocalId,
        ty: RuntimeNormalizedType,
    ) -> Self {
        Self {
            position,
            capture,
            source,
            ty,
        }
    }

    pub const fn position(&self) -> u32 {
        self.position
    }

    pub const fn capture(&self) -> CaptureId {
        self.capture
    }

    pub const fn source(&self) -> LocalId {
        self.source
    }

    pub const fn ty(&self) -> &RuntimeNormalizedType {
        &self.ty
    }
}

/// Complete closed function-site authority for an explicit closure produced
/// inside one project-function instance.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeClosureInstanceFact {
    key: RuntimeClosureInstanceKey,
    owner: ExprId,
    function_type: RuntimeNormalizedType,
    suspension: arcweft_lang_sema::final_analysis::CheckedSuspensionRole,
    control: arcweft_lang_sema::final_analysis::CheckedExecutableControlRole,
    execution: RuntimeProjectFunctionExecution,
    effects: Box<[EffectId]>,
    scope: ScopeId,
    body: ExprId,
    parameters: Box<[RuntimeClosureParameterFact]>,
    captures: Box<[RuntimeClosureCaptureFact]>,
    semantics: RuntimeProjectFunctionInstanceSemanticFacts,
}

impl RuntimeClosureInstanceFact {
    #[allow(
        clippy::too_many_arguments,
        reason = "one closure instance atomically owns identity, closed ABI, capture ABI, execution role, and nested semantic catalog"
    )]
    pub fn try_new(
        key: RuntimeClosureInstanceKey,
        owner: ExprId,
        function_type: RuntimeNormalizedType,
        suspension: arcweft_lang_sema::final_analysis::CheckedSuspensionRole,
        control: arcweft_lang_sema::final_analysis::CheckedExecutableControlRole,
        execution: RuntimeProjectFunctionExecution,
        effects: Box<[EffectId]>,
        scope: ScopeId,
        body: ExprId,
        parameters: Box<[RuntimeClosureParameterFact]>,
        captures: Box<[RuntimeClosureCaptureFact]>,
        semantics: RuntimeProjectFunctionInstanceSemanticFacts,
    ) -> Result<Self, RuntimeProjectFunctionFactError> {
        let RuntimeTypeShape::Function {
            parameters: function_parameters,
            result,
        } = function_type.shape()
        else {
            return Err(RuntimeProjectFunctionFactError::InvalidFunctionType);
        };
        if scope.module() != owner.module()
            || body.module() != owner.module()
            || semantics.partition().executable()
                != &arcweft_lang_hir::project::HirRuntimeExecutableOwner::Closure(owner)
            || semantics.expression_type(body) != Some(result.as_ref())
        {
            return Err(RuntimeProjectFunctionFactError::InvalidClosureInstance);
        }
        if parameters.len() != function_parameters.len()
            || parameters.iter().enumerate().any(|(position, parameter)| {
                u32::try_from(position).ok() != Some(parameter.position())
                    || function_parameters.get(position) != Some(parameter.ty())
                    || semantics.pattern_type(parameter.pattern()) != Some(parameter.ty())
            })
            || captures.iter().enumerate().any(|(position, capture)| {
                u32::try_from(position).ok() != Some(capture.position())
                    || semantics
                        .capture(capture.capture())
                        .is_none_or(|checked| checked.ty() != capture.ty())
            })
            || !effects.windows(2).all(|pair| pair[0] < pair[1])
        {
            return Err(RuntimeProjectFunctionFactError::InvalidClosureInstance);
        }
        let expected_execution = match (suspension, effects.is_empty(), control) {
            (
                arcweft_lang_sema::final_analysis::CheckedSuspensionRole::NonSuspending,
                true,
                arcweft_lang_sema::final_analysis::CheckedExecutableControlRole::ExpressionCompatible,
            ) => RuntimeProjectFunctionExecution::ExpressionFunctionSite,
            _ => RuntimeProjectFunctionExecution::ExecutableFunctionSite,
        };
        if execution != expected_execution {
            return Err(RuntimeProjectFunctionFactError::InvalidExecution);
        }
        Ok(Self {
            key,
            owner,
            function_type,
            suspension,
            control,
            execution,
            effects,
            scope,
            body,
            parameters,
            captures,
            semantics,
        })
    }

    pub const fn key(&self) -> &RuntimeClosureInstanceKey {
        &self.key
    }

    pub const fn owner(&self) -> ExprId {
        self.owner
    }

    pub const fn function_type(&self) -> &RuntimeNormalizedType {
        &self.function_type
    }

    pub const fn suspension(&self) -> arcweft_lang_sema::final_analysis::CheckedSuspensionRole {
        self.suspension
    }

    pub const fn control(&self) -> arcweft_lang_sema::final_analysis::CheckedExecutableControlRole {
        self.control
    }

    pub const fn execution(&self) -> RuntimeProjectFunctionExecution {
        self.execution
    }

    pub const fn effects(&self) -> &[EffectId] {
        &self.effects
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub const fn body(&self) -> ExprId {
        self.body
    }

    pub const fn parameters(&self) -> &[RuntimeClosureParameterFact] {
        &self.parameters
    }

    pub const fn captures(&self) -> &[RuntimeClosureCaptureFact] {
        &self.captures
    }

    pub const fn semantics(&self) -> &RuntimeProjectFunctionInstanceSemanticFacts {
        &self.semantics
    }
}

/// Complete closed semantic subcatalog for one exact project-function
/// executable partition.
///
/// This is the sole runtime-fact authority while lowering the associated
/// instance. It deliberately mirrors the global executable fact algebra but
/// stores every expression, pattern, statement, capture, and normalized type
/// under the instance's frozen substitution. Consumers must choose this
/// catalog as a whole; per-family fallback to global open facts is invalid.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProjectFunctionInstanceSemanticFacts {
    partition: CheckedExecutableRuntimeFactPartition,
    type_projection: Box<[RuntimeProjectFunctionTypeProjection]>,
    expressions: Box<[RuntimeProjectFunctionExpressionSemanticFact]>,
    patterns: Box<[RuntimeProjectFunctionPatternSemanticFact]>,
    statements: Box<[RuntimeProjectFunctionStatementSemanticFact]>,
    captures: Box<[RuntimeCheckedCapture]>,
}

impl RuntimeProjectFunctionInstanceSemanticFacts {
    pub fn try_new(
        partition: CheckedExecutableRuntimeFactPartition,
        type_projection: Box<[RuntimeProjectFunctionTypeProjection]>,
        expressions: Box<[RuntimeProjectFunctionExpressionSemanticFact]>,
        patterns: Box<[RuntimeProjectFunctionPatternSemanticFact]>,
        statements: Box<[RuntimeProjectFunctionStatementSemanticFact]>,
        captures: Box<[RuntimeCheckedCapture]>,
    ) -> Result<Self, RuntimeProjectFunctionFactError> {
        if type_projection
            .windows(2)
            .any(|pair| pair[0].owner() >= pair[1].owner())
        {
            return Err(RuntimeProjectFunctionFactError::NonCanonicalTypeProjection);
        }
        if partition.expressions().len() != expressions.len()
            || partition
                .expressions()
                .iter()
                .zip(&expressions)
                .any(|(expected, actual)| {
                    expected.owner() != actual.owner()
                        || expected.family() != actual.payload().family()
                        || expected.children() != actual.children()
                })
            || partition.patterns().len() != patterns.len()
            || partition
                .patterns()
                .iter()
                .zip(&patterns)
                .any(|(expected, actual)| {
                    expected.owner() != actual.owner()
                        || expected.family() != actual.payload().family()
                })
            || partition.statements().len() != statements.len()
            || partition
                .statements()
                .iter()
                .zip(&statements)
                .any(|(expected, actual)| {
                    expected.owner() != actual.owner()
                        || expected.family() != actual.payload().family()
                })
            || partition.captures().len() != captures.len()
            || partition
                .captures()
                .iter()
                .zip(&captures)
                .any(|(expected, actual)| *expected != actual.capture())
        {
            return Err(RuntimeProjectFunctionFactError::NonCanonicalSemanticFacts);
        }

        let expected_type_owners = partition
            .expressions()
            .iter()
            .map(|row| RuntimeProjectFunctionTypeOwner::Expression(row.owner()))
            .chain(
                partition
                    .patterns()
                    .iter()
                    .map(|row| RuntimeProjectFunctionTypeOwner::Pattern(row.owner())),
            )
            .chain(
                partition
                    .locals()
                    .iter()
                    .copied()
                    .map(RuntimeProjectFunctionTypeOwner::Local),
            )
            .chain(
                partition
                    .types()
                    .iter()
                    .copied()
                    .map(RuntimeProjectFunctionTypeOwner::Type),
            )
            .collect::<BTreeSet<_>>();
        let actual_type_owners = type_projection
            .iter()
            .map(RuntimeProjectFunctionTypeProjection::owner)
            .collect::<BTreeSet<_>>();
        if expected_type_owners != actual_type_owners
            || partition.expressions().iter().any(|expected| {
                type_projection
                    .binary_search_by_key(
                        &RuntimeProjectFunctionTypeOwner::Expression(expected.owner()),
                        RuntimeProjectFunctionTypeProjection::owner,
                    )
                    .ok()
                    .is_none_or(|index| {
                        type_projection[index].ty().is_some() != expected.has_runtime_type()
                    })
            })
            || partition.patterns().iter().any(|expected| {
                type_projection
                    .binary_search_by_key(
                        &RuntimeProjectFunctionTypeOwner::Pattern(expected.owner()),
                        RuntimeProjectFunctionTypeProjection::owner,
                    )
                    .ok()
                    .is_none_or(|index| type_projection[index].ty().is_none())
            })
            || partition.locals().iter().any(|owner| {
                type_projection
                    .binary_search_by_key(
                        &RuntimeProjectFunctionTypeOwner::Local(*owner),
                        RuntimeProjectFunctionTypeProjection::owner,
                    )
                    .ok()
                    .is_none_or(|index| type_projection[index].ty().is_none())
            })
            || partition.types().iter().any(|owner| {
                type_projection
                    .binary_search_by_key(
                        &RuntimeProjectFunctionTypeOwner::Type(*owner),
                        RuntimeProjectFunctionTypeProjection::owner,
                    )
                    .ok()
                    .is_none_or(|index| type_projection[index].ty().is_none())
            })
        {
            return Err(RuntimeProjectFunctionFactError::IncompleteTypeProjection);
        }

        let expression_owners = expressions
            .iter()
            .map(RuntimeProjectFunctionExpressionSemanticFact::owner)
            .collect::<BTreeSet<_>>();
        if expressions.iter().any(|row| {
            row.children()
                .iter()
                .any(|child| !expression_owners.contains(child))
        }) {
            return Err(RuntimeProjectFunctionFactError::NonCanonicalSemanticFacts);
        }

        Ok(Self {
            partition,
            type_projection,
            expressions,
            patterns,
            statements,
            captures,
        })
    }

    pub const fn partition(&self) -> &CheckedExecutableRuntimeFactPartition {
        &self.partition
    }

    pub const fn type_projection(&self) -> &[RuntimeProjectFunctionTypeProjection] {
        &self.type_projection
    }

    pub const fn expressions(&self) -> &[RuntimeProjectFunctionExpressionSemanticFact] {
        &self.expressions
    }

    pub const fn patterns(&self) -> &[RuntimeProjectFunctionPatternSemanticFact] {
        &self.patterns
    }

    pub const fn statements(&self) -> &[RuntimeProjectFunctionStatementSemanticFact] {
        &self.statements
    }

    pub const fn captures(&self) -> &[RuntimeCheckedCapture] {
        &self.captures
    }

    pub fn expression(
        &self,
        owner: ExprId,
    ) -> Option<&RuntimeProjectFunctionExpressionSemanticFact> {
        self.expressions
            .binary_search_by_key(&owner, RuntimeProjectFunctionExpressionSemanticFact::owner)
            .ok()
            .map(|index| &self.expressions[index])
    }

    pub fn pattern(&self, owner: PatternId) -> Option<&RuntimeProjectFunctionPatternSemanticFact> {
        self.patterns
            .binary_search_by_key(&owner, RuntimeProjectFunctionPatternSemanticFact::owner)
            .ok()
            .map(|index| &self.patterns[index])
    }

    pub fn statement(&self, owner: StmtId) -> Option<&RuntimeProjectFunctionStatementSemanticFact> {
        self.statements
            .binary_search_by_key(&owner, RuntimeProjectFunctionStatementSemanticFact::owner)
            .ok()
            .map(|index| &self.statements[index])
    }

    pub fn ty(&self, owner: RuntimeProjectFunctionTypeOwner) -> Option<&RuntimeNormalizedType> {
        self.type_projection
            .binary_search_by_key(&owner, RuntimeProjectFunctionTypeProjection::owner)
            .ok()
            .and_then(|index| self.type_projection[index].ty())
    }

    pub fn expression_type(&self, owner: ExprId) -> Option<&RuntimeNormalizedType> {
        self.ty(RuntimeProjectFunctionTypeOwner::Expression(owner))
    }

    pub fn pattern_type(&self, owner: PatternId) -> Option<&RuntimeNormalizedType> {
        self.ty(RuntimeProjectFunctionTypeOwner::Pattern(owner))
    }

    pub fn local_type(&self, owner: LocalId) -> Option<&RuntimeNormalizedType> {
        self.ty(RuntimeProjectFunctionTypeOwner::Local(owner))
    }

    pub fn source_type(&self, owner: TypeId) -> Option<&RuntimeNormalizedType> {
        self.ty(RuntimeProjectFunctionTypeOwner::Type(owner))
    }

    pub fn expression_children(&self, owner: ExprId) -> Option<&[ExprId]> {
        self.expression(owner)
            .map(RuntimeProjectFunctionExpressionSemanticFact::children)
    }

    pub fn call(&self, owner: ExprId) -> Option<&RuntimeResolvedCall> {
        match self.expression(owner)?.payload() {
            RuntimeProjectFunctionExpressionPayload::Call(call) => Some(call),
            _ => None,
        }
    }

    pub fn value(&self, owner: ExprId) -> Option<&RuntimeResolvedValue> {
        match self.expression(owner)?.payload() {
            RuntimeProjectFunctionExpressionPayload::Value(value) => Some(value),
            _ => None,
        }
    }

    pub fn expression_literal(&self, owner: ExprId) -> Option<&RuntimeValue> {
        match self.expression(owner)?.payload() {
            RuntimeProjectFunctionExpressionPayload::Literal(value) => Some(value),
            _ => None,
        }
    }

    pub fn select(&self, owner: ExprId) -> Option<&RuntimeResolvedSelect> {
        match self.expression(owner)?.payload() {
            RuntimeProjectFunctionExpressionPayload::Select(value) => Some(value),
            _ => None,
        }
    }

    pub fn nominal_record(&self, owner: ExprId) -> Option<&RuntimeRecordExpressionFact> {
        match self.expression(owner)?.payload() {
            RuntimeProjectFunctionExpressionPayload::NominalRecord(value) => Some(value),
            _ => None,
        }
    }

    pub fn expression_variant(&self, owner: ExprId) -> Option<&RuntimeResolvedVariant> {
        match self.expression(owner)?.payload() {
            RuntimeProjectFunctionExpressionPayload::Variant(value) => Some(value),
            _ => None,
        }
    }

    pub fn postfix_candidate(&self, owner: ExprId) -> Option<ExprId> {
        match self.expression(owner)?.payload() {
            RuntimeProjectFunctionExpressionPayload::PostfixCandidate(value) => Some(*value),
            _ => None,
        }
    }

    pub fn awaited(&self, owner: ExprId) -> Option<&RuntimeAwaitFact> {
        match self.expression(owner)?.payload() {
            RuntimeProjectFunctionExpressionPayload::Await(value) => Some(value),
            _ => None,
        }
    }

    pub fn choice(&self, owner: ExprId) -> Option<&RuntimeChoiceFact> {
        match self.expression(owner)?.payload() {
            RuntimeProjectFunctionExpressionPayload::Choice(value) => Some(value),
            _ => None,
        }
    }

    pub fn tried(&self, owner: ExprId) -> Option<&RuntimeTryFact> {
        match self.expression(owner)?.payload() {
            RuntimeProjectFunctionExpressionPayload::Try(value) => Some(value),
            RuntimeProjectFunctionExpressionPayload::ImplicitCallable {
                tried: Some(value),
                ..
            } => Some(value),
            _ => None,
        }
    }

    pub fn implicit_callable(&self, owner: ExprId) -> Option<&RuntimeImplicitCallableFact> {
        match self.expression(owner)?.payload() {
            RuntimeProjectFunctionExpressionPayload::ImplicitCallable { callable, .. } => {
                Some(callable)
            }
            _ => None,
        }
    }

    pub fn pipe(&self, owner: ExprId) -> Option<&RuntimePipeFact> {
        match self.expression(owner)?.payload() {
            RuntimeProjectFunctionExpressionPayload::Pipe(value)
            | RuntimeProjectFunctionExpressionPayload::ImplicitCallable {
                pipe: Some(value), ..
            } => Some(value),
            _ => None,
        }
    }

    pub fn dialogue_application(&self, owner: ExprId) -> Option<&RuntimeDialogueApplication> {
        match self.expression(owner)?.payload() {
            RuntimeProjectFunctionExpressionPayload::DialogueApplication {
                application, ..
            } => Some(application),
            _ => None,
        }
    }

    /// Returns the exact closed closure instance owned by `owner` in this
    /// executable catalog. A global/open closure fact is never substituted
    /// here: callers that selected an instance view must consume this row.
    pub fn closure_instance(&self, owner: ExprId) -> Option<&RuntimeClosureInstanceFact> {
        match self.expression(owner)?.payload() {
            RuntimeProjectFunctionExpressionPayload::Closure(closure) => Some(closure),
            _ => None,
        }
    }

    pub fn dialogue_content_fragment_for_source(
        &self,
        source: ExprId,
    ) -> Option<&RuntimeContentFragmentFact> {
        self.expressions.iter().find_map(|row| match row.payload() {
            RuntimeProjectFunctionExpressionPayload::DialogueApplication { fragments, .. }
            | RuntimeProjectFunctionExpressionPayload::ContentApplication { fragments } => {
                fragments
                    .iter()
                    .find(|fragment| fragment.source() == source)
            }
            _ => None,
        })
    }

    pub fn pattern_literal(&self, owner: PatternId) -> Option<&RuntimeValue> {
        match self.pattern(owner)?.payload() {
            RuntimeProjectFunctionPatternPayload::Literal(value) => Some(value),
            _ => None,
        }
    }

    pub fn pattern_item(&self, owner: PatternId) -> Option<&RuntimeProjectItem> {
        match self.pattern(owner)?.payload() {
            RuntimeProjectFunctionPatternPayload::Entity(value) => Some(value),
            _ => None,
        }
    }

    pub fn pattern_nominal_record(&self, owner: PatternId) -> Option<&RuntimeRecordPatternFact> {
        match self.pattern(owner)?.payload() {
            RuntimeProjectFunctionPatternPayload::NominalRecord(value) => Some(value),
            _ => None,
        }
    }

    pub fn pattern_variant(&self, owner: PatternId) -> Option<&RuntimeResolvedVariant> {
        match self.pattern(owner)?.payload() {
            RuntimeProjectFunctionPatternPayload::Variant(value) => Some(value),
            _ => None,
        }
    }

    pub fn assignment(&self, owner: StmtId) -> Option<&RuntimeAssignmentFact> {
        match self.statement(owner)?.payload() {
            RuntimeProjectFunctionStatementPayload::Assignment(value) => Some(value),
            _ => None,
        }
    }

    pub fn assertion(&self, owner: StmtId) -> Option<RuntimeAssertionAdmission> {
        match self.statement(owner)?.payload() {
            RuntimeProjectFunctionStatementPayload::Assertion(value) => Some(*value),
            _ => None,
        }
    }

    pub fn evaluated_effect(&self, owner: StmtId) -> Option<&RuntimeEvaluatedEffectFact> {
        match self.statement(owner)?.payload() {
            RuntimeProjectFunctionStatementPayload::EvaluatedEffect(value) => Some(value),
            _ => None,
        }
    }

    pub fn iteration(&self, owner: StmtId) -> Option<&RuntimeIteratorFact> {
        match self.statement(owner)?.payload() {
            RuntimeProjectFunctionStatementPayload::Iteration(value) => Some(value),
            _ => None,
        }
    }

    pub fn trigger(&self, owner: StmtId) -> Option<&RuntimeTriggerAdmission> {
        match self.statement(owner)?.payload() {
            RuntimeProjectFunctionStatementPayload::Trigger(value) => Some(value),
            _ => None,
        }
    }

    pub fn capture(
        &self,
        owner: arcweft_lang_hir::identity::CaptureId,
    ) -> Option<&RuntimeCheckedCapture> {
        self.captures
            .binary_search_by_key(&owner, RuntimeCheckedCapture::capture)
            .ok()
            .map(|index| &self.captures[index])
    }

    /// Visits this catalog and its lexically nested closed closure catalogs.
    pub(crate) fn visit_catalogs<'facts>(&'facts self, visitor: &mut impl FnMut(&'facts Self)) {
        visitor(self);
        for expression in &self.expressions {
            if let RuntimeProjectFunctionExpressionPayload::Closure(closure) = expression.payload()
            {
                closure.semantics().visit_catalogs(visitor);
            }
        }
    }

    pub fn visit_type_projections<'facts>(
        &'facts self,
        visitor: &mut impl FnMut(&'facts RuntimeProjectFunctionTypeProjection),
    ) {
        for projection in &self.type_projection {
            visitor(projection);
        }
        for expression in &self.expressions {
            if let RuntimeProjectFunctionExpressionPayload::Closure(closure) = expression.payload()
            {
                closure.semantics().visit_type_projections(visitor);
            }
        }
    }

    pub fn visit_captures<'facts>(
        &'facts self,
        visitor: &mut impl FnMut(&'facts RuntimeCheckedCapture),
    ) {
        for capture in &self.captures {
            visitor(capture);
        }
        for expression in &self.expressions {
            if let RuntimeProjectFunctionExpressionPayload::Closure(closure) = expression.payload()
            {
                closure.semantics().visit_captures(visitor);
            }
        }
    }

    /// Visits every nested closed closure in deterministic executable order.
    /// Each row is yielded before its nested closure catalog so consumers can
    /// reserve all function-site identities before defining any body.
    pub fn visit_closure_instances<'facts>(
        &'facts self,
        visitor: &mut impl FnMut(&'facts RuntimeClosureInstanceFact),
    ) {
        for expression in &self.expressions {
            if let RuntimeProjectFunctionExpressionPayload::Closure(closure) = expression.payload()
            {
                visitor(closure);
                closure.semantics().visit_closure_instances(visitor);
            }
        }
    }

    pub fn visit_dialogue_applications<'facts>(
        &'facts self,
        scope: RuntimeScopedExecutableSemanticFactView<'facts>,
        visitor: &mut impl FnMut(
            RuntimeScopedExecutableSemanticFactView<'facts>,
            ExprId,
            &'facts RuntimeDialogueApplication,
        ),
    ) {
        for expression in &self.expressions {
            match expression.payload() {
                RuntimeProjectFunctionExpressionPayload::DialogueApplication {
                    application,
                    ..
                } => visitor(scope, expression.owner(), application),
                RuntimeProjectFunctionExpressionPayload::Closure(closure) => {
                    let nested = RuntimeScopedExecutableSemanticFactView::closure(
                        closure.key(),
                        closure.semantics(),
                    );
                    closure
                        .semantics()
                        .visit_dialogue_applications(nested, visitor);
                }
                RuntimeProjectFunctionExpressionPayload::Structural
                | RuntimeProjectFunctionExpressionPayload::Consumed
                | RuntimeProjectFunctionExpressionPayload::Literal(_)
                | RuntimeProjectFunctionExpressionPayload::Value(_)
                | RuntimeProjectFunctionExpressionPayload::Select(_)
                | RuntimeProjectFunctionExpressionPayload::NominalRecord(_)
                | RuntimeProjectFunctionExpressionPayload::Variant(_)
                | RuntimeProjectFunctionExpressionPayload::Call(_)
                | RuntimeProjectFunctionExpressionPayload::PostfixCandidate(_)
                | RuntimeProjectFunctionExpressionPayload::Await(_)
                | RuntimeProjectFunctionExpressionPayload::Choice(_)
                | RuntimeProjectFunctionExpressionPayload::Try(_)
                | RuntimeProjectFunctionExpressionPayload::ImplicitCallable { .. }
                | RuntimeProjectFunctionExpressionPayload::Pipe(_)
                | RuntimeProjectFunctionExpressionPayload::ContentApplication { .. } => {}
            }
        }
    }

    pub fn visit_content_fragments<'facts>(
        &'facts self,
        scope: RuntimeScopedExecutableSemanticFactView<'facts>,
        visitor: &mut impl FnMut(
            RuntimeScopedExecutableSemanticFactView<'facts>,
            &'facts RuntimeContentFragmentFact,
        ),
    ) {
        for expression in &self.expressions {
            match expression.payload() {
                RuntimeProjectFunctionExpressionPayload::DialogueApplication {
                    fragments, ..
                }
                | RuntimeProjectFunctionExpressionPayload::ContentApplication { fragments } => {
                    for fragment in fragments {
                        visitor(scope, fragment);
                    }
                }
                RuntimeProjectFunctionExpressionPayload::Closure(closure) => {
                    let nested = RuntimeScopedExecutableSemanticFactView::closure(
                        closure.key(),
                        closure.semantics(),
                    );
                    closure.semantics().visit_content_fragments(nested, visitor);
                }
                RuntimeProjectFunctionExpressionPayload::Structural
                | RuntimeProjectFunctionExpressionPayload::Consumed
                | RuntimeProjectFunctionExpressionPayload::Literal(_)
                | RuntimeProjectFunctionExpressionPayload::Value(_)
                | RuntimeProjectFunctionExpressionPayload::Select(_)
                | RuntimeProjectFunctionExpressionPayload::NominalRecord(_)
                | RuntimeProjectFunctionExpressionPayload::Variant(_)
                | RuntimeProjectFunctionExpressionPayload::Call(_)
                | RuntimeProjectFunctionExpressionPayload::PostfixCandidate(_)
                | RuntimeProjectFunctionExpressionPayload::Await(_)
                | RuntimeProjectFunctionExpressionPayload::Choice(_)
                | RuntimeProjectFunctionExpressionPayload::Try(_)
                | RuntimeProjectFunctionExpressionPayload::ImplicitCallable { .. }
                | RuntimeProjectFunctionExpressionPayload::Pipe(_) => {}
            }
        }
    }

    pub fn dialogue_content_fragment(
        &self,
        template: arcweft_core::runtime_id::RuntimeDialogueContentTemplateId,
    ) -> Option<&RuntimeContentFragmentFact> {
        self.expressions
            .iter()
            .find_map(|expression| match expression.payload() {
                RuntimeProjectFunctionExpressionPayload::DialogueApplication {
                    fragments, ..
                }
                | RuntimeProjectFunctionExpressionPayload::ContentApplication { fragments } => {
                    fragments
                        .iter()
                        .find(|fragment| fragment.template().id() == template)
                }
                RuntimeProjectFunctionExpressionPayload::Closure(closure) => {
                    closure.semantics().dialogue_content_fragment(template)
                }
                RuntimeProjectFunctionExpressionPayload::Structural
                | RuntimeProjectFunctionExpressionPayload::Consumed
                | RuntimeProjectFunctionExpressionPayload::Literal(_)
                | RuntimeProjectFunctionExpressionPayload::Value(_)
                | RuntimeProjectFunctionExpressionPayload::Select(_)
                | RuntimeProjectFunctionExpressionPayload::NominalRecord(_)
                | RuntimeProjectFunctionExpressionPayload::Variant(_)
                | RuntimeProjectFunctionExpressionPayload::Call(_)
                | RuntimeProjectFunctionExpressionPayload::PostfixCandidate(_)
                | RuntimeProjectFunctionExpressionPayload::Await(_)
                | RuntimeProjectFunctionExpressionPayload::Choice(_)
                | RuntimeProjectFunctionExpressionPayload::Try(_)
                | RuntimeProjectFunctionExpressionPayload::ImplicitCallable { .. }
                | RuntimeProjectFunctionExpressionPayload::Pipe(_) => None,
            })
    }

    pub fn visit_calls<'facts>(
        &'facts self,
        visitor: &mut impl FnMut(ExprId, &'facts RuntimeResolvedCall),
    ) {
        for expression in &self.expressions {
            match expression.payload() {
                RuntimeProjectFunctionExpressionPayload::Call(call) => {
                    visitor(expression.owner(), call);
                }
                RuntimeProjectFunctionExpressionPayload::Closure(closure) => {
                    closure.semantics().visit_calls(visitor);
                }
                _ => {}
            }
        }
    }

    pub fn visit_statement_owners(&self, visitor: &mut impl FnMut(StmtId)) {
        for statement in &self.statements {
            visitor(statement.owner());
        }
        for expression in &self.expressions {
            if let RuntimeProjectFunctionExpressionPayload::Closure(closure) = expression.payload()
            {
                closure.semantics().visit_statement_owners(visitor);
            }
        }
    }
}

/// Compiler-produced closed ordinary project-function instance.
///
/// The complete body projection is owned here because the accepted runtime
/// type inventory is declaration-generic while this instance is closed under
/// one checked substitution. Runtime lowering must not reopen semantic maps or
/// adopt types from a call site's operand values.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProjectFunctionInstanceFact {
    key: RuntimeProjectFunctionInstanceKey,
    callable: RuntimeProjectCallable,
    suspension: arcweft_lang_sema::final_analysis::CheckedSuspensionRole,
    control: arcweft_lang_sema::final_analysis::CheckedExecutableControlRole,
    execution: RuntimeProjectFunctionExecution,
    function_type: RuntimeNormalizedType,
    parameters: Box<[RuntimeProjectFunctionParameterAbi]>,
    effects: Box<[EffectId]>,
    attached_default: Option<RuntimeProjectAttachedDefaultFunctionFact>,
    body: RuntimeProjectFunctionBody,
    semantics: RuntimeProjectFunctionInstanceSemanticFacts,
}

impl RuntimeProjectFunctionInstanceFact {
    #[allow(
        clippy::too_many_arguments,
        reason = "one closed instance atomically owns identity, ABI, effect row, body, and substituted type projection"
    )]
    pub fn try_new(
        key: RuntimeProjectFunctionInstanceKey,
        callable: RuntimeProjectCallable,
        suspension: arcweft_lang_sema::final_analysis::CheckedSuspensionRole,
        control: arcweft_lang_sema::final_analysis::CheckedExecutableControlRole,
        execution: RuntimeProjectFunctionExecution,
        function_type: RuntimeNormalizedType,
        parameters: Box<[RuntimeProjectFunctionParameterAbi]>,
        effects: Box<[EffectId]>,
        attached_default: Option<RuntimeProjectAttachedDefaultFunctionFact>,
        body: RuntimeProjectFunctionBody,
        semantics: RuntimeProjectFunctionInstanceSemanticFacts,
    ) -> Result<Self, RuntimeProjectFunctionFactError> {
        if callable.declaration().owner()
            != arcweft_lang_hir::symbol::CallableDeclarationOwner::Function
        {
            return Err(RuntimeProjectFunctionFactError::NotOrdinaryFunction);
        }
        if key.callable() != callable.runtime() {
            return Err(RuntimeProjectFunctionFactError::InstanceKeyMismatch);
        }
        let RuntimeTypeShape::Function {
            parameters: function_parameters,
            ..
        } = function_type.shape()
        else {
            return Err(RuntimeProjectFunctionFactError::InvalidFunctionType);
        };
        let mut previous_coordinate = None;
        let mut expected_prefix_position = 0_u32;
        let mut expected_current_position = 0_u32;
        for parameter in &parameters {
            let coordinate = (parameter.group(), parameter.parameter());
            if previous_coordinate.is_some_and(|previous| previous >= coordinate)
                || parameter.group().get() > key.group().get()
            {
                return Err(RuntimeProjectFunctionFactError::NonCanonicalParameterAbi);
            }
            previous_coordinate = Some(coordinate);
            if !parameter_binding_type_matches(
                parameter.kind(),
                parameter.abi_ty(),
                parameter.binding_ty(),
            ) {
                return Err(RuntimeProjectFunctionFactError::NonCanonicalParameterAbi);
            }
            match parameter.source() {
                RuntimeProjectFunctionParameterSource::ContinuationPrefix { position }
                    if parameter.group().get() < key.group().get()
                        && position == expected_prefix_position =>
                {
                    expected_prefix_position = expected_prefix_position
                        .checked_add(1)
                        .ok_or(RuntimeProjectFunctionFactError::NonCanonicalParameterAbi)?;
                }
                RuntimeProjectFunctionParameterSource::CurrentGroup { position }
                    if parameter.group() == key.group()
                        && position == expected_current_position =>
                {
                    expected_current_position = expected_current_position
                        .checked_add(1)
                        .ok_or(RuntimeProjectFunctionFactError::NonCanonicalParameterAbi)?;
                }
                RuntimeProjectFunctionParameterSource::ContinuationPrefix { .. }
                | RuntimeProjectFunctionParameterSource::CurrentGroup { .. } => {
                    return Err(RuntimeProjectFunctionFactError::NonCanonicalParameterAbi);
                }
            }
        }
        let mut expected_parameter_types = parameters
            .iter()
            .filter(|parameter| parameter.group() == key.group())
            .map(RuntimeProjectFunctionParameterAbi::abi_ty)
            .collect::<Vec<_>>();
        if let Some(attached) = callable.attached_content_abi()
            && attached.group() == key.group()
        {
            if attached.abi_position() != expected_current_position {
                return Err(RuntimeProjectFunctionFactError::NonCanonicalParameterAbi);
            }
            expected_parameter_types.push(attached.abi_ty());
        }
        if function_parameters.len() != expected_parameter_types.len()
            || function_parameters
                .iter()
                .zip(expected_parameter_types)
                .any(|(actual, expected)| actual != expected)
        {
            return Err(RuntimeProjectFunctionFactError::FunctionAbiMismatch);
        }
        if !effects.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(RuntimeProjectFunctionFactError::NonCanonicalEffectRow);
        }
        let expected_execution = match (suspension, effects.is_empty(), control) {
            (
                arcweft_lang_sema::final_analysis::CheckedSuspensionRole::NonSuspending,
                true,
                arcweft_lang_sema::final_analysis::CheckedExecutableControlRole::ExpressionCompatible,
            ) => RuntimeProjectFunctionExecution::ExpressionFunctionSite,
            (
                arcweft_lang_sema::final_analysis::CheckedSuspensionRole::NonSuspending,
                false,
                _,
            )
            | (
                arcweft_lang_sema::final_analysis::CheckedSuspensionRole::MaySuspend,
                _,
                _,
            )
            | (
                arcweft_lang_sema::final_analysis::CheckedSuspensionRole::NonSuspending,
                true,
                arcweft_lang_sema::final_analysis::CheckedExecutableControlRole::FlowRequired,
            ) => RuntimeProjectFunctionExecution::ExecutableFunctionSite,
        };
        if execution != expected_execution {
            return Err(RuntimeProjectFunctionFactError::InvalidExecution);
        }
        match (
            callable
                .attached_content_abi()
                .and_then(|attached| attached.default()),
            &attached_default,
        ) {
            (Some(expected), Some(default))
                if expected.source() == default.source()
                    && expected.coordinate() == default.coordinate()
                    && expected.digest() == default.digest()
                    && callable
                        .attached_content_abi()
                        .is_some_and(|attached| attached.binding_ty() == default.result())
                    && default
                        .effects()
                        .iter()
                        .all(|effect| effects.binary_search(effect).is_ok()) =>
            {
                for capture in default.captures() {
                    let Some(parameter) = parameters.iter().find(|parameter| {
                        parameter.group() == capture.group()
                            && parameter.parameter() == capture.parameter()
                    }) else {
                        return Err(
                            RuntimeProjectFunctionFactError::InvalidAttachedDefaultFunction,
                        );
                    };
                    if parameter.source() != capture.source()
                        || parameter.pattern() != capture.pattern()
                        || parameter.bindings() != capture.bindings()
                        || parameter.binding_ty() != capture.binding_ty()
                    {
                        return Err(
                            RuntimeProjectFunctionFactError::InvalidAttachedDefaultFunction,
                        );
                    }
                }
            }
            (None, None) => {}
            _ => return Err(RuntimeProjectFunctionFactError::InvalidAttachedDefaultFunction),
        }
        let module = callable.owner().module();
        if semantics.partition().executable()
            != &arcweft_lang_hir::project::HirRuntimeExecutableOwner::Item(callable.owner())
            || body.scope().module() != module
            || body.tail().module() != module
            || body
                .statements()
                .iter()
                .any(|statement| statement.module() != module)
            || parameters.iter().any(|parameter| {
                parameter.pattern().module() != module
                    || parameter.source_type().module() != module
                    || parameter
                        .bindings()
                        .iter()
                        .any(|binding| binding.module() != module)
            })
            || semantics
                .type_projection()
                .iter()
                .any(|projection| projection.owner().module() != module)
            || semantics
                .expressions()
                .iter()
                .any(|fact| fact.owner().module() != module)
            || semantics
                .patterns()
                .iter()
                .any(|fact| fact.owner().module() != module)
            || semantics
                .statements()
                .iter()
                .any(|fact| fact.owner().module() != module)
            || semantics
                .captures()
                .iter()
                .any(|fact| fact.capture().module() != module)
        {
            return Err(RuntimeProjectFunctionFactError::ForeignHirOwner);
        }
        let projected_owners = semantics
            .type_projection()
            .iter()
            .map(RuntimeProjectFunctionTypeProjection::owner)
            .collect::<BTreeSet<_>>();
        if !projected_owners.contains(&RuntimeProjectFunctionTypeOwner::Expression(body.tail()))
            || semantics.expressions().iter().any(|fact| {
                !projected_owners
                    .contains(&RuntimeProjectFunctionTypeOwner::Expression(fact.owner()))
            })
            || parameters.iter().any(|parameter| {
                !projected_owners.contains(&RuntimeProjectFunctionTypeOwner::Pattern(
                    parameter.pattern(),
                )) || !projected_owners.contains(&RuntimeProjectFunctionTypeOwner::Type(
                    parameter.source_type(),
                )) || parameter.bindings().iter().any(|binding| {
                    !projected_owners.contains(&RuntimeProjectFunctionTypeOwner::Local(*binding))
                })
            })
        {
            return Err(RuntimeProjectFunctionFactError::IncompleteTypeProjection);
        }
        Ok(Self {
            key,
            callable,
            suspension,
            control,
            execution,
            function_type,
            parameters,
            effects,
            attached_default,
            body,
            semantics,
        })
    }

    pub const fn key(&self) -> &RuntimeProjectFunctionInstanceKey {
        &self.key
    }

    pub const fn callable(&self) -> &RuntimeProjectCallable {
        &self.callable
    }

    pub const fn execution(&self) -> RuntimeProjectFunctionExecution {
        self.execution
    }

    pub const fn suspension(&self) -> arcweft_lang_sema::final_analysis::CheckedSuspensionRole {
        self.suspension
    }

    pub const fn control(&self) -> arcweft_lang_sema::final_analysis::CheckedExecutableControlRole {
        self.control
    }

    pub const fn function_type(&self) -> &RuntimeNormalizedType {
        &self.function_type
    }

    pub const fn parameters(&self) -> &[RuntimeProjectFunctionParameterAbi] {
        &self.parameters
    }

    pub const fn effects(&self) -> &[EffectId] {
        &self.effects
    }

    pub const fn attached_default(&self) -> Option<&RuntimeProjectAttachedDefaultFunctionFact> {
        self.attached_default.as_ref()
    }

    pub const fn body(&self) -> &RuntimeProjectFunctionBody {
        &self.body
    }

    pub const fn semantics(&self) -> &RuntimeProjectFunctionInstanceSemanticFacts {
        &self.semantics
    }

    pub const fn type_projection(&self) -> &[RuntimeProjectFunctionTypeProjection] {
        self.semantics.type_projection()
    }

    pub fn visit_type_projections<'facts>(
        &'facts self,
        visitor: &mut impl FnMut(&'facts RuntimeProjectFunctionTypeProjection),
    ) {
        self.semantics.visit_type_projections(visitor);
    }

    pub fn visit_captures<'facts>(
        &'facts self,
        visitor: &mut impl FnMut(&'facts RuntimeCheckedCapture),
    ) {
        self.semantics.visit_captures(visitor);
    }

    pub fn visit_closure_instances<'facts>(
        &'facts self,
        visitor: &mut impl FnMut(&'facts RuntimeClosureInstanceFact),
    ) {
        self.semantics.visit_closure_instances(visitor);
    }

    pub fn visit_dialogue_applications<'facts>(
        &'facts self,
        visitor: &mut impl FnMut(
            RuntimeScopedExecutableSemanticFactView<'facts>,
            ExprId,
            &'facts RuntimeDialogueApplication,
        ),
    ) {
        self.semantics.visit_dialogue_applications(
            RuntimeScopedExecutableSemanticFactView::project_function(&self.key, &self.semantics),
            visitor,
        );
    }

    pub fn visit_content_fragments<'facts>(
        &'facts self,
        visitor: &mut impl FnMut(
            RuntimeScopedExecutableSemanticFactView<'facts>,
            &'facts RuntimeContentFragmentFact,
        ),
    ) {
        self.semantics.visit_content_fragments(
            RuntimeScopedExecutableSemanticFactView::project_function(&self.key, &self.semantics),
            visitor,
        );
    }

    pub fn dialogue_content_fragment(
        &self,
        template: arcweft_core::runtime_id::RuntimeDialogueContentTemplateId,
    ) -> Option<&RuntimeContentFragmentFact> {
        self.semantics.dialogue_content_fragment(template)
    }

    pub fn visit_calls<'facts>(
        &'facts self,
        visitor: &mut impl FnMut(ExprId, &'facts RuntimeResolvedCall),
    ) {
        self.semantics.visit_calls(visitor);
    }

    pub fn visit_statement_owners(&self, visitor: &mut impl FnMut(StmtId)) {
        self.semantics.visit_statement_owners(visitor);
    }
}

fn parameter_binding_type_matches(
    kind: HirParameterKind,
    abi_ty: &RuntimeNormalizedType,
    binding_ty: &RuntimeNormalizedType,
) -> bool {
    match (kind, binding_ty.shape()) {
        (
            HirParameterKind::RestPositional,
            RuntimeTypeShape::Sequence {
                kind: RuntimeSequenceKind::Vec,
                item,
            },
        ) => item.as_ref() == abi_ty,
        (HirParameterKind::Fixed | HirParameterKind::ExtensionReceiver, _) => binding_ty == abi_ty,
        (HirParameterKind::RestPositional, _) => false,
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeProjectFunctionFactError {
    #[error("project-function fact does not name an ordinary Function declaration")]
    NotOrdinaryFunction,
    #[error("project-function fact has a non-Function checked ABI type")]
    InvalidFunctionType,
    #[error("project-function continuation does not advance to a later group")]
    InvalidGroupProgression,
    #[error("project-function continuation prefix ABI is not the exact checked group product")]
    InvalidContinuationAbi,
    #[error("project-function physical operands do not form one exact logical parameter row")]
    InvalidParameterMaterialization,
    #[error("project-function invocation and closed instance keys disagree")]
    InstanceKeyMismatch,
    #[error("project-function parameter ABI positions are not canonical")]
    NonCanonicalParameterAbi,
    #[error("project-function parameter ABI does not match its closed function type")]
    FunctionAbiMismatch,
    #[error("project-function effect row is not strictly ordered and unique")]
    NonCanonicalEffectRow,
    #[error("project-function execution family disagrees with its checked suspension/control role")]
    InvalidExecution,
    #[error("project-function type projection is not strictly owner-ordered and unique")]
    NonCanonicalTypeProjection,
    #[error("project-function semantic subcatalog does not match its sealed executable partition")]
    NonCanonicalSemanticFacts,
    #[error("project-function fact refers to a HIR owner in another module")]
    ForeignHirOwner,
    #[error("project-function fact omits a required substituted body or ABI type")]
    IncompleteTypeProjection,
    #[error("project-function closure instance disagrees with its checked producer or closed ABI")]
    InvalidClosureInstance,
    #[error("project-function attached default function disagrees with its checked instance")]
    InvalidAttachedDefaultFunction,
}
