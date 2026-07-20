use arcweft_id::{EntityId, PublicId};
use arcweft_manifest_model::PackageId;
use core::fmt;
use core::num::NonZeroU32;
use core::str::FromStr;
use thiserror::Error;

/// Resource identity category used by structured construction errors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceIdentityClass {
    ModulePath,
    TypeName,
    SchemaId,
    FieldId,
    FieldName,
    VariantId,
    VariantName,
    AssetPayloadKindId,
    CodecId,
    Family,
    FamilyGroupId,
    BundleSectionId,
    RuntimeHandleKindId,
    DescriptorSourceId,
    SchemaVersion,
    CodecVersion,
    BundleSectionVersion,
}

/// Why a stable resource identity could not be constructed.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceIdentityErrorKind {
    #[error("the value is empty")]
    Empty,
    #[error("the value is not in canonical form")]
    NonCanonical,
    #[error("numeric identity zero is reserved")]
    Zero,
}

/// Failure to construct one stable resource identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("invalid {class}: {kind}")]
pub struct ResourceIdentityError {
    class: ResourceIdentityClass,
    kind: ResourceIdentityErrorKind,
}

/// Canonical module path within one package.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceModulePath(Box<str>);

/// Canonical nominal type name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceTypeName(Box<str>);

/// Canonical package, module, and name identity for any nominal schema type.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NominalTypeId {
    package: PackageId,
    module: ResourceModulePath,
    name: ResourceTypeName,
}

/// Exact nominal identity of a registered configured resource type.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceTypeId(NominalTypeId);

macro_rules! stable_text_id {
    ($name:ident, $class:ident, $validator:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Box<str>);

        impl $name {
            pub fn try_new(value: impl Into<Box<str>>) -> Result<Self, ResourceIdentityError> {
                let value = value.into();
                validate_text(&value, ResourceIdentityClass::$class, $validator)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = ResourceIdentityError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::try_new(value)
            }
        }
    };
}

stable_text_id!(ResourceSchemaId, SchemaId, valid_stable_dotted_id);
stable_text_id!(ResourceFieldName, FieldName, valid_arcweft_identifier);
stable_text_id!(ResourceVariantName, VariantName, valid_arcweft_identifier);
stable_text_id!(
    ResourceAssetPayloadKindId,
    AssetPayloadKindId,
    valid_stable_dotted_id
);
stable_text_id!(ResourceCodecId, CodecId, valid_stable_dotted_id);
stable_text_id!(ResourceFamilyGroupId, FamilyGroupId, valid_stable_dotted_id);
stable_text_id!(
    ResourceBundleSectionId,
    BundleSectionId,
    valid_stable_dotted_id
);
stable_text_id!(
    ResourceRuntimeHandleKindId,
    RuntimeHandleKindId,
    valid_stable_dotted_id
);
stable_text_id!(
    ResourceDescriptorSourceId,
    DescriptorSourceId,
    valid_visible_identity
);

/// One canonical public-ID family segment supplied by a resource descriptor.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourcePublicIdFamily(Box<str>);

macro_rules! nonzero_u32_id {
    ($name:ident, $class:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(NonZeroU32);

        impl $name {
            pub fn try_new(value: u32) -> Result<Self, ResourceIdentityError> {
                NonZeroU32::new(value)
                    .map(Self)
                    .ok_or(ResourceIdentityError {
                        class: ResourceIdentityClass::$class,
                        kind: ResourceIdentityErrorKind::Zero,
                    })
            }

            pub const fn get(self) -> u32 {
                self.0.get()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.get().fmt(formatter)
            }
        }
    };
}

nonzero_u32_id!(ResourceFieldId, FieldId);
nonzero_u32_id!(ResourceVariantId, VariantId);
nonzero_u32_id!(ResourceSchemaVersion, SchemaVersion);
nonzero_u32_id!(ResourceCodecVersion, CodecVersion);
nonzero_u32_id!(ResourceBundleSectionVersion, BundleSectionVersion);

/// Stable semantic identity of one accepted resource declaration.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceDeclarationIdentity {
    entity: EntityId,
    public: PublicId,
    resource_type: ResourceTypeId,
}

impl ResourceIdentityClass {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ModulePath => "resource module path",
            Self::TypeName => "resource type name",
            Self::SchemaId => "resource schema ID",
            Self::FieldId => "resource field ID",
            Self::FieldName => "resource field name",
            Self::VariantId => "resource variant ID",
            Self::VariantName => "resource variant name",
            Self::AssetPayloadKindId => "resource asset payload-kind ID",
            Self::CodecId => "resource codec ID",
            Self::Family => "resource public-ID family",
            Self::FamilyGroupId => "resource family-group ID",
            Self::BundleSectionId => "resource bundle-section ID",
            Self::RuntimeHandleKindId => "resource runtime-handle-kind ID",
            Self::DescriptorSourceId => "resource descriptor source ID",
            Self::SchemaVersion => "resource schema version",
            Self::CodecVersion => "resource codec version",
            Self::BundleSectionVersion => "resource bundle-section version",
        }
    }
}

impl fmt::Display for ResourceIdentityClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

impl ResourceIdentityError {
    pub const fn class(&self) -> ResourceIdentityClass {
        self.class
    }

    pub const fn kind(&self) -> ResourceIdentityErrorKind {
        self.kind
    }
}

impl ResourceModulePath {
    pub fn try_new(value: impl Into<Box<str>>) -> Result<Self, ResourceIdentityError> {
        let value = value.into();
        validate_text(&value, ResourceIdentityClass::ModulePath, valid_module_path)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ResourceTypeName {
    pub fn try_new(value: impl Into<Box<str>>) -> Result<Self, ResourceIdentityError> {
        let value = value.into();
        validate_text(
            &value,
            ResourceIdentityClass::TypeName,
            valid_arcweft_identifier,
        )?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl NominalTypeId {
    pub const fn new(
        package: PackageId,
        module: ResourceModulePath,
        name: ResourceTypeName,
    ) -> Self {
        Self {
            package,
            module,
            name,
        }
    }

    pub const fn package(&self) -> &PackageId {
        &self.package
    }

    pub const fn module(&self) -> &ResourceModulePath {
        &self.module
    }

    pub const fn name(&self) -> &ResourceTypeName {
        &self.name
    }
}

impl ResourceTypeId {
    pub const fn new(nominal: NominalTypeId) -> Self {
        Self(nominal)
    }

    pub const fn nominal(&self) -> &NominalTypeId {
        &self.0
    }
}

impl ResourcePublicIdFamily {
    pub fn try_new(value: impl Into<Box<str>>) -> Result<Self, ResourceIdentityError> {
        let value = value.into();
        validate_text(&value, ResourceIdentityClass::Family, valid_family_segment)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ResourceDeclarationIdentity {
    pub const fn new(entity_id: EntityId, public_id: PublicId, type_id: ResourceTypeId) -> Self {
        Self {
            entity: entity_id,
            public: public_id,
            resource_type: type_id,
        }
    }

    pub const fn entity_id(&self) -> &EntityId {
        &self.entity
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.public
    }

    pub const fn type_id(&self) -> &ResourceTypeId {
        &self.resource_type
    }
}

impl fmt::Display for ResourceModulePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for ResourceTypeName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for NominalTypeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}::{}.{}", self.package, self.module, self.name)
    }
}

impl fmt::Display for ResourceTypeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl fmt::Display for ResourcePublicIdFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ResourceModulePath {
    type Err = ResourceIdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

impl FromStr for ResourceTypeName {
    type Err = ResourceIdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

impl FromStr for ResourcePublicIdFamily {
    type Err = ResourceIdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

fn validate_text(
    value: &str,
    class: ResourceIdentityClass,
    validator: fn(&str) -> bool,
) -> Result<(), ResourceIdentityError> {
    if value.is_empty() {
        return Err(ResourceIdentityError {
            class,
            kind: ResourceIdentityErrorKind::Empty,
        });
    }
    if !validator(value) {
        return Err(ResourceIdentityError {
            class,
            kind: ResourceIdentityErrorKind::NonCanonical,
        });
    }
    Ok(())
}

fn valid_module_path(value: &str) -> bool {
    value.split('.').all(valid_arcweft_identifier)
}

fn valid_arcweft_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_alphabetic())
        && characters.all(|character| {
            character == '_' || character.is_alphabetic() || character.is_ascii_digit()
        })
}

fn valid_stable_dotted_id(value: &str) -> bool {
    value.split('.').all(valid_stable_segment)
}

fn valid_stable_segment(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn valid_family_segment(value: &str) -> bool {
    !value.contains('.') && valid_stable_segment(value)
}

fn valid_visible_identity(value: &str) -> bool {
    value.trim() == value && !value.chars().any(char::is_control)
}
