//! AWBC-only product executable section model for AWFB products.
//!
//! This module is Sans I/O. It owns only typed data plus deterministic bytes over
//! `AwbcProgram::encode_canonical` / `AwbcProgram::decode_canonical`.

use crate::{BundleAwbcEncoding, BundleAwbcProgram, BundleCodecError};
use arcweft_core::awbc::codec::AwbcDecodeBudget;
use arcweft_core::awbc::schema::AwbcProgram;
use arcweft_core::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductExecutablePayload {
    AwbcV1,
}

impl ProductExecutablePayload {
    pub(crate) const fn wire_name(self) -> &'static str {
        match self {
            Self::AwbcV1 => "awbc_v1",
        }
    }

    pub(crate) fn from_wire_name(wire_name: &str) -> Option<Self> {
        if wire_name == Self::AwbcV1.wire_name() {
            Some(Self::AwbcV1)
        } else {
            None
        }
    }
}

impl Serialize for ProductExecutablePayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.wire_name())
    }
}

impl<'de> Deserialize<'de> for ProductExecutablePayload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire_name = String::deserialize(deserializer)?;
        Self::from_wire_name(&wire_name).ok_or_else(|| {
            D::Error::custom(format!(
                "unsupported product executable payload `{wire_name}`"
            ))
        })
    }
}

impl BundleAwbcProgram {
    pub fn new(program: AwbcProgram) -> Self {
        Self {
            encoding: BundleAwbcEncoding::AwbcV1,
            program,
        }
    }

    pub const fn program(&self) -> &AwbcProgram {
        &self.program
    }

    pub fn encode_product_section(&self) -> Result<Vec<u8>, BundleCodecError> {
        match self.encoding {
            BundleAwbcEncoding::AwbcV1 => self.program.encode_canonical().map_err(|error| {
                BundleCodecError::MalformedProductAwbcExecutable {
                    message: error.to_string(),
                }
            }),
        }
    }

    pub fn decode_product_section(bytes: &[u8]) -> Result<Self, BundleCodecError> {
        let program =
            AwbcProgram::decode_canonical(bytes, AwbcDecodeBudget::default()).map_err(|error| {
                BundleCodecError::MalformedProductAwbcExecutable {
                    message: error.to_string(),
                }
            })?;
        let executable = Self::new(program);
        executable.verify_product_executable()?;
        Ok(executable)
    }

    pub fn verify_product_executable(&self) -> Result<(), BundleCodecError> {
        self.program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .map_err(|error| BundleCodecError::ProductAwbcVerification {
                message: error.to_string(),
            })
    }
}
