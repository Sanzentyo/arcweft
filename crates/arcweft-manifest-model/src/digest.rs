use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, str::FromStr};
use thiserror::Error;

const PREFIX: &str = "blake3:";

/// BLAKE3 over exact external bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RawDigest([u8; 32]);

/// BLAKE3 derive-key digest over a canonical semantic projection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticDigest([u8; 32]);

/// Invalid canonical digest text.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DigestParseError {
    #[error("digest must be `blake3:` followed by exactly 64 lowercase hexadecimal digits")]
    InvalidText,
}

impl RawDigest {
    /// Hashes exact bytes without semantic normalization.
    pub fn for_bytes(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    /// Constructs a raw digest from already verified BLAKE3 bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl SemanticDigest {
    /// Hashes canonical semantic bytes with the exact Arcweft derive-key context.
    pub fn derive(context: &str, canonical_bytes: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new_derive_key(context);
        hasher.update(canonical_bytes);
        Self(*hasher.finalize().as_bytes())
    }

    /// Constructs a semantic digest from already verified derive-key bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

macro_rules! digest_impls {
    ($ty:ty) => {
        impl fmt::Display for $ty {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(PREFIX)?;
                for byte in self.0 {
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
        }

        impl FromStr for $ty {
            type Err = DigestParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                let Some(hex) = value.strip_prefix(PREFIX) else {
                    return Err(DigestParseError::InvalidText);
                };
                if hex.len() != 64
                    || !hex
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
                {
                    return Err(DigestParseError::InvalidText);
                }
                let mut bytes = [0_u8; 32];
                for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
                    bytes[index] = (hex_value(pair[0]) << 4) | hex_value(pair[1]);
                }
                Ok(Self(bytes))
            }
        }

        impl Serialize for $ty {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                String::deserialize(deserializer)?
                    .parse()
                    .map_err(de::Error::custom)
            }
        }
    };
}

digest_impls!(RawDigest);
digest_impls!(SemanticDigest);

const fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{DigestParseError, RawDigest, SemanticDigest};

    #[test]
    fn digest_text_is_exact_and_lowercase() {
        let digest = RawDigest::for_bytes(b"arcweft");
        let text = digest.to_string();
        assert_eq!(text.len(), 71);
        assert_eq!(text.parse::<RawDigest>().unwrap(), digest);
        assert_eq!(
            text.to_uppercase().parse::<RawDigest>(),
            Err(DigestParseError::InvalidText)
        );
    }

    #[test]
    fn semantic_context_changes_identity() {
        assert_ne!(
            SemanticDigest::derive("arcweft-a", b"{}"),
            SemanticDigest::derive("arcweft-b", b"{}")
        );
    }
}
