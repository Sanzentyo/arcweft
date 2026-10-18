//! Semantic lifetime selectors and static registry keys, without runtime leases.

use crate::{DeclarationName, DeclarationNameError};

/// A semantic lifetime selector. Activation identity belongs to its runtime
/// owner; a selector alone never authorizes access to an active scope.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LifetimeScopeKind {
    Frame,
    Tick,
    Cue,
    Line,
    Scene,
    Flow,
    Session,
    Global,
    Persistent,
    Named(NamedLifetimeScope),
}

/// A validated named selector that cannot alias a builtin selector.
/// Construct it by parsing [`LifetimeScopeKind`].
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NamedLifetimeScope(DeclarationName);

impl NamedLifetimeScope {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl LifetimeScopeKind {
    pub fn parse(source: &str) -> Result<Self, DeclarationNameError> {
        DeclarationName::try_new(source).map(Self::from)
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Frame => "frame",
            Self::Tick => "tick",
            Self::Cue => "cue",
            Self::Line => "line",
            Self::Scene => "scene",
            Self::Flow => "flow",
            Self::Session => "session",
            Self::Global => "global",
            Self::Persistent => "persistent",
            Self::Named(name) => name.as_str(),
        }
    }

    /// Version-one semantic grammar tag, independent of Rust layout.
    pub const fn semantic_tag(&self) -> u8 {
        match self {
            Self::Frame => 0,
            Self::Tick => 1,
            Self::Cue => 2,
            Self::Line => 3,
            Self::Scene => 4,
            Self::Flow => 5,
            Self::Session => 6,
            Self::Global => 7,
            Self::Persistent => 8,
            Self::Named(_) => 9,
        }
    }
}

impl From<DeclarationName> for LifetimeScopeKind {
    fn from(name: DeclarationName) -> Self {
        match name.as_str() {
            "frame" => Self::Frame,
            "tick" => Self::Tick,
            "cue" => Self::Cue,
            "line" => Self::Line,
            "scene" => Self::Scene,
            "flow" => Self::Flow,
            "session" => Self::Session,
            "global" => Self::Global,
            "persistent" => Self::Persistent,
            _ => Self::Named(NamedLifetimeScope(name)),
        }
    }
}

/// A static key within a selected lifetime. The empty path denotes that scope's
/// root. Program identity, value schema and activation custody are separate
/// admission obligations; this key cannot grant any of them.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LifetimeRegistryKey {
    scope: LifetimeScopeKind,
    segments: Box<[DeclarationName]>,
}

impl LifetimeRegistryKey {
    pub const fn new(scope: LifetimeScopeKind, segments: Box<[DeclarationName]>) -> Self {
        Self { scope, segments }
    }

    pub const fn scope(&self) -> &LifetimeScopeKind {
        &self.scope
    }

    pub fn segments(&self) -> &[DeclarationName] {
        &self.segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_normalize_builtins_and_keep_named_identity_and_stable_tags() {
        for (tag, source) in [
            "frame",
            "tick",
            "cue",
            "line",
            "scene",
            "flow",
            "session",
            "global",
            "persistent",
        ]
        .into_iter()
        .enumerate()
        {
            let scope = LifetimeScopeKind::parse(source).unwrap();
            assert_eq!(scope.as_str(), source);
            assert_eq!(usize::from(scope.semantic_tag()), tag);
            assert!(!matches!(scope, LifetimeScopeKind::Named(_)));
            assert_eq!(
                scope,
                LifetimeScopeKind::from(DeclarationName::try_new(source).unwrap())
            );
        }
        let scope = LifetimeScopeKind::parse("inventory").unwrap();
        assert!(matches!(&scope, LifetimeScopeKind::Named(name) if name.as_str() == "inventory"));
        assert_eq!(scope.semantic_tag(), 9);
        for source in ["", "'flow", "flow.flags", "bad name", "bad\0name"] {
            assert!(LifetimeScopeKind::parse(source).is_err(), "{source:?}");
        }
    }

    #[test]
    fn registry_keys_preserve_scope_root_segment_boundaries_order_and_unicode() {
        let name = |value| DeclarationName::try_new(value).unwrap();
        let key = LifetimeRegistryKey::new(
            LifetimeScopeKind::Flow,
            Box::new([name("flags"), name("seen")]),
        );
        assert_eq!(key.scope(), &LifetimeScopeKind::Flow);
        assert_eq!(
            key.segments()
                .iter()
                .map(DeclarationName::as_str)
                .collect::<Vec<_>>(),
            ["flags", "seen"]
        );
        assert_ne!(
            key,
            LifetimeRegistryKey::new(
                LifetimeScopeKind::Flow,
                Box::new([name("seen"), name("flags")])
            )
        );
        assert_ne!(
            key,
            LifetimeRegistryKey::new(
                LifetimeScopeKind::Session,
                key.segments().to_vec().into_boxed_slice()
            )
        );
        assert!(
            LifetimeRegistryKey::new(LifetimeScopeKind::Flow, Box::new([]))
                .segments()
                .is_empty()
        );
        let unicode = LifetimeRegistryKey::new(
            LifetimeScopeKind::parse("所持品").unwrap(),
            Box::new([name("鍵"), name("取得済み")]),
        );
        assert_eq!(unicode.scope().as_str(), "所持品");
        assert_eq!(unicode.segments()[1].as_str(), "取得済み");
    }
}
