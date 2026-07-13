//! Shared rich-text tag and built-in effect vocabulary.

/// Canonical family of an inferred dot-selector rich-text tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichTextTagFamily {
    /// Presentation style such as italic or opacity.
    Style,
    /// Writing-mode or ruby layout.
    Layout,
    /// Post-layout visual transform.
    Transform,
    /// Registry-extensible visual effect.
    Effect,
    /// Zero-width line marker.
    Marker,
}

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
    /// Registry identity alias used by the open host-event surface.
    Effect,
    /// Registry identity alias used by the open host-event surface.
    Name,
    /// Amplitude.
    Amp,
    /// Amount.
    Amount,
    /// Period.
    Period,
    /// Sampling speed.
    Speed,
    /// Frequency alias.
    Freq,
    /// Direction vector.
    Dir,
    /// Symbolic axis.
    Axis,
    /// Deterministic seed.
    Seed,
    /// Arc radius.
    Radius,
    /// Start offset or delay alias.
    Start,
    /// Ordinal step.
    Step,
    /// Rotation angle.
    Angle,
    /// Transform origin.
    Origin,
    /// Motion function.
    Function,
    /// Motion curve alias.
    Curve,
    /// Scale amplitude alias.
    Scale,
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
    /// Cursor opacity alias.
    CursorOpacity,
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
    BuiltinRichTextFxProperty::Id,
    BuiltinRichTextFxProperty::Effect,
    BuiltinRichTextFxProperty::Name,
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

    /// Rich-text family used by syntax, tooling, and lowering.
    #[must_use]
    pub const fn family(self) -> RichTextTagFamily {
        RichTextTagFamily::Effect
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
            Self::Wave => &[
                BuiltinRichTextFxProperty::Amp,
                BuiltinRichTextFxProperty::Amount,
                BuiltinRichTextFxProperty::Period,
                BuiltinRichTextFxProperty::Speed,
                BuiltinRichTextFxProperty::Freq,
                BuiltinRichTextFxProperty::Dir,
                BuiltinRichTextFxProperty::Axis,
                BuiltinRichTextFxProperty::Seed,
            ][..],
            Self::Shake | Self::Jitter => &[
                BuiltinRichTextFxProperty::Amp,
                BuiltinRichTextFxProperty::Amount,
                BuiltinRichTextFxProperty::Period,
                BuiltinRichTextFxProperty::Speed,
                BuiltinRichTextFxProperty::Dir,
                BuiltinRichTextFxProperty::Axis,
                BuiltinRichTextFxProperty::Seed,
            ][..],
            Self::Arc => &[
                BuiltinRichTextFxProperty::Radius,
                BuiltinRichTextFxProperty::Amp,
                BuiltinRichTextFxProperty::Amount,
                BuiltinRichTextFxProperty::Start,
                BuiltinRichTextFxProperty::Step,
                BuiltinRichTextFxProperty::Seed,
            ][..],
            Self::Spin => &[
                BuiltinRichTextFxProperty::Angle,
                BuiltinRichTextFxProperty::Amp,
                BuiltinRichTextFxProperty::Amount,
                BuiltinRichTextFxProperty::Speed,
                BuiltinRichTextFxProperty::Origin,
                BuiltinRichTextFxProperty::Seed,
            ][..],
            Self::Pulse => &[
                BuiltinRichTextFxProperty::Amp,
                BuiltinRichTextFxProperty::Amount,
                BuiltinRichTextFxProperty::Speed,
                BuiltinRichTextFxProperty::Origin,
                BuiltinRichTextFxProperty::Seed,
            ][..],
            Self::Motion => &[
                BuiltinRichTextFxProperty::Function,
                BuiltinRichTextFxProperty::Curve,
                BuiltinRichTextFxProperty::Speed,
                BuiltinRichTextFxProperty::Amp,
                BuiltinRichTextFxProperty::Radius,
                BuiltinRichTextFxProperty::Amount,
                BuiltinRichTextFxProperty::Angle,
                BuiltinRichTextFxProperty::Scale,
                BuiltinRichTextFxProperty::ScaleAmp,
                BuiltinRichTextFxProperty::Seed,
            ][..],
            Self::Typewriter => &[
                BuiltinRichTextFxProperty::Cps,
                BuiltinRichTextFxProperty::Delay,
                BuiltinRichTextFxProperty::Start,
                BuiltinRichTextFxProperty::Cursor,
                BuiltinRichTextFxProperty::CursorAlpha,
                BuiltinRichTextFxProperty::CursorOpacity,
                BuiltinRichTextFxProperty::Seed,
            ][..],
            Self::Sparkle => &[
                BuiltinRichTextFxProperty::Amp,
                BuiltinRichTextFxProperty::Amount,
                BuiltinRichTextFxProperty::Speed,
                BuiltinRichTextFxProperty::Seed,
            ][..],
            Self::Shader => &[
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
            "offscreen_pass" | "run_offscreen_pass" => Some(Self::OffscreenPass),
            "post_process" => Some(Self::PostProcess),
            "host_event" => Some(Self::HostEvent),
            _ => None,
        }
    }
}

impl BuiltinRichTextFxProperty {
    /// Complete deterministic property inventory.
    pub const ALL: [Self; 28] = [
        Self::Phase,
        Self::Target,
        Self::Id,
        Self::Effect,
        Self::Name,
        Self::Amp,
        Self::Amount,
        Self::Period,
        Self::Speed,
        Self::Freq,
        Self::Dir,
        Self::Axis,
        Self::Seed,
        Self::Radius,
        Self::Start,
        Self::Step,
        Self::Angle,
        Self::Origin,
        Self::Function,
        Self::Curve,
        Self::Scale,
        Self::ScaleAmp,
        Self::Cps,
        Self::Delay,
        Self::Cursor,
        Self::CursorAlpha,
        Self::CursorOpacity,
        Self::Color,
    ];

    /// Canonical authoring name.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Phase => "phase",
            Self::Target => "target",
            Self::Id => "id",
            Self::Effect => "effect",
            Self::Name => "name",
            Self::Amp => "amp",
            Self::Amount => "amount",
            Self::Period => "period",
            Self::Speed => "speed",
            Self::Freq => "freq",
            Self::Dir => "dir",
            Self::Axis => "axis",
            Self::Seed => "seed",
            Self::Radius => "radius",
            Self::Start => "start",
            Self::Step => "step",
            Self::Angle => "angle",
            Self::Origin => "origin",
            Self::Function => "fn",
            Self::Curve => "curve",
            Self::Scale => "scale",
            Self::ScaleAmp => "scale_amp",
            Self::Cps => "cps",
            Self::Delay => "delay",
            Self::Cursor => "cursor",
            Self::CursorAlpha => "cursor_alpha",
            Self::CursorOpacity => "cursor_opacity",
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
            "effect" => Some(Self::Effect),
            "name" => Some(Self::Name),
            "amp" => Some(Self::Amp),
            "amount" => Some(Self::Amount),
            "period" => Some(Self::Period),
            "speed" => Some(Self::Speed),
            "freq" => Some(Self::Freq),
            "dir" => Some(Self::Dir),
            "axis" => Some(Self::Axis),
            "seed" => Some(Self::Seed),
            "radius" => Some(Self::Radius),
            "start" => Some(Self::Start),
            "step" => Some(Self::Step),
            "angle" => Some(Self::Angle),
            "origin" => Some(Self::Origin),
            "fn" => Some(Self::Function),
            "curve" => Some(Self::Curve),
            "scale" => Some(Self::Scale),
            "scale_amp" => Some(Self::ScaleAmp),
            "cps" => Some(Self::Cps),
            "delay" => Some(Self::Delay),
            "cursor" => Some(Self::Cursor),
            "cursor_alpha" => Some(Self::CursorAlpha),
            "cursor_opacity" => Some(Self::CursorOpacity),
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

/// Resolves the canonical family of an inferred dot-selector tag.
#[must_use]
pub fn inferred_tag_family(selector: &str, attrs: &str) -> Option<RichTextTagFamily> {
    if let Some(effect) = BuiltinRichTextFx::from_selector(selector) {
        return Some(effect.family());
    }
    match selector {
        "italic" | "oblique" | "opacity" | "alpha" | "layer" | "object_layer" | "meta"
        | "metadata" | "data" | "z" | "z_index" => Some(RichTextTagFamily::Style),
        "horizontal_tb"
        | "vertical_rl"
        | "vertical_lr"
        | "dir"
        | "ruby_over"
        | "ruby_under"
        | "ruby_inter_character" => Some(RichTextTagFamily::Layout),
        "offset" | "pos" | "rotate" | "scale" | "skew" => Some(RichTextTagFamily::Transform),
        "host" => Some(RichTextTagFamily::Effect),
        "mark" => Some(RichTextTagFamily::Marker),
        _ if !attrs.trim().is_empty() => Some(RichTextTagFamily::Effect),
        _ => None,
    }
}

/// Canonical style-stack family for an authored rich-text tag or alias.
///
/// Syntax validation and retained-text rendering both use this inventory so
/// an accepted end tag cannot resolve to a different runtime family.
#[must_use]
pub fn canonical_tag_name(name: &str) -> &str {
    if BuiltinRichTextFx::from_selector(name).is_some() {
        return "effect";
    }
    match name {
        "" | "/" => "/",
        "i" | "italic" | "oblique" | "slant" | "opacity" | "alpha" | "layer" | "object_layer"
        | "meta" | "metadata" | "data" | "z" | "z_index" | "style" => "style",
        "vertical"
        | "vertical_rl"
        | "vertical_lr"
        | "horizontal_tb"
        | "dir"
        | "ruby_over"
        | "ruby_under"
        | "ruby_inter_character"
        | "layout" => "layout",
        "offset" | "pos" | "rotate" | "scale" | "skew" | "transform" => "transform",
        "host" | "effect" | "fx" => "effect",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BuiltinRichTextFx, BuiltinRichTextFxPhase, BuiltinRichTextFxProperty, RichTextTagFamily,
        canonical_tag_name, inferred_tag_family,
    };

    #[test]
    fn builtin_metadata_is_complete_and_round_trips() {
        for effect in BuiltinRichTextFx::ALL {
            assert_eq!(
                BuiltinRichTextFx::from_selector(effect.selector()),
                Some(effect)
            );
            assert_eq!(effect.family(), RichTextTagFamily::Effect);
            assert!(effect.supported_phases().contains(&effect.default_phase()));
            assert_eq!(canonical_tag_name(effect.selector()), "effect");
            assert_eq!(
                inferred_tag_family(effect.selector(), ""),
                Some(RichTextTagFamily::Effect)
            );
            assert_eq!(
                inferred_tag_family(effect.selector(), "amp=1"),
                Some(RichTextTagFamily::Effect)
            );

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
        for phase in BuiltinRichTextFxPhase::ALL {
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
    fn unknown_and_host_selectors_keep_the_open_registry_boundary() {
        assert_eq!(inferred_tag_family("unknown", ""), None);
        assert_eq!(
            inferred_tag_family("unknown", "amp=1"),
            Some(RichTextTagFamily::Effect)
        );
        assert_eq!(
            inferred_tag_family("host", ""),
            Some(RichTextTagFamily::Effect)
        );
        assert_eq!(BuiltinRichTextFx::from_selector("host"), None);
    }

    #[test]
    fn aliases_resolve_to_their_style_stack_family() {
        for (authored, canonical) in [
            ("slant", "style"),
            ("dir", "layout"),
            ("skew", "transform"),
            ("fx", "effect"),
            ("sparkle", "effect"),
            ("custom", "custom"),
        ] {
            assert_eq!(canonical_tag_name(authored), canonical);
        }
    }
}
