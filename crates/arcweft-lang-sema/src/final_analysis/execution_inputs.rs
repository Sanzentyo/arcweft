//! Root-bound input evidence for value creation and body invocation.
//!
//! The existing eager execution fold owns membership. The shared free-local
//! collector authenticates external bindings, and the local-use catalog owns
//! value transfer and place access. This projection stores no second access index and never
//! treats a callable value's latent body as value-creation execution.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    expr::{HirExpressionChildOwnership, HirExpressionChildRole},
    identity::{ExprId, StmtId},
    project::{
        HirDeclarationBodyRootRole, HirExpressionEvaluationEdge, HirSemanticBodyLocator,
        HirSemanticBodyOwner, HirSemanticPathRoot,
    },
    scope::CaptureAccess,
};

use crate::{
    effects::EffectSet,
    semantic_coordinate::{
        CheckedLocalInputCoordinate, CheckedSemanticPath, SemanticCoordinateIndex,
        StableCheckedBodyCoordinate,
    },
    types::TypeKind,
};

use super::{
    CheckedExecutableCapture, CheckedExecutableControlRole, CheckedExecutionBodyOwner,
    CheckedExecutionSource, CheckedExpressionResolution, CheckedLocalAccess,
    CheckedLocalCopyEvidence, CheckedLocalCopyRequirement, CheckedLocalUseSite,
    CheckedSuspensionRole, FinalSemanticAnalysisError,
    execution_regions::CheckedExecutionRegion,
    free_capture::{CheckedCaptureExpression, CheckedFreeLocalCollector},
};

/// Intent is part of the stable root grammar. An implicit callable body has
/// its callable's accepted origin; it does not invent an HIR body container.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedExecutionCoordinate {
    Value(CheckedSemanticPath),
    Binding(CheckedSemanticPath),
    Iteration(CheckedSemanticPath),
    MatchSelection(CheckedSemanticPath),
    CallableBody(CheckedSemanticPath),
    MutationBody(CheckedSemanticPath),
    DeclarationBody(StableCheckedBodyCoordinate),
    DeclarationMutationBody(StableCheckedBodyCoordinate),
}

impl CheckedExecutionCoordinate {
    pub const fn path(&self) -> &CheckedSemanticPath {
        match self {
            Self::Value(path)
            | Self::Binding(path)
            | Self::Iteration(path)
            | Self::MatchSelection(path)
            | Self::CallableBody(path)
            | Self::MutationBody(path) => path,
            Self::DeclarationBody(body) | Self::DeclarationMutationBody(body) => body.path(),
        }
    }
}

/// Stable identity of an admitted execution definition and its selected intent.
/// This identifies a lexical definition, not its body contents or a closed
/// instance. Only accepted final-analysis execution projections issue these bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedExecutionDefinitionIdentity([u8; 32]);

impl CheckedExecutionDefinitionIdentity {
    pub(in crate::final_analysis) fn from_coordinate(
        coordinate: &CheckedExecutionCoordinate,
    ) -> Result<Self, crate::semantic_coordinate::SemanticCoordinateEncodingError> {
        let (tag, bytes) = match coordinate {
            CheckedExecutionCoordinate::Value(path) => (0u8, path.canonical_bytes()?),
            CheckedExecutionCoordinate::Binding(path) => (1, path.canonical_bytes()?),
            CheckedExecutionCoordinate::Iteration(path) => (2, path.canonical_bytes()?),
            CheckedExecutionCoordinate::MatchSelection(path) => (3, path.canonical_bytes()?),
            CheckedExecutionCoordinate::CallableBody(path) => (4, path.canonical_bytes()?),
            CheckedExecutionCoordinate::MutationBody(path) => (5, path.canonical_bytes()?),
            CheckedExecutionCoordinate::DeclarationBody(body) => (6, body.canonical_bytes()?),
            CheckedExecutionCoordinate::DeclarationMutationBody(body) => {
                (7, body.canonical_bytes()?)
            }
        };
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.lang.checked-execution-definition.v1\0");
        hasher.update(&[tag]);
        hasher.update(&bytes);
        Ok(Self(*hasher.finalize().as_bytes()))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Carries the accepted definition identity without hashing it again.
    pub const fn runtime_identity(&self) -> arcweft_core::plan::RuntimeFunctionDefinitionIdentity {
        arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(self.0)
    }
}

mod parameters;
pub use parameters::{
    CheckedExecutionParameter, CheckedExecutionParameterIdentity, CheckedExecutionParameterOrigin,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedExecutionInputRole {
    Free,
    Parameter(CheckedExecutionParameterOrigin),
}

/// One incoming binding and its selected value-transfer/place occurrences.
/// Unused formal bindings retain their ingress type with no fabricated use.
/// `CaptureAccess` retains a latent body's requirement; a creation transfer
/// does not itself perform that latent mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExecutionInput {
    role: CheckedExecutionInputRole,
    binding: CheckedExecutableCapture,
    uses: Box<[CheckedExecutionInputUse]>,
    copy_requirement: Option<CheckedLocalCopyRequirement>,
    copy_evidence: Option<CheckedLocalCopyEvidence>,
}

impl CheckedExecutionInput {
    pub const fn role(&self) -> &CheckedExecutionInputRole {
        &self.role
    }
    pub const fn binding(&self) -> &CheckedExecutableCapture {
        &self.binding
    }

    pub const fn uses(&self) -> &[CheckedExecutionInputUse] {
        &self.uses
    }

    /// Exact ingress obligation retained by the local-use owner. Copy mode
    /// does not discharge this requirement for callable/opaque carriers.
    pub const fn copy_requirement(&self) -> Option<&CheckedLocalCopyRequirement> {
        self.copy_requirement.as_ref()
    }

    pub const fn copy_evidence(&self) -> Option<CheckedLocalCopyEvidence> {
        self.copy_evidence
    }
}

/// A stable input occurrence and its exact generation-bound lowering evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExecutionInputUse {
    site: CheckedLocalUseSite,
    coordinate: CheckedLocalInputCoordinate,
    access: CheckedLocalAccess,
    latent_requirement: Option<CaptureAccess>,
}

/// Exact access evidence for `_` or a pipe-bound value without a HIR local.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExecutionSyntheticUse {
    expression: ExprId,
    coordinate: CheckedSemanticPath,
    access: super::CheckedSyntheticUse,
    copy_requirement: Option<super::CheckedSyntheticCopyRequirement>,
}

impl CheckedExecutionSyntheticUse {
    pub const fn expression(&self) -> ExprId {
        self.expression
    }
    pub const fn coordinate(&self) -> &CheckedSemanticPath {
        &self.coordinate
    }
    pub const fn access(&self) -> super::CheckedSyntheticUse {
        self.access
    }
    pub const fn copy_requirement(&self) -> Option<super::CheckedSyntheticCopyRequirement> {
        self.copy_requirement
    }
}

impl CheckedExecutionInputUse {
    pub const fn site(&self) -> CheckedLocalUseSite {
        self.site
    }

    pub const fn coordinate(&self) -> &CheckedLocalInputCoordinate {
        &self.coordinate
    }

    pub const fn access(&self) -> &CheckedLocalAccess {
        &self.access
    }

    pub const fn latent_requirement(&self) -> Option<CaptureAccess> {
        self.latent_requirement
    }
}

/// An exhaustive Match selector and the owned binding layout of every arm.
/// An ordinal and an ordinary structural Choice transport one initialized arm.
#[derive(Debug)]
pub struct CheckedExecutionMatchSelection {
    product: super::CheckedMatch,
    outputs: Box<[Box<[super::CheckedExecutableCapture]>]>,
}

impl CheckedExecutionMatchSelection {
    pub const fn checked_match(&self) -> &super::CheckedMatch {
        &self.product
    }
    pub const fn outputs(&self) -> &[Box<[super::CheckedExecutableCapture]>] {
        &self.outputs
    }
    pub fn payload_type(&self, arm: usize) -> Option<TypeKind> {
        let outputs = self.outputs.get(arm)?;
        Some(if outputs.is_empty() {
            TypeKind::Unit
        } else {
            TypeKind::Tuple(outputs.iter().map(|output| output.ty().clone()).collect())
        })
    }
}

/// Final-analysis-issued ABI for one selected execution boundary. Inputs and
/// occurrences are ordered by stable accepted coordinates, never `LocalId` or
/// expression arena position. The execution inventory expands the report's
/// shared eager DAG. The context closes input/result types and issues an owned
/// snapshot of its selected local-use certificates.
/// Callers cannot manufacture an accepted region.
#[derive(Debug)]
pub struct CheckedExecutionInputAbi {
    topology: std::sync::Arc<arcweft_lang_hir::project::HirProjectEvaluationTopology>,
    environment: std::sync::Arc<super::CheckedExecutionEnvironment>,
    source: CheckedExecutionSource,
    coordinate: CheckedExecutionCoordinate,
    definition_identity: CheckedExecutionDefinitionIdentity,
    execution: CheckedExecutionRegion,
    result: crate::callable::CallableResultSchema,
    effects: EffectSet,
    inputs: Box<[CheckedExecutionInput]>,
    parameters: Box<[CheckedExecutionParameter]>,
    synthetic_uses: Box<[CheckedExecutionSyntheticUse]>,
    binding_outputs: Box<[CheckedExecutableCapture]>,
    match_selection: Option<CheckedExecutionMatchSelection>,
}

impl CheckedExecutionInputAbi {
    /// Complete frame contract, including the lexical declaration quantifiers.
    pub fn function_type(&self) -> Option<TypeKind> {
        let scope = self.environment.type_scope();
        Some(TypeKind::Function {
            binder: scope
                .binders()
                .first()
                .copied()
                .unwrap_or(crate::types::GenericBinder::EMPTY),
            predicate: crate::effect_row::EffectPredicate::unconstrained(),
            params: self
                .inputs
                .iter()
                .filter(|input| input.role() == &CheckedExecutionInputRole::Free)
                .map(|input| input.binding().ty().clone())
                .chain(
                    self.parameters
                        .iter()
                        .map(|parameter| parameter.ty().clone()),
                )
                .collect(),
            return_type: Box::new(self.result.value_type()?.clone()),
            effects: crate::effect_row::EffectRow::closed(self.effects.clone()),
        })
    }
    pub const fn hir_topology(
        &self,
    ) -> &std::sync::Arc<arcweft_lang_hir::project::HirProjectEvaluationTopology> {
        &self.topology
    }
    /// Authenticates the accepted HIR allocation before a runtime consumer uses
    /// generation-local owners. Context validation additionally authenticates
    /// the callable registration and closed substitution.
    pub fn validate_project(
        &self,
        project: arcweft_lang_hir::project::HirAnalysisProjectView<'_>,
    ) -> Result<(), super::CheckedExecutionContextError> {
        self.environment.validate_project(project)
    }

    /// Rejects pairing the snapshot with another generation or substitution.
    pub fn validate_for(
        &self,
        context: &super::CheckedClosedExecutionContext<'_>,
    ) -> Result<(), super::CheckedExecutionContextError> {
        self.validate_analysis(context.analysis())?;
        if self.environment.instance_identity() != context.instance_identity() {
            return Err(super::CheckedExecutionContextError::InstanceMismatch);
        }
        context.admit_root(&self.source)
    }

    /// Authenticates the semantic report and its exact registered callable
    /// authority before another layer projects this admitted program.
    pub fn validate_analysis(
        &self,
        analysis: &super::FinalSemanticAnalysis,
    ) -> Result<(), super::CheckedExecutionContextError> {
        self.environment.validate_analysis(analysis)
    }

    pub const fn source(&self) -> &CheckedExecutionSource {
        &self.source
    }

    pub fn instance_identity(&self) -> Option<&super::CheckedLocalUseInstanceIdentity> {
        self.environment.instance_identity()
    }

    pub const fn environment(&self) -> &std::sync::Arc<super::CheckedExecutionEnvironment> {
        &self.environment
    }

    pub const fn coordinate(&self) -> &CheckedExecutionCoordinate {
        &self.coordinate
    }

    pub const fn definition_identity(&self) -> CheckedExecutionDefinitionIdentity {
        self.definition_identity
    }

    pub fn expressions(&self) -> &[ExprId] {
        self.execution.expressions()
    }

    /// Queries the selected DAG's sorted membership without another index.
    pub fn contains_expression(&self, owner: ExprId) -> bool {
        self.execution.expressions().binary_search(&owner).is_ok()
    }

    pub fn statements(&self) -> &[StmtId] {
        self.execution.statements()
    }

    pub fn places(&self) -> &[ExprId] {
        self.execution.places()
    }

    pub fn operations(&self) -> &[super::CheckedExecutionOperation] {
        self.execution.operations()
    }

    /// Canonical source-order owned outputs of a binding extraction.
    pub fn binding_outputs(&self) -> &[CheckedExecutableCapture] {
        &self.binding_outputs
    }

    pub const fn match_selection(&self) -> Option<&CheckedExecutionMatchSelection> {
        self.match_selection.as_ref()
    }

    pub const fn result(&self) -> &crate::callable::CallableResultSchema {
        &self.result
    }

    pub const fn effects(&self) -> &EffectSet {
        &self.effects
    }

    pub const fn suspension(&self) -> CheckedSuspensionRole {
        self.execution.suspension()
    }

    pub const fn control(&self) -> CheckedExecutableControlRole {
        self.execution.control()
    }

    pub const fn inputs(&self) -> &[CheckedExecutionInput] {
        &self.inputs
    }

    pub const fn parameters(&self) -> &[CheckedExecutionParameter] {
        &self.parameters
    }

    pub const fn synthetic_uses(&self) -> &[CheckedExecutionSyntheticUse] {
        &self.synthetic_uses
    }
}

impl super::CheckedClosedExecutionContext<'_> {
    /// Issues input evidence for a value-creation or body-invocation boundary.
    /// Formal arity, patterns, unused bindings and synthetic accesses remain
    /// separate from the selected runtime occurrences.
    /// Issuance requires exact source local-use certificates for every input.
    /// This input proof does not admit an extracted runtime program: its
    /// effect/control summary does not establish boundary-relative control
    /// safety or discharge the retained Copy ingress obligations.
    pub fn checked_execution_input_abi(
        &self,
        source: impl Into<CheckedExecutionSource>,
    ) -> Result<CheckedExecutionInputAbi, super::CheckedExecutionContextError> {
        let source = source.into();
        self.admit_root(&source)?;
        let analysis = self.analysis();
        let coordinates = SemanticCoordinateIndex::new(analysis.accepted_root_catalog(), analysis);
        let mut binding_outputs = Vec::new();
        let mut match_selection = None;
        let (execution, coordinate, input_scope, result, effects) = match &source {
            CheckedExecutionSource::EvaluateValue(owner) => {
                let expression = analysis.expression(*owner).ok_or(
                    FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: *owner },
                )?;
                let result = expression.value_type().ok_or(
                    FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: *owner },
                )?;
                let execution = analysis.expression_execution_region(*owner).ok_or(
                    FinalSemanticAnalysisError::ExpressionExecutionUnavailable { owner: *owner },
                )?;
                let path = coordinates
                    .expression(*owner)
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                (
                    execution,
                    CheckedExecutionCoordinate::Value(path.clone()),
                    path,
                    crate::callable::CallableResultSchema::Value(self.instantiate_type(result)?),
                    expression.effects().clone(),
                )
            }
            CheckedExecutionSource::SelectMatch(owner) => {
                static NOT_CANCELLED: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                let product = super::semantic_transcript::build_checked_match_transaction(
                    analysis,
                    self.project(),
                    *owner,
                    super::CheckedMatchLimits::PRODUCTION,
                    super::FinalSemanticAnalysisControl::new(&NOT_CANCELLED),
                )
                .map_err(|error| Box::new(super::CheckedSemanticTranscriptError::from(error)))?;
                let module = self
                    .project()
                    .modules()
                    .find(|(_, module)| module.module_id() == owner.module())
                    .map(|(_, module)| module)
                    .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                let authored = owner
                    .resolve(module)
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let outputs = authored
                    .arms()
                    .zip(product.arms())
                    .map(|(arm, checked)| {
                        let bindings = arm
                            .locals()
                            .iter()
                            .map(|&local| {
                                let binding = analysis.local(local).ok_or(
                                    FinalSemanticAnalysisError::LocalTypeUnavailable {
                                        owner: local,
                                    },
                                )?;
                                Ok(CheckedExecutableCapture::new(
                                    local,
                                    coordinates.binding(local).map_err(|_| {
                                        FinalSemanticAnalysisError::WrongPayloadFamily
                                    })?,
                                    self.instantiate_type(binding.ty())?,
                                ))
                            })
                            .collect::<Result<Vec<_>, super::CheckedExecutionContextError>>()?;
                        if bindings.len() != checked.bindings().len()
                            || bindings
                                .iter()
                                .zip(checked.bindings())
                                .any(|(binding, checked)| {
                                    !matches!(checked.coordinate(), crate::semantic_coordinate::StableCheckedValueCoordinate::Binding(origin) if origin == binding.origin())
                                        || !analysis.local(binding.local()).is_some_and(|local| local.ty().semantic_identity_digest().is_ok_and(|ty| ty == checked.ty()))
                                })
                        {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                        }
                        Ok(bindings.into_boxed_slice())
                    })
                    .collect::<Result<Vec<_>, super::CheckedExecutionContextError>>()?;
                let payload = outputs
                    .iter()
                    .map(|bindings| {
                        if bindings.is_empty() {
                            TypeKind::Unit
                        } else {
                            TypeKind::Tuple(
                                bindings
                                    .iter()
                                    .map(|binding| binding.ty().clone())
                                    .collect(),
                            )
                        }
                    })
                    .reduce(TypeKind::join_branch)
                    .unwrap_or(TypeKind::Never);
                let result = TypeKind::Tuple(vec![TypeKind::U32, payload]);
                let execution = analysis
                    .match_selection_execution_region(authored)
                    .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                let mut effects = EffectSet::new();
                for value in std::iter::once(authored.scrutinee())
                    .chain(authored.arms().filter_map(|arm| arm.guard()))
                {
                    effects.union_with(
                        analysis
                            .expression(value)
                            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
                            .effects(),
                    );
                }
                let path = product.path().clone();
                match_selection = Some(CheckedExecutionMatchSelection {
                    product,
                    outputs: outputs.into_boxed_slice(),
                });
                (
                    execution,
                    CheckedExecutionCoordinate::MatchSelection(path.clone()),
                    path,
                    crate::callable::CallableResultSchema::Value(result),
                    effects,
                )
            }
            CheckedExecutionSource::ExportBinding(owner)
            | CheckedExecutionSource::ExportIteration(owner) => {
                let module = self
                    .project()
                    .modules()
                    .find(|(_, module)| module.module_id() == owner.module())
                    .map(|(_, module)| module)
                    .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                let statement = module
                    .resolve_stmt(*owner)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                let locals = match statement.kind().evaluation_plan() {
                    arcweft_lang_hir::stmt::HirStmtEvaluationPlan::Binding { locals, .. } => locals,
                    arcweft_lang_hir::stmt::HirStmtEvaluationPlan::LetElse {
                        success_locals,
                        ..
                    } => success_locals,
                    arcweft_lang_hir::stmt::HirStmtEvaluationPlan::For {
                        branch_locals, ..
                    } if matches!(source, CheckedExecutionSource::ExportIteration(_)) => {
                        branch_locals
                    }
                    _ => return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into()),
                };
                let checked = analysis
                    .statement(*owner)
                    .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                let iteration = matches!(source, CheckedExecutionSource::ExportIteration(_));
                let execution = if iteration {
                    analysis.iteration_execution_region(*owner)
                } else {
                    analysis.statement_execution_region(*owner)
                }
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                let coordinate = coordinates
                    .statement(*owner)
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let path = coordinate.path().clone();
                for &local in locals {
                    let binding = analysis
                        .local(local)
                        .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local })?;
                    binding_outputs.push(CheckedExecutableCapture::new(
                        local,
                        coordinates
                            .binding(local)
                            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?,
                        self.instantiate_type(binding.ty())?,
                    ));
                }
                let payload = if binding_outputs.is_empty() {
                    TypeKind::Unit
                } else {
                    TypeKind::Tuple(
                        binding_outputs
                            .iter()
                            .map(|binding| binding.ty().clone())
                            .collect(),
                    )
                };
                let (result, coordinate, effects) = if iteration {
                    let arcweft_lang_hir::stmt::HirStmtKind::For(iteration) = statement.kind()
                    else {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                    };
                    let mut effects = EffectSet::new();
                    for expression in [
                        iteration.source(),
                        iteration.iterator(),
                        iteration.next_value(),
                    ] {
                        effects.union_with(
                            analysis
                                .expression(expression)
                                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
                                .effects(),
                        );
                    }
                    (
                        TypeKind::Vec(Box::new(payload)),
                        CheckedExecutionCoordinate::Iteration(path.clone()),
                        effects,
                    )
                } else {
                    (
                        payload,
                        CheckedExecutionCoordinate::Binding(path.clone()),
                        checked.effects().clone(),
                    )
                };
                (
                    execution,
                    coordinate,
                    path,
                    crate::callable::CallableResultSchema::Value(result),
                    effects,
                )
            }
            CheckedExecutionSource::InvokeBody(owner)
            | CheckedExecutionSource::ExportMutation(owner) => {
                let execution = analysis
                    .body_execution_region(owner)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let effects = analysis
                    .body_execution_effects(owner)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let (coordinate, input_scope, result) = match owner {
                    CheckedExecutionBodyOwner::CallableValue(owner) => {
                        let expression = analysis
                            .expression(*owner)
                            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                        let path = coordinates
                            .expression(*owner)
                            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                        let (scope, result) = match expression.resolution() {
                            CheckedExpressionResolution::ImplicitCallable(callable) => {
                                (path.clone(), self.instantiate_type(callable.result())?)
                            }
                            CheckedExpressionResolution::Closure(_) => {
                                let TypeKind::Function { return_type, .. } = expression
                                    .value_type()
                                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?
                                else {
                                    return Err(
                                        FinalSemanticAnalysisError::WrongPayloadFamily.into()
                                    );
                                };
                                let body = analysis
                                    .hir_topology()
                                    .expression_edges(*owner)
                                    .iter()
                                    .find_map(|edge| match edge {
                                        HirExpressionEvaluationEdge::Expression {
                                            role: HirExpressionChildRole::ClosureBody,
                                            ownership: HirExpressionChildOwnership::Owning,
                                            child,
                                        } => Some(*child),
                                        _ => None,
                                    })
                                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                                (
                                    coordinates.expression(body).map_err(|_| {
                                        FinalSemanticAnalysisError::WrongPayloadFamily
                                    })?,
                                    self.instantiate_type(return_type)?,
                                )
                            }
                            _ => return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into()),
                        };
                        (
                            if matches!(source, CheckedExecutionSource::ExportMutation(_)) {
                                CheckedExecutionCoordinate::MutationBody(path)
                            } else {
                                CheckedExecutionCoordinate::CallableBody(path)
                            },
                            scope,
                            crate::callable::CallableResultSchema::Value(result),
                        )
                    }
                    CheckedExecutionBodyOwner::Declaration { declaration, role } => {
                        let locator = HirSemanticBodyLocator::new(
                            HirSemanticPathRoot::Declaration(declaration.clone()),
                            HirSemanticBodyOwner::declaration(*role),
                        );
                        let body = coordinates
                            .body(&locator)
                            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                        let result = if matches!(role, HirDeclarationBodyRootRole::ViewValue { .. })
                        {
                            let topology = analysis
                                .hir_topology()
                                .declaration(declaration)
                                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                            let root = topology
                                .body()
                                .roots()
                                .iter()
                                .find(|root| root.role() == *role)
                                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                            let [edge] = root.projection().children() else {
                                return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                            };
                            let arcweft_lang_hir::body_edges::HirBodyChild::Expression(owner) =
                                edge.child()
                            else {
                                return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                            };
                            let ty = analysis
                                .expression(owner)
                                .and_then(super::CheckedExpression::value_type)
                                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                            crate::callable::CallableResultSchema::Value(self.instantiate_type(ty)?)
                        } else if matches!(role, HirDeclarationBodyRootRole::ViewStatement { .. }) {
                            crate::callable::CallableResultSchema::Value(TypeKind::Unit)
                        } else {
                            match analysis.checked_callables().project_callable(declaration).map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?.result_schema() {
                                crate::callable::CallableResultSchema::Value(ty) => crate::callable::CallableResultSchema::Value(self.instantiate_type(ty)?),
                                emission @ crate::callable::CallableResultSchema::ContentEmission(_) => emission.clone(),
                            }
                        };
                        (
                            if matches!(source, CheckedExecutionSource::ExportMutation(_)) {
                                CheckedExecutionCoordinate::DeclarationMutationBody(body.clone())
                            } else {
                                CheckedExecutionCoordinate::DeclarationBody(body.clone())
                            },
                            body.path().clone(),
                            result,
                        )
                    }
                };
                (execution, coordinate, input_scope, result, effects)
            }
        };
        let parameters = self.execution_parameters(&source, &coordinate)?;
        let parameter_roles = parameters
            .iter()
            .flat_map(|parameter| {
                parameter
                    .bindings()
                    .iter()
                    .map(|&local| (local, parameter.origin()))
            })
            .collect::<BTreeMap<_, _>>();
        if parameter_roles.len()
            != parameters
                .iter()
                .map(|parameter| parameter.bindings().len())
                .sum::<usize>()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
        }
        let mut projected = Vec::new();
        let mut types = BTreeMap::new();
        for &owner in execution.expressions() {
            projected.push(
                analysis
                    .checked_capture_inputs(owner)?
                    .close_input_types(self, &mut types)?,
            );
        }
        for &owner in execution.statements() {
            let statement = analysis
                .statement(owner)
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            projected.push(
                CheckedCaptureExpression::from_statement(owner, statement)?
                    .close_input_types(self, &mut types)?,
            );
        }
        let mut uses = BTreeMap::new();
        for &owner in execution.places() {
            let site = CheckedLocalUseSite::Place(owner);
            let access = self
                .local_uses()
                .access_at(site)
                .and_then(CheckedLocalAccess::place_access)
                .ok_or(FinalSemanticAnalysisError::ExpressionInputAccessUnavailable { site })?;
            if analysis
                .expression(owner)
                .and_then(super::CheckedExpression::mutable_place)
                .as_ref()
                != Some(access.place())
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
            }
            let ty = analysis
                .local(access.place().local_id())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?
                .ty();
            projected.push(
                CheckedCaptureExpression::from_place(owner, access.place(), ty)?
                    .close_input_types(self, &mut types)?,
            );
        }
        let mut collector = CheckedFreeLocalCollector::at_path(
            input_scope,
            &coordinates,
            |local| types.get(&local).cloned(),
            self.environment().type_scope(),
        );
        let mut sources = Vec::new();
        for projection in projected {
            collector.include_with_free_sources(projection, |source| sources.push(source))?;
        }
        for source in sources {
            let local_use = self.local_uses().access_at(source.site()).ok_or(
                FinalSemanticAnalysisError::ExpressionInputAccessUnavailable {
                    site: source.site(),
                },
            )?;
            if local_use.local() != source.local() {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
            }
            let coordinate = coordinates
                .local_input(source.site(), source.local())
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            uses.entry(source.local())
                .or_insert_with(Vec::new)
                .push(CheckedExecutionInputUse {
                    site: source.site(),
                    coordinate,
                    access: local_use.clone(),
                    latent_requirement: matches!(
                        source.site(),
                        CheckedLocalUseSite::Capture { .. }
                            | CheckedLocalUseSite::StatementCapture { .. }
                    )
                    .then_some(source.access()),
                });
        }
        let mut bindings = collector
            .finish()
            .into_vec()
            .into_iter()
            .map(|binding| (binding.local(), binding))
            .collect::<BTreeMap<_, _>>();
        for &local in parameter_roles.keys() {
            let ty = analysis
                .local(local)
                .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local })?
                .ty();
            bindings
                .entry(local)
                .or_insert(CheckedExecutableCapture::new(
                    local,
                    coordinates
                        .binding(local)
                        .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?,
                    self.instantiate_type(ty)?,
                ));
        }
        let mut inputs = Vec::new();
        for binding in bindings.into_values() {
            let mut occurrences = uses.remove(&binding.local()).unwrap_or_default();
            if occurrences.is_empty() && !parameter_roles.contains_key(&binding.local()) {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
            }
            occurrences.sort_by(|left, right| left.coordinate.cmp(&right.coordinate));
            if occurrences
                .windows(2)
                .any(|pair| pair[0].coordinate == pair[1].coordinate)
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
            }
            inputs.push(CheckedExecutionInput {
                role: parameter_roles
                    .get(&binding.local())
                    .map_or(CheckedExecutionInputRole::Free, |origin| {
                        CheckedExecutionInputRole::Parameter((*origin).clone())
                    }),
                copy_requirement: self.local_uses().copy_requirement(binding.local()).cloned(),
                copy_evidence: self.local_uses().copy_evidence(binding.local()),
                binding,
                uses: occurrences.into_boxed_slice(),
            });
        }
        if !uses.is_empty() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
        }
        inputs.sort_by(|left, right| left.binding.origin().cmp(right.binding.origin()));
        let result = if matches!(source, CheckedExecutionSource::ExportMutation(_)) {
            // Export each mutated free root once, in canonical ingress order.
            // Full checked field paths remain owned by the individual use rows.
            binding_outputs.extend(
                inputs
                    .iter()
                    .filter(|input| {
                        input.role() == &CheckedExecutionInputRole::Free
                            && input
                                .uses()
                                .iter()
                                .any(|usage| usage.access().place_access().is_some())
                    })
                    .map(|input| input.binding().clone()),
            );
            let crate::callable::CallableResultSchema::Value(value) = result else {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
            };
            crate::callable::CallableResultSchema::Value(TypeKind::Tuple(
                std::iter::once(value)
                    .chain(binding_outputs.iter().map(|binding| binding.ty().clone()))
                    .collect(),
            ))
        } else {
            result
        };
        let mut synthetic_owners = execution
            .expressions()
            .iter()
            .copied()
            .filter(|owner| {
                !matches!(
                    analysis
                        .expression(*owner)
                        .map(super::CheckedExpression::resolution),
                    Some(CheckedExpressionResolution::ImplicitCallable(_))
                )
            })
            .collect::<std::collections::BTreeSet<_>>();
        if let CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::CallableValue(owner))
        | CheckedExecutionSource::ExportMutation(CheckedExecutionBodyOwner::CallableValue(
            owner,
        )) = &source
        {
            synthetic_owners.insert(*owner);
        }
        let mut synthetic_uses = synthetic_owners
            .into_iter()
            .filter_map(|expression| {
                self.local_uses()
                    .synthetic_at(expression)
                    .map(|access| (expression, access))
            })
            .map(|(expression, access)| {
                let coordinate = coordinates
                    .expression(expression)
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let copy_requirement = match access.owner() {
                    super::CheckedSyntheticUseOwner::ImplicitParameter(owner) => {
                        self.local_uses().synthetic_copy_requirement(owner)
                    }
                    super::CheckedSyntheticUseOwner::Pipe(_) => None,
                };
                Ok(CheckedExecutionSyntheticUse {
                    expression,
                    coordinate,
                    access,
                    copy_requirement,
                })
            })
            .collect::<Result<Vec<_>, FinalSemanticAnalysisError>>()?;
        synthetic_uses.sort_by(|left, right| left.coordinate.cmp(&right.coordinate));
        if synthetic_uses
            .windows(2)
            .any(|pair| pair[0].coordinate == pair[1].coordinate)
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
        }
        let definition_identity = CheckedExecutionDefinitionIdentity::from_coordinate(&coordinate)?;
        Ok(CheckedExecutionInputAbi {
            topology: std::sync::Arc::clone(analysis.hir_topology()),
            environment: std::sync::Arc::clone(self.environment()),
            source,
            coordinate,
            definition_identity,
            execution,
            result,
            effects,
            inputs: inputs.into_boxed_slice(),
            parameters,
            synthetic_uses: synthetic_uses.into_boxed_slice(),
            binding_outputs: binding_outputs.into_boxed_slice(),
            match_selection,
        })
    }
}
