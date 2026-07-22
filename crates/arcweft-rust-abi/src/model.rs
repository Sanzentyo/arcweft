use crate::identity::{
    ArcweftRustPackageId, ArcweftRustTypeParameterIndex, ArcweftRustTypeParameterName,
    ArcweftRustTypePath,
};
use serde::{Deserialize, Serialize};

/// Current Rust ABI metadata schema version.
pub const ARCWEFT_RUST_ABI_SCHEMA_VERSION: u32 = 1;

/// Package-level metadata consumed by Arcweft registration and tooling.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustManifest {
    pub schema_version: u32,
    pub package: ArcweftRustPackage,
    #[serde(default)]
    pub types: Vec<ArcweftRustTypeDecl>,
    #[serde(default)]
    pub functions: Vec<ArcweftRustFunction>,
}

/// Builder for deterministic Rust ABI manifests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArcweftRustManifestBuilder {
    pub(crate) manifest: ArcweftRustManifest,
}

/// Provenance for one Rust package that publishes Arcweft metadata.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ArcweftRustPackage {
    pub id: ArcweftRustPackageId,
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
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcweftRustPurity {
    #[default]
    External,
    Pure,
    Task,
}

/// One generic type parameter owned by an exported Rust ADT.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustTypeParameter {
    pub index: ArcweftRustTypeParameterIndex,
    pub name: ArcweftRustTypeParameterName,
}

/// A Rust struct, enum, or newtype exported as an Arcweft nominal declaration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustTypeDecl {
    pub path: ArcweftRustTypePath,
    pub rust_path: String,
    #[serde(default)]
    pub parameters: Vec<ArcweftRustTypeParameter>,
    pub kind: ArcweftRustTypeKind,
}

/// Shape of an exported Rust ADT.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcweftRustTypeKind {
    Struct { shape: ArcweftRustStructShape },
    Enum { variants: Vec<ArcweftRustVariant> },
    Newtype { inner: ArcweftRustTypeRef },
}

/// Unit, tuple, or named-field shape of an exported Rust struct.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcweftRustStructShape {
    Unit,
    Tuple { fields: Vec<ArcweftRustTypeRef> },
    Record { fields: Vec<ArcweftRustField> },
}

/// One exported record field.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustField {
    pub name: String,
    pub ty: ArcweftRustTypeRef,
}

/// One exported enum variant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustVariant {
    pub name: String,
    pub payload: ArcweftRustVariantPayload,
}

/// Unit, tuple, or named-field payload of an exported Rust enum variant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcweftRustVariantPayload {
    Unit,
    Tuple { fields: Vec<ArcweftRustTypeRef> },
    Record { fields: Vec<ArcweftRustField> },
}

/// Arcweft type metadata generated from a Rust type.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
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
    Nominal {
        package: ArcweftRustPackageId,
        path: ArcweftRustTypePath,
        #[serde(default)]
        arguments: Vec<ArcweftRustTypeRef>,
    },
    TypeParameter {
        index: ArcweftRustTypeParameterIndex,
    },
}

/// Trait implemented by `#[derive(ArcweftType)]`.
pub trait ArcweftTypeMetadata {
    /// Returns the exported ADT declaration for this Rust type.
    fn arcweft_type_decl() -> ArcweftRustTypeDecl;
}

/// Trait used by export macros to lower Rust types into Arcweft metadata.
pub trait ArcweftType {
    /// Returns the exact Arcweft type reference for this Rust type.
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

macro_rules! impl_tuple_type {
    ($($name:ident),+ $(,)?) => {
        impl<$($name: ArcweftType),+> ArcweftType for ($($name,)+) {
            fn arcweft_type_ref() -> ArcweftRustTypeRef {
                ArcweftRustTypeRef::Tuple {
                    items: vec![$($name::arcweft_type_ref()),+],
                }
            }
        }
    };
}

impl_tuple_type!(A);
impl_tuple_type!(A, B);
impl_tuple_type!(A, B, C);
impl_tuple_type!(A, B, C, D);
impl_tuple_type!(A, B, C, D, E);
impl_tuple_type!(A, B, C, D, E, F);
impl_tuple_type!(A, B, C, D, E, F, G);
impl_tuple_type!(A, B, C, D, E, F, G, H);
