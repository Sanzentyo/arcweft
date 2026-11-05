//! Closed presentation-owned `RichText` authoring schemas.

mod direct_style;
mod layout;
mod object;
mod style;
mod transform;

pub use direct_style::{RichTextDirectStyle, RichTextDirectStyleProperty};
pub use layout::{RichTextLayoutProperty, RichTextLayoutSelector};
pub use object::{RichTextObjectProperty, RichTextObjectSelector};
pub use style::{RichTextStyleProperty, RichTextStyleSelector};
pub use transform::{RichTextTransformProperty, RichTextTransformSelector};

/// One font-size policy shared by direct styles and Content callable values.
pub(super) const FONT_SIZE_LIMITS: arcweft_rich_text_schema::RichTextValueLimits =
    arcweft_rich_text_schema::RichTextValueLimits {
        numeric: Some(arcweft_rich_text_schema::RichTextNumericLimits {
            inclusive_min_milli: Some(1_000),
            inclusive_max_milli: Some(512_000),
            max_integer_digits: 19,
            max_fraction_digits: 3,
        }),
        units: &[
            arcweft_rich_text_schema::RichTextUnit::Px,
            arcweft_rich_text_schema::RichTextUnit::Pt,
        ],
        enum_values: &[],
        max_encoded_bytes: 64,
        max_decoded_bytes: 64,
    };
