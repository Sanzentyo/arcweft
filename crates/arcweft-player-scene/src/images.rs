use arcweft_bundle::{ArcweftBundle, BundleCodecError, BundleImageFormat, BundleImageObject};
use arcweft_image::{DecodedImage, ImageDecodeOptions, ImageError, ImageFormat};
use arcweft_layout::{
    LayoutRect, LayoutSize, ScalePolicy,
    stage_placement::{ResolvedStagePlacement, StagePlacement, StagePlacementContext, StageRect},
};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::image::{ImageObjectAlignment, ImageObjectFit, ImageObjectTransform};
use arcweft_render_wgpu::geometry::{RenderImage, RenderImageFrame, RenderViewport};
use num_traits::ToPrimitive;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct BundleImageCatalog {
    images: Vec<DecodedBundleImage>,
}

#[derive(Clone, Debug)]
struct DecodedBundleImage {
    asset_id: String,
    image: DecodedImage,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BundleImageCatalogError {
    #[error("bundle image asset `{0}` was not found")]
    MissingAsset(String),
    #[error("bundle image object `{object_id}` references missing asset `{asset_id}`")]
    MissingObjectAsset { object_id: String, asset_id: String },
    #[error("bundle image asset `{asset_id}` could not be read: {message}")]
    AssetRead { asset_id: String, message: String },
    #[error("bundle image asset `{asset_id}` could not be decoded: {message}")]
    Decode { asset_id: String, message: String },
    #[error("bundle image object `{object_id}` has empty bounds")]
    EmptyBounds { object_id: String },
    #[error("bundle image object `{object_id}` placement failed: {message}")]
    Placement { object_id: String, message: String },
}

impl BundleImageCatalog {
    pub fn from_bundle(bundle: &ArcweftBundle) -> Result<Self, BundleImageCatalogError> {
        bundle
            .image_assets
            .iter()
            .map(|asset| {
                let bytes = bundle
                    .image_asset_bytes(&asset.id)
                    .map_err(|error| BundleImageCatalogError::asset_read(&asset.id, &error))?
                    .ok_or_else(|| BundleImageCatalogError::MissingAsset(asset.id.clone()))?;
                let image = arcweft_image::decode_image_bytes(
                    image_format(asset.format),
                    bytes,
                    ImageDecodeOptions::default(),
                )
                .map_err(|error| BundleImageCatalogError::decode(&asset.id, &error))?;
                Ok(DecodedBundleImage {
                    asset_id: asset.id.clone(),
                    image,
                })
            })
            .collect::<Result<Vec<_>, BundleImageCatalogError>>()
            .map(|images| Self { images })
    }

    pub fn render_images(
        &self,
        objects: &[BundleImageObject],
        elapsed_millis: u64,
        viewport: RenderViewport,
    ) -> Result<Vec<RenderImage>, BundleImageCatalogError> {
        objects
            .iter()
            .map(|object| self.render_image(object, elapsed_millis, viewport))
            .collect()
    }

    fn render_image(
        &self,
        object: &BundleImageObject,
        elapsed_millis: u64,
        viewport: RenderViewport,
    ) -> Result<RenderImage, BundleImageCatalogError> {
        let decoded = self
            .images
            .iter()
            .find(|image| image.asset_id == object.asset)
            .ok_or_else(|| BundleImageCatalogError::MissingObjectAsset {
                object_id: object.id.clone(),
                asset_id: object.asset.clone(),
            })?;
        let frame = decoded
            .image
            .frame_at_time_millis(object.playback.local_time_millis(elapsed_millis))
            .ok_or_else(|| BundleImageCatalogError::Decode {
                asset_id: object.asset.clone(),
                message: "decoded image has no frame at visual time".to_owned(),
            })?;
        let placement = render_placement(object, viewport)?;
        Ok(RenderImage {
            id: object.id.clone(),
            frame: RenderImageFrame {
                width: frame.dimensions().width(),
                height: frame.dimensions().height(),
                rgba: frame.rgba().to_vec(),
            },
            bounds: hit_rect_from_layout(placement.output_bbox),
            placement: Some(placement),
            fit: render_fit(object.fit),
            alignment: ImageObjectAlignment::new(
                object.alignment.x_milli,
                object.alignment.y_milli,
            ),
            transform: ImageObjectTransform {
                m11_milli: object.transform.m11_milli,
                m12_milli: object.transform.m12_milli,
                m21_milli: object.transform.m21_milli,
                m22_milli: object.transform.m22_milli,
                tx_milli: object.transform.tx_milli,
                ty_milli: object.transform.ty_milli,
            },
            opacity_milli: object.opacity_milli,
        })
    }
}

fn render_placement(
    object: &BundleImageObject,
    viewport: RenderViewport,
) -> Result<ResolvedStagePlacement, BundleImageCatalogError> {
    let placement = object.placement.unwrap_or_else(|| {
        StagePlacement::absolute(StageRect::new(
            object.bounds.x_milli,
            object.bounds.y_milli,
            object.bounds.width_milli,
            object.bounds.height_milli,
        ))
    });
    placement
        .resolve(
            StagePlacementContext::new(
                LayoutSize::new(1280.0, 720.0),
                LayoutSize::new(viewport.logical_width, viewport.logical_height),
            )
            .with_physical_viewport(LayoutSize::new(
                viewport
                    .physical_width
                    .to_f32()
                    .unwrap_or(viewport.logical_width),
                viewport
                    .physical_height
                    .to_f32()
                    .unwrap_or(viewport.logical_height),
            ))
            .with_scale_factor(viewport.physical_scale_factor_f32())
            .with_viewport_policy(ScalePolicy::Contain),
        )
        .map_err(|error| BundleImageCatalogError::Placement {
            object_id: object.id.clone(),
            message: error.to_string(),
        })
}

fn hit_rect_from_layout(rect: LayoutRect) -> HitRect {
    HitRect::new(
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    )
}

impl BundleImageCatalogError {
    fn asset_read(asset_id: &str, error: &BundleCodecError) -> Self {
        Self::AssetRead {
            asset_id: asset_id.to_owned(),
            message: error.to_string(),
        }
    }

    fn decode(asset_id: &str, error: &ImageError) -> Self {
        Self::Decode {
            asset_id: asset_id.to_owned(),
            message: error.to_string(),
        }
    }
}

fn image_format(format: BundleImageFormat) -> ImageFormat {
    match format {
        BundleImageFormat::Png => ImageFormat::Png,
        BundleImageFormat::Jpeg => ImageFormat::Jpeg,
        BundleImageFormat::Gif => ImageFormat::Gif,
        BundleImageFormat::WebP => ImageFormat::WebP,
    }
}

fn render_fit(fit: arcweft_bundle::BundleImageObjectFit) -> ImageObjectFit {
    match fit {
        arcweft_bundle::BundleImageObjectFit::Contain => ImageObjectFit::Contain,
        arcweft_bundle::BundleImageObjectFit::Cover => ImageObjectFit::Cover,
        arcweft_bundle::BundleImageObjectFit::Stretch => ImageObjectFit::Stretch,
        arcweft_bundle::BundleImageObjectFit::Intrinsic => ImageObjectFit::Intrinsic,
    }
}
