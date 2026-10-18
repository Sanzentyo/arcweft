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
    /// Canonical engine-owned line scheduling effect label.
    pub const DIALOGUE_SCHEDULE_LABEL: &'static str = "dialogue.schedule";
    /// Canonical engine-owned dialogue voice effect label.
    pub const DIALOGUE_VOICE_LABEL: &'static str = "dialogue.voice";
    /// Canonical engine-owned log observation effect label.
    pub const LOG_WRITE_LABEL: &'static str = "log.write";
    /// Canonical engine-owned signal observation effect label.
    pub const SIGNAL_WRITE_LABEL: &'static str = "signal.write";
    /// Canonical engine-owned metric observation effect label.
    pub const METRIC_WRITE_LABEL: &'static str = "metric.write";
    /// Canonical engine-owned event observation effect label.
    pub const EVENT_EMIT_LABEL: &'static str = "event.emit";

    /// Canonical language-owned suspension capability.
    #[must_use]
    pub fn control_suspend() -> Self {
        Self(CONTROL_SUSPEND_EFFECT_ID.to_owned())
    }

    /// Canonical engine-owned line scheduling capability.
    #[must_use]
    pub fn dialogue_schedule() -> Self {
        Self(Self::DIALOGUE_SCHEDULE_LABEL.to_owned())
    }

    /// Canonical engine-owned dialogue voice capability.
    #[must_use]
    pub fn dialogue_voice() -> Self {
        Self(Self::DIALOGUE_VOICE_LABEL.to_owned())
    }

    /// Canonical engine-owned log observation capability.
    #[must_use]
    pub fn log_write() -> Self {
        Self(Self::LOG_WRITE_LABEL.to_owned())
    }

    /// Canonical engine-owned signal observation capability.
    #[must_use]
    pub fn signal_write() -> Self {
        Self(Self::SIGNAL_WRITE_LABEL.to_owned())
    }

    /// Canonical engine-owned metric observation capability.
    #[must_use]
    pub fn metric_write() -> Self {
        Self(Self::METRIC_WRITE_LABEL.to_owned())
    }

    /// Canonical engine-owned event observation capability.
    #[must_use]
    pub fn event_emit() -> Self {
        Self(Self::EVENT_EMIT_LABEL.to_owned())
    }

    pub fn parse(value: impl AsRef<str>) -> Result<Self, EffectIdError> {
        value.as_ref().parse()
    }

    /// Checks canonical source without retaining an owned identifier or
    /// allocating path/scope segment arrays. Decoders can validate all labels
    /// before reserving their aggregate storage. Errors retain source evidence.
    pub fn validate_source(source: &str) -> Result<(), EffectIdError> {
        if source.is_empty() {
            return Err(EffectIdError::Empty);
        }
        if source.trim() != source || source.chars().any(char::is_whitespace) {
            return Err(EffectIdError::Whitespace {
                value: source.to_owned(),
            });
        }
        let path = split_scope(source)?;
        let mut segments = path.split('.');
        if segments.clone().count() < 2 {
            return Err(EffectIdError::MissingOperation {
                value: source.to_owned(),
            });
        }
        if let Some(segment) = segments.find(|segment| !valid_path_segment(segment)) {
            return Err(EffectIdError::InvalidPathSegment {
                value: source.to_owned(),
                segment: segment.to_owned(),
            });
        }
        Ok(())
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
                && (self.scope_count() == 0
                    || required.scope_count() == 0
                    || (self.scope_count() == required.scope_count()
                        && self.scope_atoms().zip(required.scope_atoms()).all(
                            |(granted, needed)| {
                                granted == needed
                                    || needed
                                        .strip_prefix(granted)
                                        .is_some_and(|suffix| suffix.starts_with('.'))
                            },
                        ))))
    }

    fn scope_atoms(&self) -> impl Iterator<Item = &str> {
        self.0
            .split_once('(')
            .map_or("", |(_, scopes)| &scopes[..scopes.len() - 1])
            .split(',')
    }
}

impl FromStr for EffectId {
    type Err = EffectIdError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        Self::validate_source(source)?;
        Ok(Self(source.to_owned()))
    }
}

impl fmt::Display for EffectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn split_scope(source: &str) -> Result<&str, EffectIdError> {
    let has_open = source.contains('(');
    let has_close = source.contains(')');
    if !has_open && !has_close {
        return Ok(source);
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
    let mut scopes = scope_body.split(',');
    if scopes.clone().any(str::is_empty) {
        return Err(EffectIdError::EmptyScope {
            value: source.to_owned(),
        });
    }
    if let Some(scope) = scopes.find(|scope| !valid_scope_atom(scope)) {
        return Err(EffectIdError::InvalidScopeAtom {
            value: source.to_owned(),
            scope: scope.to_owned(),
        });
    }
    Ok(path)
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
    fn borrowed_source_validation_preserves_canonical_scoped_identity() {
        for source in [
            "fs.read",
            "fs.read(a)",
            "state.write(flow)",
            "agent.act.semantic",
            "fs.read(save,asset/path:part@owner-1,*,row')",
        ] {
            EffectId::validate_source(source).unwrap();
            assert_eq!(EffectId::parse(source).unwrap().as_str(), source);
        }
    }

    #[test]
    fn source_validation_and_owned_parse_reject_the_same_complete_grammar() {
        for source in [
            "",
            "read",
            "Fs.read",
            "fs..read",
            "1fs.read",
            "fs.read ",
            " fs.read",
            "fs.read( )",
            "fs.read()",
            "fs.read(a,)",
            "fs.read(,a)",
            "fs.read(a,,b)",
            "fs.read(a))",
            "fs.read((a)",
            "fs.read(a)b",
            "fs.read(a,b",
            "fs.read(é)",
            "fs.read(a+b)",
            "fs.read)a(",
            "fs.read\n",
        ] {
            let source_result = EffectId::validate_source(source);
            let owned_result = EffectId::parse(source).map(|_| ());
            assert_eq!(source_result, owned_result, "{source:?}");
            assert!(source_result.is_err(), "{source:?}");
        }
    }

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

    #[test]
    fn scoped_effect_coverage_respects_path_boundaries_and_argument_positions() {
        let root = EffectId::parse("state.write('flow)").expect("root grant");
        let flags = EffectId::parse("state.write('flow.flags)").expect("key grant");
        let seen = EffectId::parse("state.write('flow.flags.seen)").expect("required key");
        assert!(root.covers(&seen));
        assert!(flags.covers(&seen));
        assert!(!seen.covers(&flags));
        for other in [
            "state.write('flow.flagship)",
            "state.write('flowing.flags)",
            "state.write('scene.flags)",
            "state.read('flow.flags)",
        ] {
            assert!(
                !flags.covers(&EffectId::parse(other).expect("other capability")),
                "{other}"
            );
        }
        let pair = EffectId::parse("state.move('flow,'session.cache)").expect("pair grant");
        assert!(
            pair.covers(
                &EffectId::parse("state.move('flow.flags,'session.cache.slot)")
                    .expect("descendant pair")
            )
        );
        assert!(
            !pair.covers(
                &EffectId::parse("state.move('session.cache,'flow)").expect("reversed pair")
            )
        );
        assert!(!pair.covers(&root));
    }
}
