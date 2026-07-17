//! Preorder retained placement with exact flow and containing-block dependencies.

use super::ViewGeometryFrameInput;
use super::cache::{PlayerViewGeometryState, ViewPlaceCacheEntry};
use super::error::ViewGeometryRuntimeError;
use super::measure::MeasuredInventory;
use super::tree::ViewGeometryInventory;
use arcweft_view::geometry::{
    ViewContainingBlockDependency, ViewGeometryError, ViewGeometryOperation, ViewGeometryPoint,
    ViewGeometryRect, ViewGeometrySpan, ViewPlacedDependency, ViewPlacedGeometryKey,
    ViewPlacedGeometryRevision, ViewScrollStateInput, first_flow_border_start,
    first_reverse_flow_border_start, next_flow_border_start, next_reverse_flow_border_start,
    place_box,
};
use arcweft_view::style::{ViewPhysicalAxis, ViewPhysicalFlow, ViewPosition, ViewStyleNodeKey};
use std::collections::BTreeMap;

pub(super) struct PlacedInventory {
    pub entries: BTreeMap<ViewStyleNodeKey, ViewPlaceCacheEntry>,
}

impl PlacedInventory {
    pub fn entry(&self, node: &ViewStyleNodeKey) -> &ViewPlaceCacheEntry {
        self.entries
            .get(node)
            .expect("preorder placement covers every executable node")
    }
}

#[derive(Clone)]
struct FlowCursor {
    child: ViewStyleNodeKey,
    x: ViewGeometrySpan,
    y: ViewGeometrySpan,
}

pub(super) fn place_inventory(
    state: &PlayerViewGeometryState,
    inventory: &ViewGeometryInventory<'_>,
    input: &ViewGeometryFrameInput<'_>,
    measured: &MeasuredInventory,
) -> Result<PlacedInventory, ViewGeometryRuntimeError> {
    let mut entries = BTreeMap::new();
    let mut cursors = BTreeMap::<ViewStyleNodeKey, FlowCursor>::new();
    for key in &inventory.preorder {
        let node = inventory.node(key);
        let measured_entry = measured.entry(key);
        let (containing_block, containing_revision) =
            containing_block(inventory, input, &entries, key);
        let static_border_origin = static_origin(
            inventory,
            measured,
            &entries,
            &cursors,
            key,
            containing_block.rect,
        )?;
        let parent = node.parent.as_ref().map(|parent| {
            let entry = entries
                .get(parent)
                .expect("parent is placed before child in preorder");
            ViewPlacedDependency {
                node: parent.clone(),
                placement: entry.placement,
                revision: entry.key.revision(),
            }
        });
        let previous_flow_sibling = previous_dependency(inventory, &entries, &cursors, key);
        let scroll = parent_scroll(inventory, input, key)?;
        let cache_key = ViewPlacedGeometryKey {
            node: key.clone(),
            measured: measured_entry.measured,
            box_style: *node.box_style(),
            containing_block: ViewContainingBlockDependency {
                node: containing_block.node,
                rect: containing_block.rect,
                revision: containing_revision,
            },
            static_border_origin,
            parent,
            previous_flow_sibling,
            viewport: input.viewport,
            scroll,
        };
        let entry = match state.place_entry(key) {
            Some(entry) if entry.key == cache_key => entry.clone(),
            _ => ViewPlaceCacheEntry {
                placement: place_box(
                    key,
                    node.box_style(),
                    measured_entry.measured,
                    containing_block.rect,
                    static_border_origin,
                )?,
                key: cache_key,
            },
        };
        entries.insert(key.clone(), entry);
        if is_flow_participant(node.box_style().position)
            && let Some(parent) = &node.parent
            && inventory.node(parent).container_style().is_some()
        {
            let x = ViewGeometrySpan::from_start_extent(
                key,
                ViewPhysicalAxis::X,
                static_border_origin.x_milli,
                measured_entry.measured.x.used_border_extent_milli,
            )?;
            let y = ViewGeometrySpan::from_start_extent(
                key,
                ViewPhysicalAxis::Y,
                static_border_origin.y_milli,
                measured_entry.measured.y.used_border_extent_milli,
            )?;
            cursors.insert(
                parent.clone(),
                FlowCursor {
                    child: key.clone(),
                    x,
                    y,
                },
            );
        }
    }
    Ok(PlacedInventory { entries })
}

struct ContainingBlock {
    node: Option<ViewStyleNodeKey>,
    rect: ViewGeometryRect,
}

fn containing_block(
    inventory: &ViewGeometryInventory<'_>,
    input: &ViewGeometryFrameInput<'_>,
    entries: &BTreeMap<ViewStyleNodeKey, ViewPlaceCacheEntry>,
    key: &ViewStyleNodeKey,
) -> (ContainingBlock, ViewPlacedGeometryRevision) {
    let node = inventory.node(key);
    if node.box_style().position == ViewPosition::Fixed {
        return (
            ContainingBlock {
                node: None,
                rect: input.viewport.rect,
            },
            ViewPlacedGeometryRevision::for_root_viewport(input.viewport.revision),
        );
    }
    if node.box_style().position == ViewPosition::Absolute {
        let mut ancestor = node.parent.as_ref();
        while let Some(candidate) = ancestor {
            let parent = inventory.node(candidate);
            if parent.box_style().position != ViewPosition::Static {
                let entry = entries
                    .get(candidate)
                    .expect("positioned ancestor is placed before descendant");
                return (
                    ContainingBlock {
                        node: Some(candidate.clone()),
                        rect: entry.placement.padding_box,
                    },
                    entry.key.revision(),
                );
            }
            ancestor = parent.parent.as_ref();
        }
        return (
            ContainingBlock {
                node: None,
                rect: input.viewport.rect,
            },
            ViewPlacedGeometryRevision::for_root_viewport(input.viewport.revision),
        );
    }
    node.parent.as_ref().map_or_else(
        || {
            (
                ContainingBlock {
                    node: None,
                    rect: input.viewport.rect,
                },
                ViewPlacedGeometryRevision::for_root_viewport(input.viewport.revision),
            )
        },
        |parent| {
            let entry = entries
                .get(parent)
                .expect("parent is placed before child in preorder");
            (
                ContainingBlock {
                    node: Some(parent.clone()),
                    rect: entry.placement.content_box,
                },
                entry.key.revision(),
            )
        },
    )
}

#[expect(
    clippy::too_many_lines,
    reason = "the axis-local flow placement table keeps reverse, gap, overlay, absolute, and containing-block rules in one deterministic match"
)]
fn static_origin(
    inventory: &ViewGeometryInventory<'_>,
    measured: &MeasuredInventory,
    placed: &BTreeMap<ViewStyleNodeKey, ViewPlaceCacheEntry>,
    cursors: &BTreeMap<ViewStyleNodeKey, FlowCursor>,
    key: &ViewStyleNodeKey,
    containing: ViewGeometryRect,
) -> Result<ViewGeometryPoint, ViewGeometryRuntimeError> {
    let node = inventory.node(key);
    let own = measured.entry(key).measured;
    let Some(parent_key) = &node.parent else {
        return Ok(ViewGeometryPoint::new(
            first_flow_border_start(
                key,
                ViewPhysicalAxis::X,
                containing.left_milli,
                own.margin.left,
            )?,
            first_flow_border_start(
                key,
                ViewPhysicalAxis::Y,
                containing.top_milli,
                own.margin.top,
            )?,
        ));
    };
    let parent = inventory.node(parent_key);
    let parent_placement = placed
        .get(parent_key)
        .expect("parent is placed before child")
        .placement;
    let content = parent_placement.content_box;
    if !is_flow_participant(node.box_style().position) {
        return Ok(ViewGeometryPoint::new(
            first_flow_border_start(
                key,
                ViewPhysicalAxis::X,
                content.left_milli,
                own.margin.left,
            )?,
            first_flow_border_start(key, ViewPhysicalAxis::Y, content.top_milli, own.margin.top)?,
        ));
    }
    let container = parent
        .container_style()
        .expect("executable parent with children is a container");
    let previous = cursors.get(parent_key);
    match container.flow {
        ViewPhysicalFlow::Overlay => Ok(ViewGeometryPoint::new(
            first_flow_border_start(
                key,
                ViewPhysicalAxis::X,
                content.left_milli,
                own.margin.left,
            )?,
            first_flow_border_start(key, ViewPhysicalAxis::Y, content.top_milli, own.margin.top)?,
        )),
        ViewPhysicalFlow::Row => Ok(ViewGeometryPoint::new(
            match previous {
                Some(previous) => {
                    let previous_measure = measured.entry(&previous.child).measured;
                    next_flow_border_start(
                        key,
                        ViewPhysicalAxis::X,
                        previous.x.end_milli,
                        previous_measure.margin.right,
                        container.column_gap.value(),
                        own.margin.left,
                    )?
                }
                None => first_flow_border_start(
                    key,
                    ViewPhysicalAxis::X,
                    content.left_milli,
                    own.margin.left,
                )?,
            },
            first_flow_border_start(key, ViewPhysicalAxis::Y, content.top_milli, own.margin.top)?,
        )),
        ViewPhysicalFlow::RowReverse => Ok(ViewGeometryPoint::new(
            match previous {
                Some(previous) => {
                    let previous_measure = measured.entry(&previous.child).measured;
                    next_reverse_flow_border_start(
                        key,
                        ViewPhysicalAxis::X,
                        previous.x.start_milli,
                        previous_measure.margin.left,
                        container.column_gap.value(),
                        own.margin.right,
                        own.x.used_border_extent_milli,
                    )?
                }
                None => first_reverse_flow_border_start(
                    key,
                    ViewPhysicalAxis::X,
                    content.right_milli,
                    own.margin.right,
                    own.x.used_border_extent_milli,
                )?,
            },
            first_flow_border_start(key, ViewPhysicalAxis::Y, content.top_milli, own.margin.top)?,
        )),
        ViewPhysicalFlow::Column => Ok(ViewGeometryPoint::new(
            first_flow_border_start(
                key,
                ViewPhysicalAxis::X,
                content.left_milli,
                own.margin.left,
            )?,
            match previous {
                Some(previous) => {
                    let previous_measure = measured.entry(&previous.child).measured;
                    next_flow_border_start(
                        key,
                        ViewPhysicalAxis::Y,
                        previous.y.end_milli,
                        previous_measure.margin.bottom,
                        container.row_gap.value(),
                        own.margin.top,
                    )?
                }
                None => first_flow_border_start(
                    key,
                    ViewPhysicalAxis::Y,
                    content.top_milli,
                    own.margin.top,
                )?,
            },
        )),
        ViewPhysicalFlow::ColumnReverse => Ok(ViewGeometryPoint::new(
            first_flow_border_start(
                key,
                ViewPhysicalAxis::X,
                content.left_milli,
                own.margin.left,
            )?,
            match previous {
                Some(previous) => {
                    let previous_measure = measured.entry(&previous.child).measured;
                    next_reverse_flow_border_start(
                        key,
                        ViewPhysicalAxis::Y,
                        previous.y.start_milli,
                        previous_measure.margin.top,
                        container.row_gap.value(),
                        own.margin.bottom,
                        own.y.used_border_extent_milli,
                    )?
                }
                None => first_reverse_flow_border_start(
                    key,
                    ViewPhysicalAxis::Y,
                    content.bottom_milli,
                    own.margin.bottom,
                    own.y.used_border_extent_milli,
                )?,
            },
        )),
    }
}

fn previous_dependency(
    inventory: &ViewGeometryInventory<'_>,
    entries: &BTreeMap<ViewStyleNodeKey, ViewPlaceCacheEntry>,
    cursors: &BTreeMap<ViewStyleNodeKey, FlowCursor>,
    key: &ViewStyleNodeKey,
) -> Option<ViewPlacedDependency> {
    let node = inventory.node(key);
    if !is_flow_participant(node.box_style().position) {
        return None;
    }
    let parent = node.parent.as_ref()?;
    if inventory.node(parent).container_style()?.flow == ViewPhysicalFlow::Overlay {
        return None;
    }
    let previous = cursors.get(parent)?;
    let entry = entries
        .get(&previous.child)
        .expect("flow cursor references a placed sibling");
    Some(ViewPlacedDependency {
        node: previous.child.clone(),
        placement: entry.placement,
        revision: entry.key.revision(),
    })
}

fn parent_scroll(
    inventory: &ViewGeometryInventory<'_>,
    input: &ViewGeometryFrameInput<'_>,
    key: &ViewStyleNodeKey,
) -> Result<ViewScrollStateInput, ViewGeometryRuntimeError> {
    let Some(parent) = inventory.node(key).parent.as_ref() else {
        return Ok(ViewScrollStateInput {
            x_milli: 0,
            y_milli: 0,
            revision: arcweft_view::geometry::ViewScrollStateRevision::new(0),
        });
    };
    let scroll = input.scroll.get(parent);
    let x_milli =
        scroll
            .x_milli
            .checked_neg()
            .ok_or_else(|| ViewGeometryError::ArithmeticOverflow {
                node: key.clone(),
                axis: Some(ViewPhysicalAxis::X),
                operation: ViewGeometryOperation::Subtract,
            })?;
    let y_milli =
        scroll
            .y_milli
            .checked_neg()
            .ok_or_else(|| ViewGeometryError::ArithmeticOverflow {
                node: key.clone(),
                axis: Some(ViewPhysicalAxis::Y),
                operation: ViewGeometryOperation::Subtract,
            })?;
    Ok(ViewScrollStateInput {
        x_milli,
        y_milli,
        revision: scroll.revision,
    })
}

const fn is_flow_participant(position: ViewPosition) -> bool {
    matches!(position, ViewPosition::Static | ViewPosition::Relative)
}
