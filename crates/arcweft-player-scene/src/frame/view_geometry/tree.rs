//! Deterministic retained-node inventory and transparent-tree collapse.

use super::ViewGeometryFrameInput;
use super::error::{
    ViewGeometryProductError, ViewGeometryProductKind, ViewGeometryRuntimeError,
    ViewGeometryTargetKey, ViewGeometryTreeRelation,
};
use super::intrinsic::ViewIntrinsicProductRef;
use arcweft_bundle::resource_codec::view::ViewInputKind;
use arcweft_bundle::resource_codec::{
    ViewRuntimeGeometryOwner, ViewRuntimeGeometryParticipation, ViewRuntimeNodeStyle,
};
use arcweft_runtime_driver::presentation_handles::PresentationHandleId;
use arcweft_runtime_driver::view_runtime::{
    BundleViewInstancePathSegment, BundleViewMountOutput, BundleViewStyleNode,
    BundleViewStyleNodeKind,
};
use arcweft_view::geometry::{ViewAvailableGeometrySize, ViewPaintOutsets, ViewScrollStateInput};
use arcweft_view::style::{ViewPhysicalBoxStyle, ViewPhysicalContainerStyle, ViewStyleNodeKey};
use arcweft_view::{ViewElementKind, ViewId, ViewMountId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CallAttachmentKey {
    handle: PresentationHandleId,
    path: Vec<BundleViewInstancePathSegment>,
    instruction: u32,
}

struct RawNode<'a> {
    key: ViewStyleNodeKey,
    mount: &'a BundleViewMountOutput,
    source: &'a BundleViewStyleNode,
    style: &'a ViewRuntimeNodeStyle,
    structural_parent: Option<ViewStyleNodeKey>,
    structural_children: Vec<ViewStyleNodeKey>,
    product: ViewIntrinsicProductRef<'a>,
    target_keys: Vec<ViewGeometryTargetKey>,
}

pub(super) struct ViewGeometryInventoryNode<'a> {
    pub parent: Option<ViewStyleNodeKey>,
    pub children: Vec<ViewStyleNodeKey>,
    pub style: &'a ViewRuntimeNodeStyle,
    pub owner: ViewRuntimeGeometryOwner,
    pub product: ViewIntrinsicProductRef<'a>,
}

impl ViewGeometryInventoryNode<'_> {
    pub const fn box_style(&self) -> &ViewPhysicalBoxStyle {
        self.style
            .physical()
            .box_style()
            .expect("executable geometry nodes always carry one physical box")
    }

    pub const fn container_style(&self) -> Option<ViewPhysicalContainerStyle> {
        self.style.physical().container_style().copied()
    }
}

pub(super) struct ViewGeometryInventory<'a> {
    pub nodes: BTreeMap<ViewStyleNodeKey, ViewGeometryInventoryNode<'a>>,
    pub preorder: Vec<ViewStyleNodeKey>,
    pub postorder: Vec<ViewStyleNodeKey>,
    pub transparent: BTreeSet<ViewStyleNodeKey>,
    pub suppressed: BTreeSet<ViewStyleNodeKey>,
    pub targets: BTreeMap<ViewGeometryTargetKey, ViewStyleNodeKey>,
}

impl ViewGeometryInventory<'_> {
    pub fn node(&self, key: &ViewStyleNodeKey) -> &ViewGeometryInventoryNode<'_> {
        self.nodes
            .get(key)
            .expect("inventory traversal keys always resolve")
    }

    pub fn scroll(
        input: &ViewGeometryFrameInput<'_>,
        key: &ViewStyleNodeKey,
    ) -> ViewScrollStateInput {
        input.scroll.get(key)
    }

    pub fn paint_outsets(
        input: &ViewGeometryFrameInput<'_>,
        key: &ViewStyleNodeKey,
    ) -> ViewPaintOutsets {
        input.paint_outsets.get(key)
    }

    pub fn available(
        &self,
        input: &ViewGeometryFrameInput<'_>,
        key: &ViewStyleNodeKey,
    ) -> ViewAvailableGeometrySize {
        if self.node(key).parent.is_none() {
            let size = input.viewport.rect.size();
            ViewAvailableGeometrySize {
                width_milli: Some(size.width_milli),
                height_milli: Some(size.height_milli),
            }
        } else {
            ViewAvailableGeometrySize::default()
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one preorder inventory pass validates occurrence identity, parentage, owner products, targets, and leaf/container structure atomically"
)]
pub(super) fn build_inventory<'a>(
    input: &ViewGeometryFrameInput<'a>,
) -> Result<ViewGeometryInventory<'a>, ViewGeometryRuntimeError> {
    let mut raw = BTreeMap::<ViewStyleNodeKey, RawNode<'a>>::new();
    let mut order = Vec::new();
    let mut calls = BTreeMap::<CallAttachmentKey, Vec<(ViewId, ViewStyleNodeKey)>>::new();

    for mount in &input.frame.mounts {
        for source in &mount.style_nodes {
            let key = source.style_node_key(mount.mount);
            if raw.contains_key(&key) {
                return Err(tree_error(
                    Some(key),
                    ViewGeometryTreeRelation::DuplicateNode,
                ));
            }
            let style = input
                .styles
                .node(&key)
                .expect("Style resolution covers every retained node before geometry inventory");
            debug_assert_eq!(style.physical().node(), &key);
            debug_assert_eq!(
                style.physical().owner(),
                source.kind.runtime_geometry_owner()
            );
            let (product, target_keys) = bind_product(input, mount, source, &key)?;
            if let BundleViewStyleNodeKind::CallView { view } = &source.kind {
                calls
                    .entry(CallAttachmentKey {
                        handle: mount.handle.clone(),
                        path: source.path.segments().to_vec(),
                        instruction: source.instruction,
                    })
                    .or_default()
                    .push((view.clone(), key.clone()));
            }
            order.push(key.clone());
            raw.insert(
                key.clone(),
                RawNode {
                    key,
                    mount,
                    source,
                    style,
                    structural_parent: None,
                    structural_children: Vec::new(),
                    product,
                    target_keys,
                },
            );
        }
    }

    let mut attached_calls = BTreeSet::new();
    for key in &order {
        let parent = structural_parent(raw.get(key).expect("ordered raw node exists"), &calls)?;
        if let Some(parent_key) = &parent {
            if !raw.contains_key(parent_key) {
                return Err(tree_error(
                    Some(key.clone()),
                    ViewGeometryTreeRelation::MissingParent {
                        parent: parent_key.clone(),
                    },
                ));
            }
            if parent_key.mount() != key.mount()
                && raw
                    .get(key)
                    .is_some_and(|node| node.mount.path.segments().is_empty())
            {
                return Err(tree_error(
                    Some(key.clone()),
                    ViewGeometryTreeRelation::CrossMountParent {
                        parent: parent_key.clone(),
                    },
                ));
            }
            if parent_key.mount() != key.mount() {
                attached_calls.insert(parent_key.clone());
            }
        }
        raw.get_mut(key)
            .expect("ordered raw node exists")
            .structural_parent = parent;
    }

    validate_cycles(&raw, &order)?;
    for key in &order {
        if let Some(parent) = raw
            .get(key)
            .expect("ordered raw node exists")
            .structural_parent
            .clone()
        {
            raw.get_mut(&parent)
                .expect("validated parent exists")
                .structural_children
                .push(key.clone());
        }
    }

    for key in &order {
        let node = raw.get(key).expect("ordered raw node exists");
        if node.style.physical().participation() == ViewRuntimeGeometryParticipation::Leaf
            && let Some(first_child) = node.structural_children.first()
        {
            return Err(tree_error(
                Some(key.clone()),
                ViewGeometryTreeRelation::LeafHasChildren {
                    first_child: first_child.clone(),
                },
            ));
        }
        if matches!(node.source.kind, BundleViewStyleNodeKind::CallView { .. })
            && !attached_calls.contains(key)
        {
            return Err(tree_error(
                Some(key.clone()),
                ViewGeometryTreeRelation::MissingNestedRoot { call: key.clone() },
            ));
        }
    }

    let suppressed = suppressed_nodes(&raw, &order);
    let transparent = order
        .iter()
        .filter(|key| {
            !suppressed.contains(*key)
                && raw.get(*key).is_some_and(|node| {
                    node.style.physical().participation()
                        == ViewRuntimeGeometryParticipation::Transparent
                })
        })
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut nodes = BTreeMap::new();
    let mut preorder = Vec::new();
    let mut roots = Vec::new();
    for key in &order {
        let raw_node = raw.get(key).expect("ordered raw node exists");
        if suppressed.contains(key) || transparent.contains(key) {
            continue;
        }
        let parent = nearest_executable_parent(raw_node, &raw, &suppressed, &transparent);
        if parent.is_none() {
            roots.push(key.clone());
        }
        preorder.push(key.clone());
        nodes.insert(
            key.clone(),
            ViewGeometryInventoryNode {
                parent,
                children: Vec::new(),
                style: raw_node.style,
                owner: raw_node.source.kind.runtime_geometry_owner(),
                product: raw_node.product,
            },
        );
    }
    for key in &preorder {
        if let Some(parent) = nodes
            .get(key)
            .expect("executable node exists")
            .parent
            .clone()
        {
            nodes
                .get_mut(&parent)
                .expect("executable parent exists")
                .children
                .push(key.clone());
        }
    }

    let mut all_targets = BTreeMap::<ViewGeometryTargetKey, ViewStyleNodeKey>::new();
    for key in &order {
        for target in &raw.get(key).expect("ordered raw node exists").target_keys {
            if let Some(first) = all_targets.insert(target.clone(), key.clone()) {
                return Err(tree_error(
                    Some(key.clone()),
                    ViewGeometryTreeRelation::DuplicateTarget {
                        target: target.clone(),
                        first,
                        second: key.clone(),
                    },
                ));
            }
        }
    }
    let targets = all_targets
        .into_iter()
        .filter(|(_, node)| nodes.contains_key(node))
        .collect();

    let mut postorder = Vec::with_capacity(preorder.len());
    for root in &roots {
        append_postorder(root, &nodes, &mut postorder);
    }
    Ok(ViewGeometryInventory {
        nodes,
        preorder,
        postorder,
        transparent,
        suppressed,
        targets,
    })
}

fn structural_parent(
    node: &RawNode<'_>,
    calls: &BTreeMap<CallAttachmentKey, Vec<(ViewId, ViewStyleNodeKey)>>,
) -> Result<Option<ViewStyleNodeKey>, ViewGeometryRuntimeError> {
    if let Some(parent) = &node.source.parent {
        return Ok(Some(parent.style_node_key(node.mount.mount)));
    }
    let Some((terminal, caller_path)) = node.mount.path.segments().split_last() else {
        return Ok(None);
    };
    let BundleViewInstancePathSegment::Call { instruction, .. } = terminal else {
        return Err(tree_error(
            Some(node.key.clone()),
            ViewGeometryTreeRelation::CallAttachmentMismatch {
                expected_call: node.key.clone(),
                actual_parent: None,
            },
        ));
    };
    let candidates = calls.get(&CallAttachmentKey {
        handle: node.mount.handle.clone(),
        path: caller_path.to_vec(),
        instruction: *instruction,
    });
    let expected = candidates
        .into_iter()
        .flatten()
        .find(|(view, _)| view == &node.mount.view)
        .map(|(_, key)| key.clone());
    if let Some(expected) = expected {
        return Ok(Some(expected));
    }
    let fallback = candidates
        .and_then(|candidates| candidates.first())
        .map_or_else(|| node.key.clone(), |(_, key)| key.clone());
    Err(tree_error(
        Some(node.key.clone()),
        ViewGeometryTreeRelation::CallAttachmentMismatch {
            expected_call: fallback,
            actual_parent: None,
        },
    ))
}

fn validate_cycles(
    raw: &BTreeMap<ViewStyleNodeKey, RawNode<'_>>,
    order: &[ViewStyleNodeKey],
) -> Result<(), ViewGeometryRuntimeError> {
    for subject in order {
        let mut seen = BTreeSet::new();
        let mut current = Some(subject);
        while let Some(key) = current {
            if !seen.insert(key.clone()) {
                return Err(tree_error(
                    Some(subject.clone()),
                    ViewGeometryTreeRelation::Cycle {
                        repeated: key.clone(),
                    },
                ));
            }
            current = raw
                .get(key)
                .and_then(|node| node.structural_parent.as_ref());
        }
    }
    Ok(())
}

fn suppressed_nodes(
    raw: &BTreeMap<ViewStyleNodeKey, RawNode<'_>>,
    order: &[ViewStyleNodeKey],
) -> BTreeSet<ViewStyleNodeKey> {
    order
        .iter()
        .filter(|key| {
            let mut current = Some(*key);
            while let Some(candidate) = current {
                let node = raw.get(candidate).expect("validated ancestry exists");
                if node.style.physical().participation()
                    == ViewRuntimeGeometryParticipation::Suppressed
                {
                    return true;
                }
                current = node.structural_parent.as_ref();
            }
            false
        })
        .cloned()
        .collect()
}

fn nearest_executable_parent(
    node: &RawNode<'_>,
    raw: &BTreeMap<ViewStyleNodeKey, RawNode<'_>>,
    suppressed: &BTreeSet<ViewStyleNodeKey>,
    transparent: &BTreeSet<ViewStyleNodeKey>,
) -> Option<ViewStyleNodeKey> {
    let mut current = node.structural_parent.as_ref();
    while let Some(candidate) = current {
        if !suppressed.contains(candidate) && !transparent.contains(candidate) {
            return Some(candidate.clone());
        }
        current = raw
            .get(candidate)
            .and_then(|parent| parent.structural_parent.as_ref());
    }
    None
}

fn append_postorder(
    node: &ViewStyleNodeKey,
    nodes: &BTreeMap<ViewStyleNodeKey, ViewGeometryInventoryNode<'_>>,
    output: &mut Vec<ViewStyleNodeKey>,
) {
    for child in &nodes.get(node).expect("postorder node exists").children {
        append_postorder(child, nodes, output);
    }
    output.push(node.clone());
}

fn bind_product<'a>(
    input: &ViewGeometryFrameInput<'a>,
    mount: &'a BundleViewMountOutput,
    source: &'a BundleViewStyleNode,
    node: &ViewStyleNodeKey,
) -> Result<(ViewIntrinsicProductRef<'a>, Vec<ViewGeometryTargetKey>), ViewGeometryRuntimeError> {
    let owner = source.kind.runtime_geometry_owner();
    match &source.kind {
        BundleViewStyleNodeKind::Element { element, target } => {
            bind_element(input, mount, node, owner, *element, target.as_deref())
        }
        BundleViewStyleNodeKind::Text { text_source } => {
            let matches = mount
                .text
                .iter()
                .filter(|text| text.source_id == *text_source)
                .collect::<Vec<_>>();
            let target = ViewGeometryTargetKey::new(
                ViewGeometryProductKind::TextOutput,
                mount.scoped_id(text_source),
            );
            let text = exactly_one(
                node,
                owner,
                ViewGeometryProductKind::TextOutput,
                target,
                &matches,
            )?;
            let targets = text
                .targets
                .iter()
                .map(|fragment| {
                    ViewGeometryTargetKey::new(
                        ViewGeometryProductKind::TextOutput,
                        mount.scoped_id(&fragment.public_id),
                    )
                })
                .collect();
            Ok((ViewIntrinsicProductRef::TextOutput(text), targets))
        }
        BundleViewStyleNodeKind::Image { image, target } => {
            let id = mount.scoped_id(target.as_deref().unwrap_or(image));
            let key = ViewGeometryTargetKey::new(ViewGeometryProductKind::Image, id.clone());
            let matches = input
                .presentation
                .images
                .iter()
                .filter(|record| record.id == id || record.target.as_deref() == Some(id.as_str()))
                .collect::<Vec<_>>();
            let image = exactly_one(
                node,
                owner,
                ViewGeometryProductKind::Image,
                key.clone(),
                &matches,
            )?;
            reject_incompatible(
                input,
                node,
                owner,
                Some(ViewGeometryProductKind::Image),
                &key,
            )?;
            Ok((ViewIntrinsicProductRef::Image(image), vec![key]))
        }
        BundleViewStyleNodeKind::Custom { .. } | BundleViewStyleNodeKind::CallView { .. } => {
            Ok((ViewIntrinsicProductRef::EmptyContainer, Vec::new()))
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the closed ViewElementKind-to-product ownership table is kept exhaustive at one typed boundary"
)]
fn bind_element<'a>(
    input: &ViewGeometryFrameInput<'a>,
    mount: &BundleViewMountOutput,
    node: &ViewStyleNodeKey,
    owner: ViewRuntimeGeometryOwner,
    element: ViewElementKind,
    authored_target: Option<&str>,
) -> Result<(ViewIntrinsicProductRef<'a>, Vec<ViewGeometryTargetKey>), ViewGeometryRuntimeError> {
    let expected = match element {
        ViewElementKind::Button => Some(ViewGeometryProductKind::ActionButton),
        ViewElementKind::TextField => Some(ViewGeometryProductKind::TextField),
        ViewElementKind::TextArea => Some(ViewGeometryProductKind::TextArea),
        ViewElementKind::SecureField => Some(ViewGeometryProductKind::SecureField),
        ViewElementKind::Scroll => Some(ViewGeometryProductKind::ScrollRegion),
        ViewElementKind::Panel | ViewElementKind::Box if authored_target.is_some() => {
            Some(ViewGeometryProductKind::Surface)
        }
        ViewElementKind::Panel
        | ViewElementKind::Box
        | ViewElementKind::Row
        | ViewElementKind::Column
        | ViewElementKind::Stack => None,
    };
    let Some(expected) = expected else {
        if let Some(target) = authored_target {
            let id = mount.scoped_id(target);
            if let Some((actual, _)) = records_for_id(input, &id).into_iter().next() {
                return Err(ViewGeometryProductError::OwnerProductMismatch {
                    node: node.clone(),
                    owner,
                    expected: None,
                    actual,
                    target: ViewGeometryTargetKey::new(actual, id),
                }
                .into());
            }
        }
        return Ok((ViewIntrinsicProductRef::EmptyContainer, Vec::new()));
    };
    let Some(authored_target) = authored_target else {
        return Err(ViewGeometryProductError::MissingProductRecord {
            node: node.clone(),
            owner,
            expected,
            target: None,
        }
        .into());
    };
    let id = mount.scoped_id(authored_target);
    let key = ViewGeometryTargetKey::new(expected, id.clone());
    reject_incompatible(input, node, owner, Some(expected), &key)?;
    match expected {
        ViewGeometryProductKind::ActionButton => {
            let records = input
                .presentation
                .action_buttons
                .iter()
                .filter(|record| record.target == id)
                .collect::<Vec<_>>();
            Ok((
                ViewIntrinsicProductRef::ActionButton(exactly_one(
                    node,
                    owner,
                    expected,
                    key.clone(),
                    &records,
                )?),
                vec![key],
            ))
        }
        ViewGeometryProductKind::TextField
        | ViewGeometryProductKind::TextArea
        | ViewGeometryProductKind::SecureField => {
            let required_kind = match expected {
                ViewGeometryProductKind::TextField => ViewInputKind::TextField,
                ViewGeometryProductKind::TextArea => ViewInputKind::TextArea,
                ViewGeometryProductKind::SecureField => ViewInputKind::SecureField,
                _ => unreachable!("closed text-control product inventory"),
            };
            let records = input
                .presentation
                .text_inputs
                .iter()
                .filter(|record| record.target == id && record.kind == required_kind)
                .collect::<Vec<_>>();
            Ok((
                ViewIntrinsicProductRef::TextControl(exactly_one(
                    node,
                    owner,
                    expected,
                    key.clone(),
                    &records,
                )?),
                vec![key],
            ))
        }
        ViewGeometryProductKind::ScrollRegion => {
            let records = input
                .presentation
                .scroll_regions
                .iter()
                .filter(|record| record.target == id)
                .collect::<Vec<_>>();
            Ok((
                ViewIntrinsicProductRef::ScrollRegion(exactly_one(
                    node,
                    owner,
                    expected,
                    key.clone(),
                    &records,
                )?),
                vec![key],
            ))
        }
        ViewGeometryProductKind::Surface => {
            let records = input
                .presentation
                .surfaces
                .iter()
                .filter(|record| record.target == id)
                .collect::<Vec<_>>();
            Ok((
                ViewIntrinsicProductRef::Surface(exactly_one(
                    node,
                    owner,
                    expected,
                    key.clone(),
                    &records,
                )?),
                vec![key],
            ))
        }
        ViewGeometryProductKind::TextOutput | ViewGeometryProductKind::Image => {
            unreachable!("element products are closed above")
        }
    }
}

fn exactly_one<'a, T>(
    node: &ViewStyleNodeKey,
    owner: ViewRuntimeGeometryOwner,
    expected: ViewGeometryProductKind,
    target: ViewGeometryTargetKey,
    records: &[&'a T],
) -> Result<&'a T, ViewGeometryRuntimeError> {
    match records {
        [record] => Ok(*record),
        [] => Err(ViewGeometryProductError::MissingProductRecord {
            node: node.clone(),
            owner,
            expected,
            target: Some(target),
        }
        .into()),
        _ => Err(ViewGeometryProductError::DuplicateProductRecord {
            node: node.clone(),
            owner,
            expected,
            target,
            count: records.len(),
        }
        .into()),
    }
}

fn reject_incompatible(
    input: &ViewGeometryFrameInput<'_>,
    node: &ViewStyleNodeKey,
    owner: ViewRuntimeGeometryOwner,
    expected: Option<ViewGeometryProductKind>,
    target: &ViewGeometryTargetKey,
) -> Result<(), ViewGeometryRuntimeError> {
    if let Some((actual, _)) = records_for_id(input, target.id())
        .into_iter()
        .find(|(actual, _)| Some(*actual) != expected)
    {
        return Err(ViewGeometryProductError::OwnerProductMismatch {
            node: node.clone(),
            owner,
            expected,
            actual,
            target: ViewGeometryTargetKey::new(actual, target.id().to_owned()),
        }
        .into());
    }
    Ok(())
}

fn records_for_id<'a>(
    input: &ViewGeometryFrameInput<'a>,
    id: &str,
) -> Vec<(ViewGeometryProductKind, &'a str)> {
    let mut records = Vec::new();
    records.extend(
        input
            .presentation
            .action_buttons
            .iter()
            .filter(|record| record.target == id)
            .map(|record| {
                (
                    ViewGeometryProductKind::ActionButton,
                    record.public_id.as_str(),
                )
            }),
    );
    records.extend(
        input
            .presentation
            .text_inputs
            .iter()
            .filter(|record| record.target == id)
            .map(|record| {
                let kind = match record.kind {
                    ViewInputKind::TextField => ViewGeometryProductKind::TextField,
                    ViewInputKind::TextArea => ViewGeometryProductKind::TextArea,
                    ViewInputKind::SecureField => ViewGeometryProductKind::SecureField,
                };
                (kind, record.public_id.as_str())
            }),
    );
    records.extend(
        input
            .presentation
            .scroll_regions
            .iter()
            .filter(|record| record.target == id)
            .map(|record| {
                (
                    ViewGeometryProductKind::ScrollRegion,
                    record.public_id.as_str(),
                )
            }),
    );
    records.extend(
        input
            .presentation
            .surfaces
            .iter()
            .filter(|record| record.target == id || record.public_id == id)
            .map(|record| (ViewGeometryProductKind::Surface, record.public_id.as_str())),
    );
    records.extend(
        input
            .presentation
            .images
            .iter()
            .filter(|record| record.id == id || record.target.as_deref() == Some(id))
            .map(|record| (ViewGeometryProductKind::Image, record.id.as_str())),
    );
    records
}

fn tree_error(
    node: Option<ViewStyleNodeKey>,
    relation: ViewGeometryTreeRelation,
) -> ViewGeometryRuntimeError {
    ViewGeometryRuntimeError::Tree {
        node,
        relation: Box::new(relation),
    }
}

#[allow(
    dead_code,
    reason = "mount identity is part of inventory audit context"
)]
const fn _mount(node: &RawNode<'_>) -> ViewMountId {
    node.mount.mount
}
