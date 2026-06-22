//! Style property bindings and interaction-aware retained UI style data.

use crate::{DirtyFlags, StyleId, UiError};
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::interaction::InteractionState;
use std::collections::{BTreeMap, BTreeSet};

/// Stable identifier for a style property slot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UiPropertyId(pub u32);

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
pub enum UiPropertyKind {
    Opacity,
    TranslateX,
    TranslateY,
    Scale,
    Rotate,
    Color,
    BackgroundColor,
    OutlineColor,
    OutlineWidth,
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
pub enum UiPropertyValue {
    Bool(bool),
    Milli(Milli),
    Color(Rgba8),
    Resource(u32),
}

/// Pseudo-state selector evaluated from the shared presentation interaction state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UiInteractionSelector {
    Hovered,
    Focused,
    Pressed,
    Disabled,
}

/// The minimum retained UI work required after a property source changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Invalidation {
    None,
    Paint,
    Layout,
    Semantics,
    Fragment,
}

/// Dynamic property binding emitted by Rust or Arcweft component rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PropertyBinding {
    property: UiPropertyId,
    kind: UiPropertyKind,
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
    properties: BTreeSet<UiPropertyId>,
}

/// One concrete UI property after static and interaction rules are merged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedUiProperty {
    kind: UiPropertyKind,
    value: UiPropertyValue,
}

/// One interaction-specific style override.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiStyleRule {
    selector: UiInteractionSelector,
    property: ResolvedUiProperty,
}

/// Retained base style plus interaction-specific overrides.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UiStyle {
    base: Vec<ResolvedUiProperty>,
    rules: Vec<UiStyleRule>,
}

/// Style registry keyed by the `StyleId` stored in each fragment node.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UiStyleTable {
    styles: BTreeMap<StyleId, UiStyle>,
}

/// Fully resolved property list for one display item in one interaction state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedUiStyle {
    properties: Vec<ResolvedUiProperty>,
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
}

impl UiPropertyKind {
    pub const fn default_invalidation(self) -> Invalidation {
        match self {
            Self::Opacity
            | Self::TranslateX
            | Self::TranslateY
            | Self::Scale
            | Self::Rotate
            | Self::Color
            | Self::BackgroundColor
            | Self::OutlineColor
            | Self::OutlineWidth
            | Self::Visibility
            | Self::Custom(_) => Invalidation::Paint,
            Self::Width | Self::Height | Self::Display => Invalidation::Layout,
            Self::SemanticLabel => Invalidation::Semantics,
            Self::StructuralCondition => Invalidation::Fragment,
        }
    }

    pub const fn accepts(self, value: UiPropertyValue) -> bool {
        match self {
            Self::Opacity
            | Self::TranslateX
            | Self::TranslateY
            | Self::Scale
            | Self::Rotate
            | Self::OutlineWidth
            | Self::Width
            | Self::Height => matches!(value, UiPropertyValue::Milli(_)),
            Self::Color | Self::BackgroundColor | Self::OutlineColor => {
                matches!(value, UiPropertyValue::Color(_))
            }
            Self::Visibility | Self::Display | Self::StructuralCondition => {
                matches!(value, UiPropertyValue::Bool(_))
            }
            Self::SemanticLabel => matches!(value, UiPropertyValue::Resource(_)),
            Self::Custom(_) => true,
        }
    }
}

impl UiPropertyValue {
    pub const fn as_bool(self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(value),
            Self::Milli(_) | Self::Color(_) | Self::Resource(_) => None,
        }
    }

    pub const fn as_milli(self) -> Option<Milli> {
        match self {
            Self::Milli(value) => Some(value),
            Self::Bool(_) | Self::Color(_) | Self::Resource(_) => None,
        }
    }

    pub const fn as_color(self) -> Option<Rgba8> {
        match self {
            Self::Color(value) => Some(value),
            Self::Bool(_) | Self::Milli(_) | Self::Resource(_) => None,
        }
    }

    pub const fn as_resource(self) -> Option<u32> {
        match self {
            Self::Resource(value) => Some(value),
            Self::Bool(_) | Self::Milli(_) | Self::Color(_) => None,
        }
    }
}

impl UiInteractionSelector {
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
    pub const fn new(property: UiPropertyId, kind: UiPropertyKind, source: ValueSourceId) -> Self {
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

    pub const fn property(self) -> UiPropertyId {
        self.property
    }

    pub const fn kind(self) -> UiPropertyKind {
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
    pub fn push(&mut self, binding: PropertyBinding) -> Result<(), UiError> {
        if !self.properties.insert(binding.property()) {
            return Err(UiError::DuplicatePropertyBinding(binding.property()));
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

impl ResolvedUiProperty {
    pub fn new(kind: UiPropertyKind, value: UiPropertyValue) -> Result<Self, UiError> {
        if !kind.accepts(value) {
            return Err(UiError::InvalidUiPropertyValue { kind, value });
        }
        Ok(Self { kind, value })
    }

    pub const fn kind(self) -> UiPropertyKind {
        self.kind
    }

    pub const fn value(self) -> UiPropertyValue {
        self.value
    }
}

impl UiStyleRule {
    pub fn new(
        selector: UiInteractionSelector,
        kind: UiPropertyKind,
        value: UiPropertyValue,
    ) -> Result<Self, UiError> {
        Ok(Self {
            selector,
            property: ResolvedUiProperty::new(kind, value)?,
        })
    }

    pub const fn selector(self) -> UiInteractionSelector {
        self.selector
    }

    pub const fn property(self) -> ResolvedUiProperty {
        self.property
    }
}

impl UiStyle {
    pub fn set_base(
        &mut self,
        kind: UiPropertyKind,
        value: UiPropertyValue,
    ) -> Result<(), UiError> {
        if self.base.iter().any(|property| property.kind() == kind) {
            return Err(UiError::DuplicateStyleProperty(kind));
        }
        self.base.push(ResolvedUiProperty::new(kind, value)?);
        Ok(())
    }

    pub fn set_rule(
        &mut self,
        selector: UiInteractionSelector,
        kind: UiPropertyKind,
        value: UiPropertyValue,
    ) -> Result<(), UiError> {
        if self
            .rules
            .iter()
            .any(|rule| rule.selector() == selector && rule.property().kind() == kind)
        {
            return Err(UiError::DuplicateStyleRule { selector, kind });
        }
        self.rules.push(UiStyleRule::new(selector, kind, value)?);
        Ok(())
    }

    pub fn base(&self) -> &[ResolvedUiProperty] {
        &self.base
    }

    pub fn rules(&self) -> &[UiStyleRule] {
        &self.rules
    }

    pub fn resolve(
        &self,
        target: Option<&InteractionTarget>,
        enabled: bool,
        interaction: &InteractionState,
    ) -> ResolvedUiStyle {
        let mut resolved = ResolvedUiStyle {
            properties: self.base.clone(),
        };
        for selector in UiInteractionSelector::cascade() {
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

impl UiStyleTable {
    pub fn insert(&mut self, id: StyleId, style: UiStyle) -> Result<(), UiError> {
        if self.styles.contains_key(&id) {
            return Err(UiError::DuplicateStyle(id));
        }
        self.styles.insert(id, style);
        Ok(())
    }

    pub fn get(&self, id: StyleId) -> Option<&UiStyle> {
        self.styles.get(&id)
    }

    pub fn resolve(
        &self,
        id: StyleId,
        target: Option<&InteractionTarget>,
        enabled: bool,
        interaction: &InteractionState,
    ) -> Result<ResolvedUiStyle, UiError> {
        if self.is_empty() {
            return Ok(ResolvedUiStyle::default());
        }
        self.get(id)
            .map(|style| style.resolve(target, enabled, interaction))
            .ok_or(UiError::UnknownStyle(id))
    }

    pub fn contains(&self, id: StyleId) -> bool {
        self.styles.contains_key(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }
}

impl ResolvedUiStyle {
    fn set(&mut self, property: ResolvedUiProperty) {
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

    pub fn as_slice(&self) -> &[ResolvedUiProperty] {
        &self.properties
    }

    pub fn value(&self, kind: UiPropertyKind) -> Option<UiPropertyValue> {
        self.properties
            .iter()
            .find(|property| property.kind() == kind)
            .map(|property| property.value())
    }

    pub fn bool(&self, kind: UiPropertyKind) -> Option<bool> {
        self.value(kind).and_then(UiPropertyValue::as_bool)
    }

    pub fn milli(&self, kind: UiPropertyKind) -> Option<Milli> {
        self.value(kind).and_then(UiPropertyValue::as_milli)
    }

    pub fn color(&self, kind: UiPropertyKind) -> Option<Rgba8> {
        self.value(kind).and_then(UiPropertyValue::as_color)
    }

    pub fn is_visible(&self) -> bool {
        self.bool(UiPropertyKind::Visibility).unwrap_or(true)
            && self.bool(UiPropertyKind::Display).unwrap_or(true)
    }

    pub fn opacity(&self) -> Milli {
        self.milli(UiPropertyKind::Opacity).unwrap_or(Milli::ONE)
    }

    pub fn scale(&self) -> Milli {
        self.milli(UiPropertyKind::Scale).unwrap_or(Milli::ONE)
    }
}
