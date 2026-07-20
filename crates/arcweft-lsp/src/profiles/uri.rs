use lsp_types::Uri;
use std::path::{Path, PathBuf};

pub(crate) fn file_path_from_uri(uri: &lsp_types::Uri) -> Option<PathBuf> {
    if uri.scheme()?.as_str() != "file" {
        return None;
    }
    let path = percent_decode(uri.path().as_str())?;
    if let Some(authority) = uri.authority()
        && !authority.host().as_str().is_empty()
    {
        if authority.userinfo().is_some() || authority.port().is_some() {
            return None;
        }
        let server = percent_decode(authority.host().as_str())?;
        let share_path = path.strip_prefix('/')?;
        if server.is_empty() || share_path.is_empty() {
            return None;
        }
        return Some(PathBuf::from(format!(
            r"\\{server}\{}",
            share_path.replace('/', r"\")
        )));
    }
    Some(normalize_file_uri_path(&path))
}

pub(super) fn file_uri_from_path(path: &Path) -> Option<Uri> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let normalized = match normalized.strip_prefix("//?/") {
        Some(path) if is_drive_path(path) => path,
        Some(path) => return file_uri_from_unc_path(path.strip_prefix("UNC/")?),
        None => normalized.as_str(),
    };
    if let Some(path) = normalized.strip_prefix("//") {
        return file_uri_from_unc_path(path);
    }
    let body = if normalized
        .as_bytes()
        .get(1)
        .is_some_and(|byte| *byte == b':')
    {
        format!("/{normalized}")
    } else {
        normalized.to_owned()
    };
    format!("file://{}", percent_encode_file_path(&body))
        .parse()
        .ok()
}

fn file_uri_from_unc_path(path: &str) -> Option<Uri> {
    let (server, share_path) = path.split_once('/')?;
    if server.is_empty() || share_path.is_empty() {
        return None;
    }
    format!(
        "file://{}/{}",
        percent_encode_file_path(server),
        percent_encode_file_path(share_path)
    )
    .parse()
    .ok()
}

fn is_drive_path(path: &str) -> bool {
    path.as_bytes().get(1).is_some_and(|byte| *byte == b':')
}

fn normalize_file_uri_path(path: &str) -> PathBuf {
    let without_leading_windows_slash = path
        .strip_prefix('/')
        .filter(|rest| is_drive_path(rest))
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

#[cfg(test)]
mod tests {
    use super::{file_path_from_uri, file_uri_from_path};
    use std::path::{Path, PathBuf};

    #[test]
    fn windows_extended_drive_path_uses_the_client_file_uri_spelling() {
        let uri = file_uri_from_path(Path::new(r"\\?\C:\Project Files\src\main.arcw"))
            .expect("extended drive path is a file URI");

        assert_eq!(uri.as_str(), "file:///C:/Project%20Files/src/main.arcw");
        assert_eq!(
            file_path_from_uri(&uri),
            Some(PathBuf::from(r"C:\Project Files\src\main.arcw"))
        );
    }

    #[test]
    fn windows_extended_unc_path_remains_an_unc_authority() {
        let uri = file_uri_from_path(Path::new(r"\\?\UNC\server\share\src\main.arcw"))
            .expect("extended UNC path is a file URI");

        assert_eq!(uri.as_str(), "file://server/share/src/main.arcw");
        assert_eq!(
            file_path_from_uri(&uri),
            Some(PathBuf::from(r"\\server\share\src\main.arcw"))
        );
    }
}
