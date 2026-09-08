//! Closed value inventories used by presentation-owned content callables.

use arcweft_rich_text_schema::RichTextEnumDomain;

/// Inline direction values accepted by a layout content callable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LayoutDirection {
    /// Let the active layout context choose the direction.
    Auto,
    /// Left-to-right inline progression.
    Ltr,
    /// Right-to-left inline progression.
    Rtl,
}

/// Latin glyph orientation values accepted by vertical layout.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VerticalLatin {
    /// Use the mixed-script orientation table.
    Mixed,
    /// Keep Latin glyphs upright.
    Upright,
    /// Rotate Latin glyphs sideways.
    Sideways,
}

/// JLREQ punctuation-pair planning preset.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Jlreq {
    /// Inherit the active layout preset.
    Auto,
    /// Looser punctuation pairing.
    Loose,
    /// Balanced narrative preset.
    Normal,
    /// Strict punctuation pairing.
    Strict,
}

/// Target values accepted by a transform content callable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TransformTarget {
    /// The enclosing presentation node.
    Node,
    /// The content descendants of the enclosing node.
    Content,
    /// The background presentation cell.
    Background,
    /// The laid-out line.
    Line,
    /// One glyph or glyph run.
    Glyph,
    /// The output viewport.
    Viewport,
}

/// Pivot values accepted by a transform content callable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TransformOrigin {
    /// Start of the baseline.
    BaselineStart,
    /// Center of the baseline.
    BaselineCenter,
    /// Geometric center of the target.
    Center,
    /// Center of the glyph bounds.
    GlyphCenter,
}

macro_rules! inventory_impl {
    (
        $ty:ident,
        $domain:expr,
        $count:expr,
        [$($variant:ident => $ordinal:expr => $name:literal),+ $(,)?]
    ) => {
        impl $ty {
            /// Complete deterministic inventory in canonical source order.
            pub const ALL: [Self; $count] = [
                $(Self::$variant),+
            ];

            /// Stable zero-based member ordinal.
            #[must_use]
            pub const fn ordinal(self) -> u16 {
                match self {
                    $(Self::$variant => $ordinal),+
                }
            }

            /// Canonical source spelling.
            #[must_use]
            pub const fn canonical_name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),+
                }
            }

            /// Resolves only the canonical source spelling.
            #[must_use]
            pub fn from_source_name(source: &str) -> Option<Self> {
                match source {
                    $($name => Some(Self::$variant),)+
                    _ => None,
                }
            }

            /// Closed enum domain identity carried by schema values.
            #[must_use]
            pub const fn schema_id(self) -> arcweft_id::closed_enum::ClosedEnumDomainId {
                $domain.domain_id()
            }

            /// Canonical names in the same order as [`Self::ALL`].
            #[must_use]
            pub const fn names() -> &'static [&'static str] {
                &[$($name),+]
            }
        }

        const _: () = {
            let _ = <$ty>::ALL;
        };
    };
}

inventory_impl!(
    LayoutDirection,
    RichTextEnumDomain::LayoutDirection,
    3,
    [Auto => 0 => "auto", Ltr => 1 => "ltr", Rtl => 2 => "rtl"]
);
inventory_impl!(
    VerticalLatin,
    RichTextEnumDomain::VerticalLatin,
    3,
    [Mixed => 0 => "mixed", Upright => 1 => "upright", Sideways => 2 => "sideways"]
);
inventory_impl!(
    Jlreq,
    RichTextEnumDomain::Jlreq,
    4,
    [Auto => 0 => "auto", Loose => 1 => "loose", Normal => 2 => "normal", Strict => 3 => "strict"]
);
inventory_impl!(
    TransformTarget,
    RichTextEnumDomain::TransformTarget,
    6,
    [
        Node => 0 => "node",
        Content => 1 => "content",
        Background => 2 => "background",
        Line => 3 => "line",
        Glyph => 4 => "glyph",
        Viewport => 5 => "viewport"
    ]
);
inventory_impl!(
    TransformOrigin,
    RichTextEnumDomain::TransformOrigin,
    4,
    [
        BaselineStart => 0 => "baseline_start",
        BaselineCenter => 1 => "baseline_center",
        Center => 2 => "center",
        GlyphCenter => 3 => "glyph_center"
    ]
);
#[cfg(test)]
mod tests {
    use super::{Jlreq, LayoutDirection, TransformOrigin, TransformTarget, VerticalLatin};

    #[test]
    fn every_closed_value_round_trips_without_aliases() {
        for value in LayoutDirection::ALL {
            assert_eq!(
                LayoutDirection::from_source_name(value.canonical_name()),
                Some(value)
            );
        }
        for value in VerticalLatin::ALL {
            assert_eq!(
                VerticalLatin::from_source_name(value.canonical_name()),
                Some(value)
            );
        }
        for value in Jlreq::ALL {
            assert_eq!(Jlreq::from_source_name(value.canonical_name()), Some(value));
        }
        for value in TransformTarget::ALL {
            assert_eq!(
                TransformTarget::from_source_name(value.canonical_name()),
                Some(value)
            );
        }
        for value in TransformOrigin::ALL {
            assert_eq!(
                TransformOrigin::from_source_name(value.canonical_name()),
                Some(value)
            );
        }
    }
}
