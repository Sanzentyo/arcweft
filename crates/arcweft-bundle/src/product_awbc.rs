//! AWBC-only product executable section model for AWFB products.
//!
//! This module is Sans I/O. It owns only typed data plus deterministic bytes over
//! `AwbcProgram::encode_canonical` / `AwbcProgram::decode_canonical`.

use crate::{BundleAwbcEncoding, BundleAwbcProgram, BundleCodecError};
use arcweft_core::awbc::codec::AwbcDecodeBudget;
use arcweft_core::awbc::schema::AwbcProgram;
use arcweft_core::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};

pub const PRODUCT_EXECUTABLE_PAYLOAD_AWBC_V1: &str = "awbc_v1";

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
