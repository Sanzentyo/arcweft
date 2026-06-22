use crate::gate::AgentPolicyError;
use arcweft_agent_protocol::resource::AgentResource;
use arcweft_content_policy::RgbaImage;
use arcweft_image::{ImageDecodeOptions, ImageFormat, decode_image_bytes};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentImageEncoding {
    Png,
    Jpeg,
    RawRgba,
}

impl AgentImageEncoding {
    pub(crate) const fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::RawRgba => "application/octet-stream",
        }
    }

    pub(crate) const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::RawRgba => "rgba",
        }
    }

    pub(crate) fn encode_sanitized(
        self,
        image: &RgbaImage,
    ) -> Result<(Self, Vec<u8>), AgentPolicyError> {
        match self {
            Self::RawRgba => Ok((Self::RawRgba, image.pixels().to_vec())),
            Self::Png | Self::Jpeg => encode_png(image).map(|bytes| (Self::Png, bytes)),
        }
    }

    pub(crate) const fn pixel_format(self) -> Option<&'static str> {
        match self {
            Self::RawRgba => Some("rgba8_unorm"),
            Self::Png | Self::Jpeg => None,
        }
    }

    pub(crate) const fn row_stride_bytes(self, width: u32) -> Option<u32> {
        match self {
            Self::RawRgba => Some(width.saturating_mul(4)),
            Self::Png | Self::Jpeg => None,
        }
    }
}

pub(crate) struct DecodedAgentImage {
    pub(crate) image: RgbaImage,
    pub(crate) input_encoding: AgentImageEncoding,
}

pub(crate) fn decode_agent_image(
    resource: &AgentResource,
) -> Result<DecodedAgentImage, AgentPolicyError> {
    let metadata = resource
        .image
        .as_ref()
        .ok_or(AgentPolicyError::MissingImageMetadata)?;
    let bytes = resource
        .body
        .decoded_bytes()?
        .ok_or(AgentPolicyError::MissingImageBytes)?;
    match resource.mime_type.as_str() {
        "application/octet-stream" => {
            if metadata.pixel_format.as_deref() != Some("rgba8_unorm") {
                return Err(AgentPolicyError::UnsupportedImageEncoding(
                    resource.mime_type.clone(),
                ));
            }
            Ok(DecodedAgentImage {
                image: RgbaImage::new(metadata.width, metadata.height, bytes)?,
                input_encoding: AgentImageEncoding::RawRgba,
            })
        }
        "image/png" | "image/jpeg" => {
            let format = if resource.mime_type == "image/png" {
                ImageFormat::Png
            } else {
                ImageFormat::Jpeg
            };
            let image = decode_image_bytes(format, &bytes, ImageDecodeOptions::default())?;
            let frame = image
                .frames()
                .first()
                .ok_or(AgentPolicyError::MissingImageFrame)?;
            Ok(DecodedAgentImage {
                image: RgbaImage::new(
                    frame.dimensions().width(),
                    frame.dimensions().height(),
                    frame.rgba().to_vec(),
                )?,
                input_encoding: if resource.mime_type == "image/png" {
                    AgentImageEncoding::Png
                } else {
                    AgentImageEncoding::Jpeg
                },
            })
        }
        _ => Err(AgentPolicyError::UnsupportedImageEncoding(
            resource.mime_type.clone(),
        )),
    }
}

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, AgentPolicyError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, image.width(), image.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(image.pixels())?;
    }
    Ok(bytes)
}
