use arcweft_audio_codec::{AudioResampler, CubicResampler};
use arcweft_audio_core::DecodedAudio;

#[test]
fn cubic_resampler_preserves_channel_shape() {
    let input = DecodedAudio::new(24_000, 1, vec![0.0, 1.0, 0.0, -1.0]).expect("input");
    let output = CubicResampler.resample(&input, 48_000).expect("resample");
    assert_eq!(output.sample_rate_hz(), 48_000);
    assert_eq!(output.channels(), 1);
    assert_eq!(output.frame_count(), 8);
}
