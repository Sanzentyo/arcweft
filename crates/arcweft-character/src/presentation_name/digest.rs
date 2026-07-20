//! Nominal BLAKE3 identities for catalog semantics and locale policy.

use core::{fmt, str::FromStr};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;

/// BLAKE3 identity of accepted Character display-name records.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterPresentationSemanticDigest([u8; 32]);

/// BLAKE3 identity of the accepted Character-name locale policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterPresentationLocalePolicyDigest([u8; 32]);

/// Invalid canonical lowercase digest text.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DigestParseError {
    #[error("digest must contain exactly 64 lowercase hexadecimal characters")]
    InvalidText,
}

macro_rules! impl_digest {
    ($ty:ty) => {
        impl $ty {
            /// Constructs a digest from already verified BLAKE3 bytes.
            #[must_use]
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            pub fn parse_lower_hex(value: &str) -> Result<Self, DigestParseError> {
                if value.len() != 64
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
                {
                    return Err(DigestParseError::InvalidText);
                }

                let mut bytes = [0_u8; 32];
                for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
                    bytes[index] = (hex_value(pair[0]) << 4) | hex_value(pair[1]);
                }
                Ok(Self(bytes))
            }

            #[must_use]
            pub fn to_lower_hex(self) -> String {
                let mut output = String::with_capacity(64);
                for byte in self.0 {
                    use core::fmt::Write as _;
                    write!(output, "{byte:02x}").expect("writing to String cannot fail");
                }
                output
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.to_lower_hex())
            }
        }

        impl FromStr for $ty {
            type Err = DigestParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse_lower_hex(value)
            }
        }

        impl Serialize for $ty {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.to_lower_hex())
            }
        }

        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse_lower_hex(&value).map_err(de::Error::custom)
            }
        }
    };
}

impl_digest!(CharacterPresentationSemanticDigest);
impl_digest!(CharacterPresentationLocalePolicyDigest);

const fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => 0,
    }
}
