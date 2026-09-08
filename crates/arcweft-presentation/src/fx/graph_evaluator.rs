//! Transactional evaluation of typed Fx graphs.

use super::{
    FxApplication, FxApplicationError, FxCapabilitySet, FxDiagnostic, FxDiagnosticCode,
    FxDiagnosticContext, FxEvaluationBinding, FxEvaluationBudget, FxGraph, FxNode, FxNodeKind,
    FxPhase, FxResolvedValue, FxRuntimeValue, FxSampleContext, FxSampleGeometry, FxSelectorDomain,
    FxShaderStage, FxShaderUniform, FxStaticValue, FxTarget, ResolvedColorOperation,
    ResolvedFilterOperation, ResolvedFxOperation, ResolvedFxPlan, ResolvedMaskOperation,
    ResolvedOffscreenPassOperation, ResolvedPostProcessOperation, ResolvedShaderUniformOperation,
    ResolvedTextStyleOperation, ResolvedTransformOperation, ResolvedTransitionOperation,
    Transform2DError, ValueProgramInputs,
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

    #[must_use]
    pub const fn with_geometry(mut self, geometry: FxSampleGeometry) -> Self {
        self.geometry = geometry;
        self
    }

    #[must_use]
    pub const fn with_reduce_motion(mut self, reduce_motion: bool) -> Self {
        self.reduce_motion = reduce_motion;
        self
    }

    #[must_use]
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
            instance: Some(binding.instance.instance()),
            child_path: binding.instance.child_path().clone(),
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
        let sample_context = match FxSampleContext::from_logical_times(
            binding.runtime_time,
            binding.instance.activation_logical_time(),
            sample.ordinal,
            binding.instance.deterministic_seed(),
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
            binding.definition,
            application.template(),
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
    definition: &super::FxDefinition,
    template: &super::FxBoundApplicationTemplate,
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
                let condition = resolve_value(
                    condition,
                    definition,
                    template,
                    instance,
                    sample_context,
                    context,
                    budget,
                )?;
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
                    definition,
                    template,
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
                        definition,
                        template,
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
                interactive,
                LeafEvaluationContext {
                    definition,
                    template,
                    instance,
                    sample_context,
                    context,
                    budget,
                },
            )?),
        }
    }
    Ok(())
}

struct LeafEvaluationContext<'a> {
    definition: &'a super::FxDefinition,
    template: &'a super::FxBoundApplicationTemplate,
    instance: &'a super::FxInstanceSnapshot,
    sample_context: FxSampleContext,
    context: &'a FxDiagnosticContext,
    budget: &'a mut FxEvaluationBudget,
}

fn evaluate_leaf(
    node: &FxNode,
    interactive: bool,
    mut input: LeafEvaluationContext<'_>,
) -> Result<ResolvedFxOperation, Box<FxDiagnostic>> {
    let (kind, phase, target, values) = resolve_leaf_properties(node, &mut input)?;
    match kind {
        FxNodeKind::Style | FxNodeKind::Text => Ok(ResolvedFxOperation::TextStyle(
            text_style_operation(phase, target, values, input.context)?,
        )),
        FxNodeKind::Color => Ok(ResolvedFxOperation::Color(color_operation(
            phase,
            target,
            values,
            input.context,
        )?)),
        FxNodeKind::Transform => {
            let mut transform = None;
            for (name, value) in values {
                match (name.as_str(), value) {
                    (
                        "transform" | "sampler",
                        FxResolvedValue::Runtime(FxRuntimeValue::Transform2D(value)),
                    ) => {
                        if transform.replace(value).is_some() {
                            return Err(duplicate_property(input.context, name));
                        }
                    }
                    (name, _) => return Err(invalid_property(input.context, name)),
                }
            }
            let transform = transform.ok_or_else(|| {
                FxDiagnostic::error(
                    FxDiagnosticCode::ProgramValidation,
                    input.context.clone(),
                    "Fx.transform requires one typed transform or sampler result",
                )
            })?;
            let transform = transform.resolve().map_err(|error| {
                let code = match error {
                    Transform2DError::InvalidOpacity { .. } => FxDiagnosticCode::InvalidOpacity,
                    Transform2DError::NonFiniteResult { .. } => FxDiagnosticCode::NumericNonFinite,
                };
                FxDiagnostic::error(code, input.context.clone(), error.to_string())
            })?;
            Ok(ResolvedFxOperation::Transform(
                ResolvedTransformOperation::new(phase, target, transform, interactive),
            ))
        }
        FxNodeKind::Mask => Ok(ResolvedFxOperation::Mask(mask_operation(
            phase,
            target,
            values,
            input.context,
        )?)),
        FxNodeKind::Filter => Ok(ResolvedFxOperation::Filter(filter_operation(
            phase,
            target,
            values,
            input.context,
        )?)),
        FxNodeKind::Shader => Ok(ResolvedFxOperation::ShaderUniform(
            shader_uniform_operation(phase, target, values, input.context)?,
        )),
        FxNodeKind::OffscreenPass => Ok(ResolvedFxOperation::OffscreenPass(
            offscreen_pass_operation(phase, target, values, input.context)?,
        )),
        FxNodeKind::PostProcess => Ok(ResolvedFxOperation::PostProcess(post_process_operation(
            phase,
            target,
            values,
            input.context,
        )?)),
        FxNodeKind::Transition => Ok(ResolvedFxOperation::Transition(transition_operation(
            phase,
            target,
            values,
            input.context,
        )?)),
        FxNodeKind::Conditional | FxNodeKind::Stack => Err(Box::new(FxDiagnostic::error(
            FxDiagnosticCode::ProgramValidation,
            input.context.clone(),
            "non-leaf Fx node reached leaf evaluation",
        ))),
    }
}

fn resolve_leaf_properties(
    node: &FxNode,
    input: &mut LeafEvaluationContext<'_>,
) -> Result<(FxNodeKind, FxPhase, FxTarget, Vec<ResolvedProperty>), Box<FxDiagnostic>> {
    let kind = node.node_kind();
    let properties = node.properties().ok_or_else(|| {
        FxDiagnostic::error(
            FxDiagnosticCode::ProgramValidation,
            input.context.clone(),
            "non-leaf Fx node reached leaf evaluation",
        )
    })?;
    let phase = match properties
        .iter()
        .find(|property| property.id() == super::FxPropertyId::Phase)
    {
        Some(property) => match property.value() {
            FxStaticValue::Phase(phase) => *phase,
            _ => {
                return Err(Box::new(FxDiagnostic::error(
                    FxDiagnosticCode::ProgramValidation,
                    input.context.clone(),
                    "Fx phase property is not a closed phase",
                )));
            }
        },
        None => default_phase(kind, properties, input.context)?,
    };
    let target = properties
        .iter()
        .find(|property| property.id() == super::FxPropertyId::Target)
        .map(|property| match property.value() {
            FxStaticValue::Target(target) => Ok(*target),
            _ => Err(Box::new(FxDiagnostic::error(
                FxDiagnosticCode::ProgramValidation,
                input.context.clone(),
                "Fx target property is not a closed target",
            ))),
        })
        .transpose()?
        .unwrap_or_else(|| default_target(kind, phase));
    let values = properties
        .iter()
        .filter(|property| {
            !matches!(
                property.id(),
                super::FxPropertyId::Target | super::FxPropertyId::Phase
            )
        })
        .map(|property| {
            resolve_value(
                property.value(),
                input.definition,
                input.template,
                input.instance,
                input.sample_context,
                input.context,
                input.budget,
            )
            .map(|value| (property.name().to_owned(), value))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((kind, phase, target, values))
}

type ResolvedProperty = (String, FxResolvedValue);

fn text_style_operation(
    phase: FxPhase,
    target: FxTarget,
    values: Vec<ResolvedProperty>,
    context: &FxDiagnosticContext,
) -> Result<ResolvedTextStyleOperation, Box<FxDiagnostic>> {
    let mut operation = ResolvedTextStyleOperation {
        phase,
        target,
        opacity: None,
        weight: None,
        slant: None,
        font_family: None,
        size: None,
        spacing: None,
        color: None,
    };
    for (name, value) in values {
        match (name.as_str(), value) {
            ("opacity", FxResolvedValue::Runtime(FxRuntimeValue::F32(value))) => {
                set_property(&mut operation.opacity, value, &name, context)?;
            }
            ("weight", FxResolvedValue::Runtime(FxRuntimeValue::I32(value))) => {
                set_property(&mut operation.weight, value, &name, context)?;
            }
            ("slant", FxResolvedValue::Runtime(FxRuntimeValue::Angle(value))) => {
                set_property(&mut operation.slant, value, &name, context)?;
            }
            ("font_family", FxResolvedValue::FontFamily(value)) => {
                set_property(&mut operation.font_family, value, &name, context)?;
            }
            ("size", FxResolvedValue::Runtime(FxRuntimeValue::Length(value))) => {
                set_property(&mut operation.size, value, &name, context)?;
            }
            ("spacing", FxResolvedValue::Runtime(FxRuntimeValue::Length(value))) => {
                set_property(&mut operation.spacing, value, &name, context)?;
            }
            ("color", FxResolvedValue::Runtime(FxRuntimeValue::Color(value))) => {
                set_property(&mut operation.color, value, &name, context)?;
            }
            (name, _) => return Err(invalid_property(context, name)),
        }
    }
    Ok(operation)
}

fn color_operation(
    phase: FxPhase,
    target: FxTarget,
    values: Vec<ResolvedProperty>,
    context: &FxDiagnosticContext,
) -> Result<ResolvedColorOperation, Box<FxDiagnostic>> {
    let mut operation = ResolvedColorOperation {
        phase,
        target,
        tint: None,
        multiply: None,
        opacity: None,
    };
    for (name, value) in values {
        match (name.as_str(), value) {
            ("tint", FxResolvedValue::Runtime(FxRuntimeValue::Color(value))) => {
                set_property(&mut operation.tint, value, &name, context)?;
            }
            ("multiply", FxResolvedValue::Runtime(FxRuntimeValue::Color(value))) => {
                set_property(&mut operation.multiply, value, &name, context)?;
            }
            ("opacity", FxResolvedValue::Runtime(FxRuntimeValue::F32(value))) => {
                set_property(&mut operation.opacity, value, &name, context)?;
            }
            (name, _) => return Err(invalid_property(context, name)),
        }
    }
    Ok(operation)
}

fn mask_operation(
    phase: FxPhase,
    target: FxTarget,
    values: Vec<ResolvedProperty>,
    context: &FxDiagnosticContext,
) -> Result<ResolvedMaskOperation, Box<FxDiagnostic>> {
    let mut operation = ResolvedMaskOperation {
        phase,
        target,
        resource: None,
        coverage: None,
        invert: None,
    };
    for (name, value) in values {
        match (name.as_str(), value) {
            ("resource", FxResolvedValue::Resource(value)) => {
                set_property(&mut operation.resource, value, &name, context)?;
            }
            ("coverage", FxResolvedValue::Runtime(FxRuntimeValue::F32(value))) => {
                set_property(&mut operation.coverage, value, &name, context)?;
            }
            ("invert", FxResolvedValue::Runtime(FxRuntimeValue::Bool(value))) => {
                set_property(&mut operation.invert, value, &name, context)?;
            }
            (name, _) => return Err(invalid_property(context, name)),
        }
    }
    Ok(operation)
}

fn filter_operation(
    phase: FxPhase,
    target: FxTarget,
    values: Vec<ResolvedProperty>,
    context: &FxDiagnosticContext,
) -> Result<ResolvedFilterOperation, Box<FxDiagnostic>> {
    let mut operation = ResolvedFilterOperation {
        phase,
        target,
        blur_radius: None,
        brightness: None,
        contrast: None,
        saturation: None,
    };
    for (name, value) in values {
        match (name.as_str(), value) {
            ("blur_radius", FxResolvedValue::Runtime(FxRuntimeValue::Length(value))) => {
                set_property(&mut operation.blur_radius, value, &name, context)?;
            }
            ("brightness", FxResolvedValue::Runtime(FxRuntimeValue::F32(value))) => {
                set_property(&mut operation.brightness, value, &name, context)?;
            }
            ("contrast", FxResolvedValue::Runtime(FxRuntimeValue::F32(value))) => {
                set_property(&mut operation.contrast, value, &name, context)?;
            }
            ("saturation", FxResolvedValue::Runtime(FxRuntimeValue::F32(value))) => {
                set_property(&mut operation.saturation, value, &name, context)?;
            }
            (name, _) => return Err(invalid_property(context, name)),
        }
    }
    Ok(operation)
}

fn shader_uniform_operation(
    phase: FxPhase,
    target: FxTarget,
    values: Vec<ResolvedProperty>,
    context: &FxDiagnosticContext,
) -> Result<ResolvedShaderUniformOperation, Box<FxDiagnostic>> {
    let mut operation = ResolvedShaderUniformOperation {
        phase,
        target,
        resource: None,
        stage: None,
        uniforms: Vec::new(),
    };
    for (name, value) in values {
        match (name.as_str(), value) {
            ("resource", FxResolvedValue::Resource(value)) => {
                set_property(&mut operation.resource, value, &name, context)?;
            }
            ("stage", FxResolvedValue::ShaderStage(value)) => {
                let value = shader_stage_phase(value);
                set_property(&mut operation.stage, value, &name, context)?;
            }
            ("uniforms", FxResolvedValue::UniformRecord(value)) => {
                if !operation.uniforms.is_empty() {
                    return Err(duplicate_property(context, name));
                }
                operation.uniforms = value;
            }
            (name, _) => return Err(invalid_property(context, name)),
        }
    }
    Ok(operation)
}

fn offscreen_pass_operation(
    phase: FxPhase,
    target: FxTarget,
    values: Vec<ResolvedProperty>,
    context: &FxDiagnosticContext,
) -> Result<ResolvedOffscreenPassOperation, Box<FxDiagnostic>> {
    let mut operation = ResolvedOffscreenPassOperation {
        phase,
        target,
        resource: None,
    };
    for (name, value) in values {
        match (name.as_str(), value) {
            ("resource", FxResolvedValue::Resource(value)) => {
                set_property(&mut operation.resource, value, &name, context)?;
            }
            (name, _) => return Err(invalid_property(context, name)),
        }
    }
    Ok(operation)
}

fn post_process_operation(
    phase: FxPhase,
    target: FxTarget,
    values: Vec<ResolvedProperty>,
    context: &FxDiagnosticContext,
) -> Result<ResolvedPostProcessOperation, Box<FxDiagnostic>> {
    let mut operation = ResolvedPostProcessOperation {
        phase,
        target,
        resource: None,
    };
    for (name, value) in values {
        match (name.as_str(), value) {
            ("resource", FxResolvedValue::Resource(value)) => {
                set_property(&mut operation.resource, value, &name, context)?;
            }
            (name, _) => return Err(invalid_property(context, name)),
        }
    }
    Ok(operation)
}

fn transition_operation(
    phase: FxPhase,
    target: FxTarget,
    values: Vec<ResolvedProperty>,
    context: &FxDiagnosticContext,
) -> Result<ResolvedTransitionOperation, Box<FxDiagnostic>> {
    let mut operation = ResolvedTransitionOperation {
        phase,
        target,
        kind: None,
        easing: None,
        duration: None,
        progress: None,
    };
    for (name, value) in values {
        match (name.as_str(), value) {
            ("kind", FxResolvedValue::Selector(value))
                if value.domain() == FxSelectorDomain::TransitionKind =>
            {
                set_property(&mut operation.kind, value, &name, context)?;
            }
            ("easing", FxResolvedValue::Selector(value))
                if value.domain() == FxSelectorDomain::TransitionEasing =>
            {
                set_property(&mut operation.easing, value, &name, context)?;
            }
            ("duration", FxResolvedValue::Runtime(FxRuntimeValue::Seconds(value))) => {
                set_property(&mut operation.duration, value, &name, context)?;
            }
            ("progress", FxResolvedValue::Runtime(FxRuntimeValue::F32(value))) => {
                set_property(&mut operation.progress, value, &name, context)?;
            }
            (name, _) => return Err(invalid_property(context, name)),
        }
    }
    Ok(operation)
}

fn set_property<T>(
    slot: &mut Option<T>,
    value: T,
    name: &str,
    context: &FxDiagnosticContext,
) -> Result<(), Box<FxDiagnostic>> {
    if slot.replace(value).is_some() {
        Err(duplicate_property(context, name.to_owned()))
    } else {
        Ok(())
    }
}

fn invalid_property(context: &FxDiagnosticContext, name: &str) -> Box<FxDiagnostic> {
    Box::new(FxDiagnostic::error(
        FxDiagnosticCode::UnitMismatch,
        context.clone(),
        format!("Fx property `{name}` has an invalid closed value"),
    ))
}

fn duplicate_property(context: &FxDiagnosticContext, name: impl Into<String>) -> Box<FxDiagnostic> {
    Box::new(FxDiagnostic::error(
        FxDiagnosticCode::ProgramValidation,
        context.clone(),
        format!("Fx property `{}` occurs more than once", name.into()),
    ))
}

fn resolve_value(
    value: &FxStaticValue,
    definition: &super::FxDefinition,
    template: &super::FxBoundApplicationTemplate,
    instance: &super::FxInstanceSnapshot,
    sample_context: FxSampleContext,
    context: &FxDiagnosticContext,
    budget: &mut FxEvaluationBudget,
) -> Result<FxResolvedValue, Box<FxDiagnostic>> {
    Ok(match value {
        FxStaticValue::Runtime(value) => FxResolvedValue::Runtime(*value),
        FxStaticValue::Resource(value) => FxResolvedValue::Resource(value.clone()),
        FxStaticValue::Selector(value) => FxResolvedValue::Selector(value.clone()),
        FxStaticValue::ShaderStage(value) => FxResolvedValue::ShaderStage(*value),
        FxStaticValue::FontFamily(value) => FxResolvedValue::FontFamily(value.clone()),
        FxStaticValue::Parameter(parameter) => {
            let row = definition
                .parameter_layout()
                .abi_rows()
                .get(usize::from(parameter.index().get()))
                .filter(|row| row.parameter() == *parameter)
                .ok_or_else(|| invalid_property(context, "parameter"))?;
            match row.storage() {
                super::FxParameterStorageSlot::Runtime(slot) => FxResolvedValue::Runtime(
                    *instance
                        .parameters()
                        .get(usize::from(slot.get()))
                        .ok_or_else(|| invalid_property(context, "parameter"))?,
                ),
                super::FxParameterStorageSlot::Static(slot) => match template
                    .static_arguments()
                    .get(usize::from(slot.get()))
                    .ok_or_else(|| invalid_property(context, "parameter"))?
                {
                    super::FxStaticDefinitionArgumentValue::Resource(value) => {
                        FxResolvedValue::Resource(value.clone())
                    }
                    super::FxStaticDefinitionArgumentValue::UniformRecord(record) => {
                        resolve_uniform_record(record, instance, sample_context, context, budget)?
                    }
                },
            }
        }
        FxStaticValue::Sampler(program) => FxResolvedValue::Runtime(
            program
                .evaluate(
                    ValueProgramInputs {
                        parameters: instance.parameters(),
                        state: &[],
                    },
                    sample_context,
                    budget,
                )
                .map_err(|error| FxDiagnostic::from_evaluation(context.clone(), &error))?,
        ),
        FxStaticValue::UniformRecord(record) => {
            resolve_uniform_record(record, instance, sample_context, context, budget)?
        }
        FxStaticValue::Target(_) | FxStaticValue::Phase(_) => {
            return Err(Box::new(FxDiagnostic::error(
                FxDiagnosticCode::ProgramValidation,
                context.clone(),
                "Fx target/phase value appeared outside its named property",
            )));
        }
    })
}

fn resolve_uniform_record(
    record: &super::FxUniformRecord,
    instance: &super::FxInstanceSnapshot,
    sample_context: FxSampleContext,
    context: &FxDiagnosticContext,
    budget: &mut FxEvaluationBudget,
) -> Result<FxResolvedValue, Box<FxDiagnostic>> {
    let fields = record
        .fields()
        .iter()
        .map(|field| {
            let value = match field.value() {
                super::FxUniformValue::Constant(value) => value.value(),
                super::FxUniformValue::Parameter(parameter) => {
                    let reference = parameter.reference();
                    let value = *instance
                        .parameters()
                        .get(usize::from(reference.slot().get()))
                        .ok_or_else(|| invalid_property(context, field.name().as_str()))?;
                    if value.value_type() != reference.runtime_type() {
                        return Err(invalid_property(context, field.name().as_str()));
                    }
                    value
                }
                super::FxUniformValue::Program(program) => program
                    .sampler()
                    .evaluate(
                        ValueProgramInputs {
                            parameters: instance.parameters(),
                            state: &[],
                        },
                        sample_context,
                        budget,
                    )
                    .map_err(|error| FxDiagnostic::from_evaluation(context.clone(), &error))?,
            };
            Ok(FxShaderUniform::new(
                field.name().clone(),
                FxResolvedValue::Runtime(value),
            ))
        })
        .collect::<Result<Vec<_>, Box<FxDiagnostic>>>()?;
    Ok(FxResolvedValue::UniformRecord(fields))
}

fn default_phase(
    kind: FxNodeKind,
    properties: &[super::FxProperty],
    context: &FxDiagnosticContext,
) -> Result<FxPhase, Box<FxDiagnostic>> {
    Ok(match kind {
        FxNodeKind::Style | FxNodeKind::Text | FxNodeKind::Conditional | FxNodeKind::Stack => {
            FxPhase::BeforeLayout
        }
        FxNodeKind::Color => FxPhase::GlyphColor,
        FxNodeKind::Transform => FxPhase::GlyphTransform,
        FxNodeKind::Mask => FxPhase::GlyphMask,
        FxNodeKind::Filter | FxNodeKind::OffscreenPass => FxPhase::OffscreenPass,
        FxNodeKind::Shader => match properties
            .iter()
            .find(|property| property.id() == super::FxPropertyId::Stage)
        {
            Some(property) => match property.value() {
                FxStaticValue::ShaderStage(stage) => shader_stage_phase(*stage),
                _ => return Err(invalid_property(context, "stage")),
            },
            None => FxPhase::GlyphColor,
        },
        FxNodeKind::PostProcess => FxPhase::PostProcess,
        FxNodeKind::Transition => FxPhase::Transition,
    })
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

const fn shader_stage_phase(value: FxShaderStage) -> FxPhase {
    match value {
        FxShaderStage::GlyphColor => FxPhase::GlyphColor,
        FxShaderStage::OffscreenPass => FxPhase::OffscreenPass,
        FxShaderStage::PostProcess => FxPhase::PostProcess,
    }
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
        FxApplicationError::ArgumentCount { .. }
        | FxApplicationError::MissingArgument { .. }
        | FxApplicationError::ArgumentType { .. } => FxDiagnosticCode::UnitMismatch,
        FxApplicationError::TooManyArguments { .. }
        | FxApplicationError::ArgumentCountOverflow
        | FxApplicationError::DefinitionMismatch { .. }
        | FxApplicationError::LayoutMismatch { .. }
        | FxApplicationError::StorageMismatch => FxDiagnosticCode::ProgramValidation,
    };
    FxDiagnostic::error(code, context.clone(), error.to_string())
}
