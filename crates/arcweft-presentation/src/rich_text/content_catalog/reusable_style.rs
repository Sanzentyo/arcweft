//! Semantic fields of a reusable policy, independent of content-call syntax.

use super::{
    PresentationContentAttachedBodyPolicy, PresentationContentCallableCatalog,
    PresentationContentCallableDefinitionId as Definition,
    PresentationContentCallableParameterId as Parameter, RichTextCallableSchemaDigest,
    RichTextLayoutProperty, RichTextLayoutSelector, RichTextStyleProperty, RichTextStyleSelector,
    RichTextTransformProperty, RichTextTransformSelector, encode_enum_constraint, encode_limits,
    encode_value_kind, push_usize,
};
use arcweft_rich_text_schema::{
    RichTextEnumValueConstraint, RichTextValueKind, RichTextValueLimits,
};
use std::collections::BTreeMap;

/// One semantic setting, regardless of which source selector supplied it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PresentationReusableStylePropertyId {
    Color,
    Font,
    Size,
    Italic,
    ObliqueAngle,
    Opacity,
    Layer,
    ZIndex,
    WritingMode,
    RubyPosition,
    Direction,
    Latin,
    Jlreq,
    ColumnGap,
    RubySize,
    RubyGap,
    RubyOverhang,
    RubyCollisionGap,
    TranslateX,
    TranslateY,
    Rotate,
    ScaleX,
    ScaleY,
    SkewX,
    SkewY,
    TransformTarget,
    TransformOrigin,
}

impl PresentationReusableStylePropertyId {
    /// Version-one field identity, independent of source catalog row order.
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Color => 0,
            Self::Font => 1,
            Self::Size => 2,
            Self::Italic => 3,
            Self::ObliqueAngle => 4,
            Self::Opacity => 5,
            Self::Layer => 6,
            Self::ZIndex => 7,
            Self::WritingMode => 8,
            Self::RubyPosition => 9,
            Self::Direction => 10,
            Self::Latin => 11,
            Self::Jlreq => 12,
            Self::ColumnGap => 13,
            Self::RubySize => 14,
            Self::RubyGap => 15,
            Self::RubyOverhang => 16,
            Self::RubyCollisionGap => 17,
            Self::TranslateX => 18,
            Self::TranslateY => 19,
            Self::Rotate => 20,
            Self::ScaleX => 21,
            Self::ScaleY => 22,
            Self::SkewX => 23,
            Self::SkewY => 24,
            Self::TransformTarget => 25,
            Self::TransformOrigin => 26,
        }
    }

    /// Projects a checked source coordinate to its reusable setting.
    /// Branch discriminators without an independent value are not fields.
    #[must_use]
    pub const fn from_content_parameter(
        definition: Definition,
        parameter: Parameter,
    ) -> Option<Self> {
        match (definition, parameter) {
            (Definition::Color, Parameter::Value) => Some(Self::Color),
            (Definition::Font, Parameter::Value) => Some(Self::Font),
            (Definition::Size, Parameter::Value) => Some(Self::Size),
            (Definition::Style(RichTextStyleSelector::Italic), Parameter::Selector) => {
                Some(Self::Italic)
            }
            (Definition::Style(_), Parameter::Style(property)) => Some(match property {
                RichTextStyleProperty::Angle => Self::ObliqueAngle,
                RichTextStyleProperty::Opacity => Self::Opacity,
                RichTextStyleProperty::Layer => Self::Layer,
                RichTextStyleProperty::ZIndex => Self::ZIndex,
            }),
            (Definition::Layout(selector), Parameter::Selector) => match selector {
                RichTextLayoutSelector::HorizontalTb
                | RichTextLayoutSelector::VerticalRl
                | RichTextLayoutSelector::VerticalLr => Some(Self::WritingMode),
                RichTextLayoutSelector::RubyOver
                | RichTextLayoutSelector::RubyUnder
                | RichTextLayoutSelector::RubyInterCharacter => Some(Self::RubyPosition),
                RichTextLayoutSelector::Direction => None,
            },
            (Definition::Layout(_), Parameter::Layout(property)) => Some(match property {
                RichTextLayoutProperty::Direction => Self::Direction,
                RichTextLayoutProperty::Latin => Self::Latin,
                RichTextLayoutProperty::Jlreq => Self::Jlreq,
                RichTextLayoutProperty::ColumnGap => Self::ColumnGap,
                RichTextLayoutProperty::RubySize => Self::RubySize,
                RichTextLayoutProperty::RubyGap => Self::RubyGap,
                RichTextLayoutProperty::RubyOverhang => Self::RubyOverhang,
                RichTextLayoutProperty::RubyCollisionGap => Self::RubyCollisionGap,
            }),
            (Definition::Transform(selector), Parameter::Transform(property)) => match property {
                RichTextTransformProperty::Target => Some(Self::TransformTarget),
                RichTextTransformProperty::Origin => Some(Self::TransformOrigin),
                RichTextTransformProperty::Angle => match selector {
                    RichTextTransformSelector::Rotate => Some(Self::Rotate),
                    RichTextTransformSelector::Offset
                    | RichTextTransformSelector::Scale
                    | RichTextTransformSelector::Skew => None,
                },
                RichTextTransformProperty::X => match selector {
                    RichTextTransformSelector::Offset => Some(Self::TranslateX),
                    RichTextTransformSelector::Scale => Some(Self::ScaleX),
                    RichTextTransformSelector::Skew => Some(Self::SkewX),
                    RichTextTransformSelector::Rotate => None,
                },
                RichTextTransformProperty::Y => match selector {
                    RichTextTransformSelector::Offset => Some(Self::TranslateY),
                    RichTextTransformSelector::Scale => Some(Self::ScaleY),
                    RichTextTransformSelector::Skew => Some(Self::SkewY),
                    RichTextTransformSelector::Rotate => None,
                },
            },
            _ => None,
        }
    }
}

/// Stored value contract. Passing modes, source names and call defaults do
/// not participate in a reusable property's identity or value grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationReusableStylePropertySpec {
    pub id: PresentationReusableStylePropertyId,
    pub kind: RichTextValueKind,
    pub enum_constraint: RichTextEnumValueConstraint,
    pub limits: RichTextValueLimits,
    pub allow_empty: bool,
}

/// Unique field inventory derived from the complete content callable catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationReusableStyleSchema {
    properties: Box<[PresentationReusableStylePropertySpec]>,
    digest: RichTextCallableSchemaDigest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("conflicting reusable style value contracts for {property:?}")]
pub struct PresentationReusableStyleSchemaError {
    pub property: PresentationReusableStylePropertyId,
}

impl PresentationReusableStyleSchema {
    #[must_use]
    pub fn properties(&self) -> &[PresentationReusableStylePropertySpec] {
        &self.properties
    }

    #[must_use]
    pub const fn digest(&self) -> RichTextCallableSchemaDigest {
        self.digest
    }
}

const WRITING_MODE_ORDINALS: &[u16] = &[
    RichTextLayoutSelector::HorizontalTb.ordinal(),
    RichTextLayoutSelector::VerticalRl.ordinal(),
    RichTextLayoutSelector::VerticalLr.ordinal(),
];
const RUBY_POSITION_ORDINALS: &[u16] = &[
    RichTextLayoutSelector::RubyOver.ordinal(),
    RichTextLayoutSelector::RubyUnder.ordinal(),
    RichTextLayoutSelector::RubyInterCharacter.ordinal(),
];

impl PresentationContentCallableCatalog {
    /// Derives each semantic field once. Conflicting branch value contracts
    /// are rejected; repeated source rows cannot impose cascade precedence.
    pub fn reusable_style_schema(
        &self,
    ) -> Result<PresentationReusableStyleSchema, PresentationReusableStyleSchemaError> {
        let mut fields = BTreeMap::new();
        for definition in self.iter().filter(|row| {
            row.attached_body_policy() == PresentationContentAttachedBodyPolicy::PreserveBodyRole
        }) {
            for parameter in definition.parameters() {
                let Some(id) = PresentationReusableStylePropertyId::from_content_parameter(
                    definition.id(),
                    parameter.id,
                ) else {
                    continue;
                };
                let enum_constraint = match id {
                    PresentationReusableStylePropertyId::WritingMode => {
                        RichTextEnumValueConstraint::Allowed(WRITING_MODE_ORDINALS)
                    }
                    PresentationReusableStylePropertyId::RubyPosition => {
                        RichTextEnumValueConstraint::Allowed(RUBY_POSITION_ORDINALS)
                    }
                    _ => parameter.enum_constraint,
                };
                let field = PresentationReusableStylePropertySpec {
                    id,
                    kind: parameter.kind,
                    enum_constraint,
                    limits: parameter.limits,
                    allow_empty: parameter.allow_empty,
                };
                if let Some(previous) = fields.insert(id, field)
                    && previous != field
                {
                    return Err(PresentationReusableStyleSchemaError { property: id });
                }
            }
        }
        let properties = fields.into_values().collect::<Box<[_]>>();
        let mut bytes = b"arcweft.presentation.reusable-style-properties.v1\0".to_vec();
        push_usize(&mut bytes, properties.len());
        for field in &properties {
            bytes.push(field.id.semantic_tag());
            encode_value_kind(&mut bytes, field.kind);
            encode_enum_constraint(&mut bytes, field.enum_constraint);
            encode_limits(&mut bytes, field.limits);
            bytes.push(u8::from(field.allow_empty));
        }
        Ok(PresentationReusableStyleSchema {
            properties,
            digest: RichTextCallableSchemaDigest::derive(&bytes),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        COLOR_PARAMETERS, PresentationContentCallableDefinition, PresentationContentEmissionFamily,
        RUBY_PARAMETERS, direct_definition, ruby_definition,
    };
    use super::*;
    use crate::rich_text::PRESENTATION_CONTENT_CALLABLE_CATALOG;
    use arcweft_rich_text_schema::CheckedOutputKind;

    #[test]
    fn reusable_style_projection_requires_body_role_preservation() {
        const ROWS: &[PresentationContentCallableDefinition] = &[
            PresentationContentCallableDefinition::new(
                Definition::Color,
                COLOR_PARAMETERS,
                PresentationContentAttachedBodyPolicy::InlineOnly,
                PresentationContentEmissionFamily::Color,
                CheckedOutputKind::Span,
            ),
            PresentationContentCallableDefinition::new(
                Definition::Size,
                COLOR_PARAMETERS,
                PresentationContentAttachedBodyPolicy::LiteralOnly,
                PresentationContentEmissionFamily::Size,
                CheckedOutputKind::Span,
            ),
        ];
        let schema = PresentationContentCallableCatalog::new(ROWS)
            .reusable_style_schema()
            .unwrap();
        assert_eq!(schema.properties(), &[]);
        for definition in [Definition::Ruby, Definition::Raw, Definition::Fx] {
            let row = PRESENTATION_CONTENT_CALLABLE_CATALOG
                .get(definition)
                .unwrap();
            assert!(row.parameters().iter().all(|parameter| {
                PresentationReusableStylePropertyId::from_content_parameter(
                    definition,
                    parameter.id,
                )
                .is_none()
            }));
        }
    }

    #[test]
    fn shared_layout_and_transform_fields_have_one_semantic_slot() {
        let schema = PRESENTATION_CONTENT_CALLABLE_CATALOG
            .reusable_style_schema()
            .unwrap();
        assert_eq!(schema.properties().len(), 27);
        for selector in RichTextLayoutSelector::ALL {
            assert_eq!(
                PresentationReusableStylePropertyId::from_content_parameter(
                    Definition::Layout(selector),
                    Parameter::Layout(RichTextLayoutProperty::RubySize)
                ),
                Some(PresentationReusableStylePropertyId::RubySize)
            );
        }
        for selector in RichTextTransformSelector::ALL {
            assert_eq!(
                PresentationReusableStylePropertyId::from_content_parameter(
                    Definition::Transform(selector),
                    Parameter::Transform(RichTextTransformProperty::Origin)
                ),
                Some(PresentationReusableStylePropertyId::TransformOrigin)
            );
        }
        assert_eq!(
            PresentationReusableStylePropertyId::from_content_parameter(
                Definition::Ruby,
                Parameter::Value
            ),
            None
        );
        let writing = schema
            .properties()
            .iter()
            .find(|field| field.id == PresentationReusableStylePropertyId::WritingMode)
            .unwrap();
        assert_eq!(
            writing.enum_constraint,
            RichTextEnumValueConstraint::Allowed(WRITING_MODE_ORDINALS)
        );
    }

    #[test]
    fn content_readings_do_not_change_reusable_contract_but_values_do() {
        const BASE: &[PresentationContentCallableDefinition] = &[
            direct_definition(
                Definition::Color,
                COLOR_PARAMETERS,
                PresentationContentEmissionFamily::Color,
                CheckedOutputKind::Span,
            ),
            ruby_definition(),
        ];
        const CHANGED_READING: &[super::super::PresentationContentCallableParameterSpec] = &[{
            let mut p = RUBY_PARAMETERS[0];
            p.limits.max_decoded_bytes += 1;
            p
        }];
        const CONTENT_CHANGED: &[PresentationContentCallableDefinition] = &[
            BASE[0],
            PresentationContentCallableDefinition::new(
                Definition::Ruby,
                CHANGED_READING,
                PresentationContentAttachedBodyPolicy::InlineOnly,
                PresentationContentEmissionFamily::Ruby,
                CheckedOutputKind::Span,
            ),
        ];
        const CHANGED_COLOR: &[super::super::PresentationContentCallableParameterSpec] = &[{
            let mut p = COLOR_PARAMETERS[0];
            p.limits.max_decoded_bytes += 1;
            p
        }];
        const STYLE_CHANGED: &[PresentationContentCallableDefinition] = &[
            direct_definition(
                Definition::Color,
                CHANGED_COLOR,
                PresentationContentEmissionFamily::Color,
                CheckedOutputKind::Span,
            ),
            BASE[1],
        ];
        let base = PresentationContentCallableCatalog::new(BASE)
            .reusable_style_schema()
            .unwrap();
        assert_eq!(
            base.digest(),
            PresentationContentCallableCatalog::new(CONTENT_CHANGED)
                .reusable_style_schema()
                .unwrap()
                .digest()
        );
        assert_ne!(
            base.digest(),
            PresentationContentCallableCatalog::new(STYLE_CHANGED)
                .reusable_style_schema()
                .unwrap()
                .digest()
        );
    }

    #[test]
    fn conflicting_shared_field_contracts_reject_the_projection() {
        use super::super::{LAYOUT_RUBY_UNDER_PARAMETERS, LAYOUT_VERTICAL_RL_PARAMETERS};
        const CHANGED: &[super::super::PresentationContentCallableParameterSpec] = &[{
            let mut p = LAYOUT_RUBY_UNDER_PARAMETERS[5];
            p.limits.max_decoded_bytes += 1;
            p
        }];
        const ROWS: &[PresentationContentCallableDefinition] = &[
            PresentationContentCallableDefinition::new(
                Definition::Layout(RichTextLayoutSelector::VerticalRl),
                LAYOUT_VERTICAL_RL_PARAMETERS,
                PresentationContentAttachedBodyPolicy::PreserveBodyRole,
                PresentationContentEmissionFamily::Layout,
                CheckedOutputKind::Span,
            ),
            PresentationContentCallableDefinition::new(
                Definition::Layout(RichTextLayoutSelector::RubyUnder),
                CHANGED,
                PresentationContentAttachedBodyPolicy::PreserveBodyRole,
                PresentationContentEmissionFamily::Layout,
                CheckedOutputKind::Span,
            ),
        ];
        assert!(
            PresentationContentCallableCatalog::new(ROWS)
                .reusable_style_schema()
                .is_err()
        );
    }
}
