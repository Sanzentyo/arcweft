use super::*;
#[test]
fn wide_codec_lists_keep_cursor_frames_independent_of_child_count() {
    let codec = RuntimeCodecUse::Tuple {
        items: (0..20_000).map(|_| RuntimeCodecUse::Plain).collect(),
    };
    let mut events = CodecEvents::new(&codec);
    let mut count = 0;
    while events.next().is_some() {
        assert!(events.work.len() <= 2);
        count += 1;
    }
    assert_eq!(count, 40_001);
}
