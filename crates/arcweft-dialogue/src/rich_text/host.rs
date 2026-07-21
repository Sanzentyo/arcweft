use arcweft_rich_text_schema::{
    CheckedOutputKind, Multiplicity, PropertyPresence, RichTextDefaultValue, RichTextNumericLimits,
    RichTextPropertySpec, RichTextSourceForm, RichTextTagSchema, RichTextUnit, RichTextValueKind,
    RichTextValueLimits, SelectorContract, UnknownPropertyPolicy,
};

/// Closed dialogue-owned host-event authoring inventory.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DialogueHostEventKind {
    /// Selects voice playback source.
    Voice,
    /// Selects a face expression.
    Face,
    /// Selects a pose.
    Pose,
    /// Shows an entity.
    Show,
    /// Hides an entity.
    Hide,
    /// Moves the presentation target.
    Move,
    /// Scales the presentation target.
    Scale,
    /// Rotates the presentation target.
    Rotate,
    /// Starts an animation.
    Animation,
    /// Emits a host-owned shake event.
    Shake,
    /// Schedules a dialogue-safe call at a line-relative time.
    TimedCue,
    /// Invokes a dialogue-safe callable.
    Call,
    /// Emits a typed signal identity.
    Signal,
    /// Opens a pure Boolean conditional span.
    ConditionalStart,
    /// Selects the alternate conditional branch.
    ConditionalElse,
    /// Closes a conditional span.
    ConditionalEnd,
}

/// Semantic properties used by dialogue host-event schemas.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DialogueHostProperty {
    /// Voice source identity or the closed `auto` token.
    Source,
    /// Face expression identity.
    Expression,
    /// Pose identity.
    Pose,
    /// Presentation entity identity.
    Entity,
    /// Horizontal component.
    X,
    /// Vertical component.
    Y,
    /// Rotation angle.
    Angle,
    /// Animation identity.
    Animation,
    /// Shake amplitude.
    Amp,
    /// Timed-cue offset represented by the required positional value.
    At,
    /// Dialogue-safe call payload owned by the dedicated call grammar.
    Call,
    /// Signal identity.
    Signal,
}

impl DialogueHostEventKind {
    /// Deterministic complete host-event inventory.
    pub const ALL: [Self; 16] = [
        Self::Voice,
        Self::Face,
        Self::Pose,
        Self::Show,
        Self::Hide,
        Self::Move,
        Self::Scale,
        Self::Rotate,
        Self::Animation,
        Self::Shake,
        Self::TimedCue,
        Self::Call,
        Self::Signal,
        Self::ConditionalStart,
        Self::ConditionalElse,
        Self::ConditionalEnd,
    ];

    /// Resolves a current grammar-owned source spelling.
    #[must_use]
    pub const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"voice" => Some(Self::Voice),
            b"face" => Some(Self::Face),
            b"pose" => Some(Self::Pose),
            b"show" => Some(Self::Show),
            b"hide" => Some(Self::Hide),
            b"move" => Some(Self::Move),
            b"scale" => Some(Self::Scale),
            b"rotate" => Some(Self::Rotate),
            b"anim" => Some(Self::Animation),
            b"shake" => Some(Self::Shake),
            b"at" => Some(Self::TimedCue),
            b"call" | b"!" => Some(Self::Call),
            b"signal" => Some(Self::Signal),
            b"if" => Some(Self::ConditionalStart),
            b"else" => Some(Self::ConditionalElse),
            b"endif" => Some(Self::ConditionalEnd),
            _ => None,
        }
    }

    /// Canonical formatter spelling for this host event.
    #[must_use]
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Voice => "voice",
            Self::Face => "face",
            Self::Pose => "pose",
            Self::Show => "show",
            Self::Hide => "hide",
            Self::Move => "move",
            Self::Scale => "scale",
            Self::Rotate => "rotate",
            Self::Animation => "anim",
            Self::Shake => "shake",
            Self::TimedCue => "at",
            Self::Call => "call",
            Self::Signal => "signal",
            Self::ConditionalStart => "if",
            Self::ConditionalElse => "else",
            Self::ConditionalEnd => "endif",
        }
    }

    /// Immutable owner-typed schema for this host event.
    #[must_use]
    pub const fn schema(self) -> &'static RichTextTagSchema<DialogueHostProperty> {
        match self {
            Self::Voice => &VOICE_SCHEMA,
            Self::Face => &FACE_SCHEMA,
            Self::Pose => &POSE_SCHEMA,
            Self::Show => &SHOW_SCHEMA,
            Self::Hide => &HIDE_SCHEMA,
            Self::Move => &MOVE_SCHEMA,
            Self::Scale => &SCALE_SCHEMA,
            Self::Rotate => &ROTATE_SCHEMA,
            Self::Animation => &ANIMATION_SCHEMA,
            Self::Shake => &SHAKE_SCHEMA,
            Self::TimedCue => &TIMED_CUE_SCHEMA,
            Self::Call => &CALL_SCHEMA,
            Self::Signal => &SIGNAL_SCHEMA,
            Self::ConditionalStart => &CONDITIONAL_START_SCHEMA,
            Self::ConditionalElse => &CONDITIONAL_ELSE_SCHEMA,
            Self::ConditionalEnd => &CONDITIONAL_END_SCHEMA,
        }
    }
}

impl DialogueHostProperty {
    /// Deterministic complete host-property inventory.
    pub const ALL: [Self; 12] = [
        Self::Source,
        Self::Expression,
        Self::Pose,
        Self::Entity,
        Self::X,
        Self::Y,
        Self::Angle,
        Self::Animation,
        Self::Amp,
        Self::At,
        Self::Call,
        Self::Signal,
    ];

    /// Canonical source key. Positional-only `At` is an internal semantic name.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Expression => "expression",
            Self::Pose => "pose",
            Self::Entity => "entity",
            Self::X => "x",
            Self::Y => "y",
            Self::Angle => "angle",
            Self::Animation => "animation",
            Self::Amp => "amp",
            Self::At => "at",
            Self::Call => "call",
            Self::Signal => "signal",
        }
    }

    /// Resolves a canonical scalar source key without aliases or normalization.
    ///
    /// `At` is positional-only and `Call` is a dedicated typed payload, so
    /// neither is returned by scalar-property lookup.
    #[must_use]
    pub const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"source" => Some(Self::Source),
            b"expression" => Some(Self::Expression),
            b"pose" => Some(Self::Pose),
            b"entity" => Some(Self::Entity),
            b"x" => Some(Self::X),
            b"y" => Some(Self::Y),
            b"angle" => Some(Self::Angle),
            b"animation" => Some(Self::Animation),
            b"amp" => Some(Self::Amp),
            b"signal" => Some(Self::Signal),
            _ => None,
        }
    }
}

const SINGLE: Multiplicity = Multiplicity::Single;
const NO_PROPERTIES: &[RichTextPropertySpec<DialogueHostProperty>] = &[];
const ID_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: None,
    units: &[],
    enum_values: &[],
    max_encoded_bytes: 4_096,
    max_decoded_bytes: 4_096,
};
const VOICE_SOURCE_LIMITS: RichTextValueLimits = RichTextValueLimits {
    enum_values: &["auto"],
    ..ID_LIMITS
};
const LENGTH_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(-1_000_000_000),
        inclusive_max_milli: Some(1_000_000_000),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Px],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const NONNEGATIVE_LENGTH_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(0),
        inclusive_max_milli: Some(4_096_000),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Px],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const FIXED_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(-1_000_000),
        inclusive_max_milli: Some(1_000_000),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Unitless],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const ANGLE_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(-360_000_000),
        inclusive_max_milli: Some(360_000_000),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Deg],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const CUE_DURATION_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(0),
        inclusive_max_milli: Some(86_400_000_000),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Ms, RichTextUnit::S],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const fn required(
    id: DialogueHostProperty,
    source_name: &'static str,
    kind: RichTextValueKind,
    limits: RichTextValueLimits,
) -> RichTextPropertySpec<DialogueHostProperty> {
    RichTextPropertySpec {
        id,
        source_name,
        kind,
        presence: PropertyPresence::Required,
        multiplicity: SINGLE,
        limits,
        allow_empty: false,
    }
}

const fn defaulted(
    id: DialogueHostProperty,
    source_name: &'static str,
    kind: RichTextValueKind,
    limits: RichTextValueLimits,
    value: RichTextDefaultValue,
) -> RichTextPropertySpec<DialogueHostProperty> {
    RichTextPropertySpec {
        id,
        source_name,
        kind,
        presence: PropertyPresence::Defaulted(value),
        multiplicity: SINGLE,
        limits,
        allow_empty: false,
    }
}

const SOURCE: RichTextPropertySpec<DialogueHostProperty> = required(
    DialogueHostProperty::Source,
    "source",
    RichTextValueKind::PublicId,
    VOICE_SOURCE_LIMITS,
);
const EXPRESSION: RichTextPropertySpec<DialogueHostProperty> = required(
    DialogueHostProperty::Expression,
    "expression",
    RichTextValueKind::PublicId,
    ID_LIMITS,
);
const POSE: RichTextPropertySpec<DialogueHostProperty> = required(
    DialogueHostProperty::Pose,
    "pose",
    RichTextValueKind::PublicId,
    ID_LIMITS,
);
const ENTITY: RichTextPropertySpec<DialogueHostProperty> = required(
    DialogueHostProperty::Entity,
    "entity",
    RichTextValueKind::PublicId,
    ID_LIMITS,
);
const X_DEFAULT_ZERO: RichTextPropertySpec<DialogueHostProperty> = defaulted(
    DialogueHostProperty::X,
    "x",
    RichTextValueKind::Length,
    LENGTH_LIMITS,
    RichTextDefaultValue::Length {
        milli: 0,
        unit: RichTextUnit::Px,
    },
);
const Y_DEFAULT_ZERO: RichTextPropertySpec<DialogueHostProperty> = defaulted(
    DialogueHostProperty::Y,
    "y",
    RichTextValueKind::Length,
    LENGTH_LIMITS,
    RichTextDefaultValue::Length {
        milli: 0,
        unit: RichTextUnit::Px,
    },
);
const SCALE_X: RichTextPropertySpec<DialogueHostProperty> = required(
    DialogueHostProperty::X,
    "x",
    RichTextValueKind::FixedMilli,
    FIXED_LIMITS,
);
const SCALE_Y: RichTextPropertySpec<DialogueHostProperty> = RichTextPropertySpec {
    id: DialogueHostProperty::Y,
    source_name: "y",
    kind: RichTextValueKind::FixedMilli,
    presence: PropertyPresence::Optional,
    multiplicity: SINGLE,
    limits: FIXED_LIMITS,
    allow_empty: false,
};
const ANGLE: RichTextPropertySpec<DialogueHostProperty> = required(
    DialogueHostProperty::Angle,
    "angle",
    RichTextValueKind::Angle,
    ANGLE_LIMITS,
);
const ANIMATION: RichTextPropertySpec<DialogueHostProperty> = required(
    DialogueHostProperty::Animation,
    "animation",
    RichTextValueKind::PublicId,
    ID_LIMITS,
);
const AMP: RichTextPropertySpec<DialogueHostProperty> = required(
    DialogueHostProperty::Amp,
    "amp",
    RichTextValueKind::Length,
    NONNEGATIVE_LENGTH_LIMITS,
);
const AT: RichTextPropertySpec<DialogueHostProperty> = required(
    DialogueHostProperty::At,
    "at",
    RichTextValueKind::Duration,
    CUE_DURATION_LIMITS,
);
const SIGNAL: RichTextPropertySpec<DialogueHostProperty> = required(
    DialogueHostProperty::Signal,
    "signal",
    RichTextValueKind::PublicId,
    ID_LIMITS,
);

const fn host_schema(
    source_forms: &'static [RichTextSourceForm],
    properties: &'static [RichTextPropertySpec<DialogueHostProperty>],
) -> RichTextTagSchema<DialogueHostProperty> {
    RichTextTagSchema {
        source_forms,
        selector: SelectorContract::None,
        properties,
        unknown_policy: UnknownPropertyPolicy::Reject,
        output: CheckedOutputKind::Host,
    }
}

const VOICE_SCHEMA: RichTextTagSchema<DialogueHostProperty> =
    host_schema(&[RichTextSourceForm::CanonicalTag("voice")], &[SOURCE]);
const FACE_SCHEMA: RichTextTagSchema<DialogueHostProperty> =
    host_schema(&[RichTextSourceForm::CanonicalTag("face")], &[EXPRESSION]);
const POSE_SCHEMA: RichTextTagSchema<DialogueHostProperty> =
    host_schema(&[RichTextSourceForm::CanonicalTag("pose")], &[POSE]);
const SHOW_SCHEMA: RichTextTagSchema<DialogueHostProperty> =
    host_schema(&[RichTextSourceForm::CanonicalTag("show")], &[ENTITY]);
const HIDE_SCHEMA: RichTextTagSchema<DialogueHostProperty> =
    host_schema(&[RichTextSourceForm::CanonicalTag("hide")], &[ENTITY]);
const MOVE_SCHEMA: RichTextTagSchema<DialogueHostProperty> = host_schema(
    &[RichTextSourceForm::CanonicalTag("move")],
    &[X_DEFAULT_ZERO, Y_DEFAULT_ZERO],
);
const SCALE_SCHEMA: RichTextTagSchema<DialogueHostProperty> = host_schema(
    &[RichTextSourceForm::CanonicalTag("scale")],
    &[SCALE_X, SCALE_Y],
);
const ROTATE_SCHEMA: RichTextTagSchema<DialogueHostProperty> =
    host_schema(&[RichTextSourceForm::CanonicalTag("rotate")], &[ANGLE]);
const ANIMATION_SCHEMA: RichTextTagSchema<DialogueHostProperty> =
    host_schema(&[RichTextSourceForm::CanonicalTag("anim")], &[ANIMATION]);
const SHAKE_SCHEMA: RichTextTagSchema<DialogueHostProperty> =
    host_schema(&[RichTextSourceForm::CanonicalTag("shake")], &[AMP]);
const TIMED_CUE_SCHEMA: RichTextTagSchema<DialogueHostProperty> = host_schema(
    &[
        RichTextSourceForm::CanonicalTag("at"),
        RichTextSourceForm::DedicatedPayload,
    ],
    &[AT],
);
const CALL_SCHEMA: RichTextTagSchema<DialogueHostProperty> = host_schema(
    &[
        RichTextSourceForm::CanonicalTag("call"),
        RichTextSourceForm::GrammarSpelling {
            source: "!",
            canonical: "call",
        },
        RichTextSourceForm::DedicatedPayload,
    ],
    NO_PROPERTIES,
);
const SIGNAL_SCHEMA: RichTextTagSchema<DialogueHostProperty> =
    host_schema(&[RichTextSourceForm::CanonicalTag("signal")], &[SIGNAL]);
const CONDITIONAL_START_SCHEMA: RichTextTagSchema<DialogueHostProperty> = host_schema(
    &[
        RichTextSourceForm::CanonicalTag("if"),
        RichTextSourceForm::DedicatedPayload,
    ],
    NO_PROPERTIES,
);
const CONDITIONAL_ELSE_SCHEMA: RichTextTagSchema<DialogueHostProperty> =
    host_schema(&[RichTextSourceForm::CanonicalTag("else")], NO_PROPERTIES);
const CONDITIONAL_END_SCHEMA: RichTextTagSchema<DialogueHostProperty> =
    host_schema(&[RichTextSourceForm::CanonicalTag("endif")], NO_PROPERTIES);
