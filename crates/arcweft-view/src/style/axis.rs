//! Closed logical-axis context and canonical physical box projection types.

use super::{ViewLengthMilli, ViewOverflow, ViewStyleContributionSource, ViewStylePriority};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Closed axis progression accepted by native View Style.
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ViewBoxAxisMode {
    #[default]
    HorizontalLtr,
    HorizontalRtl,
    VerticalRl,
    VerticalLr,
}

/// One physical coordinate axis.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewPhysicalAxis {
    X,
    Y,
}

/// One physical box side.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewPhysicalSide {
    Top,
    Right,
    Bottom,
    Left,
}

/// Sign applied to positive displacement along a resolved logical axis.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewAxisSign {
    Positive,
    Negative,
}

/// Validated physical meaning of one logical axis.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewResolvedAxis {
    axis: ViewPhysicalAxis,
    start: ViewPhysicalSide,
    end: ViewPhysicalSide,
    positive_displacement: ViewAxisSign,
}

/// Validated physical meaning of the inline and block axes together.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewResolvedBoxAxes {
    inline: ViewResolvedAxis,
    block: ViewResolvedAxis,
}

/// Why an attempted resolved-axis snapshot is not one of the four supported modes.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ViewBoxAxisModeError {
    #[error("inline and block axes must be orthogonal")]
    NonOrthogonal,
    #[error("axis sides do not match the selected physical axis")]
    InvalidSides,
    #[error("unsupported View box-axis progression")]
    UnsupportedProgression,
}

/// Stable revision of the effective axis provider for a node.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewBoxAxisRevision(u64);

/// Origin of an inherited axis seed before local Style wins are considered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewBoxAxisSeedSource {
    HostDefault,
    HostExplicit,
    Parent,
}

/// Typed axis seed propagated across a retained View parent boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewInheritedBoxAxes {
    mode: ViewBoxAxisMode,
    revision: ViewBoxAxisRevision,
    source: ViewBoxAxisSeedSource,
}

/// Source retained with the effective computed axes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewBoxAxisSource {
    HostDefault,
    HostExplicit,
    Inherited {
        parent: ViewBoxAxisRevision,
    },
    Style {
        priority: ViewStylePriority,
        source: ViewStyleContributionSource,
    },
}

/// Logical property families observed while resolving a computed result.
#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewAxisUsageSet(u16);

/// Physical edge packet shared by computed Style consumers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ViewPhysicalEdges<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

/// Canonical physical box values projected from one computed Style result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPhysicalBoxStyle {
    pub axes: ViewBoxAxisMode,
    pub width: Option<ViewLengthMilli>,
    pub height: Option<ViewLengthMilli>,
    pub min_width: Option<ViewLengthMilli>,
    pub min_height: Option<ViewLengthMilli>,
    pub max_width: Option<ViewLengthMilli>,
    pub max_height: Option<ViewLengthMilli>,
    pub padding: ViewPhysicalEdges<Option<ViewLengthMilli>>,
    pub margin: ViewPhysicalEdges<Option<ViewLengthMilli>>,
    pub inset: ViewPhysicalEdges<Option<ViewLengthMilli>>,
    pub translate_x: ViewLengthMilli,
    pub translate_y: ViewLengthMilli,
    pub overflow_x: ViewOverflow,
    pub overflow_y: ViewOverflow,
}

impl ViewBoxAxisMode {
    pub const ALL: &'static [Self] = &[
        Self::HorizontalLtr,
        Self::HorizontalRtl,
        Self::VerticalRl,
        Self::VerticalLr,
    ];

    /// Canonical case-sensitive source spelling for expected-type shorthand.
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::HorizontalLtr => "HorizontalLtr",
            Self::HorizontalRtl => "HorizontalRtl",
            Self::VerticalRl => "VerticalRl",
            Self::VerticalLr => "VerticalLr",
        }
    }

    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|mode| mode.source_name() == value)
    }

    /// Stable ordinal used by deterministic hashes and codecs.
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::HorizontalLtr => 0,
            Self::HorizontalRtl => 1,
            Self::VerticalRl => 2,
            Self::VerticalLr => 3,
        }
    }

    /// Resolves the closed mode into physical axes, sides, and displacement signs.
    pub const fn resolved(self) -> ViewResolvedBoxAxes {
        const X_LTR: ViewResolvedAxis = ViewResolvedAxis::new(
            ViewPhysicalAxis::X,
            ViewPhysicalSide::Left,
            ViewPhysicalSide::Right,
            ViewAxisSign::Positive,
        );
        const X_RTL: ViewResolvedAxis = ViewResolvedAxis::new(
            ViewPhysicalAxis::X,
            ViewPhysicalSide::Right,
            ViewPhysicalSide::Left,
            ViewAxisSign::Negative,
        );
        const Y_TTB: ViewResolvedAxis = ViewResolvedAxis::new(
            ViewPhysicalAxis::Y,
            ViewPhysicalSide::Top,
            ViewPhysicalSide::Bottom,
            ViewAxisSign::Positive,
        );
        match self {
            Self::HorizontalLtr => ViewResolvedBoxAxes {
                inline: X_LTR,
                block: Y_TTB,
            },
            Self::HorizontalRtl => ViewResolvedBoxAxes {
                inline: X_RTL,
                block: Y_TTB,
            },
            Self::VerticalRl => ViewResolvedBoxAxes {
                inline: Y_TTB,
                block: X_RTL,
            },
            Self::VerticalLr => ViewResolvedBoxAxes {
                inline: Y_TTB,
                block: X_LTR,
            },
        }
    }
}

impl ViewResolvedAxis {
    /// Builds one axis component. Pair-level validation is performed by
    /// [`ViewResolvedBoxAxes::try_new`].
    pub const fn new(
        axis: ViewPhysicalAxis,
        start: ViewPhysicalSide,
        end: ViewPhysicalSide,
        positive_displacement: ViewAxisSign,
    ) -> Self {
        Self {
            axis,
            start,
            end,
            positive_displacement,
        }
    }

    pub const fn axis(self) -> ViewPhysicalAxis {
        self.axis
    }

    pub const fn start(self) -> ViewPhysicalSide {
        self.start
    }

    pub const fn end(self) -> ViewPhysicalSide {
        self.end
    }

    pub const fn positive_displacement(self) -> ViewAxisSign {
        self.positive_displacement
    }

    const fn sides_match_axis(self) -> bool {
        matches!(
            (self.axis, self.start, self.end),
            (
                ViewPhysicalAxis::X,
                ViewPhysicalSide::Left,
                ViewPhysicalSide::Right
            ) | (
                ViewPhysicalAxis::X,
                ViewPhysicalSide::Right,
                ViewPhysicalSide::Left
            ) | (
                ViewPhysicalAxis::Y,
                ViewPhysicalSide::Top,
                ViewPhysicalSide::Bottom
            ) | (
                ViewPhysicalAxis::Y,
                ViewPhysicalSide::Bottom,
                ViewPhysicalSide::Top
            )
        )
    }
}

impl ViewResolvedBoxAxes {
    pub fn try_new(
        inline: ViewResolvedAxis,
        block: ViewResolvedAxis,
    ) -> Result<Self, ViewBoxAxisModeError> {
        if inline.axis == block.axis {
            return Err(ViewBoxAxisModeError::NonOrthogonal);
        }
        if !inline.sides_match_axis() || !block.sides_match_axis() {
            return Err(ViewBoxAxisModeError::InvalidSides);
        }
        let axes = Self { inline, block };
        if ViewBoxAxisMode::ALL
            .iter()
            .any(|mode| mode.resolved() == axes)
        {
            Ok(axes)
        } else {
            Err(ViewBoxAxisModeError::UnsupportedProgression)
        }
    }

    pub const fn mode(self) -> ViewBoxAxisMode {
        match (self.inline.axis, self.inline.start, self.block.start) {
            (ViewPhysicalAxis::X, ViewPhysicalSide::Left, _) => ViewBoxAxisMode::HorizontalLtr,
            (ViewPhysicalAxis::X, _, _) => ViewBoxAxisMode::HorizontalRtl,
            (ViewPhysicalAxis::Y, _, ViewPhysicalSide::Right) => ViewBoxAxisMode::VerticalRl,
            (ViewPhysicalAxis::Y, _, _) => ViewBoxAxisMode::VerticalLr,
        }
    }

    pub const fn inline(self) -> ViewResolvedAxis {
        self.inline
    }

    pub const fn block(self) -> ViewResolvedAxis {
        self.block
    }
}

impl ViewBoxAxisRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ViewInheritedBoxAxes {
    pub const fn new(
        mode: ViewBoxAxisMode,
        revision: ViewBoxAxisRevision,
        source: ViewBoxAxisSeedSource,
    ) -> Self {
        Self {
            mode,
            revision,
            source,
        }
    }

    pub const fn host_default() -> Self {
        Self::new(
            ViewBoxAxisMode::HorizontalLtr,
            ViewBoxAxisRevision::new(0),
            ViewBoxAxisSeedSource::HostDefault,
        )
    }

    pub const fn mode(self) -> ViewBoxAxisMode {
        self.mode
    }

    pub const fn revision(self) -> ViewBoxAxisRevision {
        self.revision
    }

    pub const fn source(self) -> ViewBoxAxisSeedSource {
        self.source
    }
}

impl ViewAxisUsageSet {
    pub const NONE: Self = Self(0);
    pub const SIZE: Self = Self(1 << 0);
    pub const MIN_MAX_SIZE: Self = Self(1 << 1);
    pub const SPACING: Self = Self(1 << 2);
    pub const INSET: Self = Self(1 << 3);
    pub const TRANSLATION: Self = Self(1 << 4);
    pub const OVERFLOW: Self = Self(1 << 5);
    pub const TRANSITION_TARGET: Self = Self(1 << 6);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl Default for ViewPhysicalBoxStyle {
    fn default() -> Self {
        Self {
            axes: ViewBoxAxisMode::default(),
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            padding: ViewPhysicalEdges::default(),
            margin: ViewPhysicalEdges::default(),
            inset: ViewPhysicalEdges::default(),
            translate_x: ViewLengthMilli::new(0),
            translate_y: ViewLengthMilli::new(0),
            overflow_x: ViewOverflow::Visible,
            overflow_y: ViewOverflow::Visible,
        }
    }
}
