//! Transactional resolved-plan output consumed by shared renderers.

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error as _, SeqAccess, Visitor},
    ser::SerializeTuple,
};

use super::{
    capability::{FxCapabilitySet, FxPhase, FxRendererInterface, FxTarget},
    diagnostic::{FxDiagnostic, FxDiagnosticCode, FxDiagnosticContext},
    graph::{FxFontFamilyName, FxResourceId},
    value::{Angle, FiniteF32, FxColor, FxRuntimeValue, Length, ResolvedTransform2D, Seconds},
};

/// Version-one one-byte operation inventory shared by all presentation
/// providers and renderers.
///
/// The discriminants are part of the presentation runtime contract.  Keep
/// `GeometryTransform` out of this inventory: it is a capability required by
/// interactive transforms, not an executable operation in its own right.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum FxRuntimeOperationOpcode {
    TextStyle = 0,
    Color = 1,
    Transform = 2,
    Mask = 3,
    Filter = 4,
    ShaderUniform = 5,
    OffscreenPass = 6,
    PostProcess = 7,
    Transition = 8,
}

impl FxRuntimeOperationOpcode {
    pub const VERSION: u8 = 1;

    pub const ALL: &'static [Self] = &[
        Self::TextStyle,
        Self::Color,
        Self::Transform,
        Self::Mask,
        Self::Filter,
        Self::ShaderUniform,
        Self::OffscreenPass,
        Self::PostProcess,
        Self::Transition,
    ];

    #[must_use]
    pub const fn encoded(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_encoded(encoded: u8) -> Option<Self> {
        match encoded {
            0 => Some(Self::TextStyle),
            1 => Some(Self::Color),
            2 => Some(Self::Transform),
            3 => Some(Self::Mask),
            4 => Some(Self::Filter),
            5 => Some(Self::ShaderUniform),
            6 => Some(Self::OffscreenPass),
            7 => Some(Self::PostProcess),
            8 => Some(Self::Transition),
            _ => None,
        }
    }
}

impl Serialize for FxRuntimeOperationOpcode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(self.encoded())
    }
}

impl<'de> Deserialize<'de> for FxRuntimeOperationOpcode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = u8::deserialize(deserializer)?;
        Self::from_encoded(encoded).ok_or_else(|| {
            serde::de::Error::custom(format_args!(
                "unknown Fx runtime operation opcode {encoded}"
            ))
        })
    }
}

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
    Selector(super::FxSelectorId),
    ShaderStage(super::FxShaderStage),
    FontFamily(FxFontFamilyName),
    UniformRecord(Vec<FxShaderUniform>),
}

/// One named value in the closed shader-uniform record domain.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FxShaderUniform {
    pub name: super::FxUniformName,
    pub value: FxResolvedValue,
}

/// Typed text-style operation payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedTextStyleOperation {
    pub phase: FxPhase,
    pub target: FxTarget,
    pub opacity: Option<FiniteF32>,
    pub weight: Option<i32>,
    pub slant: Option<Angle>,
    pub font_family: Option<FxFontFamilyName>,
    pub size: Option<Length>,
    pub spacing: Option<Length>,
    pub color: Option<FxColor>,
}

/// Typed color operation payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedColorOperation {
    pub phase: FxPhase,
    pub target: FxTarget,
    pub tint: Option<FxColor>,
    pub multiply: Option<FxColor>,
    pub opacity: Option<FiniteF32>,
}

/// Resolved affine operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedTransformOperation {
    pub phase: FxPhase,
    pub target: FxTarget,
    pub transform: ResolvedTransform2D,
    pub interaction: FxInteractionGeometry,
    pub interactive: bool,
}

/// Typed mask operation payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedMaskOperation {
    pub phase: FxPhase,
    pub target: FxTarget,
    pub resource: Option<FxResourceId>,
    pub coverage: Option<FiniteF32>,
    pub invert: Option<bool>,
}

/// Typed offscreen filter operation payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedFilterOperation {
    pub phase: FxPhase,
    pub target: FxTarget,
    pub blur_radius: Option<Length>,
    pub brightness: Option<FiniteF32>,
    pub contrast: Option<FiniteF32>,
    pub saturation: Option<FiniteF32>,
}

/// One closed shader-uniform operation payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedShaderUniformOperation {
    pub phase: FxPhase,
    pub target: FxTarget,
    pub resource: Option<FxResourceId>,
    pub stage: Option<FxPhase>,
    pub uniforms: Vec<FxShaderUniform>,
}

/// Typed offscreen resource operation payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedOffscreenPassOperation {
    pub phase: FxPhase,
    pub target: FxTarget,
    pub resource: Option<FxResourceId>,
}

/// Typed post-process resource operation payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedPostProcessOperation {
    pub phase: FxPhase,
    pub target: FxTarget,
    pub resource: Option<FxResourceId>,
}

/// Typed transition operation payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedTransitionOperation {
    pub phase: FxPhase,
    pub target: FxTarget,
    pub kind: Option<super::FxSelectorId>,
    pub easing: Option<super::FxSelectorId>,
    pub duration: Option<Seconds>,
    pub progress: Option<FiniteF32>,
}

/// Arcweft-owned operation returned by builtin, Rust, and WASM providers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedFxOperation {
    TextStyle(ResolvedTextStyleOperation),
    Color(ResolvedColorOperation),
    Transform(ResolvedTransformOperation),
    Mask(ResolvedMaskOperation),
    Filter(ResolvedFilterOperation),
    ShaderUniform(ResolvedShaderUniformOperation),
    OffscreenPass(ResolvedOffscreenPassOperation),
    PostProcess(ResolvedPostProcessOperation),
    Transition(ResolvedTransitionOperation),
}

impl Serialize for ResolvedFxOperation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tuple = serializer.serialize_tuple(3)?;
        tuple.serialize_element(&FxRuntimeOperationOpcode::VERSION)?;
        tuple.serialize_element(&self.opcode())?;
        match self {
            Self::TextStyle(operation) => tuple.serialize_element(operation)?,
            Self::Color(operation) => tuple.serialize_element(operation)?,
            Self::Transform(operation) => tuple.serialize_element(operation)?,
            Self::Mask(operation) => tuple.serialize_element(operation)?,
            Self::Filter(operation) => tuple.serialize_element(operation)?,
            Self::ShaderUniform(operation) => tuple.serialize_element(operation)?,
            Self::OffscreenPass(operation) => tuple.serialize_element(operation)?,
            Self::PostProcess(operation) => tuple.serialize_element(operation)?,
            Self::Transition(operation) => tuple.serialize_element(operation)?,
        }
        tuple.end()
    }
}

struct ResolvedFxOperationVisitor;

impl<'de> Visitor<'de> for ResolvedFxOperationVisitor {
    type Value = ResolvedFxOperation;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a version-one Fx runtime operation tuple")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let version: u8 = sequence
            .next_element()?
            .ok_or_else(|| A::Error::custom("missing Fx runtime operation version"))?;
        if version != FxRuntimeOperationOpcode::VERSION {
            return Err(A::Error::custom(format_args!(
                "unsupported Fx runtime operation version {version}"
            )));
        }
        let opcode: FxRuntimeOperationOpcode = sequence
            .next_element()?
            .ok_or_else(|| A::Error::custom("missing Fx runtime operation opcode"))?;
        let operation = match opcode {
            FxRuntimeOperationOpcode::TextStyle => sequence
                .next_element()?
                .map(ResolvedFxOperation::TextStyle)
                .ok_or_else(|| A::Error::custom("missing TextStyle operation payload"))?,
            FxRuntimeOperationOpcode::Color => sequence
                .next_element()?
                .map(ResolvedFxOperation::Color)
                .ok_or_else(|| A::Error::custom("missing Color operation payload"))?,
            FxRuntimeOperationOpcode::Transform => sequence
                .next_element()?
                .map(ResolvedFxOperation::Transform)
                .ok_or_else(|| A::Error::custom("missing Transform operation payload"))?,
            FxRuntimeOperationOpcode::Mask => sequence
                .next_element()?
                .map(ResolvedFxOperation::Mask)
                .ok_or_else(|| A::Error::custom("missing Mask operation payload"))?,
            FxRuntimeOperationOpcode::Filter => sequence
                .next_element()?
                .map(ResolvedFxOperation::Filter)
                .ok_or_else(|| A::Error::custom("missing Filter operation payload"))?,
            FxRuntimeOperationOpcode::ShaderUniform => sequence
                .next_element()?
                .map(ResolvedFxOperation::ShaderUniform)
                .ok_or_else(|| A::Error::custom("missing ShaderUniform operation payload"))?,
            FxRuntimeOperationOpcode::OffscreenPass => sequence
                .next_element()?
                .map(ResolvedFxOperation::OffscreenPass)
                .ok_or_else(|| A::Error::custom("missing OffscreenPass operation payload"))?,
            FxRuntimeOperationOpcode::PostProcess => sequence
                .next_element()?
                .map(ResolvedFxOperation::PostProcess)
                .ok_or_else(|| A::Error::custom("missing PostProcess operation payload"))?,
            FxRuntimeOperationOpcode::Transition => sequence
                .next_element()?
                .map(ResolvedFxOperation::Transition)
                .ok_or_else(|| A::Error::custom("missing Transition operation payload"))?,
        };
        if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
            return Err(A::Error::custom(
                "Fx runtime operation tuple has trailing fields",
            ));
        }
        Ok(operation)
    }
}

impl<'de> Deserialize<'de> for ResolvedFxOperation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_tuple(3, ResolvedFxOperationVisitor)
    }
}

/*
 * This deliberately remains a direct match rather than a second operation
 * table.  The enum above is the semantic and serialization authority; the
 * one-byte opcode is only its fixed wire discriminator.
 */
impl ResolvedFxOperation {
    #[must_use]
    pub const fn opcode(&self) -> FxRuntimeOperationOpcode {
        match self {
            Self::TextStyle(_) => FxRuntimeOperationOpcode::TextStyle,
            Self::Color(_) => FxRuntimeOperationOpcode::Color,
            Self::Transform(_) => FxRuntimeOperationOpcode::Transform,
            Self::Mask(_) => FxRuntimeOperationOpcode::Mask,
            Self::Filter(_) => FxRuntimeOperationOpcode::Filter,
            Self::ShaderUniform(_) => FxRuntimeOperationOpcode::ShaderUniform,
            Self::OffscreenPass(_) => FxRuntimeOperationOpcode::OffscreenPass,
            Self::PostProcess(_) => FxRuntimeOperationOpcode::PostProcess,
            Self::Transition(_) => FxRuntimeOperationOpcode::Transition,
        }
    }

    #[must_use]
    pub const fn value_count(&self) -> usize {
        match self {
            Self::TextStyle(operation) => operation.value_count(),
            Self::Color(operation) => operation.value_count(),
            Self::Transform(_) => 0,
            Self::Mask(operation) => operation.value_count(),
            Self::Filter(operation) => operation.value_count(),
            Self::ShaderUniform(operation) => operation.value_count(),
            Self::OffscreenPass(operation) => operation.value_count(),
            Self::PostProcess(operation) => operation.value_count(),
            Self::Transition(operation) => operation.value_count(),
        }
    }
}

/*
 * The value count is a provider-budget concern, not semantic operation data.
 * It is computed from typed fields instead of stored as a shadow property.
 */
impl ResolvedTextStyleOperation {
    #[must_use]
    pub const fn value_count(&self) -> usize {
        self.opacity.is_some() as usize
            + self.weight.is_some() as usize
            + self.slant.is_some() as usize
            + self.font_family.is_some() as usize
            + self.size.is_some() as usize
            + self.spacing.is_some() as usize
            + self.color.is_some() as usize
    }
}

impl ResolvedColorOperation {
    #[must_use]
    pub const fn value_count(&self) -> usize {
        self.tint.is_some() as usize
            + self.multiply.is_some() as usize
            + self.opacity.is_some() as usize
    }
}

impl ResolvedMaskOperation {
    #[must_use]
    pub const fn value_count(&self) -> usize {
        self.resource.is_some() as usize
            + self.coverage.is_some() as usize
            + self.invert.is_some() as usize
    }
}

impl ResolvedFilterOperation {
    #[must_use]
    pub const fn value_count(&self) -> usize {
        self.blur_radius.is_some() as usize
            + self.brightness.is_some() as usize
            + self.contrast.is_some() as usize
            + self.saturation.is_some() as usize
    }
}

impl ResolvedShaderUniformOperation {
    #[must_use]
    pub const fn value_count(&self) -> usize {
        self.resource.is_some() as usize + self.stage.is_some() as usize + self.uniforms.len()
    }
}

impl ResolvedOffscreenPassOperation {
    #[must_use]
    pub const fn value_count(&self) -> usize {
        self.resource.is_some() as usize
    }
}

impl ResolvedPostProcessOperation {
    #[must_use]
    pub const fn value_count(&self) -> usize {
        self.resource.is_some() as usize
    }
}

impl ResolvedTransitionOperation {
    #[must_use]
    pub const fn value_count(&self) -> usize {
        self.kind.is_some() as usize
            + self.easing.is_some() as usize
            + self.duration.is_some() as usize
            + self.progress.is_some() as usize
    }
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

impl FxShaderUniform {
    pub const fn new(name: super::FxUniformName, value: FxResolvedValue) -> Self {
        Self { name, value }
    }

    pub fn runtime(name: super::FxUniformName, value: FxRuntimeValue) -> Self {
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

impl ResolvedFxOperation {
    pub const fn target(&self) -> FxTarget {
        match self {
            Self::TextStyle(operation) => operation.target,
            Self::Color(operation) => operation.target,
            Self::Transform(operation) => operation.target,
            Self::Mask(operation) => operation.target,
            Self::Filter(operation) => operation.target,
            Self::ShaderUniform(operation) => operation.target,
            Self::OffscreenPass(operation) => operation.target,
            Self::PostProcess(operation) => operation.target,
            Self::Transition(operation) => operation.target,
        }
    }

    pub const fn phase(&self) -> FxPhase {
        match self {
            Self::TextStyle(operation) => operation.phase,
            Self::Color(operation) => operation.phase,
            Self::Transform(operation) => operation.phase,
            Self::Mask(operation) => operation.phase,
            Self::Filter(operation) => operation.phase,
            Self::ShaderUniform(operation) => operation.phase,
            Self::OffscreenPass(operation) => operation.phase,
            Self::PostProcess(operation) => operation.phase,
            Self::Transition(operation) => operation.phase,
        }
    }

    pub const fn interface(&self) -> FxRendererInterface {
        match self {
            Self::TextStyle(_) => FxRendererInterface::TextStyle,
            Self::Color(_) => FxRendererInterface::Color,
            Self::Transform(_) => FxRendererInterface::Transform,
            Self::Mask(_) => FxRendererInterface::Mask,
            Self::Filter(_) => FxRendererInterface::Filter,
            Self::ShaderUniform(_) => FxRendererInterface::ShaderUniform,
            Self::OffscreenPass(_) => FxRendererInterface::OffscreenPass,
            Self::PostProcess(_) => FxRendererInterface::PostProcess,
            Self::Transition(_) => FxRendererInterface::Transition,
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
