//! Transactional resolved-plan output consumed by shared renderers.

use serde::{Deserialize, Serialize};

use super::{
    capability::{FxCapabilitySet, FxPhase, FxRendererInterface, FxTarget},
    diagnostic::{FxDiagnostic, FxDiagnosticCode, FxDiagnosticContext},
    graph::FxResourceId,
    value::{FxRuntimeValue, ResolvedTransform2D},
};

/// Interaction geometry behavior fixed by target semantics.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FxInteractionGeometry {
    /// Node bounds, focus, accessibility, hit testing, and clip share the transform.
    NodeGeometry,
    /// Descendant hit, focus, and accessibility geometry share the content transform.
    ContentDescendants,
    /// Paint-only post-layout transform; interaction/layout geometry is unchanged.
    VisualOnly,
    /// Viewport and input coordinates transform together.
    ViewportCoordinates,
}

/// Closed value in a resolved renderer operation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FxResolvedValue {
    Runtime(FxRuntimeValue),
    Resource(FxResourceId),
    Selector(String),
    String(String),
    List(Vec<FxResolvedValue>),
    Record(Vec<FxNamedValue>),
}

/// Named closed value in a resolved renderer operation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FxNamedValue {
    pub name: String,
    pub value: FxResolvedValue,
}

/// Resolved affine operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedTransformOperation {
    pub phase: FxPhase,
    pub target: FxTarget,
    pub transform: ResolvedTransform2D,
    pub interaction: FxInteractionGeometry,
    pub interactive: bool,
}

/// Other typed renderer-interface output.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedValueOperation {
    pub interface: FxRendererInterface,
    pub phase: FxPhase,
    pub target: FxTarget,
    pub values: Vec<FxNamedValue>,
}

/// Arcweft-owned operation returned by builtin, Rust, and WASM providers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "operation", rename_all = "snake_case")]
pub enum ResolvedFxOperation {
    Transform(ResolvedTransformOperation),
    Values(ResolvedValueOperation),
}

/// One transactionally committed resolved application plan.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedFxPlan {
    layout: Vec<ResolvedFxOperation>,
    glyph: Vec<ResolvedFxOperation>,
    mask: Vec<ResolvedFxOperation>,
    offscreen: Vec<ResolvedFxOperation>,
    post_process: Vec<ResolvedFxOperation>,
    transition: Vec<ResolvedFxOperation>,
    diagnostics: Vec<FxDiagnostic>,
}

impl FxNamedValue {
    pub fn new(name: impl Into<String>, value: FxResolvedValue) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }

    pub fn runtime(name: impl Into<String>, value: FxRuntimeValue) -> Self {
        Self::new(name, FxResolvedValue::Runtime(value))
    }
}

impl ResolvedTransformOperation {
    pub fn new(
        phase: FxPhase,
        target: FxTarget,
        transform: ResolvedTransform2D,
        interactive: bool,
    ) -> Self {
        let interaction = match target {
            FxTarget::Node => FxInteractionGeometry::NodeGeometry,
            FxTarget::Content => FxInteractionGeometry::ContentDescendants,
            FxTarget::Viewport => FxInteractionGeometry::ViewportCoordinates,
            FxTarget::Background | FxTarget::Line | FxTarget::Glyph => {
                FxInteractionGeometry::VisualOnly
            }
        };
        Self {
            phase,
            target,
            transform,
            interaction,
            interactive,
        }
    }
}

impl ResolvedValueOperation {
    pub fn new(
        interface: FxRendererInterface,
        phase: FxPhase,
        target: FxTarget,
        values: Vec<FxNamedValue>,
    ) -> Self {
        Self {
            interface,
            phase,
            target,
            values,
        }
    }
}

impl ResolvedFxOperation {
    pub const fn target(&self) -> FxTarget {
        match self {
            Self::Transform(operation) => operation.target,
            Self::Values(operation) => operation.target,
        }
    }

    pub const fn phase(&self) -> FxPhase {
        match self {
            Self::Transform(operation) => operation.phase,
            Self::Values(operation) => operation.phase,
        }
    }

    pub const fn interface(&self) -> FxRendererInterface {
        match self {
            Self::Transform(_) => FxRendererInterface::Transform,
            Self::Values(operation) => operation.interface,
        }
    }
}

impl ResolvedFxPlan {
    /// Creates a failed application plan without committing partial output.
    pub fn from_diagnostic(diagnostic: FxDiagnostic) -> Self {
        Self {
            diagnostics: vec![diagnostic],
            ..Self::default()
        }
    }

    /// Validates an entire application before committing any operation.
    pub fn resolve_application(
        context: &FxDiagnosticContext,
        capabilities: &FxCapabilitySet,
        operations: Vec<ResolvedFxOperation>,
    ) -> Self {
        if let Some(diagnostic) = validate_application(context, capabilities, &operations) {
            return Self {
                diagnostics: vec![diagnostic],
                ..Self::default()
            };
        }
        let mut plan = Self::default();
        for operation in operations {
            plan.push_committed(operation);
        }
        plan
    }

    /// Appends one validated application atomically to an existing frame plan.
    pub fn append_application(
        &mut self,
        context: &FxDiagnosticContext,
        capabilities: &FxCapabilitySet,
        operations: Vec<ResolvedFxOperation>,
    ) -> bool {
        if let Some(diagnostic) = validate_application(context, capabilities, &operations) {
            self.diagnostics.push(diagnostic);
            return false;
        }
        for operation in operations {
            self.push_committed(operation);
        }
        true
    }

    pub fn layout(&self) -> &[ResolvedFxOperation] {
        &self.layout
    }

    pub fn glyph(&self) -> &[ResolvedFxOperation] {
        &self.glyph
    }

    pub fn mask(&self) -> &[ResolvedFxOperation] {
        &self.mask
    }

    pub fn offscreen(&self) -> &[ResolvedFxOperation] {
        &self.offscreen
    }

    pub fn post_process(&self) -> &[ResolvedFxOperation] {
        &self.post_process
    }

    pub fn transition(&self) -> &[ResolvedFxOperation] {
        &self.transition
    }

    pub fn diagnostics(&self) -> &[FxDiagnostic] {
        &self.diagnostics
    }

    pub fn is_conformant(&self) -> bool {
        self.diagnostics.is_empty()
    }

    fn push_committed(&mut self, operation: ResolvedFxOperation) {
        match operation.phase() {
            FxPhase::BeforeLayout | FxPhase::LayoutTransform => self.layout.push(operation),
            FxPhase::GlyphTransform | FxPhase::GlyphColor => self.glyph.push(operation),
            FxPhase::GlyphMask => self.mask.push(operation),
            FxPhase::OffscreenPass => self.offscreen.push(operation),
            FxPhase::PostProcess => self.post_process.push(operation),
            FxPhase::Transition => self.transition.push(operation),
        }
    }
}

fn validate_application(
    context: &FxDiagnosticContext,
    capabilities: &FxCapabilitySet,
    operations: &[ResolvedFxOperation],
) -> Option<FxDiagnostic> {
    for operation in operations {
        if !capabilities.supports(operation.target(), operation.interface()) {
            return Some(FxDiagnostic::unsupported_capability(
                context.clone(),
                operation.target(),
                operation.interface(),
            ));
        }
        if let ResolvedFxOperation::Transform(transform) = operation {
            let geometry_required = matches!(transform.target, FxTarget::Node | FxTarget::Content)
                && transform.interactive;
            if geometry_required
                && !capabilities.supports(transform.target, FxRendererInterface::GeometryTransform)
            {
                return Some(FxDiagnostic::unsupported_capability(
                    context.clone(),
                    transform.target,
                    FxRendererInterface::GeometryTransform,
                ));
            }
            match transform.transform.is_invertible() {
                Ok(false) if geometry_required => {
                    let mut context = context.clone();
                    context.target = Some(transform.target);
                    context.interface = Some(FxRendererInterface::GeometryTransform);
                    return Some(FxDiagnostic::error(
                        FxDiagnosticCode::NonInvertibleTransform,
                        context,
                        "interactive node/content transform is not invertible",
                    ));
                }
                Err(error) => {
                    let mut context = context.clone();
                    context.target = Some(transform.target);
                    context.interface = Some(FxRendererInterface::Transform);
                    return Some(FxDiagnostic::error(
                        FxDiagnosticCode::NumericNonFinite,
                        context,
                        error.to_string(),
                    ));
                }
                Ok(false | true) => {}
            }
        }
    }
    None
}
