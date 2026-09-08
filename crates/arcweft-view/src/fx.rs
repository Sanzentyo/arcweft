//! Retained View ownership for resolved Fx applications.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_id::PublicId;
use arcweft_presentation::fx::{
    FxDefinitionArgumentValue, FxDefinitionParameterIndex, FxDefinitionParameterLayoutDigest, FxId,
    FxInstanceId, FxInstanceIdentity, FxInstanceOwnerKey,
};
use thiserror::Error;

use crate::{NodeKey, ValueSourceId};

/// Authored position of an `.fx(...)` modifier in one View modifier chain.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewFxOrdinal(u32);

/// Reactive View expression bound to one named Fx parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewFxArgumentBinding {
    parameter: FxDefinitionParameterIndex,
    source: ViewFxBindingSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewFxBindingSource {
    Reactive(ValueSourceId),
    Closed(FxDefinitionArgumentValue),
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
    instance_identity: FxInstanceIdentity,
    parameter_layout: FxDefinitionParameterLayoutDigest,
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
    #[error("Fx parameter index {0:?} is bound more than once")]
    DuplicateParameter(FxDefinitionParameterIndex),
    #[error("duplicate retained View Fx instance {0:?}")]
    DuplicateInstance(FxInstanceId),
    #[error("retained View node {node:?} has more than one Fx at authored ordinal {ordinal:?}")]
    DuplicateOrdinal {
        node: NodeKey,
        ordinal: ViewFxOrdinal,
    },
}

impl ViewFxArgumentBinding {
    pub const fn reactive(parameter: FxDefinitionParameterIndex, source: ValueSourceId) -> Self {
        Self {
            parameter,
            source: ViewFxBindingSource::Reactive(source),
        }
    }

    pub fn closed(parameter: FxDefinitionParameterIndex, value: FxDefinitionArgumentValue) -> Self {
        Self {
            parameter,
            source: ViewFxBindingSource::Closed(value),
        }
    }

    pub const fn parameter(&self) -> FxDefinitionParameterIndex {
        self.parameter
    }

    pub const fn source(&self) -> &ViewFxBindingSource {
        &self.source
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

    fn derive_instance(&self, definition: &FxId) -> FxInstanceIdentity {
        let mut canonical = Vec::new();
        canonical.push(1);
        append_bytes(&mut canonical, self.view.as_str().as_bytes());
        canonical.extend_from_slice(&self.node.0.to_le_bytes());
        append_optional_bytes(&mut canonical, self.repeat_item_key.as_deref());
        append_optional_bytes(&mut canonical, self.local_key.as_deref());
        FxInstanceIdentity::new(
            definition,
            FxInstanceOwnerKey::from_view_canonical_bytes(&canonical),
            self.ordinal.get(),
        )
    }
}

fn append_bytes(target: &mut Vec<u8>, value: &[u8]) {
    let length = u64::try_from(value.len())
        .expect("a View identity component length fits the canonical owner key");
    target.extend_from_slice(&length.to_le_bytes());
    target.extend_from_slice(value);
}

fn append_optional_bytes(target: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            target.push(1);
            append_bytes(target, value.as_bytes());
        }
        None => target.push(0),
    }
}

impl RetainedViewFxApplication {
    /// Resolves stable instance identity using the canonical View component order.
    pub fn new(
        definition: &FxId,
        parameter_layout: FxDefinitionParameterLayoutDigest,
        identity: ViewFxIdentity,
        arguments: Vec<ViewFxArgumentBinding>,
    ) -> Result<Self, ViewFxError> {
        let mut parameters = BTreeSet::new();
        if let Some(duplicate) = arguments
            .iter()
            .map(ViewFxArgumentBinding::parameter)
            .find(|parameter| !parameters.insert(*parameter))
        {
            return Err(ViewFxError::DuplicateParameter(duplicate));
        }

        let instance_identity = identity.derive_instance(definition);
        Ok(Self {
            instance_identity,
            parameter_layout,
            identity,
            arguments,
        })
    }

    pub const fn definition(&self) -> &FxId {
        self.instance_identity.definition()
    }

    pub const fn instance(&self) -> FxInstanceId {
        self.instance_identity.instance()
    }

    pub const fn instance_identity(&self) -> &FxInstanceIdentity {
        &self.instance_identity
    }

    pub const fn parameter_layout(&self) -> FxDefinitionParameterLayoutDigest {
        self.parameter_layout
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
    use arcweft_presentation::fx::{
        FxDefinition, FxDefinitionParameter, FxDefinitionParameterType, FxGraph, FxRuntimeType,
    };

    fn fx() -> FxId {
        FxId::try_new("game", "ui.effects.wave").unwrap()
    }

    fn view() -> PublicId {
        PublicId::try_new("view.battle_hud").unwrap()
    }

    fn binding_authority() -> (
        FxDefinitionParameterIndex,
        FxDefinitionParameterLayoutDigest,
    ) {
        let definition = FxDefinition::new(
            fx(),
            vec![
                FxDefinitionParameter::try_new(
                    0,
                    "amplitude",
                    FxDefinitionParameterType::Runtime(FxRuntimeType::F32),
                    None,
                )
                .unwrap(),
            ],
            FxGraph::default(),
        )
        .unwrap();
        (
            definition.parameters()[0].index(),
            definition.parameter_layout().digest(),
        )
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
        let (parameter, layout) = binding_authority();
        RetainedViewFxApplication::new(
            &fx(),
            layout,
            identity,
            vec![ViewFxArgumentBinding::reactive(parameter, ValueSourceId(7))],
        )
        .unwrap()
    }

    #[test]
    fn instance_identity_distinguishes_each_stable_view_component() {
        let baseline = application(4, 0, Some("enemy-2"), Some("damage"));
        let other_definition = RetainedViewFxApplication::new(
            &FxId::try_new("game", "ui.effects.pulse").unwrap(),
            binding_authority().1,
            ViewFxIdentity::new(view(), NodeKey(4), ViewFxOrdinal::new(0))
                .with_repeat_item_key("enemy-2")
                .with_local_key("damage"),
            vec![],
        )
        .unwrap();
        let other_view = RetainedViewFxApplication::new(
            &fx(),
            binding_authority().1,
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
    fn authored_ordinal_is_sealed_separately_from_view_owner_components() {
        let first = application(4, 0, None, None);
        let second = application(4, 1, None, None);
        let first_again = application(4, 0, None, None);

        assert_ne!(first.instance(), second.instance());
        assert_eq!(first.instance(), first_again.instance());
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
            &fx(),
            binding_authority().1,
            ViewFxIdentity::new(view(), NodeKey(1), ViewFxOrdinal::new(0)),
            vec![
                ViewFxArgumentBinding::reactive(binding_authority().0, ValueSourceId(1)),
                ViewFxArgumentBinding::reactive(binding_authority().0, ValueSourceId(2)),
            ],
        );

        assert_eq!(
            result,
            Err(ViewFxError::DuplicateParameter(binding_authority().0))
        );
    }
}
