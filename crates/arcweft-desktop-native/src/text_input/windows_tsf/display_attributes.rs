use arcweft_presentation::text_input::{
    TextByteOffset, TextCompositionSegment, TextCompositionSegmentKind, TextRange,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TsfDisplayAttributeClass {
    Input,
    Converted,
    FixedConverted,
    TargetConverted,
    TargetNotConverted,
    InputError,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TsfDisplayAttributeSegment {
    range: TextRange<TextByteOffset>,
    attribute: TsfDisplayAttributeClass,
}

impl TsfDisplayAttributeClass {
    pub const fn composition_kind(self) -> Option<TextCompositionSegmentKind> {
        match self {
            Self::Input => Some(TextCompositionSegmentKind::RawInput),
            Self::Converted | Self::FixedConverted => Some(TextCompositionSegmentKind::Converted),
            Self::TargetConverted => Some(TextCompositionSegmentKind::TargetConverted),
            Self::TargetNotConverted => Some(TextCompositionSegmentKind::TargetNotConverted),
            Self::InputError => Some(TextCompositionSegmentKind::InputError),
            Self::Other => None,
        }
    }

    pub const fn diagnostic_code(self) -> Option<&'static str> {
        match self {
            Self::Other => Some("tsf_display_attribute_other"),
            Self::Input
            | Self::Converted
            | Self::FixedConverted
            | Self::TargetConverted
            | Self::TargetNotConverted
            | Self::InputError => None,
        }
    }
}

impl TsfDisplayAttributeSegment {
    pub const fn new(
        range: TextRange<TextByteOffset>,
        attribute: TsfDisplayAttributeClass,
    ) -> Self {
        Self { range, attribute }
    }

    pub const fn range(self) -> TextRange<TextByteOffset> {
        self.range
    }

    pub const fn attribute(self) -> TsfDisplayAttributeClass {
        self.attribute
    }

    pub const fn to_composition_segment(self) -> Option<TextCompositionSegment> {
        match self.attribute.composition_kind() {
            Some(kind) => Some(TextCompositionSegment::new(self.range, kind)),
            None => None,
        }
    }
}
