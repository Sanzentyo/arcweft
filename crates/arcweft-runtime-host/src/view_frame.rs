use arcweft_presentation::input::InputEvent;
use arcweft_presentation::interaction::InteractionState;
use arcweft_presentation::layer::{LayerId, LayerTree};
use arcweft_presentation::semantic::SemanticTree;
use arcweft_view::{
    DisplayItemKind, DisplayList, ImageId, LayoutBox, NodeId, ResolvedDisplayList, ViewError,
    ViewHandlerInvocation, ViewHandlerRouteTable, ViewLayerOutput, ViewSemanticFragment,
    ViewSemanticNode, ViewStyleTable,
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// One committed View layer payload after component rendering, layout, and semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewFrameLayer {
    layer: LayerId,
    display: DisplayList,
    semantic_fragment: ViewSemanticFragment,
    semantics: SemanticTree,
    handlers: ViewHandlerRouteTable,
    styles: ViewStyleTable,
}

/// Ordered View frame payload ready for host renderer and Agent observation phases.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewFrameCommit {
    layers: Vec<ViewFrameLayer>,
}

/// One View layer after interaction selectors have been resolved for a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewFrameResolvedLayer {
    layer: LayerId,
    display: ResolvedDisplayList,
}

/// One image display item in a committed View frame with its render layer context.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewFrameImageItem {
    layer: LayerId,
    node: NodeId,
    image: ImageId,
    layout: LayoutBox,
    semantic: Option<ViewSemanticNode>,
}

/// Builder that validates View frame output against the committed `LayerTree`.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewFrameCommitBuilder {
    render_order: Vec<LayerId>,
    known_layers: BTreeSet<LayerId>,
    layers: BTreeMap<LayerId, ViewFrameLayer>,
}

/// Error while collecting a View frame commit.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ViewFrameCommitError {
    #[error("View frame layer is not present in the committed LayerTree: {0:?}")]
    UnknownLayer(LayerId),
    #[error("View frame layer was submitted more than once: {0:?}")]
    DuplicateLayer(LayerId),
}

impl ViewFrameLayer {
    pub fn new(
        layer: LayerId,
        display: DisplayList,
        semantic_fragment: ViewSemanticFragment,
    ) -> Self {
        let semantics = semantic_fragment.to_semantic_tree();
        Self {
            layer,
            display,
            semantic_fragment,
            semantics,
            handlers: ViewHandlerRouteTable::default(),
            styles: ViewStyleTable::default(),
        }
    }

    pub fn from_output(layer: LayerId, output: ViewLayerOutput) -> Self {
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

    pub const fn semantic_fragment(&self) -> &ViewSemanticFragment {
        &self.semantic_fragment
    }

    pub const fn handlers(&self) -> &ViewHandlerRouteTable {
        &self.handlers
    }

    pub const fn styles(&self) -> &ViewStyleTable {
        &self.styles
    }

    pub fn dispatch_input(&self, input: &InputEvent) -> Vec<ViewHandlerInvocation> {
        self.handlers.dispatch_input(input)
    }

    pub fn resolve_interaction_styles(
        &self,
        interaction: &InteractionState,
    ) -> Result<ViewFrameResolvedLayer, ViewError> {
        self.display
            .resolve_interaction_styles(&self.semantic_fragment, &self.styles, interaction)
            .map(|display| ViewFrameResolvedLayer {
                layer: self.layer.clone(),
                display,
            })
    }

    pub fn image_items(&self) -> Vec<ViewFrameImageItem> {
        self.display
            .as_slice()
            .iter()
            .filter_map(|item| match item.kind() {
                DisplayItemKind::Image(image) => Some(ViewFrameImageItem::new(
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

impl ViewFrameResolvedLayer {
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

impl ViewFrameImageItem {
    pub const fn new(
        layer: LayerId,
        node: NodeId,
        image: ImageId,
        layout: LayoutBox,
        semantic: Option<ViewSemanticNode>,
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

    pub const fn semantic(&self) -> Option<&ViewSemanticNode> {
        self.semantic.as_ref()
    }
}

impl ViewFrameCommit {
    pub fn as_slice(&self) -> &[ViewFrameLayer] {
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

    pub fn dispatch_input(&self, input: &InputEvent) -> Vec<ViewHandlerInvocation> {
        self.layers
            .iter()
            .flat_map(|layer| layer.dispatch_input(input))
            .collect()
    }

    pub fn resolve_interaction_styles(
        &self,
        interaction: &InteractionState,
    ) -> Result<Vec<ViewFrameResolvedLayer>, ViewError> {
        self.layers
            .iter()
            .map(|layer| layer.resolve_interaction_styles(interaction))
            .collect()
    }

    pub fn image_items(&self) -> Vec<ViewFrameImageItem> {
        self.layers
            .iter()
            .flat_map(ViewFrameLayer::image_items)
            .collect()
    }

    pub fn into_vec(self) -> Vec<ViewFrameLayer> {
        self.layers
    }
}

impl ViewFrameCommitBuilder {
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
        output: ViewLayerOutput,
    ) -> Result<(), ViewFrameCommitError> {
        if !self.known_layers.contains(&layer) {
            return Err(ViewFrameCommitError::UnknownLayer(layer));
        }
        if self.layers.contains_key(&layer) {
            return Err(ViewFrameCommitError::DuplicateLayer(layer));
        }
        let frame_layer = ViewFrameLayer::from_output(layer.clone(), output);
        self.layers.insert(layer, frame_layer);
        Ok(())
    }

    pub fn finish(self) -> ViewFrameCommit {
        let layers = self
            .render_order
            .into_iter()
            .filter_map(|layer| self.layers.get(&layer).cloned())
            .collect();
        ViewFrameCommit { layers }
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
    use arcweft_view::{
        FragmentKind, ImageId, LayoutLength, LayoutPoint, LayoutResults, LayoutSize, LayoutTree,
        NodeKey, StyleId, ViewFragmentBuilder, ViewSemanticFragment, ViewSemanticFragmentBuilder,
        ViewSemanticNode,
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
    ) -> ViewSemanticFragment {
        let mut builder = ViewSemanticFragmentBuilder::default();
        builder
            .push(
                ViewSemanticNode::new(
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
    ) -> ViewLayerOutput {
        ViewLayerOutput::new(
            DisplayList::default(),
            semantic_fragment(key, layer, target, action),
        )
    }

    fn image_layer_output(key: u64, image: ImageId, x: i32, y: i32) -> ViewLayerOutput {
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
        ViewLayerOutput::new(
            DisplayList::from_fragment(&fragment, &layouts).unwrap(),
            ViewSemanticFragment::default(),
        )
    }

    #[test]
    fn view_frame_commit_orders_layers_by_committed_layer_tree() {
        let root = layer_id("root");
        let view = layer_id("view");
        let modal = layer_id("modal");
        let mut layers = LayerTree::new(LayerNode::new(
            root.clone(),
            LayerKind::Root,
            order(RenderPhase::Background, 0),
        ));
        layers
            .insert(
                LayerNode::new(
                    view.clone(),
                    LayerKind::GameView,
                    order(RenderPhase::GameView, 0),
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

        let mut builder = ViewFrameCommitBuilder::new(&layers);
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
                view.clone(),
                layer_output(
                    2,
                    view.clone(),
                    target("view.confirm"),
                    public_id("action.confirm"),
                ),
            )
            .unwrap();

        let commit = builder.finish();
        let committed_layers = commit
            .as_slice()
            .iter()
            .map(ViewFrameLayer::layer)
            .collect::<Vec<_>>();
        assert_eq!(committed_layers, vec![&view, &modal]);
    }

    #[test]
    fn view_frame_commit_rejects_unknown_and_duplicate_layers_without_replacing_existing() {
        let root = layer_id("root");
        let view = layer_id("view");
        let missing = layer_id("missing");
        let mut layers = LayerTree::new(LayerNode::new(
            root.clone(),
            LayerKind::Root,
            order(RenderPhase::Background, 0),
        ));
        layers
            .insert(
                LayerNode::new(
                    view.clone(),
                    LayerKind::GameView,
                    order(RenderPhase::GameView, 0),
                )
                .with_parent(root),
            )
            .unwrap();

        let mut builder = ViewFrameCommitBuilder::new(&layers);
        assert_eq!(
            builder.push_layer(missing.clone(), ViewLayerOutput::default()),
            Err(ViewFrameCommitError::UnknownLayer(missing))
        );
        builder
            .push_layer(
                view.clone(),
                layer_output(
                    1,
                    view.clone(),
                    target("view.first"),
                    public_id("action.first"),
                ),
            )
            .unwrap();
        assert_eq!(
            builder.push_layer(
                view.clone(),
                layer_output(
                    2,
                    view.clone(),
                    target("view.second"),
                    public_id("action.second"),
                ),
            ),
            Err(ViewFrameCommitError::DuplicateLayer(view.clone()))
        );

        let commit = builder.finish();
        assert!(
            commit
                .merged_semantics()
                .find(&target("view.first"))
                .is_some()
        );
        assert!(
            commit
                .merged_semantics()
                .find(&target("view.second"))
                .is_none()
        );
    }

    #[test]
    fn view_frame_commit_semantics_feed_presentation_dispatch() {
        let root = layer_id("root");
        let view = layer_id("view");
        let button = target("view.confirm");
        let action = public_id("action.confirm");
        let mut layers = LayerTree::new(LayerNode::new(
            root.clone(),
            LayerKind::Root,
            order(RenderPhase::Background, 0),
        ));
        layers
            .insert(
                LayerNode::new(
                    view.clone(),
                    LayerKind::GameView,
                    order(RenderPhase::GameView, 0),
                )
                .with_parent(root),
            )
            .unwrap();

        let mut builder = ViewFrameCommitBuilder::new(&layers);
        builder
            .push_layer(
                view,
                layer_output(1, layer_id("view"), button.clone(), action.clone()),
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
            PresentationActionDestination::ViewEntity
        );
        assert_eq!(dispatched.action().target(), &ActionTarget::Entity(button));
    }

    #[test]
    fn view_frame_commit_lists_image_items_in_render_order() {
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
                    LayerKind::GameView,
                    order(RenderPhase::GameView, 0),
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

        let mut builder = ViewFrameCommitBuilder::new(&layers);
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
