use lsp_types::Uri;
use std::path::{Path, PathBuf};

pub(crate) fn file_path_from_uri(uri: &lsp_types::Uri) -> Option<PathBuf> {
    let raw = uri.as_str();
    let path = raw.strip_prefix("file://")?;
    let path = percent_decode(path)?;
    Some(normalize_file_uri_path(&path))
}

pub(super) fn file_uri_from_path(path: &Path) -> Option<Uri> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let body = if normalized
        .as_bytes()
        .get(1)
        .is_some_and(|byte| *byte == b':')
    {
        format!("/{normalized}")
    } else {
        normalized
    };
    format!("file://{}", percent_encode_file_path(&body))
        .parse()
        .ok()
}

fn normalize_file_uri_path(path: &str) -> PathBuf {
    let without_leading_windows_slash = path
        .strip_prefix('/')
        .filter(|rest| rest.as_bytes().get(1).is_some_and(|byte| *byte == b':'))
        .unwrap_or(path);
    let normalized = without_leading_windows_slash.replace('/', std::path::MAIN_SEPARATOR_STR);
    PathBuf::from(normalized)
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_value(*bytes.get(index + 1)?)?;
            let low = hex_value(*bytes.get(index + 2)?)?;
            output.push((high << 4) | low);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).ok()
}

fn percent_encode_file_path(input: &str) -> String {
    input
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                vec![char::from(byte)]
            }
            _ => {
                let mut encoded = ['%'; 3];
                encoded[1] = hex_digit(byte >> 4);
                encoded[2] = hex_digit(byte & 0x0f);
                encoded.to_vec()
            }
        })
        .collect()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        10..=15 => char::from(b'A' + value - 10),
        _ => '?',
    }
}
