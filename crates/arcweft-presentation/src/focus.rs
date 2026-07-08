//! Typed focus tree and focus-transition contract for keyboard and text input.
//!
//! Pointer hit testing remains `HitTree`-based. Keyboard and IME text input route
//! through `FocusTree` plus an active `TextInputSessionId` so composition events
//! cannot be delivered to stale or hidden targets.

use crate::input::InteractionTarget;
use crate::layer::LayerId;
use crate::text_input::{CompositionEndReason, TextInputHostCommand, TextInputSessionId};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FocusScopeId(pub u32);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FocusOrder(pub u32);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct FocusGeneration(pub u64);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FocusTree {
    records: Vec<FocusRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusRecord {
    layer: LayerId,
    target: InteractionTarget,
    scope: FocusScopeId,
    order: FocusOrder,
    kind: FocusTargetKind,
    enabled: bool,
    visible: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FocusTargetKind {
    #[default]
    Generic,
    TextInput,
    TextArea,
    SecureTextInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusOwner {
    layer: LayerId,
    target: InteractionTarget,
    scope: FocusScopeId,
    generation: FocusGeneration,
    cause: FocusCause,
    focus_visible: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FocusCause {
    #[default]
    Programmatic,
    Pointer,
    KeyboardTraversal,
    Agent,
    Restore,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FocusLease {
    owner: Option<FocusOwner>,
    text_session: Option<ActiveTextInputSession>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveTextInputSession {
    session: TextInputSessionId,
    generation: FocusGeneration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FocusDirective {
    Default,
    Preserve,
    Request(InteractionTarget),
    Clear,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusTransitionRequest {
    target: Option<InteractionTarget>,
    cause: FocusCause,
    composition_on_blur: CompositionOnBlur,
    cancelable: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompositionOnBlur {
    #[default]
    Commit,
    Cancel,
    PlatformDefault,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FocusTransitionResult {
    Unchanged,
    Cleared {
        previous: FocusOwner,
        host_commands: Vec<TextInputHostCommand>,
    },
    Changed {
        previous: Option<FocusOwner>,
        next: FocusOwner,
        host_commands: Vec<TextInputHostCommand>,
    },
    Rejected(FocusRejectReason),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FocusRejectReason {
    TargetMissing,
    TargetDisabled,
    TargetHidden,
    ScopeBlocked,
    PreservedByHandler,
}

impl FocusTree {
    pub fn push(&mut self, record: FocusRecord) {
        self.records.push(record);
        self.records
            .sort_by_key(|record| (record.scope, record.order));
    }

    pub fn find(&self, target: &InteractionTarget) -> Option<&FocusRecord> {
        self.records.iter().find(|record| record.target() == target)
    }

    pub fn next_in_scope(
        &self,
        scope: FocusScopeId,
        current: Option<&InteractionTarget>,
    ) -> Option<&FocusRecord> {
        let mut candidates = self
            .records
            .iter()
            .filter(|record| record.scope == scope && record.enabled && record.visible)
            .collect::<Vec<_>>();
        candidates.sort_by_key(|record| record.order);
        let Some(current) = current else {
            return candidates.first().copied();
        };
        let index = candidates
            .iter()
            .position(|record| record.target() == current)
            .map_or(0, |index| index.saturating_add(1));
        candidates
            .get(index)
            .copied()
            .or_else(|| candidates.first().copied())
    }

    pub fn records(&self) -> &[FocusRecord] {
        &self.records
    }
}

impl FocusRecord {
    pub fn new(
        layer: LayerId,
        target: InteractionTarget,
        scope: FocusScopeId,
        order: FocusOrder,
        kind: FocusTargetKind,
    ) -> Self {
        Self {
            layer,
            target,
            scope,
            order,
            kind,
            enabled: true,
            visible: true,
        }
    }

    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub const fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub const fn layer(&self) -> &LayerId {
        &self.layer
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub const fn scope(&self) -> FocusScopeId {
        self.scope
    }

    pub const fn order(&self) -> FocusOrder {
        self.order
    }

    pub const fn kind(&self) -> FocusTargetKind {
        self.kind
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn visible(&self) -> bool {
        self.visible
    }
}

impl FocusOwner {
    pub fn new(
        layer: LayerId,
        target: InteractionTarget,
        scope: FocusScopeId,
        generation: FocusGeneration,
        cause: FocusCause,
    ) -> Self {
        Self {
            layer,
            target,
            scope,
            generation,
            cause,
            focus_visible: matches!(cause, FocusCause::KeyboardTraversal),
        }
    }

    pub const fn layer(&self) -> &LayerId {
        &self.layer
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub const fn scope(&self) -> FocusScopeId {
        self.scope
    }

    pub const fn generation(&self) -> FocusGeneration {
        self.generation
    }

    pub const fn cause(&self) -> FocusCause {
        self.cause
    }

    pub const fn focus_visible(&self) -> bool {
        self.focus_visible
    }
}

impl FocusLease {
    pub const fn owner(&self) -> Option<&FocusOwner> {
        self.owner.as_ref()
    }

    pub const fn active_text_session(&self) -> Option<ActiveTextInputSession> {
        self.text_session
    }

    pub fn set_owner(&mut self, owner: FocusOwner) {
        self.owner = Some(owner);
    }

    pub fn clear(&mut self) -> Option<FocusOwner> {
        self.text_session = None;
        self.owner.take()
    }

    pub fn bind_text_session(
        &mut self,
        session: TextInputSessionId,
    ) -> Option<ActiveTextInputSession> {
        let generation = self.owner.as_ref()?.generation();
        let active = ActiveTextInputSession {
            session,
            generation,
        };
        self.text_session = Some(active);
        Some(active)
    }

    pub fn accepts_text_session(&self, session: TextInputSessionId) -> bool {
        self.text_session
            .is_some_and(|active| active.session == session)
    }
}

impl FocusGeneration {
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl ActiveTextInputSession {
    pub const fn session(self) -> TextInputSessionId {
        self.session
    }

    pub const fn generation(self) -> FocusGeneration {
        self.generation
    }
}

impl FocusTransitionRequest {
    pub fn request(target: InteractionTarget, cause: FocusCause) -> Self {
        Self {
            target: Some(target),
            cause,
            composition_on_blur: CompositionOnBlur::Commit,
            cancelable: true,
        }
    }

    pub const fn clear(cause: FocusCause) -> Self {
        Self {
            target: None,
            cause,
            composition_on_blur: CompositionOnBlur::Commit,
            cancelable: true,
        }
    }

    #[must_use]
    pub const fn with_composition_on_blur(mut self, policy: CompositionOnBlur) -> Self {
        self.composition_on_blur = policy;
        self
    }

    #[must_use]
    pub const fn forced(mut self) -> Self {
        self.cancelable = false;
        self
    }

    pub const fn target(&self) -> Option<&InteractionTarget> {
        self.target.as_ref()
    }

    pub const fn cause(&self) -> FocusCause {
        self.cause
    }

    pub const fn composition_on_blur(&self) -> CompositionOnBlur {
        self.composition_on_blur
    }

    pub const fn cancelable(&self) -> bool {
        self.cancelable
    }
}

pub fn composition_end_reason_for_blur(policy: CompositionOnBlur) -> CompositionEndReason {
    match policy {
        CompositionOnBlur::Commit => CompositionEndReason::Committed,
        CompositionOnBlur::Cancel => CompositionEndReason::Cancelled,
        CompositionOnBlur::PlatformDefault => CompositionEndReason::FocusChanged,
    }
}

#[cfg(test)]
mod tests {
    use super::{FocusCause, FocusGeneration, FocusLease, FocusOwner};
    use crate::input::InteractionTarget;
    use crate::layer::LayerId;
    use crate::text_input::TextInputSessionId;
    use arcweft_id::PublicId;

    #[test]
    fn focus_lease_binds_text_sessions_to_current_generation() {
        let mut lease = FocusLease::default();
        lease.set_owner(FocusOwner::new(
            layer("view"),
            target("field.name"),
            super::FocusScopeId(1),
            FocusGeneration(3),
            FocusCause::KeyboardTraversal,
        ));

        let active = lease
            .bind_text_session(TextInputSessionId(9))
            .expect("focused text field accepts a session");

        assert_eq!(active.session(), TextInputSessionId(9));
        assert_eq!(active.generation(), FocusGeneration(3));
        assert!(lease.accepts_text_session(TextInputSessionId(9)));
        assert!(!lease.accepts_text_session(TextInputSessionId(10)));
        assert!(lease.clear().is_some());
        assert!(!lease.accepts_text_session(TextInputSessionId(9)));
    }

    fn layer(name: &str) -> LayerId {
        LayerId::new(public_id(&format!("layer.{name}")))
    }

    fn target(name: &str) -> InteractionTarget {
        InteractionTarget::new(public_id(&format!("target.{name}")))
    }

    fn public_id(value: &str) -> PublicId {
        PublicId::try_new(value).expect("test id")
    }
}
