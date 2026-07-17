//! Postorder intrinsic aggregation and exact measurement caching.

use super::ViewGeometryFrameInput;
use super::cache::{PlayerViewGeometryState, ViewMeasureCacheEntry};
use super::error::ViewGeometryRuntimeError;
use super::intrinsic::{
    ViewIntrinsicGeometryError, ViewIntrinsicGeometryProvider, ViewIntrinsicGeometryRequest,
};
use super::tree::ViewGeometryInventory;
use arcweft_bundle::resource_codec::ViewRuntimeGeometryParticipation;
use arcweft_view::geometry::{
    ViewChildOuterDependency, ViewIntrinsicMeasure, ViewIntrinsicMeasureRevision,
    ViewMeasuredGeometryKey, ViewOuterMeasureRevision, flow_intrinsic_size, measure_box,
    outer_size,
};
use arcweft_view::style::{ViewPosition, ViewStyleNodeKey};
use std::collections::BTreeMap;

pub(super) struct MeasuredInventory {
    pub entries: BTreeMap<ViewStyleNodeKey, ViewMeasureCacheEntry>,
}

impl MeasuredInventory {
    pub fn entry(&self, node: &ViewStyleNodeKey) -> &ViewMeasureCacheEntry {
        self.entries
            .get(node)
            .expect("postorder measurement covers every executable node")
    }
}

pub(super) fn measure_inventory(
    state: &PlayerViewGeometryState,
    inventory: &ViewGeometryInventory<'_>,
    input: &ViewGeometryFrameInput<'_>,
    intrinsic: &mut dyn ViewIntrinsicGeometryProvider,
) -> Result<MeasuredInventory, ViewGeometryRuntimeError> {
    let mut entries = BTreeMap::<ViewStyleNodeKey, ViewMeasureCacheEntry>::new();
    for key in &inventory.postorder {
        let node = inventory.node(key);
        let children = node
            .children
            .iter()
            .filter(|child| {
                matches!(
                    inventory.node(child).box_style().position,
                    ViewPosition::Static | ViewPosition::Relative
                )
            })
            .map(|child| {
                let entry = entries
                    .get(child)
                    .expect("participating child was measured earlier in postorder");
                ViewChildOuterDependency {
                    node: child.clone(),
                    outer_size: entry.outer,
                    revision: ViewOuterMeasureRevision::for_measured(
                        entry.measured.revision,
                        entry.outer.width_milli,
                        entry.outer.height_milli,
                    ),
                }
            })
            .collect::<Vec<_>>();
        let available = inventory.available(input, key);
        let measured_intrinsic = match node.style.physical().participation() {
            ViewRuntimeGeometryParticipation::Container => {
                let container = node
                    .container_style()
                    .expect("container participation carries a container packet");
                let flow_size = flow_intrinsic_size(
                    key,
                    container,
                    &children
                        .iter()
                        .map(|child| child.outer_size)
                        .collect::<Vec<_>>(),
                )?;
                let product = node
                    .product
                    .contributes_intrinsic_size()
                    .then(|| measure_product(intrinsic, key, node, available))
                    .transpose()?;
                ViewIntrinsicMeasure {
                    content_size: product.map_or(flow_size, |product| {
                        arcweft_view::geometry::ViewGeometrySize::new(
                            flow_size.width_milli.max(product.content_size.width_milli),
                            flow_size
                                .height_milli
                                .max(product.content_size.height_milli),
                        )
                    }),
                    revision: child_intrinsic_revision(&children, product.as_ref()),
                }
            }
            ViewRuntimeGeometryParticipation::Leaf => {
                measure_product(intrinsic, key, node, available)?
            }
            ViewRuntimeGeometryParticipation::Transparent
            | ViewRuntimeGeometryParticipation::Suppressed => {
                unreachable!("inventory excludes transparent and suppressed nodes")
            }
        };
        let cache_key = ViewMeasuredGeometryKey {
            node: key.clone(),
            box_style: *node.box_style(),
            container_style: node.container_style(),
            intrinsic: measured_intrinsic,
            available,
            ordered_children: children,
        };
        let entry = match state.measure_entry(key) {
            Some(entry) if entry.key == cache_key => entry.clone(),
            _ => {
                let measured = measure_box(key, node.box_style(), measured_intrinsic)?;
                let outer = outer_size(key, measured)?;
                ViewMeasureCacheEntry {
                    key: cache_key,
                    measured,
                    outer,
                }
            }
        };
        entries.insert(key.clone(), entry);
    }
    Ok(MeasuredInventory { entries })
}

fn measure_product(
    intrinsic: &mut dyn ViewIntrinsicGeometryProvider,
    key: &ViewStyleNodeKey,
    node: &super::tree::ViewGeometryInventoryNode<'_>,
    available: arcweft_view::geometry::ViewAvailableGeometrySize,
) -> Result<ViewIntrinsicMeasure, ViewGeometryRuntimeError> {
    intrinsic
        .measure(&ViewIntrinsicGeometryRequest {
            node: key,
            owner: node.owner,
            box_style: node.box_style(),
            product: node.product,
            available,
        })
        .map_err(
            |source: ViewIntrinsicGeometryError| ViewGeometryRuntimeError::Intrinsic {
                node: key.clone(),
                owner: node.owner,
                source,
            },
        )
}

fn child_intrinsic_revision(
    children: &[ViewChildOuterDependency],
    product: Option<&ViewIntrinsicMeasure>,
) -> ViewIntrinsicMeasureRevision {
    let mut revision = 0xcbf2_9ce4_8422_2325_u64;
    write(&mut revision, &(children.len() as u64).to_le_bytes());
    for child in children {
        write(&mut revision, &child.node.mount().get().to_le_bytes());
        write(
            &mut revision,
            &(child.node.path().len() as u64).to_le_bytes(),
        );
        for word in child.node.path() {
            write(&mut revision, &word.to_le_bytes());
        }
        write(&mut revision, &child.node.instruction().to_le_bytes());
        write(&mut revision, &child.outer_size.width_milli.to_le_bytes());
        write(&mut revision, &child.outer_size.height_milli.to_le_bytes());
        write(&mut revision, &child.revision.value().to_le_bytes());
    }
    match product {
        Some(product) => {
            write(&mut revision, &[1]);
            write(
                &mut revision,
                &product.content_size.width_milli.to_le_bytes(),
            );
            write(
                &mut revision,
                &product.content_size.height_milli.to_le_bytes(),
            );
            write(&mut revision, &product.revision.value().to_le_bytes());
        }
        None => write(&mut revision, &[0]),
    }
    ViewIntrinsicMeasureRevision::new(revision)
}

fn write(revision: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *revision ^= u64::from(*byte);
        *revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
    }
}
