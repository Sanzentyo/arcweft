//! Reusable scalar style projection of the complete Content callable catalog.

use super::{
    PresentationContentAttachedBodyPolicy, PresentationContentCallableCatalog,
    PresentationContentCallableDefinition, PresentationContentCallableDefinitionId,
    PresentationContentCallableParameterSpec, PresentationContentEmissionFamily,
    RichTextCallableSchemaDigest, encode_definition_id, encode_parameter, push_usize,
};

impl PresentationContentCallableDefinition {
    /// Parameters that can be retained independently as reusable style fields.
    ///
    /// Only modifiers that preserve their body's content role contribute these
    /// fields. Ruby readings and literal bodies belong to content nodes. An Fx
    /// application remains a complete typed graph, not sparse scalar fields.
    #[must_use]
    pub const fn reusable_style_parameters(
        &self,
    ) -> &'static [PresentationContentCallableParameterSpec] {
        match self.emission_family() {
            PresentationContentEmissionFamily::Strong
            | PresentationContentEmissionFamily::Em
            | PresentationContentEmissionFamily::Color
            | PresentationContentEmissionFamily::Font
            | PresentationContentEmissionFamily::Size
            | PresentationContentEmissionFamily::Style
            | PresentationContentEmissionFamily::Layout
            | PresentationContentEmissionFamily::Transform => match self.attached_body_policy() {
                PresentationContentAttachedBodyPolicy::PreserveBodyRole => self.parameters(),
                PresentationContentAttachedBodyPolicy::InlineOnly
                | PresentationContentAttachedBodyPolicy::LiteralOnly => &[],
            },
            PresentationContentEmissionFamily::Fx
            | PresentationContentEmissionFamily::Ruby
            | PresentationContentEmissionFamily::Raw => &[],
        }
    }
}

impl PresentationContentCallableCatalog {
    /// Reusable scalar style coordinates in canonical definition and parameter
    /// order, borrowing the complete descriptors from their owning rows.
    pub fn reusable_style_parameters(
        &self,
    ) -> impl Iterator<
        Item = (
            PresentationContentCallableDefinitionId,
            &'static PresentationContentCallableParameterSpec,
        ),
    > {
        self.iter().flat_map(|definition| {
            definition
                .reusable_style_parameters()
                .iter()
                .map(move |parameter| (definition.id(), parameter))
        })
    }

    /// Version-one digest of the exact reusable style parameter projection.
    /// Content-only parameter changes do not change this contract.
    #[must_use]
    pub fn reusable_style_schema_digest(&self) -> RichTextCallableSchemaDigest {
        let mut bytes = b"arcweft.presentation.reusable-style-parameters.v1\0".to_vec();
        push_usize(&mut bytes, self.reusable_style_parameters().count());
        for (definition, parameter) in self.reusable_style_parameters() {
            encode_definition_id(&mut bytes, definition);
            encode_parameter(&mut bytes, parameter);
        }
        RichTextCallableSchemaDigest::derive(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rich_text::{
        PRESENTATION_CONTENT_CALLABLE_CATALOG, PresentationContentCallableParameterId,
        RichTextLayoutProperty, RichTextLayoutSelector,
    };
    use arcweft_rich_text_schema::{CheckedOutputKind, RichTextValueKind};
    use std::collections::BTreeSet;

    #[test]
    fn reusable_style_projection_preserves_ruby_typography_but_not_content_readings() {
        let catalog = PRESENTATION_CONTENT_CALLABLE_CATALOG;
        let ruby = catalog
            .get(PresentationContentCallableDefinitionId::Ruby)
            .unwrap();
        assert_eq!(
            ruby.parameters()[0].id,
            PresentationContentCallableParameterId::Value
        );
        assert_eq!(ruby.parameters()[0].kind, RichTextValueKind::Text);
        assert!(ruby.reusable_style_parameters().is_empty());
        let coordinates = catalog
            .reusable_style_parameters()
            .map(|(definition, parameter)| (definition, parameter.id))
            .collect::<Vec<_>>();
        assert_eq!(
            coordinates.iter().copied().collect::<BTreeSet<_>>().len(),
            coordinates.len()
        );
        assert!(coordinates.contains(&(
            PresentationContentCallableDefinitionId::Color,
            PresentationContentCallableParameterId::Value,
        )));
        for selector in [
            RichTextLayoutSelector::RubyOver,
            RichTextLayoutSelector::RubyUnder,
            RichTextLayoutSelector::RubyInterCharacter,
        ] {
            for property in [
                RichTextLayoutProperty::RubySize,
                RichTextLayoutProperty::RubyGap,
                RichTextLayoutProperty::RubyOverhang,
                RichTextLayoutProperty::RubyCollisionGap,
            ] {
                assert!(coordinates.contains(&(
                    PresentationContentCallableDefinitionId::Layout(selector),
                    PresentationContentCallableParameterId::Layout(property),
                )));
            }
        }
        for definition in [
            PresentationContentCallableDefinitionId::Ruby,
            PresentationContentCallableDefinitionId::Raw,
            PresentationContentCallableDefinitionId::Fx,
        ] {
            assert!(
                !coordinates
                    .iter()
                    .any(|(candidate, _)| *candidate == definition)
            );
        }
    }

    #[test]
    fn reusable_style_projection_requires_body_role_preservation() {
        let color = PresentationContentCallableDefinition::new(
            PresentationContentCallableDefinitionId::Color,
            super::super::COLOR_PARAMETERS,
            PresentationContentAttachedBodyPolicy::InlineOnly,
            PresentationContentEmissionFamily::Color,
            CheckedOutputKind::Span,
        );
        assert!(!color.parameters().is_empty());
        assert!(color.reusable_style_parameters().is_empty());
    }

    #[test]
    fn reusable_style_digest_tracks_projected_descriptors_independently_of_content() {
        use super::super::{COLOR_PARAMETERS, RUBY_PARAMETERS, direct_definition, ruby_definition};

        const BASE: &[PresentationContentCallableDefinition] = &[
            direct_definition(
                PresentationContentCallableDefinitionId::Color,
                COLOR_PARAMETERS,
                PresentationContentEmissionFamily::Color,
                CheckedOutputKind::Span,
            ),
            ruby_definition(),
        ];
        const CHANGED_READING: &[PresentationContentCallableParameterSpec] = &[{
            let mut parameter = RUBY_PARAMETERS[0];
            parameter.limits.max_decoded_bytes += 1;
            parameter
        }];
        const CONTENT_CHANGED: &[PresentationContentCallableDefinition] = &[
            BASE[0],
            PresentationContentCallableDefinition::new(
                PresentationContentCallableDefinitionId::Ruby,
                CHANGED_READING,
                PresentationContentAttachedBodyPolicy::InlineOnly,
                PresentationContentEmissionFamily::Ruby,
                CheckedOutputKind::Span,
            ),
        ];
        const CHANGED_COLOR: &[PresentationContentCallableParameterSpec] = &[{
            let mut parameter = COLOR_PARAMETERS[0];
            parameter.limits.max_decoded_bytes += 1;
            parameter
        }];
        const STYLE_CHANGED: &[PresentationContentCallableDefinition] = &[
            direct_definition(
                PresentationContentCallableDefinitionId::Color,
                CHANGED_COLOR,
                PresentationContentEmissionFamily::Color,
                CheckedOutputKind::Span,
            ),
            BASE[1],
        ];
        let base = PresentationContentCallableCatalog::new(BASE);
        let content_changed = PresentationContentCallableCatalog::new(CONTENT_CHANGED);
        let style_changed = PresentationContentCallableCatalog::new(STYLE_CHANGED);
        assert_ne!(base.schema_digest(), content_changed.schema_digest());
        assert_eq!(
            base.reusable_style_schema_digest(),
            content_changed.reusable_style_schema_digest()
        );
        assert_ne!(
            base.reusable_style_schema_digest(),
            style_changed.reusable_style_schema_digest()
        );
    }
}
