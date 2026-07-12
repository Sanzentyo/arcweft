use crate::serde_helpers::is_false;
use arcweft_render_text::RichTextParam;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Lightweight typed-Fx index attached to a presentation tree object node.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentPresentationFxRef {
    pub id: String,
    pub authored_ordinal: u32,
}

/// Lightweight object-proxy index attached to a presentation tree object node.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentPresentationObjectProxyRef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration: Option<arcweft_render_text::RichTextObjectProxyDeclaration>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hit_test: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, RichTextParam>,
}

/// Query for object-proxy parameter metadata in a presentation tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentPresentationObjectProxyParamQuery {
    pub key: String,
    pub value: Option<String>,
}

pub(crate) fn proxy_matches_param_query(
    proxy: &AgentPresentationObjectProxyRef,
    query: &AgentPresentationObjectProxyParamQuery,
) -> bool {
    let Some(param) = proxy.params.get(&query.key) else {
        return false;
    };
    query
        .value
        .as_ref()
        .is_none_or(|value| rich_text_param_matches_query_value(param, value))
}

pub(crate) fn rich_text_param_matches_query_value(param: &RichTextParam, value: &str) -> bool {
    match param {
        RichTextParam::Bool { value: param_value } => value == param_value.to_string(),
        RichTextParam::Int { value: param_value } => value == param_value.to_string(),
        RichTextParam::Milli { value: param_value } => value == param_value.0.to_string(),
        RichTextParam::Vec2 { value: param_value } => {
            value == format!("{},{}", param_value.x.0, param_value.y.0)
        }
        RichTextParam::Text { value: param_value }
        | RichTextParam::Raw { value: param_value }
        | RichTextParam::Selector { value: param_value } => value == param_value,
        RichTextParam::Expr { source } => value == source,
    }
}

pub(crate) fn agent_presentation_object_proxy_ref(
    proxy: &arcweft_render_text::RichTextObjectProxy,
) -> AgentPresentationObjectProxyRef {
    AgentPresentationObjectProxyRef {
        id: proxy.id.clone(),
        type_name: proxy.type_name.clone(),
        role: proxy.role.clone(),
        layer: proxy.layer.clone(),
        depth: proxy.depth.map(|depth| depth.0),
        declaration: proxy.declaration.clone(),
        hit_test: proxy.hit_test,
        params: proxy.params.clone(),
    }
}
