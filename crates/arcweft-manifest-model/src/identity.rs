use semver::Version;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, str::FromStr};
use thiserror::Error;

/// Invalid typed identifier text.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("invalid {kind} `{value}`")]
pub struct IdentifierError {
    kind: &'static str,
    value: Box<str>,
}

macro_rules! string_id {
    ($name:ident, $kind:literal, $validator:ident) => {
        #[doc = concat!("Validated ", $kind, ".")]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Box<str>);

        impl $name {
            pub fn new(value: impl Into<Box<str>>) -> Result<Self, IdentifierError> {
                let value = value.into();
                if !$validator(&value) {
                    return Err(IdentifierError { kind: $kind, value });
                }
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
            type Err = IdentifierError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Self::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
            }
        }
    };
}

/// Lowercase reverse-domain package identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PackageId(Box<str>);

/// Exact semantic version selected by a manifest or generated module.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PackageVersion(Version);

string_id!(ProfileId, "profile ID", valid_dotted_lower);
string_id!(ContentUnitId, "content unit ID", valid_lower_kebab);
string_id!(
    ExternalModuleImportId,
    "external module import ID",
    valid_lower_kebab
);
string_id!(
    ActivityImplementationId,
    "Activity implementation ID",
    valid_lower_kebab
);
string_id!(AdapterProfileId, "adapter profile ID", valid_lower_kebab);
string_id!(AdapterExportId, "adapter export ID", valid_lower_snake);
string_id!(ExternalModuleId, "external module ID", valid_lower_snake);
string_id!(CapabilityId, "capability ID", valid_dotted_lower);
string_id!(ModuleMountPath, "module mount path", valid_dotted_lower);
string_id!(ActivityId, "Activity ID", valid_activity_id);
string_id!(AdapterTypeName, "adapter type name", valid_symbol);
string_id!(FunctionName, "adapter function name", valid_lower_snake);
string_id!(FieldName, "adapter field name", valid_lower_snake);
string_id!(GeneratorName, "metadata generator name", valid_lower_kebab);
string_id!(TargetTriple, "target triple", valid_target_triple);
string_id!(WitWorldId, "WIT world ID", valid_visible_text);
string_id!(TypeReference, "adapter type reference", valid_visible_text);

impl PackageId {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, IdentifierError> {
        let value = value.into();
        let valid = value.contains('.') && value.split('.').all(valid_lower_kebab);
        if !valid {
            return Err(IdentifierError {
                kind: "package ID",
                value,
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PackageVersion {
    pub fn new(value: impl AsRef<str>) -> Result<Self, semver::Error> {
        Version::parse(value.as_ref()).map(Self)
    }

    pub const fn version(&self) -> &Version {
        &self.0
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for PackageVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for PackageId {
    type Err = IdentifierError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl FromStr for PackageVersion {
    type Err = semver::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for PackageId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl Serialize for PackageVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for PackageId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for PackageVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

fn valid_lower_kebab(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.first().is_some_and(u8::is_ascii_lowercase_or_digit)
        && bytes.last().is_some_and(u8::is_ascii_lowercase_or_digit)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn valid_lower_snake(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
}

fn valid_dotted_lower(value: &str) -> bool {
    !value.is_empty()
        && value
            .split('.')
            .all(|segment| valid_lower_kebab(segment) || valid_lower_snake(segment))
}

fn valid_activity_id(value: &str) -> bool {
    value
        .strip_prefix("activity.")
        .is_some_and(valid_lower_snake)
}

fn valid_symbol(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn valid_target_triple(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
}

fn valid_visible_text(value: &str) -> bool {
    !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control)
}

trait AsciiIdByte {
    fn is_ascii_lowercase_or_digit(&self) -> bool;
}

impl AsciiIdByte for u8 {
    fn is_ascii_lowercase_or_digit(&self) -> bool {
        self.is_ascii_lowercase() || self.is_ascii_digit()
    }
}

#[cfg(test)]
mod tests {
    use super::{AdapterExportId, PackageId, PackageVersion};

    #[test]
    fn package_id_requires_canonical_reverse_domain_text() {
        assert!(PackageId::new("com.example.truck-game").is_ok());
        for invalid in ["example", "Com.example", "com..example", "com.-example"] {
            assert!(PackageId::new(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn package_version_is_exact_semver() {
        assert!(PackageVersion::new("1.2.3-alpha.1+build.7").is_ok());
        assert!(PackageVersion::new("^1.2").is_err());
        assert!(PackageVersion::new("1.2").is_err());
    }

    #[test]
    fn adapter_exports_use_canonical_snake_case() {
        assert!(AdapterExportId::new("truck_game").is_ok());
        assert!(AdapterExportId::new("TruckGame").is_err());
    }
}
