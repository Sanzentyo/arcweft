//! Transactional evaluation of typed Fx graphs.

use super::{
    FxApplication, FxApplicationError, FxCapabilitySet, FxDiagnostic, FxDiagnosticCode,
    FxDiagnosticContext, FxEvaluationBinding, FxEvaluationBudget, FxGraph, FxNamedValue, FxNode,
    FxNodeKind, FxPhase, FxResolvedValue, FxRuntimeValue, FxSampleContext, FxSampleGeometry,
    FxStaticValue, FxTarget, ResolvedFxOperation, ResolvedFxPlan, ResolvedTransformOperation,
    ResolvedValueOperation, Transform2DError, ValueProgramInputs,
};

/// Single renderer-independent evaluator for View and `RichText` applications.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FxGraphEvaluator;

/// Renderer-supplied context for one logical Fx target sample.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FxTargetSample {
    ordinal: u32,
    geometry: FxSampleGeometry,
    reduce_motion: bool,
    interactive: bool,
}

impl FxTargetSample {
    pub const fn new(ordinal: u32) -> Self {
        Self {
            ordinal,
            geometry: FxSampleGeometry::new(
                super::Length::ZERO,
                super::Length::ZERO,
                super::Length::ZERO,
                super::Length::ZERO,
            ),
            reduce_motion: false,
            interactive: false,
        }
    }

    pub const fn with_geometry(mut self, geometry: FxSampleGeometry) -> Self {
        self.geometry = geometry;
        self
    }

    pub const fn with_reduce_motion(mut self, reduce_motion: bool) -> Self {
        self.reduce_motion = reduce_motion;
        self
    }

    pub const fn with_interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }
}

impl FxGraphEvaluator {
    /// Evaluates one application atomically with a caller-owned per-frame budget.
    ///
    /// `ordinal` is the target-local logical node/glyph ordinal. `interactive`
    /// controls whether node/content transforms must also provide invertible
    /// interaction geometry.
    pub fn evaluate(
        application: &FxApplication,
        binding: FxEvaluationBinding<'_>,
        ordinal: u32,
        reduce_motion: bool,
        interactive: bool,
        capabilities: &FxCapabilitySet,
        budget: &mut FxEvaluationBudget,
    ) -> ResolvedFxPlan {
        Self::evaluate_at(
            application,
            binding,
            FxTargetSample::new(ordinal)
                .with_reduce_motion(reduce_motion)
                .with_interactive(interactive),
            capabilities,
            budget,
        )
    }

    /// Evaluates one application with renderer-owned target geometry.
    pub fn evaluate_at(
        application: &FxApplication,
        binding: FxEvaluationBinding<'_>,
        sample: FxTargetSample,
        capabilities: &FxCapabilitySet,
        budget: &mut FxEvaluationBudget,
    ) -> ResolvedFxPlan {
        let context = FxDiagnosticContext {
            definition: Some(application.definition().clone()),
            instance: Some(binding.instance.instance),
            child_path: binding.instance.child_path.clone(),
            source_range: application.source_range(),
            ..FxDiagnosticContext::default()
        };
        if let Err(error) = application.validate_for_definition(binding.definition) {
            return ResolvedFxPlan::from_diagnostic(application_diagnostic(&context, &error));
        }
        if let Err(error) = binding.instance.validate_for_definition(binding.definition) {
            let code = match error {
                super::FxInstanceSnapshotError::AbiMismatch { .. } => FxDiagnosticCode::AbiMismatch,
                super::FxInstanceSnapshotError::ParameterCount { .. }
                | super::FxInstanceSnapshotError::ParameterType { .. } => {
                    FxDiagnosticCode::UnitMismatch
                }
                _ => FxDiagnosticCode::ProgramValidation,
            };
            return ResolvedFxPlan::from_diagnostic(FxDiagnostic::error(
                code,
                context,
                error.to_string(),
            ));
        }
        if application.parameters() != binding.instance.parameters {
            return ResolvedFxPlan::from_diagnostic(FxDiagnostic::error(
                FxDiagnosticCode::ProgramValidation,
                context,
                "live Fx parameter snapshot is stale for the authored application",
            ));
        }
        let sample_context = match FxSampleContext::from_logical_times(
            binding.runtime_time,
            binding.instance.activation_logical_time,
            sample.ordinal,
            binding.instance.deterministic_seed,
            sample.reduce_motion,
        ) {
            Ok(context) => context.with_geometry(sample.geometry),
            Err(error) => {
                return ResolvedFxPlan::from_diagnostic(FxDiagnostic::error(
                    FxDiagnosticCode::NumericNonFinite,
                    context,
                    error.to_string(),
                ));
            }
        };
        let mut operations = Vec::new();
        let mut visit = 0_usize;
        if let Err(diagnostic) = evaluate_graph(
            binding.definition.graph(),
            binding.instance,
            sample_context,
            sample.interactive,
            &context,
            budget,
            &mut visit,
            &mut operations,
        ) {
            return ResolvedFxPlan::from_diagnostic(*diagnostic);
        }
        ResolvedFxPlan::resolve_application(&context, capabilities, operations)
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_graph(
    graph: &FxGraph,
    instance: &super::FxInstanceSnapshot,
    sample_context: FxSampleContext,
    interactive: bool,
    context: &FxDiagnosticContext,
    budget: &mut FxEvaluationBudget,
    visit: &mut usize,
    operations: &mut Vec<ResolvedFxOperation>,
) -> Result<(), Box<FxDiagnostic>> {
    for (node_ordinal, node) in graph.nodes().iter().enumerate() {
        budget
            .charge(*visit)
            .map_err(|error| FxDiagnostic::from_evaluation(context.clone(), &error))?;
        *visit = visit.saturating_add(1);
        match node {
            FxNode::Conditional {
                condition,
                then_graph,
                else_graph,
            } => {
                let condition =
                    resolve_value(condition, instance, sample_context, context, budget)?;
                let FxResolvedValue::Runtime(FxRuntimeValue::Bool(condition)) = condition else {
                    return Err(Box::new(FxDiagnostic::error(
                        FxDiagnosticCode::UnitMismatch,
                        context.clone(),
                        "Fx conditional did not resolve to Bool",
                    )));
                };
                let branch_ordinal = usize::from(!condition);
                let branch = if condition { then_graph } else { else_graph };
                let child_context = child_context(context, node_ordinal, branch_ordinal)?;
                evaluate_graph(
                    branch,
                    instance,
                    sample_context,
                    interactive,
                    &child_context,
                    budget,
                    visit,
                    operations,
                )?;
            }
            FxNode::Stack { children } => {
                for (child_ordinal, child) in children.iter().enumerate() {
                    let child_context = child_context(context, node_ordinal, child_ordinal)?;
                    evaluate_graph(
                        child,
                        instance,
                        sample_context,
                        interactive,
                        &child_context,
                        budget,
                        visit,
                        operations,
                    )?;
                }
            }
            _ => operations.push(evaluate_leaf(
                node,
                instance,
                sample_context,
                interactive,
                context,
                budget,
            )?),
        }
    }
    Ok(())
}

fn evaluate_leaf(
    node: &FxNode,
    instance: &super::FxInstanceSnapshot,
    sample_context: FxSampleContext,
    interactive: bool,
    context: &FxDiagnosticContext,
    budget: &mut FxEvaluationBudget,
) -> Result<ResolvedFxOperation, Box<FxDiagnostic>> {
    let kind = node.node_kind();
    let properties = node.properties().ok_or_else(|| {
        FxDiagnostic::error(
            FxDiagnosticCode::ProgramValidation,
            context.clone(),
            "non-leaf Fx node reached leaf evaluation",
        )
    })?;
    let phase = properties
        .iter()
        .find(|property| property.name() == "phase")
        .map(|property| match property.value() {
            FxStaticValue::Phase(phase) => Ok(*phase),
            _ => Err(Box::new(FxDiagnostic::error(
                FxDiagnosticCode::ProgramValidation,
                context.clone(),
                "Fx phase property is not a closed phase",
            ))),
        })
        .transpose()?
        .unwrap_or_else(|| default_phase(kind, properties));
    let target = properties
        .iter()
        .find(|property| property.name() == "target")
        .map(|property| match property.value() {
            FxStaticValue::Target(target) => Ok(*target),
            _ => Err(Box::new(FxDiagnostic::error(
                FxDiagnosticCode::ProgramValidation,
                context.clone(),
                "Fx target property is not a closed target",
            ))),
        })
        .transpose()?
        .unwrap_or_else(|| default_target(kind, phase));
    let values = properties
        .iter()
        .filter(|property| !matches!(property.name(), "target" | "phase"))
        .map(|property| {
            resolve_value(property.value(), instance, sample_context, context, budget)
                .map(|value| FxNamedValue::new(property.name(), value))
        })
        .collect::<Result<Vec<_>, _>>()?;

    if kind == FxNodeKind::Transform {
        let mut transforms = values.iter().filter_map(|value| match value {
            FxNamedValue {
                name,
                value: FxResolvedValue::Runtime(FxRuntimeValue::Transform2D(transform)),
            } if matches!(name.as_str(), "transform" | "sampler") => Some(*transform),
            _ => None,
        });
        let transform = transforms.next().ok_or_else(|| {
            FxDiagnostic::error(
                FxDiagnosticCode::ProgramValidation,
                context.clone(),
                "Fx.transform requires one typed transform or sampler result",
            )
        })?;
        if transforms.next().is_some() {
            return Err(Box::new(FxDiagnostic::error(
                FxDiagnosticCode::ProgramValidation,
                context.clone(),
                "Fx.transform cannot provide both `transform` and `sampler`",
            )));
        }
        let transform = transform.resolve().map_err(|error| {
            let code = match error {
                Transform2DError::InvalidOpacity { .. } => FxDiagnosticCode::InvalidOpacity,
                Transform2DError::NonFiniteResult { .. } => FxDiagnosticCode::NumericNonFinite,
            };
            FxDiagnostic::error(code, context.clone(), error.to_string())
        })?;
        return Ok(ResolvedFxOperation::Transform(
            ResolvedTransformOperation::new(phase, target, transform, interactive),
        ));
    }

    let interface = node.renderer_interface().ok_or_else(|| {
        FxDiagnostic::error(
            FxDiagnosticCode::ProgramValidation,
            context.clone(),
            "Fx graph node has no renderer interface",
        )
    })?;
    Ok(ResolvedFxOperation::Values(ResolvedValueOperation::new(
        interface, phase, target, values,
    )))
}

fn resolve_value(
    value: &FxStaticValue,
    instance: &super::FxInstanceSnapshot,
    sample_context: FxSampleContext,
    context: &FxDiagnosticContext,
    budget: &mut FxEvaluationBudget,
) -> Result<FxResolvedValue, Box<FxDiagnostic>> {
    Ok(match value {
        FxStaticValue::Runtime(value) => FxResolvedValue::Runtime(*value),
        FxStaticValue::Resource(value) => FxResolvedValue::Resource(value.clone()),
        FxStaticValue::Selector(value) => FxResolvedValue::Selector(value.clone()),
        FxStaticValue::String(value) => FxResolvedValue::String(value.clone()),
        FxStaticValue::Parameter(slot) => {
            let value = instance
                .parameters
                .get(usize::from(slot.index))
                .copied()
                .ok_or_else(|| {
                    FxDiagnostic::error(
                        FxDiagnosticCode::ProgramValidation,
                        context.clone(),
                        format!("Fx parameter slot {} is out of bounds", slot.index),
                    )
                })?;
            if value.value_type() != slot.ty {
                return Err(Box::new(FxDiagnostic::error(
                    FxDiagnosticCode::UnitMismatch,
                    context.clone(),
                    format!(
                        "Fx parameter slot {} has type {:?}, expected {:?}",
                        slot.index,
                        value.value_type(),
                        slot.ty
                    ),
                )));
            }
            FxResolvedValue::Runtime(value)
        }
        FxStaticValue::Sampler(program) => FxResolvedValue::Runtime(
            program
                .evaluate(
                    ValueProgramInputs {
                        parameters: &instance.parameters,
                        state: &[],
                    },
                    sample_context,
                    budget,
                )
                .map_err(|error| FxDiagnostic::from_evaluation(context.clone(), &error))?,
        ),
        FxStaticValue::List(values) => FxResolvedValue::List(
            values
                .iter()
                .map(|value| resolve_value(value, instance, sample_context, context, budget))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        FxStaticValue::Record(properties) => FxResolvedValue::Record(
            properties
                .iter()
                .map(|property| {
                    resolve_value(property.value(), instance, sample_context, context, budget)
                        .map(|value| FxNamedValue::new(property.name(), value))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
        FxStaticValue::Target(_) | FxStaticValue::Phase(_) => {
            return Err(Box::new(FxDiagnostic::error(
                FxDiagnosticCode::ProgramValidation,
                context.clone(),
                "Fx target/phase value appeared outside its named property",
            )));
        }
    })
}

fn default_phase(kind: FxNodeKind, properties: &[super::FxProperty]) -> FxPhase {
    match kind {
        FxNodeKind::Style | FxNodeKind::Text | FxNodeKind::Conditional | FxNodeKind::Stack => {
            FxPhase::BeforeLayout
        }
        FxNodeKind::Color => FxPhase::GlyphColor,
        FxNodeKind::Transform => FxPhase::GlyphTransform,
        FxNodeKind::Mask => FxPhase::GlyphMask,
        FxNodeKind::Filter | FxNodeKind::OffscreenPass => FxPhase::OffscreenPass,
        FxNodeKind::Shader => properties
            .iter()
            .find(|property| property.name() == "stage")
            .and_then(|property| match property.value() {
                FxStaticValue::Selector(stage) => phase_from_selector(stage),
                _ => None,
            })
            .unwrap_or(FxPhase::GlyphColor),
        FxNodeKind::PostProcess => FxPhase::PostProcess,
        FxNodeKind::Transition => FxPhase::Transition,
    }
}

fn default_target(kind: FxNodeKind, phase: FxPhase) -> FxTarget {
    if matches!(phase, FxPhase::PostProcess | FxPhase::Transition)
        || matches!(kind, FxNodeKind::PostProcess | FxNodeKind::Transition)
    {
        FxTarget::Viewport
    } else {
        FxTarget::Content
    }
}

fn phase_from_selector(value: &str) -> Option<FxPhase> {
    Some(match value {
        "before_layout" => FxPhase::BeforeLayout,
        "layout_transform" => FxPhase::LayoutTransform,
        "glyph_transform" => FxPhase::GlyphTransform,
        "glyph_color" => FxPhase::GlyphColor,
        "glyph_mask" => FxPhase::GlyphMask,
        "offscreen_pass" | "run_offscreen_pass" => FxPhase::OffscreenPass,
        "post_process" => FxPhase::PostProcess,
        "transition" => FxPhase::Transition,
        _ => return None,
    })
}

fn child_context(
    context: &FxDiagnosticContext,
    node_ordinal: usize,
    child_ordinal: usize,
) -> Result<FxDiagnosticContext, Box<FxDiagnostic>> {
    let node_ordinal = u32::try_from(node_ordinal).map_err(|_| {
        FxDiagnostic::error(
            FxDiagnosticCode::ProgramValidation,
            context.clone(),
            "Fx graph node ordinal exceeds u32",
        )
    })?;
    let child_ordinal = u32::try_from(child_ordinal).map_err(|_| {
        FxDiagnostic::error(
            FxDiagnosticCode::ProgramValidation,
            context.clone(),
            "Fx graph child ordinal exceeds u32",
        )
    })?;
    let path = context
        .child_path
        .try_with_child(node_ordinal)
        .and_then(|path| path.try_with_child(child_ordinal))
        .map_err(|error| {
            FxDiagnostic::error(
                FxDiagnosticCode::ProgramValidation,
                context.clone(),
                error.to_string(),
            )
        })?;
    Ok(FxDiagnosticContext {
        child_path: path,
        ..context.clone()
    })
}

fn application_diagnostic(
    context: &FxDiagnosticContext,
    error: &FxApplicationError,
) -> FxDiagnostic {
    let code = match error {
        FxApplicationError::ParameterCount { .. } | FxApplicationError::ParameterType { .. } => {
            FxDiagnosticCode::UnitMismatch
        }
        FxApplicationError::TooManyParameters { .. }
        | FxApplicationError::DefinitionMismatch { .. } => FxDiagnosticCode::ProgramValidation,
    };
    FxDiagnostic::error(code, context.clone(), error.to_string())
}
