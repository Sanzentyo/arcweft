use crate::geometry::AgentBBox;
use crate::serde_helpers::is_zero;
use arcweft_text_model::{
    RichTextNodeIndex, RichTextObjectProxyDeclaration, RichTextParam, RichTextPresentation,
    RichTextRange, RichTextTextProxyField, RichTextTextProxySchema, RichTextTextSource,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Structured reference from an observed child object back into its parent rich-text display map.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentRichTextElementRef {
    pub kind: AgentRichTextElementKind,
    pub index: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub page: usize,
    pub range: RichTextRange,
    pub node_index: RichTextNodeIndex,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<RichTextTextSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruby: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation: Option<RichTextPresentation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<AgentGlyphOrientation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_form: Option<AgentGlyphVerticalForm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruby_base_bbox: Option<AgentBBox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruby_annotation_bbox: Option<AgentBBox>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_depth: Option<i32>,
    #[serde(default)]
    pub hit_test: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hit_regions: Vec<AgentHitRegion>,
}

/// Rich-text display-map element kind observed as a debuggable object.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRichTextElementKind {
    TextPage,
    TextLine,
    TextRun,
    TextGlyph,
    Ruby,
    GlyphCluster,
    TextObjectProxy,
}

/// Hit-test region for one observed rich-text element.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentHitRegion {
    pub kind: AgentHitRegionKind,
    pub bbox: AgentBBox,
    pub range: RichTextRange,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_declaration: Option<RichTextObjectProxyDeclaration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_schema: Option<RichTextTextProxySchema>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proxy_fields: Vec<RichTextTextProxyField>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<i32>,
    /// Image-object proxy parameters remain owned by the image resource model.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub proxy_params: BTreeMap<String, RichTextParam>,
}

impl AgentHitRegion {
    /// Exact root-object region shared by hit production and envelope validation.
    #[must_use]
    pub fn for_object(
        bbox: &AgentBBox,
        object_layer: Option<&str>,
        object_depth: Option<i32>,
    ) -> Self {
        Self {
            kind: AgentHitRegionKind::Object,
            bbox: bbox.clone(),
            range: RichTextRange::new(0, 0),
            proxy_id: None,
            proxy_type: None,
            proxy_declaration: None,
            proxy_schema: None,
            proxy_fields: Vec::new(),
            proxy_role: None,
            proxy_layer: object_layer.map(str::to_owned),
            depth: object_depth,
            proxy_params: BTreeMap::new(),
        }
    }
}

/// Semantic role for a rich-text hit-test region.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentHitRegionKind {
    Object,
    ObjectProxy,
    TextPage,
    TextLine,
    TextRun,
    TextGlyph,
    GlyphCluster,
    TextObjectProxy,
    RubyObject,
    RubyBase,
    RubyAnnotation,
}

/// Renderer-facing orientation chosen for one observed glyph cluster.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentGlyphOrientation {
    Upright,
    SidewaysCw,
    TextCombineUpright,
}

/// Vertical alternate shaping request attached to one observed glyph cluster.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentGlyphVerticalForm {
    None,
    UprightAlternate,
    RotatedAlternate,
}
