use std::{
    collections::{BTreeMap, btree_map::Entry},
    fmt,
};

use thiserror::Error;

use crate::effects::{EffectId, EffectSet};

/// Stable semantic identity of a callable node in the effect graph.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableId(String);

/// Source-level callable family used for policy and diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableKind {
    Function,
    Flow,
    Fragment,
    Agent,
    Hook,
    Entry,
    Source,
    Parser,
    Memo,
    Extern,
    Intrinsic,
}

/// Visibility/security boundary of a callable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Private,
    Public,
    Boundary,
}

/// Source anchor attached to an effect or call edge.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectSite {
    label: String,
    path: Option<String>,
    line: Option<u32>,
    column: Option<u32>,
}

/// One direct effect use in a callable body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectUse {
    effect: EffectId,
    site: EffectSite,
}

/// Effect contract attached to a callable.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectContract {
    /// `None` means omitted/inferred; `Some(empty)` means explicitly pure upper bound.
    upper_bound: Option<EffectSet>,
    forbidden: EffectSet,
    pure: bool,
    require_explicit_nonempty: bool,
}

/// External callable with a trusted or separately verified effect summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalCallable {
    name: String,
    effects: EffectSet,
}

/// Target of one call/control-transfer edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallTarget {
    Local(CallableId),
    External(ExternalCallable),
    Dynamic {
        label: String,
        effects: Option<EffectSet>,
    },
}

/// One edge in the callable effect graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallEdge {
    target: CallTarget,
    site: EffectSite,
}

/// Facts collected while the normal semantic checker traverses one callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableFacts {
    id: CallableId,
    kind: CallableKind,
    visibility: Visibility,
    contract: EffectContract,
    direct_effects: Vec<EffectUse>,
    calls: Vec<CallEdge>,
}

/// Complete first-order effect graph for one semantic analysis unit.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectProgram {
    callables: BTreeMap<CallableId, CallableFacts>,
    available_capabilities: Option<EffectSet>,
    strict_overdeclaration: bool,
}

/// Duplicate callable identity while building an effect graph.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("effect graph already contains callable `{id}`")]
pub struct DuplicateCallableError {
    id: CallableId,
}

impl CallableId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CallableId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl EffectSite {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    #[must_use]
    pub const fn with_position(mut self, line: u32, column: u32) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub const fn line(&self) -> Option<u32> {
        self.line
    }

    pub const fn column(&self) -> Option<u32> {
        self.column
    }
}

impl EffectUse {
    pub fn new(effect: EffectId, site: EffectSite) -> Self {
        Self { effect, site }
    }

    pub const fn effect(&self) -> &EffectId {
        &self.effect
    }

    pub const fn site(&self) -> &EffectSite {
        &self.site
    }
}

impl EffectContract {
    pub fn inferred() -> Self {
        Self::default()
    }

    pub fn bounded(upper_bound: EffectSet) -> Self {
        Self {
            upper_bound: Some(upper_bound),
            ..Self::default()
        }
    }

    pub fn pure() -> Self {
        Self {
            upper_bound: Some(EffectSet::new()),
            pure: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_forbidden(mut self, forbidden: EffectSet) -> Self {
        self.forbidden = forbidden;
        self
    }

    #[must_use]
    pub const fn requiring_explicit_nonempty(mut self) -> Self {
        self.require_explicit_nonempty = true;
        self
    }

    pub const fn upper_bound(&self) -> Option<&EffectSet> {
        self.upper_bound.as_ref()
    }

    pub const fn forbidden(&self) -> &EffectSet {
        &self.forbidden
    }

    pub const fn is_pure(&self) -> bool {
        self.pure
    }

    pub const fn requires_explicit_nonempty(&self) -> bool {
        self.require_explicit_nonempty
    }
}

impl ExternalCallable {
    pub fn new(name: impl Into<String>, effects: EffectSet) -> Self {
        Self {
            name: name.into(),
            effects,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn effects(&self) -> &EffectSet {
        &self.effects
    }
}

impl CallEdge {
    pub fn local(callee: CallableId, site: EffectSite) -> Self {
        Self {
            target: CallTarget::Local(callee),
            site,
        }
    }

    pub fn external(callee: ExternalCallable, site: EffectSite) -> Self {
        Self {
            target: CallTarget::External(callee),
            site,
        }
    }

    pub fn dynamic(label: impl Into<String>, effects: Option<EffectSet>, site: EffectSite) -> Self {
        Self {
            target: CallTarget::Dynamic {
                label: label.into(),
                effects,
            },
            site,
        }
    }

    pub const fn target(&self) -> &CallTarget {
        &self.target
    }

    pub const fn site(&self) -> &EffectSite {
        &self.site
    }
}

impl CallableFacts {
    pub fn new(id: CallableId, kind: CallableKind, visibility: Visibility) -> Self {
        let contract = if matches!(visibility, Visibility::Public | Visibility::Boundary) {
            EffectContract::inferred().requiring_explicit_nonempty()
        } else {
            EffectContract::inferred()
        };
        Self {
            id,
            kind,
            visibility,
            contract,
            direct_effects: Vec::new(),
            calls: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_contract(mut self, mut contract: EffectContract) -> Self {
        if matches!(self.visibility, Visibility::Public | Visibility::Boundary)
            && contract.upper_bound().is_none()
        {
            contract = contract.requiring_explicit_nonempty();
        }
        self.contract = contract;
        self
    }

    pub fn record_effect(&mut self, effect: EffectId, site: EffectSite) {
        self.direct_effects.push(EffectUse::new(effect, site));
    }

    pub fn record_call(&mut self, edge: CallEdge) {
        self.calls.push(edge);
    }

    pub const fn id(&self) -> &CallableId {
        &self.id
    }

    pub const fn kind(&self) -> CallableKind {
        self.kind
    }

    pub const fn visibility(&self) -> Visibility {
        self.visibility
    }

    pub const fn contract(&self) -> &EffectContract {
        &self.contract
    }

    pub fn direct_effects(&self) -> &[EffectUse] {
        &self.direct_effects
    }

    pub fn calls(&self) -> &[CallEdge] {
        &self.calls
    }
}

impl EffectProgram {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, callable: CallableFacts) -> Result<(), DuplicateCallableError> {
        let id = callable.id().clone();
        match self.callables.entry(id.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(callable);
                Ok(())
            }
            Entry::Occupied(_) => Err(DuplicateCallableError { id }),
        }
    }

    #[must_use]
    pub fn with_available_capabilities(mut self, capabilities: EffectSet) -> Self {
        self.available_capabilities = Some(capabilities);
        self
    }

    #[must_use]
    pub const fn with_strict_overdeclaration(mut self, strict: bool) -> Self {
        self.strict_overdeclaration = strict;
        self
    }

    pub fn callable(&self, id: &CallableId) -> Option<&CallableFacts> {
        self.callables.get(id)
    }

    pub(crate) fn callable_mut(&mut self, id: &CallableId) -> Option<&mut CallableFacts> {
        self.callables.get_mut(id)
    }

    pub fn callables(&self) -> impl ExactSizeIterator<Item = (&CallableId, &CallableFacts)> {
        self.callables.iter()
    }

    pub const fn available_capabilities(&self) -> Option<&EffectSet> {
        self.available_capabilities.as_ref()
    }

    pub const fn strict_overdeclaration(&self) -> bool {
        self.strict_overdeclaration
    }
}
