//! Checked complete-source, display-stage, and prepared-document coordinates.
//! Range behavior belongs to the admitted owning context.

use crate::RichTextRange;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Owns the exact source extents of one display and its prepared projection.
/// Geometry keeps source coordinates; transported element ranges use display
/// coordinates; interaction strings use prepared coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectedTextRangeDomain {
    display_source_range: RichTextRange,
    prepared_source_range: RichTextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectedTextCoordinate {
    Source,
    Display,
    Prepared,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ProjectedTextRangeError {
    #[error("text range {start}..{end} is reversed")]
    Reversed { start: usize, end: usize },
    #[error("prepared source extent {prepared:?} is outside display source extent {display:?}")]
    PreparedOutsideDisplay {
        display: RichTextRange,
        prepared: RichTextRange,
    },
    #[error("{coordinate:?} text range {range:?} is outside extent {extent:?}")]
    OutsideExtent {
        coordinate: ProjectedTextCoordinate,
        range: RichTextRange,
        extent: RichTextRange,
    },
    #[error("{coordinate:?} origin {origin} plus extent {length} overflows")]
    Overflow {
        coordinate: ProjectedTextCoordinate,
        origin: usize,
        length: usize,
    },
    #[error("{coordinate:?} text has {actual} bytes, expected {expected}")]
    TextLength {
        coordinate: ProjectedTextCoordinate,
        expected: usize,
        actual: usize,
    },
    #[error("{coordinate:?} offset {offset} splits a UTF-8 scalar")]
    Utf8Boundary {
        coordinate: ProjectedTextCoordinate,
        offset: usize,
    },
    #[error("prepared text differs from the admitted display slice")]
    PreparedTextMismatch,
}

impl ProjectedTextRangeDomain {
    pub fn try_new(
        display_source_range: RichTextRange,
        prepared_source_range: RichTextRange,
    ) -> Result<Self, ProjectedTextRangeError> {
        ordered(display_source_range)?;
        ordered(prepared_source_range)?;
        if !contains(display_source_range, prepared_source_range) {
            return Err(ProjectedTextRangeError::PreparedOutsideDisplay {
                display: display_source_range,
                prepared: prepared_source_range,
            });
        }
        Ok(Self {
            display_source_range,
            prepared_source_range,
        })
    }

    pub fn try_from_lengths(
        display_origin: usize,
        display_length: usize,
        prepared_origin: usize,
        prepared_length: usize,
    ) -> Result<Self, ProjectedTextRangeError> {
        let display = source_extent(display_origin, display_length)?;
        let prepared = source_extent(prepared_origin, prepared_length)?;
        Self::try_new(display, prepared)
    }

    pub fn document(origin: usize, length: usize) -> Result<Self, ProjectedTextRangeError> {
        let extent = source_extent(origin, length)?;
        Self::try_new(extent, extent)
    }

    #[must_use]
    pub const fn display_source_range(self) -> RichTextRange {
        self.display_source_range
    }

    #[must_use]
    pub const fn prepared_source_range(self) -> RichTextRange {
        self.prepared_source_range
    }

    #[must_use]
    pub const fn display_origin(self) -> usize {
        self.display_source_range.start
    }

    #[must_use]
    pub const fn prepared_origin(self) -> usize {
        self.prepared_source_range.start
    }

    #[must_use]
    pub const fn display_range(self) -> RichTextRange {
        RichTextRange::new(
            0,
            self.display_source_range.end - self.display_source_range.start,
        )
    }

    #[must_use]
    pub const fn prepared_range(self) -> RichTextRange {
        RichTextRange::new(
            0,
            self.prepared_source_range.end - self.prepared_source_range.start,
        )
    }

    pub fn source_to_display(
        self,
        source: RichTextRange,
    ) -> Result<RichTextRange, ProjectedTextRangeError> {
        checked_extent(
            ProjectedTextCoordinate::Source,
            source,
            self.display_source_range,
        )?;
        Ok(subtract_origin(source, self.display_origin()))
    }

    pub fn display_to_source(
        self,
        display: RichTextRange,
    ) -> Result<RichTextRange, ProjectedTextRangeError> {
        checked_extent(
            ProjectedTextCoordinate::Display,
            display,
            self.display_range(),
        )?;
        add_origin(display, self.display_origin())
    }

    pub fn source_to_prepared(
        self,
        source: RichTextRange,
    ) -> Result<RichTextRange, ProjectedTextRangeError> {
        checked_extent(
            ProjectedTextCoordinate::Source,
            source,
            self.prepared_source_range,
        )?;
        Ok(subtract_origin(source, self.prepared_origin()))
    }

    pub fn prepared_to_source(
        self,
        prepared: RichTextRange,
    ) -> Result<RichTextRange, ProjectedTextRangeError> {
        checked_extent(
            ProjectedTextCoordinate::Prepared,
            prepared,
            self.prepared_range(),
        )?;
        add_origin(prepared, self.prepared_origin())
    }

    pub fn display_to_prepared(
        self,
        display: RichTextRange,
    ) -> Result<RichTextRange, ProjectedTextRangeError> {
        self.source_to_prepared(self.display_to_source(display)?)
    }

    pub fn prepared_to_display(
        self,
        prepared: RichTextRange,
    ) -> Result<RichTextRange, ProjectedTextRangeError> {
        self.source_to_display(self.prepared_to_source(prepared)?)
    }

    /// Bind the domain to its actual strings once at context admission.
    pub fn validate_texts(
        self,
        display_text: &str,
        prepared_text: &str,
    ) -> Result<(), ProjectedTextRangeError> {
        self.validate_display_range(self.display_range(), display_text)?;
        text_length(
            ProjectedTextCoordinate::Prepared,
            self.prepared_range(),
            prepared_text,
        )?;
        let retained = self.source_to_display(self.prepared_source_range)?;
        self.validate_display_range(retained, display_text)?;
        if display_text.get(retained.start..retained.end) != Some(prepared_text) {
            return Err(ProjectedTextRangeError::PreparedTextMismatch);
        }
        Ok(())
    }

    pub fn validate_display_range(
        self,
        range: RichTextRange,
        display_text: &str,
    ) -> Result<(), ProjectedTextRangeError> {
        checked_extent(
            ProjectedTextCoordinate::Display,
            range,
            self.display_range(),
        )?;
        text_length(
            ProjectedTextCoordinate::Display,
            self.display_range(),
            display_text,
        )?;
        boundaries(ProjectedTextCoordinate::Display, range, display_text)
    }

    pub fn validate_prepared_range(
        self,
        range: RichTextRange,
        prepared_text: &str,
    ) -> Result<(), ProjectedTextRangeError> {
        checked_extent(
            ProjectedTextCoordinate::Prepared,
            range,
            self.prepared_range(),
        )?;
        text_length(
            ProjectedTextCoordinate::Prepared,
            self.prepared_range(),
            prepared_text,
        )?;
        boundaries(ProjectedTextCoordinate::Prepared, range, prepared_text)
    }
}

fn ordered(range: RichTextRange) -> Result<(), ProjectedTextRangeError> {
    if range.start > range.end {
        Err(ProjectedTextRangeError::Reversed {
            start: range.start,
            end: range.end,
        })
    } else {
        Ok(())
    }
}

const fn contains(outer: RichTextRange, inner: RichTextRange) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn checked_extent(
    coordinate: ProjectedTextCoordinate,
    range: RichTextRange,
    extent: RichTextRange,
) -> Result<(), ProjectedTextRangeError> {
    ordered(range)?;
    if contains(extent, range) {
        Ok(())
    } else {
        Err(ProjectedTextRangeError::OutsideExtent {
            coordinate,
            range,
            extent,
        })
    }
}

fn source_extent(origin: usize, length: usize) -> Result<RichTextRange, ProjectedTextRangeError> {
    let end = origin
        .checked_add(length)
        .ok_or(ProjectedTextRangeError::Overflow {
            coordinate: ProjectedTextCoordinate::Source,
            origin,
            length,
        })?;
    Ok(RichTextRange::new(origin, end))
}

const fn subtract_origin(range: RichTextRange, origin: usize) -> RichTextRange {
    // Called only after the admitted source extent contains both endpoints.
    RichTextRange::new(range.start - origin, range.end - origin)
}

fn add_origin(
    range: RichTextRange,
    origin: usize,
) -> Result<RichTextRange, ProjectedTextRangeError> {
    let end = origin
        .checked_add(range.end)
        .ok_or(ProjectedTextRangeError::Overflow {
            coordinate: ProjectedTextCoordinate::Source,
            origin,
            length: range.end,
        })?;
    // Ordered endpoints and checked end prove the start addition is in range.
    Ok(RichTextRange::new(range.start + origin, end))
}

fn text_length(
    coordinate: ProjectedTextCoordinate,
    extent: RichTextRange,
    text: &str,
) -> Result<(), ProjectedTextRangeError> {
    let expected = extent.end - extent.start;
    if text.len() == expected {
        Ok(())
    } else {
        Err(ProjectedTextRangeError::TextLength {
            coordinate,
            expected,
            actual: text.len(),
        })
    }
}

fn boundaries(
    coordinate: ProjectedTextCoordinate,
    range: RichTextRange,
    text: &str,
) -> Result<(), ProjectedTextRangeError> {
    for offset in [range.start, range.end] {
        if !text.is_char_boundary(offset) {
            return Err(ProjectedTextRangeError::Utf8Boundary { coordinate, offset });
        }
    }
    Ok(())
}

impl<'de> Deserialize<'de> for ProjectedTextRangeDomain {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            display_source_range: RichTextRange,
            prepared_source_range: RichTextRange,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::try_new(wire.display_source_range, wire.prepared_source_range)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_keeps_display_and_prepared_origins_distinct() {
        let domain = ProjectedTextRangeDomain::try_from_lengths(0, 11, 6, 5).unwrap();
        domain.validate_texts("BeforeAfter", "After").unwrap();
        let source = RichTextRange::new(6, 11);
        let display = domain.source_to_display(source).unwrap();
        let prepared = domain.source_to_prepared(source).unwrap();
        assert_eq!(display, source);
        assert_eq!(prepared, RichTextRange::new(0, 5));
        assert_eq!(domain.display_to_source(display).unwrap(), source);
        assert_eq!(domain.prepared_to_source(prepared).unwrap(), source);
        assert_eq!(domain.display_to_prepared(display).unwrap(), prepared);
        assert_eq!(domain.prepared_to_display(prepared).unwrap(), display);
    }

    #[test]
    fn following_page_rebases_display_once_and_retains_clear_clipping() {
        let domain = ProjectedTextRangeDomain::try_from_lengths(6, 8, 9, 5).unwrap();
        domain.validate_texts("前After", "After").unwrap();
        let source = RichTextRange::new(9, 14);
        assert_eq!(
            domain.source_to_display(source).unwrap(),
            RichTextRange::new(3, 8)
        );
        assert_eq!(
            domain.display_to_source(RichTextRange::new(3, 8)).unwrap(),
            source
        );
        assert_eq!(
            domain.source_to_prepared(source).unwrap(),
            RichTextRange::new(0, 5)
        );
        assert_eq!(
            domain.prepared_to_source(RichTextRange::new(0, 5)).unwrap(),
            source
        );
    }

    #[test]
    fn plain_view_and_empty_documents_use_their_actual_extents() {
        let view = ProjectedTextRangeDomain::document(12, 5).unwrap();
        view.validate_texts("After", "After").unwrap();
        assert_eq!(
            view.source_to_display(RichTextRange::new(12, 17)).unwrap(),
            RichTextRange::new(0, 5)
        );
        let empty = ProjectedTextRangeDomain::document(17, 0).unwrap();
        empty.validate_texts("", "").unwrap();
        assert_eq!(
            empty.prepared_to_source(RichTextRange::new(0, 0)).unwrap(),
            RichTextRange::new(17, 17)
        );
    }

    #[test]
    fn bounds_order_and_overflow_fail_without_saturation() {
        assert!(matches!(
            ProjectedTextRangeDomain::try_new(RichTextRange::new(0, 8), RichTextRange::new(7, 9)),
            Err(ProjectedTextRangeError::PreparedOutsideDisplay { .. })
        ));
        assert!(matches!(
            ProjectedTextRangeDomain::try_from_lengths(usize::MAX, 1, usize::MAX, 0),
            Err(ProjectedTextRangeError::Overflow { .. })
        ));
        let domain = ProjectedTextRangeDomain::document(8, 5).unwrap();
        assert!(matches!(
            domain.source_to_display(RichTextRange::new(7, 8)),
            Err(ProjectedTextRangeError::OutsideExtent { .. })
        ));
        assert!(matches!(
            domain.display_to_source(RichTextRange::new(0, 6)),
            Err(ProjectedTextRangeError::OutsideExtent { .. })
        ));
        assert!(matches!(
            domain.prepared_to_source(RichTextRange::new(4, 2)),
            Err(ProjectedTextRangeError::Reversed { .. })
        ));
    }

    #[test]
    fn admitted_strings_and_utf8_boundaries_are_checked() {
        let domain = ProjectedTextRangeDomain::try_from_lengths(10, 8, 13, 5).unwrap();
        domain.validate_texts("前After", "After").unwrap();
        assert!(matches!(
            domain.validate_display_range(RichTextRange::new(1, 3), "前After"),
            Err(ProjectedTextRangeError::Utf8Boundary { .. })
        ));
        assert!(matches!(
            domain.validate_texts("前After", "Other"),
            Err(ProjectedTextRangeError::PreparedTextMismatch)
        ));
        assert!(matches!(
            domain.validate_texts("After", "After"),
            Err(ProjectedTextRangeError::TextLength { .. })
        ));
    }

    #[test]
    fn transport_admits_only_the_final_checked_extent_shape() {
        let domain = ProjectedTextRangeDomain::try_from_lengths(0, 11, 6, 5).unwrap();
        let value = serde_json::to_value(domain).unwrap();
        assert_eq!(
            serde_json::from_value::<ProjectedTextRangeDomain>(value.clone()).unwrap(),
            domain
        );
        let mut invalid = value;
        invalid["prepared_source_range"]["end"] = serde_json::json!(12);
        assert!(serde_json::from_value::<ProjectedTextRangeDomain>(invalid).is_err());
        assert!(
            serde_json::from_str::<ProjectedTextRangeDomain>(
                r#"{"display_origin":0,"prepared_origin":6}"#
            )
            .is_err()
        );
    }
}
