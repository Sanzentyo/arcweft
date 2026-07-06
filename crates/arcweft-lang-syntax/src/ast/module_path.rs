//! Typed Arcweft module paths shared by syntax, project loading, HIR, and tooling.

use core::{fmt, str::FromStr};
use thiserror::Error;

/// Root spelling accepted by Arcweft module paths.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ModulePathRoot {
    /// An unqualified path. Arcweft treats this as crate-rooted.
    ImplicitCrate,
    Crate,
    SelfModule,
    Super(usize),
}

/// One validated module path segment.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModuleSegment(String);

/// Parsed source spelling such as `crate.game.routes` or `super.shared`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModulePath {
    root: ModulePathRoot,
    segments: Vec<ModuleSegment>,
}

/// Absolute package-local module identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalModulePath {
    segments: Vec<ModuleSegment>,
}

/// Module path parse and resolution failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ModulePathError {
    #[error("module path is empty")]
    Empty,
    #[error("module path `{path}` contains an empty segment")]
    EmptySegment { path: String },
    #[error("`{segment}` is not a valid module path segment")]
    InvalidSegment { segment: String },
    #[error("module path `{path}` walks {levels} level(s) above the crate root from `{base}`")]
    EscapesCrate {
        path: String,
        base: String,
        levels: usize,
    },
}

impl ModulePathRoot {
    /// Whether this spelling starts at the crate root.
    pub const fn is_crate_rooted(self) -> bool {
        matches!(self, Self::ImplicitCrate | Self::Crate)
    }

    /// Number of parent levels requested by this root spelling.
    pub const fn super_levels(self) -> usize {
        match self {
            Self::Super(levels) => levels,
            Self::ImplicitCrate | Self::Crate | Self::SelfModule => 0,
        }
    }
}

impl ModuleSegment {
    /// Creates a validated identifier-like module segment.
    pub fn new(value: impl Into<String>) -> Result<Self, ModulePathError> {
        let value = value.into();
        let mut chars = value.chars();
        let Some(first) = chars.next() else {
            return Err(ModulePathError::InvalidSegment { segment: value });
        };
        if !(first == '_' || first.is_alphabetic())
            || !chars.all(|character| character == '_' || character.is_alphanumeric())
        {
            return Err(ModulePathError::InvalidSegment { segment: value });
        }
        Ok(Self(value))
    }

    /// Source spelling without separators.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ModulePath {
    /// Creates a parsed path from an explicit root and validated segments.
    pub fn new(
        root: ModulePathRoot,
        segments: impl IntoIterator<Item = ModuleSegment>,
    ) -> Result<Self, ModulePathError> {
        let segments = segments.into_iter().collect::<Vec<_>>();
        if segments.is_empty() && matches!(root, ModulePathRoot::ImplicitCrate) {
            return Err(ModulePathError::Empty);
        }
        Ok(Self { root, segments })
    }

    /// Root behavior selected by the source spelling.
    pub const fn root(&self) -> ModulePathRoot {
        self.root
    }

    /// Validated path segments after the root spelling.
    pub fn segments(&self) -> &[ModuleSegment] {
        &self.segments
    }

    /// Last path segment, when present.
    pub fn last_segment(&self) -> Option<&str> {
        self.segments.last().map(ModuleSegment::as_str)
    }

    /// Resolves this spelling against the current module.
    pub fn resolve_from(
        &self,
        current: &CanonicalModulePath,
    ) -> Result<CanonicalModulePath, ModulePathError> {
        let mut segments = match self.root {
            ModulePathRoot::ImplicitCrate | ModulePathRoot::Crate => Vec::new(),
            ModulePathRoot::SelfModule => current.segments.clone(),
            ModulePathRoot::Super(levels) => {
                if levels > current.segments.len() {
                    return Err(ModulePathError::EscapesCrate {
                        path: self.to_string(),
                        base: current.to_string(),
                        levels,
                    });
                }
                current.segments[..current.segments.len() - levels].to_vec()
            }
        };
        segments.extend(self.segments.iter().cloned());
        Ok(CanonicalModulePath { segments })
    }

    /// Resolves a file-level `mod` declaration against its inferred parent.
    pub fn resolve_declaration_for(
        &self,
        inferred_module: &CanonicalModulePath,
    ) -> Result<CanonicalModulePath, ModulePathError> {
        let base = inferred_module
            .parent()
            .unwrap_or_else(CanonicalModulePath::crate_root);
        self.resolve_from(&base)
    }
}

impl CanonicalModulePath {
    /// Package-local root module.
    pub const fn crate_root() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Creates an absolute path from validated segments.
    pub fn from_segments(segments: impl IntoIterator<Item = ModuleSegment>) -> Self {
        Self {
            segments: segments.into_iter().collect(),
        }
    }

    /// Absolute path segments after `crate`.
    pub fn segments(&self) -> &[ModuleSegment] {
        &self.segments
    }

    /// Whether this identity is the package root module.
    pub fn is_crate_root(&self) -> bool {
        self.segments.is_empty()
    }

    /// Last path segment, when present.
    pub fn last_segment(&self) -> Option<&str> {
        self.segments.last().map(ModuleSegment::as_str)
    }

    /// Parent module, or `None` for the crate root.
    pub fn parent(&self) -> Option<Self> {
        (!self.segments.is_empty()).then(|| Self {
            segments: self.segments[..self.segments.len() - 1].to_vec(),
        })
    }

    /// Child module identity.
    #[must_use]
    pub fn join(&self, segment: ModuleSegment) -> Self {
        let mut segments = self.segments.clone();
        segments.push(segment);
        Self { segments }
    }

    /// Returns this module followed by its parents, ending at the crate root.
    pub fn ancestors_inclusive(&self) -> impl Iterator<Item = Self> + '_ {
        (0..=self.segments.len()).rev().map(|length| Self {
            segments: self.segments[..length].to_vec(),
        })
    }
}

impl FromStr for ModulePath {
    type Err = ModulePathError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let source = source.trim();
        if source.is_empty() {
            return Err(ModulePathError::Empty);
        }
        let raw = source.split('.').collect::<Vec<_>>();
        if raw.iter().any(|segment| segment.is_empty()) {
            return Err(ModulePathError::EmptySegment {
                path: source.to_owned(),
            });
        }

        let (root, start) = match raw.as_slice() {
            ["crate", ..] => (ModulePathRoot::Crate, 1),
            ["self", ..] => (ModulePathRoot::SelfModule, 1),
            ["parent", ..] => (ModulePathRoot::Super(1), 1),
            ["super", ..] => {
                let levels = raw
                    .iter()
                    .take_while(|segment| **segment == "super")
                    .count();
                (ModulePathRoot::Super(levels), levels)
            }
            _ => (ModulePathRoot::ImplicitCrate, 0),
        };
        let segments = raw[start..]
            .iter()
            .map(|segment| ModuleSegment::new((*segment).to_owned()))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(root, segments)
    }
}

impl fmt::Display for ModuleSegment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for ModulePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.root {
            ModulePathRoot::ImplicitCrate => {}
            ModulePathRoot::Crate => formatter.write_str("crate")?,
            ModulePathRoot::SelfModule => formatter.write_str("self")?,
            ModulePathRoot::Super(levels) => {
                for index in 0..levels {
                    if index > 0 {
                        formatter.write_str(".")?;
                    }
                    formatter.write_str("super")?;
                }
            }
        }
        for (index, segment) in self.segments.iter().enumerate() {
            if index > 0 || !matches!(self.root, ModulePathRoot::ImplicitCrate) {
                formatter.write_str(".")?;
            }
            fmt::Display::fmt(segment, formatter)?;
        }
        Ok(())
    }
}

impl fmt::Display for CanonicalModulePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("crate")?;
        for segment in &self.segments {
            formatter.write_str(".")?;
            fmt::Display::fmt(segment, formatter)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{CanonicalModulePath, ModulePath, ModulePathError, ModuleSegment};

    #[test]
    fn resolves_crate_self_and_super_paths() {
        let current = CanonicalModulePath::from_segments([
            ModuleSegment::new("game").unwrap(),
            ModuleSegment::new("routes").unwrap(),
        ]);
        assert_eq!(
            "crate.shared"
                .parse::<ModulePath>()
                .unwrap()
                .resolve_from(&current)
                .unwrap()
                .to_string(),
            "crate.shared"
        );
        assert_eq!(
            "self.opening"
                .parse::<ModulePath>()
                .unwrap()
                .resolve_from(&current)
                .unwrap()
                .to_string(),
            "crate.game.routes.opening"
        );
        assert_eq!(
            "super.logic"
                .parse::<ModulePath>()
                .unwrap()
                .resolve_from(&current)
                .unwrap()
                .to_string(),
            "crate.game.logic"
        );
    }

    #[test]
    fn rejects_paths_that_escape_the_crate() {
        let current = CanonicalModulePath::crate_root();
        assert!(matches!(
            "super.shared"
                .parse::<ModulePath>()
                .unwrap()
                .resolve_from(&current),
            Err(ModulePathError::EscapesCrate { .. })
        ));
    }
}
