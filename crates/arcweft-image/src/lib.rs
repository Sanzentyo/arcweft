//! Sans I/O decoded image and animation data for Arcweft presentation.

use image::metadata::LoopCount;
use image::{AnimationDecoder, DynamicImage, ImageFormat as ExternalImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use thiserror::Error;

/// Supported source image container formats.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    #[serde(rename = "webp")]
    WebP,
}

/// Decoded pixel format used by Arcweft renderer adapters.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImagePixelFormat {
    Rgba8,
}

/// Width and height in physical pixels.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ImageDimensions {
    width: u32,
    height: u32,
}

/// Looping policy carried by an animated image container.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageRepetition {
    Once,
    Infinite,
    Finite(u32),
}

/// Decode-time normalization for container frame delays.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ImageDecodeOptions {
    default_frame_duration_millis: u64,
    min_frame_duration_millis: u64,
}

/// One composited RGBA frame ready for renderer upload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DecodedImageFrame {
    index: u32,
    dimensions: ImageDimensions,
    duration_millis: u64,
    rgba: Vec<u8>,
}

/// Static or animated decoded image data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DecodedImage {
    format: ImageFormat,
    pixel_format: ImagePixelFormat,
    dimensions: ImageDimensions,
    repetition: ImageRepetition,
    frames: Vec<DecodedImageFrame>,
}

/// Error while decoding or constructing image data.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ImageError {
    #[error("image dimensions must be non-zero")]
    EmptyDimensions,
    #[error("decoded image has no frames")]
    EmptyFrames,
    #[error("frame {index} dimensions {frame:?} do not match image dimensions {image:?}")]
    FrameDimensionMismatch {
        index: u32,
        image: ImageDimensions,
        frame: ImageDimensions,
    },
    #[error("frame {index} RGBA data length {actual} does not match expected length {expected}")]
    InvalidRgbaLength {
        index: u32,
        expected: usize,
        actual: usize,
    },
    #[error("unsupported image format")]
    UnsupportedFormat,
    #[error("image decode failed: {message}")]
    Decode { message: String },
}

impl ImageDimensions {
    pub fn new(width: u32, height: u32) -> Result<Self, ImageError> {
        if width == 0 || height == 0 {
            return Err(ImageError::EmptyDimensions);
        }
        Ok(Self { width, height })
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }

    pub fn rgba_len(self) -> Option<usize> {
        let pixels = u64::from(self.width).checked_mul(u64::from(self.height))?;
        let bytes = pixels.checked_mul(4)?;
        usize::try_from(bytes).ok()
    }
}

impl ImageRepetition {
    pub const fn from_loop_count(loop_count: LoopCount) -> Self {
        match loop_count {
            LoopCount::Infinite => Self::Infinite,
            LoopCount::Finite(count) => Self::Finite(count.get()),
        }
    }
}

impl Default for ImageDecodeOptions {
    fn default() -> Self {
        Self {
            default_frame_duration_millis: 100,
            min_frame_duration_millis: 10,
        }
    }
}

impl ImageDecodeOptions {
    pub const fn new(default_frame_duration_millis: u64, min_frame_duration_millis: u64) -> Self {
        Self {
            default_frame_duration_millis,
            min_frame_duration_millis,
        }
    }

    pub const fn default_frame_duration_millis(self) -> u64 {
        self.default_frame_duration_millis
    }

    pub const fn min_frame_duration_millis(self) -> u64 {
        self.min_frame_duration_millis
    }

    fn normalize_duration(self, duration_millis: u64) -> u64 {
        if duration_millis == 0 {
            return self.default_frame_duration_millis;
        }
        duration_millis.max(self.min_frame_duration_millis)
    }
}

impl DecodedImageFrame {
    pub fn new(
        index: u32,
        dimensions: ImageDimensions,
        duration_millis: u64,
        rgba: Vec<u8>,
    ) -> Result<Self, ImageError> {
        validate_rgba_len(index, dimensions, rgba.len())?;
        Ok(Self {
            index,
            dimensions,
            duration_millis,
            rgba,
        })
    }

    pub const fn index(&self) -> u32 {
        self.index
    }

    pub const fn dimensions(&self) -> ImageDimensions {
        self.dimensions
    }

    pub const fn duration_millis(&self) -> u64 {
        self.duration_millis
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

impl DecodedImage {
    pub fn new(
        format: ImageFormat,
        dimensions: ImageDimensions,
        repetition: ImageRepetition,
        frames: Vec<DecodedImageFrame>,
    ) -> Result<Self, ImageError> {
        if frames.is_empty() {
            return Err(ImageError::EmptyFrames);
        }
        for frame in &frames {
            if frame.dimensions != dimensions {
                return Err(ImageError::FrameDimensionMismatch {
                    index: frame.index,
                    image: dimensions,
                    frame: frame.dimensions,
                });
            }
            validate_rgba_len(frame.index, frame.dimensions, frame.rgba.len())?;
        }
        Ok(Self {
            format,
            pixel_format: ImagePixelFormat::Rgba8,
            dimensions,
            repetition,
            frames,
        })
    }

    pub const fn format(&self) -> ImageFormat {
        self.format
    }

    pub const fn pixel_format(&self) -> ImagePixelFormat {
        self.pixel_format
    }

    pub const fn dimensions(&self) -> ImageDimensions {
        self.dimensions
    }

    pub const fn repetition(&self) -> ImageRepetition {
        self.repetition
    }

    pub fn frames(&self) -> &[DecodedImageFrame] {
        &self.frames
    }

    pub fn is_animated(&self) -> bool {
        self.frames.len() > 1
    }

    pub fn total_duration_millis(&self) -> u64 {
        self.frames
            .iter()
            .map(DecodedImageFrame::duration_millis)
            .sum()
    }

    pub fn frame_at_time_millis(&self, elapsed_millis: u64) -> Option<&DecodedImageFrame> {
        if self.frames.len() == 1 {
            return self.frames.first();
        }

        let total = self.total_duration_millis();
        if total == 0 {
            return self.frames.first();
        }

        let local_time = match self.repetition {
            ImageRepetition::Infinite => elapsed_millis % total,
            ImageRepetition::Once => elapsed_millis.min(total.saturating_sub(1)),
            ImageRepetition::Finite(count) => {
                let loops = u64::from(count.max(1));
                let max_time = total.saturating_mul(loops).saturating_sub(1);
                let clamped = elapsed_millis.min(max_time);
                clamped % total
            }
        };

        let mut cursor = 0_u64;
        self.frames
            .iter()
            .find(|frame| {
                cursor = cursor.saturating_add(frame.duration_millis());
                local_time < cursor
            })
            .or_else(|| self.frames.last())
    }
}

pub fn decode_image_bytes(
    format: ImageFormat,
    bytes: &[u8],
    options: ImageDecodeOptions,
) -> Result<DecodedImage, ImageError> {
    match format {
        ImageFormat::Png | ImageFormat::Jpeg => decode_static(format, bytes),
        ImageFormat::Gif => decode_gif(bytes, options),
        ImageFormat::WebP => decode_webp(bytes, options),
    }
}

fn decode_static(format: ImageFormat, bytes: &[u8]) -> Result<DecodedImage, ImageError> {
    let external = match format {
        ImageFormat::Png => ExternalImageFormat::Png,
        ImageFormat::Jpeg => ExternalImageFormat::Jpeg,
        ImageFormat::Gif | ImageFormat::WebP => return Err(ImageError::UnsupportedFormat),
    };
    let image = image::load_from_memory_with_format(bytes, external)
        .map_err(|error| decode_error(&error))?;
    decoded_static_from_dynamic(format, image)
}

fn decode_gif(bytes: &[u8], options: ImageDecodeOptions) -> Result<DecodedImage, ImageError> {
    let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes))
        .map_err(|error| decode_error(&error))?;
    let repetition = ImageRepetition::from_loop_count(decoder.loop_count());
    decode_animation(ImageFormat::Gif, repetition, decoder, options)
}

fn decode_webp(bytes: &[u8], options: ImageDecodeOptions) -> Result<DecodedImage, ImageError> {
    let decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))
        .map_err(|error| decode_error(&error))?;
    if decoder.has_animation() {
        let repetition = ImageRepetition::from_loop_count(decoder.loop_count());
        decode_animation(ImageFormat::WebP, repetition, decoder, options)
    } else {
        let image = ImageReader::with_format(Cursor::new(bytes), ExternalImageFormat::WebP)
            .decode()
            .map_err(|error| decode_error(&error))?;
        decoded_static_from_dynamic(ImageFormat::WebP, image)
    }
}

fn decode_animation<'a, D>(
    format: ImageFormat,
    repetition: ImageRepetition,
    decoder: D,
    options: ImageDecodeOptions,
) -> Result<DecodedImage, ImageError>
where
    D: AnimationDecoder<'a>,
{
    let frames = decoder
        .into_frames()
        .enumerate()
        .map(|(index, frame)| {
            let frame = frame.map_err(|error| decode_error(&error))?;
            let image = frame.buffer();
            let dimensions = ImageDimensions::new(image.width(), image.height())?;
            let duration = std::time::Duration::from(frame.delay());
            DecodedImageFrame::new(
                u32::try_from(index).map_err(|_| ImageError::Decode {
                    message: "too many image frames".to_owned(),
                })?,
                dimensions,
                options.normalize_duration(u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)),
                image.as_raw().clone(),
            )
        })
        .collect::<Result<Vec<_>, ImageError>>()?;

    let dimensions = frames
        .first()
        .map(DecodedImageFrame::dimensions)
        .ok_or(ImageError::EmptyFrames)?;
    DecodedImage::new(format, dimensions, repetition, frames)
}

fn decoded_static_from_dynamic(
    format: ImageFormat,
    image: DynamicImage,
) -> Result<DecodedImage, ImageError> {
    let rgba = image.into_rgba8();
    let dimensions = ImageDimensions::new(rgba.width(), rgba.height())?;
    let frame = DecodedImageFrame::new(0, dimensions, 0, rgba.into_raw())?;
    DecodedImage::new(format, dimensions, ImageRepetition::Once, vec![frame])
}

fn validate_rgba_len(
    index: u32,
    dimensions: ImageDimensions,
    actual: usize,
) -> Result<(), ImageError> {
    let expected = dimensions.rgba_len().ok_or(ImageError::InvalidRgbaLength {
        index,
        expected: usize::MAX,
        actual,
    })?;
    if actual == expected {
        Ok(())
    } else {
        Err(ImageError::InvalidRgbaLength {
            index,
            expected,
            actual,
        })
    }
}

fn decode_error(error: &image::ImageError) -> ImageError {
    ImageError::Decode {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame, RgbaImage};

    #[test]
    fn frame_selection_loops_animated_images() {
        let dimensions = ImageDimensions::new(1, 1).unwrap();
        let frames = vec![
            DecodedImageFrame::new(0, dimensions, 40, vec![255, 0, 0, 255]).unwrap(),
            DecodedImageFrame::new(1, dimensions, 60, vec![0, 255, 0, 255]).unwrap(),
        ];
        let image = DecodedImage::new(
            ImageFormat::Gif,
            dimensions,
            ImageRepetition::Infinite,
            frames,
        )
        .unwrap();

        assert_eq!(image.frame_at_time_millis(0).unwrap().index(), 0);
        assert_eq!(image.frame_at_time_millis(39).unwrap().index(), 0);
        assert_eq!(image.frame_at_time_millis(40).unwrap().index(), 1);
        assert_eq!(image.frame_at_time_millis(99).unwrap().index(), 1);
        assert_eq!(image.frame_at_time_millis(100).unwrap().index(), 0);
    }

    #[test]
    fn finite_animation_clamps_to_final_loop() {
        let dimensions = ImageDimensions::new(1, 1).unwrap();
        let frames = vec![
            DecodedImageFrame::new(0, dimensions, 10, vec![0, 0, 0, 255]).unwrap(),
            DecodedImageFrame::new(1, dimensions, 10, vec![255, 255, 255, 255]).unwrap(),
        ];
        let image = DecodedImage::new(
            ImageFormat::Gif,
            dimensions,
            ImageRepetition::Finite(2),
            frames,
        )
        .unwrap();

        assert_eq!(image.frame_at_time_millis(0).unwrap().index(), 0);
        assert_eq!(image.frame_at_time_millis(25).unwrap().index(), 0);
        assert_eq!(image.frame_at_time_millis(999).unwrap().index(), 1);
    }

    #[test]
    fn validates_rgba_frame_length() {
        let dimensions = ImageDimensions::new(2, 2).unwrap();

        assert_eq!(
            DecodedImageFrame::new(0, dimensions, 0, vec![0; 8]),
            Err(ImageError::InvalidRgbaLength {
                index: 0,
                expected: 16,
                actual: 8,
            })
        );
    }

    #[test]
    fn decodes_gif_animation_to_rgba_frames() {
        let mut bytes = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut bytes);
            encoder.set_repeat(Repeat::Infinite).unwrap();
            encoder
                .encode_frame(Frame::from_parts(
                    RgbaImage::from_raw(1, 1, vec![255, 0, 0, 255]).unwrap(),
                    0,
                    0,
                    Delay::from_numer_denom_ms(20, 1),
                ))
                .unwrap();
            encoder
                .encode_frame(Frame::from_parts(
                    RgbaImage::from_raw(1, 1, vec![0, 255, 0, 255]).unwrap(),
                    0,
                    0,
                    Delay::from_numer_denom_ms(30, 1),
                ))
                .unwrap();
        }

        let decoded =
            decode_image_bytes(ImageFormat::Gif, &bytes, ImageDecodeOptions::new(100, 10)).unwrap();

        assert!(decoded.is_animated());
        assert_eq!(decoded.repetition(), ImageRepetition::Infinite);
        assert_eq!(decoded.dimensions(), ImageDimensions::new(1, 1).unwrap());
        assert_eq!(decoded.frames().len(), 2);
        assert_eq!(decoded.frames()[0].duration_millis(), 20);
        assert_eq!(decoded.frames()[1].duration_millis(), 30);
        assert_eq!(decoded.frames()[0].rgba(), &[255, 0, 0, 255]);
        assert_eq!(decoded.frames()[1].rgba(), &[0, 255, 0, 255]);
        assert_eq!(decoded.frame_at_time_millis(49).unwrap().index(), 1);
        assert_eq!(decoded.frame_at_time_millis(50).unwrap().index(), 0);
    }

    #[test]
    fn image_format_serializes_webp_without_word_split() {
        assert_eq!(
            serde_json::to_value(ImageFormat::WebP).expect("format serializes"),
            "webp"
        );
    }
}
