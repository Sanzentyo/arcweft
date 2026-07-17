//! Typed intrinsic-product adapters.

use arcweft_bundle::BundleImageObject;
use arcweft_bundle::resource_codec::{
    ViewRuntimeActionButton, ViewRuntimeGeometryOwner, ViewRuntimeScrollRegion, ViewRuntimeSurface,
    ViewRuntimeTextControl,
};
use arcweft_runtime_driver::view_runtime::BundleViewTextOutput;
use arcweft_view::geometry::{
    ViewAvailableGeometrySize, ViewGeometrySize, ViewIntrinsicMeasure, ViewIntrinsicMeasureRevision,
};
use arcweft_view::style::{ViewPhysicalBoxStyle, ViewStyleNodeKey};
use thiserror::Error;

#[derive(Clone, Copy, Debug)]
pub(crate) enum ViewIntrinsicProductRef<'a> {
    EmptyContainer,
    ActionButton(&'a ViewRuntimeActionButton),
    TextControl(&'a ViewRuntimeTextControl),
    ScrollRegion(&'a ViewRuntimeScrollRegion),
    Surface(&'a ViewRuntimeSurface),
    TextOutput(&'a BundleViewTextOutput),
    Image(&'a BundleImageObject),
}

impl ViewIntrinsicProductRef<'_> {
    pub const fn contributes_intrinsic_size(self) -> bool {
        !matches!(self, Self::EmptyContainer)
    }
}

#[allow(
    dead_code,
    reason = "custom intrinsic providers consume the exact node, owner, Style, and available-size context; the stock presentation provider is product-only"
)]
pub(crate) struct ViewIntrinsicGeometryRequest<'a> {
    pub node: &'a ViewStyleNodeKey,
    pub owner: ViewRuntimeGeometryOwner,
    pub box_style: &'a ViewPhysicalBoxStyle,
    pub product: ViewIntrinsicProductRef<'a>,
    pub available: ViewAvailableGeometrySize,
}

pub(crate) trait ViewIntrinsicGeometryProvider {
    fn measure(
        &mut self,
        request: &ViewIntrinsicGeometryRequest<'_>,
    ) -> Result<ViewIntrinsicMeasure, ViewIntrinsicGeometryError>;
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum ViewIntrinsicGeometryError {
    #[error("the retained product has no intrinsic measurement")]
    MissingIntrinsicMeasure,
    #[error("the retained product intrinsic bounds exceed the physical milli range")]
    IntrinsicBoundsOverflow,
}

#[derive(Debug, Default)]
pub(crate) struct PresentationIntrinsicGeometryProvider;

impl ViewIntrinsicGeometryProvider for PresentationIntrinsicGeometryProvider {
    fn measure(
        &mut self,
        request: &ViewIntrinsicGeometryRequest<'_>,
    ) -> Result<ViewIntrinsicMeasure, ViewIntrinsicGeometryError> {
        let (size, revision) = match request.product {
            ViewIntrinsicProductRef::EmptyContainer => (ViewGeometrySize::default(), 0),
            ViewIntrinsicProductRef::ActionButton(button) => (
                ViewGeometrySize::new(button.bounds.width_milli, button.bounds.height_milli),
                revision([
                    button.public_id.as_bytes(),
                    button.target.as_bytes(),
                    &button.bounds.x_milli.to_le_bytes(),
                    &button.bounds.y_milli.to_le_bytes(),
                    &button.bounds.width_milli.to_le_bytes(),
                    &button.bounds.height_milli.to_le_bytes(),
                ]),
            ),
            ViewIntrinsicProductRef::TextControl(control) => (
                ViewGeometrySize::new(control.bounds.width_milli, control.bounds.height_milli),
                revision([
                    control.public_id.as_bytes(),
                    control.target.as_bytes(),
                    &control.bounds.x_milli.to_le_bytes(),
                    &control.bounds.y_milli.to_le_bytes(),
                    &control.bounds.width_milli.to_le_bytes(),
                    &control.bounds.height_milli.to_le_bytes(),
                ]),
            ),
            ViewIntrinsicProductRef::ScrollRegion(region) => (
                ViewGeometrySize::new(region.bounds.width_milli, region.bounds.height_milli),
                revision([
                    region.public_id.as_bytes(),
                    region.target.as_bytes(),
                    &region.bounds.x_milli.to_le_bytes(),
                    &region.bounds.y_milli.to_le_bytes(),
                    &region.bounds.width_milli.to_le_bytes(),
                    &region.bounds.height_milli.to_le_bytes(),
                ]),
            ),
            ViewIntrinsicProductRef::Surface(surface) => (
                ViewGeometrySize::new(surface.bounds.width_milli, surface.bounds.height_milli),
                revision([
                    surface.public_id.as_bytes(),
                    surface.target.as_bytes(),
                    &surface.bounds.x_milli.to_le_bytes(),
                    &surface.bounds.y_milli.to_le_bytes(),
                    &surface.bounds.width_milli.to_le_bytes(),
                    &surface.bounds.height_milli.to_le_bytes(),
                ]),
            ),
            ViewIntrinsicProductRef::TextOutput(text) => {
                let size = text_outer_size(text)?;
                let mut transcript = RevisionTranscript::new();
                transcript.bytes(text.source_id.as_bytes());
                for target in &text.targets {
                    transcript.bytes(target.public_id.as_bytes());
                    transcript.i32(target.bounds.x_milli);
                    transcript.i32(target.bounds.y_milli);
                    transcript.u32(target.bounds.width_milli);
                    transcript.u32(target.bounds.height_milli);
                }
                (size, transcript.finish())
            }
            ViewIntrinsicProductRef::Image(image) => (
                ViewGeometrySize::new(image.bounds.width_milli, image.bounds.height_milli),
                revision([
                    image.id.as_bytes(),
                    image.target.as_deref().unwrap_or_default().as_bytes(),
                    &image.bounds.x_milli.to_le_bytes(),
                    &image.bounds.y_milli.to_le_bytes(),
                    &image.bounds.width_milli.to_le_bytes(),
                    &image.bounds.height_milli.to_le_bytes(),
                ]),
            ),
        };
        Ok(ViewIntrinsicMeasure {
            content_size: size,
            revision: ViewIntrinsicMeasureRevision::new(revision),
        })
    }
}

fn text_outer_size(
    text: &BundleViewTextOutput,
) -> Result<ViewGeometrySize, ViewIntrinsicGeometryError> {
    let mut targets = text.targets.iter();
    let Some(first) = targets.next() else {
        return Err(ViewIntrinsicGeometryError::MissingIntrinsicMeasure);
    };
    let mut left = i64::from(first.bounds.x_milli);
    let mut top = i64::from(first.bounds.y_milli);
    let mut right = left + i64::from(first.bounds.width_milli);
    let mut bottom = top + i64::from(first.bounds.height_milli);
    for target in targets {
        let target_left = i64::from(target.bounds.x_milli);
        let target_top = i64::from(target.bounds.y_milli);
        left = left.min(target_left);
        top = top.min(target_top);
        right = right.max(target_left + i64::from(target.bounds.width_milli));
        bottom = bottom.max(target_top + i64::from(target.bounds.height_milli));
    }
    let width = u32::try_from(right - left)
        .map_err(|_| ViewIntrinsicGeometryError::IntrinsicBoundsOverflow)?;
    let height = u32::try_from(bottom - top)
        .map_err(|_| ViewIntrinsicGeometryError::IntrinsicBoundsOverflow)?;
    Ok(ViewGeometrySize::new(width, height))
}

fn revision<const N: usize>(parts: [&[u8]; N]) -> u64 {
    let mut transcript = RevisionTranscript::new();
    for part in parts {
        transcript.bytes(part);
    }
    transcript.finish()
}

struct RevisionTranscript(u64);

impl RevisionTranscript {
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, value: &[u8]) {
        self.u64(value.len() as u64);
        for byte in value {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn i32(&mut self, value: i32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}
