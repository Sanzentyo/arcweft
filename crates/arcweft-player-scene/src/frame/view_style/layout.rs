//! Current node geometry, subtree bounds, and container-gap placement.

use super::consumer::{box_style, scaled_dimension};
use super::{RuntimeNodeId, StyleTargetKey, StyleTargetKind};
use arcweft_bundle::resource_codec::ViewRuntimeNodeStyle;
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use arcweft_view::ViewElementKind;
use arcweft_view::style::{ViewPropertyKind, ViewSpecifiedValue};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(super) struct ResolvedLayoutNode {
    pub(super) id: RuntimeNodeId,
    pub(super) parent: Option<RuntimeNodeId>,
    pub(super) element: Option<ViewElementKind>,
    pub(super) keys: Vec<StyleTargetKey>,
    pub(super) style: ViewRuntimeNodeStyle,
}

#[derive(Clone, Copy, Debug)]
enum LayoutAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug)]
struct MilliRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl MilliRect {
    fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            left: x,
            top: y,
            right: x.saturating_add(i32::try_from(width).unwrap_or(i32::MAX)),
            bottom: y.saturating_add(i32::try_from(height).unwrap_or(i32::MAX)),
        }
    }

    fn union(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    const fn shifted(self, x: i32, y: i32) -> Self {
        Self {
            left: self.left.saturating_add(x),
            top: self.top.saturating_add(y),
            right: self.right.saturating_add(x),
            bottom: self.bottom.saturating_add(y),
        }
    }

    const fn start(self, axis: LayoutAxis) -> i32 {
        match axis {
            LayoutAxis::Horizontal => self.left,
            LayoutAxis::Vertical => self.top,
        }
    }

    const fn end(self, axis: LayoutAxis) -> i32 {
        match axis {
            LayoutAxis::Horizontal => self.right,
            LayoutAxis::Vertical => self.bottom,
        }
    }
}

pub(super) fn resolve_layout_offsets(
    presentation: &BundlePresentationSnapshot,
    nodes: &[ResolvedLayoutNode],
) -> BTreeMap<StyleTargetKey, (i32, i32)> {
    let parents = nodes
        .iter()
        .map(|node| (node.id.clone(), node.parent.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut subtree_bounds = nodes
        .iter()
        .map(|node| {
            let bounds = node
                .keys
                .iter()
                .filter_map(|key| target_bounds(presentation, key))
                .map(|bounds| styled_layout_bounds(bounds, &node.style))
                .reduce(MilliRect::union);
            (node.id.clone(), bounds)
        })
        .collect::<BTreeMap<_, _>>();
    for node in nodes.iter().rev() {
        let Some(parent) = &node.parent else {
            continue;
        };
        let child_bounds = subtree_bounds.get(&node.id).copied().flatten();
        if let Some(child_bounds) = child_bounds {
            subtree_bounds
                .entry(parent.clone())
                .and_modify(|parent_bounds| {
                    *parent_bounds = Some(
                        parent_bounds.map_or(child_bounds, |bounds| bounds.union(child_bounds)),
                    );
                });
        }
    }

    let mut node_offsets = nodes
        .iter()
        .map(|node| (node.id.clone(), (0_i32, 0_i32)))
        .collect::<BTreeMap<_, _>>();
    for container in nodes {
        let Some((axis, gap)) = container_gap(container) else {
            continue;
        };
        let mut previous_end = None;
        for child in nodes
            .iter()
            .filter(|node| node.parent.as_ref() == Some(&container.id))
        {
            let Some(bounds) = subtree_bounds.get(&child.id).copied().flatten() else {
                continue;
            };
            let inherited = node_offsets.get(&child.id).copied().unwrap_or_default();
            let current = bounds.shifted(inherited.0, inherited.1);
            let delta = previous_end.map_or(0, |end: i32| {
                end.saturating_add(gap).saturating_sub(current.start(axis))
            });
            if delta != 0 {
                for descendant in nodes
                    .iter()
                    .filter(|candidate| is_descendant_or_self(&candidate.id, &child.id, &parents))
                {
                    let offset = node_offsets.entry(descendant.id.clone()).or_default();
                    match axis {
                        LayoutAxis::Horizontal => offset.0 = offset.0.saturating_add(delta),
                        LayoutAxis::Vertical => offset.1 = offset.1.saturating_add(delta),
                    }
                }
            }
            previous_end = Some(current.end(axis).saturating_add(delta));
        }
    }

    nodes
        .iter()
        .flat_map(|node| {
            let offset = node_offsets.get(&node.id).copied().unwrap_or_default();
            node.keys.iter().cloned().map(move |key| (key, offset))
        })
        .collect()
}

fn styled_layout_bounds(bounds: MilliRect, style: &ViewRuntimeNodeStyle) -> MilliRect {
    let box_style = box_style(style);
    let width = u32::try_from(bounds.right.saturating_sub(bounds.left)).unwrap_or_default();
    let height = u32::try_from(bounds.bottom.saturating_sub(bounds.top)).unwrap_or_default();
    MilliRect::new(
        bounds.left.saturating_add(box_style.translate_x),
        bounds.top.saturating_add(box_style.translate_y),
        scaled_dimension(box_style.width.unwrap_or(width), box_style.scale_milli),
        scaled_dimension(box_style.height.unwrap_or(height), box_style.scale_milli),
    )
}

fn container_gap(node: &ResolvedLayoutNode) -> Option<(LayoutAxis, i32)> {
    let (axis, axis_property) = match node.element? {
        ViewElementKind::Column => (LayoutAxis::Vertical, ViewPropertyKind::RowGap),
        ViewElementKind::Row => (LayoutAxis::Horizontal, ViewPropertyKind::ColumnGap),
        ViewElementKind::Panel
        | ViewElementKind::Box
        | ViewElementKind::Scroll
        | ViewElementKind::Stack
        | ViewElementKind::Button
        | ViewElementKind::TextField
        | ViewElementKind::TextArea
        | ViewElementKind::SecureField => return None,
    };
    let value = node
        .style
        .layout()
        .value(axis_property)
        .or_else(|| node.style.layout().value(ViewPropertyKind::Gap));
    match value {
        Some(ViewSpecifiedValue::Length { value }) => Some((axis, value.value())),
        _ => None,
    }
}

fn is_descendant_or_self(
    candidate: &RuntimeNodeId,
    ancestor: &RuntimeNodeId,
    parents: &BTreeMap<RuntimeNodeId, Option<RuntimeNodeId>>,
) -> bool {
    let mut current = Some(candidate);
    while let Some(node) = current {
        if node == ancestor {
            return true;
        }
        current = parents.get(node).and_then(Option::as_ref);
    }
    false
}

fn target_bounds(
    presentation: &BundlePresentationSnapshot,
    key: &StyleTargetKey,
) -> Option<MilliRect> {
    match key.kind {
        StyleTargetKind::Control => control_target_bounds(presentation, &key.id),
        StyleTargetKind::Text => text_target_bounds(presentation, &key.id),
        StyleTargetKind::Image => image_target_bounds(presentation, &key.id),
        StyleTargetKind::Part => part_target_bounds(presentation, &key.id),
    }
}

fn control_target_bounds(presentation: &BundlePresentationSnapshot, id: &str) -> Option<MilliRect> {
    let mut bounds = presentation
        .text_inputs
        .iter()
        .filter(|control| control.target == id)
        .map(|control| {
            MilliRect::new(
                control.bounds.x_milli,
                control.bounds.y_milli,
                control.bounds.width_milli,
                control.bounds.height_milli,
            )
        })
        .collect::<Vec<_>>();
    bounds.extend(
        presentation
            .action_buttons
            .iter()
            .filter(|button| button.target == id)
            .map(|button| {
                MilliRect::new(
                    button.bounds.x_milli,
                    button.bounds.y_milli,
                    button.bounds.width_milli,
                    button.bounds.height_milli,
                )
            }),
    );
    bounds.extend(
        presentation
            .scroll_regions
            .iter()
            .filter(|region| region.target == id)
            .map(|region| {
                MilliRect::new(
                    region.bounds.x_milli,
                    region.bounds.y_milli,
                    region.bounds.width_milli,
                    region.bounds.height_milli,
                )
            }),
    );
    bounds.extend(
        presentation
            .surfaces
            .iter()
            .filter(|surface| surface.target == id)
            .map(|surface| {
                MilliRect::new(
                    surface.bounds.x_milli,
                    surface.bounds.y_milli,
                    surface.bounds.width_milli,
                    surface.bounds.height_milli,
                )
            }),
    );
    bounds.into_iter().reduce(MilliRect::union)
}

fn text_target_bounds(presentation: &BundlePresentationSnapshot, id: &str) -> Option<MilliRect> {
    presentation
        .view
        .mounts
        .iter()
        .flat_map(|mount| {
            mount.text.iter().flat_map(|text| {
                text.targets
                    .iter()
                    .filter(|target| mount.scoped_id(&target.public_id) == id)
                    .map(|target| {
                        MilliRect::new(
                            target.bounds.x_milli,
                            target.bounds.y_milli,
                            target.bounds.width_milli,
                            target.bounds.height_milli,
                        )
                    })
            })
        })
        .reduce(MilliRect::union)
}

fn image_target_bounds(presentation: &BundlePresentationSnapshot, id: &str) -> Option<MilliRect> {
    presentation
        .images
        .iter()
        .filter(|image| image.target.as_deref() == Some(id) || image.id == id)
        .map(|image| {
            MilliRect::new(
                image.bounds.x_milli,
                image.bounds.y_milli,
                image.bounds.width_milli,
                image.bounds.height_milli,
            )
        })
        .reduce(MilliRect::union)
}

fn part_target_bounds(presentation: &BundlePresentationSnapshot, id: &str) -> Option<MilliRect> {
    presentation
        .surfaces
        .iter()
        .filter(|surface| surface.public_id == id || surface.target == id)
        .map(|surface| {
            MilliRect::new(
                surface.bounds.x_milli,
                surface.bounds.y_milli,
                surface.bounds.width_milli,
                surface.bounds.height_milli,
            )
        })
        .reduce(MilliRect::union)
}
