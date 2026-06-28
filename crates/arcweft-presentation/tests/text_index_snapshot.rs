use arcweft_presentation::text_index::{TextIndexError, TextIndexSnapshot};
use arcweft_presentation::text_input::{TextByteOffset, TextRange, TextUtf16Offset};

#[test]
fn utf16_ranges_convert_to_canonical_byte_ranges() {
    let snapshot = TextIndexSnapshot::new("a😀b");

    let emoji = snapshot
        .byte_range_from_utf16(TextRange::new(TextUtf16Offset(1), TextUtf16Offset(3)))
        .expect("emoji range maps");

    assert_eq!(emoji, TextRange::new(TextByteOffset(1), TextByteOffset(5)));
    assert_eq!(snapshot.slice_byte_range(emoji).expect("slice"), "😀");
}

#[test]
fn utf16_offset_inside_surrogate_pair_is_rejected() {
    let snapshot = TextIndexSnapshot::new("a😀b");

    let error = snapshot
        .byte_range_from_utf16(TextRange::new(TextUtf16Offset(2), TextUtf16Offset(3)))
        .expect_err("mid-surrogate range rejects");

    assert_eq!(
        error,
        TextIndexError::Utf16OffsetInsideSurrogatePair {
            offset: TextUtf16Offset(2),
        }
    );
}

#[test]
fn byte_offset_inside_utf8_codepoint_is_rejected() {
    let snapshot = TextIndexSnapshot::new("é");

    let error = snapshot
        .utf16_offset_for_byte(TextByteOffset(1))
        .expect_err("mid-codepoint byte offset rejects");

    assert_eq!(
        error,
        TextIndexError::ByteOffsetInsideCodePoint {
            offset: TextByteOffset(1),
        }
    );
}
