//! Exact computed-Style cache identity and deterministic eviction.

use super::provider::ViewAxisProviderUpdatePlan;
use super::{
    ViewStyleEnvironmentUsage, ViewStyleNodeFacts, ViewStyleNodeKey, ViewStyleResolveContext,
    ViewStyleResolver, ViewStyleRevisionSet,
};
use crate::style::{
    ComputedViewAxes, ComputedViewStyle, ComputedViewStyleRevision, ViewBoxAxisRevision,
    ViewInheritedBoxAxes, ViewPropertyKind, ViewSpecifiedValue, ViewStyleTrace,
};
use crate::{ViewElementKind, ViewMountId};
use arcweft_presentation::appearance::{
    ColorScheme, ContrastPreference, PresentationEnvironment, PresentationEnvironmentField,
    PresentationEnvironmentFieldRevisions, PresentationEnvironmentFieldSet,
    PresentationEnvironmentValues,
};
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ViewStyleCacheKey {
    node: ViewStyleNodeKey,
    facts: Vec<ViewStyleNodeFacts>,
    revisions: ViewStyleRevisionSet,
    axis_mode: u8,
    axis_revision: ViewBoxAxisRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ViewStyleCacheEntry {
    pub(super) computed: Arc<ComputedViewStyle>,
    pub(super) environment_usage: ViewStyleEnvironmentUsage,
    pub(super) selection_stamp: ViewStyleSelectionStamp,
    pub(super) parent_identity: Option<ViewInheritedStyleIdentity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ViewStyleSelectionStamp {
    fields: PresentationEnvironmentFieldSet,
    values: PresentationEnvironmentValues,
    revisions: PresentationEnvironmentFieldRevisions,
}

/// Computed result, environment usage, and optional deterministic trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewStyleResolveResult {
    pub(super) computed: Arc<ComputedViewStyle>,
    pub(super) environment_usage: ViewStyleEnvironmentUsage,
    pub(super) trace: ViewStyleTrace,
    pub(super) cache_hit: bool,
}

/// Exact inherited values participating in child cache identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewInheritedStyleIdentity {
    axes: ViewInheritedBoxAxes,
    properties: Box<[(ViewPropertyKind, ViewSpecifiedValue)]>,
}

impl ViewInheritedStyleIdentity {
    pub fn from_computed(computed: &ComputedViewStyle) -> Self {
        let properties = computed
            .properties()
            .filter(|(property, _)| property.is_inherited() && !property.is_axis_context())
            .map(|(property, computed)| (property, computed.value().clone()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            axes: computed.axes().inherited_snapshot(),
            properties,
        }
    }

    pub const fn axes(&self) -> ViewInheritedBoxAxes {
        self.axes
    }

    pub fn properties(&self) -> &[(ViewPropertyKind, ViewSpecifiedValue)] {
        &self.properties
    }
}

impl ViewStyleResolveResult {
    pub fn computed(&self) -> &ComputedViewStyle {
        &self.computed
    }

    pub fn computed_arc(&self) -> Arc<ComputedViewStyle> {
        Arc::clone(&self.computed)
    }

    pub const fn environment_usage(&self) -> ViewStyleEnvironmentUsage {
        self.environment_usage
    }

    pub const fn trace(&self) -> &ViewStyleTrace {
        &self.trace
    }

    pub const fn cache_hit(&self) -> bool {
        self.cache_hit
    }

    pub fn into_computed(self) -> Arc<ComputedViewStyle> {
        self.computed
    }
}

impl ViewStyleCacheKey {
    pub(super) fn new(context: &ViewStyleResolveContext<'_>, axes: &ComputedViewAxes) -> Self {
        let mut facts = context.ancestors.to_vec();
        facts.push(context.node.clone());
        Self {
            node: context.node_key.clone(),
            facts,
            revisions: context.revisions,
            axis_mode: axes.mode().canonical_tag(),
            axis_revision: axes.revision(),
        }
    }
}

impl ViewStyleSelectionStamp {
    pub(super) fn new(
        fields: PresentationEnvironmentFieldSet,
        environment: PresentationEnvironment,
    ) -> Self {
        Self {
            fields,
            values: environment.values(),
            revisions: environment.field_revisions(),
        }
    }

    pub(super) fn matches(self, environment: PresentationEnvironment) -> bool {
        self.fields.iter().all(|field| {
            self.values.value(field) == environment.values().value(field)
                && self.revisions.field_revision(field) == environment.field_revision(field)
        })
    }
}

impl ViewStyleResolver {
    pub fn clear(&mut self) {
        self.cache.clear();
        self.cache_order.clear();
        self.axis_providers.clear();
    }

    pub fn invalidate_node(&mut self, node: &ViewStyleNodeKey) {
        self.cache.retain(|key, _| &key.node != node);
        self.cache_order.retain(|key| &key.node != node);
    }

    /// Removes retained provider and cache state owned by one dead View mount.
    ///
    /// The returned count is the number of provider records removed. Repeating
    /// the operation for an already absent mount returns zero.
    pub fn invalidate_mount(&mut self, mount: ViewMountId) -> usize {
        let removed = self.axis_providers.invalidate_mount(mount);
        self.cache.retain(|key, _| key.node.mount() != mount);
        self.cache_order.retain(|key| key.node.mount() != mount);
        removed
    }

    pub(super) fn insert_cache(&mut self, key: ViewStyleCacheKey, entry: ViewStyleCacheEntry) {
        if self.limits.max_cache_entries == 0 {
            return;
        }
        if !self.cache.contains_key(&key) {
            while self.cache.len() >= self.limits.max_cache_entries {
                if let Some(oldest) = self.cache_order.pop_front() {
                    self.cache.remove(&oldest);
                } else {
                    break;
                }
            }
            self.cache_order.push_back(key.clone());
        }
        self.cache.insert(key, entry);
    }

    pub(super) fn commit_provider_update(&mut self, update: Option<ViewAxisProviderUpdatePlan>) {
        let Some(plan) = update else {
            return;
        };
        if plan.provider_changed() {
            let invalidated: BTreeSet<_> = plan.invalidated_nodes().cloned().collect();
            self.cache.retain(|key, _| !invalidated.contains(&key.node));
            self.cache_order
                .retain(|key| !invalidated.contains(&key.node));
        }
        self.axis_providers.commit(plan);
    }
}

impl Default for ViewStyleResolver {
    fn default() -> Self {
        Self::new(super::ViewStyleResolverLimits::default())
    }
}

pub(super) fn computed_revision(
    key: &ViewStyleCacheKey,
    selection: ViewStyleSelectionStamp,
    parent: Option<&ViewInheritedStyleIdentity>,
) -> ComputedViewStyleRevision {
    let mut revision = 0xcbf2_9ce4_8422_2325_u64;
    for value in [
        key.node.mount.get(),
        u64::from(key.node.instruction),
        key.revisions.sheets,
        key.revisions.patches,
        key.revisions.tokens,
        key.revisions.applications,
        key.revisions.interactions,
        key.revisions.containers,
        u64::from(key.axis_mode),
        key.axis_revision.value(),
    ] {
        revision ^= value;
        revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
    }
    revision ^= u64::from(parent.is_some());
    revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
    if let Some(parent) = parent {
        revision ^= u64::from(parent.axes().mode().canonical_tag());
        revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
        revision ^= parent.axes().revision().value();
        revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
        revision ^= u64::try_from(parent.properties().len()).unwrap_or(u64::MAX);
        revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
    }
    for segment in &key.node.path {
        revision ^= *segment;
        revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
    }
    for facts in &key.facts {
        revision ^= facts
            .element
            .and_then(|element| {
                ViewElementKind::ALL
                    .into_iter()
                    .position(|candidate| candidate == element)
            })
            .and_then(|index| u64::try_from(index).ok())
            .unwrap_or(u64::MAX);
        revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
        for part in [
            facts
                .implementation_part
                .as_ref()
                .map(|part| part.as_public_id().as_str()),
            facts
                .exported_part
                .as_ref()
                .map(|part| part.as_public_id().as_str()),
        ] {
            revision ^= u64::from(part.is_some());
            revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
            if let Some(part) = part {
                for byte in part.bytes() {
                    revision ^= u64::from(byte);
                    revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
        revision ^= u64::from(facts.interactions.0) | (u64::from(facts.element_states.0) << 8);
        revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
        for scope in &facts.active_scopes {
            revision ^= scope.value();
            revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    for field in selection.fields.iter() {
        revision ^= environment_field_rank(field);
        revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
        revision ^= selection.revisions.field_revision(field).value();
        revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
        revision ^= environment_value_rank(selection.values, field);
        revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
    }
    ComputedViewStyleRevision::new(revision)
}

const fn environment_field_rank(field: PresentationEnvironmentField) -> u64 {
    match field {
        PresentationEnvironmentField::ColorScheme => 0,
        PresentationEnvironmentField::Contrast => 1,
        PresentationEnvironmentField::ReducedMotion => 2,
        PresentationEnvironmentField::TextScale => 3,
    }
}

const fn environment_value_rank(
    values: PresentationEnvironmentValues,
    field: PresentationEnvironmentField,
) -> u64 {
    match field {
        PresentationEnvironmentField::ColorScheme => {
            color_scheme_rank(values.color_scheme()) as u64
        }
        PresentationEnvironmentField::Contrast => contrast_rank(values.contrast()) as u64,
        PresentationEnvironmentField::ReducedMotion => values.reduced_motion() as u64,
        PresentationEnvironmentField::TextScale => values.text_scale().value() as u64,
    }
}

const fn color_scheme_rank(value: ColorScheme) -> u8 {
    match value {
        ColorScheme::Light => 0,
        ColorScheme::Dark => 1,
    }
}

const fn contrast_rank(value: ContrastPreference) -> u8 {
    match value {
        ContrastPreference::Standard => 0,
        ContrastPreference::More => 1,
    }
}
