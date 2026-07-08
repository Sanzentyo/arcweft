//! Style property bindings and interaction-aware retained View style data.

use crate::{DirtyFlags, StyleId, ViewError};
use arcweft_presentation::appearance::{
    PresentationColor, PresentationEnvironment, SystemColor, SystemPaletteSet,
};
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::interaction::InteractionState;
use std::collections::{BTreeMap, BTreeSet};

/// Stable identifier for a style property slot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPropertyId(pub u32);

/// Stable identifier for a dynamic value source.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueSourceId(pub u32);

/// Fixed-point scalar value in milli-units.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Milli(pub i32);

/// RGBA color value in 8-bit channels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rgba8 {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

/// Property family used by style/property binding and invalidation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewPropertyKind {
    Opacity,
    TranslateX,
    TranslateY,
    Scale,
    Rotate,
    Color,
    BackgroundColor,
    PlaceholderColor,
    SelectionColor,
    CaretColor,
    CompositionUnderlineColor,
    OutlineColor,
    OutlineWidth,
    BorderRadius,
    FontSize,
    Visibility,
    Width,
    Height,
    Display,
    SemanticLabel,
    StructuralCondition,
    Custom(u32),
}

/// Typed property value used by style evaluation output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewPropertyValue {
    Bool(bool),
    Milli(Milli),
    Color(Rgba8),
    SystemColor(SystemColor),
    Resource(u32),
}

/// Pseudo-state selector evaluated from the shared presentation interaction state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewInteractionSelector {
    Hovered,
    Focused,
    Pressed,
    Disabled,
}

/// The minimum retained View work required after a property source changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Invalidation {
    None,
    Paint,
    Layout,
    Semantics,
    Fragment,
}

/// Dynamic property binding emitted by Rust or Arcweft view rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PropertyBinding {
    property: ViewPropertyId,
    kind: ViewPropertyKind,
    source: ValueSourceId,
    invalidation: Invalidation,
}

/// Ordered dynamic property binding table for a fragment or node.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PropertyBindingTable {
    bindings: Vec<PropertyBinding>,
}

/// Builder that rejects duplicate property slots deterministically.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropertyBindingTableBuilder {
    bindings: Vec<PropertyBinding>,
    properties: BTreeSet<ViewPropertyId>,
}

/// One concrete View property after static and interaction rules are merged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedViewProperty {
    kind: ViewPropertyKind,
    value: ViewPropertyValue,
}

/// One interaction-specific style override.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewStyleRule {
    selector: ViewInteractionSelector,
    property: ResolvedViewProperty,
}

/// Retained base style plus interaction-specific overrides.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewStyle {
    base: Vec<ResolvedViewProperty>,
    rules: Vec<ViewStyleRule>,
}

/// Style registry keyed by the `StyleId` stored in each fragment node.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewStyleTable {
    styles: BTreeMap<StyleId, ViewStyle>,
}

/// Fully resolved property list for one display item in one interaction state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedViewStyle {
    properties: Vec<ResolvedViewProperty>,
}

impl Milli {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1_000);

    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> i32 {
        self.0
    }

    #[must_use]
    pub fn lerp(self, target: Self, progress: Self) -> Self {
        let progress = i64::from(progress.value().clamp(0, Self::ONE.value()));
        let source = i64::from(self.value());
        let delta = i64::from(target.value()).saturating_sub(source);
        let value = source.saturating_add(
            (delta.saturating_mul(progress) + i64::from(Self::ONE.value() / 2))
                / i64::from(Self::ONE.value()),
        );
        Self(
            i32::try_from(value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)))
                .unwrap_or(if value < 0 { i32::MIN } else { i32::MAX }),
        )
    }
}

impl Rgba8 {
    pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub const fn from_presentation(color: PresentationColor) -> Self {
        Self {
            red: color.red,
            green: color.green,
            blue: color.blue,
            alpha: color.alpha,
        }
    }

    #[must_use]
    pub fn lerp(self, target: Self, progress: Milli) -> Self {
        let progress = progress
            .value()
            .clamp(Milli::ZERO.value(), Milli::ONE.value());
        Self {
            red: lerp_channel(self.red, target.red, progress),
            green: lerp_channel(self.green, target.green, progress),
            blue: lerp_channel(self.blue, target.blue, progress),
            alpha: lerp_channel(self.alpha, target.alpha, progress),
        }
    }
}

impl ViewPropertyKind {
    pub const fn is_transitionable(self) -> bool {
        matches!(
            self,
            Self::Opacity
                | Self::TranslateX
                | Self::TranslateY
                | Self::Scale
                | Self::Rotate
                | Self::Color
                | Self::BackgroundColor
                | Self::PlaceholderColor
                | Self::SelectionColor
                | Self::CaretColor
                | Self::CompositionUnderlineColor
                | Self::OutlineColor
                | Self::OutlineWidth
                | Self::BorderRadius
        )
    }

    pub fn interpolate_value(
        self,
        source: ViewPropertyValue,
        target: ViewPropertyValue,
        progress: Milli,
    ) -> Option<ViewPropertyValue> {
        if !self.is_transitionable() || !self.accepts(source) || !self.accepts(target) {
            return None;
        }
        match (source, target) {
            (ViewPropertyValue::Milli(source), ViewPropertyValue::Milli(target)) => {
                Some(ViewPropertyValue::Milli(source.lerp(target, progress)))
            }
            (ViewPropertyValue::Color(source), ViewPropertyValue::Color(target)) => {
                Some(ViewPropertyValue::Color(source.lerp(target, progress)))
            }
            (
                ViewPropertyValue::Bool(_)
                | ViewPropertyValue::SystemColor(_)
                | ViewPropertyValue::Resource(_),
                _,
            )
            | (
                _,
                ViewPropertyValue::Bool(_)
                | ViewPropertyValue::SystemColor(_)
                | ViewPropertyValue::Resource(_),
            )
            | (ViewPropertyValue::Milli(_), ViewPropertyValue::Color(_))
            | (ViewPropertyValue::Color(_), ViewPropertyValue::Milli(_)) => None,
        }
    }

    pub const fn default_invalidation(self) -> Invalidation {
        match self {
            Self::Opacity
            | Self::TranslateX
            | Self::TranslateY
            | Self::Scale
            | Self::Rotate
            | Self::Color
            | Self::BackgroundColor
            | Self::PlaceholderColor
            | Self::SelectionColor
            | Self::CaretColor
            | Self::CompositionUnderlineColor
            | Self::OutlineColor
            | Self::OutlineWidth
            | Self::BorderRadius
            | Self::Visibility
            | Self::Custom(_) => Invalidation::Paint,
            Self::Width | Self::Height | Self::Display | Self::FontSize => Invalidation::Layout,
            Self::SemanticLabel => Invalidation::Semantics,
            Self::StructuralCondition => Invalidation::Fragment,
        }
    }

    pub const fn accepts(self, value: ViewPropertyValue) -> bool {
        match self {
            Self::Opacity
            | Self::TranslateX
            | Self::TranslateY
            | Self::Scale
            | Self::Rotate
            | Self::OutlineWidth
            | Self::BorderRadius
            | Self::FontSize
            | Self::Width
            | Self::Height => matches!(value, ViewPropertyValue::Milli(_)),
            Self::Color
            | Self::BackgroundColor
            | Self::PlaceholderColor
            | Self::SelectionColor
            | Self::CaretColor
            | Self::CompositionUnderlineColor
            | Self::OutlineColor => matches!(
                value,
                ViewPropertyValue::Color(_) | ViewPropertyValue::SystemColor(_)
            ),
            Self::Visibility | Self::Display | Self::StructuralCondition => {
                matches!(value, ViewPropertyValue::Bool(_))
            }
            Self::SemanticLabel => matches!(value, ViewPropertyValue::Resource(_)),
            Self::Custom(_) => true,
        }
    }
}

impl ViewPropertyValue {
    pub const fn as_bool(self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(value),
            Self::Milli(_) | Self::Color(_) | Self::SystemColor(_) | Self::Resource(_) => None,
        }
    }

    pub const fn as_milli(self) -> Option<Milli> {
        match self {
            Self::Milli(value) => Some(value),
            Self::Bool(_) | Self::Color(_) | Self::SystemColor(_) | Self::Resource(_) => None,
        }
    }

    pub const fn as_color(self) -> Option<Rgba8> {
        match self {
            Self::Color(value) => Some(value),
            Self::Bool(_) | Self::Milli(_) | Self::SystemColor(_) | Self::Resource(_) => None,
        }
    }

    pub const fn as_system_color(self) -> Option<SystemColor> {
        match self {
            Self::SystemColor(value) => Some(value),
            Self::Bool(_) | Self::Milli(_) | Self::Color(_) | Self::Resource(_) => None,
        }
    }

    pub fn resolve_color(
        self,
        environment: &PresentationEnvironment,
        palette: SystemPaletteSet,
    ) -> Option<Rgba8> {
        self.as_color().or_else(|| {
            self.as_system_color().map(|role| {
                Rgba8::from_presentation(palette.color(environment.color_scheme(), role))
            })
        })
    }

    pub const fn as_resource(self) -> Option<u32> {
        match self {
            Self::Resource(value) => Some(value),
            Self::Bool(_) | Self::Milli(_) | Self::Color(_) | Self::SystemColor(_) => None,
        }
    }
}

impl ViewInteractionSelector {
    pub const fn cascade() -> [Self; 4] {
        [Self::Hovered, Self::Focused, Self::Pressed, Self::Disabled]
    }

    pub fn matches(
        self,
        target: Option<&InteractionTarget>,
        enabled: bool,
        interaction: &InteractionState,
    ) -> bool {
        match self {
            Self::Hovered => target.is_some_and(|target| interaction.is_hovered(target)),
            Self::Focused => target.is_some_and(|target| interaction.is_focused(target)),
            Self::Pressed => target.is_some_and(|target| interaction.is_pressed(target)),
            Self::Disabled => !enabled,
        }
    }
}

impl Invalidation {
    pub const fn dirty_flags(self) -> DirtyFlags {
        match self {
            Self::None => DirtyFlags::NONE,
            Self::Paint => DirtyFlags::PAINT,
            Self::Layout => DirtyFlags::LAYOUT,
            Self::Semantics => DirtyFlags::SEMANTICS,
            Self::Fragment => DirtyFlags::FRAGMENT,
        }
    }
}

impl PropertyBinding {
    pub const fn new(
        property: ViewPropertyId,
        kind: ViewPropertyKind,
        source: ValueSourceId,
    ) -> Self {
        Self {
            property,
            kind,
            source,
            invalidation: kind.default_invalidation(),
        }
    }

    #[must_use]
    pub const fn with_invalidation(mut self, invalidation: Invalidation) -> Self {
        self.invalidation = invalidation;
        self
    }

    pub const fn property(self) -> ViewPropertyId {
        self.property
    }

    pub const fn kind(self) -> ViewPropertyKind {
        self.kind
    }

    pub const fn source(self) -> ValueSourceId {
        self.source
    }

    pub const fn invalidation(self) -> Invalidation {
        self.invalidation
    }

    pub const fn dirty_flags(self) -> DirtyFlags {
        self.invalidation.dirty_flags()
    }
}

impl PropertyBindingTable {
    pub fn as_slice(&self) -> &[PropertyBinding] {
        &self.bindings
    }

    pub fn dirty_flags_for_source(&self, source: ValueSourceId) -> DirtyFlags {
        self.bindings
            .iter()
            .filter(|binding| binding.source() == source)
            .fold(DirtyFlags::NONE, |mut flags, binding| {
                flags.insert(binding.dirty_flags());
                flags
            })
    }
}

impl PropertyBindingTableBuilder {
    pub fn push(&mut self, binding: PropertyBinding) -> Result<(), ViewError> {
        if !self.properties.insert(binding.property()) {
            return Err(ViewError::DuplicatePropertyBinding(binding.property()));
        }
        self.bindings.push(binding);
        Ok(())
    }

    pub fn finish(self) -> PropertyBindingTable {
        PropertyBindingTable {
            bindings: self.bindings,
        }
    }
}

impl ResolvedViewProperty {
    pub fn new(kind: ViewPropertyKind, value: ViewPropertyValue) -> Result<Self, ViewError> {
        if !kind.accepts(value) {
            return Err(ViewError::InvalidViewPropertyValue { kind, value });
        }
        Ok(Self { kind, value })
    }

    pub const fn kind(self) -> ViewPropertyKind {
        self.kind
    }

    pub const fn value(self) -> ViewPropertyValue {
        self.value
    }
}

impl ViewStyleRule {
    pub fn new(
        selector: ViewInteractionSelector,
        kind: ViewPropertyKind,
        value: ViewPropertyValue,
    ) -> Result<Self, ViewError> {
        Ok(Self {
            selector,
            property: ResolvedViewProperty::new(kind, value)?,
        })
    }

    pub const fn selector(self) -> ViewInteractionSelector {
        self.selector
    }

    pub const fn property(self) -> ResolvedViewProperty {
        self.property
    }
}

impl ViewStyle {
    pub fn set_base(
        &mut self,
        kind: ViewPropertyKind,
        value: ViewPropertyValue,
    ) -> Result<(), ViewError> {
        if self.base.iter().any(|property| property.kind() == kind) {
            return Err(ViewError::DuplicateStyleProperty(kind));
        }
        self.base.push(ResolvedViewProperty::new(kind, value)?);
        Ok(())
    }

    pub fn set_rule(
        &mut self,
        selector: ViewInteractionSelector,
        kind: ViewPropertyKind,
        value: ViewPropertyValue,
    ) -> Result<(), ViewError> {
        if self
            .rules
            .iter()
            .any(|rule| rule.selector() == selector && rule.property().kind() == kind)
        {
            return Err(ViewError::DuplicateStyleRule { selector, kind });
        }
        self.rules.push(ViewStyleRule::new(selector, kind, value)?);
        Ok(())
    }

    pub fn base(&self) -> &[ResolvedViewProperty] {
        &self.base
    }

    pub fn rules(&self) -> &[ViewStyleRule] {
        &self.rules
    }

    pub fn resolve(
        &self,
        target: Option<&InteractionTarget>,
        enabled: bool,
        interaction: &InteractionState,
    ) -> ResolvedViewStyle {
        let mut resolved = ResolvedViewStyle {
            properties: self.base.clone(),
        };
        for selector in ViewInteractionSelector::cascade() {
            if selector.matches(target, enabled, interaction) {
                for rule in self
                    .rules
                    .iter()
                    .copied()
                    .filter(|rule| rule.selector() == selector)
                {
                    resolved.set(rule.property());
                }
            }
        }
        resolved
    }
}

impl ViewStyleTable {
    pub fn insert(&mut self, id: StyleId, style: ViewStyle) -> Result<(), ViewError> {
        if self.styles.contains_key(&id) {
            return Err(ViewError::DuplicateStyle(id));
        }
        self.styles.insert(id, style);
        Ok(())
    }

    pub fn get(&self, id: StyleId) -> Option<&ViewStyle> {
        self.styles.get(&id)
    }

    pub fn resolve(
        &self,
        id: StyleId,
        target: Option<&InteractionTarget>,
        enabled: bool,
        interaction: &InteractionState,
    ) -> Result<ResolvedViewStyle, ViewError> {
        if self.is_empty() {
            return Ok(ResolvedViewStyle::default());
        }
        self.get(id)
            .map(|style| style.resolve(target, enabled, interaction))
            .ok_or(ViewError::UnknownStyle(id))
    }

    pub fn contains(&self, id: StyleId) -> bool {
        self.styles.contains_key(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }
}

impl ResolvedViewStyle {
    fn set(&mut self, property: ResolvedViewProperty) {
        if let Some(existing) = self
            .properties
            .iter_mut()
            .find(|existing| existing.kind() == property.kind())
        {
            *existing = property;
        } else {
            self.properties.push(property);
        }
    }

    pub fn as_slice(&self) -> &[ResolvedViewProperty] {
        &self.properties
    }

    pub fn value(&self, kind: ViewPropertyKind) -> Option<ViewPropertyValue> {
        self.properties
            .iter()
            .find(|property| property.kind() == kind)
            .map(|property| property.value())
    }

    pub fn bool(&self, kind: ViewPropertyKind) -> Option<bool> {
        self.value(kind).and_then(ViewPropertyValue::as_bool)
    }

    pub fn milli(&self, kind: ViewPropertyKind) -> Option<Milli> {
        self.value(kind).and_then(ViewPropertyValue::as_milli)
    }

    pub fn color(&self, kind: ViewPropertyKind) -> Option<Rgba8> {
        self.value(kind).and_then(ViewPropertyValue::as_color)
    }

    pub fn resolved_color(
        &self,
        kind: ViewPropertyKind,
        environment: &PresentationEnvironment,
    ) -> Option<Rgba8> {
        self.value(kind)
            .and_then(|value| value.resolve_color(environment, SystemPaletteSet::ENGINE_DEFAULT))
    }

    pub fn is_visible(&self) -> bool {
        self.bool(ViewPropertyKind::Visibility).unwrap_or(true)
            && self.bool(ViewPropertyKind::Display).unwrap_or(true)
    }

    pub fn opacity(&self) -> Milli {
        self.milli(ViewPropertyKind::Opacity).unwrap_or(Milli::ONE)
    }

    pub fn scale(&self) -> Milli {
        self.milli(ViewPropertyKind::Scale).unwrap_or(Milli::ONE)
    }
}

fn lerp_channel(source: u8, target: u8, progress: i32) -> u8 {
    let source = i32::from(source);
    let delta = i32::from(target).saturating_sub(source);
    let value = source.saturating_add(
        (delta.saturating_mul(progress) + Milli::ONE.value() / 2) / Milli::ONE.value(),
    );
    u8::try_from(value.clamp(0, 255)).unwrap_or(0)
}
