use std::{collections::BTreeSet, fmt, iter::FromIterator, str::FromStr};

use thiserror::Error;

/// Canonical identity for one Arcweft effect capability.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectId(String);

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

/// Parse failure while constructing an effect set from source labels.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("invalid effect at index {index}: {source}")]
pub struct EffectSetParseError {
    index: usize,
    #[source]
    source: EffectIdError,
}

/// Deterministically ordered set of canonical effects.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectSet(BTreeSet<EffectId>);

impl EffectId {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, EffectIdError> {
        value.as_ref().parse()
    }

    pub fn as_str(&self) -> &str {
        &self.0
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

impl EffectSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_labels<I, S>(labels: I) -> Result<Self, EffectSetParseError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        labels
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                EffectId::parse(label).map_err(|source| EffectSetParseError { index, source })
            })
            .collect()
    }

    pub fn insert(&mut self, effect: EffectId) -> bool {
        self.0.insert(effect)
    }

    pub fn contains(&self, effect: &EffectId) -> bool {
        self.0.contains(effect)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &EffectId> + DoubleEndedIterator {
        self.0.iter()
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        self.0.is_subset(&other.0)
    }

    pub fn union_with(&mut self, other: &Self) -> bool {
        let previous_len = self.len();
        self.0.extend(other.iter().cloned());
        self.len() != previous_len
    }

    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        self.iter().chain(other.iter()).cloned().collect()
    }

    #[must_use]
    pub fn difference(&self, other: &Self) -> Self {
        self.0.difference(&other.0).cloned().collect()
    }

    #[must_use]
    pub fn intersection(&self, other: &Self) -> Self {
        self.0.intersection(&other.0).cloned().collect()
    }

    pub fn to_labels(&self) -> Vec<String> {
        self.iter().map(ToString::to_string).collect()
    }
}

impl FromIterator<EffectId> for EffectSet {
    fn from_iter<T: IntoIterator<Item = EffectId>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl IntoIterator for EffectSet {
    type Item = EffectId;
    type IntoIter = std::collections::btree_set::IntoIter<EffectId>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a EffectSet {
    type Item = &'a EffectId;
    type IntoIter = std::collections::btree_set::Iter<'a, EffectId>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl fmt::Display for EffectSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("{")?;
        for (index, effect) in self.iter().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{effect}")?;
        }
        formatter.write_str("}")
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
        && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
}

fn valid_scope_atom(scope: &str) -> bool {
    scope.chars().all(|char| {
        char.is_ascii_alphanumeric()
            || matches!(char, '_' | '-' | '.' | '/' | ':' | '@' | '*' | '\'')
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
    fn effect_sets_are_sorted_and_deduplicated() {
        let effects =
            EffectSet::from_labels(["ui.show", "fs.read", "ui.show"]).expect("valid effect set");
        assert_eq!(effects.to_labels(), vec!["fs.read", "ui.show"]);
    }
}
