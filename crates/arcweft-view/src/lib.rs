//! Sans I/O View entity and fragment data for Arcweft presentation.

pub mod display;
pub mod entity;
pub mod fragment;
pub mod frame;
pub mod fx;
pub mod handler;
pub mod image;
pub mod layout;
pub mod motion;
pub mod presentation_image;
pub mod program;
pub mod reactive;
pub mod semantics;
pub mod style;
pub mod style_authoring;
pub mod text_field;
pub mod text_source;
pub mod view;
pub mod virtualization;

use thiserror::Error;

pub use display::{
    DisplayItem, DisplayItemId, DisplayItemKind, DisplayList, ResolvedDisplayItem,
    ResolvedDisplayList,
};
pub use entity::{DirtyFlags, Entity, EntityStore, RawEntity};
pub use fragment::{
    ContainerKind, CustomElementId, EventBinding, EventKind, FragmentKind, FragmentNode, HandlerId,
    ImageId, NodeId, RichTextSourceId, SemanticSpecId, Span32, StyleId, TextSourceId, ViewFragment,
    ViewFragmentBuilder,
};
pub use frame::ViewLayerOutput;
pub use fx::{
    RetainedViewFxApplication, RetainedViewFxTable, ViewFxArgumentBinding, ViewFxError,
    ViewFxIdentity, ViewFxOrdinal,
};
pub use handler::{ViewHandlerInvocation, ViewHandlerRoute, ViewHandlerRouteTable};
pub use image::{
    ImageAlignment, ImageFit, ImagePlayback, ViewImagePresentationMetadata, ViewImageSource,
    ViewImageSourceTable, ViewResolvedImageFrame,
};
pub use layout::{
    LayoutBox, LayoutKind, LayoutLength, LayoutNode, LayoutPoint, LayoutResults, LayoutSize,
    LayoutTree,
};
pub use motion::{
    ViewCubicBezier, ViewEasingFunction, ViewKeyframe, ViewKeyframeTrack, ViewMotionError,
    ViewMotionSample, ViewReducedMotionPolicy, ViewStepPosition, ViewTimelineMillis,
    ViewTransition, ViewTransitionSpec,
};
pub use presentation_image::{ViewImagePresentationFrame, ViewImagePresentationInput};
pub use program::{
    ViewBranch, ViewCall, ViewCustomSpec, ViewElementKind, ViewElementLayoutKind, ViewElementSpec,
    ViewElementTextInputKind, ViewEventBindingSpec, ViewExpressionId, ViewHandlerProgram,
    ViewImageSpec, ViewInstruction, ViewInstructionRange, ViewPartExport, ViewPartId, ViewProgram,
    ViewProgramBuilder, ViewRepeat, ViewSemanticSpec, ViewStableKey, ViewStyleApply,
    ViewStylePatchId, ViewTextSpec,
};
pub use reactive::{EntityInvalidation, ReactiveGraph, ReactiveInvalidation, Revision};
pub use semantics::{
    ViewNodeId, ViewSemanticFragment, ViewSemanticFragmentBuilder, ViewSemanticNode,
};
pub use style::{
    Invalidation, Milli, PropertyBinding, PropertyBindingTable, PropertyBindingTableBuilder,
    ResolvedViewProperty, ResolvedViewStyle, Rgba8, ValueSourceId, ViewInteractionSelector,
    ViewPropertyId, ViewPropertyKind, ViewPropertyValue, ViewStyle, ViewStyleRule, ViewStyleTable,
};
pub use text_field::{
    ExternalTextUpdatePolicy, TextEditError, TextEditOutcome, TextEditState, TextEditorMode,
    TextEditorPart, TextFieldBindingCommitPolicy, TextFieldEditPolicy, TextFieldGeometryPolicy,
    TextFieldId, TextFieldMetrics, TextFieldPartId, TextFieldPartRect, TextFieldPolicyEditError,
    TextFieldSpec, TextFieldVisualBuffer,
};
pub use text_source::{ViewRichTextHandle, ViewTextByteRange, ViewTextSource, ViewTextSourceTable};
pub use view::{
    RustViewId, ViewDescriptor, ViewId, ViewImplementation, ViewProgramId, ViewRegistry,
    ViewSchemaId,
};
/// Stable key for one retained View fragment node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeKey(pub u64);

/// Error while building or updating View state.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ViewError {
    #[error("duplicate View node key {0:?}")]
    DuplicateNodeKey(NodeKey),
    #[error("duplicate view public id {0}")]
    DuplicateViewPublicId(arcweft_id::PublicId),
    #[error("stale View entity {0:?}")]
    StaleEntity(RawEntity),
    #[error("View entity has a different state type: {0:?}")]
    EntityTypeMismatch(RawEntity),
    #[error("invalid View fragment node {0:?}")]
    InvalidFragmentNode(NodeId),
    #[error("missing layout for View fragment node {0:?}")]
    MissingLayout(NodeId),
    #[error("duplicate View property binding {0:?}")]
    DuplicatePropertyBinding(ViewPropertyId),
    #[error("duplicate View image source {0:?}")]
    DuplicateImageSource(ImageId),
    #[error("unknown View image source {0:?}")]
    UnknownImageSource(ImageId),
    #[error("View node {0:?} binds an event without semantic target metadata")]
    HandlerNodeMissingSemantics(NodeId),
    #[error("View node {node:?} references unknown handler semantic {semantic:?}")]
    UnknownHandlerSemantic {
        node: NodeId,
        semantic: SemanticSpecId,
    },
    #[error("View node {node:?} references unknown display semantic {semantic:?}")]
    UnknownDisplaySemantic {
        node: NodeId,
        semantic: SemanticSpecId,
    },
    #[error("duplicate View style {0:?}")]
    DuplicateStyle(StyleId),
    #[error("unknown View style {0:?}")]
    UnknownStyle(StyleId),
    #[error("duplicate base View style property {0:?}")]
    DuplicateStyleProperty(ViewPropertyKind),
    #[error("duplicate View style rule {selector:?} for {kind:?}")]
    DuplicateStyleRule {
        selector: ViewInteractionSelector,
        kind: ViewPropertyKind,
    },
    #[error("View property {kind:?} rejects value {value:?}")]
    InvalidViewPropertyValue {
        kind: ViewPropertyKind,
        value: ViewPropertyValue,
    },
    #[error("too many View items")]
    CapacityExceeded,
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_id::PublicId;
    use arcweft_image::{
        DecodedImage, DecodedImageFrame, ImageDimensions, ImageFormat, ImageRepetition,
    };
    use arcweft_presentation::hit::HitRect;
    use arcweft_presentation::input::{ActionTarget, InputEpoch, InteractionTarget};
    use arcweft_presentation::interaction::InteractionState;
    use arcweft_presentation::layer::{
        LayerId, LayerInputPolicy, LayerKind, LayerNode, LayerOrder, LayerTree, RenderPhase,
    };
    use arcweft_presentation::semantic::SemanticRole;

    #[derive(Debug, Eq, PartialEq)]
    struct DialogueSkinState {
        hovered_nameplate: bool,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct InventoryState {
        selected_slot: u8,
    }

    fn public_id(value: &str) -> PublicId {
        PublicId::try_new(value).unwrap()
    }

    fn one_pixel_frame(index: u32, duration_millis: u64, rgba: [u8; 4]) -> DecodedImageFrame {
        DecodedImageFrame::new(
            index,
            ImageDimensions::new(1, 1).unwrap(),
            duration_millis,
            rgba.to_vec(),
        )
        .unwrap()
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

    #[test]
    fn view_fragment_keeps_text_media_view_and_custom_nodes_flat() {
        let mut entities = EntityStore::default();
        let view_state = entities
            .insert(
                DialogueSkinState {
                    hovered_nameplate: false,
                },
                Some(ViewId(4)),
            )
            .unwrap();

        let mut builder = ViewFragmentBuilder::default();
        let rich_text = builder
            .push_node(
                NodeKey(10),
                FragmentKind::RichText(RichTextSourceId(1)),
                StyleId(1),
                &[],
                &[EventBinding::new(EventKind::Activate, HandlerId(9))],
                Some(SemanticSpecId(1)),
            )
            .unwrap();
        let image = builder
            .push_node(
                NodeKey(11),
                FragmentKind::Image(ImageId(2)),
                StyleId(2),
                &[],
                &[],
                None,
            )
            .unwrap();
        let nested_view = builder
            .push_node(
                NodeKey(12),
                FragmentKind::View(view_state.raw()),
                StyleId(3),
                &[],
                &[],
                None,
            )
            .unwrap();
        let custom = builder
            .push_node(
                NodeKey(13),
                FragmentKind::Custom(CustomElementId(7)),
                StyleId(4),
                &[],
                &[],
                None,
            )
            .unwrap();
        let root = builder
            .push_node(
                NodeKey(14),
                FragmentKind::Container(ContainerKind::Stack),
                StyleId(5),
                &[rich_text, image, nested_view, custom],
                &[],
                None,
            )
            .unwrap();

        let fragment = builder.finish();
        assert_eq!(fragment.nodes().len(), 5);
        assert_eq!(
            fragment.node_children(root),
            Some([rich_text, image, nested_view, custom].as_slice())
        );
        assert_eq!(
            fragment.node_events(rich_text),
            Some([EventBinding::new(EventKind::Activate, HandlerId(9))].as_slice())
        );
        assert_eq!(fragment.nodes()[rich_text.0 as usize].style(), StyleId(1));
    }

    #[test]
    fn view_fragment_rejects_duplicate_keys_and_missing_children() {
        let mut builder = ViewFragmentBuilder::default();
        builder
            .push_node(
                NodeKey(1),
                FragmentKind::Text(TextSourceId(1)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();

        assert_eq!(
            builder.push_node(
                NodeKey(1),
                FragmentKind::Text(TextSourceId(2)),
                StyleId(1),
                &[],
                &[],
                None
            ),
            Err(ViewError::DuplicateNodeKey(NodeKey(1)))
        );
        assert_eq!(
            builder.push_node(
                NodeKey(2),
                FragmentKind::Container(ContainerKind::Block),
                StyleId(1),
                &[NodeId(99)],
                &[],
                None
            ),
            Err(ViewError::InvalidFragmentNode(NodeId(99)))
        );
    }

    #[test]
    fn layout_tree_preserves_fragment_node_order_and_child_counts() {
        let mut builder = ViewFragmentBuilder::default();
        let text = builder
            .push_node(
                NodeKey(1),
                FragmentKind::Text(TextSourceId(1)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let image = builder
            .push_node(
                NodeKey(2),
                FragmentKind::Image(ImageId(1)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let root = builder
            .push_node(
                NodeKey(3),
                FragmentKind::Container(ContainerKind::Block),
                StyleId(1),
                &[text, image],
                &[],
                None,
            )
            .unwrap();
        let fragment = builder.finish();

        let tree = LayoutTree::from_fragment(&fragment).unwrap();
        assert_eq!(tree.len(), 3);
        assert_eq!(tree.nodes()[text.0 as usize].node(), text);
        assert_eq!(tree.nodes()[text.0 as usize].kind(), LayoutKind::Text);
        assert_eq!(tree.nodes()[text.0 as usize].child_count(), 0);
        assert_eq!(tree.nodes()[image.0 as usize].kind(), LayoutKind::Image);
        assert_eq!(tree.nodes()[root.0 as usize].kind(), LayoutKind::Container);
        assert_eq!(tree.nodes()[root.0 as usize].child_count(), 2);
    }

    #[test]
    fn layout_results_report_missing_and_invalid_nodes() {
        let mut builder = ViewFragmentBuilder::default();
        let text = builder
            .push_node(
                NodeKey(1),
                FragmentKind::Text(TextSourceId(1)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let tree = LayoutTree::from_fragment(&builder.finish()).unwrap();
        let mut results = LayoutResults::new(&tree);

        assert_eq!(results.require(text), Err(ViewError::MissingLayout(text)));
        let layout = LayoutBox::new(
            LayoutPoint::new(LayoutLength::px(4), LayoutLength::px(8)),
            LayoutSize::new(LayoutLength::px(120), LayoutLength::px(24)),
        );
        results.set(text, layout).unwrap();
        assert_eq!(results.require(text), Ok(layout));
        assert_eq!(
            results.set(NodeId(99), layout),
            Err(ViewError::InvalidFragmentNode(NodeId(99)))
        );
    }

    #[test]
    fn display_list_emits_laid_out_paint_nodes_in_fragment_order() {
        let mut entities = EntityStore::default();
        let view = entities
            .insert(
                DialogueSkinState {
                    hovered_nameplate: false,
                },
                Some(ViewId(1)),
            )
            .unwrap();
        let mut builder = ViewFragmentBuilder::default();
        let text = builder
            .push_node(
                NodeKey(1),
                FragmentKind::Text(TextSourceId(1)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let rich_text = builder
            .push_node(
                NodeKey(2),
                FragmentKind::RichText(RichTextSourceId(2)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let image = builder
            .push_node(
                NodeKey(3),
                FragmentKind::Image(ImageId(3)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let mounted = builder
            .push_node(
                NodeKey(4),
                FragmentKind::View(view.raw()),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let custom = builder
            .push_node(
                NodeKey(5),
                FragmentKind::Custom(CustomElementId(4)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let root = builder
            .push_node(
                NodeKey(6),
                FragmentKind::Container(ContainerKind::Stack),
                StyleId(1),
                &[text, rich_text, image, mounted, custom],
                &[],
                None,
            )
            .unwrap();
        let fragment = builder.finish();
        let tree = LayoutTree::from_fragment(&fragment).unwrap();
        let mut layouts = LayoutResults::new(&tree);
        for node in [text, rich_text, image, mounted, custom, root] {
            let x = i32::try_from(node.0).unwrap();
            layouts
                .set(
                    node,
                    LayoutBox::new(
                        LayoutPoint::new(LayoutLength::px(x), LayoutLength::px(0)),
                        LayoutSize::new(LayoutLength::px(10), LayoutLength::px(10)),
                    ),
                )
                .unwrap();
        }

        let display = DisplayList::from_fragment(&fragment, &layouts).unwrap();
        let items = display.as_slice();
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].node(), text);
        assert_eq!(items[0].kind(), DisplayItemKind::Text(TextSourceId(1)));
        assert_eq!(items[1].node(), rich_text);
        assert_eq!(
            items[1].kind(),
            DisplayItemKind::RichText(RichTextSourceId(2))
        );
        assert_eq!(items[2].node(), image);
        assert_eq!(items[2].kind(), DisplayItemKind::Image(ImageId(3)));
        assert_eq!(items[3].node(), custom);
        assert_eq!(items[3].kind(), DisplayItemKind::Custom(CustomElementId(4)));
    }

    #[test]
    fn image_source_table_resolves_static_and_animated_frames() {
        let dimensions = ImageDimensions::new(1, 1).unwrap();
        let static_image = DecodedImage::new(
            ImageFormat::Png,
            dimensions,
            ImageRepetition::Once,
            vec![one_pixel_frame(0, 0, [10, 20, 30, 255])],
        )
        .unwrap();
        let animated_image = DecodedImage::new(
            ImageFormat::Gif,
            dimensions,
            ImageRepetition::Infinite,
            vec![
                one_pixel_frame(0, 40, [255, 0, 0, 255]),
                one_pixel_frame(1, 60, [0, 255, 0, 255]),
            ],
        )
        .unwrap();
        let layout = LayoutBox::new(
            LayoutPoint::new(LayoutLength::px(0), LayoutLength::px(0)),
            LayoutSize::new(LayoutLength::px(100), LayoutLength::px(100)),
        );

        let mut table = ViewImageSourceTable::default();
        let static_id = table.insert(ViewImageSource::new(static_image)).unwrap();
        let animated_id = table
            .insert(
                ViewImageSource::new(animated_image)
                    .with_fit(ImageFit::Cover)
                    .with_alignment(ImageAlignment::top_left())
                    .with_playback(ImagePlayback::new(1_000)),
            )
            .unwrap();

        let static_frame = table.resolve_frame(static_id, layout, 9_999).unwrap();
        assert_eq!(static_frame.frame().rgba(), &[10, 20, 30, 255]);
        assert_eq!(static_frame.fit(), ImageFit::Contain);

        let first = table.resolve_frame(animated_id, layout, 1_039).unwrap();
        assert_eq!(first.frame().index(), 0);
        assert_eq!(first.fit(), ImageFit::Cover);
        assert_eq!(first.alignment(), ImageAlignment::top_left());

        let second = table.resolve_frame(animated_id, layout, 1_040).unwrap();
        assert_eq!(second.frame().index(), 1);
    }

    #[test]
    fn image_source_playback_can_pause_and_scale_time() {
        let dimensions = ImageDimensions::new(1, 1).unwrap();
        let animated_image = DecodedImage::new(
            ImageFormat::Gif,
            dimensions,
            ImageRepetition::Infinite,
            vec![
                one_pixel_frame(0, 100, [255, 0, 0, 255]),
                one_pixel_frame(1, 100, [0, 255, 0, 255]),
            ],
        )
        .unwrap();
        let layout = LayoutBox::new(
            LayoutPoint::new(LayoutLength::px(0), LayoutLength::px(0)),
            LayoutSize::new(LayoutLength::px(1), LayoutLength::px(1)),
        );
        let mut table = ViewImageSourceTable::default();
        let paused = table
            .insert(
                ViewImageSource::new(animated_image.clone())
                    .with_playback(ImagePlayback::new(0).paused_at(150)),
            )
            .unwrap();
        let slow = table
            .insert(
                ViewImageSource::new(animated_image)
                    .with_playback(ImagePlayback::new(0).with_rate_milli(500)),
            )
            .unwrap();

        assert_eq!(
            table
                .resolve_frame(paused, layout, 0)
                .unwrap()
                .frame()
                .index(),
            1
        );
        assert_eq!(
            table
                .resolve_frame(paused, layout, 10_000)
                .unwrap()
                .frame()
                .index(),
            1
        );
        assert_eq!(
            table
                .resolve_frame(slow, layout, 199)
                .unwrap()
                .frame()
                .index(),
            0
        );
        assert_eq!(
            table
                .resolve_frame(slow, layout, 200)
                .unwrap()
                .frame()
                .index(),
            1
        );
    }

    #[test]
    fn display_list_requires_layout_for_paint_nodes_only() {
        let mut builder = ViewFragmentBuilder::default();
        let text = builder
            .push_node(
                NodeKey(1),
                FragmentKind::Text(TextSourceId(1)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let root = builder
            .push_node(
                NodeKey(2),
                FragmentKind::Container(ContainerKind::Block),
                StyleId(1),
                &[text],
                &[],
                None,
            )
            .unwrap();
        let fragment = builder.finish();
        let tree = LayoutTree::from_fragment(&fragment).unwrap();
        let mut layouts = LayoutResults::new(&tree);
        layouts
            .set(
                root,
                LayoutBox::new(
                    LayoutPoint::new(LayoutLength::px(0), LayoutLength::px(0)),
                    LayoutSize::new(LayoutLength::px(100), LayoutLength::px(20)),
                ),
            )
            .unwrap();

        assert_eq!(
            DisplayList::from_fragment(&fragment, &layouts),
            Err(ViewError::MissingLayout(text))
        );
    }

    #[test]
    fn view_layer_output_pairs_display_list_and_semantics_for_frame_commit() {
        let view_layer = layer_id("view");
        let button = target("view.confirm");
        let action = public_id("action.confirm");
        let mut fragment_builder = ViewFragmentBuilder::default();
        let rich_text = fragment_builder
            .push_node(
                NodeKey(1),
                FragmentKind::RichText(RichTextSourceId(1)),
                StyleId(1),
                &[],
                &[],
                Some(SemanticSpecId(1)),
            )
            .unwrap();
        let root = fragment_builder
            .push_node(
                NodeKey(2),
                FragmentKind::Container(ContainerKind::Block),
                StyleId(1),
                &[rich_text],
                &[],
                None,
            )
            .unwrap();
        let fragment = fragment_builder.finish();
        let tree = LayoutTree::from_fragment(&fragment).unwrap();
        let mut layouts = LayoutResults::new(&tree);
        for node in [rich_text, root] {
            layouts
                .set(
                    node,
                    LayoutBox::new(
                        LayoutPoint::new(LayoutLength::px(0), LayoutLength::px(0)),
                        LayoutSize::new(LayoutLength::px(120), LayoutLength::px(24)),
                    ),
                )
                .unwrap();
        }

        let mut semantic_builder = ViewSemanticFragmentBuilder::default();
        semantic_builder
            .push(
                ViewSemanticNode::new(
                    NodeKey(1),
                    view_layer,
                    button.clone(),
                    SemanticRole::Button,
                    HitRect::new(0.0, 0.0, 120.0, 24.0),
                )
                .with_label("Confirm")
                .with_action(action),
            )
            .unwrap();

        let output =
            ViewLayerOutput::from_fragment(&fragment, &layouts, semantic_builder.finish()).unwrap();
        assert_eq!(output.display().as_slice().len(), 1);
        assert_eq!(
            output.display().as_slice()[0].kind(),
            DisplayItemKind::RichText(RichTextSourceId(1))
        );
        assert_eq!(output.semantics().as_slice().len(), 1);
        assert_eq!(output.semantics().as_slice()[0].target(), &button);
        assert_eq!(output.semantics().as_slice()[0].label(), Some("Confirm"));
    }

    #[test]
    fn view_registry_resolves_dense_view_ids() {
        let mut registry = ViewRegistry::default();
        let public_id = public_id("view.dialogue.standard");
        let descriptor = ViewDescriptor::new(
            Some(public_id.clone()),
            ViewSchemaId(7),
            0x1234,
            ViewImplementation::Arcweft(ViewProgramId(3)),
        );

        let id = registry.register(descriptor).unwrap();
        assert_eq!(id, ViewId(0));
        assert_eq!(registry.resolve_public_id(&public_id), Some(id));
        assert_eq!(registry.get(id).unwrap().schema(), ViewSchemaId(7));
    }

    #[test]
    fn property_binding_invalidation_keeps_paint_changes_out_of_layout_and_fragment() {
        let mut builder = PropertyBindingTableBuilder::default();
        builder
            .push(PropertyBinding::new(
                ViewPropertyId(1),
                ViewPropertyKind::Opacity,
                ValueSourceId(10),
            ))
            .unwrap();
        builder
            .push(PropertyBinding::new(
                ViewPropertyId(2),
                ViewPropertyKind::Rotate,
                ValueSourceId(10),
            ))
            .unwrap();
        builder
            .push(PropertyBinding::new(
                ViewPropertyId(3),
                ViewPropertyKind::Width,
                ValueSourceId(11),
            ))
            .unwrap();
        builder
            .push(PropertyBinding::new(
                ViewPropertyId(4),
                ViewPropertyKind::SemanticLabel,
                ValueSourceId(12),
            ))
            .unwrap();

        let table = builder.finish();
        let paint_flags = table.dirty_flags_for_source(ValueSourceId(10));
        assert!(paint_flags.contains(DirtyFlags::PAINT));
        assert!(!paint_flags.contains(DirtyFlags::LAYOUT));
        assert!(!paint_flags.contains(DirtyFlags::FRAGMENT));

        let layout_flags = table.dirty_flags_for_source(ValueSourceId(11));
        assert!(layout_flags.contains(DirtyFlags::LAYOUT));
        assert!(!layout_flags.contains(DirtyFlags::FRAGMENT));

        let semantics_flags = table.dirty_flags_for_source(ValueSourceId(12));
        assert!(semantics_flags.contains(DirtyFlags::SEMANTICS));
        assert!(!semantics_flags.contains(DirtyFlags::LAYOUT));
    }

    #[test]
    fn property_binding_rejects_duplicate_property_slots() {
        let mut builder = PropertyBindingTableBuilder::default();
        builder
            .push(PropertyBinding::new(
                ViewPropertyId(1),
                ViewPropertyKind::Color,
                ValueSourceId(1),
            ))
            .unwrap();

        assert_eq!(
            builder.push(PropertyBinding::new(
                ViewPropertyId(1),
                ViewPropertyKind::BackgroundColor,
                ValueSourceId(2)
            )),
            Err(ViewError::DuplicatePropertyBinding(ViewPropertyId(1)))
        );
    }

    #[test]
    fn reactive_graph_coalesces_property_bindings_by_source_and_entity() {
        let mut store = EntityStore::default();
        let entity = store
            .insert(
                DialogueSkinState {
                    hovered_nameplate: false,
                },
                Some(ViewId(3)),
            )
            .unwrap();
        let mut bindings = PropertyBindingTableBuilder::default();
        bindings
            .push(PropertyBinding::new(
                ViewPropertyId(1),
                ViewPropertyKind::Opacity,
                ValueSourceId(1),
            ))
            .unwrap();
        bindings
            .push(PropertyBinding::new(
                ViewPropertyId(2),
                ViewPropertyKind::Rotate,
                ValueSourceId(1),
            ))
            .unwrap();
        bindings
            .push(PropertyBinding::new(
                ViewPropertyId(3),
                ViewPropertyKind::Width,
                ValueSourceId(2),
            ))
            .unwrap();

        let mut graph = ReactiveGraph::default();
        graph.watch_property_table(entity.raw(), &bindings.finish());

        let paint = graph.invalidate(ValueSourceId(1)).unwrap();
        assert_eq!(paint.source(), ValueSourceId(1));
        assert_eq!(paint.revision(), Revision(1));
        assert_eq!(paint.entities().len(), 1);
        assert_eq!(paint.entities()[0].entity(), entity.raw());
        assert!(paint.entities()[0].dirty().contains(DirtyFlags::PAINT));
        assert!(!paint.entities()[0].dirty().contains(DirtyFlags::LAYOUT));
        assert!(!paint.entities()[0].dirty().contains(DirtyFlags::FRAGMENT));

        let layout = graph.invalidate(ValueSourceId(2)).unwrap();
        assert_eq!(layout.revision(), Revision(1));
        assert!(layout.entities()[0].dirty().contains(DirtyFlags::LAYOUT));
        assert!(!layout.entities()[0].dirty().contains(DirtyFlags::FRAGMENT));

        assert_eq!(
            graph.invalidate(ValueSourceId(1)).unwrap().revision(),
            Revision(2)
        );
    }

    #[test]
    fn reactive_graph_returns_empty_invalidation_for_unwatched_sources() {
        let mut graph = ReactiveGraph::default();
        let invalidation = graph.invalidate(ValueSourceId(99)).unwrap();

        assert_eq!(invalidation.source(), ValueSourceId(99));
        assert_eq!(invalidation.revision(), Revision(1));
        assert!(invalidation.entities().is_empty());
    }

    #[test]
    fn view_registry_rejects_duplicate_public_ids() {
        let mut registry = ViewRegistry::default();
        let public_id = public_id("view.dialogue.standard");
        let descriptor = || {
            ViewDescriptor::new(
                Some(public_id.clone()),
                ViewSchemaId(1),
                0,
                ViewImplementation::Rust(RustViewId(1)),
            )
        };
        registry.register(descriptor()).unwrap();

        assert_eq!(
            registry.register(descriptor()),
            Err(ViewError::DuplicateViewPublicId(public_id))
        );
    }

    #[test]
    fn entity_store_rejects_stale_reused_handles() {
        let mut store = EntityStore::default();
        let first = store
            .insert(
                DialogueSkinState {
                    hovered_nameplate: false,
                },
                Some(ViewId(1)),
            )
            .unwrap();
        assert_eq!(store.view(first), Some(ViewId(1)));
        assert!(store.dirty(first).unwrap().contains(DirtyFlags::FRAGMENT));

        let removed = store.remove(first).unwrap();
        assert_eq!(
            removed,
            DialogueSkinState {
                hovered_nameplate: false
            }
        );

        let second = store
            .insert(InventoryState { selected_slot: 2 }, Some(ViewId(2)))
            .unwrap();
        assert_eq!(second.raw().index(), first.raw().index());
        assert_ne!(second.raw().generation(), first.raw().generation());
        assert!(store.get(first).is_none());
        assert_eq!(
            store.mark_dirty(first, DirtyFlags::LAYOUT),
            Err(ViewError::StaleEntity(first.raw()))
        );
        assert_eq!(
            store.get(second),
            Some(&InventoryState { selected_slot: 2 })
        );
    }

    #[test]
    fn entity_store_detects_wrong_state_type_on_remove() {
        let mut store = EntityStore::default();
        let state = store
            .insert(InventoryState { selected_slot: 1 }, None)
            .unwrap();
        let wrong_type = Entity::<DialogueSkinState>::from_raw(state.raw());

        assert_eq!(
            store.remove(wrong_type),
            Err(ViewError::EntityTypeMismatch(state.raw()))
        );
        assert_eq!(store.get(state), Some(&InventoryState { selected_slot: 1 }));
    }

    #[test]
    fn view_semantic_fragment_lowers_to_presentation_semantic_tree() {
        let view_layer = layer_id("view");
        let button_target = target("view.confirm");
        let action = public_id("action.confirm");
        let mut builder = ViewSemanticFragmentBuilder::default();
        let id = builder
            .push(
                ViewSemanticNode::new(
                    NodeKey(10),
                    view_layer,
                    button_target.clone(),
                    SemanticRole::Button,
                    HitRect::new(0.0, 0.0, 80.0, 24.0),
                )
                .with_label("Confirm")
                .with_action(action.clone()),
            )
            .unwrap();
        assert_eq!(id, ViewNodeId(0));

        let tree = builder.finish().to_semantic_tree();
        let lowered = tree
            .lower_action(&button_target, &action)
            .expect("View action lowers through presentation semantics");
        assert_eq!(lowered.target(), &ActionTarget::Entity(button_target));
        assert_eq!(lowered.kind(), &action);
    }

    #[test]
    fn view_semantic_fragment_rejects_duplicate_node_keys() {
        let view_layer = layer_id("view");
        let mut builder = ViewSemanticFragmentBuilder::default();
        builder
            .push(ViewSemanticNode::new(
                NodeKey(1),
                view_layer.clone(),
                target("view.first"),
                SemanticRole::Button,
                HitRect::new(0.0, 0.0, 10.0, 10.0),
            ))
            .unwrap();

        assert_eq!(
            builder.push(ViewSemanticNode::new(
                NodeKey(1),
                view_layer,
                target("view.second"),
                SemanticRole::Button,
                HitRect::new(10.0, 0.0, 10.0, 10.0),
            )),
            Err(ViewError::DuplicateNodeKey(NodeKey(1)))
        );
    }

    #[test]
    fn view_semantic_tree_routes_agent_invoke_through_layer_policy() {
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
                .with_parent(root)
                .with_input_policy(LayerInputPolicy::HitTest),
            )
            .unwrap();

        let mut builder = ViewSemanticFragmentBuilder::default();
        builder
            .push(
                ViewSemanticNode::new(
                    NodeKey(2),
                    view,
                    button.clone(),
                    SemanticRole::Button,
                    HitRect::new(0.0, 0.0, 80.0, 24.0),
                )
                .with_action(action.clone()),
            )
            .unwrap();

        let tree = builder.finish().to_semantic_tree();
        let lowered = tree
            .route_and_lower_action(
                InputEpoch(1),
                &button,
                &action,
                &layers,
                &InteractionState::default(),
            )
            .unwrap();
        assert_eq!(lowered.target(), &ActionTarget::Entity(button));
    }
}
