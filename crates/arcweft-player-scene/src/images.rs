use arcweft_bundle::{ArcweftBundle, BundleCodecError, BundleImageFormat, BundleImageObject};
use arcweft_image::{DecodedImage, ImageDecodeOptions, ImageError, ImageFormat};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::image::{ImageObjectAlignment, ImageObjectFit, ImageObjectTransform};
use arcweft_render_wgpu::geometry::{RenderImage, RenderImageFrame};
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
    ) -> Result<Vec<RenderImage>, BundleImageCatalogError> {
        objects
            .iter()
            .map(|object| self.render_image(object, elapsed_millis))
            .collect()
    }

    fn render_image(
        &self,
        object: &BundleImageObject,
        elapsed_millis: u64,
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
        Ok(RenderImage {
            id: object.id.clone(),
            frame: RenderImageFrame {
                width: frame.dimensions().width(),
                height: frame.dimensions().height(),
                rgba: frame.rgba().to_vec(),
            },
            bounds: render_bounds(object)?,
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

fn render_bounds(object: &BundleImageObject) -> Result<HitRect, BundleImageCatalogError> {
    let bounds = object.bounds;
    if bounds.width_milli == 0 || bounds.height_milli == 0 {
        return Err(BundleImageCatalogError::EmptyBounds {
            object_id: object.id.clone(),
        });
    }
    Ok(HitRect::new(
        milli_i32_to_f32(bounds.x_milli),
        milli_i32_to_f32(bounds.y_milli),
        milli_u32_to_f32(bounds.width_milli),
        milli_u32_to_f32(bounds.height_milli),
    ))
}

fn milli_i32_to_f32(value: i32) -> f32 {
    value.to_f32().unwrap_or(0.0) / 1_000.0
}

fn milli_u32_to_f32(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX) / 1_000.0
}
