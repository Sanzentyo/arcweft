use crate::{
    ImageAlignment, ImageFit, ImagePlayback, LayoutBox, LayoutLength, LayoutPoint, LayoutSize,
    ViewImageSource, ViewImageSourceTable,
};
use arcweft_image::{
    DecodedImage, DecodedImageFrame, ImageDimensions, ImageFormat, ImageRepetition,
};

fn one_pixel_frame(index: u32, duration_millis: u64, rgba: [u8; 4]) -> DecodedImageFrame {
    DecodedImageFrame::new(
        index,
        ImageDimensions::new(1, 1).unwrap(),
        duration_millis,
        rgba.to_vec(),
    )
    .unwrap()
}

#[test]
fn image_source_table_resolves_static_and_animated_frames() {
    let dimensions = ImageDimensions::new(1, 1).unwrap();
    let static_image = DecodedImage::new(
        ImageFormat::Png,
        dimensions,
        ImageRepetition::Once,
        vec![one_pixel_frame(0, 0, [10, 20, 30, 255])],
    )
    .unwrap();
    let animated_image = DecodedImage::new(
        ImageFormat::Gif,
        dimensions,
        ImageRepetition::Infinite,
        vec![
            one_pixel_frame(0, 40, [255, 0, 0, 255]),
            one_pixel_frame(1, 60, [0, 255, 0, 255]),
        ],
    )
    .unwrap();
    let layout = LayoutBox::new(
        LayoutPoint::new(LayoutLength::px(0), LayoutLength::px(0)),
        LayoutSize::new(LayoutLength::px(100), LayoutLength::px(100)),
    );

    let mut table = ViewImageSourceTable::default();
    let static_id = table.insert(ViewImageSource::new(static_image)).unwrap();
    let animated_id = table
        .insert(
            ViewImageSource::new(animated_image)
                .with_fit(ImageFit::Cover)
                .with_alignment(ImageAlignment::top_left())
                .with_playback(ImagePlayback::new(1_000)),
        )
        .unwrap();

    let static_frame = table.resolve_frame(static_id, layout, 9_999).unwrap();
    assert_eq!(static_frame.frame().rgba(), &[10, 20, 30, 255]);
    assert_eq!(static_frame.fit(), ImageFit::Contain);

    let first = table.resolve_frame(animated_id, layout, 1_039).unwrap();
    assert_eq!(first.frame().index(), 0);
    assert_eq!(first.fit(), ImageFit::Cover);
    assert_eq!(first.alignment(), ImageAlignment::top_left());

    let second = table.resolve_frame(animated_id, layout, 1_040).unwrap();
    assert_eq!(second.frame().index(), 1);
}

#[test]
fn image_source_playback_can_pause_and_scale_time() {
    let dimensions = ImageDimensions::new(1, 1).unwrap();
    let animated_image = DecodedImage::new(
        ImageFormat::Gif,
        dimensions,
        ImageRepetition::Infinite,
        vec![
            one_pixel_frame(0, 100, [255, 0, 0, 255]),
            one_pixel_frame(1, 100, [0, 255, 0, 255]),
        ],
    )
    .unwrap();
    let layout = LayoutBox::new(
        LayoutPoint::new(LayoutLength::px(0), LayoutLength::px(0)),
        LayoutSize::new(LayoutLength::px(1), LayoutLength::px(1)),
    );
    let mut table = ViewImageSourceTable::default();
    let paused = table
        .insert(
            ViewImageSource::new(animated_image.clone())
                .with_playback(ImagePlayback::new(0).paused_at(150)),
        )
        .unwrap();
    let slow = table
        .insert(
            ViewImageSource::new(animated_image)
                .with_playback(ImagePlayback::new(0).with_rate_milli(500)),
        )
        .unwrap();

    assert_eq!(
        table
            .resolve_frame(paused, layout, 0)
            .unwrap()
            .frame()
            .index(),
        1
    );
    assert_eq!(
        table
            .resolve_frame(paused, layout, 10_000)
            .unwrap()
            .frame()
            .index(),
        1
    );
    assert_eq!(
        table
            .resolve_frame(slow, layout, 199)
            .unwrap()
            .frame()
            .index(),
        0
    );
    assert_eq!(
        table
            .resolve_frame(slow, layout, 200)
            .unwrap()
            .frame()
            .index(),
        1
    );
}
