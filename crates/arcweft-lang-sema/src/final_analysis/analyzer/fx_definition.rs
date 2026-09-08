//! Sema-owned sealing of project `#[fx]` definition bodies.
//!
//! This pass is deliberately earlier than runtime/compiler lowering. It
//! consumes final HIR together with the accepted callable/type authority and
//! publishes a HIR-free symbolic body. Compiler consumers must not reopen an
//! `ExprId` or reconstruct a call selection from rejected tooling facts.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::{
    expr::{HirCallArgument, HirCallInvocation, HirExprKind, HirRecordField, HirSelectedMember},
    identity::{ExprId, HirModuleId, LocalId},
    item::{HirFunctionBody, HirItemKind},
    pattern::{HirPatternBinding, HirPatternKind},
    project::{
        HirExpressionEvaluationEdge, HirProjectEvaluationTopology, HirSemanticPathOwnerId,
        HirSemanticPathRoot,
    },
    symbol::CallableDeclarationKey,
};
use arcweft_presentation::fx::{
    BUILTIN_FX_CALLABLE_CATALOG, BuiltinFxActiveAbiParameterSet, BuiltinFxCallableParameter,
    BuiltinFxCallableRow, BuiltinFxCallableRowId, BuiltinFxDefaultValue, BuiltinFxParameterBinding,
    BuiltinFxParameterId, BuiltinFxParameterPresence, BuiltinFxParameterType,
    BuiltinFxSpecialization, BuiltinFxValueConstraint, FiniteF32, FxContextSlot,
    FxDefinitionArgumentValue, FxDefinitionParameterRef, FxDefinitionParameterSchema,
    FxDefinitionParameterType, FxFontFamilyName, FxGraph, FxId, FxNode, FxPhase, FxRuntimeType,
    FxRuntimeValue, FxSamplerProgram, FxSelectorDomain, FxSelectorId, FxShaderStage,
    FxSourceConstructor, FxSourceParameterPassing, FxSourceParameterPresence,
    FxSourceParameterType, FxStaticType, FxTarget, FxVec2, Length, MotionFunction, Transform2D,
    ValueInstruction, ValueProgramSchema, build_builtin_fx_definition,
};

use crate::{
    callable::{
        BuiltinCallableId, CallableCandidateId, CallableParameterPassing,
        CallableParameterPresence, VectorDimensions,
    },
    types::{CompileTimeFxType, CompileTimeScalarKind, TypeKind},
};

use super::Analyzer;
use crate::final_analysis::fx_application::{
    builtin_fx_presence_default, checked_builtin_fx_binding, checked_project_fx_argument,
    checked_project_fx_parameter_type,
};
use crate::final_analysis::{
    CheckedFxArgument, CheckedFxBindingDecision, CheckedFxBody, CheckedFxBodyCall,
    CheckedFxConstant, CheckedFxConstructorArgument, CheckedFxConstructorArgumentValue,
    CheckedFxConstructorCall, CheckedFxDefinition, CheckedFxDefinitionCatalog,
    CheckedFxDefinitionRef, CheckedFxDefinitionSealError, CheckedFxGraphExpression,
    CheckedFxSourceParameter, CheckedFxSymbolicValue, CheckedProjectFxDefinition,
    CheckedSymbolicFxBinding, FinalSemanticAnalysisError, PreparedExpressionFact,
};

#[derive(Clone)]
struct PreparedProjectFxDefinition {
    declaration: CallableDeclarationKey,
    definition: FxId,
    schema: crate::callable::CallableSignatureSchemaDigest,
    parameter_schema: FxDefinitionParameterSchema,
    module: HirModuleId,
    body: ExprId,
    parameters: BTreeMap<LocalId, FxDefinitionParameterRef>,
    parameter_presence: Vec<CallableParameterPresence>,
    runtime_parameter_types: Vec<FxRuntimeType>,
}

struct CheckedGraph {
    expression: CheckedFxGraphExpression,
    nodes: usize,
    visits: usize,
    depth: usize,
}

/// Transient proof that these final-HIR expression owners are wholly owned by
/// successfully sealed project Fx bodies. Raw HIR identities deliberately do
/// not enter [`CheckedFxDefinitionCatalog`]; this proof exists only until the
/// selected-expression graph has discharged the alternate carrier boundary.
#[derive(Debug, Default)]
pub(in crate::final_analysis) struct PreparedFxDefinitionBodyObligations {
    declarations: BTreeSet<CallableDeclarationKey>,
    expressions: BTreeSet<ExprId>,
}

impl PreparedFxDefinitionBodyObligations {
    fn seal(
        topology: &HirProjectEvaluationTopology,
        declarations: &BTreeSet<CallableDeclarationKey>,
        roots: impl IntoIterator<Item = ExprId>,
    ) -> Result<Self, CheckedFxDefinitionSealError> {
        let mut expressions = BTreeSet::new();
        let mut pending = roots.into_iter().collect::<Vec<_>>();
        while let Some(owner) = pending.pop() {
            if !expressions.insert(owner) {
                continue;
            }
            let location = topology
                .semantic_path(HirSemanticPathOwnerId::Expression(owner))
                .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?
                .ok_or(CheckedFxDefinitionSealError::OwnerInvariant)?;
            let HirSemanticPathRoot::Declaration(declaration) = location.root() else {
                return Err(CheckedFxDefinitionSealError::OwnerInvariant);
            };
            if !declarations.contains(declaration)
                || !super::executable_ingress::executable_body_path(location.path().steps())
            {
                return Err(CheckedFxDefinitionSealError::OwnerInvariant);
            }
            pending.extend(topology.expression_edges(owner).iter().filter_map(|edge| {
                (!matches!(
                    edge,
                    HirExpressionEvaluationEdge::Expression {
                        ownership:
                            arcweft_lang_hir::expr::HirExpressionChildOwnership::ReferenceOnly,
                        ..
                    }
                ))
                .then_some(edge.child())
            }));
        }
        Ok(Self {
            declarations: declarations.clone(),
            expressions,
        })
    }

    pub(in crate::final_analysis) fn contains(&self, owner: ExprId) -> bool {
        self.expressions.contains(&owner)
    }

    pub(in crate::final_analysis) fn owners(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.expressions.iter().copied()
    }

    pub(in crate::final_analysis) fn declarations(
        &self,
    ) -> impl Iterator<Item = &CallableDeclarationKey> {
        self.declarations.iter()
    }
}

enum FxGraphCallIdentity {
    Constructor(FxSourceConstructor),
    Builtin(BuiltinFxCallableRowId),
    Project(FxId),
}

struct FxDefinitionSealer<'analyzer, 'project, 'catalog, 'control> {
    analyzer: &'analyzer mut Analyzer<'project, 'catalog, 'control>,
    prepared: BTreeMap<CallableDeclarationKey, PreparedProjectFxDefinition>,
    by_id: BTreeMap<FxId, CallableDeclarationKey>,
    catalog: CheckedFxDefinitionCatalog,
    project_cache: BTreeMap<CallableDeclarationKey, CheckedProjectFxDefinition>,
    stack: Vec<CallableDeclarationKey>,
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn seal_fx_definition_catalog(
        &mut self,
        declarations: &[CallableDeclarationKey],
    ) -> Result<
        (
            CheckedFxDefinitionCatalog,
            PreparedFxDefinitionBodyObligations,
        ),
        FinalSemanticAnalysisError,
    > {
        let mut prepared = BTreeMap::new();
        let mut by_id = BTreeMap::new();
        for declaration in declarations {
            let definition = self
                .prepare_project_fx_definition(declaration)
                .map_err(|cause| FinalSemanticAnalysisError::FxDefinition {
                    declaration: declaration.clone(),
                    cause,
                })?;
            if by_id
                .insert(definition.definition.clone(), declaration.clone())
                .is_some()
                || prepared.insert(declaration.clone(), definition).is_some()
            {
                return Err(FinalSemanticAnalysisError::FxDefinition {
                    declaration: declaration.clone(),
                    cause: CheckedFxDefinitionSealError::InvalidSchema,
                });
            }
        }
        let declaration_set = declarations.iter().cloned().collect::<BTreeSet<_>>();
        let obligations = PreparedFxDefinitionBodyObligations::seal(
            &self.topology,
            &declaration_set,
            prepared.values().map(|definition| definition.body),
        );
        let obligations = match obligations {
            Ok(obligations) => obligations,
            Err(cause) => {
                let declaration = declarations
                    .first()
                    .cloned()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                return Err(FinalSemanticAnalysisError::FxDefinition { declaration, cause });
            }
        };
        let mut sealer = FxDefinitionSealer {
            analyzer: self,
            prepared,
            by_id,
            catalog: CheckedFxDefinitionCatalog::default(),
            project_cache: BTreeMap::new(),
            stack: Vec::new(),
        };
        for declaration in declarations {
            sealer.seal_project(declaration).map_err(|cause| {
                FinalSemanticAnalysisError::FxDefinition {
                    declaration: declaration.clone(),
                    cause,
                }
            })?;
        }
        Ok((sealer.catalog, obligations))
    }

    fn prepare_project_fx_definition(
        &mut self,
        declaration: &CallableDeclarationKey,
    ) -> Result<PreparedProjectFxDefinition, CheckedFxDefinitionSealError> {
        let symbol = self
            .symbols
            .callable(declaration)
            .filter(|symbol| symbol.is_fx())
            .ok_or(CheckedFxDefinitionSealError::MissingDeclaration)?;
        let definition =
            FxId::try_new(declaration.package().as_str(), declaration.qualified_name())
                .map_err(|_| CheckedFxDefinitionSealError::InvalidSchema)?;
        let callable_schema = self
            .staged_callables
            .as_ref()
            .and_then(|staged| staged.accepted.project_record(declaration))
            .map(|record| record.schema().clone())
            .ok_or(CheckedFxDefinitionSealError::InvalidSchema)?;
        let [group] = callable_schema.groups() else {
            return Err(CheckedFxDefinitionSealError::InvalidSchema);
        };
        let module_id = symbol.source_item().module();
        let module = self
            .module(module_id)
            .map_err(|_| CheckedFxDefinitionSealError::MissingDeclaration)?;
        let item = module
            .resolve_item(symbol.source_item())
            .map_err(|_| CheckedFxDefinitionSealError::MissingDeclaration)?;
        let HirItemKind::Function(function) = item.kind() else {
            return Err(CheckedFxDefinitionSealError::InvalidSchema);
        };
        let [hir_group] = function.parameter_groups() else {
            return Err(CheckedFxDefinitionSealError::InvalidSchema);
        };
        if group.parameters().len() != hir_group.parameters().len() {
            return Err(CheckedFxDefinitionSealError::InvalidSchema);
        }
        let body = match function.body() {
            HirFunctionBody::Block {
                statements, tail, ..
            } if statements.is_empty() => *tail,
            HirFunctionBody::Block { .. } => {
                return Err(CheckedFxDefinitionSealError::InvalidBody);
            }
            HirFunctionBody::Error(_) => return Err(CheckedFxDefinitionSealError::InvalidBody),
        };
        let mut parameters = BTreeMap::new();
        let mut parameter_presence = Vec::with_capacity(group.parameters().len());
        let mut definition_parameters = Vec::with_capacity(group.parameters().len());
        for (index, (parameter, hir_parameter)) in group
            .parameters()
            .iter()
            .zip(hir_group.parameters())
            .enumerate()
        {
            if parameter.passing() != CallableParameterPassing::NamedOnly
                || parameter.index().get() != index
            {
                return Err(CheckedFxDefinitionSealError::InvalidSchema);
            }
            let [local] = hir_parameter.locals() else {
                return Err(CheckedFxDefinitionSealError::InvalidSchema);
            };
            let name = parameter
                .name()
                .ok_or(CheckedFxDefinitionSealError::InvalidSchema)?;
            let declared = parameter
                .declared_type()
                .ok_or(CheckedFxDefinitionSealError::InvalidSchema)?;
            let parameter_type = checked_project_fx_parameter_type(declared)
                .ok_or(CheckedFxDefinitionSealError::InvalidSchema)?;
            let default = hir_parameter
                .default()
                .map(|source| {
                    self.check_expression_published(source, Some(declared))
                        .map_err(|_| CheckedFxDefinitionSealError::InvalidValue {
                            owner: source,
                            expected: parameter_type.static_type(),
                        })?;
                    let value = self
                        .checked_compile_time_value(module, source, declared)
                        .map_err(|_| CheckedFxDefinitionSealError::InvalidValue {
                            owner: source,
                            expected: parameter_type.static_type(),
                        })?;
                    checked_project_fx_argument(parameter_type, &value).map_err(|_| {
                        CheckedFxDefinitionSealError::InvalidValue {
                            owner: source,
                            expected: parameter_type.static_type(),
                        }
                    })
                })
                .transpose()?;
            let definition_parameter = arcweft_presentation::fx::FxDefinitionParameter::try_new(
                index,
                name.as_str(),
                parameter_type,
                default,
            )
            .map_err(|_| CheckedFxDefinitionSealError::InvalidSchema)?;
            if parameters
                .insert(*local, definition_parameter.parameter_ref())
                .is_some()
            {
                return Err(CheckedFxDefinitionSealError::InvalidSchema);
            }
            definition_parameters.push(definition_parameter);
            parameter_presence.push(parameter.presence());
        }
        let parameter_schema =
            FxDefinitionParameterSchema::new(definition.clone(), definition_parameters)
                .map_err(|_| CheckedFxDefinitionSealError::InvalidSchema)?;
        let runtime_parameter_types = parameter_schema
            .parameter_layout()
            .runtime_rows()
            .iter()
            .map(|row| row.reference().runtime_type())
            .collect();
        Ok(PreparedProjectFxDefinition {
            declaration: declaration.clone(),
            definition,
            schema: callable_schema.semantic_digest(),
            parameter_schema,
            module: module_id,
            body,
            parameters,
            parameter_presence,
            runtime_parameter_types,
        })
    }
}

impl FxDefinitionSealer<'_, '_, '_, '_> {
    fn seal_project(
        &mut self,
        declaration: &CallableDeclarationKey,
    ) -> Result<CheckedProjectFxDefinition, CheckedFxDefinitionSealError> {
        if let Some(definition) = self.project_cache.get(declaration) {
            return Ok(definition.clone());
        }
        let prepared = self
            .prepared
            .get(declaration)
            .cloned()
            .ok_or(CheckedFxDefinitionSealError::MissingDeclaration)?;
        if self.stack.iter().any(|active| active == declaration) {
            return Err(CheckedFxDefinitionSealError::DependencyCycle {
                definition: prepared.definition,
            });
        }
        self.stack.push(declaration.clone());
        let module = self
            .analyzer
            .module(prepared.module)
            .map_err(|_| CheckedFxDefinitionSealError::MissingDeclaration)?;
        let module_id = module.module_id();
        let graph = self.seal_graph_expression(
            module_id,
            prepared.body,
            &prepared.parameters,
            &prepared.runtime_parameter_types,
            1,
        );
        self.stack.pop();
        let graph = graph?;
        validate_limits(&graph)?;
        let body = CheckedFxBody::seal(
            graph.expression,
            u32::try_from(graph.nodes)
                .map_err(|_| CheckedFxDefinitionSealError::AccountingOverflow)?,
            u32::try_from(graph.visits)
                .map_err(|_| CheckedFxDefinitionSealError::AccountingOverflow)?,
            u16::try_from(graph.depth)
                .map_err(|_| CheckedFxDefinitionSealError::AccountingOverflow)?,
        )?;
        let definition = CheckedProjectFxDefinition::new(
            prepared.declaration,
            prepared.definition,
            prepared.schema,
            prepared.parameter_schema,
            body,
        )
        .ok_or(CheckedFxDefinitionSealError::InvalidSchema)?;
        self.catalog
            .insert(CheckedFxDefinition::Project(definition.clone()))?;
        self.project_cache
            .insert(declaration.clone(), definition.clone());
        Ok(definition)
    }

    fn seal_graph_expression(
        &mut self,
        module_id: HirModuleId,
        owner: ExprId,
        parameters: &BTreeMap<LocalId, FxDefinitionParameterRef>,
        runtime_parameter_types: &[FxRuntimeType],
        depth: usize,
    ) -> Result<CheckedGraph, CheckedFxDefinitionSealError> {
        check_depth(depth)?;
        let module = self
            .analyzer
            .module(module_id)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        let expression = module
            .resolve_expr(owner)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        if let HirExprKind::Block(block) = expression.kind() {
            return self.seal_graph_expression(
                module_id,
                block.tail(),
                parameters,
                runtime_parameter_types,
                depth,
            );
        }
        let HirExprKind::Call(call) = expression.kind() else {
            return Err(CheckedFxDefinitionSealError::UnsupportedExpression { owner });
        };
        match self.classify_graph_call(module_id, owner, call)? {
            FxGraphCallIdentity::Constructor(constructor) => self.seal_constructor(
                module_id,
                owner,
                call,
                constructor,
                parameters,
                runtime_parameter_types,
                depth,
            ),
            FxGraphCallIdentity::Builtin(row) => self.seal_builtin(
                module_id,
                owner,
                call,
                row,
                parameters,
                runtime_parameter_types,
                depth,
            ),
            FxGraphCallIdentity::Project(definition) => self.seal_project_call(
                module_id,
                owner,
                call,
                &definition,
                parameters,
                runtime_parameter_types,
                depth,
            ),
        }
    }

    fn classify_graph_call(
        &mut self,
        module_id: HirModuleId,
        owner: ExprId,
        call: &HirCallInvocation,
    ) -> Result<FxGraphCallIdentity, CheckedFxDefinitionSealError> {
        if let Some(facts) = self.analyzer.facts.calls().get(&owner) {
            let mut identities = facts
                .outcome()
                .candidate_ids()
                .filter_map(|candidate| match candidate {
                    CallableCandidateId::FxConstructor(constructor) => {
                        Some(FxGraphCallIdentity::Constructor(*constructor))
                    }
                    CallableCandidateId::Project(declaration) => {
                        self.prepared.get(declaration).map(|definition| {
                            FxGraphCallIdentity::Project(definition.definition.clone())
                        })
                    }
                    candidate => self
                        .analyzer
                        .staged_callables
                        .as_ref()
                        .and_then(|staged| staged.accepted.record(candidate))
                        .and_then(|record| match record.schema().validator() {
                            crate::callable::CallableValidator::BuiltinFx(row) => {
                                Some(FxGraphCallIdentity::Builtin(*row))
                            }
                            _ => None,
                        }),
                })
                .collect::<Vec<_>>();
            identities.sort_by_key(|identity| match identity {
                FxGraphCallIdentity::Constructor(value) => (0, value.semantic_tag(), 0),
                FxGraphCallIdentity::Builtin(value) => (1, value.semantic_tag(), 0),
                FxGraphCallIdentity::Project(value) => {
                    let digest = blake3::hash(value.function().as_bytes());
                    (2, digest.as_bytes()[0], digest.as_bytes()[1])
                }
            });
            identities.dedup_by(|left, right| match (&*left, &*right) {
                (
                    FxGraphCallIdentity::Constructor(left),
                    FxGraphCallIdentity::Constructor(right),
                ) => left == right,
                (FxGraphCallIdentity::Builtin(left), FxGraphCallIdentity::Builtin(right)) => {
                    left == right
                }
                (FxGraphCallIdentity::Project(left), FxGraphCallIdentity::Project(right)) => {
                    left == right
                }
                _ => false,
            });
            if let [identity] = identities.as_slice() {
                return Ok(match identity {
                    FxGraphCallIdentity::Constructor(value) => {
                        FxGraphCallIdentity::Constructor(*value)
                    }
                    FxGraphCallIdentity::Builtin(value) => FxGraphCallIdentity::Builtin(*value),
                    FxGraphCallIdentity::Project(value) => {
                        FxGraphCallIdentity::Project(value.clone())
                    }
                });
            }
            let builtin_rows = identities
                .iter()
                .filter_map(|identity| match identity {
                    FxGraphCallIdentity::Builtin(row) => Some(*row),
                    FxGraphCallIdentity::Constructor(_) | FxGraphCallIdentity::Project(_) => None,
                })
                .collect::<Vec<_>>();
            if !builtin_rows.is_empty() {
                return self
                    .select_builtin_row(module_id, owner, call, &builtin_rows)
                    .map(FxGraphCallIdentity::Builtin);
            }
        }
        if let Some(constructor) = self.constructor_from_callee(module_id, owner, call)? {
            return Ok(FxGraphCallIdentity::Constructor(constructor));
        }
        if let Some(callable) = self.builtin_callable_from_callee(module_id, call)? {
            let rows = BUILTIN_FX_CALLABLE_CATALOG
                .rows_for(callable)
                .map(|row| row.id())
                .collect::<Vec<_>>();
            return self
                .select_builtin_row(module_id, owner, call, &rows)
                .map(FxGraphCallIdentity::Builtin);
        }
        Err(CheckedFxDefinitionSealError::UnsupportedExpression { owner })
    }

    fn constructor_from_callee(
        &self,
        module_id: HirModuleId,
        owner: ExprId,
        call: &HirCallInvocation,
    ) -> Result<Option<FxSourceConstructor>, CheckedFxDefinitionSealError> {
        let Some((receiver, _, member)) = call.callee().associated_parts() else {
            return Ok(None);
        };
        let member = member
            .resolved()
            .ok_or(CheckedFxDefinitionSealError::InvalidCallShape { owner })?;
        let receiver_is_fx = receiver
            .type_id()
            .and_then(|owner| self.analyzer.types.get(&owner))
            .is_some_and(|ty| {
                matches!(ty, TypeKind::CompileTimeFx(CompileTimeFxType::Abstract))
                    || matches!(ty, TypeKind::MetaType(inner) if matches!(inner.as_ref(), TypeKind::CompileTimeFx(CompileTimeFxType::Abstract)))
                    || matches!(ty, TypeKind::Named(name) if name == "Fx")
            })
            || receiver.type_id().is_some_and(|owner| {
                self.analyzer
                    .module(module_id)
                    .ok()
                    .and_then(|module| module.resolve_type(owner).ok())
                    .is_some_and(|ty| {
                        matches!(
                            ty.kind(),
                            arcweft_lang_hir::type_ref::HirTypeKind::Path(path)
                                if path.lexical_name() == Some("Fx")
                        )
                    })
            });
        if receiver_is_fx {
            Ok(FxSourceConstructor::from_source_name(member.as_str()))
        } else {
            Ok(None)
        }
    }

    fn builtin_callable_from_callee(
        &self,
        module_id: HirModuleId,
        call: &HirCallInvocation,
    ) -> Result<Option<arcweft_presentation::fx::BuiltinFxCallableId>, CheckedFxDefinitionSealError>
    {
        let Some(value) = call.callee().value_expression() else {
            return Ok(None);
        };
        let module = self
            .analyzer
            .module(module_id)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        let expression = module
            .resolve_expr(value)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        let HirExprKind::Path(path) = expression.kind() else {
            return Ok(None);
        };
        let Some(path) = path.as_resolved() else {
            return Ok(None);
        };
        let [segment] = path.segments() else {
            return Ok(None);
        };
        let name = match segment {
            arcweft_lang_hir::leaf::HirPathSegment::Identifier(name) => name.as_str(),
            arcweft_lang_hir::leaf::HirPathSegment::ProjectSymbol(name) => name.as_str(),
        };
        Ok(BUILTIN_FX_CALLABLE_CATALOG.resolve(name))
    }

    fn select_builtin_row(
        &mut self,
        module_id: HirModuleId,
        owner: ExprId,
        call: &HirCallInvocation,
        rows: &[BuiltinFxCallableRowId],
    ) -> Result<BuiltinFxCallableRowId, CheckedFxDefinitionSealError> {
        let phase_source = call.arguments().iter().find_map(|argument| {
            (argument.resolved_name().map(|name| name.as_str()) == Some("phase"))
                .then_some(argument.value())
        });
        let selected = if let Some(source) = phase_source {
            let phase = self.seal_phase(module_id, source)?;
            rows.iter()
                .copied()
                .filter(|row| row.phase() == phase)
                .collect::<Vec<_>>()
        } else {
            rows.iter()
                .copied()
                .filter(|row| row.is_default_phase())
                .collect::<Vec<_>>()
        };
        let [selected] = selected.as_slice() else {
            return Err(CheckedFxDefinitionSealError::InvalidCallShape { owner });
        };
        Ok(*selected)
    }

    fn seal_phase(
        &mut self,
        module_id: HirModuleId,
        owner: ExprId,
    ) -> Result<FxPhase, CheckedFxDefinitionSealError> {
        if let Some(value) = self
            .checked_closed_enum(owner)
            .and_then(FxPhase::from_value_id)
        {
            return Ok(value);
        }
        let module = self
            .analyzer
            .module(module_id)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        if let HirExprKind::ShortVariant(name) = module
            .resolve_expr(owner)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?
            .kind()
        {
            return name
                .as_resolved()
                .and_then(|name| FxPhase::from_source_name(name.as_str()))
                .ok_or(CheckedFxDefinitionSealError::InvalidValue {
                    owner,
                    expected: FxStaticType::Phase,
                });
        }
        let expected = TypeKind::CompileTimeEnum(crate::types::CompileTimeEnumType::any(
            arcweft_presentation::fx::FxEnumDomain::Phase.domain_id(),
        ));
        self.analyzer
            .check_expression_published(owner, Some(&expected))
            .map_err(|_| CheckedFxDefinitionSealError::InvalidValue {
                owner,
                expected: FxStaticType::Phase,
            })?;
        self.checked_closed_enum(owner)
            .and_then(FxPhase::from_value_id)
            .ok_or(CheckedFxDefinitionSealError::InvalidValue {
                owner,
                expected: FxStaticType::Phase,
            })
    }

    fn seal_target(
        &mut self,
        module_id: HirModuleId,
        owner: ExprId,
    ) -> Result<FxTarget, CheckedFxDefinitionSealError> {
        if let Some(value) = self
            .checked_closed_enum(owner)
            .and_then(FxTarget::from_value_id)
        {
            return Ok(value);
        }
        let module = self
            .analyzer
            .module(module_id)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        if let HirExprKind::ShortVariant(name) = module
            .resolve_expr(owner)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?
            .kind()
        {
            return name
                .as_resolved()
                .and_then(|name| FxTarget::from_source_name(name.as_str()))
                .ok_or(CheckedFxDefinitionSealError::InvalidValue {
                    owner,
                    expected: FxStaticType::Target,
                });
        }
        let expected = TypeKind::CompileTimeEnum(crate::types::CompileTimeEnumType::any(
            arcweft_presentation::fx::FxEnumDomain::Target.domain_id(),
        ));
        self.analyzer
            .check_expression_published(owner, Some(&expected))
            .map_err(|_| CheckedFxDefinitionSealError::InvalidValue {
                owner,
                expected: FxStaticType::Target,
            })?;
        self.checked_closed_enum(owner)
            .and_then(FxTarget::from_value_id)
            .ok_or(CheckedFxDefinitionSealError::InvalidValue {
                owner,
                expected: FxStaticType::Target,
            })
    }

    fn seal_motion_function(
        &mut self,
        module_id: HirModuleId,
        owner: ExprId,
    ) -> Result<MotionFunction, CheckedFxDefinitionSealError> {
        if let Some(value) = self
            .checked_closed_enum(owner)
            .and_then(MotionFunction::from_value_id)
        {
            return Ok(value);
        }
        let module = self
            .analyzer
            .module(module_id)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        if let HirExprKind::ShortVariant(name) = module
            .resolve_expr(owner)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?
            .kind()
        {
            return name
                .as_resolved()
                .and_then(|name| MotionFunction::from_source_name(name.as_str()))
                .ok_or(CheckedFxDefinitionSealError::InvalidValue {
                    owner,
                    expected: FxStaticType::Target,
                });
        }
        let expected = TypeKind::CompileTimeEnum(crate::types::CompileTimeEnumType::any(
            arcweft_presentation::fx::FxEnumDomain::MotionFunction.domain_id(),
        ));
        self.analyzer
            .check_expression_published(owner, Some(&expected))
            .map_err(|_| CheckedFxDefinitionSealError::InvalidBody)?;
        self.checked_closed_enum(owner)
            .and_then(MotionFunction::from_value_id)
            .ok_or(CheckedFxDefinitionSealError::InvalidBody)
    }

    fn checked_closed_enum(
        &self,
        owner: ExprId,
    ) -> Option<arcweft_id::closed_enum::ClosedEnumValueId> {
        match self
            .analyzer
            .facts
            .expressions()
            .get(&owner)?
            .checked_resolution()?
        {
            crate::final_analysis::CheckedExpressionResolution::CompileTimeEnum(value) => {
                Some(*value)
            }
            _ => None,
        }
    }

    fn seal_constructor(
        &mut self,
        module_id: HirModuleId,
        owner: ExprId,
        call: &HirCallInvocation,
        constructor: FxSourceConstructor,
        parameters: &BTreeMap<LocalId, FxDefinitionParameterRef>,
        runtime_parameter_types: &[FxRuntimeType],
        depth: usize,
    ) -> Result<CheckedGraph, CheckedFxDefinitionSealError> {
        let mapped = map_constructor_arguments(owner, call, constructor)?;
        let mut arguments = Vec::with_capacity(mapped.len());
        let mut nodes = 1usize;
        let mut visits = 1usize;
        let mut graph_depth = 1usize;
        for (index, source) in mapped {
            let schema = constructor
                .parameter_schema()
                .get(index)
                .copied()
                .ok_or(CheckedFxDefinitionSealError::InvalidCallShape { owner })?;
            let value = match schema.value_type() {
                FxSourceParameterType::Static(expected) => {
                    CheckedFxConstructorArgumentValue::Value(self.seal_symbolic_value(
                        module_id,
                        source,
                        expected,
                        parameters,
                        runtime_parameter_types,
                    )?)
                }
                FxSourceParameterType::TransitionKind => {
                    CheckedFxConstructorArgumentValue::Value(self.seal_symbolic_value(
                        module_id,
                        source,
                        FxStaticType::Selector(FxSelectorDomain::TransitionKind),
                        parameters,
                        runtime_parameter_types,
                    )?)
                }
                FxSourceParameterType::TransitionEasing => {
                    CheckedFxConstructorArgumentValue::Value(self.seal_symbolic_value(
                        module_id,
                        source,
                        FxStaticType::Selector(FxSelectorDomain::TransitionEasing),
                        parameters,
                        runtime_parameter_types,
                    )?)
                }
                FxSourceParameterType::Fx => {
                    let child = self.seal_graph_expression(
                        module_id,
                        source,
                        parameters,
                        runtime_parameter_types,
                        depth
                            .checked_add(1)
                            .ok_or(CheckedFxDefinitionSealError::AccountingOverflow)?,
                    )?;
                    nodes = checked_add(nodes, child.nodes)?;
                    visits = checked_add(visits, child.visits)?;
                    graph_depth = graph_depth.max(checked_add(1, child.depth)?);
                    CheckedFxConstructorArgumentValue::Graph(Box::new(child.expression))
                }
                FxSourceParameterType::FxList => {
                    let module = self
                        .analyzer
                        .module(module_id)
                        .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
                    let expression = module
                        .resolve_expr(source)
                        .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
                    let HirExprKind::BracketSequence(sequence) = expression.kind() else {
                        return Err(CheckedFxDefinitionSealError::InvalidValue {
                            owner: source,
                            expected: FxStaticType::Runtime(FxRuntimeType::Bool),
                        });
                    };
                    let mut children = Vec::with_capacity(sequence.elements().len());
                    for child in sequence.elements() {
                        let child = self.seal_graph_expression(
                            module_id,
                            *child,
                            parameters,
                            runtime_parameter_types,
                            depth
                                .checked_add(1)
                                .ok_or(CheckedFxDefinitionSealError::AccountingOverflow)?,
                        )?;
                        nodes = checked_add(nodes, child.nodes)?;
                        visits = checked_add(visits, child.visits)?;
                        graph_depth = graph_depth.max(checked_add(1, child.depth)?);
                        children.push(child.expression);
                    }
                    CheckedFxConstructorArgumentValue::Graphs(children.into_boxed_slice())
                }
            };
            arguments.push(CheckedFxConstructorArgument::new(
                u16::try_from(index)
                    .map_err(|_| CheckedFxDefinitionSealError::AccountingOverflow)?,
                value,
            ));
        }
        let expression = CheckedFxGraphExpression::Constructor(CheckedFxConstructorCall::new(
            constructor,
            arguments,
        ));
        let graph = CheckedGraph {
            expression,
            nodes,
            visits,
            depth: graph_depth,
        };
        validate_limits(&graph)?;
        Ok(graph)
    }

    fn seal_project_call(
        &mut self,
        module_id: HirModuleId,
        owner: ExprId,
        call: &HirCallInvocation,
        definition: &FxId,
        parameters: &BTreeMap<LocalId, FxDefinitionParameterRef>,
        runtime_parameter_types: &[FxRuntimeType],
        _depth: usize,
    ) -> Result<CheckedGraph, CheckedFxDefinitionSealError> {
        let declaration = self
            .by_id
            .get(definition)
            .cloned()
            .ok_or(CheckedFxDefinitionSealError::MissingDeclaration)?;
        let callee = self.seal_project(&declaration)?;
        let prepared = self
            .prepared
            .get(&declaration)
            .cloned()
            .ok_or(CheckedFxDefinitionSealError::MissingDeclaration)?;
        let sources = map_project_arguments(owner, call, &prepared)?;
        let callable_schema = self
            .analyzer
            .staged_callables
            .as_ref()
            .and_then(|staged| staged.accepted.project_record(&declaration))
            .map(|record| record.schema().clone())
            .ok_or(CheckedFxDefinitionSealError::InvalidSchema)?;
        let [group] = callable_schema.groups() else {
            return Err(CheckedFxDefinitionSealError::InvalidSchema);
        };
        let mut arguments = Vec::with_capacity(group.parameters().len());
        for (index, (parameter, definition_parameter)) in group
            .parameters()
            .iter()
            .zip(prepared.parameter_schema.parameters())
            .enumerate()
        {
            let decision = match sources.get(&index).copied() {
                Some(source) => CheckedFxBindingDecision::Explicit(
                    CheckedSymbolicFxBinding::Value(self.seal_symbolic_value(
                        module_id,
                        source,
                        definition_parameter.parameter_type().static_type(),
                        parameters,
                        runtime_parameter_types,
                    )?),
                ),
                None if parameter.presence() == CallableParameterPresence::Defaulted => {
                    CheckedFxBindingDecision::Defaulted
                }
                None => {
                    return Err(CheckedFxDefinitionSealError::InvalidCallShape { owner });
                }
            };
            arguments.push(CheckedFxArgument::new(
                CheckedFxSourceParameter::Project(definition_parameter.index()),
                decision,
            ));
        }
        let nodes = callee.body().expanded_nodes() as usize;
        let visits = checked_add(1, callee.body().expanded_visits() as usize)?;
        let depth = checked_add(1, callee.body().expanded_depth() as usize)?;
        let graph = CheckedGraph {
            expression: CheckedFxGraphExpression::Project(CheckedFxBodyCall::new(
                callee.reference(),
                arguments,
            )),
            nodes,
            visits,
            depth,
        };
        validate_limits(&graph)?;
        Ok(graph)
    }

    fn seal_builtin(
        &mut self,
        module_id: HirModuleId,
        owner: ExprId,
        call: &HirCallInvocation,
        row_id: BuiltinFxCallableRowId,
        parameters: &BTreeMap<LocalId, FxDefinitionParameterRef>,
        runtime_parameter_types: &[FxRuntimeType],
        _depth: usize,
    ) -> Result<CheckedGraph, CheckedFxDefinitionSealError> {
        let row = BUILTIN_FX_CALLABLE_CATALOG
            .get(row_id)
            .ok_or(CheckedFxDefinitionSealError::InvalidSchema)?;
        let sources = map_builtin_arguments(owner, call, row)?;
        let mut decisions = Vec::with_capacity(row.parameters().len());
        let mut active_abi = Vec::new();
        let mut target = None;
        let mut motion_function = None;
        for (index, source) in row.parameters().iter().copied().enumerate() {
            let supplied = sources.get(&index).copied();
            let predicate_holds = match source.presence() {
                BuiltinFxParameterPresence::Conditional { predicate, .. } => {
                    Some(self.symbolic_builtin_predicate(
                        module_id,
                        row,
                        predicate,
                        &sources,
                        parameters,
                        runtime_parameter_types,
                    )?)
                }
                _ => None,
            };
            let decision = match (source.presence(), supplied) {
                (_, Some(_)) if predicate_holds == Some(false) => {
                    return Err(CheckedFxDefinitionSealError::InvalidCallShape { owner });
                }
                (_, Some(expression)) => {
                    let binding = self.seal_builtin_binding(
                        module_id,
                        expression,
                        source,
                        parameters,
                        runtime_parameter_types,
                    )?;
                    CheckedFxBindingDecision::Explicit(binding)
                }
                (BuiltinFxParameterPresence::Required, None) => {
                    return Err(CheckedFxDefinitionSealError::InvalidCallShape { owner });
                }
                (BuiltinFxParameterPresence::Defaulted(_), None) => {
                    CheckedFxBindingDecision::Defaulted
                }
                (BuiltinFxParameterPresence::Optional, None) => CheckedFxBindingDecision::Omitted,
                (BuiltinFxParameterPresence::Conditional { default, .. }, None) => {
                    match predicate_holds {
                        Some(false) => CheckedFxBindingDecision::Omitted,
                        Some(true) if default.is_some() => CheckedFxBindingDecision::Defaulted,
                        Some(true) | None => {
                            return Err(CheckedFxDefinitionSealError::InvalidCallShape { owner });
                        }
                    }
                }
            };
            validate_builtin_constraint(source, &decision)?;
            let effective = structural_binding(source, &decision)?;
            match source.id() {
                BuiltinFxParameterId::Phase => {
                    let phase = effective.and_then(EffectiveStructuralBinding::phase);
                    if phase != Some(row_id.phase()) {
                        return Err(CheckedFxDefinitionSealError::InvalidCallShape { owner });
                    }
                }
                BuiltinFxParameterId::Target => {
                    target = effective.and_then(EffectiveStructuralBinding::target);
                }
                BuiltinFxParameterId::MotionFunction => {
                    motion_function = effective.and_then(EffectiveStructuralBinding::motion);
                }
                _ => {}
            }
            if source.binding() == BuiltinFxParameterBinding::Abi
                && match source.presence() {
                    BuiltinFxParameterPresence::Required
                    | BuiltinFxParameterPresence::Defaulted(_) => true,
                    BuiltinFxParameterPresence::Optional => {
                        matches!(decision, CheckedFxBindingDecision::Explicit(_))
                    }
                    BuiltinFxParameterPresence::Conditional { .. } => predicate_holds == Some(true),
                }
            {
                active_abi.push(source.id());
            }
            decisions.push(CheckedFxArgument::new(
                CheckedFxSourceParameter::Builtin(source.id()),
                decision,
            ));
        }
        let specialization = BuiltinFxSpecialization::try_new(
            row_id,
            target.ok_or(CheckedFxDefinitionSealError::InvalidCallShape { owner })?,
            motion_function,
            BuiltinFxActiveAbiParameterSet::from_parameters(active_abi),
        )
        .map_err(|_| CheckedFxDefinitionSealError::InvalidCallShape { owner })?;
        let template = build_builtin_fx_definition(specialization)
            .map_err(|_| CheckedFxDefinitionSealError::InvalidCallShape { owner })?;
        let reference = CheckedFxDefinitionRef::Builtin {
            row: row_id,
            specialization,
            schema: row.schema_digest(),
            definition: template.definition().id().clone(),
            layout: template.definition().parameter_layout().digest(),
        };
        self.catalog.insert(
            reference
                .builtin_definition()
                .ok_or(CheckedFxDefinitionSealError::OwnerInvariant)?,
        )?;
        let nodes = template.definition().graph().total_node_count();
        let depth = graph_depth(template.definition().graph());
        let graph = CheckedGraph {
            expression: CheckedFxGraphExpression::Builtin(CheckedFxBodyCall::new(
                reference, decisions,
            )),
            nodes,
            visits: 1,
            depth,
        };
        validate_limits(&graph)?;
        Ok(graph)
    }

    fn symbolic_builtin_predicate(
        &mut self,
        module_id: HirModuleId,
        row: BuiltinFxCallableRow,
        predicate: arcweft_presentation::fx::BuiltinFxParameterPredicate,
        sources: &BTreeMap<usize, ExprId>,
        parameters: &BTreeMap<LocalId, FxDefinitionParameterRef>,
        runtime_parameter_types: &[FxRuntimeType],
    ) -> Result<bool, CheckedFxDefinitionSealError> {
        let arcweft_presentation::fx::BuiltinFxParameterPredicate::BoolEquals {
            parameter,
            value: expected,
        } = predicate;
        let index = row
            .parameters()
            .iter()
            .position(|candidate| candidate.id() == parameter)
            .ok_or(CheckedFxDefinitionSealError::InvalidSchema)?;
        let actual = if let Some(source) = sources.get(&index).copied() {
            let value = self.seal_symbolic_value(
                module_id,
                source,
                FxStaticType::Runtime(FxRuntimeType::Bool),
                parameters,
                runtime_parameter_types,
            )?;
            symbolic_bool(&value)
                .ok_or(CheckedFxDefinitionSealError::UnprovableBuiltinConstraint { parameter })?
        } else {
            match builtin_fx_presence_default(row.parameters()[index].presence()) {
                Some(BuiltinFxDefaultValue::Bool(value)) => value,
                _ => return Err(CheckedFxDefinitionSealError::InvalidSchema),
            }
        };
        Ok(actual == expected)
    }

    fn seal_builtin_binding(
        &mut self,
        module_id: HirModuleId,
        owner: ExprId,
        parameter: BuiltinFxCallableParameter,
        parameters: &BTreeMap<LocalId, FxDefinitionParameterRef>,
        runtime_parameter_types: &[FxRuntimeType],
    ) -> Result<CheckedSymbolicFxBinding, CheckedFxDefinitionSealError> {
        match parameter.parameter_type() {
            BuiltinFxParameterType::Phase => self
                .seal_phase(module_id, owner)
                .map(CheckedSymbolicFxBinding::Phase),
            BuiltinFxParameterType::Target => self
                .seal_target(module_id, owner)
                .map(CheckedSymbolicFxBinding::Target),
            BuiltinFxParameterType::MotionFunction => self
                .seal_motion_function(module_id, owner)
                .map(CheckedSymbolicFxBinding::MotionFunction),
            parameter_type => {
                let expected = parameter_type
                    .direct_definition_parameter_type()
                    .ok_or(CheckedFxDefinitionSealError::InvalidSchema)?
                    .static_type();
                self.seal_symbolic_value(
                    module_id,
                    owner,
                    expected,
                    parameters,
                    runtime_parameter_types,
                )
                .map(CheckedSymbolicFxBinding::Value)
            }
        }
    }

    fn seal_symbolic_value(
        &mut self,
        module_id: HirModuleId,
        owner: ExprId,
        expected: FxStaticType,
        parameters: &BTreeMap<LocalId, FxDefinitionParameterRef>,
        runtime_parameter_types: &[FxRuntimeType],
    ) -> Result<CheckedFxSymbolicValue, CheckedFxDefinitionSealError> {
        if let Some(local) = self.checked_local(module_id, owner)?
            && let Some(parameter) = parameters.get(&local).copied()
        {
            if parameter.parameter_type().static_type() != expected {
                return Err(CheckedFxDefinitionSealError::InvalidValue { owner, expected });
            }
            return Ok(CheckedFxSymbolicValue::Parameter(parameter));
        }
        let module = self
            .analyzer
            .module(module_id)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        let expression = module
            .resolve_expr(owner)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        if let HirExprKind::Closure(closure) = expression.kind() {
            if expected != FxStaticType::Runtime(FxRuntimeType::Transform2D) {
                return Err(CheckedFxDefinitionSealError::InvalidValue { owner, expected });
            }
            return self
                .seal_sampler(module_id, closure, parameters, runtime_parameter_types)
                .map(CheckedFxSymbolicValue::Program);
        }
        if let HirExprKind::RecordLiteral(record) = expression.kind()
            && expected == FxStaticType::Runtime(FxRuntimeType::Transform2D)
        {
            let value = self.seal_transform_record(
                module_id,
                record.fields(),
                parameters,
                runtime_parameter_types,
            )?;
            return Ok(CheckedFxSymbolicValue::Constant(CheckedFxConstant::Abi(
                FxDefinitionArgumentValue::Runtime(value),
            )));
        }
        if expected == FxStaticType::Runtime(FxRuntimeType::Vec2)
            && matches!(expression.kind(), HirExprKind::Call(_))
        {
            if let Some(value) =
                self.seal_vec2_call(module_id, owner, parameters, runtime_parameter_types)?
            {
                return Ok(value);
            }
        }
        let expected_type = self
            .type_for_static(expected)
            .ok_or(CheckedFxDefinitionSealError::InvalidValue { owner, expected })?;
        self.analyzer
            .check_expression_published(owner, Some(&expected_type))
            .map_err(|_| CheckedFxDefinitionSealError::InvalidValue { owner, expected })?;
        let value = self
            .analyzer
            .checked_compile_time_value(module, owner, &expected_type)
            .map_err(|_| CheckedFxDefinitionSealError::InvalidValue { owner, expected })?;
        let constant = match expected {
            FxStaticType::Runtime(runtime) => {
                let value = checked_project_fx_argument(
                    FxDefinitionParameterType::Runtime(runtime),
                    &value,
                )
                .map_err(|_| CheckedFxDefinitionSealError::InvalidValue { owner, expected })?;
                CheckedFxConstant::Abi(value)
            }
            FxStaticType::Resource => {
                let crate::final_analysis::CheckedContentFxBinding::Abi(value) =
                    checked_builtin_fx_binding(BuiltinFxParameterType::Resource, &value).map_err(
                        |_| CheckedFxDefinitionSealError::InvalidValue { owner, expected },
                    )?
                else {
                    return Err(CheckedFxDefinitionSealError::InvalidValue { owner, expected });
                };
                CheckedFxConstant::Abi(value)
            }
            FxStaticType::Selector(domain) => {
                let crate::final_analysis::CheckedCompileTimeValue::Scalar(
                    crate::checked_compile_time::CheckedCompileTimeScalar::Text(value),
                ) = value
                else {
                    return Err(CheckedFxDefinitionSealError::InvalidValue { owner, expected });
                };
                CheckedFxConstant::Selector(
                    FxSelectorId::try_new(domain, value).map_err(|_| {
                        CheckedFxDefinitionSealError::InvalidValue { owner, expected }
                    })?,
                )
            }
            FxStaticType::ShaderStage => {
                let crate::final_analysis::CheckedCompileTimeValue::Enum(value) = value else {
                    return Err(CheckedFxDefinitionSealError::InvalidValue { owner, expected });
                };
                CheckedFxConstant::ShaderStage(
                    FxShaderStage::from_value_id(value)
                        .ok_or(CheckedFxDefinitionSealError::InvalidValue { owner, expected })?,
                )
            }
            FxStaticType::FontFamily => {
                let crate::final_analysis::CheckedCompileTimeValue::Scalar(
                    crate::checked_compile_time::CheckedCompileTimeScalar::Text(value),
                ) = value
                else {
                    return Err(CheckedFxDefinitionSealError::InvalidValue { owner, expected });
                };
                CheckedFxConstant::FontFamily(
                    FxFontFamilyName::try_new(value).map_err(|_| {
                        CheckedFxDefinitionSealError::InvalidValue { owner, expected }
                    })?,
                )
            }
            FxStaticType::Target => {
                let crate::final_analysis::CheckedCompileTimeValue::Enum(value) = value else {
                    return Err(CheckedFxDefinitionSealError::InvalidValue { owner, expected });
                };
                CheckedFxConstant::Target(
                    FxTarget::from_value_id(value)
                        .ok_or(CheckedFxDefinitionSealError::InvalidValue { owner, expected })?,
                )
            }
            FxStaticType::Phase => {
                let crate::final_analysis::CheckedCompileTimeValue::Enum(value) = value else {
                    return Err(CheckedFxDefinitionSealError::InvalidValue { owner, expected });
                };
                CheckedFxConstant::Phase(
                    FxPhase::from_value_id(value)
                        .ok_or(CheckedFxDefinitionSealError::InvalidValue { owner, expected })?,
                )
            }
            FxStaticType::UniformRecord => {
                return Err(CheckedFxDefinitionSealError::InvalidValue { owner, expected });
            }
        };
        Ok(CheckedFxSymbolicValue::Constant(constant))
    }

    fn checked_local(
        &self,
        module_id: HirModuleId,
        owner: ExprId,
    ) -> Result<Option<LocalId>, CheckedFxDefinitionSealError> {
        if let Some(crate::final_analysis::CheckedExpressionResolution::Value(
            crate::final_analysis::CheckedValueResolution::Local(local),
        )) = self
            .analyzer
            .facts
            .expressions()
            .get(&owner)
            .and_then(PreparedExpressionFact::checked_resolution)
        {
            return Ok(Some(*local));
        }
        let module = self
            .analyzer
            .module(module_id)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        let expression = module
            .resolve_expr(owner)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        let HirExprKind::Path(arcweft_lang_hir::leaf::HirPathValue::Resolved(path)) =
            expression.kind()
        else {
            return Ok(None);
        };
        let resolution = self
            .analyzer
            .resolve_path_value(module, owner, expression.scope(), path)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        Ok(match resolution {
            Some(crate::final_analysis::CheckedValueResolution::Local(local)) => Some(local),
            _ => None,
        })
    }

    fn type_for_static(&self, expected: FxStaticType) -> Option<TypeKind> {
        let scalars = self
            .analyzer
            .catalogs
            .world
            .environment()
            .compile_time_scalars();
        let scalar = |role| scalars.type_for(role).clone();
        Some(match expected {
            FxStaticType::Runtime(runtime) => match runtime {
                FxRuntimeType::Bool => {
                    scalar(crate::registration::CompileTimeScalarTypeRoleId::Bool)
                }
                FxRuntimeType::I32 => scalar(crate::registration::CompileTimeScalarTypeRoleId::Int),
                FxRuntimeType::U32 => TypeKind::U32,
                FxRuntimeType::F32 => TypeKind::F32,
                FxRuntimeType::Length => {
                    scalar(crate::registration::CompileTimeScalarTypeRoleId::Length)
                }
                FxRuntimeType::Angle => {
                    scalar(crate::registration::CompileTimeScalarTypeRoleId::Angle)
                }
                FxRuntimeType::Seconds => {
                    scalar(crate::registration::CompileTimeScalarTypeRoleId::Duration)
                }
                FxRuntimeType::Color => {
                    scalar(crate::registration::CompileTimeScalarTypeRoleId::Color)
                }
                FxRuntimeType::Vec2 => TypeKind::FixedVector(crate::types::FixedVectorType::new(
                    VectorDimensions::Two,
                    TypeKind::F32,
                )),
                FxRuntimeType::Transform2D => TypeKind::Named("Transform2D".to_owned()),
            },
            FxStaticType::Resource => {
                scalar(crate::registration::CompileTimeScalarTypeRoleId::PublicId)
            }
            FxStaticType::Selector(_) | FxStaticType::FontFamily => {
                scalar(crate::registration::CompileTimeScalarTypeRoleId::Text)
            }
            FxStaticType::Target => {
                TypeKind::CompileTimeEnum(crate::types::CompileTimeEnumType::any(
                    arcweft_presentation::fx::FxEnumDomain::Target.domain_id(),
                ))
            }
            FxStaticType::Phase => {
                TypeKind::CompileTimeEnum(crate::types::CompileTimeEnumType::any(
                    arcweft_presentation::fx::FxEnumDomain::Phase.domain_id(),
                ))
            }
            FxStaticType::ShaderStage => {
                TypeKind::CompileTimeEnum(crate::types::CompileTimeEnumType::any(
                    arcweft_presentation::fx::FxEnumDomain::ShaderStage.domain_id(),
                ))
            }
            FxStaticType::UniformRecord => return None,
        })
    }

    fn seal_vec2_call(
        &mut self,
        module_id: HirModuleId,
        owner: ExprId,
        parameters: &BTreeMap<LocalId, FxDefinitionParameterRef>,
        runtime_parameter_types: &[FxRuntimeType],
    ) -> Result<Option<CheckedFxSymbolicValue>, CheckedFxDefinitionSealError> {
        let selected = self
            .analyzer
            .facts
            .calls()
            .get(&owner)
            .and_then(crate::callable::CallTargetFacts::selected_application);
        if !selected.is_some_and(|application| {
            matches!(
                application.core().candidates().selected().id(),
                CallableCandidateId::Builtin(BuiltinCallableId::Vector {
                    dimensions: VectorDimensions::Two
                })
            )
        }) {
            return Ok(None);
        }
        let sources = selected
            .expect("selected vector application was checked above")
            .core()
            .execution()
            .arguments()
            .iter()
            .flat_map(|argument| argument.slots())
            .map(|slot| match slot.source().raw() {
                crate::callable::CheckedCallArgumentSlotSource::Expression(expression) => {
                    Ok(expression)
                }
                crate::callable::CheckedCallArgumentSlotSource::CompactNumericElement {
                    ..
                } => Err(CheckedFxDefinitionSealError::InvalidValue {
                    owner,
                    expected: FxStaticType::Runtime(FxRuntimeType::Vec2),
                }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let [x, y] = sources.as_slice() else {
            return Err(CheckedFxDefinitionSealError::InvalidValue {
                owner,
                expected: FxStaticType::Runtime(FxRuntimeType::Vec2),
            });
        };
        let x = self.seal_symbolic_value(
            module_id,
            *x,
            FxStaticType::Runtime(FxRuntimeType::F32),
            parameters,
            runtime_parameter_types,
        )?;
        let y = self.seal_symbolic_value(
            module_id,
            *y,
            FxStaticType::Runtime(FxRuntimeType::F32),
            parameters,
            runtime_parameter_types,
        )?;
        match (x, y) {
            (
                CheckedFxSymbolicValue::Constant(CheckedFxConstant::Abi(
                    FxDefinitionArgumentValue::Runtime(FxRuntimeValue::F32(x)),
                )),
                CheckedFxSymbolicValue::Constant(CheckedFxConstant::Abi(
                    FxDefinitionArgumentValue::Runtime(FxRuntimeValue::F32(y)),
                )),
            ) => Ok(Some(CheckedFxSymbolicValue::Constant(
                CheckedFxConstant::Abi(FxDefinitionArgumentValue::Runtime(FxRuntimeValue::Vec2(
                    FxVec2 { x, y },
                ))),
            ))),
            _ => Err(CheckedFxDefinitionSealError::UnprovableBuiltinConstraint {
                parameter: BuiltinFxParameterId::Direction,
            }),
        }
    }

    fn seal_transform_record(
        &mut self,
        module_id: HirModuleId,
        fields: &[HirRecordField],
        parameters: &BTreeMap<LocalId, FxDefinitionParameterRef>,
        runtime_parameter_types: &[FxRuntimeType],
    ) -> Result<FxRuntimeValue, CheckedFxDefinitionSealError> {
        let mut values = BTreeMap::<&str, FxRuntimeValue>::new();
        for field in fields {
            let HirRecordField::Explicit {
                name,
                value: source,
            } = field
            else {
                return Err(CheckedFxDefinitionSealError::InvalidBody);
            };
            let expected = transform_field_type(name.as_str())
                .ok_or(CheckedFxDefinitionSealError::InvalidBody)?;
            let value = self.seal_symbolic_value(
                module_id,
                *source,
                FxStaticType::Runtime(expected),
                parameters,
                runtime_parameter_types,
            )?;
            let CheckedFxSymbolicValue::Constant(CheckedFxConstant::Abi(
                FxDefinitionArgumentValue::Runtime(value),
            )) = value
            else {
                return Err(CheckedFxDefinitionSealError::InvalidValue {
                    owner: *source,
                    expected: FxStaticType::Runtime(expected),
                });
            };
            if values.insert(name.as_str(), value).is_some() {
                return Err(CheckedFxDefinitionSealError::InvalidBody);
            }
        }
        let get = |name: &str| values.get(name).copied();
        Ok(FxRuntimeValue::Transform2D(Transform2D {
            translate_x: runtime_length(get("translate_x"))?.unwrap_or(Length::ZERO),
            translate_y: runtime_length(get("translate_y"))?.unwrap_or(Length::ZERO),
            scale_x: runtime_f32(get("scale_x"))?.unwrap_or(FiniteF32::ONE),
            scale_y: runtime_f32(get("scale_y"))?.unwrap_or(FiniteF32::ONE),
            skew_x: runtime_angle(get("skew_x"))?.unwrap_or(arcweft_presentation::fx::Angle::ZERO),
            skew_y: runtime_angle(get("skew_y"))?.unwrap_or(arcweft_presentation::fx::Angle::ZERO),
            rotation: runtime_angle(get("rotation"))?
                .unwrap_or(arcweft_presentation::fx::Angle::ZERO),
            origin_x: runtime_length(get("origin_x"))?.unwrap_or(Length::ZERO),
            origin_y: runtime_length(get("origin_y"))?.unwrap_or(Length::ZERO),
            opacity: runtime_f32(get("opacity"))?.unwrap_or(FiniteF32::ONE),
        }))
    }

    fn seal_sampler(
        &mut self,
        module_id: HirModuleId,
        closure: &arcweft_lang_hir::expr::HirClosureExpr,
        parameters: &BTreeMap<LocalId, FxDefinitionParameterRef>,
        runtime_parameter_types: &[FxRuntimeType],
    ) -> Result<FxSamplerProgram, CheckedFxDefinitionSealError> {
        let [parameter] = closure.parameters() else {
            return Err(CheckedFxDefinitionSealError::InvalidBody);
        };
        let module = self
            .analyzer
            .module(module_id)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        let pattern = module
            .resolve_pattern(parameter.pattern())
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        let HirPatternKind::Binding(HirPatternBinding::Bound { local: context, .. }) =
            pattern.kind()
        else {
            return Err(CheckedFxDefinitionSealError::InvalidBody);
        };
        let body = module
            .resolve_expr(closure.body())
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?;
        let HirExprKind::RecordLiteral(record) = body.kind() else {
            return Err(CheckedFxDefinitionSealError::InvalidBody);
        };
        let expected = [
            ("translate_x", FxRuntimeType::Length),
            ("translate_y", FxRuntimeType::Length),
            ("scale_x", FxRuntimeType::F32),
            ("scale_y", FxRuntimeType::F32),
            ("skew_x", FxRuntimeType::Angle),
            ("skew_y", FxRuntimeType::Angle),
            ("rotation", FxRuntimeType::Angle),
            ("origin_x", FxRuntimeType::Length),
            ("origin_y", FxRuntimeType::Length),
            ("opacity", FxRuntimeType::F32),
        ];
        let mut fields = BTreeMap::<&str, ExprId>::new();
        for field in record.fields() {
            let HirRecordField::Explicit { name, value } = field else {
                return Err(CheckedFxDefinitionSealError::InvalidBody);
            };
            if fields.insert(name.as_str(), *value).is_some() {
                return Err(CheckedFxDefinitionSealError::InvalidBody);
            }
        }
        if fields
            .keys()
            .any(|name| !expected.iter().any(|(expected, _)| name == expected))
        {
            return Err(CheckedFxDefinitionSealError::InvalidBody);
        }
        let defaults = Transform2D::default();
        let default_values = [
            FxRuntimeValue::Length(defaults.translate_x),
            FxRuntimeValue::Length(defaults.translate_y),
            FxRuntimeValue::F32(defaults.scale_x),
            FxRuntimeValue::F32(defaults.scale_y),
            FxRuntimeValue::Angle(defaults.skew_x),
            FxRuntimeValue::Angle(defaults.skew_y),
            FxRuntimeValue::Angle(defaults.rotation),
            FxRuntimeValue::Length(defaults.origin_x),
            FxRuntimeValue::Length(defaults.origin_y),
            FxRuntimeValue::F32(defaults.opacity),
        ];
        let mut instructions = Vec::new();
        for ((name, ty), default) in expected.into_iter().zip(default_values) {
            if let Some(expression) = fields.get(name).copied() {
                let (mut part, actual) = self.seal_sampler_expression(
                    module_id,
                    expression,
                    *context,
                    parameters,
                    runtime_parameter_types,
                )?;
                if actual != ty {
                    return Err(CheckedFxDefinitionSealError::InvalidValue {
                        owner: expression,
                        expected: FxStaticType::Runtime(ty),
                    });
                }
                instructions.append(&mut part);
            } else {
                instructions.push(ValueInstruction::Constant { value: default });
            }
        }
        instructions.push(ValueInstruction::MakeTransform2D);
        instructions.push(ValueInstruction::Return);
        FxSamplerProgram::validate(
            ValueProgramSchema::new(
                runtime_parameter_types.to_vec(),
                Vec::new(),
                FxRuntimeType::Transform2D,
            ),
            instructions,
        )
        .map_err(|_| CheckedFxDefinitionSealError::InvalidBody)
    }

    fn seal_sampler_expression(
        &mut self,
        module_id: HirModuleId,
        owner: ExprId,
        context: LocalId,
        parameters: &BTreeMap<LocalId, FxDefinitionParameterRef>,
        runtime_parameter_types: &[FxRuntimeType],
    ) -> Result<(Vec<ValueInstruction>, FxRuntimeType), CheckedFxDefinitionSealError> {
        let mut sealer = FxSamplerValueProgramContext {
            owner: self,
            module_id,
            sample_context: context,
            parameters,
            runtime_parameter_types,
        };
        super::checked_value_program::seal_checked_value_expression(&mut sealer, owner)
    }
}

struct FxSamplerValueProgramContext<'a, 'b, 'analyzer, 'project, 'catalog, 'control> {
    owner: &'a mut FxDefinitionSealer<'analyzer, 'project, 'catalog, 'control>,
    module_id: HirModuleId,
    sample_context: LocalId,
    parameters: &'b BTreeMap<LocalId, FxDefinitionParameterRef>,
    runtime_parameter_types: &'b [FxRuntimeType],
}

impl super::checked_value_program::CheckedValueProgramSealContext
    for FxSamplerValueProgramContext<'_, '_, '_, '_, '_, '_>
{
    type Error = CheckedFxDefinitionSealError;

    fn expression(
        &mut self,
        owner: ExprId,
    ) -> Result<arcweft_lang_hir::expr::HirExpr, Self::Error> {
        self.owner
            .analyzer
            .module(self.module_id)
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)?
            .resolve_expr(owner)
            .cloned()
            .map_err(|_| CheckedFxDefinitionSealError::OwnerInvariant)
    }

    fn inferred_type(&mut self, owner: ExprId) -> Result<FxRuntimeType, Self::Error> {
        self.owner
            .analyzer
            .facts
            .expressions()
            .get(&owner)
            .and_then(PreparedExpressionFact::value_type)
            .and_then(fx_runtime_type)
            .ok_or(CheckedFxDefinitionSealError::InvalidBody)
    }

    fn constant(
        &mut self,
        owner: ExprId,
        expected: FxRuntimeType,
    ) -> Result<Option<FxRuntimeValue>, Self::Error> {
        let value = self.owner.seal_symbolic_value(
            self.module_id,
            owner,
            FxStaticType::Runtime(expected),
            self.parameters,
            self.runtime_parameter_types,
        )?;
        let CheckedFxSymbolicValue::Constant(CheckedFxConstant::Abi(
            FxDefinitionArgumentValue::Runtime(value),
        )) = value
        else {
            return Ok(None);
        };
        Ok(Some(value))
    }

    fn input(
        &mut self,
        owner: ExprId,
    ) -> Result<Option<arcweft_presentation::fx::FxRuntimeParameterRef>, Self::Error> {
        let Some(local) = self.owner.checked_local(self.module_id, owner)? else {
            return Ok(None);
        };
        let parameter = self
            .parameters
            .get(&local)
            .copied()
            .ok_or(CheckedFxDefinitionSealError::InvalidBody)?;
        let FxDefinitionParameterType::Runtime(ty) = parameter.parameter_type() else {
            return Err(CheckedFxDefinitionSealError::InvalidBody);
        };
        let schema = ValueProgramSchema::new(self.runtime_parameter_types.to_vec(), Vec::new(), ty);
        schema
            .parameter_ref(usize::from(parameter.index().get()))
            .map(Some)
            .ok_or(CheckedFxDefinitionSealError::InvalidBody)
    }

    fn context_slot(&mut self, owner: ExprId) -> Result<Option<FxContextSlot>, Self::Error> {
        let expression = self.expression(owner)?;
        let HirExprKind::Select(select) = expression.kind() else {
            return Ok(None);
        };
        let target_local = self
            .owner
            .checked_local(self.module_id, select.target())?
            .ok_or(CheckedFxDefinitionSealError::InvalidBody)?;
        if target_local != self.sample_context {
            return Err(CheckedFxDefinitionSealError::InvalidBody);
        }
        let HirSelectedMember::Name(name) = select.member() else {
            return Err(CheckedFxDefinitionSealError::InvalidBody);
        };
        Ok(Some(match name.as_str() {
            "time" => FxContextSlot::Time,
            "ordinal" => FxContextSlot::Ordinal,
            "reduce_motion" => FxContextSlot::ReduceMotion,
            "target_center_x" => FxContextSlot::TargetCenterX,
            "target_center_y" => FxContextSlot::TargetCenterY,
            "glyph_center_x" => FxContextSlot::GlyphCenterX,
            "glyph_center_y" => FxContextSlot::GlyphCenterY,
            _ => return Err(CheckedFxDefinitionSealError::InvalidBody),
        }))
    }

    fn call(&mut self, owner: ExprId) -> Result<(CallableCandidateId, Vec<ExprId>), Self::Error> {
        let application = self
            .owner
            .analyzer
            .facts
            .calls()
            .get(&owner)
            .and_then(crate::callable::CallTargetFacts::selected_application)
            .ok_or(CheckedFxDefinitionSealError::InvalidBody)?;
        let sources = application
            .core()
            .execution()
            .arguments()
            .iter()
            .flat_map(|argument| argument.slots())
            .map(|slot| match slot.source().raw() {
                crate::callable::CheckedCallArgumentSlotSource::Expression(expression) => {
                    Ok(expression)
                }
                crate::callable::CheckedCallArgumentSlotSource::CompactNumericElement {
                    ..
                } => Err(CheckedFxDefinitionSealError::InvalidBody),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((
            application.core().candidates().selected().id().clone(),
            sources,
        ))
    }

    fn invalid(&self, owner: ExprId) -> Self::Error {
        CheckedFxDefinitionSealError::UnsupportedExpression { owner }
    }
}

fn map_constructor_arguments(
    owner: ExprId,
    call: &HirCallInvocation,
    constructor: FxSourceConstructor,
) -> Result<BTreeMap<usize, ExprId>, CheckedFxDefinitionSealError> {
    let schema = constructor.parameter_schema();
    let mut output = BTreeMap::new();
    let mut positional = 0usize;
    for argument in call.arguments() {
        let index = match argument {
            HirCallArgument::Positional { .. } => {
                let index = (positional..schema.len())
                    .find(|index| {
                        schema[*index].passing() == FxSourceParameterPassing::PositionalOnly
                    })
                    .ok_or(CheckedFxDefinitionSealError::InvalidCallShape { owner })?;
                positional = index
                    .checked_add(1)
                    .ok_or(CheckedFxDefinitionSealError::AccountingOverflow)?;
                index
            }
            HirCallArgument::Named { .. } => {
                let name = argument
                    .resolved_name()
                    .ok_or(CheckedFxDefinitionSealError::InvalidCallShape { owner })?;
                schema
                    .iter()
                    .position(|parameter| {
                        parameter.passing() == FxSourceParameterPassing::NamedOnly
                            && parameter.source_name() == name.as_str()
                    })
                    .ok_or(CheckedFxDefinitionSealError::InvalidCallShape { owner })?
            }
            HirCallArgument::Spread { .. } => {
                return Err(CheckedFxDefinitionSealError::InvalidCallShape { owner });
            }
        };
        if output.insert(index, argument.value()).is_some() {
            return Err(CheckedFxDefinitionSealError::InvalidCallShape { owner });
        }
    }
    if schema.iter().enumerate().any(|(index, parameter)| {
        parameter.presence() == FxSourceParameterPresence::Required && !output.contains_key(&index)
    }) {
        return Err(CheckedFxDefinitionSealError::InvalidCallShape { owner });
    }
    Ok(output)
}

fn map_builtin_arguments(
    owner: ExprId,
    call: &HirCallInvocation,
    row: BuiltinFxCallableRow,
) -> Result<BTreeMap<usize, ExprId>, CheckedFxDefinitionSealError> {
    let mut output = BTreeMap::new();
    for argument in call.arguments() {
        let HirCallArgument::Named { .. } = argument else {
            return Err(CheckedFxDefinitionSealError::InvalidCallShape { owner });
        };
        let name = argument
            .resolved_name()
            .ok_or(CheckedFxDefinitionSealError::InvalidCallShape { owner })?;
        let index = row
            .parameters()
            .iter()
            .position(|parameter| parameter.source_name() == name.as_str())
            .ok_or(CheckedFxDefinitionSealError::InvalidCallShape { owner })?;
        if output.insert(index, argument.value()).is_some() {
            return Err(CheckedFxDefinitionSealError::InvalidCallShape { owner });
        }
    }
    Ok(output)
}

fn map_project_arguments(
    owner: ExprId,
    call: &HirCallInvocation,
    definition: &PreparedProjectFxDefinition,
) -> Result<BTreeMap<usize, ExprId>, CheckedFxDefinitionSealError> {
    let mut output = BTreeMap::new();
    for argument in call.arguments() {
        let HirCallArgument::Named { .. } = argument else {
            return Err(CheckedFxDefinitionSealError::InvalidCallShape { owner });
        };
        let name = argument
            .resolved_name()
            .ok_or(CheckedFxDefinitionSealError::InvalidCallShape { owner })?;
        let index = definition
            .parameter_schema
            .parameters()
            .iter()
            .position(|parameter| parameter.name() == name.as_str())
            .ok_or(CheckedFxDefinitionSealError::InvalidCallShape { owner })?;
        if output.insert(index, argument.value()).is_some() {
            return Err(CheckedFxDefinitionSealError::InvalidCallShape { owner });
        }
    }
    if definition
        .parameter_presence
        .iter()
        .enumerate()
        .any(|(index, presence)| {
            *presence == CallableParameterPresence::Required && !output.contains_key(&index)
        })
    {
        return Err(CheckedFxDefinitionSealError::InvalidCallShape { owner });
    }
    Ok(output)
}

#[derive(Clone, Copy)]
enum EffectiveStructuralBinding {
    Phase(FxPhase),
    Target(FxTarget),
    Motion(MotionFunction),
}

impl EffectiveStructuralBinding {
    const fn phase(self) -> Option<FxPhase> {
        match self {
            Self::Phase(value) => Some(value),
            Self::Target(_) | Self::Motion(_) => None,
        }
    }

    const fn target(self) -> Option<FxTarget> {
        match self {
            Self::Target(value) => Some(value),
            Self::Phase(_) | Self::Motion(_) => None,
        }
    }

    const fn motion(self) -> Option<MotionFunction> {
        match self {
            Self::Motion(value) => Some(value),
            Self::Phase(_) | Self::Target(_) => None,
        }
    }
}

fn structural_binding(
    parameter: BuiltinFxCallableParameter,
    decision: &CheckedFxBindingDecision<CheckedSymbolicFxBinding>,
) -> Result<Option<EffectiveStructuralBinding>, CheckedFxDefinitionSealError> {
    let value = match decision {
        CheckedFxBindingDecision::Explicit(CheckedSymbolicFxBinding::Phase(value)) => {
            Some(EffectiveStructuralBinding::Phase(*value))
        }
        CheckedFxBindingDecision::Explicit(CheckedSymbolicFxBinding::Target(value)) => {
            Some(EffectiveStructuralBinding::Target(*value))
        }
        CheckedFxBindingDecision::Explicit(CheckedSymbolicFxBinding::MotionFunction(value)) => {
            Some(EffectiveStructuralBinding::Motion(*value))
        }
        CheckedFxBindingDecision::Explicit(CheckedSymbolicFxBinding::Value(_)) => None,
        CheckedFxBindingDecision::Defaulted => {
            match builtin_fx_presence_default(parameter.presence()) {
                Some(BuiltinFxDefaultValue::Phase(value)) => {
                    Some(EffectiveStructuralBinding::Phase(value))
                }
                Some(BuiltinFxDefaultValue::Target(value)) => {
                    Some(EffectiveStructuralBinding::Target(value))
                }
                Some(BuiltinFxDefaultValue::MotionFunction(value)) => {
                    Some(EffectiveStructuralBinding::Motion(value))
                }
                Some(
                    BuiltinFxDefaultValue::Bool(_)
                    | BuiltinFxDefaultValue::Milli(_)
                    | BuiltinFxDefaultValue::RatioMilli(_)
                    | BuiltinFxDefaultValue::LengthMilliPx(_)
                    | BuiltinFxDefaultValue::AngleMilliDegrees(_)
                    | BuiltinFxDefaultValue::DurationMillis(_)
                    | BuiltinFxDefaultValue::Seed32(_)
                    | BuiltinFxDefaultValue::Vec2Milli(_),
                )
                | None => None,
            }
        }
        CheckedFxBindingDecision::Omitted => None,
    };
    if parameter.binding() == BuiltinFxParameterBinding::Structural && value.is_none() {
        return Err(CheckedFxDefinitionSealError::InvalidSchema);
    }
    Ok(value)
}

fn validate_builtin_constraint(
    parameter: BuiltinFxCallableParameter,
    decision: &CheckedFxBindingDecision<CheckedSymbolicFxBinding>,
) -> Result<(), CheckedFxDefinitionSealError> {
    let CheckedFxBindingDecision::Explicit(binding) = decision else {
        return Ok(());
    };
    match parameter.constraint() {
        BuiltinFxValueConstraint::None => Ok(()),
        BuiltinFxValueConstraint::ExactPhase(expected) => match binding {
            CheckedSymbolicFxBinding::Phase(actual) if *actual == expected => Ok(()),
            _ => Err(CheckedFxDefinitionSealError::InvalidSchema),
        },
        BuiltinFxValueConstraint::AllowedTargets(expected) => match binding {
            CheckedSymbolicFxBinding::Target(actual) if expected.contains(actual) => Ok(()),
            _ => Err(CheckedFxDefinitionSealError::InvalidSchema),
        },
        BuiltinFxValueConstraint::Numeric(constraint) => {
            let CheckedSymbolicFxBinding::Value(CheckedFxSymbolicValue::Constant(
                CheckedFxConstant::Abi(value),
            )) = binding
            else {
                return Err(CheckedFxDefinitionSealError::UnprovableBuiltinConstraint {
                    parameter: parameter.id(),
                });
            };
            let components = runtime_numeric_components(value).ok_or(
                CheckedFxDefinitionSealError::UnprovableBuiltinConstraint {
                    parameter: parameter.id(),
                },
            )?;
            let minimum = integer_as_f64(constraint.inclusive_min_milli);
            let maximum = integer_as_f64(constraint.inclusive_max_milli);
            if components
                .into_iter()
                .all(|component| (minimum..=maximum).contains(&(component * 1_000.0)))
            {
                Ok(())
            } else {
                Err(CheckedFxDefinitionSealError::InvalidSchema)
            }
        }
    }
}

fn runtime_numeric_components(value: &FxDefinitionArgumentValue) -> Option<Vec<f64>> {
    let FxDefinitionArgumentValue::Runtime(value) = value else {
        return None;
    };
    Some(match value {
        FxRuntimeValue::F32(value) => vec![f64::from(value.get())],
        FxRuntimeValue::Length(value) => vec![f64::from(value.pixels())],
        FxRuntimeValue::Angle(value) => vec![f64::from(value.radians()).to_degrees()],
        FxRuntimeValue::Seconds(value) => vec![f64::from(value.seconds())],
        FxRuntimeValue::Vec2(value) => vec![f64::from(value.x.get()), f64::from(value.y.get())],
        FxRuntimeValue::Bool(_)
        | FxRuntimeValue::I32(_)
        | FxRuntimeValue::Color(_)
        | FxRuntimeValue::Transform2D(_)
        | FxRuntimeValue::U32(_) => return None,
    })
}

fn integer_as_f64(value: i64) -> f64 {
    let high = value / 1_000;
    let low = value % 1_000;
    high as f64 * 1_000.0 + low as f64
}

fn symbolic_bool(value: &CheckedFxSymbolicValue) -> Option<bool> {
    match value {
        CheckedFxSymbolicValue::Constant(CheckedFxConstant::Abi(
            FxDefinitionArgumentValue::Runtime(FxRuntimeValue::Bool(value)),
        )) => Some(*value),
        CheckedFxSymbolicValue::Parameter(_) | CheckedFxSymbolicValue::Program(_) => None,
        CheckedFxSymbolicValue::Constant(_) => None,
    }
}

fn validate_limits(graph: &CheckedGraph) -> Result<(), CheckedFxDefinitionSealError> {
    let limits = arcweft_lang_hir::fx::FX_EXPANSION_LIMITS;
    if graph.depth > limits.max_depth {
        return Err(CheckedFxDefinitionSealError::DepthLimit {
            actual: graph.depth,
            limit: limits.max_depth,
        });
    }
    if graph.visits > limits.max_visits {
        return Err(CheckedFxDefinitionSealError::VisitLimit {
            actual: graph.visits,
            limit: limits.max_visits,
        });
    }
    if graph.nodes > limits.max_nodes {
        return Err(CheckedFxDefinitionSealError::NodeLimit {
            actual: graph.nodes,
            limit: limits.max_nodes,
        });
    }
    Ok(())
}

fn check_depth(depth: usize) -> Result<(), CheckedFxDefinitionSealError> {
    let limit = arcweft_lang_hir::fx::FX_EXPANSION_LIMITS.max_depth;
    if depth > limit {
        Err(CheckedFxDefinitionSealError::DepthLimit {
            actual: depth,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_add(left: usize, right: usize) -> Result<usize, CheckedFxDefinitionSealError> {
    left.checked_add(right)
        .ok_or(CheckedFxDefinitionSealError::AccountingOverflow)
}

fn graph_depth(graph: &FxGraph) -> usize {
    graph
        .nodes()
        .iter()
        .map(|node| match node {
            FxNode::Conditional {
                then_graph,
                else_graph,
                ..
            } => 1 + graph_depth(then_graph).max(graph_depth(else_graph)),
            FxNode::Stack { children } => 1 + children.iter().map(graph_depth).max().unwrap_or(0),
            _ => 1,
        })
        .max()
        .unwrap_or(0)
}

fn transform_field_type(name: &str) -> Option<FxRuntimeType> {
    Some(match name {
        "translate_x" | "translate_y" | "origin_x" | "origin_y" => FxRuntimeType::Length,
        "scale_x" | "scale_y" | "opacity" => FxRuntimeType::F32,
        "skew_x" | "skew_y" | "rotation" => FxRuntimeType::Angle,
        _ => return None,
    })
}

fn runtime_length(
    value: Option<FxRuntimeValue>,
) -> Result<Option<Length>, CheckedFxDefinitionSealError> {
    match value {
        Some(FxRuntimeValue::Length(value)) => Ok(Some(value)),
        Some(_) => Err(CheckedFxDefinitionSealError::InvalidBody),
        None => Ok(None),
    }
}

fn runtime_angle(
    value: Option<FxRuntimeValue>,
) -> Result<Option<arcweft_presentation::fx::Angle>, CheckedFxDefinitionSealError> {
    match value {
        Some(FxRuntimeValue::Angle(value)) => Ok(Some(value)),
        Some(_) => Err(CheckedFxDefinitionSealError::InvalidBody),
        None => Ok(None),
    }
}

fn runtime_f32(
    value: Option<FxRuntimeValue>,
) -> Result<Option<FiniteF32>, CheckedFxDefinitionSealError> {
    match value {
        Some(FxRuntimeValue::F32(value)) => Ok(Some(value)),
        Some(_) => Err(CheckedFxDefinitionSealError::InvalidBody),
        None => Ok(None),
    }
}

fn fx_runtime_type(ty: &TypeKind) -> Option<FxRuntimeType> {
    Some(match ty {
        TypeKind::Bool => FxRuntimeType::Bool,
        TypeKind::I32 => FxRuntimeType::I32,
        TypeKind::U32 => FxRuntimeType::U32,
        TypeKind::F32 => FxRuntimeType::F32,
        TypeKind::Duration => FxRuntimeType::Seconds,
        TypeKind::CompileTimeScalar(value) => match value.kind() {
            CompileTimeScalarKind::Milli | CompileTimeScalarKind::Ratio => FxRuntimeType::F32,
            CompileTimeScalarKind::Length => FxRuntimeType::Length,
            CompileTimeScalarKind::Angle => FxRuntimeType::Angle,
            CompileTimeScalarKind::Color => FxRuntimeType::Color,
            CompileTimeScalarKind::PublicId => return None,
        },
        TypeKind::FixedVector(vector)
            if vector.dimensions() == VectorDimensions::Two
                && matches!(vector.component(), TypeKind::F32) =>
        {
            FxRuntimeType::Vec2
        }
        TypeKind::Named(name) if name == "Transform2D" => FxRuntimeType::Transform2D,
        _ => return None,
    })
}
