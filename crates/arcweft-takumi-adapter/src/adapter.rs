use crate::{
    diagnostic::TakumiAdapterError,
    metadata::{ArcweftNodeMetadata, TakumiMetadataMap, TakumiPath},
    style::{DirectCssSupport, TakumiCssBundle},
    text::ArcweftTextLayoutBridge,
};
use arcweft_id::PublicId;
use arcweft_view::{
    ContainerKind, FragmentKind, ImageId, NodeId, ViewFragment, ViewId, ViewPartId, ViewProgram,
};
use takumi::prelude::{Node, StyleSheet};

#[derive(Clone, Debug)]
pub struct TakumiAdapterInput<'a> {
    pub fragment: &'a ViewFragment,
    pub root: NodeId,
    pub stylesheets: TakumiCssBundle,
    pub text: &'a ArcweftTextLayoutBridge,
    pub view: Option<ViewId>,
    pub program: Option<&'a ViewProgram>,
    pub node_parts: &'a [(NodeId, ViewPartId)],
    pub agent: Option<&'a PublicId>,
}

#[derive(Clone, Debug)]
pub struct TakumiAdapterOutput {
    pub node: Node,
    pub stylesheet: StyleSheet,
    pub metadata: TakumiMetadataMap,
    pub direct_css_support: DirectCssSupport,
}

#[derive(Clone, Debug, Default)]
pub struct TakumiAdapter;

impl TakumiAdapter {
    pub fn adapt(
        input: &TakumiAdapterInput<'_>,
    ) -> Result<TakumiAdapterOutput, TakumiAdapterError> {
        let stylesheet = input.stylesheets.parse()?;
        let direct_css_support = input.stylesheets.direct_support();
        let mut metadata = TakumiMetadataMap::default();
        let node = Self::build_node(input, input.root, TakumiPath::root(), &mut metadata)?;

        Ok(TakumiAdapterOutput {
            node,
            stylesheet,
            metadata,
            direct_css_support,
        })
    }

    fn build_node(
        input: &TakumiAdapterInput<'_>,
        node_id: NodeId,
        path: TakumiPath,
        metadata: &mut TakumiMetadataMap,
    ) -> Result<Node, TakumiAdapterError> {
        let fragment_node = input
            .fragment
            .nodes()
            .get(node_id.0 as usize)
            .ok_or(TakumiAdapterError::MissingFragmentRoot(node_id))?;
        let event_bindings = input.fragment.node_events(node_id).unwrap_or(&[]);
        let mut arcweft_metadata =
            ArcweftNodeMetadata::from_fragment_node(node_id, fragment_node, event_bindings);

        if let Some(view) = input.view {
            arcweft_metadata = arcweft_metadata.with_view(view);
        }
        if let Some(program) = input.program {
            arcweft_metadata = arcweft_metadata.with_program(program.id());
        }
        if let Some(part) = input
            .node_parts
            .iter()
            .find_map(|(candidate, part)| (*candidate == node_id).then_some(*part))
        {
            arcweft_metadata = arcweft_metadata.with_part(part);
        }
        if let Some(agent) = input.agent {
            arcweft_metadata = arcweft_metadata.with_agent(agent.clone());
        }

        let children = input
            .fragment
            .node_children(node_id)
            .unwrap_or(&[])
            .iter()
            .copied()
            .enumerate()
            .map(|(index, child)| Self::build_node(input, child, path.child(index), metadata))
            .collect::<Result<Vec<_>, _>>()?;

        let tag_name = tag_name(fragment_node.kind());
        let class_name = class_name(fragment_node.kind());
        let attributes = arcweft_metadata.attributes(&path);
        metadata.push(path, arcweft_metadata);

        Ok(match fragment_node.kind() {
            FragmentKind::Container(_) | FragmentKind::View(_) | FragmentKind::Custom(_) => {
                Node::container(children)
            }
            FragmentKind::Text(_) | FragmentKind::RichText(_) => {
                Node::text(input.text.placeholder_text(node_id).unwrap_or("\u{fffc}"))
            }
            FragmentKind::Image(image) => Node::image(image_url(image)),
        }
        .with_tag_name(tag_name)
        .with_class_name(class_name)
        .with_id(format!("aw-node-{}", node_id.0))
        .with_attributes(attributes))
    }
}

fn image_url(image: ImageId) -> String {
    format!("arcweft://image/{}", image.0)
}

fn class_name(kind: FragmentKind) -> &'static str {
    match kind {
        FragmentKind::Container(ContainerKind::Block) => "aw-container aw-block",
        FragmentKind::Container(ContainerKind::Inline) => "aw-container aw-inline",
        FragmentKind::Container(ContainerKind::Stack) => "aw-container aw-stack",
        FragmentKind::Text(_) => "aw-text",
        FragmentKind::RichText(_) => "aw-rich-text",
        FragmentKind::Image(_) => "aw-image",
        FragmentKind::View(_) => "aw-view",
        FragmentKind::Custom(_) => "aw-custom",
    }
}

fn tag_name(kind: FragmentKind) -> &'static str {
    match kind {
        FragmentKind::Container(ContainerKind::Inline)
        | FragmentKind::Text(_)
        | FragmentKind::RichText(_) => "span",
        FragmentKind::Image(_) => "img",
        FragmentKind::Container(ContainerKind::Block | ContainerKind::Stack)
        | FragmentKind::View(_)
        | FragmentKind::Custom(_) => "div",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_view::{
        EventBinding, EventKind, HandlerId, NodeKey, SemanticSpecId, StyleId, ViewFragmentBuilder,
    };

    #[test]
    fn adapter_preserves_metadata_sidecar_and_takumi_attributes() {
        let mut builder = ViewFragmentBuilder::default();
        let text = builder
            .push_node(
                NodeKey(1),
                FragmentKind::Text(arcweft_view::TextSourceId(1)),
                StyleId(2),
                &[],
                &[EventBinding::new(EventKind::Activate, HandlerId(3))],
                Some(SemanticSpecId(4)),
            )
            .expect("text node builds");
        let root = builder
            .push_node(
                NodeKey(5),
                FragmentKind::Container(ContainerKind::Block),
                StyleId(6),
                &[text],
                &[],
                None,
            )
            .expect("root node builds");
        let fragment = builder.finish();

        let output = TakumiAdapter::adapt(&TakumiAdapterInput {
            fragment: &fragment,
            root,
            stylesheets: TakumiCssBundle::new([".aw-text { opacity: 1; }"]),
            text: &ArcweftTextLayoutBridge::default(),
            view: Some(ViewId(7)),
            program: None,
            node_parts: &[(text, ViewPartId(8))],
            agent: None,
        })
        .expect("adapter output");

        let metadata = output.metadata.get_by_node(text).expect("text metadata");
        assert_eq!(metadata.view(), Some(ViewId(7)));
        assert_eq!(metadata.part(), Some(ViewPartId(8)));
        assert_eq!(metadata.semantic(), Some(SemanticSpecId(4)));
        assert_eq!(metadata.handlers(), &[HandlerId(3)]);
    }
}
