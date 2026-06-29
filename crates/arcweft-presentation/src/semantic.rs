use crate::hit::{HitRecord, HitRect, HitTree};
use crate::input::{
    Action, ActionTarget, AgentInput, InputEpoch, InputEventKind, InteractionTarget, RawInputEvent,
    RawInputKind,
};
use crate::interaction::InteractionState;
use crate::layer::{LayerId, LayerTree};
use crate::router::{InputRouter, RouteDecision};
use crate::text_input::TextInputOptions;
use arcweft_id::PublicId;

/// Semantic role shared by `TextBox`, `Activity`, UI, Agent, and accessibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticRole {
    TextBox,
    Activity,
    Button,
    TextField,
    TextArea,
    SecureTextField,
    Image,
    Debug,
    Custom,
}

impl SemanticRole {
    /// Returns whether this semantic role represents an Arcweft text-input control.
    pub const fn is_text_input_control(self) -> bool {
        matches!(
            self,
            Self::TextField | Self::TextArea | Self::SecureTextField
        )
    }

    /// Applies text-input options implied by the semantic role.
    ///
    /// Keeping this behavior on `SemanticRole` makes the text-control boundary
    /// discoverable for renderer, player, and accessibility callers.
    #[must_use]
    pub fn text_input_options(self, options: TextInputOptions) -> Option<TextInputOptions> {
        match self {
            Self::TextField => Some(options),
            Self::TextArea => Some(options.multiline(true)),
            Self::SecureTextField => Some(options.secure(true)),
            Self::TextBox
            | Self::Activity
            | Self::Button
            | Self::Image
            | Self::Debug
            | Self::Custom => None,
        }
    }
}

/// One semantic object visible to Agent/debug/accessibility routing.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticNode {
    layer: LayerId,
    target: InteractionTarget,
    role: SemanticRole,
    label: Option<String>,
    bounds: HitRect,
    actions: Vec<PublicId>,
    enabled: bool,
    visible: bool,
}

/// Frame semantic tree normalized across `TextBox`, `Activity`, and UI output.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SemanticTree {
    nodes: Vec<SemanticNode>,
}

/// Rejection reason for semantic action lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticActionError {
    UnknownTarget(InteractionTarget),
    Hidden(InteractionTarget),
    Disabled(InteractionTarget),
    UndeclaredAction {
        target: InteractionTarget,
        action: PublicId,
    },
    RejectedByRouter(RouteDecision),
}

impl SemanticNode {
    pub fn new(
        layer: LayerId,
        target: InteractionTarget,
        role: SemanticRole,
        bounds: HitRect,
    ) -> Self {
        Self {
            layer,
            target,
            role,
            label: None,
            bounds,
            actions: Vec::new(),
            enabled: true,
            visible: true,
        }
    }

    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn with_action(mut self, action: PublicId) -> Self {
        self.actions.push(action);
        self
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

    pub const fn role(&self) -> SemanticRole {
        self.role
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub const fn bounds(&self) -> HitRect {
        self.bounds
    }

    pub fn actions(&self) -> &[PublicId] {
        &self.actions
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn visible(&self) -> bool {
        self.visible
    }

    fn action_target(&self) -> ActionTarget {
        match self.role {
            SemanticRole::Activity => ActionTarget::Activity(self.target.clone()),
            SemanticRole::TextBox
            | SemanticRole::Button
            | SemanticRole::TextField
            | SemanticRole::TextArea
            | SemanticRole::SecureTextField
            | SemanticRole::Image
            | SemanticRole::Debug
            | SemanticRole::Custom => ActionTarget::Entity(self.target.clone()),
        }
    }

    fn to_hit_record(&self) -> HitRecord {
        HitRecord::new(self.layer.clone(), self.target.clone(), self.bounds)
            .with_enabled(self.enabled)
            .with_visible(self.visible)
    }
}

impl SemanticTree {
    pub fn push(&mut self, node: SemanticNode) {
        self.nodes.push(node);
    }

    pub fn as_slice(&self) -> &[SemanticNode] {
        &self.nodes
    }

    pub fn find(&self, target: &InteractionTarget) -> Option<&SemanticNode> {
        self.nodes.iter().find(|node| node.target() == target)
    }

    pub fn to_hit_tree(&self) -> HitTree {
        let mut hits = HitTree::default();
        for node in &self.nodes {
            hits.push(node.to_hit_record());
        }
        hits
    }

    pub fn lower_action(
        &self,
        target: &InteractionTarget,
        action: &PublicId,
    ) -> Result<Action, SemanticActionError> {
        let Some(node) = self.find(target) else {
            return Err(SemanticActionError::UnknownTarget(target.clone()));
        };
        if !node.visible() {
            return Err(SemanticActionError::Hidden(target.clone()));
        }
        if !node.enabled() {
            return Err(SemanticActionError::Disabled(target.clone()));
        }
        if !node.actions().iter().any(|candidate| candidate == action) {
            return Err(SemanticActionError::UndeclaredAction {
                target: target.clone(),
                action: action.clone(),
            });
        }
        Ok(Action::new(node.action_target(), action.clone()))
    }

    pub fn route_and_lower_action(
        &self,
        epoch: InputEpoch,
        target: &InteractionTarget,
        action: &PublicId,
        layers: &LayerTree,
        state: &InteractionState,
    ) -> Result<Action, SemanticActionError> {
        let lowered = self.lower_action(target, action)?;
        let hits = self.to_hit_tree();
        let raw = RawInputEvent::new(
            epoch,
            RawInputKind::Agent(AgentInput {
                action: action.clone(),
                target: Some(target.clone()),
            }),
        );
        let routed = InputRouter::route(&raw, layers, &hits, state);
        match routed.decision() {
            RouteDecision::Routed(event)
                if event.target() == target
                    && matches!(
                        event.kind(),
                        InputEventKind::AgentInvoke {
                            action: routed_action
                        } if routed_action == action
                    ) =>
            {
                Ok(lowered)
            }
            decision => Err(SemanticActionError::RejectedByRouter(decision.clone())),
        }
    }
}
