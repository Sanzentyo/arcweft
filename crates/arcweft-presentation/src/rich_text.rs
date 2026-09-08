//! Renderer-independent rich-text content-call vocabulary.

mod authoring_schema;
mod content_catalog;
mod inventories;

pub use crate::fx::{FxPhase, FxTarget};
use arcweft_id::closed_enum::{ClosedEnumDomainDescriptor, ClosedEnumMemberDescriptor};

pub use authoring_schema::{
    RichTextDirectStyle, RichTextDirectStyleProperty, RichTextLayoutProperty,
    RichTextLayoutSelector, RichTextObjectProperty, RichTextObjectSelector, RichTextStyleProperty,
    RichTextStyleSelector, RichTextTransformProperty, RichTextTransformSelector,
};
pub use content_catalog::{
    PRESENTATION_CONTENT_CALLABLE_CATALOG, PresentationContentAttachedBodyPolicy,
    PresentationContentCallableCatalog, PresentationContentCallableDefinition,
    PresentationContentCallableDefinitionId, PresentationContentCallableHead,
    PresentationContentCallableParameterId, PresentationContentCallableParameterSpec,
    PresentationContentEmissionFamily,
};
pub use inventories::{Jlreq, LayoutDirection, TransformOrigin, TransformTarget, VerticalLatin};

pub use RichTextLayoutSelector as LayoutSelector;
pub use RichTextStyleSelector as StyleSelector;
pub use RichTextTransformSelector as TransformSelector;

const STYLE_ENUM_MEMBERS: &[ClosedEnumMemberDescriptor] = &[
    ClosedEnumMemberDescriptor::new(0, "italic"),
    ClosedEnumMemberDescriptor::new(1, "oblique"),
    ClosedEnumMemberDescriptor::new(2, "opacity"),
    ClosedEnumMemberDescriptor::new(3, "layer"),
    ClosedEnumMemberDescriptor::new(4, "z_index"),
];
const LAYOUT_ENUM_MEMBERS: &[ClosedEnumMemberDescriptor] = &[
    ClosedEnumMemberDescriptor::new(0, "horizontal_tb"),
    ClosedEnumMemberDescriptor::new(1, "vertical_rl"),
    ClosedEnumMemberDescriptor::new(2, "vertical_lr"),
    ClosedEnumMemberDescriptor::new(3, "dir"),
    ClosedEnumMemberDescriptor::new(4, "ruby_over"),
    ClosedEnumMemberDescriptor::new(5, "ruby_under"),
    ClosedEnumMemberDescriptor::new(6, "ruby_inter_character"),
];
const TRANSFORM_ENUM_MEMBERS: &[ClosedEnumMemberDescriptor] = &[
    ClosedEnumMemberDescriptor::new(0, "offset"),
    ClosedEnumMemberDescriptor::new(1, "rotate"),
    ClosedEnumMemberDescriptor::new(2, "scale"),
    ClosedEnumMemberDescriptor::new(3, "skew"),
];
const DIRECTION_ENUM_MEMBERS: &[ClosedEnumMemberDescriptor] = &[
    ClosedEnumMemberDescriptor::new(0, "auto"),
    ClosedEnumMemberDescriptor::new(1, "ltr"),
    ClosedEnumMemberDescriptor::new(2, "rtl"),
];
const LATIN_ENUM_MEMBERS: &[ClosedEnumMemberDescriptor] = &[
    ClosedEnumMemberDescriptor::new(0, "mixed"),
    ClosedEnumMemberDescriptor::new(1, "upright"),
    ClosedEnumMemberDescriptor::new(2, "sideways"),
];
const JLREQ_ENUM_MEMBERS: &[ClosedEnumMemberDescriptor] = &[
    ClosedEnumMemberDescriptor::new(0, "auto"),
    ClosedEnumMemberDescriptor::new(1, "loose"),
    ClosedEnumMemberDescriptor::new(2, "normal"),
    ClosedEnumMemberDescriptor::new(3, "strict"),
];
const TARGET_ENUM_MEMBERS: &[ClosedEnumMemberDescriptor] = &[
    ClosedEnumMemberDescriptor::new(0, "node"),
    ClosedEnumMemberDescriptor::new(1, "content"),
    ClosedEnumMemberDescriptor::new(2, "background"),
    ClosedEnumMemberDescriptor::new(3, "line"),
    ClosedEnumMemberDescriptor::new(4, "glyph"),
    ClosedEnumMemberDescriptor::new(5, "viewport"),
];
const ORIGIN_ENUM_MEMBERS: &[ClosedEnumMemberDescriptor] = &[
    ClosedEnumMemberDescriptor::new(0, "baseline_start"),
    ClosedEnumMemberDescriptor::new(1, "baseline_center"),
    ClosedEnumMemberDescriptor::new(2, "center"),
    ClosedEnumMemberDescriptor::new(3, "glyph_center"),
];

pub const RICH_TEXT_CLOSED_ENUM_DOMAINS: &[ClosedEnumDomainDescriptor] = &[
    ClosedEnumDomainDescriptor::new(
        arcweft_rich_text_schema::RichTextEnumDomain::StyleSelector.domain_id(),
        "RichText style selector",
        STYLE_ENUM_MEMBERS,
    ),
    ClosedEnumDomainDescriptor::new(
        arcweft_rich_text_schema::RichTextEnumDomain::LayoutSelector.domain_id(),
        "RichText layout selector",
        LAYOUT_ENUM_MEMBERS,
    ),
    ClosedEnumDomainDescriptor::new(
        arcweft_rich_text_schema::RichTextEnumDomain::TransformSelector.domain_id(),
        "RichText transform selector",
        TRANSFORM_ENUM_MEMBERS,
    ),
    ClosedEnumDomainDescriptor::new(
        arcweft_rich_text_schema::RichTextEnumDomain::LayoutDirection.domain_id(),
        "RichText layout direction",
        DIRECTION_ENUM_MEMBERS,
    ),
    ClosedEnumDomainDescriptor::new(
        arcweft_rich_text_schema::RichTextEnumDomain::VerticalLatin.domain_id(),
        "RichText vertical Latin",
        LATIN_ENUM_MEMBERS,
    ),
    ClosedEnumDomainDescriptor::new(
        arcweft_rich_text_schema::RichTextEnumDomain::Jlreq.domain_id(),
        "RichText JLREQ",
        JLREQ_ENUM_MEMBERS,
    ),
    ClosedEnumDomainDescriptor::new(
        arcweft_rich_text_schema::RichTextEnumDomain::TransformTarget.domain_id(),
        "RichText transform target",
        TARGET_ENUM_MEMBERS,
    ),
    ClosedEnumDomainDescriptor::new(
        arcweft_rich_text_schema::RichTextEnumDomain::TransformOrigin.domain_id(),
        "RichText transform origin",
        ORIGIN_ENUM_MEMBERS,
    ),
];
