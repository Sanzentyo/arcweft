use arcweft_presentation::input::InputEvent;
use arcweft_presentation::interaction::InteractionState;
use arcweft_presentation::layer::{LayerId, LayerTree};
use arcweft_presentation::semantic::SemanticTree;
use arcweft_ui::{
    DisplayItemKind, DisplayList, ImageId, LayoutBox, NodeId, ResolvedDisplayList, UiError,
    UiHandlerInvocation, UiHandlerRouteTable, UiLayerOutput, UiSemanticFragment, UiSemanticNode,
    UiStyleTable,
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// One committed UI layer payload after component rendering, layout, and semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct UiFrameLayer {
    layer: LayerId,
    display: DisplayList,
    semantic_fragment: UiSemanticFragment,
    semantics: SemanticTree,
    handlers: UiHandlerRouteTable,
    styles: UiStyleTable,
}

/// Ordered UI frame payload ready for host renderer and Agent observation phases.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiFrameCommit {
    layers: Vec<UiFrameLayer>,
}

/// One UI layer after interaction selectors have been resolved for a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct UiFrameResolvedLayer {
    layer: LayerId,
    display: ResolvedDisplayList,
}

/// One image display item in a committed UI frame with its render layer context.
#[derive(Clone, Debug, PartialEq)]
pub struct UiFrameImageItem {
    layer: LayerId,
    node: NodeId,
    image: ImageId,
    layout: LayoutBox,
    semantic: Option<UiSemanticNode>,
}

/// Builder that validates UI frame output against the committed `LayerTree`.
#[derive(Clone, Debug, PartialEq)]
pub struct UiFrameCommitBuilder {
    render_order: Vec<LayerId>,
    known_layers: BTreeSet<LayerId>,
    layers: BTreeMap<LayerId, UiFrameLayer>,
}

/// Error while collecting a UI frame commit.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum UiFrameCommitError {
    #[error("UI frame layer is not present in the committed LayerTree: {0:?}")]
    UnknownLayer(LayerId),
    #[error("UI frame layer was submitted more than once: {0:?}")]
    DuplicateLayer(LayerId),
}

impl UiFrameLayer {
    pub fn new(
        layer: LayerId,
        display: DisplayList,
        semantic_fragment: UiSemanticFragment,
    ) -> Self {
        let semantics = semantic_fragment.to_semantic_tree();
        Self {
            layer,
            display,
            semantic_fragment,
            semantics,
            handlers: UiHandlerRouteTable::default(),
            styles: UiStyleTable::default(),
        }
    }

    pub fn from_output(layer: LayerId, output: UiLayerOutput) -> Self {
        let (display, semantic_fragment, handlers, styles) = output.into_frame_parts();
        let semantics = semantic_fragment.to_semantic_tree();
        Self {
            layer,
            display,
            semantic_fragment,
            semantics,
            handlers,
            styles,
        }
    }

    pub const fn layer(&self) -> &LayerId {
        &self.layer
    }

    pub const fn display(&self) -> &DisplayList {
        &self.display
    }

    pub const fn semantics(&self) -> &SemanticTree {
        &self.semantics
    }

    pub const fn semantic_fragment(&self) -> &UiSemanticFragment {
        &self.semantic_fragment
    }

    pub const fn handlers(&self) -> &UiHandlerRouteTable {
        &self.handlers
    }

    pub const fn styles(&self) -> &UiStyleTable {
        &self.styles
    }

    pub fn dispatch_input(&self, input: &InputEvent) -> Vec<UiHandlerInvocation> {
        self.handlers.dispatch_input(input)
    }

    pub fn resolve_interaction_styles(
        &self,
        interaction: &InteractionState,
    ) -> Result<UiFrameResolvedLayer, UiError> {
        self.display
            .resolve_interaction_styles(&self.semantic_fragment, &self.styles, interaction)
            .map(|display| UiFrameResolvedLayer {
                layer: self.layer.clone(),
                display,
            })
    }

    pub fn image_items(&self) -> Vec<UiFrameImageItem> {
        self.display
            .as_slice()
            .iter()
            .filter_map(|item| match item.kind() {
                DisplayItemKind::Image(image) => Some(UiFrameImageItem::new(
                    self.layer.clone(),
                    item.node(),
                    image,
                    item.layout(),
                    item.semantics().and_then(|semantic| {
                        self.semantic_fragment()
                            .as_slice()
                            .get(semantic.0 as usize)
                            .cloned()
                    }),
                )),
                DisplayItemKind::Text(_)
                | DisplayItemKind::RichText(_)
                | DisplayItemKind::Custom(_) => None,
            })
            .collect()
    }
}

impl UiFrameResolvedLayer {
    pub const fn layer(&self) -> &LayerId {
        &self.layer
    }

    pub const fn display(&self) -> &ResolvedDisplayList {
        &self.display
    }

    pub fn into_display(self) -> ResolvedDisplayList {
        self.display
    }
}

impl UiFrameImageItem {
    pub const fn new(
        layer: LayerId,
        node: NodeId,
        image: ImageId,
        layout: LayoutBox,
        semantic: Option<UiSemanticNode>,
    ) -> Self {
        Self {
            layer,
            node,
            image,
            layout,
            semantic,
        }
    }

    pub const fn layer(&self) -> &LayerId {
        &self.layer
    }

    pub const fn node(&self) -> NodeId {
        self.node
    }

    pub const fn image(&self) -> ImageId {
        self.image
    }

    pub const fn layout(&self) -> LayoutBox {
        self.layout
    }

    pub const fn semantic(&self) -> Option<&UiSemanticNode> {
        self.semantic.as_ref()
    }
}

impl UiFrameCommit {
    pub fn as_slice(&self) -> &[UiFrameLayer] {
        &self.layers
    }

    pub fn merged_semantics(&self) -> SemanticTree {
        let mut tree = SemanticTree::default();
        self.layers
            .iter()
            .flat_map(|layer| layer.semantics().as_slice().iter().cloned())
            .for_each(|node| tree.push(node));
        tree
    }

    pub fn dispatch_input(&self, input: &InputEvent) -> Vec<UiHandlerInvocation> {
        self.layers
            .iter()
            .flat_map(|layer| layer.dispatch_input(input))
            .collect()
    }

    pub fn resolve_interaction_styles(
        &self,
        interaction: &InteractionState,
    ) -> Result<Vec<UiFrameResolvedLayer>, UiError> {
        self.layers
            .iter()
            .map(|layer| layer.resolve_interaction_styles(interaction))
            .collect()
    }

    pub fn image_items(&self) -> Vec<UiFrameImageItem> {
        self.layers
            .iter()
            .flat_map(UiFrameLayer::image_items)
            .collect()
    }

    pub fn into_vec(self) -> Vec<UiFrameLayer> {
        self.layers
    }
}

impl UiFrameCommitBuilder {
    pub fn new(layer_tree: &LayerTree) -> Self {
        Self {
            render_order: layer_tree.render_order().to_vec(),
            known_layers: layer_tree.render_order().iter().cloned().collect(),
            layers: BTreeMap::new(),
        }
    }

    pub fn push_layer(
        &mut self,
        layer: LayerId,
        output: UiLayerOutput,
    ) -> Result<(), UiFrameCommitError> {
        if !self.known_layers.contains(&layer) {
            return Err(UiFrameCommitError::UnknownLayer(layer));
        }
        if self.layers.contains_key(&layer) {
            return Err(UiFrameCommitError::DuplicateLayer(layer));
        }
        let frame_layer = UiFrameLayer::from_output(layer.clone(), output);
        self.layers.insert(layer, frame_layer);
        Ok(())
    }

    pub fn finish(self) -> UiFrameCommit {
        let layers = self
            .render_order
            .into_iter()
            .filter_map(|layer| self.layers.get(&layer).cloned())
            .collect();
        UiFrameCommit { layers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PresentationActionDestination, dispatch_presentation_action};
    use arcweft_id::PublicId;
    use arcweft_presentation::hit::HitRect;
    use arcweft_presentation::input::{ActionTarget, InteractionTarget};
    use arcweft_presentation::layer::{LayerKind, LayerNode, LayerOrder, RenderPhase};
    use arcweft_presentation::semantic::SemanticRole;
    use arcweft_ui::{
        FragmentKind, ImageId, LayoutLength, LayoutPoint, LayoutResults, LayoutSize, LayoutTree,
        NodeKey, StyleId, UiSemanticFragment, UiSemanticFragmentBuilder, UiSemanticNode,
        ViewFragmentBuilder,
    };

    fn public_id(name: &str) -> PublicId {
        PublicId::try_new(name).unwrap()
    }

    fn layer_id(name: &str) -> LayerId {
        LayerId::new(public_id(&format!("layer.{name}")))
    }

    fn target(name: &str) -> InteractionTarget {
        InteractionTarget::new(public_id(&format!("target.{name}")))
    }

    fn order(phase: RenderPhase, z: i32) -> LayerOrder {
        LayerOrder {
            phase,
            z,
            stable_index: 0,
        }
    }

    fn semantic_fragment(
        key: u64,
        layer: LayerId,
        target: InteractionTarget,
        action: PublicId,
    ) -> UiSemanticFragment {
        let mut builder = UiSemanticFragmentBuilder::default();
        builder
            .push(
                UiSemanticNode::new(
                    NodeKey(key),
                    layer,
                    target,
                    SemanticRole::Button,
                    HitRect::new(0.0, 0.0, 20.0, 20.0),
                )
                .with_action(action),
            )
            .unwrap();
        builder.finish()
    }

    fn layer_output(
        key: u64,
        layer: LayerId,
        target: InteractionTarget,
        action: PublicId,
    ) -> UiLayerOutput {
        UiLayerOutput::new(
            DisplayList::default(),
            semantic_fragment(key, layer, target, action),
        )
    }

    fn image_layer_output(key: u64, image: ImageId, x: i32, y: i32) -> UiLayerOutput {
        let mut fragment = ViewFragmentBuilder::default();
        let image_node = fragment
            .push_node(
                NodeKey(key),
                FragmentKind::Image(image),
                StyleId(0),
                &[],
                &[],
                None,
            )
            .unwrap();
        let fragment = fragment.finish();
        let layout_tree = LayoutTree::from_fragment(&fragment).unwrap();
        let mut layouts = LayoutResults::new(&layout_tree);
        layouts
            .set(
                image_node,
                LayoutBox::new(
                    LayoutPoint::new(LayoutLength::px(x), LayoutLength::px(y)),
                    LayoutSize::new(LayoutLength::px(64), LayoutLength::px(32)),
                ),
            )
            .unwrap();
        UiLayerOutput::new(
            DisplayList::from_fragment(&fragment, &layouts).unwrap(),
            UiSemanticFragment::default(),
        )
    }

    #[test]
    fn ui_frame_commit_orders_layers_by_committed_layer_tree() {
        let root = layer_id("root");
        let ui = layer_id("ui");
        let modal = layer_id("modal");
        let mut layers = LayerTree::new(LayerNode::new(
            root.clone(),
            LayerKind::Root,
            order(RenderPhase::Background, 0),
        ));
        layers
            .insert(
                LayerNode::new(ui.clone(), LayerKind::GameUi, order(RenderPhase::GameUi, 0))
                    .with_parent(root.clone()),
            )
            .unwrap();
        layers
            .insert(
                LayerNode::new(
                    modal.clone(),
                    LayerKind::Modal,
                    order(RenderPhase::Modal, 0),
                )
                .with_parent(root),
            )
            .unwrap();

        let mut builder = UiFrameCommitBuilder::new(&layers);
        builder
            .push_layer(
                modal.clone(),
                layer_output(
                    1,
                    modal.clone(),
                    target("modal.close"),
                    public_id("action.close"),
                ),
            )
            .unwrap();
        builder
            .push_layer(
                ui.clone(),
                layer_output(
                    2,
                    ui.clone(),
                    target("ui.confirm"),
                    public_id("action.confirm"),
                ),
            )
            .unwrap();

        let commit = builder.finish();
        let committed_layers = commit
            .as_slice()
            .iter()
            .map(UiFrameLayer::layer)
            .collect::<Vec<_>>();
        assert_eq!(committed_layers, vec![&ui, &modal]);
    }

    #[test]
    fn ui_frame_commit_rejects_unknown_and_duplicate_layers_without_replacing_existing() {
        let root = layer_id("root");
        let ui = layer_id("ui");
        let missing = layer_id("missing");
        let mut layers = LayerTree::new(LayerNode::new(
            root.clone(),
            LayerKind::Root,
            order(RenderPhase::Background, 0),
        ));
        layers
            .insert(
                LayerNode::new(ui.clone(), LayerKind::GameUi, order(RenderPhase::GameUi, 0))
                    .with_parent(root),
            )
            .unwrap();

        let mut builder = UiFrameCommitBuilder::new(&layers);
        assert_eq!(
            builder.push_layer(missing.clone(), UiLayerOutput::default()),
            Err(UiFrameCommitError::UnknownLayer(missing))
        );
        builder
            .push_layer(
                ui.clone(),
                layer_output(1, ui.clone(), target("ui.first"), public_id("action.first")),
            )
            .unwrap();
        assert_eq!(
            builder.push_layer(
                ui.clone(),
                layer_output(
                    2,
                    ui.clone(),
                    target("ui.second"),
                    public_id("action.second"),
                ),
            ),
            Err(UiFrameCommitError::DuplicateLayer(ui.clone()))
        );

        let commit = builder.finish();
        assert!(
            commit
                .merged_semantics()
                .find(&target("ui.first"))
                .is_some()
        );
        assert!(
            commit
                .merged_semantics()
                .find(&target("ui.second"))
                .is_none()
        );
    }

    #[test]
    fn ui_frame_commit_semantics_feed_presentation_dispatch() {
        let root = layer_id("root");
        let ui = layer_id("ui");
        let button = target("ui.confirm");
        let action = public_id("action.confirm");
        let mut layers = LayerTree::new(LayerNode::new(
            root.clone(),
            LayerKind::Root,
            order(RenderPhase::Background, 0),
        ));
        layers
            .insert(
                LayerNode::new(ui.clone(), LayerKind::GameUi, order(RenderPhase::GameUi, 0))
                    .with_parent(root),
            )
            .unwrap();

        let mut builder = UiFrameCommitBuilder::new(&layers);
        builder
            .push_layer(
                ui,
                layer_output(1, layer_id("ui"), button.clone(), action.clone()),
            )
            .unwrap();
        let semantics = builder.finish().merged_semantics();
        let dispatched = dispatch_presentation_action(
            &semantics,
            semantics.lower_action(&button, &action).unwrap(),
        )
        .unwrap();

        assert_eq!(
            dispatched.destination(),
            PresentationActionDestination::UiEntity
        );
        assert_eq!(dispatched.action().target(), &ActionTarget::Entity(button));
    }

    #[test]
    fn ui_frame_commit_lists_image_items_in_render_order() {
        let root = layer_id("root");
        let hud = layer_id("hud");
        let modal = layer_id("modal");
        let mut layers = LayerTree::new(LayerNode::new(
            root.clone(),
            LayerKind::Root,
            order(RenderPhase::Background, 0),
        ));
        layers
            .insert(
                LayerNode::new(
                    hud.clone(),
                    LayerKind::GameUi,
                    order(RenderPhase::GameUi, 0),
                )
                .with_parent(root.clone()),
            )
            .unwrap();
        layers
            .insert(
                LayerNode::new(
                    modal.clone(),
                    LayerKind::Modal,
                    order(RenderPhase::Modal, 0),
                )
                .with_parent(root),
            )
            .unwrap();

        let mut builder = UiFrameCommitBuilder::new(&layers);
        builder
            .push_layer(modal.clone(), image_layer_output(20, ImageId(2), 30, 40))
            .unwrap();
        builder
            .push_layer(hud.clone(), image_layer_output(10, ImageId(1), 10, 20))
            .unwrap();
        let images = builder.finish().image_items();

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].layer(), &hud);
        assert_eq!(images[0].image(), ImageId(1));
        assert_eq!(images[0].node(), NodeId(0));
        assert_eq!(images[0].layout().origin.x, LayoutLength::px(10));
        assert_eq!(images[1].layer(), &modal);
        assert_eq!(images[1].image(), ImageId(2));
        assert_eq!(images[1].layout().origin.y, LayoutLength::px(40));
    }
}
