//! Typed intrinsic-product adapters.

use arcweft_bundle::resource_codec::{
    ViewRuntimeActionButton, ViewRuntimeGeometryOwner, ViewRuntimeSurface, ViewRuntimeTextControl,
};
use arcweft_interaction_model::presentation::PresentationImage;
use arcweft_runtime_driver::view_runtime::BundleViewTextTarget;
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
    Surface(&'a ViewRuntimeSurface),
    TextOutput(&'a BundleViewTextTarget),
    Image(&'a PresentationImage),
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
            ViewIntrinsicProductRef::TextOutput(target) => {
                if !target.bounds.is_valid() {
                    return Err(ViewIntrinsicGeometryError::MissingIntrinsicMeasure);
                }
                (
                    ViewGeometrySize::new(target.bounds.width_milli, target.bounds.height_milli),
                    revision([
                        target.public_id.as_bytes(),
                        &target.bounds.x_milli.to_le_bytes(),
                        &target.bounds.y_milli.to_le_bytes(),
                        &target.bounds.width_milli.to_le_bytes(),
                        &target.bounds.height_milli.to_le_bytes(),
                    ]),
                )
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
