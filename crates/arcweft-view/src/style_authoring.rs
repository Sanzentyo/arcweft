//! Author-facing View style declarations and view-local style overrides.
//!
//! The renderer consumes resolved `ViewStyle`/paint data, not this authoring model.
//! Both Arcweft native style syntax and CSS lower into this representation before
//! being interned into frame-local `StyleId` values.

use crate::{Invalidation, ViewInteractionSelector, ViewPropertyKind, ViewPropertyValue};
use arcweft_id::PublicId;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum StyleSyntax {
    #[default]
    Arcweft,
    Css,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StyleSource {
    Inline(String),
    Files(Vec<StyleFileRef>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleFileRef {
    mode: StyleFileMode,
    path: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleFileMode {
    File,
    Embed,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StylePatch {
    properties: Vec<StylePropertyAssignment>,
    tokens: Vec<StyleTokenBinding>,
    rules: Vec<StyleConditionalRule>,
    part_rules: Vec<PartStyleRule>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StylePropertyAssignment {
    kind: ViewPropertyKind,
    value: ViewPropertyValue,
    op: StyleAssignOp,
    invalidation: Invalidation,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StyleAssignOp {
    #[default]
    Replace,
    Append,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleTokenBinding {
    name: PublicId,
    value: StyleTokenValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StyleTokenValue {
    Property(ViewPropertyValue),
    SystemColor(arcweft_presentation::appearance::SystemColor),
    Resource(PublicId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleConditionalRule {
    condition: StyleCondition,
    patch: StylePatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartStyleRule {
    part: StylePartId,
    condition: Option<StyleCondition>,
    patch: StylePatch,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StylePartId(PublicId);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StyleCondition {
    Interaction(ViewInteractionSelector),
    ElementState(ViewElementStateSelector),
    Environment(EnvironmentStylePredicate),
    Expression(StyleExpressionId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ViewElementStateSelector {
    FocusVisible,
    ReadOnly,
    Invalid,
    Composing,
    PlaceholderShown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentStylePredicate {
    ColorScheme(arcweft_presentation::appearance::ColorScheme),
    Contrast(arcweft_presentation::appearance::ContrastPreference),
    ReduceMotion(bool),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StyleExpressionId(pub u32);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewStyleOverride {
    layers: Vec<StyleOverrideLayer>,
    exported_parts: BTreeMap<StylePartId, PublicId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleOverrideLayer {
    syntax: StyleSyntax,
    source: StyleSource,
    patch: StylePatch,
}

impl StyleSource {
    pub fn inline(source: impl Into<String>) -> Self {
        Self::Inline(source.into())
    }

    pub fn from_file(path: impl Into<String>) -> Self {
        Self::Files(vec![StyleFileRef::file(path)])
    }

    pub fn from_embed(path: impl Into<String>) -> Self {
        Self::Files(vec![StyleFileRef::embed(path)])
    }

    pub fn files(files: Vec<StyleFileRef>) -> Self {
        Self::Files(files)
    }
}

impl StyleFileRef {
    pub fn file(path: impl Into<String>) -> Self {
        Self {
            mode: StyleFileMode::File,
            path: path.into(),
        }
    }

    pub fn embed(path: impl Into<String>) -> Self {
        Self {
            mode: StyleFileMode::Embed,
            path: path.into(),
        }
    }

    pub const fn mode(&self) -> StyleFileMode {
        self.mode
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl StylePatch {
    pub fn push_property(&mut self, kind: ViewPropertyKind, value: ViewPropertyValue) {
        self.properties
            .push(StylePropertyAssignment::replace(kind, value));
    }

    pub fn push_rule(&mut self, condition: StyleCondition, patch: StylePatch) {
        self.rules.push(StyleConditionalRule { condition, patch });
    }

    pub fn push_part_rule(&mut self, part: StylePartId, patch: StylePatch) {
        self.part_rules.push(PartStyleRule {
            part,
            condition: None,
            patch,
        });
    }

    pub fn properties(&self) -> &[StylePropertyAssignment] {
        &self.properties
    }

    pub fn tokens(&self) -> &[StyleTokenBinding] {
        &self.tokens
    }

    pub fn rules(&self) -> &[StyleConditionalRule] {
        &self.rules
    }

    pub fn part_rules(&self) -> &[PartStyleRule] {
        &self.part_rules
    }
}

impl StylePropertyAssignment {
    pub fn replace(kind: ViewPropertyKind, value: ViewPropertyValue) -> Self {
        Self {
            kind,
            value,
            op: StyleAssignOp::Replace,
            invalidation: kind.default_invalidation(),
        }
    }

    pub fn append(kind: ViewPropertyKind, value: ViewPropertyValue) -> Self {
        Self {
            kind,
            value,
            op: StyleAssignOp::Append,
            invalidation: kind.default_invalidation(),
        }
    }

    pub const fn kind(&self) -> ViewPropertyKind {
        self.kind
    }

    pub const fn value(&self) -> ViewPropertyValue {
        self.value
    }

    pub const fn op(&self) -> StyleAssignOp {
        self.op
    }

    pub const fn invalidation(&self) -> Invalidation {
        self.invalidation
    }
}

impl StylePartId {
    pub const fn new(id: PublicId) -> Self {
        Self(id)
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.0
    }
}

impl StyleOverrideLayer {
    pub const fn new(syntax: StyleSyntax, source: StyleSource, patch: StylePatch) -> Self {
        Self {
            syntax,
            source,
            patch,
        }
    }

    pub const fn syntax(&self) -> StyleSyntax {
        self.syntax
    }

    pub const fn source(&self) -> &StyleSource {
        &self.source
    }

    pub const fn patch(&self) -> &StylePatch {
        &self.patch
    }

    pub fn arcweft_inline(source: impl Into<String>, patch: StylePatch) -> Self {
        Self::new(StyleSyntax::Arcweft, StyleSource::inline(source), patch)
    }

    pub fn css_inline(source: impl Into<String>, patch: StylePatch) -> Self {
        Self::new(StyleSyntax::Css, StyleSource::inline(source), patch)
    }

    pub fn arcweft_file(source_path: impl Into<String>, style_patch: StylePatch) -> Self {
        Self::new(
            StyleSyntax::Arcweft,
            StyleSource::from_file(source_path),
            style_patch,
        )
    }

    pub fn css_file(source_path: impl Into<String>, style_patch: StylePatch) -> Self {
        Self::new(
            StyleSyntax::Css,
            StyleSource::from_file(source_path),
            style_patch,
        )
    }

    pub fn css_embed(source_path: impl Into<String>, style_patch: StylePatch) -> Self {
        Self::new(
            StyleSyntax::Css,
            StyleSource::from_embed(source_path),
            style_patch,
        )
    }
}

impl ViewStyleOverride {
    pub fn push_layer(&mut self, layer: StyleOverrideLayer) {
        self.layers.push(layer);
    }

    pub fn export_part(&mut self, part: StylePartId, target: PublicId) {
        self.exported_parts.insert(part, target);
    }

    pub fn layers(&self) -> &[StyleOverrideLayer] {
        &self.layers
    }

    pub const fn exported_parts(&self) -> &BTreeMap<StylePartId, PublicId> {
        &self.exported_parts
    }
}

#[cfg(test)]
mod tests {
    use super::{
        StyleFileMode, StyleFileRef, StyleOverrideLayer, StylePatch, StyleSource, StyleSyntax,
        ViewStyleOverride,
    };
    use crate::{Milli, ViewPropertyKind, ViewPropertyValue};
    use arcweft_id::PublicId;

    #[test]
    fn style_file_refs_preserve_file_vs_embed_identity() {
        let file = StyleFileRef::file("view/dialogue.css");
        let embed = StyleFileRef::embed("view/default.css");

        assert_eq!(file.mode(), StyleFileMode::File);
        assert_eq!(file.path(), "view/dialogue.css");
        assert_eq!(embed.mode(), StyleFileMode::Embed);
        assert_eq!(embed.path(), "view/default.css");
    }

    #[test]
    fn view_style_overrides_keep_ordered_layers_and_exported_parts() {
        let mut patch = StylePatch::default();
        patch.push_property(
            ViewPropertyKind::Opacity,
            ViewPropertyValue::Milli(Milli(900)),
        );
        let mut overrides = ViewStyleOverride::default();
        overrides.push_layer(StyleOverrideLayer::arcweft_inline("opacity: 0.9", patch));
        overrides.push_layer(StyleOverrideLayer::css_file(
            "view/dialogue.css",
            StylePatch::default(),
        ));
        let part = super::StylePartId::new(public_id("part.label"));
        let target = public_id("view.dialogue.label");
        overrides.export_part(part.clone(), target.clone());

        assert_eq!(overrides.layers().len(), 2);
        assert_eq!(overrides.layers()[0].syntax(), StyleSyntax::Arcweft);
        assert_eq!(overrides.layers()[1].syntax(), StyleSyntax::Css);
        assert_eq!(
            overrides.layers()[1].source(),
            &StyleSource::from_file("view/dialogue.css")
        );
        assert_eq!(overrides.exported_parts().get(&part), Some(&target));
    }

    fn public_id(value: &str) -> PublicId {
        PublicId::try_new(value).expect("test id")
    }
}
