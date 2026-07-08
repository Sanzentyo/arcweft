use crate::object::{AgentObservedLayer, AgentObservedObject, AgentObservedObjectContent};
use crate::proxy::{
    AgentPresentationEffectRef, AgentPresentationObjectProxyParamQuery,
    AgentPresentationObjectProxyRef, AgentPresentationShaderRef,
    agent_presentation_object_proxy_ref, proxy_matches_param_query,
};
use crate::rich_text::AgentRichTextElementKind;
use crate::serde_helpers::is_false;
use arcweft_render_text::{RichTextParam, RichTextPresentation};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Typed presentation object tree for renderable and render-adjacent objects.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentPresentationTree {
    pub root: String,
    pub nodes: Vec<AgentPresentationTreeNode>,
}

/// One node in the typed presentation object tree.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentPresentationTreeNode {
    pub id: String,
    pub kind: AgentPresentationTreeNodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rich_text_kind: Option<AgentRichTextElementKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_depth: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<AgentPresentationEffectRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shaders: Vec<AgentPresentationShaderRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub object_proxy_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub object_proxies: Vec<AgentPresentationObjectProxyRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motion_function_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub has_transform: bool,
}

/// Typed presentation tree filter used by Agent resource readback.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AgentPresentationTreeQuery {
    pub role: Option<String>,
    pub rich_text_kind: Option<AgentRichTextElementKind>,
    pub object_layer: Option<String>,
    pub effect_id: Option<String>,
    pub shader_id: Option<String>,
    pub motion_function_id: Option<String>,
    pub object_proxy_id: Option<String>,
    pub object_proxy_type: Option<String>,
    pub object_proxy_role: Option<String>,
    pub object_proxy_struct: Option<String>,
    pub object_proxy_param: Option<AgentPresentationObjectProxyParamQuery>,
    pub has_transform: Option<bool>,
}

/// Presentation tree node category.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPresentationTreeNodeKind {
    Root,
    Layer,
    Object,
}

impl AgentPresentationTree {
    /// Builds a stable layer/object tree from observed layers and objects.
    pub fn from_layers_and_objects(
        layers: &[AgentObservedLayer],
        objects: &[AgentObservedObject],
    ) -> Self {
        let root = "presentation.root".to_owned();
        let layer_ids = presentation_tree_layer_ids(layers, objects);
        let object_ids = objects
            .iter()
            .map(|object| object.id.clone())
            .collect::<Vec<_>>();
        let mut children_by_parent =
            presentation_tree_children_by_parent(&root, &layer_ids, &object_ids, objects);

        let mut nodes = Vec::with_capacity(1 + layer_ids.len() + objects.len());
        nodes.push(agent_presentation_root_node(
            &root,
            children_by_parent.remove(&root).unwrap_or_default(),
        ));
        nodes.extend(layer_ids.iter().map(|layer_id| {
            agent_presentation_layer_node(&root, layer_id, &mut children_by_parent)
        }));
        nodes.extend(objects.iter().map(|object| {
            agent_presentation_object_node(object, &object_ids, &mut children_by_parent)
        }));

        Self { root, nodes }
    }

    /// Returns a pruned tree that keeps matching nodes and their ancestors.
    #[must_use]
    pub fn filtered(&self, query: &AgentPresentationTreeQuery) -> Self {
        if query.is_empty() {
            return self.clone();
        }

        let parent_by_id = self
            .nodes
            .iter()
            .filter_map(|node| {
                node.parent_id
                    .as_ref()
                    .map(|parent_id| (node.id.as_str(), parent_id.as_str()))
            })
            .collect::<BTreeMap<_, _>>();
        let mut included = BTreeSet::new();
        included.insert(self.root.as_str());

        for node in &self.nodes {
            if query.matches(node) {
                include_presentation_tree_ancestors(&node.id, &parent_by_id, &mut included);
            }
        }

        let nodes = self
            .nodes
            .iter()
            .filter(|node| included.contains(node.id.as_str()))
            .map(|node| {
                let mut node = node.clone();
                node.children
                    .retain(|child_id| included.contains(child_id.as_str()));
                node
            })
            .collect();

        Self {
            root: self.root.clone(),
            nodes,
        }
    }
}

impl AgentPresentationTreeQuery {
    /// Returns true when the query has no active filter fields.
    pub fn is_empty(&self) -> bool {
        self.role.is_none()
            && self.rich_text_kind.is_none()
            && self.object_layer.is_none()
            && self.effect_id.is_none()
            && self.shader_id.is_none()
            && self.motion_function_id.is_none()
            && self.object_proxy_id.is_none()
            && self.object_proxy_type.is_none()
            && self.object_proxy_role.is_none()
            && self.object_proxy_struct.is_none()
            && self.object_proxy_param.is_none()
            && self.has_transform.is_none()
    }

    fn matches(&self, node: &AgentPresentationTreeNode) -> bool {
        self.role
            .as_ref()
            .is_none_or(|role| node.role.as_ref() == Some(role))
            && self
                .rich_text_kind
                .is_none_or(|kind| node.rich_text_kind == Some(kind))
            && self
                .object_layer
                .as_ref()
                .is_none_or(|object_layer| node.object_layer.as_ref() == Some(object_layer))
            && self
                .effect_id
                .as_ref()
                .is_none_or(|effect_id| node.effects.iter().any(|effect| effect.id == *effect_id))
            && self
                .shader_id
                .as_ref()
                .is_none_or(|shader_id| node.shaders.iter().any(|shader| shader.id == *shader_id))
            && self
                .motion_function_id
                .as_ref()
                .is_none_or(|motion_function_id| {
                    node.motion_function_ids
                        .iter()
                        .any(|candidate| candidate == motion_function_id)
                })
            && self.object_proxy_id.as_ref().is_none_or(|object_proxy_id| {
                node.object_proxy_ids
                    .iter()
                    .any(|candidate| candidate == object_proxy_id)
            })
            && self
                .object_proxy_type
                .as_ref()
                .is_none_or(|object_proxy_type| {
                    node.object_proxies.iter().any(|proxy| {
                        proxy.type_name.as_ref() == Some(object_proxy_type)
                            || proxy.id == *object_proxy_type
                    })
                })
            && self
                .object_proxy_role
                .as_ref()
                .is_none_or(|object_proxy_role| {
                    node.object_proxies
                        .iter()
                        .any(|proxy| proxy.role.as_ref() == Some(object_proxy_role))
                })
            && self
                .object_proxy_struct
                .as_ref()
                .is_none_or(|object_proxy_struct| {
                    node.object_proxies.iter().any(|proxy| {
                        proxy.declaration.as_ref().is_some_and(|declaration| {
                            declaration.struct_name == *object_proxy_struct
                        })
                    })
                })
            && self.object_proxy_param.as_ref().is_none_or(|param_query| {
                node.object_proxies
                    .iter()
                    .any(|proxy| proxy_matches_param_query(proxy, param_query))
            })
            && self
                .has_transform
                .is_none_or(|has_transform| node.has_transform == has_transform)
    }
}

fn include_presentation_tree_ancestors<'a>(
    node_id: &'a str,
    parent_by_id: &BTreeMap<&'a str, &'a str>,
    included: &mut BTreeSet<&'a str>,
) {
    if included.insert(node_id)
        && let Some(parent_id) = parent_by_id.get(node_id)
    {
        include_presentation_tree_ancestors(parent_id, parent_by_id, included);
    }
}

fn presentation_tree_layer_ids(
    layers: &[AgentObservedLayer],
    objects: &[AgentObservedObject],
) -> Vec<String> {
    let mut layer_ids = layers
        .iter()
        .map(|layer| layer.id.clone())
        .collect::<Vec<_>>();
    for object in objects {
        if !layer_ids.iter().any(|layer_id| layer_id == &object.layer) {
            layer_ids.push(object.layer.clone());
        }
    }
    layer_ids
}

fn presentation_tree_children_by_parent(
    root: &str,
    layer_ids: &[String],
    object_ids: &[String],
    objects: &[AgentObservedObject],
) -> BTreeMap<String, Vec<String>> {
    let mut children_by_parent = BTreeMap::<String, Vec<String>>::new();
    children_by_parent.insert(
        root.to_owned(),
        layer_ids
            .iter()
            .map(|layer_id| presentation_layer_node_id(layer_id))
            .collect(),
    );
    for layer_id in layer_ids {
        children_by_parent
            .entry(presentation_layer_node_id(layer_id))
            .or_default();
    }
    for object in objects {
        children_by_parent
            .entry(presentation_tree_object_parent_id(object, object_ids))
            .or_default()
            .push(object.id.clone());
    }
    children_by_parent
}

fn agent_presentation_root_node(root: &str, children: Vec<String>) -> AgentPresentationTreeNode {
    AgentPresentationTreeNode {
        id: root.to_owned(),
        kind: AgentPresentationTreeNodeKind::Root,
        parent_id: None,
        children,
        layer_id: None,
        object_id: None,
        role: None,
        rich_text_kind: None,
        object_layer: None,
        object_depth: None,
        effects: Vec::new(),
        shaders: Vec::new(),
        object_proxy_ids: Vec::new(),
        object_proxies: Vec::new(),
        motion_function_ids: Vec::new(),
        has_transform: false,
    }
}

fn agent_presentation_layer_node(
    root: &str,
    layer_id: &str,
    children_by_parent: &mut BTreeMap<String, Vec<String>>,
) -> AgentPresentationTreeNode {
    let node_id = presentation_layer_node_id(layer_id);
    AgentPresentationTreeNode {
        id: node_id.clone(),
        kind: AgentPresentationTreeNodeKind::Layer,
        parent_id: Some(root.to_owned()),
        children: children_by_parent.remove(&node_id).unwrap_or_default(),
        layer_id: Some(layer_id.to_owned()),
        object_id: None,
        role: None,
        rich_text_kind: None,
        object_layer: None,
        object_depth: None,
        effects: Vec::new(),
        shaders: Vec::new(),
        object_proxy_ids: Vec::new(),
        object_proxies: Vec::new(),
        motion_function_ids: Vec::new(),
        has_transform: false,
    }
}

fn agent_presentation_object_node(
    object: &AgentObservedObject,
    object_ids: &[String],
    children_by_parent: &mut BTreeMap<String, Vec<String>>,
) -> AgentPresentationTreeNode {
    let rich_text_ref = object.rich_text_ref.as_ref();
    let presentation = rich_text_ref
        .and_then(|rich_text_ref| {
            rich_text_ref
                .presentation
                .as_ref()
                .map(agent_presentation_node_summary)
        })
        .or_else(|| agent_image_presentation_node_summary(object));
    AgentPresentationTreeNode {
        id: object.id.clone(),
        kind: AgentPresentationTreeNodeKind::Object,
        parent_id: Some(presentation_tree_object_parent_id(object, object_ids)),
        children: children_by_parent.remove(&object.id).unwrap_or_default(),
        layer_id: Some(object.layer.clone()),
        object_id: Some(object.id.clone()),
        role: Some(object.role.clone()),
        rich_text_kind: rich_text_ref.map(|rich_text_ref| rich_text_ref.kind),
        object_layer: object.resolved_object_layer(),
        object_depth: object.resolved_object_depth(),
        effects: presentation
            .as_ref()
            .map_or_else(Vec::new, |summary| summary.effects.clone()),
        shaders: presentation
            .as_ref()
            .map_or_else(Vec::new, |summary| summary.shaders.clone()),
        object_proxy_ids: presentation
            .as_ref()
            .map_or_else(Vec::new, |summary| summary.object_proxy_ids.clone()),
        object_proxies: presentation
            .as_ref()
            .map_or_else(Vec::new, |summary| summary.object_proxies.clone()),
        motion_function_ids: presentation
            .as_ref()
            .map_or_else(Vec::new, |summary| summary.motion_function_ids.clone()),
        has_transform: presentation.is_some_and(|summary| summary.has_transform),
    }
}

fn presentation_tree_object_parent_id(
    object: &AgentObservedObject,
    object_ids: &[String],
) -> String {
    object
        .parent_id
        .as_ref()
        .filter(|parent_id| object_ids.iter().any(|object_id| object_id == *parent_id))
        .cloned()
        .unwrap_or_else(|| presentation_layer_node_id(&object.layer))
}

#[derive(Clone, Debug, Default)]
struct AgentPresentationNodeSummary {
    effects: Vec<AgentPresentationEffectRef>,
    shaders: Vec<AgentPresentationShaderRef>,
    object_proxy_ids: Vec<String>,
    object_proxies: Vec<AgentPresentationObjectProxyRef>,
    motion_function_ids: Vec<String>,
    has_transform: bool,
}

fn agent_presentation_node_summary(
    presentation: &RichTextPresentation,
) -> AgentPresentationNodeSummary {
    AgentPresentationNodeSummary {
        effects: presentation
            .effects
            .iter()
            .map(|effect| AgentPresentationEffectRef {
                id: effect.id.clone(),
                phase: effect.phase,
            })
            .collect(),
        shaders: presentation
            .shaders
            .iter()
            .map(|shader| AgentPresentationShaderRef {
                id: shader.id.clone(),
                phase: shader.phase,
            })
            .collect(),
        object_proxy_ids: presentation
            .object_proxies
            .iter()
            .map(|proxy| proxy.id.clone())
            .collect(),
        object_proxies: presentation
            .object_proxies
            .iter()
            .map(agent_presentation_object_proxy_ref)
            .collect(),
        motion_function_ids: presentation
            .effects
            .iter()
            .filter(|effect| effect.id == "motion")
            .filter_map(|effect| match effect.params.get("fn") {
                Some(
                    RichTextParam::Text { value }
                    | RichTextParam::Raw { value }
                    | RichTextParam::Selector { value },
                ) => Some(value.clone()),
                _ => None,
            })
            .collect(),
        has_transform: presentation.transform.is_some(),
    }
}

fn agent_image_presentation_node_summary(
    object: &AgentObservedObject,
) -> Option<AgentPresentationNodeSummary> {
    let AgentObservedObjectContent::Image(content) = &object.content else {
        return None;
    };
    (!content.proxies.is_empty()).then(|| AgentPresentationNodeSummary {
        object_proxy_ids: content
            .proxies
            .iter()
            .map(|proxy| proxy.id.clone())
            .collect(),
        object_proxies: content.proxies.clone(),
        ..AgentPresentationNodeSummary::default()
    })
}

fn presentation_layer_node_id(layer_id: &str) -> String {
    format!("presentation.layer.{layer_id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::AgentPresentationObjectProxyParamQuery;
    use crate::rich_text::AgentRichTextElementKind;
    use arcweft_render_text::{RichTextEffectPhase, RichTextObjectProxyDeclaration, RichTextParam};
    use std::collections::BTreeMap;

    #[test]
    fn presentation_tree_filter_keeps_matching_objects_and_ancestors() {
        let tree = presentation_filter_fixture();

        let filtered = tree.filtered(&AgentPresentationTreeQuery {
            shader_id: Some("warm_glow".to_owned()),
            motion_function_id: Some("breath_orbit".to_owned()),
            has_transform: Some(true),
            ..AgentPresentationTreeQuery::default()
        });

        assert_eq!(
            node_ids(&filtered),
            vec![
                "presentation.root",
                "presentation.layer.dialogue",
                "object.dialogue.0.0"
            ]
        );

        let proxy_filtered = tree.filtered(&AgentPresentationTreeQuery {
            object_proxy_id: Some("hotspot".to_owned()),
            rich_text_kind: Some(AgentRichTextElementKind::TextObjectProxy),
            ..AgentPresentationTreeQuery::default()
        });
        assert_eq!(
            node_ids(&proxy_filtered),
            vec![
                "presentation.root",
                "presentation.layer.dialogue",
                "object.dialogue.0.0",
                "object.dialogue.0.0.proxy.0"
            ]
        );

        let typed_proxy_filtered = tree.filtered(&AgentPresentationTreeQuery {
            object_proxy_type: Some("KeywordHit".to_owned()),
            object_proxy_role: Some("keyword".to_owned()),
            object_proxy_struct: Some("KeywordHit".to_owned()),
            ..AgentPresentationTreeQuery::default()
        });
        assert_eq!(
            node_ids(&typed_proxy_filtered),
            vec![
                "presentation.root",
                "presentation.layer.dialogue",
                "object.dialogue.0.0",
                "object.dialogue.0.0.proxy.0"
            ]
        );

        let proxy_param_filtered = tree.filtered(&AgentPresentationTreeQuery {
            object_proxy_param: Some(AgentPresentationObjectProxyParamQuery {
                key: "channel".to_owned(),
                value: Some("choice".to_owned()),
            }),
            ..AgentPresentationTreeQuery::default()
        });
        assert_eq!(
            node_ids(&proxy_param_filtered),
            vec![
                "presentation.root",
                "presentation.layer.dialogue",
                "object.dialogue.0.0",
                "object.dialogue.0.0.proxy.0"
            ]
        );

        let empty_filtered = tree.filtered(&AgentPresentationTreeQuery {
            shader_id: Some("missing".to_owned()),
            ..AgentPresentationTreeQuery::default()
        });
        assert_eq!(empty_filtered.nodes.len(), 1);
        assert_eq!(empty_filtered.nodes[0].id, "presentation.root");
        assert!(empty_filtered.nodes[0].children.is_empty());
    }

    fn node_ids(tree: &AgentPresentationTree) -> Vec<&str> {
        tree.nodes.iter().map(|node| node.id.as_str()).collect()
    }

    fn presentation_filter_fixture() -> AgentPresentationTree {
        AgentPresentationTree {
            root: "presentation.root".to_owned(),
            nodes: vec![
                root_node(),
                dialogue_layer_node(),
                animated_text_node(),
                proxy_object_node(),
                sibling_text_node(),
            ],
        }
    }

    fn base_node(id: &str, kind: AgentPresentationTreeNodeKind) -> AgentPresentationTreeNode {
        AgentPresentationTreeNode {
            id: id.to_owned(),
            kind,
            parent_id: None,
            children: Vec::new(),
            layer_id: None,
            object_id: None,
            role: None,
            rich_text_kind: None,
            object_layer: None,
            object_depth: None,
            effects: Vec::new(),
            shaders: Vec::new(),
            object_proxy_ids: Vec::new(),
            object_proxies: Vec::new(),
            motion_function_ids: Vec::new(),
            has_transform: false,
        }
    }

    fn root_node() -> AgentPresentationTreeNode {
        AgentPresentationTreeNode {
            children: vec!["presentation.layer.dialogue".to_owned()],
            ..base_node("presentation.root", AgentPresentationTreeNodeKind::Root)
        }
    }

    fn dialogue_layer_node() -> AgentPresentationTreeNode {
        AgentPresentationTreeNode {
            parent_id: Some("presentation.root".to_owned()),
            children: vec![
                "object.dialogue.0.0".to_owned(),
                "object.dialogue.0.1".to_owned(),
            ],
            layer_id: Some("dialogue".to_owned()),
            ..base_node(
                "presentation.layer.dialogue",
                AgentPresentationTreeNodeKind::Layer,
            )
        }
    }

    fn animated_text_node() -> AgentPresentationTreeNode {
        AgentPresentationTreeNode {
            parent_id: Some("presentation.layer.dialogue".to_owned()),
            children: vec!["object.dialogue.0.0.proxy.0".to_owned()],
            layer_id: Some("dialogue.rich_text".to_owned()),
            object_id: Some("object.dialogue.0.0".to_owned()),
            role: Some("rich_text_run".to_owned()),
            rich_text_kind: Some(AgentRichTextElementKind::TextRun),
            object_layer: Some("view".to_owned()),
            object_depth: Some(4000),
            effects: vec![AgentPresentationEffectRef {
                id: "motion".to_owned(),
                phase: RichTextEffectPhase::GlyphTransform,
            }],
            shaders: vec![AgentPresentationShaderRef {
                id: "warm_glow".to_owned(),
                phase: RichTextEffectPhase::RunOffscreenPass,
            }],
            motion_function_ids: vec!["breath_orbit".to_owned()],
            has_transform: true,
            ..base_node("object.dialogue.0.0", AgentPresentationTreeNodeKind::Object)
        }
    }

    fn proxy_object_node() -> AgentPresentationTreeNode {
        AgentPresentationTreeNode {
            parent_id: Some("object.dialogue.0.0".to_owned()),
            layer_id: Some("dialogue.rich_text".to_owned()),
            object_id: Some("object.dialogue.0.0.proxy.0".to_owned()),
            role: Some("rich_text_proxy".to_owned()),
            rich_text_kind: Some(AgentRichTextElementKind::TextObjectProxy),
            object_layer: Some("hit".to_owned()),
            object_depth: Some(4100),
            object_proxy_ids: vec!["hotspot".to_owned()],
            object_proxies: vec![hotspot_proxy()],
            ..base_node(
                "object.dialogue.0.0.proxy.0",
                AgentPresentationTreeNodeKind::Object,
            )
        }
    }

    fn hotspot_proxy() -> AgentPresentationObjectProxyRef {
        AgentPresentationObjectProxyRef {
            id: "hotspot".to_owned(),
            type_name: Some("KeywordHit".to_owned()),
            role: Some("keyword".to_owned()),
            layer: Some("hit".to_owned()),
            depth: Some(4100),
            declaration: Some(RichTextObjectProxyDeclaration {
                struct_name: "KeywordHit".to_owned(),
                attribute: "text_proxy".to_owned(),
            }),
            hit_test: true,
            params: BTreeMap::from([(
                "channel".to_owned(),
                RichTextParam::Selector {
                    value: "choice".to_owned(),
                },
            )]),
        }
    }

    fn sibling_text_node() -> AgentPresentationTreeNode {
        AgentPresentationTreeNode {
            parent_id: Some("presentation.layer.dialogue".to_owned()),
            layer_id: Some("dialogue.rich_text".to_owned()),
            object_id: Some("object.dialogue.0.1".to_owned()),
            role: Some("rich_text_run".to_owned()),
            rich_text_kind: Some(AgentRichTextElementKind::TextRun),
            object_layer: Some("view".to_owned()),
            ..base_node("object.dialogue.0.1", AgentPresentationTreeNodeKind::Object)
        }
    }
}
