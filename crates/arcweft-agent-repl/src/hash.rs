pub(crate) fn stable_hash(label: &str, bytes: impl AsRef<[u8]>) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(label.as_bytes());
    hasher.update(bytes.as_ref());
    format!("blake3:{}", hasher.finalize().to_hex())
}

pub(crate) fn hash_parts(label: &str, parts: impl IntoIterator<Item = String>) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(label.as_bytes());
    for part in parts {
        let bytes = part.as_bytes();
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    format!("blake3:{}", hasher.finalize().to_hex())
}
