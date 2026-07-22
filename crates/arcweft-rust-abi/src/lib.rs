//! Typed, Sans I/O metadata for Rust APIs exported to Arcweft adapters.
//!
//! The root is a deliberate data-model facade. Identity validation, recursive
//! model validation, and presentation formatting live in responsibility
//! modules; adapters own discovery and all filesystem or Cargo interaction.

mod display;
mod identity;
mod model;
mod validation;

pub use identity::{
    ArcweftRustIdentityError, ArcweftRustPackageId, ArcweftRustTypeParameterIndex,
    ArcweftRustTypeParameterName, ArcweftRustTypePath, ArcweftRustTypePathSegment,
};
pub use model::{
    ARCWEFT_RUST_ABI_SCHEMA_VERSION, ArcweftRustField, ArcweftRustFunction, ArcweftRustManifest,
    ArcweftRustManifestBuilder, ArcweftRustPackage, ArcweftRustParam, ArcweftRustPurity,
    ArcweftRustStructShape, ArcweftRustTypeDecl, ArcweftRustTypeKind, ArcweftRustTypeParameter,
    ArcweftRustTypeRef, ArcweftRustVariant, ArcweftRustVariantPayload, ArcweftType,
    ArcweftTypeMetadata,
};
pub use validation::{
    ArcweftRustAbiLimits, ArcweftRustManifestError, ArcweftRustTypeSite, ArcweftRustTypeSiteRoot,
    ArcweftRustTypeSiteStep,
};

use thiserror::Error;

impl ArcweftRustManifest {
    /// Creates an empty manifest for one package.
    pub fn new(package: ArcweftRustPackage) -> Self {
        Self {
            schema_version: ARCWEFT_RUST_ABI_SCHEMA_VERSION,
            package,
            types: Vec::new(),
            functions: Vec::new(),
        }
    }

    /// Starts a manifest builder for one package.
    pub fn builder(package: ArcweftRustPackage) -> ArcweftRustManifestBuilder {
        ArcweftRustManifestBuilder::new(package)
    }

    /// Appends one exported type declaration.
    #[must_use]
    pub fn with_type(mut self, ty: ArcweftRustTypeDecl) -> Self {
        self.types.push(ty);
        self
    }

    /// Appends one exported function declaration.
    #[must_use]
    pub fn with_function(mut self, function: ArcweftRustFunction) -> Self {
        self.functions.push(function);
        self
    }

    /// Encodes one validated manifest as deterministic pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String, ArcweftRustAbiError> {
        self.validate(ArcweftRustAbiLimits::PRODUCTION)?;
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Decodes, version-checks, and validates one final-schema manifest.
    pub fn from_json(source: &str) -> Result<Self, ArcweftRustAbiError> {
        let manifest = serde_json::from_str::<Self>(source)?;
        if manifest.schema_version != ARCWEFT_RUST_ABI_SCHEMA_VERSION {
            return Err(ArcweftRustAbiError::UnsupportedSchema {
                found: manifest.schema_version,
                expected: ARCWEFT_RUST_ABI_SCHEMA_VERSION,
            });
        }
        manifest.validate(ArcweftRustAbiLimits::PRODUCTION)?;
        Ok(manifest)
    }
}

impl ArcweftRustManifestBuilder {
    /// Creates an empty manifest builder for one package.
    pub fn new(package: ArcweftRustPackage) -> Self {
        Self {
            manifest: ArcweftRustManifest::new(package),
        }
    }

    /// Appends one exported type declaration.
    #[must_use]
    pub fn with_type(mut self, ty: ArcweftRustTypeDecl) -> Self {
        self.manifest.types.push(ty);
        self
    }

    /// Appends metadata for a Rust ADT implementing `ArcweftTypeMetadata`.
    #[must_use]
    pub fn with_type_metadata<T: ArcweftTypeMetadata>(self) -> Self {
        self.with_type(T::arcweft_type_decl())
    }

    /// Appends one exported function declaration.
    #[must_use]
    pub fn with_function(mut self, function: ArcweftRustFunction) -> Self {
        self.manifest.functions.push(function);
        self
    }

    /// Finishes the typed model. Call `validate` before registration.
    pub fn build(self) -> ArcweftRustManifest {
        self.manifest
    }
}

/// Rust ABI metadata codec and validation errors.
#[derive(Debug, Error)]
pub enum ArcweftRustAbiError {
    #[error("failed to encode or decode Rust ABI metadata: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported Rust ABI metadata schema {found}, expected {expected}")]
    UnsupportedSchema { found: u32, expected: u32 },
    #[error(transparent)]
    Manifest(#[from] ArcweftRustManifestError),
}

#[cfg(test)]
mod tests;
