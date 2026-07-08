use crate::{
    id::{Identifier, IdentifierError},
    payload::InteractionPayload,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ActionId(Identifier);

impl ActionId {
    /// Creates an action identifier.
    ///
    /// # Errors
    ///
    /// Returns [`IdentifierError`] when the identifier is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        Identifier::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ActionTarget {
    Runtime,
    TextBox,
    Activity(Identifier),
    ViewEntity(Identifier),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionKind {
    Activate,
    Advance,
    Submit,
    Cancel,
    Select { index: usize },
    Custom { name: Identifier },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Action {
    pub id: ActionId,
    pub target: ActionTarget,
    pub action: ActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<InteractionPayload>,
}

impl Action {
    #[must_use]
    pub fn new(id: ActionId, target: ActionTarget, action: ActionKind) -> Self {
        Self {
            id,
            target,
            action,
            payload: None,
        }
    }

    #[must_use]
    pub fn with_payload(mut self, payload: InteractionPayload) -> Self {
        self.payload = Some(payload);
        self
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ActionBatch(Vec<Action>);

impl ActionBatch {
    #[must_use]
    pub fn new(actions: Vec<Action>) -> Self {
        Self(actions)
    }

    #[must_use]
    pub fn as_slice(&self) -> &[Action] {
        &self.0
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Action> {
        self.0.iter()
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<Action> {
        self.0
    }
}
