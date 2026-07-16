//! Single native computed-Style resolver for retained and control nodes.

mod axis;
mod cache;
mod environment;
mod matching;
mod provider;

use super::trace::ViewStyleTraceRecorder;
use super::{
    ComputedViewAxes, ComputedViewStyle, ComputedViewStyleBuilder, ViewBoxAxisMode,
    ViewBoxAxisRevision, ViewBoxAxisSeedSource, ViewComputedPropertyKind, ViewElementState,
    ViewInheritedBoxAxes, ViewInteractionSelector, ViewPropertyKind, ViewSpecifiedValue,
    ViewStyleApplication, ViewStyleApplicationTarget, ViewStyleContribution,
    ViewStyleContributionSource, ViewStylePatch, ViewStylePatchId, ViewStylePriority,
    ViewStyleProgram, ViewStyleScopeId, ViewStyleSheet, ViewStyleSheetId, ViewStyleSourceId,
    ViewStyleTokenId, ViewStyleTraceMode, ViewStyleTraceRejection,
};
use crate::{ViewElementKind, ViewMountId, ViewPartLocalName, ViewPartName};
use arcweft_presentation::appearance::{PresentationEnvironment, PresentationEnvironmentFieldSet};
use axis::{PendingViewStyleContribution, resolve_axes, resolve_contribution, resolve_transitions};
use cache::{ViewStyleCacheEntry, ViewStyleCacheKey, ViewStyleSelectionStamp, computed_revision};
use environment::projection_environment_usage;
use matching::{consume_selector_steps, scoped_ancestors, selector_matches};
use provider::ViewAxisProviderIndex;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use thiserror::Error;

pub use cache::{ViewInheritedStyleIdentity, ViewStyleResolveResult};
pub use environment::ViewStyleEnvironmentUsage;

/// Stable runtime identity used by the bounded computed-style cache.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ViewStyleNodeKey {
    mount: ViewMountId,
    path: Vec<u64>,
    instruction: u32,
}

/// Simultaneously active interaction predicates for one node snapshot.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ViewInteractionStateSet(u8);

/// Simultaneously active element-owned predicates for one node snapshot.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ViewElementStateSet(u8);

/// Typed facts available while matching one selector sequence.
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ViewStyleNodeFacts {
    element: Option<ViewElementKind>,
    implementation_part: Option<ViewPartLocalName>,
    exported_part: Option<ViewPartName>,
    interactions: ViewInteractionStateSet,
    element_states: ViewElementStateSet,
    active_scopes: Vec<ViewStyleScopeId>,
}

/// Explicit revisions that participate in computed-style cache identity.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ViewStyleRevisionSet {
    pub sheets: u64,
    pub patches: u64,
    pub tokens: u64,
    pub applications: u64,
    pub interactions: u64,
    pub containers: u64,
}

/// Hard limits for untrusted-but-decoded Style resolution work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewStyleResolverLimits {
    pub max_applications: usize,
    pub max_rules: usize,
    pub max_contributions: usize,
    pub max_token_inventory: usize,
    pub max_selector_steps: usize,
    pub max_token_depth: usize,
    pub max_cache_entries: usize,
    pub max_axis_invalidation_nodes: usize,
}

/// Inputs for resolving one concrete retained node.
/// The inherited axis snapshot is mandatory; ambient locale/text direction is not a fallback.
/// ```compile_fail
/// use arcweft_view::style::{ViewStyleResolveContext, ViewStyleTraceMode};
/// let _context = ViewStyleResolveContext {
///     node_key: todo!(), node: todo!(), ancestors: &[], applications: &[],
///     parent: None, parent_node_key: None, environment: todo!(), axis_provider_participation:
///     Default::default(), revisions: Default::default(), trace: ViewStyleTraceMode::Off,
/// };
/// ```
pub struct ViewStyleResolveContext<'a> {
    pub node_key: &'a ViewStyleNodeKey,
    pub node: &'a ViewStyleNodeFacts,
    /// Root-to-parent ancestry inside the currently visible View boundary.
    pub ancestors: &'a [ViewStyleNodeFacts],
    pub applications: &'a [ViewStyleApplication],
    pub parent: Option<&'a ComputedViewStyle>,
    pub parent_node_key: Option<&'a ViewStyleNodeKey>,
    pub inherited_axes: ViewInheritedBoxAxes,
    pub axis_provider_participation: ViewAxisProviderParticipation,
    pub environment: &'a PresentationEnvironment,
    pub revisions: ViewStyleRevisionSet,
    pub trace: ViewStyleTraceMode,
}

/// Whether one resolution owns retained provider state or is a projection of it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewAxisProviderParticipation {
    #[default]
    RetainedPrimary,
    ProjectionOnly,
}

/// One native resolver with deterministic FIFO cache eviction.
#[derive(Clone, Debug)]
pub struct ViewStyleResolver {
    limits: ViewStyleResolverLimits,
    cache: BTreeMap<ViewStyleCacheKey, ViewStyleCacheEntry>,
    cache_order: VecDeque<ViewStyleCacheKey>,
    axis_providers: ViewAxisProviderIndex,
}

/// Failure to resolve typed data within the configured hard bounds.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewStyleResolveError {
    #[error("Style application count {actual} exceeds the runtime maximum {limit}")]
    ApplicationBudget { actual: usize, limit: usize },
    #[error("Style rule count exceeds the runtime maximum {limit}")]
    RuleBudget { limit: usize },
    #[error("Style contribution count exceeds the runtime maximum {limit}")]
    ContributionBudget { limit: usize },
    #[error("Style token inventory exceeds the runtime maximum {limit}")]
    TokenInventoryBudget { limit: usize },
    #[error("Style selector matching exceeds the runtime maximum {limit}")]
    SelectorBudget { limit: usize },
    #[error("Style selector specificity exceeds the exact runtime maximum {limit}")]
    SelectorSpecificityBudget { limit: usize },
    #[error("Style token resolution exceeds the runtime maximum {limit}")]
    TokenBudget { limit: usize },
    #[error("Style application references missing sheet {0:?}")]
    UnknownSheet(ViewStyleSheetId),
    #[error("Style application references missing patch {0:?}")]
    UnknownPatch(ViewStylePatchId),
    #[error("Style declaration references missing token {0:?}")]
    UnknownToken(ViewStyleTokenId),
    #[error("inline Style token {0:?} is not uniquely owned by one named sheet")]
    AmbiguousInlineToken(ViewStyleTokenId),
    #[error(
        "logical property {authored_property:?} overflows while resolving to {resolved_property:?} in {mode:?}"
    )]
    AxisValueOverflow {
        style_source: ViewStyleSourceId,
        authored_property: ViewPropertyKind,
        resolved_property: ViewComputedPropertyKind,
        mode: ViewBoxAxisMode,
    },
    #[error("root axis provider {node:?} cannot use seed source {seed_source:?}")]
    AxisProviderInvalidRootSeed {
        node: ViewStyleNodeKey,
        seed_source: ViewBoxAxisSeedSource,
    },
    #[error("child axis provider {node:?} cannot use seed source {seed_source:?}")]
    AxisProviderInvalidChildSeed {
        node: ViewStyleNodeKey,
        seed_source: ViewBoxAxisSeedSource,
    },
    #[error("axis provider {node:?} must supply both parent Style and parent node key, or neither")]
    AxisProviderParentShape { node: ViewStyleNodeKey },
    #[error("axis provider {node:?} references missing parent {parent:?}")]
    AxisProviderMissingParent {
        node: ViewStyleNodeKey,
        parent: ViewStyleNodeKey,
    },
    #[error("axis provider edge from {node:?} to {parent:?} forms a cycle")]
    AxisProviderCycle {
        node: ViewStyleNodeKey,
        parent: ViewStyleNodeKey,
    },
    #[error(
        "axis provider {node:?} inherited mode {actual:?} does not match parent {parent:?} mode {expected:?}"
    )]
    AxisProviderModeMismatch {
        node: ViewStyleNodeKey,
        parent: ViewStyleNodeKey,
        expected: ViewBoxAxisMode,
        actual: ViewBoxAxisMode,
    },
    #[error(
        "axis provider {node:?} inherited revision {actual:?} does not match parent {parent:?} revision {expected:?}"
    )]
    AxisProviderRevisionMismatch {
        node: ViewStyleNodeKey,
        parent: ViewStyleNodeKey,
        expected: ViewBoxAxisRevision,
        actual: ViewBoxAxisRevision,
    },
    #[error("axis provider invalidation from {node:?} exceeds the descendant maximum {limit}")]
    AxisProviderInvalidationBudget {
        node: ViewStyleNodeKey,
        limit: usize,
    },
    #[error("axis provider child index edge {parent:?} -> {child:?} is corrupt")]
    AxisProviderCorruptChildIndex {
        parent: ViewStyleNodeKey,
        child: ViewStyleNodeKey,
    },
}

#[derive(Default)]
struct ResolveBudget {
    rules: usize,
    selector_steps: usize,
    selector_exhausted: bool,
    environment_selection: PresentationEnvironmentFieldSet,
}

#[derive(Clone, Debug)]
enum InlineTokenOwner {
    Unique(ViewStyleSheetId),
    Ambiguous,
}

#[derive(Clone, Debug, Default)]
struct InlineTokenOwners(BTreeMap<ViewStyleTokenId, InlineTokenOwner>);

impl Default for ViewStyleResolverLimits {
    fn default() -> Self {
        Self {
            max_applications: 4_096,
            max_rules: 65_536,
            max_contributions: 262_144,
            max_token_inventory: 65_536,
            max_selector_steps: 262_144,
            max_token_depth: ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH,
            max_cache_entries: 1_024,
            max_axis_invalidation_nodes: 65_536,
        }
    }
}

impl InlineTokenOwners {
    fn new(program: &ViewStyleProgram, max_tokens: usize) -> Result<Self, ViewStyleResolveError> {
        let mut token_count = 0_usize;
        let mut owners = BTreeMap::new();
        for sheet in program.sheets() {
            token_count = token_count.saturating_add(sheet.tokens().len());
            if token_count > max_tokens {
                return Err(ViewStyleResolveError::TokenInventoryBudget { limit: max_tokens });
            }
            for token in sheet.tokens() {
                owners
                    .entry(token.id().clone())
                    .and_modify(|owner| *owner = InlineTokenOwner::Ambiguous)
                    .or_insert_with(|| InlineTokenOwner::Unique(sheet.id().clone()));
            }
        }
        Ok(Self(owners))
    }

    fn unique_owner(
        &self,
        token: &ViewStyleTokenId,
    ) -> Result<&ViewStyleSheetId, ViewStyleResolveError> {
        match self.0.get(token) {
            Some(InlineTokenOwner::Unique(sheet)) => Ok(sheet),
            Some(InlineTokenOwner::Ambiguous) => {
                Err(ViewStyleResolveError::AmbiguousInlineToken(token.clone()))
            }
            None => Err(ViewStyleResolveError::UnknownToken(token.clone())),
        }
    }
}

impl ViewStyleNodeKey {
    pub const fn new(mount: ViewMountId, path: Vec<u64>, instruction: u32) -> Self {
        Self {
            mount,
            path,
            instruction,
        }
    }

    pub const fn mount(&self) -> ViewMountId {
        self.mount
    }

    pub fn path(&self) -> &[u64] {
        &self.path
    }

    pub const fn instruction(&self) -> u32 {
        self.instruction
    }
}

impl ViewInteractionStateSet {
    pub const fn contains(self, state: ViewInteractionSelector) -> bool {
        self.0 & interaction_bit(state) != 0
    }

    #[must_use]
    pub const fn with(mut self, state: ViewInteractionSelector) -> Self {
        self.0 |= interaction_bit(state);
        self
    }
}

impl ViewElementStateSet {
    pub const fn contains(self, state: ViewElementState) -> bool {
        self.0 & element_state_bit(state) != 0
    }

    #[must_use]
    pub const fn with(mut self, state: ViewElementState) -> Self {
        self.0 |= element_state_bit(state);
        self
    }
}

impl ViewStyleNodeFacts {
    pub const fn new(element: Option<ViewElementKind>) -> Self {
        Self {
            element,
            implementation_part: None,
            exported_part: None,
            interactions: ViewInteractionStateSet(0),
            element_states: ViewElementStateSet(0),
            active_scopes: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_parts(
        mut self,
        implementation_part: Option<ViewPartLocalName>,
        exported_part: Option<ViewPartName>,
    ) -> Self {
        self.implementation_part = implementation_part;
        self.exported_part = exported_part;
        self
    }

    #[must_use]
    pub const fn with_interactions(mut self, interactions: ViewInteractionStateSet) -> Self {
        self.interactions = interactions;
        self
    }

    #[must_use]
    pub const fn with_element_states(mut self, element_states: ViewElementStateSet) -> Self {
        self.element_states = element_states;
        self
    }

    #[must_use]
    pub fn with_active_scopes(mut self, active_scopes: Vec<ViewStyleScopeId>) -> Self {
        self.active_scopes = active_scopes;
        self
    }

    pub const fn element(&self) -> Option<ViewElementKind> {
        self.element
    }

    pub const fn implementation_part(&self) -> Option<&ViewPartLocalName> {
        self.implementation_part.as_ref()
    }

    pub const fn exported_part(&self) -> Option<&ViewPartName> {
        self.exported_part.as_ref()
    }

    pub const fn interactions(&self) -> ViewInteractionStateSet {
        self.interactions
    }

    pub const fn element_states(&self) -> ViewElementStateSet {
        self.element_states
    }

    pub fn active_scopes(&self) -> &[ViewStyleScopeId] {
        &self.active_scopes
    }
}

impl ViewStyleResolver {
    pub fn new(limits: ViewStyleResolverLimits) -> Self {
        Self {
            limits,
            cache: BTreeMap::new(),
            cache_order: VecDeque::new(),
            axis_providers: ViewAxisProviderIndex::default(),
        }
    }

    pub const fn limits(&self) -> ViewStyleResolverLimits {
        self.limits
    }

    pub fn resolve(
        &mut self,
        program: &ViewStyleProgram,
        context: &ViewStyleResolveContext<'_>,
    ) -> Result<ViewStyleResolveResult, ViewStyleResolveError> {
        let mut trace = ViewStyleTraceRecorder::new(context.trace);
        let (contributions, environment_selection) =
            self.collect_contributions(program, context, &mut trace)?;
        let resolved_axes = resolve_axes(context.node_key, context.inherited_axes, &contributions);
        let provider_update = self.axis_providers.prepare(
            context,
            &resolved_axes.axes,
            resolved_axes.local_barrier,
            self.limits.max_axis_invalidation_nodes,
        )?;
        let cache_key = ViewStyleCacheKey::new(context, &resolved_axes.axes);
        let parent_identity = context
            .parent
            .map(ViewInheritedStyleIdentity::from_computed);
        if context.trace != ViewStyleTraceMode::Full
            && let Some(cached) = self.cache.get(&cache_key).cloned()
            && cached.selection_stamp.matches(*context.environment)
            && cached.parent_identity == parent_identity
        {
            let trace = trace.finish(&cached.computed);
            self.commit_provider_update(provider_update);
            self.insert_cache(cache_key, cached.clone());
            return Ok(ViewStyleResolveResult {
                computed: cached.computed,
                environment_usage: cached.environment_usage,
                trace,
                cache_hit: true,
            });
        }

        let selection_stamp =
            ViewStyleSelectionStamp::new(environment_selection, *context.environment);
        let revision = computed_revision(&cache_key, selection_stamp, parent_identity.as_ref());
        let mode = resolved_axes.axes.mode();
        let mut builder = ComputedViewStyleBuilder::inherit(context.parent, resolved_axes.axes);
        let mut resolved_contribution_count = 0_usize;
        for contribution in contributions {
            if contribution.property.is_axis_context() {
                continue;
            }
            if contribution.property.is_axis_dependent() {
                builder.include_axis_usage(contribution.property.axis_usage());
            }
            let contributions = resolve_contribution(contribution, mode)?;
            if contributions.len()
                > self
                    .limits
                    .max_contributions
                    .saturating_sub(resolved_contribution_count)
            {
                return Err(ViewStyleResolveError::ContributionBudget {
                    limit: self.limits.max_contributions,
                });
            }
            resolved_contribution_count += contributions.len();
            for contribution in contributions {
                apply_contribution(&mut builder, contribution, &mut trace);
            }
        }
        let (transitions, usage) =
            resolve_transitions(builder.value(ViewPropertyKind::Transition), mode);
        builder.include_axis_usage(usage);
        builder.set_transitions(transitions);
        let computed = Arc::new(builder.finish(revision));
        let environment_usage = ViewStyleEnvironmentUsage::new(
            environment_selection,
            projection_environment_usage(&computed),
        );
        let trace = trace.finish(&computed);
        self.commit_provider_update(provider_update);
        if context.trace != ViewStyleTraceMode::Full {
            self.insert_cache(
                cache_key,
                ViewStyleCacheEntry {
                    computed: Arc::clone(&computed),
                    environment_usage,
                    selection_stamp,
                    parent_identity,
                },
            );
        }
        Ok(ViewStyleResolveResult {
            computed,
            environment_usage,
            trace,
            cache_hit: false,
        })
    }

    fn collect_contributions(
        &self,
        program: &ViewStyleProgram,
        context: &ViewStyleResolveContext<'_>,
        trace: &mut ViewStyleTraceRecorder,
    ) -> Result<
        (
            Vec<PendingViewStyleContribution>,
            PresentationEnvironmentFieldSet,
        ),
        ViewStyleResolveError,
    > {
        if context.applications.len() > self.limits.max_applications {
            return Err(ViewStyleResolveError::ApplicationBudget {
                actual: context.applications.len(),
                limit: self.limits.max_applications,
            });
        }
        let inline_token_owners = InlineTokenOwners::new(program, self.limits.max_token_inventory)?;
        let mut budget = ResolveBudget::default();
        let mut contributions = Vec::new();
        for application in context.applications {
            match application.target() {
                ViewStyleApplicationTarget::Named { sheet } => {
                    let sheet = program
                        .sheet(sheet)
                        .ok_or_else(|| ViewStyleResolveError::UnknownSheet(sheet.clone()))?;
                    self.apply_sheet(
                        program,
                        sheet,
                        application,
                        context,
                        trace,
                        &mut budget,
                        &mut contributions,
                    )?;
                }
                ViewStyleApplicationTarget::Inline { patch } => {
                    let patch_resource = program
                        .patch(*patch)
                        .ok_or(ViewStyleResolveError::UnknownPatch(*patch))?;
                    self.apply_patch(
                        program,
                        &inline_token_owners,
                        patch_resource,
                        application,
                        context,
                        trace,
                        &mut contributions,
                    )?;
                }
            }
        }
        contributions.sort_by_key(|contribution| contribution.priority);
        Ok((contributions, budget.environment_selection))
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "patch resolution keeps token ownership, application priority, node applicability, trace, and output explicit"
    )]
    fn apply_patch(
        &self,
        program: &ViewStyleProgram,
        token_owners: &InlineTokenOwners,
        patch: &ViewStylePatch,
        application: &ViewStyleApplication,
        context: &ViewStyleResolveContext<'_>,
        trace: &mut ViewStyleTraceRecorder,
        contributions: &mut Vec<PendingViewStyleContribution>,
    ) -> Result<(), ViewStyleResolveError> {
        for (declaration_order, declaration) in patch.declarations().iter().enumerate() {
            if context
                .node
                .element()
                .is_some_and(|element| !declaration.property().applies_to(element))
            {
                trace.patch_rejected(
                    patch.id(),
                    declaration.source(),
                    ViewStyleTraceRejection::PropertyNotApplicable,
                );
                continue;
            }
            let value = resolve_inline_value(
                program,
                token_owners,
                declaration.value(),
                self.limits.max_token_depth,
            )?;
            push_contribution(
                contributions,
                PendingViewStyleContribution {
                    property: declaration.property(),
                    value,
                    operation: declaration.op(),
                    priority: ViewStylePriority::new(
                        application.scope_depth(),
                        application.application_order(),
                        0,
                        0,
                        0,
                        u32::try_from(declaration_order).unwrap_or(u32::MAX),
                    ),
                    source: ViewStyleContributionSource::Patch {
                        patch: patch.id(),
                        declaration: declaration.source(),
                    },
                },
                self.limits.max_contributions,
            )?;
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the resolver keeps program, application, node context, output, trace, and hard budget explicit"
    )]
    fn apply_sheet(
        &self,
        program: &ViewStyleProgram,
        sheet: &ViewStyleSheet,
        application: &ViewStyleApplication,
        context: &ViewStyleResolveContext<'_>,
        trace: &mut ViewStyleTraceRecorder,
        budget: &mut ResolveBudget,
        contributions: &mut Vec<PendingViewStyleContribution>,
    ) -> Result<(), ViewStyleResolveError> {
        let scoped_ancestors = scoped_ancestors(context.ancestors, application.scope());
        for rule in sheet.rules() {
            budget.rules = budget.rules.saturating_add(1);
            if budget.rules > self.limits.max_rules {
                return Err(ViewStyleResolveError::RuleBudget {
                    limit: self.limits.max_rules,
                });
            }
            if !consume_selector_steps(
                budget,
                self.limits.max_selector_steps,
                rule.selector().sequences().len(),
            ) {
                return Err(ViewStyleResolveError::SelectorBudget {
                    limit: self.limits.max_selector_steps,
                });
            }
            let specificity = rule.selector().specificity().ok_or(
                ViewStyleResolveError::SelectorSpecificityBudget {
                    limit: usize::from(u16::MAX),
                },
            )?;
            let matched = selector_matches(
                rule.selector(),
                scoped_ancestors,
                context.node,
                application,
                budget,
                self.limits.max_selector_steps,
            );
            if budget.selector_exhausted {
                return Err(ViewStyleResolveError::SelectorBudget {
                    limit: self.limits.max_selector_steps,
                });
            }
            if let Err(reason) = matched {
                trace.rule_rejected(sheet.id(), rule.source_order(), reason);
                continue;
            }
            if let Some(condition) = rule.environment() {
                let environment_match = condition.matches(*context.environment);
                budget.environment_selection = budget
                    .environment_selection
                    .union(environment_match.usage());
                if !environment_match.matched() {
                    trace.rule_rejected(
                        sheet.id(),
                        rule.source_order(),
                        ViewStyleTraceRejection::EnvironmentMismatch,
                    );
                    continue;
                }
            }
            for (declaration_order, declaration) in rule.declarations().iter().enumerate() {
                if context
                    .node
                    .element()
                    .is_some_and(|element| !declaration.property().applies_to(element))
                {
                    trace.rule_rejected(
                        sheet.id(),
                        rule.source_order(),
                        ViewStyleTraceRejection::PropertyNotApplicable,
                    );
                    continue;
                }
                let value = resolve_sheet_value(
                    program,
                    sheet.id(),
                    declaration.value(),
                    self.limits.max_token_depth,
                )?;
                let contribution = PendingViewStyleContribution {
                    property: declaration.property(),
                    value,
                    operation: declaration.op(),
                    priority: ViewStylePriority::new(
                        application.scope_depth(),
                        application.application_order(),
                        specificity.predicates(),
                        specificity.elements(),
                        rule.source_order(),
                        u32::try_from(declaration_order).unwrap_or(u32::MAX),
                    ),
                    source: ViewStyleContributionSource::Sheet {
                        sheet: sheet.id().clone(),
                        rule: rule.source(),
                        declaration: declaration.source(),
                    },
                };
                push_contribution(contributions, contribution, self.limits.max_contributions)?;
            }
        }
        Ok(())
    }
}

fn apply_contribution(
    builder: &mut ComputedViewStyleBuilder,
    contribution: ViewStyleContribution,
    trace: &mut ViewStyleTraceRecorder,
) {
    if !trace.is_full() {
        builder.apply(contribution);
        return;
    }
    let property = contribution.property();
    let priority = contribution.priority();
    let source = contribution.source().clone();
    let accepted = builder.apply(contribution);
    trace.contribution(property, priority, source, accepted);
}

fn push_contribution(
    contributions: &mut Vec<PendingViewStyleContribution>,
    contribution: PendingViewStyleContribution,
    limit: usize,
) -> Result<(), ViewStyleResolveError> {
    if contributions.len() >= limit {
        return Err(ViewStyleResolveError::ContributionBudget { limit });
    }
    contributions.push(contribution);
    Ok(())
}

fn resolve_sheet_value(
    program: &ViewStyleProgram,
    sheet: &ViewStyleSheetId,
    value: &ViewSpecifiedValue,
    max_depth: usize,
) -> Result<ViewSpecifiedValue, ViewStyleResolveError> {
    let mut value = value;
    for _ in 0..=max_depth {
        let ViewSpecifiedValue::Token { token, .. } = value else {
            return Ok(value.clone());
        };
        value = program
            .resolve_token(sheet, token)
            .ok_or_else(|| ViewStyleResolveError::UnknownToken(token.clone()))?
            .value();
    }
    Err(ViewStyleResolveError::TokenBudget { limit: max_depth })
}

fn resolve_inline_value(
    program: &ViewStyleProgram,
    owners: &InlineTokenOwners,
    value: &ViewSpecifiedValue,
    max_depth: usize,
) -> Result<ViewSpecifiedValue, ViewStyleResolveError> {
    let ViewSpecifiedValue::Token { token, .. } = value else {
        return Ok(value.clone());
    };
    resolve_sheet_value(program, owners.unique_owner(token)?, value, max_depth)
}

const fn interaction_bit(state: ViewInteractionSelector) -> u8 {
    match state {
        ViewInteractionSelector::Hovered => 1 << 0,
        ViewInteractionSelector::Focused => 1 << 1,
        ViewInteractionSelector::Pressed => 1 << 2,
        ViewInteractionSelector::Disabled => 1 << 3,
    }
}

const fn element_state_bit(state: ViewElementState) -> u8 {
    match state {
        ViewElementState::FocusVisible => 1 << 0,
        ViewElementState::ReadOnly => 1 << 1,
        ViewElementState::Invalid => 1 << 2,
        ViewElementState::Composing => 1 << 3,
        ViewElementState::PlaceholderShown => 1 << 4,
    }
}
