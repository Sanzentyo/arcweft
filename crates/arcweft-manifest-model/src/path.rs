use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, path::Path, str::FromStr};
use thiserror::Error;

/// UTF-8, slash-normalized, project-relative path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NormalizedProjectPath(Box<str>);

/// Invalid lexical project path.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("invalid normalized project path `{value}`")]
pub struct NormalizedProjectPathError {
    value: Box<str>,
}

impl NormalizedProjectPath {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, NormalizedProjectPathError> {
        let value = value.into();
        let valid = !value.is_empty()
            && !value.starts_with('/')
            && !value.contains('\\')
            && !value.contains(':')
            && !value.chars().any(char::is_control)
            && value
                .split('/')
                .all(|segment| !segment.is_empty() && !matches!(segment, "." | ".."));
        if !valid {
            return Err(NormalizedProjectPathError { value });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_path(&self) -> &Path {
        Path::new(self.as_str())
    }
}

impl fmt::Display for NormalizedProjectPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for NormalizedProjectPath {
    type Err = NormalizedProjectPathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for NormalizedProjectPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for NormalizedProjectPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::NormalizedProjectPath;

    #[test]
    fn admits_only_portable_project_relative_paths() {
        assert!(NormalizedProjectPath::new("target/arcweft").is_ok());
        for invalid in ["", "/tmp", "C:/tmp", "a\\b", ".", "a/../b", "a//b"] {
            assert!(NormalizedProjectPath::new(invalid).is_err(), "{invalid}");
        }
    }
}
