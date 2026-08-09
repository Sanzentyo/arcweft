//! Renderer-independent rich-text tag and built-in effect vocabulary.

mod authoring_schema;
mod builtin_schema;

pub use authoring_schema::{
    RichTextDirectStyle, RichTextDirectStyleProperty, RichTextLayoutProperty,
    RichTextLayoutSelector, RichTextObjectProperty, RichTextObjectSelector, RichTextStyleProperty,
    RichTextStyleSelector, RichTextTransformProperty, RichTextTransformSelector,
};
pub use builtin_schema::BuiltinPropertyDisposition;

/// Arcweft-owned rich-text effects with closed executable definitions.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BuiltinRichTextFx {
    /// Periodic displacement.
    Wave,
    /// Time-varying deterministic displacement.
    Shake,
    /// Stable deterministic displacement.
    Jitter,
    /// Ordinal arc placement.
    Arc,
    /// Time-varying rotation.
    Spin,
    /// Time-varying scale.
    Pulse,
    /// Shared motion-provider sampling.
    Motion,
    /// Glyph reveal mask.
    Typewriter,
    /// Transform, color, or post-process shimmer.
    Sparkle,
    /// Typed shader-resource application.
    Shader,
}

/// Closed phase vocabulary accepted by rich-text effect authoring.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BuiltinRichTextFxPhase {
    /// Runs before layout.
    BeforeLayout,
    /// Participates in layout transforms.
    LayoutTransform,
    /// Transforms laid-out glyphs.
    GlyphTransform,
    /// Changes glyph color.
    GlyphColor,
    /// Changes glyph coverage.
    GlyphMask,
    /// Runs an offscreen pass.
    OffscreenPass,
    /// Applies a viewport post-process.
    PostProcess,
    /// Emits a host event instead of a visual effect.
    HostEvent,
}

/// Closed property names understood by Arcweft-owned rich-text effects.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BuiltinRichTextFxProperty {
    /// Authored execution phase.
    Phase,
    /// Authored effect target.
    Target,
    /// Resource or registry identity.
    Id,
    /// Amplitude.
    Amp,
    /// Amount.
    Amount,
    /// Period.
    Period,
    /// Sampling speed.
    Speed,
    /// Direction vector.
    Dir,
    /// Deterministic seed.
    Seed,
    /// Arc radius.
    Radius,
    /// Arc start angle.
    Start,
    /// Ordinal step.
    Step,
    /// Rotation angle.
    Angle,
    /// Motion function.
    Function,
    /// Explicit scale amplitude.
    ScaleAmp,
    /// Characters per second.
    Cps,
    /// Reveal delay.
    Delay,
    /// Cursor visibility.
    Cursor,
    /// Cursor alpha.
    CursorAlpha,
    /// Shader color uniform.
    Color,
}

/// Property schema for one Arcweft-owned rich-text effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuiltinRichTextFxPropertySchema {
    specific: &'static [BuiltinRichTextFxProperty],
}

const COMMON_FX_PROPERTIES: &[BuiltinRichTextFxProperty] = &[
    BuiltinRichTextFxProperty::Phase,
    BuiltinRichTextFxProperty::Target,
];

const TRANSFORM_AND_POST_PHASES: &[BuiltinRichTextFxPhase] = &[
    BuiltinRichTextFxPhase::GlyphTransform,
    BuiltinRichTextFxPhase::PostProcess,
];
const SPARKLE_PHASES: &[BuiltinRichTextFxPhase] = &[
    BuiltinRichTextFxPhase::GlyphTransform,
    BuiltinRichTextFxPhase::GlyphColor,
    BuiltinRichTextFxPhase::PostProcess,
];
const TYPEWRITER_PHASES: &[BuiltinRichTextFxPhase] = &[BuiltinRichTextFxPhase::GlyphMask];
const SHADER_PHASES: &[BuiltinRichTextFxPhase] = &[
    BuiltinRichTextFxPhase::GlyphColor,
    BuiltinRichTextFxPhase::OffscreenPass,
    BuiltinRichTextFxPhase::PostProcess,
];

impl BuiltinRichTextFx {
    /// Complete deterministic inventory of Arcweft-owned effects.
    pub const ALL: [Self; 10] = [
        Self::Wave,
        Self::Shake,
        Self::Jitter,
        Self::Arc,
        Self::Spin,
        Self::Pulse,
        Self::Motion,
        Self::Typewriter,
        Self::Sparkle,
        Self::Shader,
    ];

    /// Canonical dot-selector name without the leading dot.
    #[must_use]
    pub const fn selector(self) -> &'static str {
        match self {
            Self::Wave => "wave",
            Self::Shake => "shake",
            Self::Jitter => "jitter",
            Self::Arc => "arc",
            Self::Spin => "spin",
            Self::Pulse => "pulse",
            Self::Motion => "motion",
            Self::Typewriter => "typewriter",
            Self::Sparkle => "sparkle",
            Self::Shader => "shader",
        }
    }

    /// Resolves a canonical selector to its Arcweft-owned effect.
    #[must_use]
    pub fn from_selector(selector: &str) -> Option<Self> {
        match selector.trim().trim_start_matches('.') {
            "wave" => Some(Self::Wave),
            "shake" => Some(Self::Shake),
            "jitter" => Some(Self::Jitter),
            "arc" => Some(Self::Arc),
            "spin" => Some(Self::Spin),
            "pulse" => Some(Self::Pulse),
            "motion" => Some(Self::Motion),
            "typewriter" => Some(Self::Typewriter),
            "sparkle" => Some(Self::Sparkle),
            "shader" => Some(Self::Shader),
            _ => None,
        }
    }

    /// Default phase when the author omits `phase`.
    #[must_use]
    pub const fn default_phase(self) -> BuiltinRichTextFxPhase {
        match self {
            Self::Typewriter => BuiltinRichTextFxPhase::GlyphMask,
            Self::Shader => BuiltinRichTextFxPhase::OffscreenPass,
            Self::Wave
            | Self::Shake
            | Self::Jitter
            | Self::Arc
            | Self::Spin
            | Self::Pulse
            | Self::Motion
            | Self::Sparkle => BuiltinRichTextFxPhase::GlyphTransform,
        }
    }

    /// Phases for which this effect owns an executable program.
    #[must_use]
    pub const fn supported_phases(self) -> &'static [BuiltinRichTextFxPhase] {
        match self {
            Self::Wave
            | Self::Shake
            | Self::Jitter
            | Self::Arc
            | Self::Spin
            | Self::Pulse
            | Self::Motion => TRANSFORM_AND_POST_PHASES,
            Self::Sparkle => SPARKLE_PHASES,
            Self::Typewriter => TYPEWRITER_PHASES,
            Self::Shader => SHADER_PHASES,
        }
    }

    /// Closed property schema for this effect.
    #[must_use]
    pub fn property_schema(self) -> BuiltinRichTextFxPropertySchema {
        let specific = match self {
            Self::Wave | Self::Shake | Self::Jitter => &[
                BuiltinRichTextFxProperty::Amp,
                BuiltinRichTextFxProperty::Period,
                BuiltinRichTextFxProperty::Speed,
                BuiltinRichTextFxProperty::Dir,
                BuiltinRichTextFxProperty::Seed,
            ][..],
            Self::Arc => &[
                BuiltinRichTextFxProperty::Radius,
                BuiltinRichTextFxProperty::Amount,
                BuiltinRichTextFxProperty::Start,
                BuiltinRichTextFxProperty::Step,
            ][..],
            Self::Spin => &[
                BuiltinRichTextFxProperty::Angle,
                BuiltinRichTextFxProperty::Amount,
                BuiltinRichTextFxProperty::Speed,
            ][..],
            Self::Pulse => &[
                BuiltinRichTextFxProperty::Amp,
                BuiltinRichTextFxProperty::Amount,
            ][..],
            Self::Motion => &[
                BuiltinRichTextFxProperty::Function,
                BuiltinRichTextFxProperty::Speed,
                BuiltinRichTextFxProperty::Amp,
                BuiltinRichTextFxProperty::Amount,
                BuiltinRichTextFxProperty::Angle,
                BuiltinRichTextFxProperty::ScaleAmp,
            ][..],
            Self::Typewriter => &[
                BuiltinRichTextFxProperty::Cps,
                BuiltinRichTextFxProperty::Delay,
                BuiltinRichTextFxProperty::Cursor,
                BuiltinRichTextFxProperty::CursorAlpha,
            ][..],
            Self::Sparkle => &[
                BuiltinRichTextFxProperty::Amp,
                BuiltinRichTextFxProperty::Amount,
                BuiltinRichTextFxProperty::Speed,
                BuiltinRichTextFxProperty::Seed,
            ][..],
            Self::Shader => &[
                BuiltinRichTextFxProperty::Id,
                BuiltinRichTextFxProperty::Amount,
                BuiltinRichTextFxProperty::Dir,
                BuiltinRichTextFxProperty::Color,
            ][..],
        };
        BuiltinRichTextFxPropertySchema { specific }
    }
}

impl BuiltinRichTextFxPhase {
    /// Complete deterministic phase inventory.
    pub const ALL: [Self; 8] = [
        Self::BeforeLayout,
        Self::LayoutTransform,
        Self::GlyphTransform,
        Self::GlyphColor,
        Self::GlyphMask,
        Self::OffscreenPass,
        Self::PostProcess,
        Self::HostEvent,
    ];

    /// Canonical authoring name.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::BeforeLayout => "before_layout",
            Self::LayoutTransform => "layout_transform",
            Self::GlyphTransform => "glyph_transform",
            Self::GlyphColor => "glyph_color",
            Self::GlyphMask => "glyph_mask",
            Self::OffscreenPass => "run_offscreen_pass",
            Self::PostProcess => "post_process",
            Self::HostEvent => "host_event",
        }
    }

    /// Resolves an authored phase name.
    #[must_use]
    pub fn from_source_name(name: &str) -> Option<Self> {
        match name.trim().trim_start_matches('.') {
            "before_layout" => Some(Self::BeforeLayout),
            "layout_transform" => Some(Self::LayoutTransform),
            "glyph_transform" => Some(Self::GlyphTransform),
            "glyph_color" => Some(Self::GlyphColor),
            "glyph_mask" => Some(Self::GlyphMask),
            "run_offscreen_pass" => Some(Self::OffscreenPass),
            "post_process" => Some(Self::PostProcess),
            _ => None,
        }
    }
}

impl BuiltinRichTextFxProperty {
    /// Complete deterministic property inventory.
    pub const ALL: [Self; 20] = [
        Self::Phase,
        Self::Target,
        Self::Id,
        Self::Amp,
        Self::Amount,
        Self::Period,
        Self::Speed,
        Self::Dir,
        Self::Seed,
        Self::Radius,
        Self::Start,
        Self::Step,
        Self::Angle,
        Self::Function,
        Self::ScaleAmp,
        Self::Cps,
        Self::Delay,
        Self::Cursor,
        Self::CursorAlpha,
        Self::Color,
    ];

    /// Canonical authoring name.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Phase => "phase",
            Self::Target => "target",
            Self::Id => "id",
            Self::Amp => "amp",
            Self::Amount => "amount",
            Self::Period => "period",
            Self::Speed => "speed",
            Self::Dir => "dir",
            Self::Seed => "seed",
            Self::Radius => "radius",
            Self::Start => "start",
            Self::Step => "step",
            Self::Angle => "angle",
            Self::Function => "fn",
            Self::ScaleAmp => "scale_amp",
            Self::Cps => "cps",
            Self::Delay => "delay",
            Self::Cursor => "cursor",
            Self::CursorAlpha => "cursor_alpha",
            Self::Color => "color",
        }
    }

    /// Resolves a canonical authoring property name.
    #[must_use]
    pub fn from_source_name(name: &str) -> Option<Self> {
        match name {
            "phase" => Some(Self::Phase),
            "target" => Some(Self::Target),
            "id" => Some(Self::Id),
            "amp" => Some(Self::Amp),
            "amount" => Some(Self::Amount),
            "period" => Some(Self::Period),
            "speed" => Some(Self::Speed),
            "dir" => Some(Self::Dir),
            "seed" => Some(Self::Seed),
            "radius" => Some(Self::Radius),
            "start" => Some(Self::Start),
            "step" => Some(Self::Step),
            "angle" => Some(Self::Angle),
            "fn" => Some(Self::Function),
            "scale_amp" => Some(Self::ScaleAmp),
            "cps" => Some(Self::Cps),
            "delay" => Some(Self::Delay),
            "cursor" => Some(Self::Cursor),
            "cursor_alpha" => Some(Self::CursorAlpha),
            "color" => Some(Self::Color),
            _ => None,
        }
    }
}

impl BuiltinRichTextFxPropertySchema {
    /// Whether the property belongs to the schema.
    #[must_use]
    pub fn accepts(self, property: BuiltinRichTextFxProperty) -> bool {
        COMMON_FX_PROPERTIES.contains(&property) || self.specific.contains(&property)
    }

    /// All accepted properties in deterministic order.
    pub fn properties(self) -> impl Iterator<Item = BuiltinRichTextFxProperty> {
        COMMON_FX_PROPERTIES.iter().chain(self.specific).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BuiltinPropertyDisposition, BuiltinRichTextFx, BuiltinRichTextFxPhase,
        BuiltinRichTextFxProperty,
    };

    #[test]
    fn builtin_metadata_is_complete_and_round_trips() {
        for effect in BuiltinRichTextFx::ALL {
            assert_eq!(
                BuiltinRichTextFx::from_selector(effect.selector()),
                Some(effect)
            );
            assert!(effect.supported_phases().contains(&effect.default_phase()));
            let properties = effect.property_schema().properties().collect::<Vec<_>>();
            for (index, property) in properties.iter().copied().enumerate() {
                assert_eq!(
                    BuiltinRichTextFxProperty::from_source_name(property.source_name()),
                    Some(property)
                );
                assert!(!properties[..index].contains(&property));
            }
        }
    }

    #[test]
    fn phase_and_property_inventories_round_trip() {
        for phase in BuiltinRichTextFxPhase::ALL
            .into_iter()
            .filter(|phase| *phase != BuiltinRichTextFxPhase::HostEvent)
        {
            assert_eq!(
                BuiltinRichTextFxPhase::from_source_name(phase.source_name()),
                Some(phase)
            );
        }
        for property in BuiltinRichTextFxProperty::ALL {
            assert_eq!(
                BuiltinRichTextFxProperty::from_source_name(property.source_name()),
                Some(property)
            );
        }
    }

    #[test]
    fn phase_property_inventory_is_owned_by_the_original_effect_enums() {
        for effect in BuiltinRichTextFx::ALL {
            for phase in BuiltinRichTextFxPhase::ALL {
                let properties = effect.properties_for_phase(phase);
                for property in BuiltinRichTextFxProperty::ALL {
                    match effect.property_spec(phase, property) {
                        BuiltinPropertyDisposition::Accepted(spec) => {
                            assert!(properties.contains(&property));
                            assert_eq!(spec.id, property);
                            assert_eq!(spec.source_name, property.source_name());
                        }
                        BuiltinPropertyDisposition::UnsupportedInPhase => {
                            assert!(!properties.contains(&property));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn conditional_defaults_are_owned_by_the_original_effect_enum() {
        use arcweft_rich_text_schema::RichTextDefaultValue;

        for effect in BuiltinRichTextFx::ALL {
            for phase in BuiltinRichTextFxPhase::ALL {
                for property in BuiltinRichTextFxProperty::ALL {
                    let default = effect.conditional_default(phase, property);
                    if default.is_some() {
                        let BuiltinPropertyDisposition::Accepted(spec) =
                            effect.property_spec(phase, property)
                        else {
                            panic!("a conditional default must have an accepted property schema");
                        };
                        assert!(matches!(
                            spec.presence,
                            arcweft_rich_text_schema::PropertyPresence::Conditional { .. }
                        ));
                    }
                }
            }
        }
        assert_eq!(
            BuiltinRichTextFx::Typewriter.conditional_default(
                BuiltinRichTextFxPhase::GlyphMask,
                BuiltinRichTextFxProperty::CursorAlpha,
            ),
            Some(RichTextDefaultValue::RatioMilli(350))
        );
    }

    #[test]
    fn provisional_property_and_phase_aliases_are_not_source_members() {
        for removed in [
            "freq",
            "axis",
            "effect",
            "name",
            "curve",
            "scale",
            "cursor_opacity",
            "origin",
        ] {
            assert_eq!(BuiltinRichTextFxProperty::from_source_name(removed), None);
        }
        assert_eq!(
            BuiltinRichTextFxPhase::from_source_name("run_offscreen_pass"),
            Some(BuiltinRichTextFxPhase::OffscreenPass)
        );
        assert_eq!(
            BuiltinRichTextFxPhase::from_source_name("offscreen_pass"),
            None
        );
        assert_eq!(BuiltinRichTextFxPhase::from_source_name("host_event"), None);
    }

    #[test]
    fn removed_open_registry_selectors_have_no_builtin_identity() {
        assert_eq!(BuiltinRichTextFx::from_selector("host"), None);
        assert_eq!(BuiltinRichTextFx::from_selector("unknown"), None);
    }
}
