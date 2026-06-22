use std::fs;

/// Commit 3/5 CI supplies raw RGBA captures and identical dimensions through
/// environment variables. The test is ignored locally but is a required
/// self-hosted WebGPU job once those capture producers are connected.
#[test]
#[ignore = "requires native and browser capture artifacts"]
fn native_and_web_captures_stay_within_the_approved_tolerance() {
    let native_path = std::env::var("ARCWEFT_NATIVE_RGBA")
        .expect("ARCWEFT_NATIVE_RGBA must point to the native raw RGBA capture");
    let web_path = std::env::var("ARCWEFT_WEB_RGBA")
        .expect("ARCWEFT_WEB_RGBA must point to the browser raw RGBA capture");
    let width: usize = std::env::var("ARCWEFT_CAPTURE_WIDTH")
        .expect("ARCWEFT_CAPTURE_WIDTH is required")
        .parse()
        .expect("capture width must be an integer");
    let height: usize = std::env::var("ARCWEFT_CAPTURE_HEIGHT")
        .expect("ARCWEFT_CAPTURE_HEIGHT is required")
        .parse()
        .expect("capture height must be an integer");
    let maximum_changed_pixel_ratio: f64 = std::env::var("ARCWEFT_VISUAL_MAX_CHANGED_RATIO")
        .unwrap_or_else(|_| "0.0025".to_owned())
        .parse()
        .expect("visual tolerance must be a number");

    let native = fs::read(native_path).expect("native capture is readable");
    let web = fs::read(web_path).expect("browser capture is readable");
    let expected_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .expect("capture dimensions fit usize");
    assert_eq!(native.len(), expected_len, "native RGBA byte length");
    assert_eq!(web.len(), expected_len, "browser RGBA byte length");

    let changed_pixels = native
        .chunks_exact(4)
        .zip(web.chunks_exact(4))
        .filter(|(left, right)| {
            left.iter()
                .zip(right.iter())
                .any(|(left, right)| left.abs_diff(*right) > 3)
        })
        .count();
    let pixel_count = u32::try_from(
        width
            .checked_mul(height)
            .expect("capture dimensions fit u32 pixel count"),
    )
    .expect("capture dimensions fit u32 pixel count");
    let changed_ratio = f64::from(u32::try_from(changed_pixels).unwrap_or(u32::MAX))
        / f64::from(pixel_count.max(1));
    assert!(
        changed_ratio <= maximum_changed_pixel_ratio,
        "native/Web changed pixel ratio {changed_ratio:.6} exceeded {maximum_changed_pixel_ratio:.6}"
    );
}
