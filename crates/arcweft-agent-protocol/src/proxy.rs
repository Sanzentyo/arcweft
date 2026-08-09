use crate::serde_helpers::is_false;
use arcweft_text_model::{
    RichTextParam, RichTextTextProxyField, RichTextTextProxyScalar, RichTextTextProxySchema,
};
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
    pub declaration: Option<arcweft_text_model::RichTextObjectProxyDeclaration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<RichTextTextProxySchema>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hit_test: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<RichTextTextProxyField>,
    /// Image-object proxy parameters remain owned by the image resource model.
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
    proxy
        .fields
        .iter()
        .find(|field| field.name == query.key)
        .is_some_and(|field| {
            query
                .value
                .as_ref()
                .is_none_or(|value| text_proxy_scalar_matches_query_value(&field.value, value))
        })
        || proxy.params.get(&query.key).is_some_and(|param| {
            query
                .value
                .as_ref()
                .is_none_or(|value| rich_text_param_matches_query_value(param, value))
        })
}

fn text_proxy_scalar_matches_query_value(param: &RichTextTextProxyScalar, value: &str) -> bool {
    match param {
        RichTextTextProxyScalar::Bool { value: param_value } => value == param_value.to_string(),
        RichTextTextProxyScalar::Int { value: param_value } => value == param_value.to_string(),
        RichTextTextProxyScalar::Milli { value: param_value } => value == param_value.0.to_string(),
        RichTextTextProxyScalar::Ratio { milli } => value == milli.to_string(),
        RichTextTextProxyScalar::Length { value: length } => value == length.milli.to_string(),
        RichTextTextProxyScalar::Angle { milli_degrees } => value == milli_degrees.to_string(),
        RichTextTextProxyScalar::Duration { millis } => value == millis.to_string(),
        RichTextTextProxyScalar::ClosedEnum { enum_id, variant } => {
            value == format!("{enum_id}#{variant}")
        }
        RichTextTextProxyScalar::PublicId { value: param_value }
        | RichTextTextProxyScalar::Text { value: param_value } => value == param_value,
        RichTextTextProxyScalar::Color { value: param_value } => match param_value {
            arcweft_text_model::RichTextColor::Rgba8 { value: rgba } => {
                value
                    == format!(
                        "#{:02x}{:02x}{:02x}{:02x}",
                        rgba[0], rgba[1], rgba[2], rgba[3]
                    )
            }
            arcweft_text_model::RichTextColor::Resource { id } => value == id,
        },
    }
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
        | RichTextParam::Selector { value: param_value } => value == param_value,
        RichTextParam::Color { value: param_value } => {
            value
                == format!(
                    "#{:02x}{:02x}{:02x}{:02x}",
                    param_value[0], param_value[1], param_value[2], param_value[3]
                )
        }
    }
}

pub(crate) fn agent_presentation_object_proxy_ref(
    proxy: &arcweft_text_model::RichTextObjectProxy,
) -> AgentPresentationObjectProxyRef {
    AgentPresentationObjectProxyRef {
        id: proxy.id.clone(),
        type_name: proxy.type_name.clone(),
        role: proxy.role.clone(),
        layer: proxy.layer.clone(),
        depth: proxy.depth.map(|depth| depth.0),
        declaration: proxy.declaration.clone(),
        schema: proxy.schema.clone(),
        hit_test: proxy.hit_test,
        fields: proxy.fields.clone(),
        params: BTreeMap::new(),
    }
}
