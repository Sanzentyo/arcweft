//! Retained View ownership for resolved Fx applications.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_id::PublicId;
use arcweft_presentation::fx::{FxId, FxInstanceId};
use thiserror::Error;

use crate::{NodeKey, ValueSourceId};

/// Authored position of an `.fx(...)` modifier in one View modifier chain.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewFxOrdinal(u32);

/// Reactive View expression bound to one named Fx parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewFxArgumentBinding {
    parameter: String,
    source: ValueSourceId,
}

/// Stable retained owner path used to derive one View Fx instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewFxIdentity {
    view: PublicId,
    node: NodeKey,
    repeat_item_key: Option<String>,
    ordinal: ViewFxOrdinal,
    local_key: Option<String>,
}

/// One resolved Fx application retained for a View node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedViewFxApplication {
    definition: FxId,
    instance: FxInstanceId,
    identity: ViewFxIdentity,
    arguments: Vec<ViewFxArgumentBinding>,
}

/// Stable sidecar indexed by Fx instance identity.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RetainedViewFxTable {
    applications: BTreeMap<FxInstanceId, RetainedViewFxApplication>,
    nodes: BTreeMap<NodeKey, BTreeMap<ViewFxOrdinal, FxInstanceId>>,
}

/// Invalid retained View Fx application data.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewFxError {
    #[error("Fx parameter `{0}` is bound more than once")]
    DuplicateParameter(String),
    #[error("duplicate retained View Fx instance {0:?}")]
    DuplicateInstance(FxInstanceId),
    #[error("retained View node {node:?} has more than one Fx at authored ordinal {ordinal:?}")]
    DuplicateOrdinal {
        node: NodeKey,
        ordinal: ViewFxOrdinal,
    },
}

impl ViewFxArgumentBinding {
    pub fn new(parameter: impl Into<String>, source: ValueSourceId) -> Self {
        Self {
            parameter: parameter.into(),
            source,
        }
    }

    pub fn parameter(&self) -> &str {
        &self.parameter
    }

    pub const fn source(&self) -> ValueSourceId {
        self.source
    }
}

impl ViewFxOrdinal {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl ViewFxIdentity {
    pub const fn new(view: PublicId, node: NodeKey, ordinal: ViewFxOrdinal) -> Self {
        Self {
            view,
            node,
            repeat_item_key: None,
            ordinal,
            local_key: None,
        }
    }

    #[must_use]
    pub fn with_repeat_item_key(mut self, key: impl Into<String>) -> Self {
        self.repeat_item_key = Some(key.into());
        self
    }

    #[must_use]
    pub fn with_local_key(mut self, key: impl Into<String>) -> Self {
        self.local_key = Some(key.into());
        self
    }

    pub const fn view(&self) -> &PublicId {
        &self.view
    }

    pub const fn node(&self) -> NodeKey {
        self.node
    }

    pub fn repeat_item_key(&self) -> Option<&str> {
        self.repeat_item_key.as_deref()
    }

    pub const fn ordinal(&self) -> ViewFxOrdinal {
        self.ordinal
    }

    pub fn local_key(&self) -> Option<&str> {
        self.local_key.as_deref()
    }

    fn derive_instance(&self, definition: &FxId) -> FxInstanceId {
        let node_key = self.node.0.to_string();
        let ordinal_key = self.ordinal.get().to_string();
        FxInstanceId::derive(
            definition,
            [
                self.view.as_str(),
                node_key.as_str(),
                self.repeat_item_key.as_deref().unwrap_or(""),
                ordinal_key.as_str(),
                self.local_key.as_deref().unwrap_or(""),
            ],
        )
    }
}

impl RetainedViewFxApplication {
    /// Resolves stable instance identity using the canonical View component order.
    pub fn new(
        definition: FxId,
        identity: ViewFxIdentity,
        arguments: Vec<ViewFxArgumentBinding>,
    ) -> Result<Self, ViewFxError> {
        let mut parameters = BTreeSet::new();
        if let Some(duplicate) = arguments
            .iter()
            .map(ViewFxArgumentBinding::parameter)
            .find(|parameter| !parameters.insert((*parameter).to_owned()))
        {
            return Err(ViewFxError::DuplicateParameter(duplicate.to_owned()));
        }

        let instance = identity.derive_instance(&definition);
        Ok(Self {
            definition,
            instance,
            identity,
            arguments,
        })
    }

    pub const fn definition(&self) -> &FxId {
        &self.definition
    }

    pub const fn instance(&self) -> FxInstanceId {
        self.instance
    }

    pub const fn identity(&self) -> &ViewFxIdentity {
        &self.identity
    }

    pub const fn view(&self) -> &PublicId {
        self.identity.view()
    }

    pub const fn node(&self) -> NodeKey {
        self.identity.node()
    }

    pub fn repeat_item_key(&self) -> Option<&str> {
        self.identity.repeat_item_key()
    }

    pub const fn ordinal(&self) -> ViewFxOrdinal {
        self.identity.ordinal()
    }

    pub fn local_key(&self) -> Option<&str> {
        self.identity.local_key()
    }

    pub fn arguments(&self) -> &[ViewFxArgumentBinding] {
        &self.arguments
    }
}

impl RetainedViewFxTable {
    pub fn insert(&mut self, application: RetainedViewFxApplication) -> Result<(), ViewFxError> {
        let instance = application.instance();
        if self.applications.contains_key(&instance) {
            return Err(ViewFxError::DuplicateInstance(instance));
        }
        let node = application.node();
        let ordinal = application.ordinal();
        let node_applications = self.nodes.entry(node).or_default();
        if node_applications.contains_key(&ordinal) {
            return Err(ViewFxError::DuplicateOrdinal { node, ordinal });
        }
        node_applications.insert(ordinal, instance);
        self.applications.insert(instance, application);
        Ok(())
    }

    pub fn get(&self, instance: FxInstanceId) -> Option<&RetainedViewFxApplication> {
        self.applications.get(&instance)
    }

    pub fn for_node(&self, node: NodeKey) -> impl Iterator<Item = &RetainedViewFxApplication> {
        self.nodes
            .get(&node)
            .into_iter()
            .flat_map(BTreeMap::values)
            .filter_map(|instance| self.applications.get(instance))
    }

    pub fn len(&self) -> usize {
        self.applications.len()
    }

    pub fn is_empty(&self) -> bool {
        self.applications.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx() -> FxId {
        FxId::try_new("game", "ui.effects.wave").unwrap()
    }

    fn view() -> PublicId {
        PublicId::try_new("view.battle_hud").unwrap()
    }

    fn application(
        node: u64,
        ordinal: u32,
        repeat_key: Option<&str>,
        local_key: Option<&str>,
    ) -> RetainedViewFxApplication {
        let mut identity = ViewFxIdentity::new(view(), NodeKey(node), ViewFxOrdinal::new(ordinal));
        if let Some(key) = repeat_key {
            identity = identity.with_repeat_item_key(key);
        }
        if let Some(key) = local_key {
            identity = identity.with_local_key(key);
        }
        RetainedViewFxApplication::new(
            fx(),
            identity,
            vec![ViewFxArgumentBinding::new("amplitude", ValueSourceId(7))],
        )
        .unwrap()
    }

    #[test]
    fn instance_identity_distinguishes_each_stable_view_component() {
        let baseline = application(4, 0, Some("enemy-2"), Some("damage"));
        let other_definition = RetainedViewFxApplication::new(
            FxId::try_new("game", "ui.effects.pulse").unwrap(),
            ViewFxIdentity::new(view(), NodeKey(4), ViewFxOrdinal::new(0))
                .with_repeat_item_key("enemy-2")
                .with_local_key("damage"),
            vec![],
        )
        .unwrap();
        let other_view = RetainedViewFxApplication::new(
            fx(),
            ViewFxIdentity::new(
                PublicId::try_new("view.other_hud").unwrap(),
                NodeKey(4),
                ViewFxOrdinal::new(0),
            )
            .with_repeat_item_key("enemy-2")
            .with_local_key("damage"),
            vec![],
        )
        .unwrap();

        for distinct in [
            other_definition,
            other_view,
            application(5, 0, Some("enemy-2"), Some("damage")),
            application(4, 1, Some("enemy-2"), Some("damage")),
            application(4, 0, Some("enemy-3"), Some("damage")),
            application(4, 0, Some("enemy-2"), Some("healing")),
        ] {
            assert_ne!(baseline.instance(), distinct.instance());
        }
        assert_eq!(
            baseline.instance(),
            application(4, 0, Some("enemy-2"), Some("damage")).instance()
        );
    }

    #[test]
    fn table_rejects_duplicate_instances_and_queries_by_node() {
        let first = application(4, 0, None, None);
        let duplicate = first.clone();
        let second = application(4, 1, None, None);
        let mut table = RetainedViewFxTable::default();

        table.insert(second).unwrap();
        table.insert(first).unwrap();
        assert_eq!(table.for_node(NodeKey(4)).count(), 2);
        assert_eq!(
            table
                .for_node(NodeKey(4))
                .map(RetainedViewFxApplication::ordinal)
                .collect::<Vec<_>>(),
            [ViewFxOrdinal::new(0), ViewFxOrdinal::new(1)]
        );
        assert!(matches!(
            table.insert(duplicate),
            Err(ViewFxError::DuplicateInstance(_))
        ));
        assert!(matches!(
            table.insert(application(4, 1, None, Some("other"))),
            Err(ViewFxError::DuplicateOrdinal {
                node: NodeKey(4),
                ordinal
            }) if ordinal == ViewFxOrdinal::new(1)
        ));
    }

    #[test]
    fn application_rejects_duplicate_parameter_bindings() {
        let result = RetainedViewFxApplication::new(
            fx(),
            ViewFxIdentity::new(view(), NodeKey(1), ViewFxOrdinal::new(0)),
            vec![
                ViewFxArgumentBinding::new("speed", ValueSourceId(1)),
                ViewFxArgumentBinding::new("speed", ValueSourceId(2)),
            ],
        );

        assert_eq!(
            result,
            Err(ViewFxError::DuplicateParameter("speed".to_owned()))
        );
    }
}
