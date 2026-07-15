//! Per-property native Style cascade and application-layer priority.

use super::{
    ComputedViewAxes, ComputedViewProperty, ComputedViewStyle, ComputedViewStyleRevision,
    ComputedViewTransition, ViewAxisUsageSet, ViewComputedPropertyKind, ViewFontFamilyList,
    ViewPropertyKind, ViewSpecifiedValue, ViewStyleAssignOp, ViewStylePatchId, ViewStyleSheetId,
    ViewStyleSourceId,
};
use std::collections::BTreeMap;

/// Canonical winner key. Later tuple components only decide otherwise-equal
/// contributions from the same stronger application layer.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ViewStylePriority {
    scope_depth: u16,
    application_order: u32,
    specificity_predicates: u16,
    specificity_elements: u16,
    rule_source_order: u32,
    declaration_order: u32,
}

/// Provenance retained with a winning computed property.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewStyleContributionSource {
    Inherited,
    Sheet {
        sheet: ViewStyleSheetId,
        rule: ViewStyleSourceId,
        declaration: ViewStyleSourceId,
    },
    Patch {
        patch: ViewStylePatchId,
        declaration: ViewStyleSourceId,
    },
}

/// One token-resolved declaration ready for per-property winner comparison.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewStyleContribution {
    authored_property: ViewPropertyKind,
    expanded_property: ViewPropertyKind,
    resolved_property: ViewComputedPropertyKind,
    value: ViewSpecifiedValue,
    operation: ViewStyleAssignOp,
    priority: ViewStylePriority,
    source: ViewStyleContributionSource,
}

/// Public construction seam for typed adapters and the canonical resolver.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ComputedViewStyleBuilder {
    axes: ComputedViewAxes,
    properties: BTreeMap<ViewComputedPropertyKind, ComputedViewProperty>,
    transitions: Vec<ComputedViewTransition>,
    axis_usage: ViewAxisUsageSet,
}

impl ViewStylePriority {
    pub const INHERITED: Self = Self {
        scope_depth: 0,
        application_order: 0,
        specificity_predicates: 0,
        specificity_elements: 0,
        rule_source_order: 0,
        declaration_order: 0,
    };

    pub const fn new(
        scope_depth: u16,
        application_order: u32,
        specificity_predicates: u16,
        specificity_elements: u16,
        rule_source_order: u32,
        declaration_order: u32,
    ) -> Self {
        Self {
            scope_depth,
            application_order,
            specificity_predicates,
            specificity_elements,
            rule_source_order,
            declaration_order,
        }
    }

    pub const fn scope_depth(self) -> u16 {
        self.scope_depth
    }

    pub const fn application_order(self) -> u32 {
        self.application_order
    }

    pub const fn specificity(self) -> (u16, u16) {
        (self.specificity_predicates, self.specificity_elements)
    }

    pub const fn rule_source_order(self) -> u32 {
        self.rule_source_order
    }

    pub const fn declaration_order(self) -> u32 {
        self.declaration_order
    }
}

impl ViewStyleContribution {
    /// Constructs an already-canonical adapter contribution.
    ///
    /// # Panics
    ///
    /// Panics when `property` is an axis context, shorthand, or logical alias.
    /// Such authored declarations must pass through [`super::ViewStyleResolver`].
    pub fn new(
        property: ViewPropertyKind,
        value: ViewSpecifiedValue,
        operation: ViewStyleAssignOp,
        priority: ViewStylePriority,
        source: ViewStyleContributionSource,
    ) -> Self {
        let resolved_property = ViewComputedPropertyKind::try_from_property(property)
            .expect("computed Style contributions require a canonical property");
        Self::resolved(
            property,
            property,
            resolved_property,
            value,
            operation,
            priority,
            source,
        )
    }

    pub(super) const fn resolved(
        authored_property: ViewPropertyKind,
        expanded_property: ViewPropertyKind,
        resolved_property: ViewComputedPropertyKind,
        value: ViewSpecifiedValue,
        operation: ViewStyleAssignOp,
        priority: ViewStylePriority,
        source: ViewStyleContributionSource,
    ) -> Self {
        Self {
            authored_property,
            expanded_property,
            resolved_property,
            value,
            operation,
            priority,
            source,
        }
    }

    pub const fn property(&self) -> ViewPropertyKind {
        self.resolved_property.as_property()
    }

    pub const fn authored_property(&self) -> ViewPropertyKind {
        self.authored_property
    }

    pub const fn expanded_property(&self) -> ViewPropertyKind {
        self.expanded_property
    }

    pub const fn resolved_property(&self) -> ViewComputedPropertyKind {
        self.resolved_property
    }

    pub const fn value(&self) -> &ViewSpecifiedValue {
        &self.value
    }

    pub const fn operation(&self) -> ViewStyleAssignOp {
        self.operation
    }

    pub const fn priority(&self) -> ViewStylePriority {
        self.priority
    }

    pub const fn source(&self) -> &ViewStyleContributionSource {
        &self.source
    }
}

impl ComputedViewStyleBuilder {
    pub(crate) fn inherit(parent: Option<&ComputedViewStyle>, axes: ComputedViewAxes) -> Self {
        let properties = parent.map_or_else(BTreeMap::new, |parent| {
            parent
                .canonical_properties()
                .filter(|(property, _)| property.as_property().is_inherited())
                .map(|(resolved, value)| {
                    let property = resolved.as_property();
                    (
                        resolved,
                        ComputedViewProperty::new(
                            property,
                            property,
                            resolved,
                            value.value().clone(),
                            ViewStylePriority::INHERITED,
                            ViewStyleContributionSource::Inherited,
                        ),
                    )
                })
                .collect()
        });
        Self {
            axes,
            properties,
            transitions: Vec::new(),
            axis_usage: ViewAxisUsageSet::NONE,
        }
    }

    pub fn set_axes(&mut self, axes: ComputedViewAxes) {
        self.axes = axes;
    }

    pub fn set_transitions(&mut self, transitions: Vec<ComputedViewTransition>) {
        self.transitions = transitions;
    }

    pub fn include_axis_usage(&mut self, usage: ViewAxisUsageSet) {
        self.axis_usage = self.axis_usage.union(usage);
    }

    pub(super) fn value(&self, property: ViewPropertyKind) -> Option<&ViewSpecifiedValue> {
        ViewComputedPropertyKind::try_from_property(property)
            .and_then(|property| self.properties.get(&property))
            .map(ComputedViewProperty::value)
    }

    /// Applies a contribution when it wins its property slot. Append operates
    /// on the lower-priority computed list; a later replace discards that list.
    pub fn apply(&mut self, contribution: ViewStyleContribution) -> bool {
        if self
            .properties
            .get(&contribution.resolved_property)
            .is_some_and(|current| current.priority() > contribution.priority)
        {
            return false;
        }
        let value = if contribution.operation == ViewStyleAssignOp::Append {
            self.properties
                .get(&contribution.resolved_property)
                .and_then(|current| append_values(current.value(), &contribution.value))
                .unwrap_or_else(|| contribution.value.clone())
        } else {
            contribution.value.clone()
        };
        self.properties.insert(
            contribution.resolved_property,
            ComputedViewProperty::new(
                contribution.authored_property,
                contribution.expanded_property,
                contribution.resolved_property,
                value,
                contribution.priority,
                contribution.source,
            ),
        );
        true
    }

    pub fn finish(self, revision: ComputedViewStyleRevision) -> ComputedViewStyle {
        ComputedViewStyle::from_properties(
            self.axes,
            self.properties,
            self.transitions,
            self.axis_usage,
            revision,
        )
    }
}

fn append_values(
    existing: &ViewSpecifiedValue,
    appended: &ViewSpecifiedValue,
) -> Option<ViewSpecifiedValue> {
    match (existing, appended) {
        (
            ViewSpecifiedValue::FontFamilyList { value: existing },
            ViewSpecifiedValue::FontFamilyList { value: appended },
        ) => {
            let families = existing
                .as_slice()
                .iter()
                .chain(appended.as_slice())
                .cloned()
                .collect();
            ViewFontFamilyList::new(families)
                .map(|value| ViewSpecifiedValue::FontFamilyList { value })
        }
        (
            ViewSpecifiedValue::ShadowList { value: existing },
            ViewSpecifiedValue::ShadowList { value: appended },
        ) => Some(ViewSpecifiedValue::ShadowList {
            value: existing.iter().chain(appended).copied().collect(),
        }),
        (
            ViewSpecifiedValue::FilterList { value: existing },
            ViewSpecifiedValue::FilterList { value: appended },
        ) => Some(ViewSpecifiedValue::FilterList {
            value: existing.iter().chain(appended).copied().collect(),
        }),
        (
            ViewSpecifiedValue::Transition { value: existing },
            ViewSpecifiedValue::Transition { value: appended },
        ) => Some(ViewSpecifiedValue::Transition {
            value: existing.iter().chain(appended).copied().collect(),
        }),
        _ => None,
    }
}
