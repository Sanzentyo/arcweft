//! Closed Fx targets, phases, renderer interfaces, and capability inventories.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Closed application target vocabulary.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FxTarget {
    Node = 0,
    #[default]
    Content = 1,
    Background = 2,
    Line = 3,
    Glyph = 4,
    Viewport = 5,
}

/// Closed renderer contract used in ABI hashes and capability resolution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FxRendererInterface {
    TextStyle = 0,
    Color = 1,
    Transform = 2,
    Mask = 3,
    Filter = 4,
    ShaderUniform = 5,
    OffscreenPass = 6,
    PostProcess = 7,
    Transition = 8,
    GeometryTransform = 9,
}

/// Fixed evaluation and renderer submission phases.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FxPhase {
    BeforeLayout = 0,
    LayoutTransform = 1,
    GlyphTransform = 2,
    GlyphColor = 3,
    GlyphMask = 4,
    OffscreenPass = 5,
    PostProcess = 6,
    Transition = 7,
}

/// Deterministic set of renderer interfaces.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FxRendererInterfaceSet(BTreeSet<FxRendererInterface>);

/// One exact target/interface capability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxCapability {
    pub target: FxTarget,
    pub interface: FxRendererInterface,
}

/// Renderer/provider capability inventory. Unsupported pairs are observable errors.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FxCapabilitySet(BTreeSet<FxCapability>);

impl FxRendererInterfaceSet {
    pub fn new(values: impl IntoIterator<Item = FxRendererInterface>) -> Self {
        Self(values.into_iter().collect())
    }

    pub fn contains(&self, interface: FxRendererInterface) -> bool {
        self.0.contains(&interface)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = FxRendererInterface> + '_ {
        self.0.iter().copied()
    }

    pub fn insert(&mut self, interface: FxRendererInterface) -> bool {
        self.0.insert(interface)
    }
}

impl FxCapabilitySet {
    pub fn new(values: impl IntoIterator<Item = FxCapability>) -> Self {
        Self(values.into_iter().collect())
    }

    /// Full capability contract described by the shared Arcweft renderer model.
    pub fn canonical() -> Self {
        let mut values = BTreeSet::new();
        for target in [
            FxTarget::Node,
            FxTarget::Content,
            FxTarget::Background,
            FxTarget::Line,
            FxTarget::Glyph,
            FxTarget::Viewport,
        ] {
            for interface in [
                FxRendererInterface::TextStyle,
                FxRendererInterface::Color,
                FxRendererInterface::Transform,
                FxRendererInterface::Mask,
                FxRendererInterface::Filter,
                FxRendererInterface::ShaderUniform,
                FxRendererInterface::OffscreenPass,
                FxRendererInterface::PostProcess,
                FxRendererInterface::Transition,
                FxRendererInterface::GeometryTransform,
            ] {
                if canonical_supports(target, interface) {
                    values.insert(FxCapability { target, interface });
                }
            }
        }
        Self(values)
    }

    pub fn supports(&self, target: FxTarget, interface: FxRendererInterface) -> bool {
        self.0.contains(&FxCapability { target, interface })
    }

    pub fn insert(&mut self, target: FxTarget, interface: FxRendererInterface) -> bool {
        self.0.insert(FxCapability { target, interface })
    }
}

fn canonical_supports(target: FxTarget, interface: FxRendererInterface) -> bool {
    use FxRendererInterface as Interface;
    match target {
        FxTarget::Node => matches!(
            interface,
            Interface::TextStyle
                | Interface::Color
                | Interface::Transform
                | Interface::GeometryTransform
                | Interface::Mask
                | Interface::Filter
                | Interface::ShaderUniform
                | Interface::OffscreenPass
                | Interface::Transition
        ),
        FxTarget::Content => matches!(
            interface,
            Interface::TextStyle
                | Interface::Color
                | Interface::Transform
                | Interface::GeometryTransform
                | Interface::Mask
                | Interface::Filter
                | Interface::ShaderUniform
                | Interface::OffscreenPass
        ),
        FxTarget::Background => matches!(
            interface,
            Interface::Color
                | Interface::Transform
                | Interface::Mask
                | Interface::Filter
                | Interface::ShaderUniform
                | Interface::OffscreenPass
        ),
        FxTarget::Line => matches!(
            interface,
            Interface::TextStyle
                | Interface::Color
                | Interface::Transform
                | Interface::Mask
                | Interface::ShaderUniform
                | Interface::OffscreenPass
        ),
        FxTarget::Glyph => matches!(
            interface,
            Interface::TextStyle
                | Interface::Color
                | Interface::Transform
                | Interface::Mask
                | Interface::ShaderUniform
        ),
        FxTarget::Viewport => matches!(
            interface,
            Interface::Transform
                | Interface::GeometryTransform
                | Interface::ShaderUniform
                | Interface::PostProcess
                | Interface::Transition
        ),
    }
}
