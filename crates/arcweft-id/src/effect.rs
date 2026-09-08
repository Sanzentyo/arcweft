use core::{fmt, str::FromStr};

use thiserror::Error;

const CONTROL_SUSPEND_EFFECT_ID: &str = "control.suspend";

/// Canonical identity for one Arcweft effect capability.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectId(String);

/// One-way semantic identity of an already canonical effect capability.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectSemanticDigest([u8; 32]);

impl EffectSemanticDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Parse failure for a canonical effect identifier.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EffectIdError {
    #[error("effect identifier cannot be empty")]
    Empty,
    #[error("effect identifier `{value}` contains whitespace")]
    Whitespace { value: String },
    #[error("effect identifier `{value}` must contain at least two path segments")]
    MissingOperation { value: String },
    #[error("effect identifier `{value}` has an invalid path segment `{segment}`")]
    InvalidPathSegment { value: String, segment: String },
    #[error("effect identifier `{value}` has malformed scope parentheses")]
    MalformedScope { value: String },
    #[error("effect identifier `{value}` has an empty scope")]
    EmptyScope { value: String },
    #[error("effect identifier `{value}` has an invalid scope atom `{scope}`")]
    InvalidScopeAtom { value: String, scope: String },
}

impl EffectId {
    /// Canonical language-owned suspension capability.
    #[must_use]
    pub fn control_suspend() -> Self {
        Self(CONTROL_SUSPEND_EFFECT_ID.to_owned())
    }

    pub fn parse(value: impl AsRef<str>) -> Result<Self, EffectIdError> {
        value.as_ref().parse()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Hashes this parsed canonical identity without reparsing display text.
    ///
    /// # Panics
    ///
    /// Panics on a target whose address space can hold a string longer than
    /// the canonical `u64` transcript length.
    #[must_use]
    pub fn semantic_digest(&self) -> EffectSemanticDigest {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.lang.effect-semantic.v1\0");
        hasher.update(
            &u64::try_from(self.0.len())
                .expect("Rust string lengths fit the semantic u64 grammar")
                .to_le_bytes(),
        );
        hasher.update(self.0.as_bytes());
        EffectSemanticDigest(hasher.finalize().into())
    }

    /// Returns whether this is the canonical direct-style suspension effect.
    pub fn is_control_suspend(&self) -> bool {
        self.0 == CONTROL_SUSPEND_EFFECT_ID
    }

    pub fn family(&self) -> &str {
        self.0.split('.').next().unwrap_or_default()
    }

    pub fn path(&self) -> &str {
        self.0
            .split_once('(')
            .map_or(self.as_str(), |(path, _)| path)
    }

    pub fn scope_count(&self) -> usize {
        self.0.split_once('(').map_or(0, |(_, scopes)| {
            scopes[..scopes.len() - 1].split(',').count()
        })
    }

    pub fn is_in_namespace(&self, namespace: &str) -> bool {
        let path = self.path();
        path == namespace
            || path
                .strip_prefix(namespace)
                .is_some_and(|rest| rest.starts_with('.'))
    }

    pub fn covers(&self, required: &Self) -> bool {
        self == required
            || (self.path() == required.path()
                && (self.scope_count() == 0 || required.scope_count() == 0))
    }
}

impl FromStr for EffectId {
    type Err = EffectIdError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        if source.is_empty() {
            return Err(EffectIdError::Empty);
        }
        if source.trim() != source || source.chars().any(char::is_whitespace) {
            return Err(EffectIdError::Whitespace {
                value: source.to_owned(),
            });
        }

        let (path, scopes) = split_scope(source)?;
        let segments = path.split('.').collect::<Vec<_>>();
        if segments.len() < 2 {
            return Err(EffectIdError::MissingOperation {
                value: source.to_owned(),
            });
        }
        if let Some(segment) = segments
            .iter()
            .copied()
            .find(|segment| !valid_path_segment(segment))
        {
            return Err(EffectIdError::InvalidPathSegment {
                value: source.to_owned(),
                segment: segment.to_owned(),
            });
        }

        let canonical = scopes.map_or_else(
            || path.to_owned(),
            |scopes| format!("{path}({})", scopes.join(",")),
        );
        Ok(Self(canonical))
    }
}

impl fmt::Display for EffectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn split_scope(source: &str) -> Result<(&str, Option<Vec<&str>>), EffectIdError> {
    let has_open = source.contains('(');
    let has_close = source.contains(')');
    if !has_open && !has_close {
        return Ok((source, None));
    }
    if !source.ends_with(')')
        || source.matches('(').count() != 1
        || source.matches(')').count() != 1
    {
        return Err(EffectIdError::MalformedScope {
            value: source.to_owned(),
        });
    }
    let Some((path, scope_body)) = source[..source.len() - 1].split_once('(') else {
        return Err(EffectIdError::MalformedScope {
            value: source.to_owned(),
        });
    };
    if path.is_empty() || scope_body.is_empty() {
        return Err(EffectIdError::EmptyScope {
            value: source.to_owned(),
        });
    }
    let scopes = scope_body.split(',').collect::<Vec<_>>();
    if scopes.iter().any(|scope| scope.is_empty()) {
        return Err(EffectIdError::EmptyScope {
            value: source.to_owned(),
        });
    }
    if let Some(scope) = scopes
        .iter()
        .copied()
        .find(|scope| !valid_scope_atom(scope))
    {
        return Err(EffectIdError::InvalidScopeAtom {
            value: source.to_owned(),
            scope: scope.to_owned(),
        });
    }
    Ok((path, Some(scopes)))
}

fn valid_path_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn valid_scope_atom(scope: &str) -> bool {
    scope.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || matches!(character, '_' | '-' | '.' | '/' | ':' | '@' | '*' | '\'')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_canonicalizes_effect_ids() {
        assert_eq!(
            EffectId::parse("state.write(flow)")
                .expect("valid effect")
                .as_str(),
            "state.write(flow)"
        );
        assert_eq!(
            EffectId::parse("agent.act.semantic")
                .expect("valid effect")
                .family(),
            "agent"
        );
    }

    #[test]
    fn rejects_noncanonical_effect_ids() {
        assert!(EffectId::parse("read").is_err());
        assert!(EffectId::parse("Fs.read").is_err());
        assert!(EffectId::parse("fs.read( )").is_err());
        assert!(EffectId::parse(" fs.read").is_err());
        assert!(EffectId::parse("fs.read ").is_err());
        assert!(EffectId::parse("fs.read(save").is_err());
    }

    #[test]
    fn effect_coverage_matches_scoped_and_unscoped_path_bounds() {
        let read = EffectId::parse("fs.read").expect("valid effect");
        let read_save = EffectId::parse("fs.read(save)").expect("valid effect");
        let read_asset = EffectId::parse("fs.read(asset)").expect("valid effect");

        assert!(read.covers(&read_save));
        assert!(read_save.covers(&read));
        assert!(!read_save.covers(&read_asset));
        assert!(!read_asset.covers(&read_save));
    }

    #[test]
    fn effect_semantic_digest_is_canonical_and_payload_sensitive() {
        let first = EffectId::parse("fs.read(save)").expect("valid effect");
        let same = EffectId::parse("fs.read(save)").expect("same valid effect");
        let other_scope = EffectId::parse("fs.read(asset)").expect("valid effect");
        let other_path = EffectId::parse("fs.write(save)").expect("valid effect");

        assert_eq!(first.semantic_digest(), same.semantic_digest());
        assert_ne!(first.semantic_digest(), other_scope.semantic_digest());
        assert_ne!(first.semantic_digest(), other_path.semantic_digest());
    }
}
