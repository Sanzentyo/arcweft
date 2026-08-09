//! Final semantic projection into runtime-plan facts.
//!
//! This module is the compiler-owned dependency inversion boundary between
//! semantic analysis and runtime-plan lowering. It consumes the exact accepted
//! final-HIR generation and never opens source text, rebuilds a detached HIR,
//! or consults the removed `TypeCheckReport` sidecar.

use std::collections::BTreeMap;

use arcweft_core::{
    entry::RuntimeNominalTypeId,
    pattern::RuntimeCheckedVariantCase,
    plan::{
        FlowRuntimeId, RuntimeIteratorEvidence, RuntimeIteratorIdentityWitnessCalls,
        RuntimeIteratorWitnessCalls, RuntimeIteratorWitnessEvidence,
        RuntimeIteratorWitnessExecutable, RuntimeLineId, RuntimeTraitMethodId,
    },
    step::RuntimeHostCallMode,
    time::LogicalDuration,
    value::{
        RuntimeInt, RuntimeIntrinsic, RuntimeSignedIntWidth, RuntimeUInt, RuntimeUnsignedIntWidth,
        RuntimeValue, runtime_sequence_from_literal_values,
    },
};
use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, ItemId, PatternId, StmtId},
    leaf::{
        HirBigUint, HirCharacterLiteral, HirDecimal, HirDurationLiteral, HirFloatLiteral,
        HirIntegerLiteral, HirLiteral, HirStringLiteral, HirUnitNumberLiteral,
    },
    project::HirExecutableProjectView,
    symbol::{CallableDeclarationKey, ProjectSymbolTable, nominal::ProjectNominalDeclarationId},
};
use arcweft_lang_sema::{
    assertion::AssertionRuntimePolicy,
    callable::{
        BuiltinCallableId, CallPoison, CallTargetFact, CallableFamily, CallableInstantiation,
        CheckedCallableExecution, MathCallableId, ReductionConstructorKind, ResolvedCallable,
        SignatureOrigin, StdFloatOperation,
    },
    effects::EffectId,
    final_analysis::{
        CheckedAssertionDisposition, CheckedExpressionResolution, CheckedItemRole,
        CheckedIteration, CheckedIteratorFamily, CheckedPatternResolution, CheckedProjectItemOwner,
        CheckedProjectNominal, CheckedSelectResolution, CheckedStatementRole,
        CheckedTraitConformance, CheckedTraitIdentity, CheckedValueResolution, CheckedVariantOwner,
        CheckedVariantResolution, FinalSemanticAnalysis, FinalSemanticAnalysisError,
    },
    types::{ArrayLength, TypeKind},
};
use arcweft_runtime_plan::{
    assertion_identity::RuntimeAssertionMode,
    semantic_facts::{
        RuntimeAssertionAdmission, RuntimeCallResultShape, RuntimeCheckedCapture,
        RuntimeNormalizedType, RuntimePlanSemanticFactInput, RuntimePlanSemanticFacts,
        RuntimeProjectCallable, RuntimeProjectItem, RuntimeReductionConstructor,
        RuntimeRegisteredValueId, RuntimeResolvedCall, RuntimeResolvedCallArgument,
        RuntimeResolvedCallTarget, RuntimeResolvedNominal, RuntimeResolvedSelect,
        RuntimeResolvedValue, RuntimeResolvedVariant, RuntimeSemanticFactsError,
        RuntimeSemanticTypeId, RuntimeSequenceKind, RuntimeTraitIdentity, RuntimeTraitMethodFact,
        RuntimeTypeShape,
    },
};
use thiserror::Error;

/// Failure to project one accepted semantic generation into the closed runtime
/// fact vocabulary.
#[derive(Debug, Error)]
pub enum RuntimeSemanticProjectionError {
    #[error(transparent)]
    Generation(Box<FinalSemanticAnalysisError>),
    #[error(transparent)]
    Facts(Box<RuntimeSemanticFactsError>),
    #[error("final semantic owner {owner:?} belongs to no executable HIR module")]
    MissingModule { owner: ExprId },
    #[error("project nominal {declaration:?} is absent from the accepted symbol table")]
    MissingNominal {
        declaration: Box<ProjectNominalDeclarationId>,
    },
    #[error("flow item {owner:?} has no executable absolute or named identity")]
    InvalidFlowIdentity { owner: ItemId },
    #[error("expression literal {owner:?} has no exact runtime value: {reason}")]
    ExpressionLiteral { owner: ExprId, reason: String },
    #[error("pattern literal {owner:?} has no exact runtime value: {reason}")]
    PatternLiteral { owner: PatternId, reason: String },
    #[error("semantic type cannot enter runtime lowering: {reason}")]
    Type { reason: String },
    #[error("value expression {owner:?} has no exact runtime projection: {reason}")]
    Value { owner: ExprId, reason: String },
    #[error("call {owner:?} is not an accepted executable call: {reason}")]
    Call { owner: ExprId, reason: String },
    #[error("iteration statement {owner:?} references an unbound runtime trait method")]
    MissingIterationMethod { owner: StmtId },
    #[error("one checked iterator conformance was assigned conflicting self types")]
    InconsistentIterationConformance,
    #[error("assertion statement {owner:?} has an invalid runtime disposition")]
    InvalidAssertionDisposition { owner: StmtId },
}

impl From<FinalSemanticAnalysisError> for RuntimeSemanticProjectionError {
    fn from(error: FinalSemanticAnalysisError) -> Self {
        Self::Generation(Box::new(error))
    }
}

impl From<RuntimeSemanticFactsError> for RuntimeSemanticProjectionError {
    fn from(error: RuntimeSemanticFactsError) -> Self {
        Self::Facts(Box::new(error))
    }
}

/// Projects the sole accepted semantic generation into runtime-plan facts.
///
/// The resulting fact set is validated against the same executable project
/// lease before it is returned. No partially projected fact set is observable.
#[expect(
    clippy::too_many_lines,
    reason = "the final semantic fact admission matrix is intentionally exhaustive and atomic"
)]
pub fn project_runtime_semantic_facts(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimePlanSemanticFacts, RuntimeSemanticProjectionError> {
    analysis.validate_generation(project, symbols)?;
    let mut input = RuntimePlanSemanticFactInput::new();

    let iteration_methods = runtime_iteration_methods(analysis)?;
    let mut method_ids = BTreeMap::new();
    for (ordinal, (conformance, self_type)) in iteration_methods.iter().enumerate() {
        let id = RuntimeTraitMethodId(ordinal);
        method_ids.insert(conformance.clone(), id);
        input.push_trait_method(RuntimeTraitMethodFact::new(
            id,
            conformance.implementation(),
            conformance.method(),
            runtime_trait_identity(conformance.trait_identity()),
            self_type.source_label(),
        ));
    }

    for (owner, item) in analysis.items() {
        if matches!(item.role(), CheckedItemRole::Flow { .. }) {
            let symbol = symbols
                .flow_symbol_for_item(owner)
                .ok_or(RuntimeSemanticProjectionError::InvalidFlowIdentity { owner })?;
            let CallableDeclarationKey::Flow(declaration) = symbol.declaration() else {
                return Err(RuntimeSemanticProjectionError::InvalidFlowIdentity { owner });
            };
            let identity = runtime_flow_identity(declaration)
                .map_err(|_| RuntimeSemanticProjectionError::InvalidFlowIdentity { owner })?;
            input.push_flow(owner, identity);
        }
    }

    for (owner, ty) in analysis.types() {
        input.push_type(owner, runtime_type(ty, symbols)?);
    }

    for (owner, expression) in analysis.expressions() {
        match expression.resolution() {
            CheckedExpressionResolution::Structural => {
                let module = project
                    .modules()
                    .find_map(|(_, module)| {
                        (module.module_id() == owner.module()).then_some(module.as_ref())
                    })
                    .ok_or(RuntimeSemanticProjectionError::MissingModule { owner })?;
                let hir = module.resolve_expr(owner).map_err(|error| {
                    RuntimeSemanticProjectionError::ExpressionLiteral {
                        owner,
                        reason: error.to_string(),
                    }
                })?;
                if let HirExprKind::NumericBracketSequence(sequence) = hir.kind() {
                    let TypeKind::Vec(item) = expression.ty() else {
                        return Err(RuntimeSemanticProjectionError::ExpressionLiteral {
                            owner,
                            reason: "compact numeric sequence did not retain its checked item type"
                                .to_owned(),
                        });
                    };
                    let values = sequence
                        .elements()
                        .iter()
                        .map(|element| runtime_integer_magnitude(element.magnitude(), item))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|reason| RuntimeSemanticProjectionError::ExpressionLiteral {
                            owner,
                            reason,
                        })?;
                    input.push_expression_literal(
                        owner,
                        runtime_sequence_from_literal_values(values),
                    );
                }
            }
            CheckedExpressionResolution::Literal(literal) => {
                input.push_expression_literal(
                    owner,
                    runtime_literal(literal, expression.ty()).map_err(|reason| {
                        RuntimeSemanticProjectionError::ExpressionLiteral { owner, reason }
                    })?,
                );
            }
            CheckedExpressionResolution::Value(value) => {
                let project_item_is_runtime_entity =
                    if matches!(value, CheckedValueResolution::ProjectItem(_)) {
                        let module = project
                            .modules()
                            .find_map(|(_, module)| {
                                (module.module_id() == owner.module()).then_some(module.as_ref())
                            })
                            .ok_or(RuntimeSemanticProjectionError::MissingModule { owner })?;
                        matches!(
                            module
                                .resolve_expr(owner)
                                .map_err(|error| RuntimeSemanticProjectionError::Value {
                                    owner,
                                    reason: error.to_string(),
                                })?
                                .kind(),
                            HirExprKind::EntityReference(_)
                        )
                    } else {
                        false
                    };
                if let Some(value) =
                    runtime_value_resolution(value, expression.ty(), project_item_is_runtime_entity)
                        .map_err(|reason| RuntimeSemanticProjectionError::Value { owner, reason })?
                {
                    input.push_value(owner, value);
                }
            }
            CheckedExpressionResolution::Select(select) => {
                if let Some(select) = runtime_select(select) {
                    input.push_select(owner, select);
                }
            }
            CheckedExpressionResolution::Nominal(nominal) => {
                input.push_nominal(owner, runtime_nominal(nominal));
            }
            CheckedExpressionResolution::Variant(variant) => {
                input.push_expression_variant(owner, runtime_variant(variant, symbols, analysis)?);
            }
            CheckedExpressionResolution::PostfixBracket(resolution) => {
                input.push_postfix_candidate(owner, resolution.candidate());
            }
            CheckedExpressionResolution::DialogueLineReference(target) => {
                let line =
                    RuntimeLineId::from_source_entity_body(target.as_str()).map_err(|error| {
                        RuntimeSemanticProjectionError::Value {
                            owner,
                            reason: error.to_string(),
                        }
                    })?;
                input.push_value(owner, RuntimeResolvedValue::DialogueLine(line));
            }
            CheckedExpressionResolution::Call
            | CheckedExpressionResolution::ViewCall(_)
            | CheckedExpressionResolution::ViewCallee(_)
            | CheckedExpressionResolution::StyleValue(_)
            | CheckedExpressionResolution::StyleCallee(_)
            | CheckedExpressionResolution::DialogueApplication { .. }
            | CheckedExpressionResolution::Effect(_) => {}
        }
    }

    for (owner, pattern) in analysis.patterns() {
        match pattern.resolution() {
            CheckedPatternResolution::Literal(literal) => {
                input.push_pattern_literal(
                    owner,
                    runtime_literal(literal, pattern.ty()).map_err(|reason| {
                        RuntimeSemanticProjectionError::PatternLiteral { owner, reason }
                    })?,
                );
            }
            CheckedPatternResolution::Nominal(nominal) => {
                input.push_pattern_nominal(owner, runtime_nominal(nominal));
            }
            CheckedPatternResolution::Variant(variant) => {
                input.push_pattern_variant(owner, runtime_variant(variant, symbols, analysis)?);
            }
            CheckedPatternResolution::Entity(item) => {
                input.push_pattern_item(owner, runtime_project_item(item)?);
            }
            CheckedPatternResolution::Structural => {}
        }
    }

    for (owner, statement) in analysis.statements() {
        match statement.role() {
            CheckedStatementRole::Assertion(disposition) => {
                input.push_assertion(owner, runtime_assertion(owner, *disposition)?);
            }
            CheckedStatementRole::Iteration(iteration) => {
                input.push_iteration(owner, runtime_iteration(owner, iteration, &method_ids)?);
            }
            CheckedStatementRole::Ordinary
            | CheckedStatementRole::Suspension
            | CheckedStatementRole::Yield
            | CheckedStatementRole::UnsafeAudit => {}
        }
    }

    for (owner, capture) in analysis.captures() {
        input.push_capture(RuntimeCheckedCapture::new(
            owner,
            runtime_type(capture.ty(), symbols)?,
        ));
    }

    for (owner, call) in analysis.calls() {
        input.push_call(owner, runtime_call(owner, call, symbols, analysis)?);
    }

    RuntimePlanSemanticFacts::try_new(project, input).map_err(Into::into)
}

#[expect(
    clippy::too_many_lines,
    reason = "the closed semantic type vocabulary must be projected exhaustively in one boundary"
)]
fn runtime_type(
    ty: &TypeKind,
    symbols: &ProjectSymbolTable,
) -> Result<RuntimeNormalizedType, RuntimeSemanticProjectionError> {
    let identity = RuntimeSemanticTypeId::from_bytes(*ty.semantic_identity_digest().as_bytes());
    let nested = |ty: &TypeKind| runtime_type(ty, symbols).map(Box::new);
    let shape = match ty {
        TypeKind::Unit => RuntimeTypeShape::Unit,
        TypeKind::Never => RuntimeTypeShape::Never,
        TypeKind::Bool => RuntimeTypeShape::Bool,
        TypeKind::I8 => RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I8),
        TypeKind::I16 => RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I16),
        TypeKind::I32 => RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I32),
        TypeKind::I64 => RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I64),
        TypeKind::I128 => RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I128),
        TypeKind::ISize => RuntimeTypeShape::Signed(RuntimeSignedIntWidth::ISize),
        TypeKind::U8 => RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U8),
        TypeKind::U16 => RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U16),
        TypeKind::U32 => RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U32),
        TypeKind::U64 => RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U64),
        TypeKind::U128 => RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U128),
        TypeKind::USize => RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::USize),
        TypeKind::F32 => RuntimeTypeShape::F32,
        TypeKind::F64 => RuntimeTypeShape::F64,
        TypeKind::String => RuntimeTypeShape::String,
        TypeKind::Char => RuntimeTypeShape::Char,
        TypeKind::Bytes => RuntimeTypeShape::Bytes,
        TypeKind::Duration => RuntimeTypeShape::Duration,
        TypeKind::Ref(_) => RuntimeTypeShape::EntityReference,
        TypeKind::Range(item) => RuntimeTypeShape::Range(nested(item)?),
        TypeKind::IteratorState { item, .. } => RuntimeTypeShape::Iterator(nested(item)?),
        TypeKind::Vec(item) => RuntimeTypeShape::Sequence {
            kind: RuntimeSequenceKind::Vec,
            item: nested(item)?,
        },
        TypeKind::Array {
            item,
            len: ArrayLength::Const(length),
        } => RuntimeTypeShape::Array {
            item: nested(item)?,
            length: *length,
        },
        TypeKind::Slice(item) => RuntimeTypeShape::Sequence {
            kind: RuntimeSequenceKind::Slice,
            item: nested(item)?,
        },
        TypeKind::Seq(item) => RuntimeTypeShape::Sequence {
            kind: RuntimeSequenceKind::Seq,
            item: nested(item)?,
        },
        TypeKind::Map { key, value, .. } => RuntimeTypeShape::Map {
            key: nested(key)?,
            value: nested(value)?,
        },
        TypeKind::BorrowRef { inner, .. } => RuntimeTypeShape::Reference(nested(inner)?),
        TypeKind::Need { ready, error } => RuntimeTypeShape::Need {
            ready: nested(ready)?,
            error: nested(error)?,
        },
        TypeKind::Stream { item, error } => RuntimeTypeShape::Stream {
            item: nested(item)?,
            error: nested(error)?,
        },
        TypeKind::Source { item, error } => RuntimeTypeShape::Source {
            item: nested(item)?,
            error: nested(error)?,
        },
        TypeKind::Result { ok, error } => RuntimeTypeShape::Result {
            value: nested(ok)?,
            error: nested(error)?,
        },
        TypeKind::Option(item) => RuntimeTypeShape::Option(nested(item)?),
        TypeKind::ThreadHandle(item) => RuntimeTypeShape::ThreadHandle(nested(item)?),
        TypeKind::Shared(item) => RuntimeTypeShape::Shared(nested(item)?),
        TypeKind::Function {
            params,
            return_type,
            ..
        } => RuntimeTypeShape::Function {
            parameters: params
                .iter()
                .map(|parameter| runtime_type(parameter, symbols))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
            result: nested(return_type)?,
        },
        TypeKind::ProjectNominal(nominal) => {
            let declaration = symbols.nominal(nominal.declaration()).ok_or_else(|| {
                RuntimeSemanticProjectionError::MissingNominal {
                    declaration: Box::new(nominal.declaration().clone()),
                }
            })?;
            RuntimeTypeShape::ProjectNominal {
                nominal: RuntimeResolvedNominal::new(
                    nominal.declaration().clone(),
                    declaration.owner(),
                    identity,
                ),
                arguments: nominal
                    .arguments()
                    .iter()
                    .map(|argument| runtime_type(argument, symbols))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            }
        }
        TypeKind::AcceptedNominal(nominal) => RuntimeTypeShape::Named {
            nominal: RuntimeNominalTypeId::try_new(
                nominal.declaration().canonical_path().canonical_string(),
            )
            .map_err(|error| RuntimeSemanticProjectionError::Type {
                reason: format!("checked accepted nominal identity is invalid: {error}"),
            })?,
        },
        TypeKind::Tuple(items) => RuntimeTypeShape::Tuple(
            items
                .iter()
                .map(|item| runtime_type(item, symbols))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        ),
        TypeKind::Choice(items) => RuntimeTypeShape::Choice(
            items
                .iter()
                .map(|item| runtime_type(item, symbols))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        ),
        TypeKind::Named(name) => RuntimeTypeShape::Named {
            nominal: RuntimeNominalTypeId::try_new(name.clone()).map_err(|error| {
                RuntimeSemanticProjectionError::Type {
                    reason: format!("checked named type identity is invalid: {error}"),
                }
            })?,
        },
        TypeKind::Error(poison) => {
            return Err(RuntimeSemanticProjectionError::Type {
                reason: format!(
                    "semantic poison {} reached runtime projection",
                    poison.index()
                ),
            });
        }
        TypeKind::Array { .. }
        | TypeKind::TextCluster
        | TypeKind::DisplayText
        | TypeKind::DebugStatePath
        | TypeKind::ObservationFieldPath
        | TypeKind::Probe(_)
        | TypeKind::Predicate
        | TypeKind::Observation
        | TypeKind::ObservedObject
        | TypeKind::AgentBBox
        | TypeKind::ActionName
        | TypeKind::ActionTarget
        | TypeKind::ActionResult
        | TypeKind::AgentValue
        | TypeKind::DataFormat
        | TypeKind::DataShape
        | TypeKind::AgentEntityMetadata
        | TypeKind::AgentSourceAnchor
        | TypeKind::AgentProjectGraphNeighborhood
        | TypeKind::AgentProjectGraphSymbol
        | TypeKind::AgentProjectGraphEdge
        | TypeKind::CaptureTarget
        | TypeKind::CaptureRef
        | TypeKind::AgentResource
        | TypeKind::AgentResourceBody
        | TypeKind::RagContextPack
        | TypeKind::Handle { .. }
        | TypeKind::GenericParam(_)
        | TypeKind::OpenNominal(_)
        | TypeKind::Projection { .. }
        | TypeKind::CharacterPatch(_)
        | TypeKind::FocusPatch
        | TypeKind::CharacterNominal(_) => RuntimeTypeShape::Opaque,
    };
    Ok(RuntimeNormalizedType::new(identity, shape))
}

fn runtime_literal(literal: &HirLiteral, ty: &TypeKind) -> Result<RuntimeValue, String> {
    match literal {
        HirLiteral::String(HirStringLiteral::Value(value)) => {
            Ok(RuntimeValue::String(value.to_string()))
        }
        HirLiteral::Character(HirCharacterLiteral::Value(value)) => Ok(RuntimeValue::Char(*value)),
        HirLiteral::Integer(HirIntegerLiteral::Value { magnitude, .. }) => {
            runtime_integer_magnitude(magnitude, ty)
        }
        HirLiteral::Float(HirFloatLiteral::Value { decimal, .. }) => runtime_decimal(decimal, ty),
        HirLiteral::UnitNumber(HirUnitNumberLiteral::Value { decimal, .. }) => {
            runtime_decimal(decimal, ty)
        }
        HirLiteral::Boolean(value) => Ok(RuntimeValue::Bool(*value)),
        HirLiteral::Duration(HirDurationLiteral::Value(value)) => {
            let nanos = big_uint_to_u128(value.semantic_value().nanoseconds())
                .and_then(|value| u64::try_from(value).ok())
                .ok_or_else(|| {
                    "Duration literal exceeds the runtime u64 nanosecond domain".to_owned()
                })?;
            Ok(RuntimeValue::Duration(LogicalDuration::from_nanos(nanos)))
        }
        HirLiteral::String(HirStringLiteral::Invalid(issue)) => Err(issue.to_string()),
        HirLiteral::Character(HirCharacterLiteral::Invalid(issue)) => Err(issue.to_string()),
        HirLiteral::Integer(HirIntegerLiteral::Invalid(issue)) => Err(issue.to_string()),
        HirLiteral::Float(HirFloatLiteral::Invalid(issue)) => Err(issue.to_string()),
        HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(issue)) => Err(issue.to_string()),
        HirLiteral::Duration(HirDurationLiteral::Invalid(issue)) => Err(issue.to_string()),
    }
}

fn runtime_integer_magnitude(
    magnitude: &HirBigUint,
    ty: &TypeKind,
) -> Result<RuntimeValue, String> {
    let value = big_uint_to_u128(magnitude)
        .ok_or_else(|| "integer literal exceeds the runtime u128 magnitude domain".to_owned())?;
    let signed = |width| {
        let value = i128::try_from(value)
            .map_err(|_| "positive integer literal exceeds the runtime i128 domain".to_owned())?;
        RuntimeInt::from_i128(width, value)
            .map(RuntimeValue::Int)
            .ok_or_else(|| format!("integer literal does not fit {width:?}"))
    };
    let unsigned = |width| {
        RuntimeUInt::from_u128(width, value)
            .map(RuntimeValue::UInt)
            .ok_or_else(|| format!("integer literal does not fit {width:?}"))
    };
    match ty {
        TypeKind::I8 => signed(RuntimeSignedIntWidth::I8),
        TypeKind::I16 => signed(RuntimeSignedIntWidth::I16),
        TypeKind::I32 => signed(RuntimeSignedIntWidth::I32),
        TypeKind::I64 => signed(RuntimeSignedIntWidth::I64),
        TypeKind::I128 => signed(RuntimeSignedIntWidth::I128),
        TypeKind::ISize => signed(RuntimeSignedIntWidth::ISize),
        TypeKind::U8 => unsigned(RuntimeUnsignedIntWidth::U8),
        TypeKind::U16 => unsigned(RuntimeUnsignedIntWidth::U16),
        TypeKind::U32 => unsigned(RuntimeUnsignedIntWidth::U32),
        TypeKind::U64 => unsigned(RuntimeUnsignedIntWidth::U64),
        TypeKind::U128 => unsigned(RuntimeUnsignedIntWidth::U128),
        TypeKind::USize => unsigned(RuntimeUnsignedIntWidth::USize),
        _ => Err("integer literal has a non-integer checked type".to_owned()),
    }
}

fn big_uint_to_u128(value: &HirBigUint) -> Option<u128> {
    if value.limbs_le().len() > 4 {
        return None;
    }
    Some(
        value
            .limbs_le()
            .iter()
            .rev()
            .fold(0_u128, |accumulator, limb| {
                (accumulator << 32) | u128::from(*limb)
            }),
    )
}

fn runtime_decimal(decimal: &HirDecimal, ty: &TypeKind) -> Result<RuntimeValue, String> {
    let digits = decimal
        .coefficient()
        .digits()
        .iter()
        .map(|digit| char::from(b'0' + *digit))
        .collect::<String>();
    let exponent = i64::from(decimal.exponent10()) - i64::from(decimal.scale());
    let canonical = format!("{digits}e{exponent}");
    match ty {
        TypeKind::F32 => canonical
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .map(RuntimeValue::F32)
            .ok_or_else(|| "decimal literal is outside the finite f32 domain".to_owned()),
        TypeKind::F64 => canonical
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .map(RuntimeValue::F64)
            .ok_or_else(|| "decimal literal is outside the finite f64 domain".to_owned()),
        _ => Err(
            "decimal literal has no runtime scalar representation for its checked type".to_owned(),
        ),
    }
}

fn runtime_value_resolution(
    value: &CheckedValueResolution,
    ty: &TypeKind,
    project_item_is_runtime_entity: bool,
) -> Result<Option<RuntimeResolvedValue>, String> {
    Ok(Some(match value {
        CheckedValueResolution::Local(local) => RuntimeResolvedValue::Local(*local),
        CheckedValueResolution::ProjectItem(item) if project_item_is_runtime_entity => {
            RuntimeResolvedValue::ProjectItem(
                runtime_project_item(item).map_err(|error| error.to_string())?,
            )
        }
        // A retained item selected through a Path is semantic input owned by
        // its enclosing typed construct (for example Dialogue application),
        // not a standalone runtime scalar. The parent checked fact carries
        // the exact retained owner into runtime-plan lowering.
        // The selected call fact, not a callee path expression, owns the
        // generation-bound checked callable digest. A bare callable reference
        // needs a typed function-value identity before it can be executable.
        CheckedValueResolution::ProjectCallable(_)
        | CheckedValueResolution::ProjectItem(_)
        // Entry references are generation-bound tooling/selection identities;
        // they are not executable scalar values in the runtime expression VM.
        | CheckedValueResolution::Entry(_) => return Ok(None),
        CheckedValueResolution::Registered(registered) => RuntimeResolvedValue::Registered(
            RuntimeRegisteredValueId::from_bytes(*registered.as_bytes()),
        ),
        CheckedValueResolution::Constant(literal) => {
            RuntimeResolvedValue::Constant(runtime_literal(literal, ty)?)
        }
    }))
}

fn runtime_project_item(
    item: &arcweft_lang_sema::final_analysis::CheckedProjectItem,
) -> Result<RuntimeProjectItem, RuntimeSemanticProjectionError> {
    match item.owner() {
        CheckedProjectItemOwner::Retained(owner) => Ok(RuntimeProjectItem::new_retained(
            item.public_id().clone(),
            item.family(),
            *owner,
        )),
        CheckedProjectItemOwner::Flow {
            declaration,
            item: owner,
        } => {
            let CallableDeclarationKey::Flow(flow) = declaration else {
                unreachable!("checked structural Flow owns a Flow declaration key")
            };
            let runtime = runtime_flow_identity(flow).map_err(|_| {
                RuntimeSemanticProjectionError::InvalidFlowIdentity { owner: *owner }
            })?;
            Ok(RuntimeProjectItem::new_structural_flow(
                item.public_id().clone(),
                *owner,
                runtime,
            ))
        }
        CheckedProjectItemOwner::External(_) => Ok(RuntimeProjectItem::new_external_character(
            item.public_id().clone(),
        )),
    }
}

fn runtime_flow_identity(
    declaration: &arcweft_lang_hir::symbol::FlowDeclarationId,
) -> Result<FlowRuntimeId, arcweft_core::runtime_id::RuntimeIdError> {
    FlowRuntimeId::from_checked_declaration_digest(
        declaration.semantic_digest().into_bytes(),
        declaration.public_id().as_str(),
    )
}

fn runtime_select(select: &CheckedSelectResolution) -> Option<RuntimeResolvedSelect> {
    Some(match select {
        CheckedSelectResolution::DialogueView { .. } => return None,
        CheckedSelectResolution::Field { nominal, name } => RuntimeResolvedSelect::Field {
            nominal: nominal.as_ref().map(runtime_nominal),
            name: name.clone(),
        },
        CheckedSelectResolution::TupleElement { ordinal } => {
            RuntimeResolvedSelect::TupleElement { ordinal: *ordinal }
        }
        CheckedSelectResolution::RecordElement {
            nominal,
            ordinal,
            name,
        } => RuntimeResolvedSelect::RecordElement {
            nominal: nominal.as_ref().map(runtime_nominal),
            ordinal: *ordinal,
            name: name.clone(),
        },
    })
}

fn runtime_nominal(nominal: &CheckedProjectNominal) -> RuntimeResolvedNominal {
    RuntimeResolvedNominal::new(
        nominal.declaration().clone(),
        nominal.owner(),
        RuntimeSemanticTypeId::from_bytes(*nominal.identity().as_bytes()),
    )
}

#[expect(
    clippy::too_many_lines,
    reason = "variant ownership is a closed semantic matrix whose validation stays atomic"
)]
fn runtime_variant(
    variant: &CheckedVariantResolution,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<
    arcweft_runtime_plan::semantic_facts::RuntimeResolvedVariant,
    RuntimeSemanticProjectionError,
> {
    let projected = match variant.owner() {
        CheckedVariantOwner::Project(nominal) => {
            let declaration = symbols.nominal(nominal.declaration()).ok_or_else(|| {
                RuntimeSemanticProjectionError::MissingNominal {
                    declaration: Box::new(nominal.declaration().clone()),
                }
            })?;
            let arcweft_lang_hir::symbol::nominal::ProjectNominalBody::Enum { variants } =
                declaration.body()
            else {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason: "checked project variant owner is not an enum".to_owned(),
                });
            };
            let selected = usize::try_from(variant.ordinal())
                .ok()
                .and_then(|ordinal| variants.get(ordinal))
                .ok_or_else(|| RuntimeSemanticProjectionError::Type {
                    reason: "checked project variant ordinal is outside its enum declaration"
                        .to_owned(),
                })?;
            RuntimeResolvedVariant::project(
                runtime_nominal(nominal),
                variant.ordinal(),
                selected,
                runtime_project_variant_cases(declaration, nominal, symbols, analysis)?,
            )
        }
        CheckedVariantOwner::CharacterNominal { nominal, cases } => {
            let ty = TypeKind::CharacterNominal(nominal.clone());
            RuntimeResolvedVariant::character(
                RuntimeSemanticTypeId::from_bytes(*ty.semantic_identity_digest().as_bytes()),
                RuntimeNominalTypeId::from_checked_digest(
                    *ty.semantic_identity_digest().as_bytes(),
                ),
                cases
                    .iter()
                    .map(|name| RuntimeCheckedVariantCase {
                        name: name.clone(),
                        payload: None,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                variant.ordinal(),
                variant.name(),
            )
        }
        CheckedVariantOwner::BuiltinClosed {
            nominal,
            semantic_identity,
            cases,
        } => {
            let cases = cases
                .iter()
                .map(|case| {
                    let payload = case
                        .payload()
                        .map(|payload| {
                            runtime_type(payload, symbols)?
                                .checked_type()
                                .map(Box::new)
                                .map_err(|reason| RuntimeSemanticProjectionError::Type { reason })
                        })
                        .transpose()?;
                    Ok(RuntimeCheckedVariantCase {
                        name: case.name().to_owned(),
                        payload,
                    })
                })
                .collect::<Result<Vec<_>, RuntimeSemanticProjectionError>>()?
                .into_boxed_slice();
            RuntimeResolvedVariant::builtin_closed(
                RuntimeSemanticTypeId::from_bytes(*semantic_identity.as_bytes()),
                RuntimeNominalTypeId::try_new(nominal.as_str().to_owned()).map_err(|error| {
                    RuntimeSemanticProjectionError::Type {
                        reason: format!(
                            "checked base-environment enum identity is invalid: {error}"
                        ),
                    }
                })?,
                cases,
                variant.ordinal(),
                variant.name(),
            )
        }
        CheckedVariantOwner::Option { item } => match variant.ordinal() {
            0 => RuntimeResolvedVariant::option_some(runtime_type(item, symbols)?),
            1 => RuntimeResolvedVariant::option_none(runtime_type(item, symbols)?),
            _ => {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason: "checked Option variant ordinal is outside the closed case set"
                        .to_owned(),
                });
            }
        },
        CheckedVariantOwner::Result { ok, error } => {
            let ok = runtime_type(ok, symbols)?;
            let error = runtime_type(error, symbols)?;
            match variant.ordinal() {
                0 => RuntimeResolvedVariant::result_ok(ok, error),
                1 => RuntimeResolvedVariant::result_err(ok, error),
                _ => {
                    return Err(RuntimeSemanticProjectionError::Type {
                        reason: "checked Result variant ordinal is outside the closed case set"
                            .to_owned(),
                    });
                }
            }
        }
    };
    Ok(projected)
}

fn runtime_assertion(
    owner: StmtId,
    disposition: CheckedAssertionDisposition,
) -> Result<RuntimeAssertionAdmission, RuntimeSemanticProjectionError> {
    match disposition {
        CheckedAssertionDisposition::PendingProof => {
            Err(RuntimeSemanticProjectionError::InvalidAssertionDisposition { owner })
        }
        CheckedAssertionDisposition::Discharged => Ok(RuntimeAssertionAdmission::Discharged),
        CheckedAssertionDisposition::OmittedDebug => Ok(RuntimeAssertionAdmission::OmittedDebug),
        CheckedAssertionDisposition::Runtime(AssertionRuntimePolicy::AlwaysGuard) => Ok(
            RuntimeAssertionAdmission::Runtime(RuntimeAssertionMode::Check),
        ),
        CheckedAssertionDisposition::Runtime(AssertionRuntimePolicy::DebugGuard) => Ok(
            RuntimeAssertionAdmission::Runtime(RuntimeAssertionMode::Debug),
        ),
    }
}

fn runtime_iteration(
    owner: StmtId,
    iteration: &CheckedIteration,
    methods: &BTreeMap<CheckedTraitConformance, RuntimeTraitMethodId>,
) -> Result<RuntimeIteratorEvidence, RuntimeSemanticProjectionError> {
    match iteration {
        CheckedIteration::Builtin { family, .. } => Ok(match family {
            CheckedIteratorFamily::Range => RuntimeIteratorEvidence::builtin_range(),
            CheckedIteratorFamily::Seq => RuntimeIteratorEvidence::builtin_seq(),
            CheckedIteratorFamily::Stream => RuntimeIteratorEvidence::builtin_stream(),
            CheckedIteratorFamily::Vec => RuntimeIteratorEvidence::builtin_vec(),
            CheckedIteratorFamily::Array => RuntimeIteratorEvidence::builtin_array(),
            CheckedIteratorFamily::Slice => RuntimeIteratorEvidence::builtin_slice(),
        }),
        CheckedIteration::Witness {
            item,
            into_iter,
            into_iterator,
            iterator,
            ..
        } => Ok(RuntimeIteratorEvidence::Witness(
            RuntimeIteratorWitnessEvidence {
                item_type: item.source_label(),
                into_iter_type: into_iter.source_label(),
                executable: RuntimeIteratorWitnessExecutable::TraitCalls(
                    RuntimeIteratorWitnessCalls {
                        into_iter: *methods.get(into_iterator).ok_or(
                            RuntimeSemanticProjectionError::MissingIterationMethod { owner },
                        )?,
                        next: *methods.get(iterator).ok_or(
                            RuntimeSemanticProjectionError::MissingIterationMethod { owner },
                        )?,
                    },
                ),
            },
        )),
        CheckedIteration::IteratorWitness {
            source,
            item,
            iterator,
        } => Ok(RuntimeIteratorEvidence::Witness(
            RuntimeIteratorWitnessEvidence {
                item_type: item.source_label(),
                into_iter_type: source.source_label(),
                executable: RuntimeIteratorWitnessExecutable::IdentityIntoIterator(
                    RuntimeIteratorIdentityWitnessCalls {
                        next: *methods.get(iterator).ok_or(
                            RuntimeSemanticProjectionError::MissingIterationMethod { owner },
                        )?,
                    },
                ),
            },
        )),
    }
}

fn runtime_iteration_methods(
    analysis: &FinalSemanticAnalysis,
) -> Result<BTreeMap<CheckedTraitConformance, TypeKind>, RuntimeSemanticProjectionError> {
    let mut methods = BTreeMap::new();
    let mut insert = |conformance: &CheckedTraitConformance, self_type: &TypeKind| match methods
        .insert(conformance.clone(), self_type.clone())
    {
        Some(existing) if existing != *self_type => {
            Err(RuntimeSemanticProjectionError::InconsistentIterationConformance)
        }
        _ => Ok(()),
    };
    for (_, statement) in analysis.statements() {
        let CheckedStatementRole::Iteration(iteration) = statement.role() else {
            continue;
        };
        match iteration.as_ref() {
            CheckedIteration::Builtin { .. } => {}
            CheckedIteration::Witness {
                source,
                into_iter,
                into_iterator,
                iterator,
                ..
            } => {
                insert(into_iterator, source)?;
                insert(iterator, into_iter)?;
            }
            CheckedIteration::IteratorWitness {
                source, iterator, ..
            } => insert(iterator, source)?,
        }
    }
    Ok(methods)
}

fn runtime_trait_identity(identity: &CheckedTraitIdentity) -> RuntimeTraitIdentity {
    match identity {
        CheckedTraitIdentity::Project(owner) => RuntimeTraitIdentity::Project(*owner),
        CheckedTraitIdentity::StandardIterator => RuntimeTraitIdentity::StandardIterator,
        CheckedTraitIdentity::StandardIntoIterator => RuntimeTraitIdentity::StandardIntoIterator,
    }
}

fn runtime_call(
    owner: ExprId,
    facts: &arcweft_lang_sema::callable::CallTargetFacts,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeResolvedCall, RuntimeSemanticProjectionError> {
    if facts.poison() != CallPoison::Clean {
        return Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: "recovered or rejected call facts cannot enter runtime lowering".to_owned(),
        });
    }
    let CallTargetFact::Selected { selected, .. } = facts.target() else {
        return Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: "call target is not uniquely selected".to_owned(),
        });
    };
    let target = runtime_call_target(owner, facts, selected, symbols, analysis)?;
    let mut arguments = facts
        .arguments()
        .iter()
        .enumerate()
        .map(|(ordinal, _)| {
            u32::try_from(ordinal)
                .map(|ordinal| RuntimeResolvedCallArgument::Authored { ordinal })
                .map_err(|_| RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "authored argument ordinal exceeds u32".to_owned(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    match selected.instantiation() {
        CallableInstantiation::Receiver { .. } => {
            arguments.insert(0, RuntimeResolvedCallArgument::Receiver);
        }
        CallableInstantiation::DataLast { parameter, .. } => {
            let position = parameter.get();
            if position > arguments.len() {
                return Err(RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "data-last receiver parameter is outside the resolved argument order"
                        .to_owned(),
                });
            }
            arguments.insert(position, RuntimeResolvedCallArgument::Receiver);
        }
        CallableInstantiation::None
        | CallableInstantiation::ExpectedEnum { .. }
        | CallableInstantiation::Result { .. }
        | CallableInstantiation::Option { .. }
        | CallableInstantiation::Character { .. }
        | CallableInstantiation::TypeReceiver { .. }
        | CallableInstantiation::Curried { .. } => {}
    }
    Ok(RuntimeResolvedCall::new(
        target,
        arguments,
        if facts.next_group().is_some() {
            RuntimeCallResultShape::PartialFunction
        } else {
            RuntimeCallResultShape::Value
        },
    ))
}

#[expect(
    clippy::too_many_lines,
    reason = "checked callable families are exhaustively selected at this runtime boundary"
)]
fn runtime_call_target(
    owner: ExprId,
    facts: &arcweft_lang_sema::callable::CallTargetFacts,
    selected: &ResolvedCallable,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeResolvedCallTarget, RuntimeSemanticProjectionError> {
    if let Some(variant) = runtime_variant_constructor(owner, facts, selected, symbols, analysis)? {
        return Ok(RuntimeResolvedCallTarget::Variant(variant));
    }
    if let arcweft_lang_sema::callable::CallableCandidateId::Presentation(presentation) =
        selected.id()
    {
        return Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: format!(
                "presentation callable {presentation:?} requires the pending typed Presentation command ABI"
            ),
        });
    }
    if selected.family() == CallableFamily::TraitMethod {
        return Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: "trait-call witness has no accepted runtime method-ID inventory".to_owned(),
        });
    }
    if matches!(
        selected.family(),
        CallableFamily::Lexical | CallableFamily::FunctionValue
    ) {
        return Ok(RuntimeResolvedCallTarget::FunctionValue);
    }
    if matches!(
        selected.id(),
        arcweft_lang_sema::callable::CallableCandidateId::Builtin(BuiltinCallableId::Reduction(
            ReductionConstructorKind::Unchanged
        ))
    ) {
        return Ok(RuntimeResolvedCallTarget::Reduction(
            RuntimeReductionConstructor::Unchanged,
        ));
    }
    if let Some(intrinsic) = runtime_intrinsic(selected.id()) {
        return Ok(RuntimeResolvedCallTarget::Intrinsic(intrinsic));
    }
    if let SignatureOrigin::Project { declaration, .. } = selected.origin() {
        let symbol =
            symbols
                .callable(declaration)
                .ok_or_else(|| RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "selected project callable is absent from the accepted symbol table"
                        .to_owned(),
                })?;
        let runtime = RuntimeProjectCallable::new(
            declaration.clone(),
            symbol.source_item(),
            runtime_selected_callable_id(owner, selected)?,
        );
        if declaration.owner()
            == arcweft_lang_hir::symbol::CallableDeclarationOwner::ExternCapability
        {
            let checked = analysis
                .checked_callables()
                .project_callable(declaration)
                .map_err(|error| RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: format!(
                        "extern capability call has no accepted checked callable facts: {error:?}"
                    ),
                })?;
            if !matches!(checked.execution(), CheckedCallableExecution::Runtime(_)) {
                return Err(RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "extern capability call was classified as a dispatch-only contract"
                        .to_owned(),
                });
            }
            let mode = if checked
                .exposed_row()
                .concrete()
                .iter()
                .any(EffectId::is_control_suspend)
            {
                RuntimeHostCallMode::Suspend
            } else {
                RuntimeHostCallMode::Immediate
            };
            return Ok(RuntimeResolvedCallTarget::Host {
                declaration: runtime,
                mode,
            });
        }
        return Ok(RuntimeResolvedCallTarget::Declaration(runtime));
    }
    let checked = selected
        .checked()
        .ok_or_else(|| RuntimeSemanticProjectionError::Call {
            owner,
            reason: format!(
                "language callable family {:?} has no typed runtime intrinsic",
                selected.family()
            ),
        })?;
    Ok(RuntimeResolvedCallTarget::Registered(
        arcweft_core::entry::RuntimeCallableId::from_checked_digest(
            checked.semantic_digest().into_bytes(),
        ),
    ))
}

#[expect(
    clippy::too_many_lines,
    reason = "constructor instantiations form a closed semantic validation matrix"
)]
fn runtime_variant_constructor(
    owner: ExprId,
    facts: &arcweft_lang_sema::callable::CallTargetFacts,
    selected: &ResolvedCallable,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<Option<RuntimeResolvedVariant>, RuntimeSemanticProjectionError> {
    let invalid = |reason: &str| RuntimeSemanticProjectionError::Call {
        owner,
        reason: reason.to_owned(),
    };
    match selected.instantiation() {
        CallableInstantiation::Result { kind, .. } => {
            let Some(TypeKind::Result { ok, error }) = facts.result() else {
                return Err(invalid(
                    "Result constructor did not retain its exact checked Result type",
                ));
            };
            let ok = runtime_type(ok, symbols)?;
            let error = runtime_type(error, symbols)?;
            Ok(Some(match kind {
                arcweft_lang_sema::callable::ResultConstructorKind::Ok => {
                    RuntimeResolvedVariant::result_ok(ok, error)
                }
                arcweft_lang_sema::callable::ResultConstructorKind::Err => {
                    RuntimeResolvedVariant::result_err(ok, error)
                }
            }))
        }
        CallableInstantiation::Option { .. } => {
            let Some(TypeKind::Option(item)) = facts.result() else {
                return Err(invalid(
                    "Option constructor did not retain its exact checked Option type",
                ));
            };
            if !matches!(
                selected.id(),
                arcweft_lang_sema::callable::CallableCandidateId::Option(
                    arcweft_lang_sema::callable::OptionConstructorKind::Some
                )
            ) {
                return Err(invalid(
                    "Option constructor instantiation has a non-Option candidate identity",
                ));
            }
            Ok(Some(RuntimeResolvedVariant::option_some(runtime_type(
                item, symbols,
            )?)))
        }
        CallableInstantiation::ExpectedEnum { expected } => {
            let TypeKind::ProjectNominal(nominal) = expected else {
                return Err(invalid(
                    "enum constructor expected type is not a project nominal enum",
                ));
            };
            if facts.result() != Some(expected) {
                return Err(invalid(
                    "enum constructor result differs from its checked project nominal type",
                ));
            }
            let arcweft_lang_sema::callable::CallableCandidateId::EnumVariant(candidate) =
                selected.id()
            else {
                return Err(invalid(
                    "enum constructor instantiation has a non-enum candidate identity",
                ));
            };
            let declaration = symbols.nominal(nominal.declaration()).ok_or_else(|| {
                RuntimeSemanticProjectionError::MissingNominal {
                    declaration: Box::new(nominal.declaration().clone()),
                }
            })?;
            if candidate.owner().package().as_str()
                != nominal.declaration().world().package().as_str()
                || candidate.owner().module() != nominal.declaration().module()
                || candidate.owner().name().as_str() != nominal.declaration().name().as_str()
            {
                return Err(invalid(
                    "enum constructor candidate does not own the checked project nominal type",
                ));
            }
            let arcweft_lang_hir::symbol::nominal::ProjectNominalBody::Enum { variants } =
                declaration.body()
            else {
                return Err(invalid(
                    "enum constructor checked type does not resolve to an enum declaration",
                ));
            };
            let (ordinal, variant) = variants
                .iter()
                .enumerate()
                .find(|(_, variant)| variant.name().as_str() == candidate.variant().as_str())
                .ok_or_else(|| {
                    invalid("enum constructor variant is absent from its declaration")
                })?;
            let ordinal = u32::try_from(ordinal)
                .map_err(|_| invalid("enum constructor variant ordinal exceeds u32"))?;
            let checked_nominal = CheckedProjectNominal::new(
                nominal.declaration().clone(),
                declaration.owner(),
                expected.semantic_identity_digest(),
                nominal.arguments().to_vec(),
            );
            Ok(Some(RuntimeResolvedVariant::project(
                runtime_nominal(&checked_nominal),
                ordinal,
                variant,
                runtime_project_variant_cases(declaration, &checked_nominal, symbols, analysis)?,
            )))
        }
        CallableInstantiation::None
        | CallableInstantiation::Character { .. }
        | CallableInstantiation::Receiver { .. }
        | CallableInstantiation::TypeReceiver { .. }
        | CallableInstantiation::Curried { .. }
        | CallableInstantiation::DataLast { .. } => Ok(None),
    }
}

fn runtime_project_variant_cases(
    declaration: &arcweft_lang_hir::symbol::nominal::ProjectNominalDeclaration,
    nominal: &CheckedProjectNominal,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<Box<[RuntimeCheckedVariantCase]>, RuntimeSemanticProjectionError> {
    let arcweft_lang_hir::symbol::nominal::ProjectNominalBody::Enum { variants } =
        declaration.body()
    else {
        return Err(RuntimeSemanticProjectionError::Type {
            reason: "checked project variant owner is not an enum".to_owned(),
        });
    };
    variants
        .iter()
        .map(|variant| {
            let payload = variant
                .payload()
                .map(|owner| {
                    let ty = analysis.ty(owner).ok_or_else(|| {
                        RuntimeSemanticProjectionError::Type {
                            reason: format!(
                                "project enum payload {owner:?} has no accepted semantic type"
                            ),
                        }
                    })?;
                    let ty = nominal
                        .instantiate_declaration_type(declaration, ty)
                        .ok_or_else(|| RuntimeSemanticProjectionError::Type {
                            reason: format!(
                                "project enum payload {owner:?} cannot apply its checked nominal arguments"
                            ),
                        })?;
                    runtime_type(&ty, symbols)?
                        .checked_type()
                        .map(Box::new)
                        .map_err(|reason| RuntimeSemanticProjectionError::Type { reason })
                })
                .transpose()?;
            Ok(RuntimeCheckedVariantCase {
                name: variant.name().as_str().to_owned(),
                payload,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn runtime_selected_callable_id(
    owner: ExprId,
    selected: &ResolvedCallable,
) -> Result<arcweft_core::entry::RuntimeCallableId, RuntimeSemanticProjectionError> {
    let checked = selected
        .checked()
        .ok_or_else(|| RuntimeSemanticProjectionError::Call {
            owner,
            reason: "selected record-backed callable has no checked semantic identity".to_owned(),
        })?;
    Ok(arcweft_core::entry::RuntimeCallableId::from_checked_digest(
        checked.semantic_digest().into_bytes(),
    ))
}

#[expect(
    clippy::too_many_lines,
    reason = "the builtin-to-runtime intrinsic table is intentionally exhaustive and declarative"
)]
fn runtime_intrinsic(
    candidate: &arcweft_lang_sema::callable::CallableCandidateId,
) -> Option<RuntimeIntrinsic> {
    let builtin = match candidate {
        arcweft_lang_sema::callable::CallableCandidateId::Builtin(builtin) => builtin,
        arcweft_lang_sema::callable::CallableCandidateId::Curried(curried) => {
            return runtime_intrinsic(curried.base());
        }
        arcweft_lang_sema::callable::CallableCandidateId::DataLast(data_last) => {
            return runtime_intrinsic(data_last.callable());
        }
        _ => return None,
    };
    Some(match builtin {
        BuiltinCallableId::Math(MathCallableId::MatMulF32) => RuntimeIntrinsic::MathMatmulF32,
        BuiltinCallableId::Math(MathCallableId::MatrixAddF32) => RuntimeIntrinsic::MathMatrixAddF32,
        BuiltinCallableId::Math(MathCallableId::TensorAddF32) => RuntimeIntrinsic::MathTensorAddF32,
        BuiltinCallableId::Math(MathCallableId::MatMulF64) => RuntimeIntrinsic::MathMatmulF64,
        BuiltinCallableId::Math(MathCallableId::MatrixAddF64) => RuntimeIntrinsic::MathMatrixAddF64,
        BuiltinCallableId::Math(MathCallableId::TensorAddF64) => RuntimeIntrinsic::MathTensorAddF64,
        BuiltinCallableId::StdFloat(float) => match (float.width(), float.operation()) {
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Abs) => {
                RuntimeIntrinsic::StdF32Abs
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Floor) => {
                RuntimeIntrinsic::StdF32Floor
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Ceil) => {
                RuntimeIntrinsic::StdF32Ceil
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Round) => {
                RuntimeIntrinsic::StdF32Round
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Trunc) => {
                RuntimeIntrinsic::StdF32Trunc
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Fract) => {
                RuntimeIntrinsic::StdF32Fract
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Sqrt) => {
                RuntimeIntrinsic::StdF32Sqrt
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Sin) => {
                RuntimeIntrinsic::StdF32Sin
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Cos) => {
                RuntimeIntrinsic::StdF32Cos
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Tan) => {
                RuntimeIntrinsic::StdF32Tan
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Exp) => {
                RuntimeIntrinsic::StdF32Exp
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Exp2) => {
                RuntimeIntrinsic::StdF32Exp2
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Ln) => {
                RuntimeIntrinsic::StdF32Ln
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Log2) => {
                RuntimeIntrinsic::StdF32Log2
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Log10) => {
                RuntimeIntrinsic::StdF32Log10
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Powf) => {
                RuntimeIntrinsic::StdF32Powf
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Atan2) => {
                RuntimeIntrinsic::StdF32Atan2
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::MulAdd) => {
                RuntimeIntrinsic::StdF32MulAdd
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::IsNan) => {
                RuntimeIntrinsic::StdF32IsNan
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::IsInfinite) => {
                RuntimeIntrinsic::StdF32IsInfinite
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::IsFinite) => {
                RuntimeIntrinsic::StdF32IsFinite
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::IsSignPositive) => {
                RuntimeIntrinsic::StdF32IsSignPositive
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::IsSignNegative) => {
                RuntimeIntrinsic::StdF32IsSignNegative
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::ToBits) => {
                RuntimeIntrinsic::StdF32ToBits
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::FromBits) => {
                RuntimeIntrinsic::StdF32FromBits
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::ToF64) => {
                RuntimeIntrinsic::StdF32ToF64
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Abs) => {
                RuntimeIntrinsic::StdF64Abs
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Floor) => {
                RuntimeIntrinsic::StdF64Floor
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Ceil) => {
                RuntimeIntrinsic::StdF64Ceil
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Round) => {
                RuntimeIntrinsic::StdF64Round
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Trunc) => {
                RuntimeIntrinsic::StdF64Trunc
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Fract) => {
                RuntimeIntrinsic::StdF64Fract
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Sqrt) => {
                RuntimeIntrinsic::StdF64Sqrt
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Sin) => {
                RuntimeIntrinsic::StdF64Sin
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Cos) => {
                RuntimeIntrinsic::StdF64Cos
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Tan) => {
                RuntimeIntrinsic::StdF64Tan
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Exp) => {
                RuntimeIntrinsic::StdF64Exp
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Exp2) => {
                RuntimeIntrinsic::StdF64Exp2
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Ln) => {
                RuntimeIntrinsic::StdF64Ln
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Log2) => {
                RuntimeIntrinsic::StdF64Log2
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Log10) => {
                RuntimeIntrinsic::StdF64Log10
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Powf) => {
                RuntimeIntrinsic::StdF64Powf
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Atan2) => {
                RuntimeIntrinsic::StdF64Atan2
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::MulAdd) => {
                RuntimeIntrinsic::StdF64MulAdd
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::IsNan) => {
                RuntimeIntrinsic::StdF64IsNan
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::IsInfinite) => {
                RuntimeIntrinsic::StdF64IsInfinite
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::IsFinite) => {
                RuntimeIntrinsic::StdF64IsFinite
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::IsSignPositive) => {
                RuntimeIntrinsic::StdF64IsSignPositive
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::IsSignNegative) => {
                RuntimeIntrinsic::StdF64IsSignNegative
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::ToBits) => {
                RuntimeIntrinsic::StdF64ToBits
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::FromBits) => {
                RuntimeIntrinsic::StdF64FromBits
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::ToF32) => {
                RuntimeIntrinsic::StdF64ToF32
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::ToF32)
            | (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::ToF64) => {
                return None;
            }
        },
        BuiltinCallableId::InlineFailureFallback
        | BuiltinCallableId::Panic
        | BuiltinCallableId::Fail
        | BuiltinCallableId::Bail
        | BuiltinCallableId::Ensure
        | BuiltinCallableId::Rgb
        | BuiltinCallableId::Sin
        | BuiltinCallableId::Cos
        | BuiltinCallableId::Vector { .. }
        | BuiltinCallableId::Capability(_)
        | BuiltinCallableId::Reduction(_) => return None,
    })
}
