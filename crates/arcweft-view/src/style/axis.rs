//! Closed logical-axis context and canonical physical box projection types.

use super::resolver::ViewStyleNodeKey;
use super::{
    ViewDisplay, ViewFlexDirection, ViewLengthMilli, ViewOverflow, ViewPosition, ViewScalarMilli,
    ViewStyleContributionSource, ViewStylePriority,
};
use crate::ViewMountId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const AXIS_REVISION_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const AXIS_REVISION_PRIME: u64 = 0x0000_0100_0000_01b3;
const HOST_AXIS_REVISION_DOMAIN: &[u8] = b"arcweft.view-axis.host.v1";
const LOCAL_AXIS_REVISION_DOMAIN: &[u8] = b"arcweft.view-axis.local.v1";

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
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct ViewBoxAxisRevision(u64);

/// Host-owned intent used to seed one top-level View mount.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(
    deny_unknown_fields,
    tag = "kind",
    content = "mode",
    rename_all = "snake_case"
)]
pub enum ViewBoxAxisHostSeed {
    #[default]
    Default,
    Explicit(ViewBoxAxisMode),
}

/// Monotonic identity generation owned by one root View mount.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct ViewBoxAxisSeedGeneration(u64);

/// Failure to allocate the next host-seed generation without wrapping.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ViewBoxAxisSeedGenerationError {
    #[error("View box-axis seed generation is exhausted")]
    Exhausted,
}

/// Origin of an inherited axis seed before local Style wins are considered.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewBoxAxisSeedSource {
    HostDefault,
    HostExplicit,
    Parent,
}

/// Typed axis seed propagated across a retained View parent boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPhysicalEdges<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

/// Canonical physical box values projected from one computed Style result.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPhysicalBoxStyle {
    pub axes: ViewBoxAxisMode,
    pub display: Option<ViewDisplay>,
    pub position: ViewPosition,
    pub width: Option<ViewLengthMilli>,
    pub height: Option<ViewLengthMilli>,
    pub min_width: Option<ViewLengthMilli>,
    pub min_height: Option<ViewLengthMilli>,
    pub max_width: Option<ViewLengthMilli>,
    pub max_height: Option<ViewLengthMilli>,
    pub padding: ViewPhysicalEdges<ViewLengthMilli>,
    pub border: ViewPhysicalEdges<ViewLengthMilli>,
    pub margin: ViewPhysicalEdges<ViewLengthMilli>,
    pub inset: ViewPhysicalEdges<Option<ViewLengthMilli>>,
    pub translate_x: ViewLengthMilli,
    pub translate_y: ViewLengthMilli,
    pub scale: ViewScalarMilli,
    pub overflow_x: ViewOverflow,
    pub overflow_y: ViewOverflow,
}

/// Canonical physical container values projected from one computed Style result.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPhysicalContainerStyle {
    pub flow: ViewPhysicalFlow,
    pub row_gap: ViewLengthMilli,
    pub column_gap: ViewLengthMilli,
}

/// One-dimensional physical flow used by native View geometry.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewPhysicalFlow {
    #[default]
    Overlay,
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

impl<T> ViewPhysicalEdges<T> {
    pub const fn new(top: T, right: T, bottom: T, left: T) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
}

impl<T: Copy> ViewPhysicalEdges<T> {
    pub const fn all(value: T) -> Self {
        Self::new(value, value, value, value)
    }

    pub const fn start(self, axis: ViewPhysicalAxis) -> T {
        match axis {
            ViewPhysicalAxis::X => self.left,
            ViewPhysicalAxis::Y => self.top,
        }
    }

    pub const fn end(self, axis: ViewPhysicalAxis) -> T {
        match axis {
            ViewPhysicalAxis::X => self.right,
            ViewPhysicalAxis::Y => self.bottom,
        }
    }
}

impl ViewPhysicalFlow {
    /// Stable ordinal used by deterministic hashes and codecs.
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::Overlay => 0,
            Self::Row => 1,
            Self::RowReverse => 2,
            Self::Column => 3,
            Self::ColumnReverse => 4,
        }
    }

    pub const fn from_flex_direction(direction: ViewFlexDirection) -> Self {
        match direction {
            ViewFlexDirection::Row => Self::Row,
            ViewFlexDirection::RowReverse => Self::RowReverse,
            ViewFlexDirection::Column => Self::Column,
            ViewFlexDirection::ColumnReverse => Self::ColumnReverse,
        }
    }

    pub const fn main_axis(self) -> Option<ViewPhysicalAxis> {
        match self {
            Self::Overlay => None,
            Self::Row | Self::RowReverse => Some(ViewPhysicalAxis::X),
            Self::Column | Self::ColumnReverse => Some(ViewPhysicalAxis::Y),
        }
    }

    pub const fn is_reverse(self) -> bool {
        matches!(self, Self::RowReverse | Self::ColumnReverse)
    }
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
    pub(crate) const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    /// Derives the stable provider identity for one root mount host seed.
    pub fn for_host_seed(
        mount: ViewMountId,
        generation: ViewBoxAxisSeedGeneration,
        seed: ViewBoxAxisHostSeed,
    ) -> Self {
        let mut transcript = AxisRevisionTranscript::new();
        transcript.length_prefixed(HOST_AXIS_REVISION_DOMAIN);
        transcript.u64(mount.get());
        transcript.u64(generation.value());
        transcript.u8(match seed {
            ViewBoxAxisHostSeed::Default => 0,
            ViewBoxAxisHostSeed::Explicit(_) => 1,
        });
        transcript.u8(seed.mode().canonical_tag());
        Self(transcript.finish())
    }

    pub(crate) fn for_local_provider(
        node: &ViewStyleNodeKey,
        mode: ViewBoxAxisMode,
        priority: ViewStylePriority,
        source: &ViewStyleContributionSource,
    ) -> Self {
        let mut transcript = AxisRevisionTranscript::new();
        transcript.length_prefixed(LOCAL_AXIS_REVISION_DOMAIN);
        transcript.u64(node.mount().get());
        transcript.u64(node.path().len() as u64);
        for segment in node.path() {
            transcript.u64(*segment);
        }
        transcript.u32(node.instruction());
        transcript.u8(mode.canonical_tag());
        transcript.u16(priority.scope_depth());
        transcript.u32(priority.application_order());
        transcript.u16(priority.specificity().0);
        transcript.u16(priority.specificity().1);
        transcript.u32(priority.rule_source_order());
        transcript.u32(priority.declaration_order());
        match source {
            ViewStyleContributionSource::Inherited => transcript.u8(0),
            ViewStyleContributionSource::Sheet {
                sheet,
                rule,
                declaration,
            } => {
                transcript.u8(1);
                transcript.length_prefixed(sheet.public_id().as_str().as_bytes());
                transcript.u32(rule.value());
                transcript.u32(declaration.value());
            }
            ViewStyleContributionSource::Patch { patch, declaration } => {
                transcript.u8(2);
                transcript.u32(patch.value());
                transcript.u32(declaration.value());
            }
        }
        Self(transcript.finish())
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ViewBoxAxisHostSeed {
    pub const fn mode(self) -> ViewBoxAxisMode {
        match self {
            Self::Default => ViewBoxAxisMode::HorizontalLtr,
            Self::Explicit(mode) => mode,
        }
    }

    pub const fn source(self) -> ViewBoxAxisSeedSource {
        match self {
            Self::Default => ViewBoxAxisSeedSource::HostDefault,
            Self::Explicit(_) => ViewBoxAxisSeedSource::HostExplicit,
        }
    }
}

impl ViewBoxAxisSeedGeneration {
    pub const INITIAL: Self = Self(0);

    pub const fn value(self) -> u64 {
        self.0
    }

    pub fn checked_next(self) -> Result<Self, ViewBoxAxisSeedGenerationError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(ViewBoxAxisSeedGenerationError::Exhausted)
    }
}

impl ViewInheritedBoxAxes {
    pub(crate) const fn from_raw(
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

    pub fn for_host_seed(
        mount: ViewMountId,
        generation: ViewBoxAxisSeedGeneration,
        seed: ViewBoxAxisHostSeed,
    ) -> Self {
        Self::from_raw(
            seed.mode(),
            ViewBoxAxisRevision::for_host_seed(mount, generation, seed),
            seed.source(),
        )
    }

    pub const fn from_parent(mode: ViewBoxAxisMode, revision: ViewBoxAxisRevision) -> Self {
        Self::from_raw(mode, revision, ViewBoxAxisSeedSource::Parent)
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

struct AxisRevisionTranscript {
    value: u64,
}

impl AxisRevisionTranscript {
    const fn new() -> Self {
        Self {
            value: AXIS_REVISION_OFFSET_BASIS,
        }
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.value ^= u64::from(*byte);
            self.value = self.value.wrapping_mul(AXIS_REVISION_PRIME);
        }
    }

    fn length_prefixed(&mut self, bytes: &[u8]) {
        self.u64(bytes.len() as u64);
        self.bytes(bytes);
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&value.to_le_bytes());
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    const fn finish(self) -> u64 {
        self.value
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
            display: None,
            position: ViewPosition::Static,
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            padding: ViewPhysicalEdges::all(ViewLengthMilli::new(0)),
            border: ViewPhysicalEdges::all(ViewLengthMilli::new(0)),
            margin: ViewPhysicalEdges::all(ViewLengthMilli::new(0)),
            inset: ViewPhysicalEdges::default(),
            translate_x: ViewLengthMilli::new(0),
            translate_y: ViewLengthMilli::new(0),
            scale: ViewScalarMilli::ONE,
            overflow_x: ViewOverflow::Visible,
            overflow_y: ViewOverflow::Visible,
        }
    }
}

impl Default for ViewPhysicalContainerStyle {
    fn default() -> Self {
        Self {
            flow: ViewPhysicalFlow::Overlay,
            row_gap: ViewLengthMilli::new(0),
            column_gap: ViewLengthMilli::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{ViewStylePatchId, ViewStyleSheetId, ViewStyleSourceId};

    #[test]
    fn local_provider_revisions_match_the_canonical_transcript() {
        let patch_node = ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 0);
        let zero_priority = ViewStylePriority::new(0, 0, 0, 0, 0, 0);
        let zero_patch = ViewStyleContributionSource::Patch {
            patch: ViewStylePatchId::new(0),
            declaration: ViewStyleSourceId::new(0),
        };
        let stable = ViewBoxAxisRevision::for_local_provider(
            &patch_node,
            ViewBoxAxisMode::HorizontalLtr,
            zero_priority,
            &zero_patch,
        );
        assert_eq!(stable.value(), 0xeb85_2c36_ca94_9613);
        assert_eq!(
            ViewBoxAxisRevision::for_local_provider(
                &patch_node,
                ViewBoxAxisMode::HorizontalLtr,
                zero_priority,
                &zero_patch,
            ),
            stable
        );
        assert_ne!(
            ViewBoxAxisRevision::for_local_provider(
                &ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 1),
                ViewBoxAxisMode::HorizontalLtr,
                zero_priority,
                &zero_patch,
            ),
            stable
        );
        assert_ne!(
            ViewBoxAxisRevision::for_local_provider(
                &patch_node,
                ViewBoxAxisMode::HorizontalLtr,
                ViewStylePriority::new(0, 1, 0, 0, 0, 0),
                &zero_patch,
            ),
            stable
        );
        assert_ne!(
            ViewBoxAxisRevision::for_local_provider(
                &patch_node,
                ViewBoxAxisMode::HorizontalLtr,
                zero_priority,
                &ViewStyleContributionSource::Patch {
                    patch: ViewStylePatchId::new(1),
                    declaration: ViewStyleSourceId::new(0),
                },
            ),
            stable
        );

        let sheet_node = ViewStyleNodeKey::new(ViewMountId::from_raw(7), vec![10, 20], 42);
        assert_eq!(
            ViewBoxAxisRevision::for_local_provider(
                &sheet_node,
                ViewBoxAxisMode::VerticalRl,
                ViewStylePriority::new(1, 2, 3, 4, 5, 6),
                &ViewStyleContributionSource::Sheet {
                    sheet: ViewStyleSheetId::try_new("main-style").unwrap(),
                    rule: ViewStyleSourceId::new(7),
                    declaration: ViewStyleSourceId::new(8),
                },
            )
            .value(),
            0x6298_9f22_c1b5_63c5
        );
    }
}
