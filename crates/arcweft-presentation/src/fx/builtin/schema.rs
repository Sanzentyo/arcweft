//! Immutable callable schemas for Arcweft-owned Fx entries.

use super::super::{
    FxDefinitionParameterType, FxPhase, FxRuntimeType, FxTarget, MotionFunction,
    canonical::{CanonicalEncoder, CanonicalHashSink, CanonicalSink},
};

/// Arcweft-owned version of the builtin Fx callable catalog.
pub const BUILTIN_FX_CALLABLE_SCHEMA_VERSION: u8 = 1;

/// Stable builtin Fx callable identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum BuiltinFxCallableId {
    Wave = 0,
    Shake = 1,
    Jitter = 2,
    Arc = 3,
    Spin = 4,
    Pulse = 5,
    Motion = 6,
    Sparkle = 7,
    Typewriter = 8,
    Shader = 9,
}

/// One exact phase-specialized overload row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum BuiltinFxCallableRowId {
    WaveGlyphTransform = 0,
    WavePostProcess = 1,
    ShakeGlyphTransform = 2,
    ShakePostProcess = 3,
    JitterGlyphTransform = 4,
    JitterPostProcess = 5,
    ArcGlyphTransform = 6,
    ArcPostProcess = 7,
    SpinGlyphTransform = 8,
    SpinPostProcess = 9,
    PulseGlyphTransform = 10,
    PulsePostProcess = 11,
    MotionGlyphTransform = 12,
    MotionPostProcess = 13,
    SparkleGlyphTransform = 14,
    SparkleGlyphColor = 15,
    SparklePostProcess = 16,
    TypewriterGlyphMask = 17,
    ShaderGlyphColor = 18,
    ShaderOffscreenPass = 19,
    ShaderPostProcess = 20,
}

/// Stable parameter identity shared by every builtin Fx row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum BuiltinFxParameterId {
    Phase = 0,
    Target = 1,
    Resource = 2,
    Amplitude = 3,
    Amount = 4,
    Period = 5,
    Speed = 6,
    Direction = 7,
    Seed = 8,
    Radius = 9,
    StartAngle = 10,
    StepAngle = 11,
    RotationAmplitude = 12,
    MotionFunction = 13,
    ScaleAmplitude = 14,
    CharactersPerSecond = 15,
    Delay = 16,
    Cursor = 17,
    CursorAlpha = 18,
    Color = 19,
}

/// Exact source type of one builtin Fx parameter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuiltinFxParameterType {
    Bool,
    Seed32,
    FixedMilli,
    Ratio,
    Length,
    Angle,
    Duration,
    Color,
    Vec2,
    Resource,
    Phase,
    Target,
    MotionFunction,
}

impl BuiltinFxParameterType {
    pub const fn runtime_value_type(self) -> Option<FxRuntimeType> {
        Some(match self {
            Self::Bool => FxRuntimeType::Bool,
            Self::Seed32 => FxRuntimeType::U32,
            Self::FixedMilli | Self::Ratio => FxRuntimeType::F32,
            Self::Length => FxRuntimeType::Length,
            Self::Angle => FxRuntimeType::Angle,
            Self::Duration => FxRuntimeType::Seconds,
            Self::Color => FxRuntimeType::Color,
            Self::Vec2 => FxRuntimeType::Vec2,
            Self::Resource | Self::Phase | Self::Target | Self::MotionFunction => return None,
        })
    }

    pub const fn direct_definition_parameter_type(self) -> Option<FxDefinitionParameterType> {
        Some(match self {
            Self::Bool => FxDefinitionParameterType::Runtime(FxRuntimeType::Bool),
            Self::Seed32 => FxDefinitionParameterType::Runtime(FxRuntimeType::U32),
            Self::FixedMilli | Self::Ratio => {
                FxDefinitionParameterType::Runtime(FxRuntimeType::F32)
            }
            Self::Length => FxDefinitionParameterType::Runtime(FxRuntimeType::Length),
            Self::Angle => FxDefinitionParameterType::Runtime(FxRuntimeType::Angle),
            Self::Duration => FxDefinitionParameterType::Runtime(FxRuntimeType::Seconds),
            Self::Color => FxDefinitionParameterType::Runtime(FxRuntimeType::Color),
            Self::Vec2 => FxDefinitionParameterType::Runtime(FxRuntimeType::Vec2),
            Self::Resource => FxDefinitionParameterType::Resource,
            Self::Phase | Self::Target | Self::MotionFunction => {
                return None;
            }
        })
    }
}

/// Whether a parameter selects graph structure or occupies an application ABI
/// slot.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuiltinFxParameterBinding {
    Structural,
    Abi,
}

/// All builtin Fx call parameters are named-only.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuiltinFxParameterPassing {
    NamedOnly,
}

/// Unit spellings admitted by a numeric parameter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum BuiltinFxUnit {
    Unitless = 0,
    Px = 1,
    Deg = 2,
    Ms = 3,
    S = 4,
}

/// Closed numeric admission limits in source milli-units.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BuiltinFxNumericConstraint {
    pub inclusive_min_milli: i64,
    pub inclusive_max_milli: i64,
    pub units: &'static [BuiltinFxUnit],
    pub max_integer_digits: u8,
    pub max_fraction_digits: u8,
}

/// Non-type constraints retained on the owning callable row.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuiltinFxValueConstraint {
    None,
    Numeric(BuiltinFxNumericConstraint),
    ExactPhase(FxPhase),
    AllowedTargets(&'static [FxTarget]),
}

/// Closed materializable defaults used by builtin Fx rows.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuiltinFxDefaultValue {
    Bool(bool),
    Milli(i64),
    RatioMilli(i64),
    LengthMilliPx(i64),
    AngleMilliDegrees(i64),
    DurationMillis(i64),
    Seed32(u32),
    Vec2Milli([i64; 2]),
    Phase(FxPhase),
    Target(FxTarget),
    MotionFunction(MotionFunction),
}

/// Typed predicate used by conditional parameter admission.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuiltinFxParameterPredicate {
    BoolEquals {
        parameter: BuiltinFxParameterId,
        value: bool,
    },
}

/// Presence/default policy for one row parameter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuiltinFxParameterPresence {
    Required,
    Optional,
    Defaulted(BuiltinFxDefaultValue),
    Conditional {
        predicate: BuiltinFxParameterPredicate,
        default: Option<BuiltinFxDefaultValue>,
    },
}

/// One exact parameter descriptor in a builtin Fx overload row.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BuiltinFxCallableParameter {
    id: BuiltinFxParameterId,
    source_name: &'static str,
    parameter_type: BuiltinFxParameterType,
    passing: BuiltinFxParameterPassing,
    binding: BuiltinFxParameterBinding,
    presence: BuiltinFxParameterPresence,
    constraint: BuiltinFxValueConstraint,
}

/// One immutable builtin Fx overload row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuiltinFxCallableRow {
    id: BuiltinFxCallableRowId,
    parameters: &'static [BuiltinFxCallableParameter],
}

/// Stable digest of a complete builtin Fx callable row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BuiltinFxCallableSchemaDigest([u8; 32]);

/// Immutable catalog of all Arcweft-owned builtin Fx overload rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuiltinFxCallableCatalog {
    rows: &'static [BuiltinFxCallableRow],
}

impl BuiltinFxCallableId {
    pub const ALL: [Self; 10] = [
        Self::Wave,
        Self::Shake,
        Self::Jitter,
        Self::Arc,
        Self::Spin,
        Self::Pulse,
        Self::Motion,
        Self::Sparkle,
        Self::Typewriter,
        Self::Shader,
    ];

    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Wave => "wave",
            Self::Shake => "shake",
            Self::Jitter => "jitter",
            Self::Arc => "arc",
            Self::Spin => "spin",
            Self::Pulse => "pulse",
            Self::Motion => "motion",
            Self::Sparkle => "sparkle",
            Self::Typewriter => "typewriter",
            Self::Shader => "shader",
        }
    }

    #[must_use]
    pub fn from_source_name(source_name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.source_name() == source_name)
    }

    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Wave => 0,
            Self::Shake => 1,
            Self::Jitter => 2,
            Self::Arc => 3,
            Self::Spin => 4,
            Self::Pulse => 5,
            Self::Motion => 6,
            Self::Sparkle => 7,
            Self::Typewriter => 8,
            Self::Shader => 9,
        }
    }
}

impl BuiltinFxCallableRowId {
    pub const ALL: [Self; 21] = [
        Self::WaveGlyphTransform,
        Self::WavePostProcess,
        Self::ShakeGlyphTransform,
        Self::ShakePostProcess,
        Self::JitterGlyphTransform,
        Self::JitterPostProcess,
        Self::ArcGlyphTransform,
        Self::ArcPostProcess,
        Self::SpinGlyphTransform,
        Self::SpinPostProcess,
        Self::PulseGlyphTransform,
        Self::PulsePostProcess,
        Self::MotionGlyphTransform,
        Self::MotionPostProcess,
        Self::SparkleGlyphTransform,
        Self::SparkleGlyphColor,
        Self::SparklePostProcess,
        Self::TypewriterGlyphMask,
        Self::ShaderGlyphColor,
        Self::ShaderOffscreenPass,
        Self::ShaderPostProcess,
    ];

    #[must_use]
    pub const fn callable(self) -> BuiltinFxCallableId {
        match self {
            Self::WaveGlyphTransform | Self::WavePostProcess => BuiltinFxCallableId::Wave,
            Self::ShakeGlyphTransform | Self::ShakePostProcess => BuiltinFxCallableId::Shake,
            Self::JitterGlyphTransform | Self::JitterPostProcess => BuiltinFxCallableId::Jitter,
            Self::ArcGlyphTransform | Self::ArcPostProcess => BuiltinFxCallableId::Arc,
            Self::SpinGlyphTransform | Self::SpinPostProcess => BuiltinFxCallableId::Spin,
            Self::PulseGlyphTransform | Self::PulsePostProcess => BuiltinFxCallableId::Pulse,
            Self::MotionGlyphTransform | Self::MotionPostProcess => BuiltinFxCallableId::Motion,
            Self::SparkleGlyphTransform | Self::SparkleGlyphColor | Self::SparklePostProcess => {
                BuiltinFxCallableId::Sparkle
            }
            Self::TypewriterGlyphMask => BuiltinFxCallableId::Typewriter,
            Self::ShaderGlyphColor | Self::ShaderOffscreenPass | Self::ShaderPostProcess => {
                BuiltinFxCallableId::Shader
            }
        }
    }

    #[must_use]
    pub const fn phase(self) -> FxPhase {
        match self {
            Self::WaveGlyphTransform
            | Self::ShakeGlyphTransform
            | Self::JitterGlyphTransform
            | Self::ArcGlyphTransform
            | Self::SpinGlyphTransform
            | Self::PulseGlyphTransform
            | Self::MotionGlyphTransform
            | Self::SparkleGlyphTransform => FxPhase::GlyphTransform,
            Self::SparkleGlyphColor | Self::ShaderGlyphColor => FxPhase::GlyphColor,
            Self::TypewriterGlyphMask => FxPhase::GlyphMask,
            Self::ShaderOffscreenPass => FxPhase::OffscreenPass,
            Self::WavePostProcess
            | Self::ShakePostProcess
            | Self::JitterPostProcess
            | Self::ArcPostProcess
            | Self::SpinPostProcess
            | Self::PulsePostProcess
            | Self::MotionPostProcess
            | Self::SparklePostProcess
            | Self::ShaderPostProcess => FxPhase::PostProcess,
        }
    }

    #[must_use]
    pub const fn is_default_phase(self) -> bool {
        matches!(
            self,
            Self::WaveGlyphTransform
                | Self::ShakeGlyphTransform
                | Self::JitterGlyphTransform
                | Self::ArcGlyphTransform
                | Self::SpinGlyphTransform
                | Self::PulseGlyphTransform
                | Self::MotionGlyphTransform
                | Self::SparkleGlyphTransform
                | Self::TypewriterGlyphMask
                | Self::ShaderOffscreenPass
        )
    }

    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::WaveGlyphTransform => 0,
            Self::WavePostProcess => 1,
            Self::ShakeGlyphTransform => 2,
            Self::ShakePostProcess => 3,
            Self::JitterGlyphTransform => 4,
            Self::JitterPostProcess => 5,
            Self::ArcGlyphTransform => 6,
            Self::ArcPostProcess => 7,
            Self::SpinGlyphTransform => 8,
            Self::SpinPostProcess => 9,
            Self::PulseGlyphTransform => 10,
            Self::PulsePostProcess => 11,
            Self::MotionGlyphTransform => 12,
            Self::MotionPostProcess => 13,
            Self::SparkleGlyphTransform => 14,
            Self::SparkleGlyphColor => 15,
            Self::SparklePostProcess => 16,
            Self::TypewriterGlyphMask => 17,
            Self::ShaderGlyphColor => 18,
            Self::ShaderOffscreenPass => 19,
            Self::ShaderPostProcess => 20,
        }
    }

    /// Zero-based overload coordinate within this row's source callable.
    ///
    /// The global semantic tag identifies the closed row catalog. Callable
    /// publication uses this local coordinate because each source name owns
    /// an independent, contiguous overload family.
    #[must_use]
    pub const fn callable_overload_ordinal(self) -> u8 {
        match self {
            Self::WaveGlyphTransform
            | Self::ShakeGlyphTransform
            | Self::JitterGlyphTransform
            | Self::ArcGlyphTransform
            | Self::SpinGlyphTransform
            | Self::PulseGlyphTransform
            | Self::MotionGlyphTransform
            | Self::SparkleGlyphTransform
            | Self::TypewriterGlyphMask
            | Self::ShaderGlyphColor => 0,
            Self::WavePostProcess
            | Self::ShakePostProcess
            | Self::JitterPostProcess
            | Self::ArcPostProcess
            | Self::SpinPostProcess
            | Self::PulsePostProcess
            | Self::MotionPostProcess
            | Self::SparkleGlyphColor
            | Self::ShaderOffscreenPass => 1,
            Self::SparklePostProcess | Self::ShaderPostProcess => 2,
        }
    }
}

impl BuiltinFxParameterId {
    pub const ALL: [Self; 20] = [
        Self::Phase,
        Self::Target,
        Self::Resource,
        Self::Amplitude,
        Self::Amount,
        Self::Period,
        Self::Speed,
        Self::Direction,
        Self::Seed,
        Self::Radius,
        Self::StartAngle,
        Self::StepAngle,
        Self::RotationAmplitude,
        Self::MotionFunction,
        Self::ScaleAmplitude,
        Self::CharactersPerSecond,
        Self::Delay,
        Self::Cursor,
        Self::CursorAlpha,
        Self::Color,
    ];

    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Phase => "phase",
            Self::Target => "target",
            Self::Resource => "resource",
            Self::Amplitude => "amplitude",
            Self::Amount => "amount",
            Self::Period => "period",
            Self::Speed => "speed",
            Self::Direction => "direction",
            Self::Seed => "seed",
            Self::Radius => "radius",
            Self::StartAngle => "start_angle",
            Self::StepAngle => "step_angle",
            Self::RotationAmplitude => "rotation_amplitude",
            Self::MotionFunction => "motion_function",
            Self::ScaleAmplitude => "scale_amplitude",
            Self::CharactersPerSecond => "characters_per_second",
            Self::Delay => "delay",
            Self::Cursor => "cursor",
            Self::CursorAlpha => "cursor_alpha",
            Self::Color => "color",
        }
    }

    #[must_use]
    pub fn from_source_name(source_name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.source_name() == source_name)
    }

    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Phase => 0,
            Self::Target => 1,
            Self::Resource => 2,
            Self::Amplitude => 3,
            Self::Amount => 4,
            Self::Period => 5,
            Self::Speed => 6,
            Self::Direction => 7,
            Self::Seed => 8,
            Self::Radius => 9,
            Self::StartAngle => 10,
            Self::StepAngle => 11,
            Self::RotationAmplitude => 12,
            Self::MotionFunction => 13,
            Self::ScaleAmplitude => 14,
            Self::CharactersPerSecond => 15,
            Self::Delay => 16,
            Self::Cursor => 17,
            Self::CursorAlpha => 18,
            Self::Color => 19,
        }
    }
}

impl BuiltinFxCallableParameter {
    #[must_use]
    pub const fn id(self) -> BuiltinFxParameterId {
        self.id
    }
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        self.source_name
    }
    #[must_use]
    pub const fn parameter_type(self) -> BuiltinFxParameterType {
        self.parameter_type
    }
    #[must_use]
    pub const fn passing(self) -> BuiltinFxParameterPassing {
        self.passing
    }
    #[must_use]
    pub const fn binding(self) -> BuiltinFxParameterBinding {
        self.binding
    }
    #[must_use]
    pub const fn presence(self) -> BuiltinFxParameterPresence {
        self.presence
    }
    #[must_use]
    pub const fn constraint(self) -> BuiltinFxValueConstraint {
        self.constraint
    }
}

impl BuiltinFxCallableRow {
    #[must_use]
    pub const fn id(self) -> BuiltinFxCallableRowId {
        self.id
    }
    #[must_use]
    pub const fn callable(self) -> BuiltinFxCallableId {
        self.id.callable()
    }
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        self.callable().source_name()
    }
    #[must_use]
    pub const fn phase(self) -> FxPhase {
        self.id.phase()
    }
    #[must_use]
    pub const fn parameters(self) -> &'static [BuiltinFxCallableParameter] {
        self.parameters
    }
    #[must_use]
    pub fn parameter(self, id: BuiltinFxParameterId) -> Option<BuiltinFxCallableParameter> {
        self.parameters.iter().copied().find(|row| row.id == id)
    }
    #[must_use]
    pub fn schema_digest(self) -> BuiltinFxCallableSchemaDigest {
        BuiltinFxCallableSchemaDigest::derive(self)
    }
}

impl BuiltinFxCallableSchemaDigest {
    #[must_use]
    pub fn derive(row: BuiltinFxCallableRow) -> Self {
        let mut hasher = blake3::Hasher::new();
        let mut encoder = CanonicalEncoder::new(CanonicalHashSink::new(&mut hasher));
        infallible(encode_row(&mut encoder, row));
        Self(*hasher.finalize().as_bytes())
    }
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

fn infallible(result: Result<(), std::convert::Infallible>) {
    match result {
        Ok(()) => {}
        Err(never) => match never {},
    }
}

impl BuiltinFxCallableCatalog {
    const fn new(rows: &'static [BuiltinFxCallableRow]) -> Self {
        Self { rows }
    }
    #[must_use]
    pub const fn rows(self) -> &'static [BuiltinFxCallableRow] {
        self.rows
    }
    #[must_use]
    pub const fn len(self) -> usize {
        self.rows.len()
    }
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.rows.is_empty()
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = BuiltinFxCallableRow> {
        self.rows.iter().copied()
    }
    #[must_use]
    pub fn get(self, id: BuiltinFxCallableRowId) -> Option<BuiltinFxCallableRow> {
        self.rows.iter().copied().find(|row| row.id == id)
    }
    pub fn rows_for(
        self,
        callable: BuiltinFxCallableId,
    ) -> impl Iterator<Item = BuiltinFxCallableRow> {
        self.iter().filter(move |row| row.callable() == callable)
    }
    #[must_use]
    pub fn resolve(self, source_name: &str) -> Option<BuiltinFxCallableId> {
        BuiltinFxCallableId::from_source_name(source_name)
    }
}

impl Default for BuiltinFxCallableCatalog {
    fn default() -> Self {
        BUILTIN_FX_CALLABLE_CATALOG
    }
}

const UNITLESS: &[BuiltinFxUnit] = &[BuiltinFxUnit::Unitless];
const PX: &[BuiltinFxUnit] = &[BuiltinFxUnit::Px];
const DEG: &[BuiltinFxUnit] = &[BuiltinFxUnit::Deg];
const DURATION: &[BuiltinFxUnit] = &[BuiltinFxUnit::Ms, BuiltinFxUnit::S];
const CONTENT_TARGETS: &[FxTarget] = &[FxTarget::Content, FxTarget::Line, FxTarget::Glyph];
const VIEWPORT_TARGETS: &[FxTarget] = &[FxTarget::Viewport];

const fn parameter(
    id: BuiltinFxParameterId,
    parameter_type: BuiltinFxParameterType,
    binding: BuiltinFxParameterBinding,
    presence: BuiltinFxParameterPresence,
    constraint: BuiltinFxValueConstraint,
) -> BuiltinFxCallableParameter {
    BuiltinFxCallableParameter {
        id,
        source_name: id.source_name(),
        parameter_type,
        passing: BuiltinFxParameterPassing::NamedOnly,
        binding,
        presence,
        constraint,
    }
}

const fn numeric(
    minimum: i64,
    maximum: i64,
    units: &'static [BuiltinFxUnit],
) -> BuiltinFxValueConstraint {
    BuiltinFxValueConstraint::Numeric(BuiltinFxNumericConstraint {
        inclusive_min_milli: minimum,
        inclusive_max_milli: maximum,
        units,
        max_integer_digits: 19,
        max_fraction_digits: 3,
    })
}

const fn phase(row: BuiltinFxCallableRowId) -> BuiltinFxCallableParameter {
    let presence = if row.is_default_phase() {
        BuiltinFxParameterPresence::Defaulted(BuiltinFxDefaultValue::Phase(row.phase()))
    } else {
        BuiltinFxParameterPresence::Required
    };
    parameter(
        BuiltinFxParameterId::Phase,
        BuiltinFxParameterType::Phase,
        BuiltinFxParameterBinding::Structural,
        presence,
        BuiltinFxValueConstraint::ExactPhase(row.phase()),
    )
}

const fn target(post_process: bool) -> BuiltinFxCallableParameter {
    let (default, allowed) = if post_process {
        (FxTarget::Viewport, VIEWPORT_TARGETS)
    } else {
        (FxTarget::Content, CONTENT_TARGETS)
    };
    parameter(
        BuiltinFxParameterId::Target,
        BuiltinFxParameterType::Target,
        BuiltinFxParameterBinding::Structural,
        BuiltinFxParameterPresence::Defaulted(BuiltinFxDefaultValue::Target(default)),
        BuiltinFxValueConstraint::AllowedTargets(allowed),
    )
}

const fn abi(
    id: BuiltinFxParameterId,
    parameter_type: BuiltinFxParameterType,
    presence: BuiltinFxParameterPresence,
    constraint: BuiltinFxValueConstraint,
) -> BuiltinFxCallableParameter {
    parameter(
        id,
        parameter_type,
        BuiltinFxParameterBinding::Abi,
        presence,
        constraint,
    )
}

const fn default(value: BuiltinFxDefaultValue) -> BuiltinFxParameterPresence {
    BuiltinFxParameterPresence::Defaulted(value)
}

const AMP_4PX: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Amplitude,
    BuiltinFxParameterType::Length,
    default(BuiltinFxDefaultValue::LengthMilliPx(4_000)),
    numeric(0, 4_096_000, PX),
);
const AMP_3PX: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Amplitude,
    BuiltinFxParameterType::Length,
    default(BuiltinFxDefaultValue::LengthMilliPx(3_000)),
    numeric(0, 4_096_000, PX),
);
const AMP_2PX: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Amplitude,
    BuiltinFxParameterType::Length,
    default(BuiltinFxDefaultValue::LengthMilliPx(2_000)),
    numeric(0, 4_096_000, PX),
);
const AMP_1_6PX: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Amplitude,
    BuiltinFxParameterType::Length,
    default(BuiltinFxDefaultValue::LengthMilliPx(1_600)),
    numeric(0, 4_096_000, PX),
);
const SCALE_AMP_0_08: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::ScaleAmplitude,
    BuiltinFxParameterType::Ratio,
    default(BuiltinFxDefaultValue::RatioMilli(80)),
    numeric(0, 10_000, UNITLESS),
);
const PERIOD_12: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Period,
    BuiltinFxParameterType::FixedMilli,
    default(BuiltinFxDefaultValue::Milli(12_000)),
    numeric(1, 65_536_000, UNITLESS),
);
const PERIOD_64PX: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Period,
    BuiltinFxParameterType::Length,
    default(BuiltinFxDefaultValue::LengthMilliPx(64_000)),
    numeric(1, 65_536_000, PX),
);
const SPEED_1_POSITIVE: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Speed,
    BuiltinFxParameterType::FixedMilli,
    default(BuiltinFxDefaultValue::Milli(1_000)),
    numeric(1, 1_000_000, UNITLESS),
);
const SPEED_1_NONNEGATIVE: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Speed,
    BuiltinFxParameterType::FixedMilli,
    default(BuiltinFxDefaultValue::Milli(1_000)),
    numeric(0, 1_000_000, UNITLESS),
);
const SPEED_16: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Speed,
    BuiltinFxParameterType::FixedMilli,
    default(BuiltinFxDefaultValue::Milli(16_000)),
    numeric(1, 1_000_000, UNITLESS),
);
const SPEED_2_2: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Speed,
    BuiltinFxParameterType::FixedMilli,
    default(BuiltinFxDefaultValue::Milli(2_200)),
    numeric(1, 1_000_000, UNITLESS),
);
const DIRECTION_VERTICAL: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Direction,
    BuiltinFxParameterType::Vec2,
    default(BuiltinFxDefaultValue::Vec2Milli([0, 1_000])),
    numeric(-1_000_000, 1_000_000, UNITLESS),
);
const DIRECTION_HORIZONTAL: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Direction,
    BuiltinFxParameterType::Vec2,
    default(BuiltinFxDefaultValue::Vec2Milli([1_000, 0])),
    numeric(-1_000_000, 1_000_000, UNITLESS),
);
const SEED_ZERO: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Seed,
    BuiltinFxParameterType::Seed32,
    default(BuiltinFxDefaultValue::Seed32(0)),
    BuiltinFxValueConstraint::None,
);
const RADIUS_120PX: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Radius,
    BuiltinFxParameterType::Length,
    default(BuiltinFxDefaultValue::LengthMilliPx(120_000)),
    numeric(0, 65_536_000, PX),
);
const START_ANGLE_ZERO: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::StartAngle,
    BuiltinFxParameterType::Angle,
    default(BuiltinFxDefaultValue::AngleMilliDegrees(0)),
    numeric(-360_000_000, 360_000_000, DEG),
);
const STEP_ANGLE_EIGHT: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::StepAngle,
    BuiltinFxParameterType::Angle,
    default(BuiltinFxDefaultValue::AngleMilliDegrees(8_000)),
    numeric(-360_000_000, 360_000_000, DEG),
);
const ROTATION_AMP_SIX: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::RotationAmplitude,
    BuiltinFxParameterType::Angle,
    default(BuiltinFxDefaultValue::AngleMilliDegrees(6_000)),
    numeric(-360_000_000, 360_000_000, DEG),
);
const AMOUNT_0_18: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Amount,
    BuiltinFxParameterType::Ratio,
    default(BuiltinFxDefaultValue::RatioMilli(180)),
    numeric(0, 1_000, UNITLESS),
);
const AMOUNT_0_35: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Amount,
    BuiltinFxParameterType::Ratio,
    default(BuiltinFxDefaultValue::RatioMilli(350)),
    numeric(0, 1_000, UNITLESS),
);
const MOTION_FUNCTION: BuiltinFxCallableParameter = parameter(
    BuiltinFxParameterId::MotionFunction,
    BuiltinFxParameterType::MotionFunction,
    BuiltinFxParameterBinding::Structural,
    default(BuiltinFxDefaultValue::MotionFunction(
        MotionFunction::BreathOrbit,
    )),
    BuiltinFxValueConstraint::None,
);
const CPS_28: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::CharactersPerSecond,
    BuiltinFxParameterType::FixedMilli,
    default(BuiltinFxDefaultValue::Milli(28_000)),
    numeric(1_000, 240_000, UNITLESS),
);
const DELAY_ZERO: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Delay,
    BuiltinFxParameterType::Duration,
    default(BuiltinFxDefaultValue::DurationMillis(0)),
    numeric(0, 86_400_000_000, DURATION),
);
const CURSOR_FALSE: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Cursor,
    BuiltinFxParameterType::Bool,
    default(BuiltinFxDefaultValue::Bool(false)),
    BuiltinFxValueConstraint::None,
);
const CURSOR_ALPHA: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::CursorAlpha,
    BuiltinFxParameterType::Ratio,
    BuiltinFxParameterPresence::Conditional {
        predicate: BuiltinFxParameterPredicate::BoolEquals {
            parameter: BuiltinFxParameterId::Cursor,
            value: true,
        },
        default: Some(BuiltinFxDefaultValue::RatioMilli(350)),
    },
    numeric(0, 1_000, UNITLESS),
);
const SHADER_RESOURCE: BuiltinFxCallableParameter = parameter(
    BuiltinFxParameterId::Resource,
    BuiltinFxParameterType::Resource,
    BuiltinFxParameterBinding::Abi,
    BuiltinFxParameterPresence::Required,
    BuiltinFxValueConstraint::None,
);
const SHADER_AMOUNT: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Amount,
    BuiltinFxParameterType::Ratio,
    BuiltinFxParameterPresence::Optional,
    numeric(0, 1_000, UNITLESS),
);
const SHADER_DIRECTION: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Direction,
    BuiltinFxParameterType::Vec2,
    BuiltinFxParameterPresence::Optional,
    numeric(-1_000_000, 1_000_000, UNITLESS),
);
const SHADER_COLOR: BuiltinFxCallableParameter = abi(
    BuiltinFxParameterId::Color,
    BuiltinFxParameterType::Color,
    BuiltinFxParameterPresence::Optional,
    BuiltinFxValueConstraint::None,
);

const WAVE_GLYPH: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::WaveGlyphTransform),
    target(false),
    AMP_4PX,
    PERIOD_12,
    SPEED_1_POSITIVE,
    DIRECTION_VERTICAL,
];
const WAVE_POST: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::WavePostProcess),
    target(true),
    AMP_3PX,
    PERIOD_64PX,
    SPEED_1_NONNEGATIVE,
    DIRECTION_HORIZONTAL,
    SEED_ZERO,
];
const SHAKE_GLYPH: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::ShakeGlyphTransform),
    target(false),
    AMP_2PX,
    SPEED_16,
    DIRECTION_VERTICAL,
    SEED_ZERO,
];
const SHAKE_POST: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::ShakePostProcess),
    target(true),
    AMP_3PX,
    PERIOD_64PX,
    SPEED_1_NONNEGATIVE,
    DIRECTION_HORIZONTAL,
    SEED_ZERO,
];
const JITTER_GLYPH: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::JitterGlyphTransform),
    target(false),
    AMP_2PX,
    DIRECTION_VERTICAL,
    SEED_ZERO,
];
const JITTER_POST: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::JitterPostProcess),
    target(true),
    AMP_3PX,
    PERIOD_64PX,
    DIRECTION_HORIZONTAL,
    SEED_ZERO,
];
const ARC_GLYPH: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::ArcGlyphTransform),
    target(false),
    RADIUS_120PX,
    START_ANGLE_ZERO,
    STEP_ANGLE_EIGHT,
];
const ARC_POST: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::ArcPostProcess),
    target(true),
    AMOUNT_0_18,
];
const SPIN_GLYPH: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::SpinGlyphTransform),
    target(false),
    ROTATION_AMP_SIX,
    SPEED_1_NONNEGATIVE,
];
const SPIN_POST: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::SpinPostProcess),
    target(true),
    AMOUNT_0_18,
];
const PULSE_GLYPH: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::PulseGlyphTransform),
    target(false),
    SCALE_AMP_0_08,
    SPEED_1_NONNEGATIVE,
];
const PULSE_POST: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::PulsePostProcess),
    target(true),
    AMOUNT_0_18,
];
const MOTION_GLYPH: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::MotionGlyphTransform),
    target(false),
    MOTION_FUNCTION,
    SPEED_1_NONNEGATIVE,
    AMP_4PX,
    ROTATION_AMP_SIX,
    SCALE_AMP_0_08,
];
const MOTION_POST: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::MotionPostProcess),
    target(true),
    AMOUNT_0_18,
];
const SPARKLE_GLYPH: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::SparkleGlyphTransform),
    target(false),
    AMP_1_6PX,
    SPEED_2_2,
];
const SPARKLE_COLOR: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::SparkleGlyphColor),
    target(false),
    SPEED_2_2,
];
const SPARKLE_POST: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::SparklePostProcess),
    target(true),
    AMOUNT_0_35,
    SEED_ZERO,
];
const TYPEWRITER_MASK: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::TypewriterGlyphMask),
    target(false),
    CPS_28,
    DELAY_ZERO,
    CURSOR_FALSE,
    CURSOR_ALPHA,
];
const SHADER_GLYPH: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::ShaderGlyphColor),
    target(false),
    SHADER_RESOURCE,
    SHADER_AMOUNT,
    SHADER_DIRECTION,
    SHADER_COLOR,
];
const SHADER_OFFSCREEN: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::ShaderOffscreenPass),
    target(false),
    SHADER_RESOURCE,
    SHADER_AMOUNT,
    SHADER_DIRECTION,
    SHADER_COLOR,
];
const SHADER_POST: &[BuiltinFxCallableParameter] = &[
    phase(BuiltinFxCallableRowId::ShaderPostProcess),
    target(true),
    SHADER_RESOURCE,
    SHADER_AMOUNT,
    SHADER_DIRECTION,
    SHADER_COLOR,
];

const fn row(
    id: BuiltinFxCallableRowId,
    parameters: &'static [BuiltinFxCallableParameter],
) -> BuiltinFxCallableRow {
    BuiltinFxCallableRow { id, parameters }
}

const BUILTIN_FX_CALLABLE_ROWS: &[BuiltinFxCallableRow] = &[
    row(BuiltinFxCallableRowId::WaveGlyphTransform, WAVE_GLYPH),
    row(BuiltinFxCallableRowId::WavePostProcess, WAVE_POST),
    row(BuiltinFxCallableRowId::ShakeGlyphTransform, SHAKE_GLYPH),
    row(BuiltinFxCallableRowId::ShakePostProcess, SHAKE_POST),
    row(BuiltinFxCallableRowId::JitterGlyphTransform, JITTER_GLYPH),
    row(BuiltinFxCallableRowId::JitterPostProcess, JITTER_POST),
    row(BuiltinFxCallableRowId::ArcGlyphTransform, ARC_GLYPH),
    row(BuiltinFxCallableRowId::ArcPostProcess, ARC_POST),
    row(BuiltinFxCallableRowId::SpinGlyphTransform, SPIN_GLYPH),
    row(BuiltinFxCallableRowId::SpinPostProcess, SPIN_POST),
    row(BuiltinFxCallableRowId::PulseGlyphTransform, PULSE_GLYPH),
    row(BuiltinFxCallableRowId::PulsePostProcess, PULSE_POST),
    row(BuiltinFxCallableRowId::MotionGlyphTransform, MOTION_GLYPH),
    row(BuiltinFxCallableRowId::MotionPostProcess, MOTION_POST),
    row(BuiltinFxCallableRowId::SparkleGlyphTransform, SPARKLE_GLYPH),
    row(BuiltinFxCallableRowId::SparkleGlyphColor, SPARKLE_COLOR),
    row(BuiltinFxCallableRowId::SparklePostProcess, SPARKLE_POST),
    row(BuiltinFxCallableRowId::TypewriterGlyphMask, TYPEWRITER_MASK),
    row(BuiltinFxCallableRowId::ShaderGlyphColor, SHADER_GLYPH),
    row(
        BuiltinFxCallableRowId::ShaderOffscreenPass,
        SHADER_OFFSCREEN,
    ),
    row(BuiltinFxCallableRowId::ShaderPostProcess, SHADER_POST),
];

/// The one published Arcweft-owned builtin Fx callable catalog.
pub const BUILTIN_FX_CALLABLE_CATALOG: BuiltinFxCallableCatalog =
    BuiltinFxCallableCatalog::new(BUILTIN_FX_CALLABLE_ROWS);

fn encode_row<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    row: BuiltinFxCallableRow,
) -> Result<(), S::Error> {
    encoder.domain_v1(b"arcweft.builtin-fx-callable-schema")?;
    encoder.tag(row.id.semantic_tag())?;
    encode_string(encoder, row.source_name())?;
    encoder.unsigned(
        u64::try_from(row.parameters.len()).expect("builtin Fx parameter count fits u64"),
    )?;
    for parameter in row.parameters {
        encode_parameter(encoder, *parameter)?;
    }
    Ok(())
}

fn encode_string<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    value: &str,
) -> Result<(), S::Error> {
    encoder.unsigned(u64::try_from(value.len()).expect("builtin Fx name length fits u64"))?;
    encoder.raw_bytes(value.as_bytes())
}

fn encode_parameter<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    parameter: BuiltinFxCallableParameter,
) -> Result<(), S::Error> {
    encoder.tag(parameter.id.semantic_tag())?;
    encode_string(encoder, parameter.source_name)?;
    encoder.tag(match parameter.parameter_type {
        BuiltinFxParameterType::Bool => 0,
        BuiltinFxParameterType::Seed32 => 1,
        BuiltinFxParameterType::FixedMilli => 2,
        BuiltinFxParameterType::Ratio => 3,
        BuiltinFxParameterType::Length => 4,
        BuiltinFxParameterType::Angle => 5,
        BuiltinFxParameterType::Duration => 6,
        BuiltinFxParameterType::Color => 7,
        BuiltinFxParameterType::Vec2 => 8,
        BuiltinFxParameterType::Resource => 9,
        BuiltinFxParameterType::Phase => 10,
        BuiltinFxParameterType::Target => 11,
        BuiltinFxParameterType::MotionFunction => 12,
    })?;
    encoder.tag(match parameter.passing {
        BuiltinFxParameterPassing::NamedOnly => 0,
    })?;
    encoder.tag(match parameter.binding {
        BuiltinFxParameterBinding::Structural => 0,
        BuiltinFxParameterBinding::Abi => 1,
    })?;
    encode_presence(encoder, parameter.presence)?;
    encode_constraint(encoder, parameter.constraint)
}

fn encode_presence<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    presence: BuiltinFxParameterPresence,
) -> Result<(), S::Error> {
    match presence {
        BuiltinFxParameterPresence::Required => encoder.tag(0),
        BuiltinFxParameterPresence::Optional => encoder.tag(1),
        BuiltinFxParameterPresence::Defaulted(value) => {
            encoder.tag(2)?;
            encode_default(encoder, value)
        }
        BuiltinFxParameterPresence::Conditional { predicate, default } => {
            encoder.tag(3)?;
            match predicate {
                BuiltinFxParameterPredicate::BoolEquals { parameter, value } => {
                    encoder.tag(0)?;
                    encoder.tag(parameter.semantic_tag())?;
                    encoder.boolean(value)?;
                }
            }
            encoder.boolean(default.is_some())?;
            if let Some(value) = default {
                encode_default(encoder, value)?;
            }
            Ok(())
        }
    }
}

fn encode_default<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    value: BuiltinFxDefaultValue,
) -> Result<(), S::Error> {
    match value {
        BuiltinFxDefaultValue::Bool(value) => {
            encoder.tag(0)?;
            encoder.boolean(value)
        }
        BuiltinFxDefaultValue::Milli(value) => encode_tagged_i64(encoder, 1, value),
        BuiltinFxDefaultValue::RatioMilli(value) => encode_tagged_i64(encoder, 2, value),
        BuiltinFxDefaultValue::LengthMilliPx(value) => encode_tagged_i64(encoder, 3, value),
        BuiltinFxDefaultValue::AngleMilliDegrees(value) => encode_tagged_i64(encoder, 4, value),
        BuiltinFxDefaultValue::DurationMillis(value) => encode_tagged_i64(encoder, 5, value),
        BuiltinFxDefaultValue::Seed32(value) => {
            encoder.tag(6)?;
            encoder.unsigned(u64::from(value))
        }
        BuiltinFxDefaultValue::Vec2Milli(value) => {
            encoder.tag(7)?;
            encode_signed_i64(encoder, value[0])?;
            encode_signed_i64(encoder, value[1])
        }
        BuiltinFxDefaultValue::Phase(value) => {
            encoder.tag(8)?;
            encoder.unsigned(u64::from(value.tag()))
        }
        BuiltinFxDefaultValue::Target(value) => {
            encoder.tag(9)?;
            encoder.unsigned(u64::from(value.tag()))
        }
        BuiltinFxDefaultValue::MotionFunction(value) => {
            encoder.tag(10)?;
            encoder.unsigned(u64::from(value.tag()))
        }
    }
}

fn encode_constraint<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    constraint: BuiltinFxValueConstraint,
) -> Result<(), S::Error> {
    match constraint {
        BuiltinFxValueConstraint::None => encoder.tag(0),
        BuiltinFxValueConstraint::Numeric(value) => {
            encoder.tag(1)?;
            encode_signed_i64(encoder, value.inclusive_min_milli)?;
            encode_signed_i64(encoder, value.inclusive_max_milli)?;
            encoder.unsigned(u64::from(value.max_integer_digits))?;
            encoder.unsigned(u64::from(value.max_fraction_digits))?;
            encoder.unsigned(
                u64::try_from(value.units.len()).expect("builtin Fx unit count fits u64"),
            )?;
            for unit in value.units {
                encoder.tag(unit_tag(*unit))?;
            }
            Ok(())
        }
        BuiltinFxValueConstraint::ExactPhase(value) => {
            encoder.tag(2)?;
            encoder.unsigned(u64::from(value.tag()))
        }
        BuiltinFxValueConstraint::AllowedTargets(values) => {
            encoder.tag(3)?;
            encoder
                .unsigned(u64::try_from(values.len()).expect("builtin Fx target count fits u64"))?;
            for value in values {
                encoder.unsigned(u64::from(value.tag()))?;
            }
            Ok(())
        }
    }
}

fn encode_tagged_i64<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    tag: u8,
    value: i64,
) -> Result<(), S::Error> {
    encoder.tag(tag)?;
    encode_signed_i64(encoder, value)
}

fn encode_signed_i64<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    value: i64,
) -> Result<(), S::Error> {
    let magnitude = value.unsigned_abs();
    let zigzag = if value < 0 {
        magnitude * 2 - 1
    } else {
        magnitude * 2
    };
    encoder.unsigned(zigzag)
}

const fn unit_tag(value: BuiltinFxUnit) -> u8 {
    match value {
        BuiltinFxUnit::Unitless => 0,
        BuiltinFxUnit::Px => 1,
        BuiltinFxUnit::Deg => 2,
        BuiltinFxUnit::Ms => 3,
        BuiltinFxUnit::S => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn catalog_is_complete_and_rows_have_unique_digests() {
        assert_eq!(BUILTIN_FX_CALLABLE_SCHEMA_VERSION, 1);
        assert_eq!(BUILTIN_FX_CALLABLE_CATALOG.len(), 21);
        assert_eq!(
            BUILTIN_FX_CALLABLE_CATALOG
                .iter()
                .map(BuiltinFxCallableRow::id)
                .collect::<Vec<_>>(),
            BuiltinFxCallableRowId::ALL,
        );
        assert_eq!(
            BUILTIN_FX_CALLABLE_CATALOG
                .iter()
                .map(BuiltinFxCallableRow::schema_digest)
                .collect::<BTreeSet<_>>()
                .len(),
            BUILTIN_FX_CALLABLE_CATALOG.len(),
        );
    }

    #[test]
    fn public_names_have_no_legacy_aliases() {
        for parameter in BuiltinFxParameterId::ALL {
            assert_eq!(
                BuiltinFxParameterId::from_source_name(parameter.source_name()),
                Some(parameter)
            );
        }
        for removed in [
            "amp",
            "dir",
            "angle",
            "scale_amp",
            "cps",
            "start",
            "step",
            "id",
            "fn",
            "cursor_opacity",
        ] {
            assert_eq!(BuiltinFxParameterId::from_source_name(removed), None);
        }
    }

    #[test]
    fn every_callable_has_one_default_phase_row() {
        for callable in BuiltinFxCallableId::ALL {
            let rows = BUILTIN_FX_CALLABLE_CATALOG
                .rows_for(callable)
                .collect::<Vec<_>>();
            assert!(!rows.is_empty());
            assert_eq!(
                rows.iter()
                    .filter(|row| row.id().is_default_phase())
                    .count(),
                1
            );
        }
    }

    #[test]
    fn every_callable_owns_contiguous_local_overload_coordinates() {
        for callable in BuiltinFxCallableId::ALL {
            assert_eq!(
                BUILTIN_FX_CALLABLE_CATALOG
                    .rows_for(callable)
                    .map(|row| row.id().callable_overload_ordinal())
                    .collect::<Vec<_>>(),
                (0..BUILTIN_FX_CALLABLE_CATALOG.rows_for(callable).count())
                    .map(|ordinal| u8::try_from(ordinal).unwrap())
                    .collect::<Vec<_>>(),
            );
        }
    }

    #[test]
    fn pulse_speed_and_shader_color_are_typed_abi_parameters() {
        let pulse = BUILTIN_FX_CALLABLE_CATALOG
            .get(BuiltinFxCallableRowId::PulseGlyphTransform)
            .unwrap();
        assert!(pulse.parameter(BuiltinFxParameterId::Speed).is_some());
        let shader = BUILTIN_FX_CALLABLE_CATALOG
            .get(BuiltinFxCallableRowId::ShaderPostProcess)
            .unwrap();
        assert_eq!(
            shader
                .parameter(BuiltinFxParameterId::Color)
                .unwrap()
                .presence(),
            BuiltinFxParameterPresence::Optional
        );
    }
}
