use arcweft_bundle::{ArcweftBundle, BundleCodecError, BundleImageFormat, BundleImageObject};
use arcweft_image::{DecodedImage, ImageDecodeOptions, ImageError, ImageFormat};
use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::geometry::{RenderImage, RenderImageFrame};
use num_traits::ToPrimitive;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct BrowserImageCatalog {
    images: Vec<DecodedBundleImage>,
}

#[derive(Clone, Debug)]
struct DecodedBundleImage {
    asset_id: String,
    image: DecodedImage,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BrowserImageCatalogError {
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

impl BrowserImageCatalog {
    pub fn from_bundle(bundle: &ArcweftBundle) -> Result<Self, BrowserImageCatalogError> {
        bundle
            .image_assets
            .iter()
            .map(|asset| {
                let bytes = bundle
                    .image_asset_bytes(&asset.id)
                    .map_err(|error| BrowserImageCatalogError::asset_read(&asset.id, &error))?
                    .ok_or_else(|| BrowserImageCatalogError::MissingAsset(asset.id.clone()))?;
                let image = arcweft_image::decode_image_bytes(
                    image_format(asset.format),
                    bytes,
                    ImageDecodeOptions::default(),
                )
                .map_err(|error| BrowserImageCatalogError::decode(&asset.id, &error))?;
                Ok(DecodedBundleImage {
                    asset_id: asset.id.clone(),
                    image,
                })
            })
            .collect::<Result<Vec<_>, BrowserImageCatalogError>>()
            .map(|images| Self { images })
    }

    pub fn render_images(
        &self,
        objects: &[BundleImageObject],
        elapsed_millis: u64,
    ) -> Result<Vec<RenderImage>, BrowserImageCatalogError> {
        objects
            .iter()
            .map(|object| self.render_image(object, elapsed_millis))
            .collect()
    }

    fn render_image(
        &self,
        object: &BundleImageObject,
        elapsed_millis: u64,
    ) -> Result<RenderImage, BrowserImageCatalogError> {
        let decoded = self
            .images
            .iter()
            .find(|image| image.asset_id == object.asset)
            .ok_or_else(|| BrowserImageCatalogError::MissingObjectAsset {
                object_id: object.id.clone(),
                asset_id: object.asset.clone(),
            })?;
        let frame = decoded
            .image
            .frame_at_time_millis(elapsed_millis)
            .ok_or_else(|| BrowserImageCatalogError::Decode {
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
            opacity_milli: object.opacity_milli,
        })
    }
}

impl BrowserImageCatalogError {
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

fn render_bounds(object: &BundleImageObject) -> Result<HitRect, BrowserImageCatalogError> {
    let bounds = object.bounds;
    if bounds.width_milli == 0 || bounds.height_milli == 0 {
        return Err(BrowserImageCatalogError::EmptyBounds {
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

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::{
        BundleImageAnimation, BundleImageAsset, BundleImageDimensions, BundleImageObjectBounds,
        BundleManifest, BundleRuntimeSummary, BundleSource, BundleVirtualFile,
        BundleVirtualFileRef, BundleVirtualFileSpace,
    };
    use arcweft_core::bytecode::BytecodeProgram;
    use arcweft_render_text::LineDisplayCatalog;

    #[test]
    fn renders_declared_bundle_image_object_frames() {
        let bundle = image_bundle(
            BundleImageFormat::Gif,
            include_bytes!("../../../web/assets/generated-pulse.gif").to_vec(),
        );
        let catalog = BrowserImageCatalog::from_bundle(&bundle).expect("image catalog decodes");
        let images = catalog
            .render_images(&bundle.image_objects, 170)
            .expect("render images");

        assert_eq!(images.len(), 1);
        assert_eq!(images[0].id, "image.generated.pulse");
        assert_eq!(images[0].bounds, HitRect::new(12.0, 34.0, 56.0, 78.0));
        assert_eq!(images[0].frame.width, 96);
        assert_eq!(images[0].frame.height, 96);
        assert_eq!(images[0].opacity_milli, 875);
    }

    fn image_bundle(format: BundleImageFormat, bytes: Vec<u8>) -> ArcweftBundle {
        ArcweftBundle::new(
            BundleManifest {
                source_label: "test.arcw".to_owned(),
                profile_id: None,
                profile_kind: None,
                entry: None,
                adapter: None,
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    entry_flow: None,
                    flows: 0,
                    bytecode_instructions: 0,
                    line_task_groups: 0,
                    stream_plans: 0,
                    source_plans: 0,
                },
            },
            BundleSource {
                label: "test.arcw".to_owned(),
                text: String::new(),
            },
            BytecodeProgram::default(),
            LineDisplayCatalog::default(),
        )
        .with_virtual_files([BundleVirtualFile {
            space: BundleVirtualFileSpace::Asset,
            path: "generated/pulse.gif".to_owned(),
            bytes,
        }])
        .with_image_assets([BundleImageAsset {
            id: "asset.generated.pulse".to_owned(),
            file: BundleVirtualFileRef {
                space: BundleVirtualFileSpace::Asset,
                path: "generated/pulse.gif".to_owned(),
            },
            format,
            animation: BundleImageAnimation::Animated,
            dimensions: Some(BundleImageDimensions::new(96, 96)),
        }])
        .with_image_objects([BundleImageObject {
            id: "image.generated.pulse".to_owned(),
            asset: "asset.generated.pulse".to_owned(),
            bounds: BundleImageObjectBounds::from_px(12, 34, 56, 78),
            opacity_milli: 875,
        }])
    }
}
