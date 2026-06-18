//! Style property bindings and invalidation metadata for retained UI fragments.

use crate::{DirtyFlags, UiError};
use std::collections::BTreeSet;

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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiPropertyKind {
    Opacity,
    TranslateX,
    TranslateY,
    Scale,
    Rotate,
    Color,
    BackgroundColor,
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
            | Self::Visibility
            | Self::Custom(_) => Invalidation::Paint,
            Self::Width | Self::Height | Self::Display => Invalidation::Layout,
            Self::SemanticLabel => Invalidation::Semantics,
            Self::StructuralCondition => Invalidation::Fragment,
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
