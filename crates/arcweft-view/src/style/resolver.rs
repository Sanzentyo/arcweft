//! Single native computed-Style resolver for retained and control nodes.

mod axis;
mod provider;

use super::{
    ComputedViewAxes, ComputedViewStyle, ComputedViewStyleBuilder, ComputedViewStyleRevision,
    ViewBoxAxisMode, ViewBoxAxisRevision, ViewBoxAxisSeedSource, ViewComputedPropertyKind,
    ViewElementState, ViewEnvironmentPredicate, ViewInheritedBoxAxes, ViewInteractionSelector,
    ViewPartName, ViewPropertyKind, ViewSpecifiedValue, ViewStyleApplication,
    ViewStyleApplicationTarget, ViewStyleCombinator, ViewStyleComparison, ViewStyleContribution,
    ViewStyleContributionSource, ViewStylePatch, ViewStylePatchId, ViewStylePredicate,
    ViewStylePriority, ViewStyleProgram, ViewStyleScopeId, ViewStyleSelector,
    ViewStyleSelectorSequence, ViewStyleSheet, ViewStyleSheetId, ViewStyleSourceId,
    ViewStyleTokenId, ViewStyleTrace, ViewStyleTraceEntry, ViewStyleTraceMode,
    ViewStyleTraceRejection,
};
use crate::{ViewElementKind, ViewMountId};
use arcweft_presentation::appearance::{ColorScheme, ContrastPreference, PresentationEnvironment};
use axis::{PendingViewStyleContribution, resolve_axes, resolve_contribution, resolve_transitions};
use provider::{ViewAxisProviderIndex, ViewAxisProviderUpdatePlan};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

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
    implementation_part: Option<ViewPartName>,
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

/// Computed result and optional deterministic trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewStyleResolution {
    computed: ComputedViewStyle,
    trace: ViewStyleTrace,
    cache_hit: bool,
}

/// One native resolver with deterministic FIFO cache eviction.
#[derive(Clone, Debug)]
pub struct ViewStyleResolver {
    limits: ViewStyleResolverLimits,
    cache: BTreeMap<ViewStyleCacheKey, ComputedViewStyle>,
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

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ViewStyleCacheKey {
    node: ViewStyleNodeKey,
    facts: Vec<ViewStyleNodeFacts>,
    revisions: ViewStyleRevisionSet,
    parent_revision: Option<ComputedViewStyleRevision>,
    color_scheme: u8,
    contrast: u8,
    reduce_motion: bool,
    text_scale: u16,
    locale: Option<String>,
    environment_revision: u64,
    axis_mode: u8,
    axis_revision: ViewBoxAxisRevision,
}

#[derive(Default)]
struct ResolveBudget {
    rules: usize,
    selector_steps: usize,
    selector_exhausted: bool,
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

impl Default for ViewStyleResolver {
    fn default() -> Self {
        Self::new(ViewStyleResolverLimits::default())
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
        implementation_part: Option<ViewPartName>,
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

    pub const fn implementation_part(&self) -> Option<&ViewPartName> {
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

impl ViewStyleResolution {
    pub const fn computed(&self) -> &ComputedViewStyle {
        &self.computed
    }

    pub const fn trace(&self) -> &ViewStyleTrace {
        &self.trace
    }

    pub const fn cache_hit(&self) -> bool {
        self.cache_hit
    }

    pub fn into_computed(self) -> ComputedViewStyle {
        self.computed
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

    pub fn resolve(
        &mut self,
        program: &ViewStyleProgram,
        context: &ViewStyleResolveContext<'_>,
    ) -> Result<ViewStyleResolution, ViewStyleResolveError> {
        let mut trace = ViewStyleTrace::default();
        let contributions = self.collect_contributions(program, context, &mut trace)?;
        let resolved_axes = resolve_axes(context.node_key, context.inherited_axes, &contributions);
        let provider_update = self.axis_providers.prepare(
            context,
            &resolved_axes.axes,
            resolved_axes.local_barrier,
            self.limits.max_axis_invalidation_nodes,
        )?;
        let cache_key = ViewStyleCacheKey::new(context, &resolved_axes.axes);
        if context.trace != ViewStyleTraceMode::Full
            && let Some(computed) = self.cache.get(&cache_key).cloned()
        {
            trace.finish_winners(context.trace, &computed);
            self.commit_provider_update(provider_update);
            self.insert_cache(cache_key, computed.clone());
            return Ok(ViewStyleResolution {
                computed,
                trace,
                cache_hit: true,
            });
        }

        let revision = computed_revision(&cache_key);
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
                apply_contribution(&mut builder, contribution, context.trace, &mut trace);
            }
        }
        let (transitions, usage) =
            resolve_transitions(builder.value(ViewPropertyKind::Transition), mode);
        builder.include_axis_usage(usage);
        builder.set_transitions(transitions);
        let computed = builder.finish(revision);
        trace.finish_winners(context.trace, &computed);
        self.commit_provider_update(provider_update);
        if context.trace != ViewStyleTraceMode::Full {
            self.insert_cache(cache_key, computed.clone());
        }
        Ok(ViewStyleResolution {
            computed,
            trace,
            cache_hit: false,
        })
    }

    fn collect_contributions(
        &self,
        program: &ViewStyleProgram,
        context: &ViewStyleResolveContext<'_>,
        trace: &mut ViewStyleTrace,
    ) -> Result<Vec<PendingViewStyleContribution>, ViewStyleResolveError> {
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
        Ok(contributions)
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
        trace: &mut ViewStyleTrace,
        contributions: &mut Vec<PendingViewStyleContribution>,
    ) -> Result<(), ViewStyleResolveError> {
        for (declaration_order, declaration) in patch.declarations().iter().enumerate() {
            if context
                .node
                .element()
                .is_some_and(|element| !declaration.property().applies_to(element))
            {
                trace.push(
                    context.trace,
                    ViewStyleTraceEntry::PatchRejected {
                        patch: patch.id(),
                        declaration: declaration.source(),
                        reason: ViewStyleTraceRejection::PropertyNotApplicable,
                    },
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
        trace: &mut ViewStyleTrace,
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
                context.environment,
                budget,
                self.limits.max_selector_steps,
            );
            if budget.selector_exhausted {
                return Err(ViewStyleResolveError::SelectorBudget {
                    limit: self.limits.max_selector_steps,
                });
            }
            if let Err(reason) = matched {
                trace.push(
                    context.trace,
                    ViewStyleTraceEntry::RuleRejected {
                        sheet: sheet.id().clone(),
                        source_order: rule.source_order(),
                        reason,
                    },
                );
                continue;
            }
            for (declaration_order, declaration) in rule.declarations().iter().enumerate() {
                if context
                    .node
                    .element()
                    .is_some_and(|element| !declaration.property().applies_to(element))
                {
                    trace.push(
                        context.trace,
                        ViewStyleTraceEntry::RuleRejected {
                            sheet: sheet.id().clone(),
                            source_order: rule.source_order(),
                            reason: ViewStyleTraceRejection::PropertyNotApplicable,
                        },
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

    fn insert_cache(&mut self, key: ViewStyleCacheKey, computed: ComputedViewStyle) {
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
        self.cache.insert(key, computed);
    }

    fn commit_provider_update(&mut self, update: Option<ViewAxisProviderUpdatePlan>) {
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

impl ViewStyleCacheKey {
    fn new(context: &ViewStyleResolveContext<'_>, axes: &ComputedViewAxes) -> Self {
        let mut facts = context.ancestors.to_vec();
        facts.push(context.node.clone());
        Self {
            node: context.node_key.clone(),
            facts,
            revisions: context.revisions,
            parent_revision: context.parent.map(ComputedViewStyle::revision),
            color_scheme: color_scheme_rank(context.environment.color_scheme()),
            contrast: contrast_rank(context.environment.contrast()),
            reduce_motion: context.environment.reduce_motion(),
            text_scale: context.environment.text_scale().value(),
            locale: context
                .environment
                .locale()
                .map(|locale| locale.as_str().to_owned()),
            environment_revision: context.environment.revision().0,
            axis_mode: axes.mode().canonical_tag(),
            axis_revision: axes.revision(),
        }
    }
}

fn apply_contribution(
    builder: &mut ComputedViewStyleBuilder,
    contribution: ViewStyleContribution,
    trace_mode: ViewStyleTraceMode,
    trace: &mut ViewStyleTrace,
) {
    let property = contribution.property();
    let priority = contribution.priority();
    let source = contribution.source().clone();
    let accepted = builder.apply(contribution);
    trace.push(
        trace_mode,
        ViewStyleTraceEntry::Contribution {
            property,
            priority,
            source,
            accepted,
        },
    );
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

fn selector_matches(
    selector: &ViewStyleSelector,
    ancestors: &[ViewStyleNodeFacts],
    node: &ViewStyleNodeFacts,
    application: &ViewStyleApplication,
    environment: &PresentationEnvironment,
    budget: &mut ResolveBudget,
    selector_limit: usize,
) -> Result<(), ViewStyleTraceRejection> {
    let sequences = selector.sequences();
    let last_index = sequences
        .len()
        .checked_sub(1)
        .ok_or(ViewStyleTraceRejection::SelectorMismatch)?;
    if application.boundary().is_nested_view_boundary() {
        let target = &sequences[last_index];
        let targets_inherited_root = application.boundary().allows_inherited_root();
        let targets_exported_part = application.boundary().is_exported_part()
            && target.part().is_some()
            && application
                .boundary()
                .selector_part(
                    node.implementation_part()
                        .map(|part| part.public_id().as_str()),
                    node.exported_part().map(|part| part.public_id().as_str()),
                )
                .is_some();
        // A public part is one target capability, not permission to expose the
        // private child ancestry. Until facts carry explicit boundary segments,
        // structural selectors stop at every crossed View boundary.
        if !(targets_inherited_root || targets_exported_part) || last_index != 0 {
            return Err(ViewStyleTraceRejection::BoundaryTraversalBlocked);
        }
    }
    match_sequence(
        &sequences[last_index],
        node,
        Some(application),
        environment,
        budget,
        selector_limit,
    )?;
    let mut ancestor_limit = ancestors.len();
    for index in (0..last_index).rev() {
        let sequence = &sequences[index];
        match sequences[index + 1]
            .relation_to_previous()
            .unwrap_or(ViewStyleCombinator::Descendant)
        {
            ViewStyleCombinator::Child => {
                ancestor_limit = ancestor_limit
                    .checked_sub(1)
                    .ok_or(ViewStyleTraceRejection::SelectorMismatch)?;
                match_sequence(
                    sequence,
                    &ancestors[ancestor_limit],
                    application
                        .boundary()
                        .is_nested_view_boundary()
                        .then_some(application),
                    environment,
                    budget,
                    selector_limit,
                )?;
            }
            ViewStyleCombinator::Descendant => {
                let mut matched = None;
                for candidate in (0..ancestor_limit).rev() {
                    let result = match_sequence(
                        sequence,
                        &ancestors[candidate],
                        application
                            .boundary()
                            .is_nested_view_boundary()
                            .then_some(application),
                        environment,
                        budget,
                        selector_limit,
                    );
                    if budget.selector_exhausted {
                        return Err(ViewStyleTraceRejection::BoundaryTraversalBlocked);
                    }
                    if result.is_ok() {
                        matched = Some(candidate);
                        break;
                    }
                }
                ancestor_limit = matched.ok_or(ViewStyleTraceRejection::SelectorMismatch)?;
            }
        }
    }
    Ok(())
}

fn scoped_ancestors(
    ancestors: &[ViewStyleNodeFacts],
    scope: ViewStyleScopeId,
) -> &[ViewStyleNodeFacts] {
    ancestors
        .iter()
        .position(|facts| facts.active_scopes().contains(&scope))
        .map_or(&[][..], |scope_root| &ancestors[scope_root..])
}

fn match_sequence(
    sequence: &ViewStyleSelectorSequence,
    node: &ViewStyleNodeFacts,
    application: Option<&ViewStyleApplication>,
    environment: &PresentationEnvironment,
    budget: &mut ResolveBudget,
    selector_limit: usize,
) -> Result<(), ViewStyleTraceRejection> {
    if !consume_selector_step(budget, selector_limit) {
        return Err(ViewStyleTraceRejection::BoundaryTraversalBlocked);
    }
    if sequence
        .element()
        .is_some_and(|element| node.element() != Some(element))
    {
        return Err(ViewStyleTraceRejection::SelectorMismatch);
    }
    let visible_part = application.map_or_else(
        || node.implementation_part(),
        |application| {
            application
                .boundary()
                .selector_part(
                    node.implementation_part()
                        .map(|part| part.public_id().as_str()),
                    node.exported_part().map(|part| part.public_id().as_str()),
                )
                .and_then(|visible| {
                    node.implementation_part()
                        .filter(|part| part.public_id().as_str() == visible)
                        .or_else(|| {
                            node.exported_part()
                                .filter(|part| part.public_id().as_str() == visible)
                        })
                })
        },
    );
    if sequence
        .part()
        .is_some_and(|part| visible_part != Some(part))
    {
        return Err(ViewStyleTraceRejection::SelectorMismatch);
    }
    for predicate in sequence.predicates() {
        if !consume_selector_step(budget, selector_limit) {
            return Err(ViewStyleTraceRejection::BoundaryTraversalBlocked);
        }
        match predicate {
            ViewStylePredicate::Interaction(state) if !node.interactions().contains(*state) => {
                return Err(ViewStyleTraceRejection::InteractionStateMismatch);
            }
            ViewStylePredicate::ElementState(state) if !node.element_states().contains(*state) => {
                return Err(ViewStyleTraceRejection::ElementStateMismatch);
            }
            ViewStylePredicate::Environment(predicate)
                if !environment_matches(*predicate, environment) =>
            {
                return Err(ViewStyleTraceRejection::EnvironmentMismatch);
            }
            ViewStylePredicate::Container(_) => {
                return Err(ViewStyleTraceRejection::ContainerFactsUnavailable);
            }
            ViewStylePredicate::Interaction(_)
            | ViewStylePredicate::ElementState(_)
            | ViewStylePredicate::Environment(_) => {}
        }
    }
    Ok(())
}

fn consume_selector_step(budget: &mut ResolveBudget, limit: usize) -> bool {
    consume_selector_steps(budget, limit, 1)
}

fn consume_selector_steps(budget: &mut ResolveBudget, limit: usize, steps: usize) -> bool {
    if steps > limit.saturating_sub(budget.selector_steps) {
        budget.selector_exhausted = true;
        false
    } else {
        budget.selector_steps += steps;
        true
    }
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

const fn environment_matches(
    predicate: ViewEnvironmentPredicate,
    environment: &PresentationEnvironment,
) -> bool {
    match predicate {
        ViewEnvironmentPredicate::ReduceMotion(expected) => environment.reduce_motion() == expected,
        ViewEnvironmentPredicate::ColorScheme(comparison, expected) => compare_u16(
            comparison,
            color_scheme_rank(environment.color_scheme()) as u16,
            color_scheme_rank(expected) as u16,
        ),
        ViewEnvironmentPredicate::Contrast(comparison, expected) => compare_u16(
            comparison,
            contrast_rank(environment.contrast()) as u16,
            contrast_rank(expected) as u16,
        ),
        ViewEnvironmentPredicate::TextScale(comparison, expected) => compare_u16(
            comparison,
            environment.text_scale().value(),
            expected.value(),
        ),
    }
}

const fn compare_u16(comparison: ViewStyleComparison, actual: u16, expected: u16) -> bool {
    match comparison {
        ViewStyleComparison::Equal => actual == expected,
        ViewStyleComparison::NotEqual => actual != expected,
        ViewStyleComparison::Less => actual < expected,
        ViewStyleComparison::LessOrEqual => actual <= expected,
        ViewStyleComparison::Greater => actual > expected,
        ViewStyleComparison::GreaterOrEqual => actual >= expected,
    }
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

fn computed_revision(key: &ViewStyleCacheKey) -> ComputedViewStyleRevision {
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
        key.environment_revision,
        u64::from(key.text_scale),
        u64::from(key.axis_mode),
        key.axis_revision.value(),
    ] {
        revision ^= value;
        revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
    }
    revision ^= u64::from(key.parent_revision.is_some());
    revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
    if let Some(parent_revision) = key.parent_revision {
        revision ^= parent_revision.value();
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
            facts.implementation_part.as_ref(),
            facts.exported_part.as_ref(),
        ] {
            revision ^= u64::from(part.is_some());
            revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
            if let Some(part) = part {
                for byte in part.public_id().as_str().bytes() {
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
    revision ^= u64::from(key.color_scheme)
        | (u64::from(key.contrast) << 8)
        | (u64::from(key.reduce_motion) << 16);
    if let Some(locale) = &key.locale {
        for byte in locale.bytes() {
            revision ^= u64::from(byte);
            revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    ComputedViewStyleRevision::new(revision)
}
