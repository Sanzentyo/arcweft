use arcweft_rich_text_schema::{
    Multiplicity, PropertyPresence, RichTextDefaultValue, RichTextEnumSchemaId,
    RichTextNumericLimits, RichTextPropertyPredicate, RichTextPropertySpec, RichTextUnit,
    RichTextValueKind, RichTextValueLimits,
};

use super::{BuiltinRichTextFx, BuiltinRichTextFxPhase, BuiltinRichTextFxProperty};

/// Result of asking an Arcweft-owned effect for one phase-specific property schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinPropertyDisposition {
    /// This phase accepts the property with the returned exact schema.
    Accepted(&'static RichTextPropertySpec<BuiltinRichTextFxProperty>),
    /// The property is canonical, but the selected effect/phase does not consume it.
    UnsupportedInPhase,
}

const PHASE_ENUM: RichTextEnumSchemaId = RichTextEnumSchemaId::new("rich_text.fx.phase");
const TARGET_ENUM: RichTextEnumSchemaId = RichTextEnumSchemaId::new("rich_text.fx.target");
const MOTION_FN_ENUM: RichTextEnumSchemaId = RichTextEnumSchemaId::new("rich_text.fx.motion.fn");

const TRANSFORM_PHASE_NAMES: &[&str] = &["glyph_transform", "post_process"];
const SPARKLE_PHASE_NAMES: &[&str] = &["glyph_transform", "glyph_color", "post_process"];
const TYPEWRITER_PHASE_NAMES: &[&str] = &["glyph_mask"];
const SHADER_PHASE_NAMES: &[&str] = &["glyph_color", "run_offscreen_pass", "post_process"];
const CONTENT_TARGET_NAMES: &[&str] = &["content", "line", "glyph"];
const VIEWPORT_TARGET_NAMES: &[&str] = &["viewport"];
const MOTION_FUNCTION_NAMES: &[&str] = &["breath_orbit", "elastic_bloom"];

const UNITLESS: &[RichTextUnit] = &[RichTextUnit::Unitless];
const PX: &[RichTextUnit] = &[RichTextUnit::Px];
const DEG: &[RichTextUnit] = &[RichTextUnit::Deg];
const DURATION: &[RichTextUnit] = &[RichTextUnit::Ms, RichTextUnit::S];

const TRANSFORM_PHASE: RichTextPropertySpec<BuiltinRichTextFxProperty> = enum_property(
    BuiltinRichTextFxProperty::Phase,
    PHASE_ENUM,
    TRANSFORM_PHASE_NAMES,
    PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(0)),
);
const SPARKLE_PHASE: RichTextPropertySpec<BuiltinRichTextFxProperty> = enum_property(
    BuiltinRichTextFxProperty::Phase,
    PHASE_ENUM,
    SPARKLE_PHASE_NAMES,
    PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(0)),
);
const TYPEWRITER_PHASE: RichTextPropertySpec<BuiltinRichTextFxProperty> = enum_property(
    BuiltinRichTextFxProperty::Phase,
    PHASE_ENUM,
    TYPEWRITER_PHASE_NAMES,
    PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(0)),
);
const SHADER_PHASE: RichTextPropertySpec<BuiltinRichTextFxProperty> = enum_property(
    BuiltinRichTextFxProperty::Phase,
    PHASE_ENUM,
    SHADER_PHASE_NAMES,
    PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(1)),
);
const CONTENT_TARGET: RichTextPropertySpec<BuiltinRichTextFxProperty> = enum_property(
    BuiltinRichTextFxProperty::Target,
    TARGET_ENUM,
    CONTENT_TARGET_NAMES,
    PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(0)),
);
const VIEWPORT_TARGET: RichTextPropertySpec<BuiltinRichTextFxProperty> = enum_property(
    BuiltinRichTextFxProperty::Target,
    TARGET_ENUM,
    VIEWPORT_TARGET_NAMES,
    PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(0)),
);

const AMP_4PX: RichTextPropertySpec<BuiltinRichTextFxProperty> = length(
    BuiltinRichTextFxProperty::Amp,
    RichTextDefaultValue::Length {
        milli: 4_000,
        unit: RichTextUnit::Px,
    },
    0,
    4_096_000,
);
const AMP_3PX: RichTextPropertySpec<BuiltinRichTextFxProperty> = length(
    BuiltinRichTextFxProperty::Amp,
    RichTextDefaultValue::Length {
        milli: 3_000,
        unit: RichTextUnit::Px,
    },
    0,
    4_096_000,
);
const AMP_2PX: RichTextPropertySpec<BuiltinRichTextFxProperty> = length(
    BuiltinRichTextFxProperty::Amp,
    RichTextDefaultValue::Length {
        milli: 2_000,
        unit: RichTextUnit::Px,
    },
    0,
    4_096_000,
);
const AMP_1_6PX: RichTextPropertySpec<BuiltinRichTextFxProperty> = length(
    BuiltinRichTextFxProperty::Amp,
    RichTextDefaultValue::Length {
        milli: 1_600,
        unit: RichTextUnit::Px,
    },
    0,
    4_096_000,
);
const AMP_0_08: RichTextPropertySpec<BuiltinRichTextFxProperty> = fixed(
    BuiltinRichTextFxProperty::Amp,
    RichTextDefaultValue::Milli(80),
    0,
    10_000,
);
const PERIOD_12: RichTextPropertySpec<BuiltinRichTextFxProperty> = fixed(
    BuiltinRichTextFxProperty::Period,
    RichTextDefaultValue::Milli(12_000),
    1,
    65_536_000,
);
const PERIOD_64PX: RichTextPropertySpec<BuiltinRichTextFxProperty> = length(
    BuiltinRichTextFxProperty::Period,
    RichTextDefaultValue::Length {
        milli: 64_000,
        unit: RichTextUnit::Px,
    },
    1,
    65_536_000,
);
const SPEED_1_POSITIVE: RichTextPropertySpec<BuiltinRichTextFxProperty> = fixed(
    BuiltinRichTextFxProperty::Speed,
    RichTextDefaultValue::Milli(1_000),
    1,
    1_000_000,
);
const SPEED_1_NONNEGATIVE: RichTextPropertySpec<BuiltinRichTextFxProperty> = fixed(
    BuiltinRichTextFxProperty::Speed,
    RichTextDefaultValue::Milli(1_000),
    0,
    1_000_000,
);
const SPEED_16: RichTextPropertySpec<BuiltinRichTextFxProperty> = fixed(
    BuiltinRichTextFxProperty::Speed,
    RichTextDefaultValue::Milli(16_000),
    1,
    1_000_000,
);
const SPEED_2_2: RichTextPropertySpec<BuiltinRichTextFxProperty> = fixed(
    BuiltinRichTextFxProperty::Speed,
    RichTextDefaultValue::Milli(2_200),
    1,
    1_000_000,
);
const DIR_VERTICAL: RichTextPropertySpec<BuiltinRichTextFxProperty> = vec2(
    BuiltinRichTextFxProperty::Dir,
    RichTextDefaultValue::Vec2Milli([0, 1_000]),
);
const DIR_HORIZONTAL: RichTextPropertySpec<BuiltinRichTextFxProperty> = vec2(
    BuiltinRichTextFxProperty::Dir,
    RichTextDefaultValue::Vec2Milli([1_000, 0]),
);
const SEED_ZERO: RichTextPropertySpec<BuiltinRichTextFxProperty> = scalar(
    BuiltinRichTextFxProperty::Seed,
    RichTextValueKind::Seed32,
    PropertyPresence::Defaulted(RichTextDefaultValue::Seed32(0)),
    &[],
);
const RADIUS_120PX: RichTextPropertySpec<BuiltinRichTextFxProperty> = length(
    BuiltinRichTextFxProperty::Radius,
    RichTextDefaultValue::Length {
        milli: 120_000,
        unit: RichTextUnit::Px,
    },
    0,
    65_536_000,
);
const START_ZERO: RichTextPropertySpec<BuiltinRichTextFxProperty> = angle(
    BuiltinRichTextFxProperty::Start,
    RichTextDefaultValue::AngleMilliDegrees(0),
);
const STEP_EIGHT: RichTextPropertySpec<BuiltinRichTextFxProperty> = angle(
    BuiltinRichTextFxProperty::Step,
    RichTextDefaultValue::AngleMilliDegrees(8_000),
);
const ANGLE_SIX: RichTextPropertySpec<BuiltinRichTextFxProperty> = angle(
    BuiltinRichTextFxProperty::Angle,
    RichTextDefaultValue::AngleMilliDegrees(6_000),
);
const AMOUNT_0_18: RichTextPropertySpec<BuiltinRichTextFxProperty> = ratio(
    BuiltinRichTextFxProperty::Amount,
    PropertyPresence::Defaulted(RichTextDefaultValue::RatioMilli(180)),
);
const AMOUNT_0_35: RichTextPropertySpec<BuiltinRichTextFxProperty> = ratio(
    BuiltinRichTextFxProperty::Amount,
    PropertyPresence::Defaulted(RichTextDefaultValue::RatioMilli(350)),
);
const MOTION_FUNCTION: RichTextPropertySpec<BuiltinRichTextFxProperty> = enum_property(
    BuiltinRichTextFxProperty::Function,
    MOTION_FN_ENUM,
    MOTION_FUNCTION_NAMES,
    PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(0)),
);
const SCALE_AMP_0_08: RichTextPropertySpec<BuiltinRichTextFxProperty> = fixed(
    BuiltinRichTextFxProperty::ScaleAmp,
    RichTextDefaultValue::Milli(80),
    0,
    10_000,
);
const CPS_28: RichTextPropertySpec<BuiltinRichTextFxProperty> = fixed(
    BuiltinRichTextFxProperty::Cps,
    RichTextDefaultValue::Milli(28_000),
    1_000,
    240_000,
);
const DELAY_ZERO: RichTextPropertySpec<BuiltinRichTextFxProperty> = duration(
    BuiltinRichTextFxProperty::Delay,
    RichTextDefaultValue::DurationMillis(0),
);
const CURSOR_FALSE: RichTextPropertySpec<BuiltinRichTextFxProperty> = scalar(
    BuiltinRichTextFxProperty::Cursor,
    RichTextValueKind::Bool,
    PropertyPresence::Defaulted(RichTextDefaultValue::Bool(false)),
    &["false", "true"],
);
const CURSOR_ALPHA: RichTextPropertySpec<BuiltinRichTextFxProperty> = RichTextPropertySpec {
    id: BuiltinRichTextFxProperty::CursorAlpha,
    source_name: "cursor_alpha",
    kind: RichTextValueKind::Ratio,
    presence: PropertyPresence::Conditional {
        predicate: RichTextPropertyPredicate::BoolEquals {
            property: BuiltinRichTextFxProperty::Cursor,
            value: true,
        },
    },
    multiplicity: Multiplicity::Single,
    limits: numeric_limits(0, 1_000, UNITLESS),
    allow_empty: false,
};
const SHADER_ID: RichTextPropertySpec<BuiltinRichTextFxProperty> = scalar(
    BuiltinRichTextFxProperty::Id,
    RichTextValueKind::PublicId,
    PropertyPresence::Required,
    &[],
);
const SHADER_AMOUNT: RichTextPropertySpec<BuiltinRichTextFxProperty> = ratio(
    BuiltinRichTextFxProperty::Amount,
    PropertyPresence::Optional,
);
const SHADER_DIR: RichTextPropertySpec<BuiltinRichTextFxProperty> = RichTextPropertySpec {
    id: BuiltinRichTextFxProperty::Dir,
    source_name: "dir",
    kind: RichTextValueKind::Vec2,
    presence: PropertyPresence::Optional,
    multiplicity: Multiplicity::Single,
    limits: numeric_limits(-1_000_000, 1_000_000, &[]),
    allow_empty: false,
};
const SHADER_COLOR: RichTextPropertySpec<BuiltinRichTextFxProperty> = scalar(
    BuiltinRichTextFxProperty::Color,
    RichTextValueKind::Color,
    PropertyPresence::Optional,
    &[],
);

const WAVE_GLYPH: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Amp,
    BuiltinRichTextFxProperty::Period,
    BuiltinRichTextFxProperty::Speed,
    BuiltinRichTextFxProperty::Dir,
];
const WAVE_POST: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Amp,
    BuiltinRichTextFxProperty::Period,
    BuiltinRichTextFxProperty::Speed,
    BuiltinRichTextFxProperty::Dir,
    BuiltinRichTextFxProperty::Seed,
];
const SHAKE_GLYPH: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Amp,
    BuiltinRichTextFxProperty::Speed,
];
const JITTER_GLYPH: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Amp,
];
const JITTER_POST: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Amp,
    BuiltinRichTextFxProperty::Period,
    BuiltinRichTextFxProperty::Dir,
    BuiltinRichTextFxProperty::Seed,
];
const ARC_GLYPH: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Radius,
    BuiltinRichTextFxProperty::Start,
    BuiltinRichTextFxProperty::Step,
];
const POST_AMOUNT: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Amount,
];
const SPIN_GLYPH: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Angle,
    BuiltinRichTextFxProperty::Speed,
];
const PULSE_GLYPH: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Amp,
];
const MOTION_GLYPH: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Function,
    BuiltinRichTextFxProperty::Speed,
    BuiltinRichTextFxProperty::Amp,
    BuiltinRichTextFxProperty::Angle,
    BuiltinRichTextFxProperty::ScaleAmp,
];
const TYPEWRITER_MASK: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Cps,
    BuiltinRichTextFxProperty::Delay,
    BuiltinRichTextFxProperty::Cursor,
    BuiltinRichTextFxProperty::CursorAlpha,
];
const SPARKLE_GLYPH: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Amp,
    BuiltinRichTextFxProperty::Speed,
];
const SPARKLE_COLOR: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Speed,
];
const SPARKLE_POST: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Amount,
    BuiltinRichTextFxProperty::Seed,
];
const SHADER_PROPERTIES: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
    BuiltinRichTextFxProperty::Id,
    BuiltinRichTextFxProperty::Amount,
    BuiltinRichTextFxProperty::Dir,
    BuiltinRichTextFxProperty::Color,
];

impl BuiltinRichTextFx {
    /// Resolves one canonical property through this effect's exact phase schema.
    #[must_use]
    pub const fn property_spec(
        self,
        phase: BuiltinRichTextFxPhase,
        property: BuiltinRichTextFxProperty,
    ) -> BuiltinPropertyDisposition {
        use BuiltinPropertyDisposition::{Accepted, UnsupportedInPhase};
        use BuiltinRichTextFxPhase::{
            GlyphColor, GlyphMask, GlyphTransform, OffscreenPass, PostProcess,
        };
        use BuiltinRichTextFxProperty::{
            Amount, Amp, Angle, Color, Cps, Cursor, CursorAlpha, Delay, Dir, Function, Id, Period,
            Phase, Radius, ScaleAmp, Seed, Speed, Start, Step, Target,
        };
        let schema = match (self, phase, property) {
            (
                Self::Wave
                | Self::Shake
                | Self::Jitter
                | Self::Arc
                | Self::Spin
                | Self::Pulse
                | Self::Motion,
                GlyphTransform | PostProcess,
                Phase,
            ) => &TRANSFORM_PHASE,
            (Self::Sparkle, GlyphTransform | GlyphColor | PostProcess, Phase) => &SPARKLE_PHASE,
            (Self::Typewriter, GlyphMask, Phase) => &TYPEWRITER_PHASE,
            (Self::Shader, GlyphColor | OffscreenPass | PostProcess, Phase) => &SHADER_PHASE,
            (
                Self::Wave
                | Self::Shake
                | Self::Jitter
                | Self::Arc
                | Self::Spin
                | Self::Pulse
                | Self::Motion
                | Self::Sparkle
                | Self::Shader,
                PostProcess,
                Target,
            ) => &VIEWPORT_TARGET,
            (
                Self::Wave
                | Self::Shake
                | Self::Jitter
                | Self::Arc
                | Self::Spin
                | Self::Pulse
                | Self::Motion,
                GlyphTransform,
                Target,
            )
            | (Self::Sparkle, GlyphTransform | GlyphColor, Target)
            | (Self::Typewriter, GlyphMask, Target)
            | (Self::Shader, GlyphColor | OffscreenPass, Target) => &CONTENT_TARGET,
            (Self::Wave | Self::Motion, GlyphTransform, Amp) => &AMP_4PX,
            (Self::Wave, GlyphTransform, Period) => &PERIOD_12,
            (Self::Wave, GlyphTransform, Speed) => &SPEED_1_POSITIVE,
            (Self::Wave, GlyphTransform, Dir) => &DIR_VERTICAL,
            (Self::Wave | Self::Shake | Self::Jitter, PostProcess, Amp) => &AMP_3PX,
            (Self::Wave | Self::Shake | Self::Jitter, PostProcess, Period) => &PERIOD_64PX,
            (Self::Wave | Self::Shake, PostProcess, Speed)
            | (Self::Spin | Self::Motion, GlyphTransform, Speed) => &SPEED_1_NONNEGATIVE,
            (Self::Wave | Self::Shake | Self::Jitter, PostProcess, Dir) => &DIR_HORIZONTAL,
            (Self::Wave | Self::Shake | Self::Jitter | Self::Sparkle, PostProcess, Seed) => {
                &SEED_ZERO
            }
            (Self::Shake | Self::Jitter, GlyphTransform, Amp) => &AMP_2PX,
            (Self::Shake, GlyphTransform, Speed) => &SPEED_16,
            (Self::Arc, GlyphTransform, Radius) => &RADIUS_120PX,
            (Self::Arc, GlyphTransform, Start) => &START_ZERO,
            (Self::Arc, GlyphTransform, Step) => &STEP_EIGHT,
            (Self::Arc | Self::Spin | Self::Pulse | Self::Motion, PostProcess, Amount) => {
                &AMOUNT_0_18
            }
            (Self::Spin | Self::Motion, GlyphTransform, Angle) => &ANGLE_SIX,
            (Self::Pulse, GlyphTransform, Amp) => &AMP_0_08,
            (Self::Motion, GlyphTransform, Function) => &MOTION_FUNCTION,
            (Self::Motion, GlyphTransform, ScaleAmp) => &SCALE_AMP_0_08,
            (Self::Typewriter, GlyphMask, Cps) => &CPS_28,
            (Self::Typewriter, GlyphMask, Delay) => &DELAY_ZERO,
            (Self::Typewriter, GlyphMask, Cursor) => &CURSOR_FALSE,
            (Self::Typewriter, GlyphMask, CursorAlpha) => &CURSOR_ALPHA,
            (Self::Sparkle, GlyphTransform, Amp) => &AMP_1_6PX,
            (Self::Sparkle, GlyphTransform | GlyphColor, Speed) => &SPEED_2_2,
            (Self::Sparkle, PostProcess, Amount) => &AMOUNT_0_35,
            (Self::Shader, GlyphColor | OffscreenPass | PostProcess, Id) => &SHADER_ID,
            (Self::Shader, GlyphColor | OffscreenPass | PostProcess, Amount) => &SHADER_AMOUNT,
            (Self::Shader, GlyphColor | OffscreenPass | PostProcess, Dir) => &SHADER_DIR,
            (Self::Shader, GlyphColor | OffscreenPass | PostProcess, Color) => &SHADER_COLOR,
            _ => return UnsupportedInPhase,
        };
        Accepted(schema)
    }

    /// Canonical property order for this effect and phase.
    #[must_use]
    pub const fn properties_for_phase(
        self,
        phase: BuiltinRichTextFxPhase,
    ) -> &'static [BuiltinRichTextFxProperty] {
        use BuiltinRichTextFxPhase::{
            GlyphColor, GlyphMask, GlyphTransform, OffscreenPass, PostProcess,
        };
        match (self, phase) {
            (Self::Wave, GlyphTransform) => WAVE_GLYPH,
            (Self::Wave | Self::Shake, PostProcess) => WAVE_POST,
            (Self::Shake, GlyphTransform) => SHAKE_GLYPH,
            (Self::Jitter, GlyphTransform) => JITTER_GLYPH,
            (Self::Jitter, PostProcess) => JITTER_POST,
            (Self::Arc, GlyphTransform) => ARC_GLYPH,
            (Self::Arc | Self::Spin | Self::Pulse | Self::Motion, PostProcess) => POST_AMOUNT,
            (Self::Spin, GlyphTransform) => SPIN_GLYPH,
            (Self::Pulse, GlyphTransform) => PULSE_GLYPH,
            (Self::Motion, GlyphTransform) => MOTION_GLYPH,
            (Self::Typewriter, GlyphMask) => TYPEWRITER_MASK,
            (Self::Sparkle, GlyphTransform) => SPARKLE_GLYPH,
            (Self::Sparkle, GlyphColor) => SPARKLE_COLOR,
            (Self::Sparkle, PostProcess) => SPARKLE_POST,
            (Self::Shader, GlyphColor | OffscreenPass | PostProcess) => SHADER_PROPERTIES,
            _ => &[],
        }
    }

    /// Returns the absence-only default for a conditional property.
    ///
    /// Conditional presence and the value to materialize are separate schema
    /// concerns. Keeping the value on the original effect owner lets semantic
    /// checking stay generic without duplicating effect or property identities.
    #[must_use]
    pub const fn conditional_default(
        self,
        phase: BuiltinRichTextFxPhase,
        property: BuiltinRichTextFxProperty,
    ) -> Option<RichTextDefaultValue> {
        match (self, phase, property) {
            (
                Self::Typewriter,
                BuiltinRichTextFxPhase::GlyphMask,
                BuiltinRichTextFxProperty::CursorAlpha,
            ) => Some(RichTextDefaultValue::RatioMilli(350)),
            _ => None,
        }
    }
}

const fn enum_property(
    property: BuiltinRichTextFxProperty,
    enum_id: RichTextEnumSchemaId,
    values: &'static [&'static str],
    presence: PropertyPresence<BuiltinRichTextFxProperty>,
) -> RichTextPropertySpec<BuiltinRichTextFxProperty> {
    scalar(
        property,
        RichTextValueKind::ClosedEnum(enum_id),
        presence,
        values,
    )
}

const fn length(
    property: BuiltinRichTextFxProperty,
    default: RichTextDefaultValue,
    minimum: i64,
    maximum: i64,
) -> RichTextPropertySpec<BuiltinRichTextFxProperty> {
    RichTextPropertySpec {
        id: property,
        source_name: property.source_name(),
        kind: RichTextValueKind::Length,
        presence: PropertyPresence::Defaulted(default),
        multiplicity: Multiplicity::Single,
        limits: numeric_limits(minimum, maximum, PX),
        allow_empty: false,
    }
}

const fn fixed(
    property: BuiltinRichTextFxProperty,
    default: RichTextDefaultValue,
    minimum: i64,
    maximum: i64,
) -> RichTextPropertySpec<BuiltinRichTextFxProperty> {
    RichTextPropertySpec {
        id: property,
        source_name: property.source_name(),
        kind: RichTextValueKind::FixedMilli,
        presence: PropertyPresence::Defaulted(default),
        multiplicity: Multiplicity::Single,
        limits: numeric_limits(minimum, maximum, UNITLESS),
        allow_empty: false,
    }
}

const fn ratio(
    property: BuiltinRichTextFxProperty,
    presence: PropertyPresence<BuiltinRichTextFxProperty>,
) -> RichTextPropertySpec<BuiltinRichTextFxProperty> {
    RichTextPropertySpec {
        id: property,
        source_name: property.source_name(),
        kind: RichTextValueKind::Ratio,
        presence,
        multiplicity: Multiplicity::Single,
        limits: numeric_limits(0, 1_000, UNITLESS),
        allow_empty: false,
    }
}

const fn angle(
    property: BuiltinRichTextFxProperty,
    default: RichTextDefaultValue,
) -> RichTextPropertySpec<BuiltinRichTextFxProperty> {
    RichTextPropertySpec {
        id: property,
        source_name: property.source_name(),
        kind: RichTextValueKind::Angle,
        presence: PropertyPresence::Defaulted(default),
        multiplicity: Multiplicity::Single,
        limits: numeric_limits(-360_000_000, 360_000_000, DEG),
        allow_empty: false,
    }
}

const fn vec2(
    property: BuiltinRichTextFxProperty,
    default: RichTextDefaultValue,
) -> RichTextPropertySpec<BuiltinRichTextFxProperty> {
    RichTextPropertySpec {
        id: property,
        source_name: property.source_name(),
        kind: RichTextValueKind::Vec2,
        presence: PropertyPresence::Defaulted(default),
        multiplicity: Multiplicity::Single,
        limits: numeric_limits(-1_000_000, 1_000_000, &[]),
        allow_empty: false,
    }
}

const fn duration(
    property: BuiltinRichTextFxProperty,
    default: RichTextDefaultValue,
) -> RichTextPropertySpec<BuiltinRichTextFxProperty> {
    RichTextPropertySpec {
        id: property,
        source_name: property.source_name(),
        kind: RichTextValueKind::Duration,
        presence: PropertyPresence::Defaulted(default),
        multiplicity: Multiplicity::Single,
        limits: numeric_limits(0, 86_400_000_000, DURATION),
        allow_empty: false,
    }
}

const fn scalar(
    property: BuiltinRichTextFxProperty,
    kind: RichTextValueKind,
    presence: PropertyPresence<BuiltinRichTextFxProperty>,
    enum_values: &'static [&'static str],
) -> RichTextPropertySpec<BuiltinRichTextFxProperty> {
    RichTextPropertySpec {
        id: property,
        source_name: property.source_name(),
        kind,
        presence,
        multiplicity: Multiplicity::Single,
        limits: RichTextValueLimits {
            numeric: None,
            units: &[],
            enum_values,
            max_encoded_bytes: 4_096,
            max_decoded_bytes: 4_096,
        },
        allow_empty: false,
    }
}

const fn numeric_limits(
    minimum: i64,
    maximum: i64,
    units: &'static [RichTextUnit],
) -> RichTextValueLimits {
    RichTextValueLimits {
        numeric: Some(RichTextNumericLimits {
            inclusive_min_milli: Some(minimum),
            inclusive_max_milli: Some(maximum),
            max_integer_digits: 19,
            max_fraction_digits: 3,
        }),
        units,
        enum_values: &[],
        max_encoded_bytes: 64,
        max_decoded_bytes: 64,
    }
}
