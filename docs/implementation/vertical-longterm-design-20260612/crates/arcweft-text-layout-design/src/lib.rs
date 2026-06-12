#![forbid(unsafe_code)]
//! Design skeleton for Arcweft's long-term Sans I/O text layout engine.
//!
//! The code is intentionally small, deterministic, and dependency-light. It is
//! not a production text engine; it captures the stable API boundaries and the
//! shape of difficult algorithms such as vertical orientation, line breaking,
//! text-combine grouping, and hit-testing.

pub mod hit_test;
pub mod line_break;
pub mod model;
pub mod pipeline;
pub mod segmentation;
pub mod shaping;
pub mod style;
pub mod unicode_orientation;

pub use hit_test::{HitMap, HitTestResult};
pub use line_break::{LineBreak, LayoutItem, break_lines_dp};
pub use model::{LaidOutText, LineBox, PlacedGlyph};
pub use pipeline::{ParagraphLayoutInput, layout_paragraph};
pub use segmentation::{OrientedCluster, segment_and_orient};
pub use shaping::{MonospaceShaper, ShapePlanRun, ShapedGlyph, ShapingBackend};
pub use style::{InlineDirection, RubyPosition, TextLayoutStyle, TextOrientation, WritingMode};
pub use unicode_orientation::{ResolvedOrientation, VerticalOrientation, vertical_orientation};
