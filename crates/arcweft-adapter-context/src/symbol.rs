//! Language-free typed paths for non-callable adapter symbols.

use core::fmt;

use thiserror::Error;

/// One validated segment of an adapter symbol path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterSymbolSegment(String);

/// An ordered source-visible path for a non-callable adapter symbol.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterSymbolPath(Vec<AdapterSymbolSegment>);

/// Invalid adapter symbol path.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AdapterSymbolPathError {
    #[error("adapter symbol path must contain at least one typed segment")]
    Empty,
    #[error("adapter symbol segment must not be empty")]
    EmptySegment,
    #[error("invalid adapter symbol segment `{segment}`")]
    InvalidSegment { segment: String },
    #[error("adapter symbol path has invalid first segment `{segment}`")]
    InvalidImplicitRoot { segment: String },
}

impl AdapterSymbolSegment {
    /// Creates a non-empty segment containing letters, numbers, `_`, or `-`.
    pub fn try_new(value: impl Into<String>) -> Result<Self, AdapterSymbolPathError> {
        let value = value.into();
        if value.is_empty() {
            return Err(AdapterSymbolPathError::EmptySegment);
        }
        if !value
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-'))
        {
            return Err(AdapterSymbolPathError::InvalidSegment { segment: value });
        }
        Ok(Self(value))
    }

    /// Exact source-visible segment spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AdapterSymbolPath {
    /// Creates a non-empty implicit adapter path from validated segments.
    pub fn try_new(
        segments: impl IntoIterator<Item = AdapterSymbolSegment>,
    ) -> Result<Self, AdapterSymbolPathError> {
        let segments = segments.into_iter().collect::<Vec<_>>();
        let Some(first) = segments.first() else {
            return Err(AdapterSymbolPathError::Empty);
        };
        if !first
            .as_str()
            .chars()
            .next()
            .is_some_and(|character| character == '_' || character.is_alphabetic())
        {
            return Err(AdapterSymbolPathError::InvalidImplicitRoot {
                segment: first.as_str().to_owned(),
            });
        }
        Ok(Self(segments))
    }

    /// Ordered validated path segments.
    pub fn segments(&self) -> &[AdapterSymbolSegment] {
        &self.0
    }

    /// Final path segment.
    ///
    /// # Panics
    ///
    /// Panics only if the constructor invariant requiring at least one segment
    /// is violated inside this crate.
    pub fn last_segment(&self) -> &AdapterSymbolSegment {
        self.0
            .last()
            .expect("adapter symbol paths contain at least one segment")
    }
}

impl fmt::Display for AdapterSymbolSegment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for AdapterSymbolPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.0.iter().enumerate() {
            if index > 0 {
                formatter.write_str(".")?;
            }
            fmt::Display::fmt(segment, formatter)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AdapterSymbolPath, AdapterSymbolPathError, AdapterSymbolSegment};

    fn segment(value: &str) -> AdapterSymbolSegment {
        AdapterSymbolSegment::try_new(value).expect("test segment is valid")
    }

    #[test]
    fn adapter_symbol_path_validates_and_retains_segments() {
        let path = AdapterSymbolPath::try_new([segment("adapter"), segment("hero-pack")])
            .expect("qualified adapter path is valid");

        assert_eq!(
            path.segments()
                .iter()
                .map(AdapterSymbolSegment::as_str)
                .collect::<Vec<_>>(),
            ["adapter", "hero-pack"]
        );
        assert_eq!(path.last_segment().as_str(), "hero-pack");
        assert_eq!(path.to_string(), "adapter.hero-pack");
    }

    #[test]
    fn adapter_symbol_path_rejects_invalid_segments_and_roots() {
        assert_eq!(
            AdapterSymbolSegment::try_new(""),
            Err(AdapterSymbolPathError::EmptySegment)
        );
        for value in [
            "adapter.view",
            "adapter:view",
            "adapter/view",
            "adapter\\view",
            "\u{7}",
        ] {
            assert!(matches!(
                AdapterSymbolSegment::try_new(value),
                Err(AdapterSymbolPathError::InvalidSegment { .. })
            ));
        }
        assert_eq!(
            AdapterSymbolPath::try_new([]),
            Err(AdapterSymbolPathError::Empty)
        );
        assert!(matches!(
            AdapterSymbolPath::try_new([segment("2d"), segment("viewport")]),
            Err(AdapterSymbolPathError::InvalidImplicitRoot { segment }) if segment == "2d"
        ));
        assert!(
            AdapterSymbolPath::try_new([segment("adapter"), segment("2d")]).is_ok(),
            "only the implicit root segment has the leading-character restriction"
        );
    }
}
