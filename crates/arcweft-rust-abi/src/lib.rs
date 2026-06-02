//! Typed metadata for Rust APIs exported to Arcweft adapters.
//!
//! This crate is data and codecs only. It does not inspect Rust source, run
//! Cargo, or perform file I/O; adapter crates and build scripts decide where
//! metadata is produced and stored.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current stable metadata schema version.
pub const ARCWEFT_RUST_ABI_SCHEMA_VERSION: u32 = 1;

/// Package-level metadata consumed by Arcweft sema, tooling, and LSP adapters.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustManifest {
    pub schema_version: u32,
    pub package: ArcweftRustPackage,
    #[serde(default)]
    pub types: Vec<ArcweftRustTypeDecl>,
    #[serde(default)]
    pub functions: Vec<ArcweftRustFunction>,
}

/// Builder for deterministic Rust ABI metadata manifests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArcweftRustManifestBuilder {
    manifest: ArcweftRustManifest,
}

/// Rust package identity for an Arcweft-aware adapter crate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustPackage {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub metadata_hash: Option<String>,
}

/// A Rust function exported into Arcweft's callable value space.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustFunction {
    pub name: String,
    pub rust_path: String,
    #[serde(default)]
    pub params: Vec<ArcweftRustParam>,
    pub return_type: ArcweftRustTypeRef,
    #[serde(default)]
    pub purity: ArcweftRustPurity,
    #[serde(default)]
    pub effects: Vec<String>,
}

/// One exported function parameter.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustParam {
    pub name: String,
    pub ty: ArcweftRustTypeRef,
}

/// Purity/effect class for a Rust export.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcweftRustPurity {
    #[default]
    External,
    Pure,
    Task,
}

/// A Rust struct, enum, or newtype exported as an Arcweft ADT.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustTypeDecl {
    pub name: String,
    pub rust_path: String,
    pub kind: ArcweftRustTypeKind,
}

/// Shape of an exported Rust ADT.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcweftRustTypeKind {
    Struct { fields: Vec<ArcweftRustField> },
    Enum { variants: Vec<ArcweftRustVariant> },
    Newtype { inner: ArcweftRustTypeRef },
}

/// One exported struct field.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustField {
    pub name: String,
    pub ty: ArcweftRustTypeRef,
}

/// One exported enum variant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustVariant {
    pub name: String,
    #[serde(default)]
    pub fields: Vec<ArcweftRustField>,
}

/// Arcweft type metadata generated from a Rust type.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcweftRustTypeRef {
    Unit,
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    F32,
    F64,
    String,
    Char,
    Vec {
        item: Box<ArcweftRustTypeRef>,
    },
    Seq {
        item: Box<ArcweftRustTypeRef>,
    },
    Option {
        item: Box<ArcweftRustTypeRef>,
    },
    Result {
        ok: Box<ArcweftRustTypeRef>,
        error: Box<ArcweftRustTypeRef>,
    },
    Tuple {
        items: Vec<ArcweftRustTypeRef>,
    },
    Named {
        name: String,
    },
}

/// Trait implemented by `#[derive(ArcweftType)]`.
pub trait ArcweftTypeMetadata {
    /// Returns the exported ADT declaration for this Rust type.
    fn arcweft_type_decl() -> ArcweftRustTypeDecl;
}

/// Trait used by export macros to lower Rust types into Arcweft metadata.
pub trait ArcweftType {
    /// Returns the Arcweft type reference for this Rust type.
    fn arcweft_type_ref() -> ArcweftRustTypeRef;
}

macro_rules! impl_primitive_type {
    ($ty:ty, $variant:expr) => {
        impl ArcweftType for $ty {
            fn arcweft_type_ref() -> ArcweftRustTypeRef {
                $variant
            }
        }
    };
}

impl_primitive_type!((), ArcweftRustTypeRef::Unit);
impl_primitive_type!(bool, ArcweftRustTypeRef::Bool);
impl_primitive_type!(i8, ArcweftRustTypeRef::I8);
impl_primitive_type!(i16, ArcweftRustTypeRef::I16);
impl_primitive_type!(i32, ArcweftRustTypeRef::I32);
impl_primitive_type!(i64, ArcweftRustTypeRef::I64);
impl_primitive_type!(i128, ArcweftRustTypeRef::I128);
impl_primitive_type!(isize, ArcweftRustTypeRef::ISize);
impl_primitive_type!(u8, ArcweftRustTypeRef::U8);
impl_primitive_type!(u16, ArcweftRustTypeRef::U16);
impl_primitive_type!(u32, ArcweftRustTypeRef::U32);
impl_primitive_type!(u64, ArcweftRustTypeRef::U64);
impl_primitive_type!(u128, ArcweftRustTypeRef::U128);
impl_primitive_type!(usize, ArcweftRustTypeRef::USize);
impl_primitive_type!(f32, ArcweftRustTypeRef::F32);
impl_primitive_type!(f64, ArcweftRustTypeRef::F64);
impl_primitive_type!(char, ArcweftRustTypeRef::Char);

impl ArcweftType for String {
    fn arcweft_type_ref() -> ArcweftRustTypeRef {
        ArcweftRustTypeRef::String
    }
}

impl ArcweftType for str {
    fn arcweft_type_ref() -> ArcweftRustTypeRef {
        ArcweftRustTypeRef::String
    }
}

impl<T: ArcweftType> ArcweftType for Vec<T> {
    fn arcweft_type_ref() -> ArcweftRustTypeRef {
        ArcweftRustTypeRef::Vec {
            item: Box::new(T::arcweft_type_ref()),
        }
    }
}

impl<T: ArcweftType> ArcweftType for Option<T> {
    fn arcweft_type_ref() -> ArcweftRustTypeRef {
        ArcweftRustTypeRef::Option {
            item: Box::new(T::arcweft_type_ref()),
        }
    }
}

impl<T: ArcweftType, E: ArcweftType> ArcweftType for Result<T, E> {
    fn arcweft_type_ref() -> ArcweftRustTypeRef {
        ArcweftRustTypeRef::Result {
            ok: Box::new(T::arcweft_type_ref()),
            error: Box::new(E::arcweft_type_ref()),
        }
    }
}

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

    /// Appends one exported type.
    #[must_use]
    pub fn with_type(mut self, ty: ArcweftRustTypeDecl) -> Self {
        self.types.push(ty);
        self
    }

    /// Appends one exported function.
    #[must_use]
    pub fn with_function(mut self, function: ArcweftRustFunction) -> Self {
        self.functions.push(function);
        self
    }

    /// Encodes this manifest as deterministic pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String, ArcweftRustAbiError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Decodes a manifest from JSON.
    pub fn from_json(source: &str) -> Result<Self, ArcweftRustAbiError> {
        let manifest = serde_json::from_str::<Self>(source)?;
        if manifest.schema_version != ARCWEFT_RUST_ABI_SCHEMA_VERSION {
            return Err(ArcweftRustAbiError::UnsupportedSchema {
                found: manifest.schema_version,
                expected: ARCWEFT_RUST_ABI_SCHEMA_VERSION,
            });
        }
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

    /// Finishes the manifest.
    pub fn build(self) -> ArcweftRustManifest {
        self.manifest
    }
}

/// ABI metadata codec errors.
#[derive(Debug, Error)]
pub enum ArcweftRustAbiError {
    #[error("failed to encode or decode Rust ABI metadata: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported Rust ABI metadata schema {found}, expected {expected}")]
    UnsupportedSchema { found: u32, expected: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip_preserves_function_and_type_metadata() {
        let manifest = ArcweftRustManifest::new(ArcweftRustPackage {
            name: "truck_game".to_owned(),
            version: "0.1.0".to_owned(),
            metadata_hash: None,
        })
        .with_type(ArcweftRustTypeDecl {
            name: "Rank".to_owned(),
            rust_path: "truck_game::Rank".to_owned(),
            kind: ArcweftRustTypeKind::Enum {
                variants: vec![ArcweftRustVariant {
                    name: "Gold".to_owned(),
                    fields: Vec::new(),
                }],
            },
        })
        .with_function(ArcweftRustFunction {
            name: "mini_games.truck.score_to_rank".to_owned(),
            rust_path: "truck_game::score_to_rank".to_owned(),
            params: vec![ArcweftRustParam {
                name: "score".to_owned(),
                ty: ArcweftRustTypeRef::I32,
            }],
            return_type: ArcweftRustTypeRef::Named {
                name: "Rank".to_owned(),
            },
            purity: ArcweftRustPurity::Pure,
            effects: Vec::new(),
        });

        let json = manifest.to_json_pretty().expect("json encodes");
        let decoded = ArcweftRustManifest::from_json(&json).expect("json decodes");

        assert_eq!(decoded, manifest);
        assert!(!json.contains("D:\\"));
        assert!(!json.contains("/tmp/"));
    }

    #[test]
    fn builder_collects_type_metadata_and_functions() {
        struct LocalType;

        impl ArcweftType for LocalType {
            fn arcweft_type_ref() -> ArcweftRustTypeRef {
                ArcweftRustTypeRef::Named {
                    name: "LocalType".to_owned(),
                }
            }
        }

        impl ArcweftTypeMetadata for LocalType {
            fn arcweft_type_decl() -> ArcweftRustTypeDecl {
                ArcweftRustTypeDecl {
                    name: "LocalType".to_owned(),
                    rust_path: "fixture::LocalType".to_owned(),
                    kind: ArcweftRustTypeKind::Struct { fields: Vec::new() },
                }
            }
        }

        let manifest = ArcweftRustManifest::builder(ArcweftRustPackage {
            name: "fixture".to_owned(),
            version: "0.1.0".to_owned(),
            metadata_hash: None,
        })
        .with_type_metadata::<LocalType>()
        .with_function(ArcweftRustFunction {
            name: "fixture.identity".to_owned(),
            rust_path: "fixture::identity".to_owned(),
            params: vec![ArcweftRustParam {
                name: "value".to_owned(),
                ty: LocalType::arcweft_type_ref(),
            }],
            return_type: LocalType::arcweft_type_ref(),
            purity: ArcweftRustPurity::Pure,
            effects: Vec::new(),
        })
        .build();

        assert_eq!(manifest.types[0].name, "LocalType");
        assert_eq!(manifest.functions[0].params[0].name, "value");
    }
}
