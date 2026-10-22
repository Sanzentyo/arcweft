use std::{fmt::Write as _, io::Write as _};

use crate::{DataError, DataErrorKind, EncodeOptions};

use super::EncodeBuffer;

#[test]
fn encoded_output_checks_the_entire_append_before_growth_or_copy() {
    let mut output = EncodeBuffer::new(8);
    assert_eq!(output.bytes.capacity(), 0);
    assert_eq!(
        output.append(&[42; 9]).unwrap_err().kind(),
        &DataErrorKind::LimitExceeded
    );
    assert!(output.bytes.is_empty());
    assert_eq!(output.bytes.capacity(), 0);
    assert!(output.append(&[]).is_err());
    assert!(output.finish(Ok(())).is_err());
}

#[test]
fn encoded_output_accumulates_sibling_writes_and_preserves_the_limit() {
    let mut output = EncodeBuffer::new(5);
    assert_eq!(output.remaining_len().unwrap(), 5);
    output.append(b"ab").unwrap();
    output.push(b'c').unwrap();
    output.append(b"de").unwrap();
    assert_eq!(output.remaining_len().unwrap(), 0);
    let capacity = output.bytes.capacity();
    assert!(capacity <= 5);
    assert!(output.push(b'f').is_err());
    assert_eq!(
        output.remaining_len().unwrap_err().kind(),
        &DataErrorKind::LimitExceeded
    );
    assert_eq!(output.bytes, b"abcde");
    assert_eq!(output.bytes.capacity(), capacity);
    assert!(output.flush().is_err());
}

#[test]
fn encoded_output_empty_and_exact_utf8_limits_publish_complete_bytes() {
    assert_eq!(
        EncodeOptions {
            max_output_len: 0,
            ..EncodeOptions::default()
        }
        .encode_with(|_| Ok(()))
        .unwrap(),
        b""
    );
    let options = EncodeOptions {
        max_output_len: 6,
        ..EncodeOptions::default()
    };
    assert_eq!(
        options
            .encode_with(|output| {
                output.write_str("蟹").unwrap();
                output.write_all("貝".as_bytes()).unwrap();
                Ok(())
            })
            .unwrap(),
        "蟹貝".as_bytes()
    );
}

#[test]
fn caught_io_and_formatter_failures_cannot_publish_success_or_replace_cause() {
    let options = EncodeOptions {
        max_output_len: 3,
        ..EncodeOptions::default()
    };
    for replace_error in [false, true] {
        let error = options
            .encode_with(|output| {
                output.write_all(b"abc").unwrap();
                assert!(output.write_str("d").is_err());
                assert!(output.write_all(b"e").is_err());
                if replace_error {
                    Err(DataError::unsupported("caught output error"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
    }
}

#[test]
fn encoder_refusal_never_publishes_a_partial_output() {
    let error = EncodeOptions::default()
        .encode_with(|output| {
            output.append(b"partial")?;
            Err(DataError::unsupported("producer refused"))
        })
        .unwrap_err();
    assert_eq!(error.kind(), &DataErrorKind::UnsupportedFormat);
    assert_eq!(EncodeBuffer::new(usize::MAX).bytes.capacity(), 0);
}
