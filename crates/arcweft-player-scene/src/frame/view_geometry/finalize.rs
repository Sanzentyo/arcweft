//! Postorder transform, clip, scroll, and consumer aggregation.

use super::ViewGeometryFrameInput;
use super::cache::{
    PlayerViewGeometryState, ViewCommittedGeometryFrame, ViewFinalCacheEntry,
    ViewGeometryPreparedFrame,
};
use super::error::ViewGeometryRuntimeError;
use super::measure::MeasuredInventory;
use super::place::PlacedInventory;
use super::tree::ViewGeometryInventory;
use arcweft_view::geometry::{
    ViewChildFinalDependency, ViewFinalGeometry, ViewFinalGeometryKey, ViewGeometryClip,
    ViewGeometryPoint, ViewGeometryRect, ViewGeometryTransform, ViewScrollGeometry,
    ViewTransformDependency, consumer_geometry, scroll_axis_geometry, transform_rect, union_rects,
};
use arcweft_view::style::{ViewOverflow, ViewPhysicalAxis, ViewPosition, ViewStyleNodeKey};
use std::collections::{BTreeMap, BTreeSet};

#[expect(
    clippy::too_many_lines,
    reason = "finalization performs one postorder pass that must keep exact child, clip, scroll, transform, and cache dependencies adjacent"
)]
pub(super) fn finalize_inventory(
    state: &PlayerViewGeometryState,
    inventory: ViewGeometryInventory<'_>,
    input: &ViewGeometryFrameInput<'_>,
    measured: MeasuredInventory,
    placed: PlacedInventory,
) -> Result<ViewGeometryPreparedFrame, ViewGeometryRuntimeError> {
    let mut entries = BTreeMap::<ViewStyleNodeKey, ViewFinalCacheEntry>::new();
    let mut scroll_subtrees = BTreeMap::<ViewStyleNodeKey, ViewGeometryRect>::new();
    for key in &inventory.postorder {
        let node = inventory.node(key);
        let placement = placed.entry(key).placement;
        let inherited_clip = inherited_clip(&inventory, input, &placed, key)?;
        let world_border_box = world_rect(&inventory, input, &placed, key, placement.border_box)?;
        let world_padding_box = world_rect(&inventory, input, &placed, key, placement.padding_box)?;
        let descendant_clip = inherited_clip.with_overflow(
            world_padding_box,
            node.box_style().overflow_x,
            node.box_style().overflow_y,
        );
        let paint_outsets = ViewGeometryInventory::paint_outsets(input, key);
        let local_paint_box = placement
            .border_box
            .outset_non_negative(key, paint_outsets.edges)?;
        let world_paint_box = world_rect(&inventory, input, &placed, key, local_paint_box)?;
        let consumers = consumer_geometry(world_border_box, world_paint_box, inherited_clip);

        let child_entries = node
            .children
            .iter()
            .map(|child| {
                entries
                    .get(child)
                    .expect("children finalize before parents in postorder")
            })
            .collect::<Vec<_>>();
        let ordered_children = child_entries
            .iter()
            .map(|entry| ViewChildFinalDependency {
                node: entry.geometry.node.clone(),
                world_border_box: entry.geometry.world_border_box,
                layout_subtree_bounds: entry.geometry.layout_subtree_bounds,
                paint_subtree_bounds: entry.geometry.paint_subtree_bounds,
                descendant_clip: entry.geometry.descendant_clip,
                revision: entry.geometry.final_revision,
            })
            .collect::<Vec<_>>();
        let layout_subtree_bounds = union_rects(
            core::iter::once(placement.margin_box).chain(
                child_entries
                    .iter()
                    .map(|entry| entry.geometry.layout_subtree_bounds),
            ),
        )
        .expect("one node margin box always seeds the layout subtree");
        let scroll_children = node
            .children
            .iter()
            .filter(|child| inventory.node(child).box_style().position != ViewPosition::Fixed)
            .map(|child| {
                let child_geometry = &entries
                    .get(child)
                    .expect("child final geometry exists")
                    .geometry;
                let subtree = *scroll_subtrees
                    .get(child)
                    .expect("child scroll subtree exists");
                visible_layout_contribution(
                    child_geometry.margin_box,
                    child_geometry.padding_box,
                    subtree,
                    inventory.node(child).box_style().overflow_x,
                    inventory.node(child).box_style().overflow_y,
                )
            })
            .collect::<Vec<_>>();
        let scroll_content = union_rects(
            core::iter::once(placement.padding_box).chain(scroll_children.iter().copied()),
        )
        .expect("padding box always seeds scroll content");
        let own_scroll = ViewGeometryInventory::scroll(input, key);
        let scroll = ViewScrollGeometry {
            x: scroll_axis_geometry(
                key,
                ViewPhysicalAxis::X,
                node.box_style().overflow_x,
                placement.padding_box.x(),
                scroll_content.x(),
                own_scroll.x_milli,
            )?,
            y: scroll_axis_geometry(
                key,
                ViewPhysicalAxis::Y,
                node.box_style().overflow_y,
                placement.padding_box.y(),
                scroll_content.y(),
                own_scroll.y_milli,
            )?,
        };
        let paint_subtree_bounds = union_rects(
            consumers.paint_bounds.into_iter().chain(
                child_entries
                    .iter()
                    .filter_map(|entry| entry.geometry.paint_subtree_bounds),
            ),
        );
        let transform_chain = transform_dependencies(&inventory, &placed, key);
        let final_key = ViewFinalGeometryKey {
            node: key.clone(),
            placement,
            box_style: *node.box_style(),
            transform_chain,
            inherited_clip,
            paint_outsets,
            scroll: own_scroll,
            ordered_children,
        };
        let final_revision = final_key.revision();
        let entry = match state.final_entry(key) {
            Some(entry) if entry.key == final_key => entry.clone(),
            _ => ViewFinalCacheEntry {
                geometry: ViewFinalGeometry {
                    node: key.clone(),
                    content_box: placement.content_box,
                    padding_box: placement.padding_box,
                    border_box: placement.border_box,
                    margin_box: placement.margin_box,
                    world_border_box,
                    descendant_clip,
                    consumers,
                    layout_subtree_bounds,
                    paint_subtree_bounds,
                    scroll,
                    measured_revision: measured.entry(key).measured.revision,
                    placed_revision: placed.entry(key).key.revision(),
                    final_revision,
                },
                key: final_key,
            },
        };
        let scroll_subtree =
            union_rects(core::iter::once(placement.margin_box).chain(scroll_children))
                .expect("one node margin box always seeds the scroll subtree");
        scroll_subtrees.insert(key.clone(), scroll_subtree);
        entries.insert(key.clone(), entry);
    }

    let base_generation = state.generation();
    let next_generation = base_generation.checked_next()?;
    let final_nodes = entries
        .iter()
        .map(|(node, entry)| (node.clone(), entry.geometry.clone()))
        .collect::<BTreeMap<_, _>>();
    let live_nodes = inventory.preorder.iter().cloned().collect::<BTreeSet<_>>();
    let committed = ViewCommittedGeometryFrame::new(
        next_generation,
        input.viewport.rect,
        final_nodes,
        inventory.transparent,
        inventory.suppressed,
        inventory.targets,
    );
    Ok(ViewGeometryPreparedFrame::new(
        base_generation,
        next_generation,
        live_nodes,
        measured.entries,
        placed.entries,
        entries,
        committed,
    ))
}

fn visible_layout_contribution(
    margin_box: ViewGeometryRect,
    padding_box: ViewGeometryRect,
    subtree: ViewGeometryRect,
    overflow_x: ViewOverflow,
    overflow_y: ViewOverflow,
) -> ViewGeometryRect {
    let clipped = ViewGeometryRect {
        left_milli: if overflow_x.clips_descendants() {
            subtree.left_milli.max(padding_box.left_milli)
        } else {
            subtree.left_milli
        },
        right_milli: if overflow_x.clips_descendants() {
            subtree.right_milli.min(padding_box.right_milli)
        } else {
            subtree.right_milli
        },
        top_milli: if overflow_y.clips_descendants() {
            subtree.top_milli.max(padding_box.top_milli)
        } else {
            subtree.top_milli
        },
        bottom_milli: if overflow_y.clips_descendants() {
            subtree.bottom_milli.min(padding_box.bottom_milli)
        } else {
            subtree.bottom_milli
        },
    };
    if clipped.left_milli <= clipped.right_milli && clipped.top_milli <= clipped.bottom_milli {
        margin_box.union(clipped)
    } else {
        margin_box
    }
}

fn world_rect(
    inventory: &ViewGeometryInventory<'_>,
    input: &ViewGeometryFrameInput<'_>,
    placed: &PlacedInventory,
    subject: &ViewStyleNodeKey,
    rect: ViewGeometryRect,
) -> Result<ViewGeometryRect, ViewGeometryRuntimeError> {
    let mut current_rect = rect;
    let mut current = Some(subject);
    while let Some(key) = current {
        let node = inventory.node(key);
        let placement = placed.entry(key).placement;
        current_rect = transform_rect(
            subject,
            current_rect,
            ViewGeometryTransform {
                border_box: placement.border_box,
                translate: ViewGeometryPoint::new(
                    node.box_style().translate_x.value(),
                    node.box_style().translate_y.value(),
                ),
                scale: node.box_style().scale,
            },
        )?;
        current = node.parent.as_ref();
        if let Some(parent) = current {
            let scroll = ViewGeometryInventory::scroll(input, parent);
            current_rect = current_rect.translated(
                subject,
                ViewGeometryPoint::new(
                    scroll.x_milli.checked_neg().ok_or_else(|| {
                        arcweft_view::geometry::ViewGeometryError::ArithmeticOverflow {
                            node: subject.clone(),
                            axis: Some(ViewPhysicalAxis::X),
                            operation: arcweft_view::geometry::ViewGeometryOperation::Subtract,
                        }
                    })?,
                    scroll.y_milli.checked_neg().ok_or_else(|| {
                        arcweft_view::geometry::ViewGeometryError::ArithmeticOverflow {
                            node: subject.clone(),
                            axis: Some(ViewPhysicalAxis::Y),
                            operation: arcweft_view::geometry::ViewGeometryOperation::Subtract,
                        }
                    })?,
                ),
            )?;
        }
    }
    Ok(current_rect)
}

fn inherited_clip(
    inventory: &ViewGeometryInventory<'_>,
    input: &ViewGeometryFrameInput<'_>,
    placed: &PlacedInventory,
    key: &ViewStyleNodeKey,
) -> Result<ViewGeometryClip, ViewGeometryRuntimeError> {
    let mut clip = ViewGeometryClip::from_rect(input.viewport.rect);
    let mut ancestor = inventory.node(key).parent.as_ref();
    while let Some(parent) = ancestor {
        let node = inventory.node(parent);
        let world_padding = world_rect(
            inventory,
            input,
            placed,
            parent,
            placed.entry(parent).placement.padding_box,
        )?;
        clip = clip.with_overflow(
            world_padding,
            node.box_style().overflow_x,
            node.box_style().overflow_y,
        );
        ancestor = node.parent.as_ref();
    }
    Ok(clip)
}

fn transform_dependencies(
    inventory: &ViewGeometryInventory<'_>,
    placed: &PlacedInventory,
    key: &ViewStyleNodeKey,
) -> Vec<ViewTransformDependency> {
    let mut dependencies = Vec::new();
    let mut current = Some(key);
    while let Some(node_key) = current {
        let node = inventory.node(node_key);
        let entry = placed.entry(node_key);
        dependencies.push(ViewTransformDependency {
            node: node_key.clone(),
            transform: ViewGeometryTransform {
                border_box: entry.placement.border_box,
                translate: ViewGeometryPoint::new(
                    node.box_style().translate_x.value(),
                    node.box_style().translate_y.value(),
                ),
                scale: node.box_style().scale,
            },
            placed_revision: entry.key.revision(),
        });
        current = node.parent.as_ref();
    }
    dependencies
}
