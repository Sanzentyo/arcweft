//! Live native Style resolution for executed View node producers.

mod axis_seed;
mod consumer;
mod environment;

use super::PlayerFrameError;
use crate::input::InputController;
use arcweft_bundle::resource_codec::ViewRuntimeNodeStyle;
use arcweft_id::PublicId;
use arcweft_presentation::appearance::{PresentationEnvironment, SystemPaletteSet};
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::interaction::InteractionState;
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use arcweft_runtime_driver::presentation_handles::PresentationHandleId;
use arcweft_runtime_driver::view_runtime::{
    BundleViewInstancePathSegment, BundleViewMountOutput, BundleViewStyleNode,
    BundleViewStyleNodeKind,
};
use arcweft_view::style::{
    ComputedViewStyle, ViewAxisProviderParticipation, ViewElementState, ViewElementStateSet,
    ViewInheritedBoxAxes, ViewInteractionSelector, ViewInteractionStateSet, ViewStyleApplication,
    ViewStyleApplicationTarget, ViewStyleEnvironmentUsage, ViewStyleNodeFacts, ViewStyleNodeKey,
    ViewStyleProgram, ViewStyleResolveContext, ViewStyleResolveError, ViewStyleResolveResult,
    ViewStyleResolver, ViewStyleRevisionSet, ViewStyleTraceMode,
};
use arcweft_view::{ViewId, ViewMountId};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axis_seed::{inherited_axes, validate_mount_seed_shape};
pub(super) use consumer::StyledViewResources;
use consumer::validate_supported_properties;

#[cfg(test)]
use consumer::{StyleConsumer, validate_consumer_properties};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum StyleTargetKind {
    Control,
    Text,
    Image,
    Part,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct StyleTargetKey {
    kind: StyleTargetKind,
    id: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CallerKey {
    handle: PresentationHandleId,
    path: Vec<BundleViewInstancePathSegment>,
    instruction: u32,
    child_view: ViewId,
}

#[derive(Clone, Debug)]
struct ResolvedNode {
    facts: ViewStyleNodeFacts,
    ancestors: Vec<ViewStyleNodeFacts>,
    computed: Arc<ComputedViewStyle>,
    environment_usage: ViewStyleEnvironmentUsage,
}

struct LiveStyleResolveContext<'a> {
    input: &'a InputController,
    presentation: &'a BundlePresentationSnapshot,
    program: &'a ViewStyleProgram,
    environment: &'a PresentationEnvironment,
    palettes: &'a SystemPaletteSet,
}

#[derive(Default)]
struct LiveStyleFrameState {
    output: ResolvedViewStyleFrame,
    resolved: BTreeMap<ViewStyleNodeKey, ResolvedNode>,
    callers: BTreeMap<CallerKey, ViewStyleNodeKey>,
}

struct PrimaryNodeStyle {
    parent_id: Option<ViewStyleNodeKey>,
    parent_computed: Option<Arc<ComputedViewStyle>>,
    ancestors: Vec<ViewStyleNodeFacts>,
    bindings: Vec<NodeBinding>,
    facts: ViewStyleNodeFacts,
    computed: Arc<ComputedViewStyle>,
    environment_usage: ViewStyleEnvironmentUsage,
    projected: ViewRuntimeNodeStyle,
    inherited_axes: ViewInheritedBoxAxes,
}

#[derive(Clone)]
struct NodeBinding {
    keys: Vec<StyleTargetKey>,
    target: Option<InteractionTarget>,
    enabled: bool,
    composing: bool,
    placeholder_shown: bool,
}

/// Current projected snapshot indexed by concrete mount-scoped render target.
#[derive(Clone, Debug, Default)]
pub(super) struct ResolvedViewStyleFrame {
    nodes: BTreeMap<ViewStyleNodeKey, ViewRuntimeNodeStyle>,
    targets: BTreeMap<StyleTargetKey, ViewRuntimeNodeStyle>,
}

/// Long-lived resolver and program identity retained by `PlayerFramePlannerState`.
#[derive(Clone, Debug, Default)]
pub(super) struct PlayerViewStyleState {
    resolver: ViewStyleResolver,
    program: Option<ViewStyleProgram>,
    program_revision: u64,
    live_mounts: BTreeSet<ViewMountId>,
    environment_usage: BTreeMap<ViewStyleNodeKey, ViewStyleEnvironmentUsage>,
}

impl ResolvedViewStyleFrame {
    pub(super) fn node(&self, node: &ViewStyleNodeKey) -> Option<&ViewRuntimeNodeStyle> {
        self.nodes.get(node)
    }

    pub(super) fn nodes(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ViewStyleNodeKey, &ViewRuntimeNodeStyle)> {
        self.nodes.iter()
    }

    pub(super) fn control(&self, id: &str) -> Option<&ViewRuntimeNodeStyle> {
        self.target(StyleTargetKind::Control, id)
    }

    pub(super) fn text(&self, id: &str) -> Option<&ViewRuntimeNodeStyle> {
        self.target(StyleTargetKind::Text, id)
    }

    pub(super) fn image(&self, id: &str) -> Option<&ViewRuntimeNodeStyle> {
        self.target(StyleTargetKind::Image, id)
    }

    pub(super) fn part(&self, id: &str) -> Option<&ViewRuntimeNodeStyle> {
        self.target(StyleTargetKind::Part, id)
    }

    fn target(&self, kind: StyleTargetKind, id: &str) -> Option<&ViewRuntimeNodeStyle> {
        self.targets.get(&StyleTargetKey {
            kind,
            id: id.to_owned(),
        })
    }

    fn insert(
        &mut self,
        key: StyleTargetKey,
        style: ViewRuntimeNodeStyle,
    ) -> Result<(), PlayerFrameError> {
        if self.targets.insert(key.clone(), style).is_some() {
            return Err(PlayerFrameError::DuplicateStyleTarget { target: key.id });
        }
        Ok(())
    }

    fn insert_node(
        &mut self,
        node: ViewStyleNodeKey,
        style: ViewRuntimeNodeStyle,
    ) -> Result<(), PlayerFrameError> {
        let mount = node.mount().get();
        let instruction = node.instruction();
        if self.nodes.insert(node, style).is_some() {
            return Err(PlayerFrameError::DuplicateStyleNode { mount, instruction });
        }
        Ok(())
    }
}

impl PlayerViewStyleState {
    pub(super) fn resolve(
        &mut self,
        input: &InputController,
        presentation: &BundlePresentationSnapshot,
        program: Option<&ViewStyleProgram>,
        environment: &PresentationEnvironment,
        palettes: &SystemPaletteSet,
    ) -> Result<ResolvedViewStyleFrame, PlayerFrameError> {
        self.synchronize_live_mounts(presentation);
        for mount in &presentation.view.mounts {
            validate_mount_seed_shape(mount)?;
        }
        let has_applications = presentation
            .view
            .mounts
            .iter()
            .flat_map(|mount| &mount.style_nodes)
            .any(|node| !node.applications.is_empty());
        if program.is_none() && has_applications {
            return Err(PlayerFrameError::MissingStyleProgram);
        }
        let default_program = ViewStyleProgram::default();
        let program = program.unwrap_or(&default_program);
        self.synchronize_program(program);
        let context = LiveStyleResolveContext {
            input,
            presentation,
            program,
            environment,
            palettes,
        };
        let mut frame = LiveStyleFrameState::default();
        for mount in &presentation.view.mounts {
            for node in &mount.style_nodes {
                self.resolve_runtime_node(&context, mount, node, &mut frame)?;
            }
        }
        self.environment_usage = frame
            .resolved
            .iter()
            .map(|(node, resolved)| (node.clone(), resolved.environment_usage))
            .collect();
        Ok(frame.output)
    }

    fn resolve_runtime_node(
        &mut self,
        context: &LiveStyleResolveContext<'_>,
        mount: &BundleViewMountOutput,
        node: &BundleViewStyleNode,
        frame: &mut LiveStyleFrameState,
    ) -> Result<(), PlayerFrameError> {
        let node_id = node.style_node_key(mount.mount);
        if frame.resolved.contains_key(&node_id) {
            return Err(PlayerFrameError::DuplicateStyleNode {
                mount: mount.mount.get(),
                instruction: node.instruction,
            });
        }
        let mut primary = self.resolve_primary_style(context, mount, node, &node_id, frame)?;
        primary.environment_usage =
            self.resolve_bound_styles(context, mount, node, &node_id, &primary, &mut frame.output)?;
        retain_resolved_node(frame, mount, node, node_id, primary)
    }

    fn resolve_primary_style(
        &mut self,
        context: &LiveStyleResolveContext<'_>,
        mount: &BundleViewMountOutput,
        node: &BundleViewStyleNode,
        node_id: &ViewStyleNodeKey,
        frame: &LiveStyleFrameState,
    ) -> Result<PrimaryNodeStyle, PlayerFrameError> {
        let parent_id = parent_node_id(mount, node, &frame.callers)?;
        let parent = parent_id
            .as_ref()
            .map(|parent| {
                frame
                    .resolved
                    .get(parent)
                    .ok_or(PlayerFrameError::MissingStyleParent {
                        mount: mount.mount.get(),
                        instruction: node.instruction,
                    })
            })
            .transpose()?;
        let ancestors = parent.map_or_else(Vec::new, |parent| {
            parent
                .ancestors
                .iter()
                .cloned()
                .chain(core::iter::once(parent.facts.clone()))
                .collect()
        });
        let parent_computed = parent.map(|parent| parent.computed.clone());
        let inherited_axes = inherited_axes(mount, node, parent)?;
        let bindings = node_bindings(context.presentation, context.input, mount, node)?;
        let primary_binding = bindings.first().cloned().unwrap_or(NodeBinding {
            keys: Vec::new(),
            target: None,
            enabled: true,
            composing: false,
            placeholder_shown: false,
        });
        let facts = node_facts(context.input, node, &primary_binding);
        let resolution = self.resolve_node(
            context.program,
            context.presentation,
            context.environment,
            node,
            node_id,
            &facts,
            &ancestors,
            parent_computed.as_deref(),
            parent_id.as_ref(),
            inherited_axes,
            ViewAxisProviderParticipation::RetainedPrimary,
        )?;
        let environment_usage = resolution.environment_usage();
        let computed = resolution.into_computed();
        validate_supported_properties(context.presentation, mount, node, &bindings, &computed)?;
        let projected = ViewRuntimeNodeStyle::try_from_computed(
            node_id.clone(),
            node.kind.runtime_geometry_owner(),
            &computed,
            context.environment,
            context.palettes,
        )?;
        Ok(PrimaryNodeStyle {
            parent_id,
            parent_computed,
            ancestors,
            bindings,
            facts,
            computed,
            environment_usage,
            projected,
            inherited_axes,
        })
    }

    fn resolve_bound_styles(
        &mut self,
        context: &LiveStyleResolveContext<'_>,
        mount: &BundleViewMountOutput,
        node: &BundleViewStyleNode,
        node_id: &ViewStyleNodeKey,
        primary: &PrimaryNodeStyle,
        output: &mut ResolvedViewStyleFrame,
    ) -> Result<ViewStyleEnvironmentUsage, PlayerFrameError> {
        let mut environment_usage = primary.environment_usage;
        for binding in &primary.bindings {
            let facts = node_facts(context.input, node, binding);
            let (computed, binding_usage) = if facts == primary.facts {
                (primary.computed.clone(), primary.environment_usage)
            } else {
                let resolution = self.resolve_node(
                    context.program,
                    context.presentation,
                    context.environment,
                    node,
                    node_id,
                    &facts,
                    &primary.ancestors,
                    primary.parent_computed.as_deref(),
                    primary.parent_id.as_ref(),
                    primary.inherited_axes,
                    ViewAxisProviderParticipation::ProjectionOnly,
                )?;
                let usage = resolution.environment_usage();
                (resolution.into_computed(), usage)
            };
            environment_usage = environment_usage.union(binding_usage);
            validate_supported_properties(
                context.presentation,
                mount,
                node,
                &primary.bindings,
                &computed,
            )?;
            let projected = if computed == primary.computed {
                primary.projected.clone()
            } else {
                ViewRuntimeNodeStyle::try_from_computed(
                    node_id.clone(),
                    node.kind.runtime_geometry_owner(),
                    &computed,
                    context.environment,
                    context.palettes,
                )?
            };
            for key in &binding.keys {
                output.insert(key.clone(), projected.clone())?;
            }
        }
        Ok(environment_usage)
    }

    fn synchronize_program(&mut self, program: &ViewStyleProgram) {
        if self.program.as_ref() != Some(program) {
            self.program = Some(program.clone());
            self.program_revision = self.program_revision.saturating_add(1);
            self.resolver.clear();
            self.environment_usage.clear();
        }
    }

    fn synchronize_live_mounts(&mut self, presentation: &BundlePresentationSnapshot) {
        let current = presentation
            .view
            .mounts
            .iter()
            .map(|mount| mount.mount)
            .collect::<BTreeSet<_>>();
        for removed in self.live_mounts.difference(&current) {
            self.resolver.invalidate_mount(*removed);
        }
        self.environment_usage
            .retain(|node, _| current.contains(&node.mount()));
        self.live_mounts = current;
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "live resolution keeps the canonical program, node identity, ancestry, and cache revisions explicit"
    )]
    fn resolve_node(
        &mut self,
        program: &ViewStyleProgram,
        presentation: &BundlePresentationSnapshot,
        environment: &PresentationEnvironment,
        node: &BundleViewStyleNode,
        node_id: &ViewStyleNodeKey,
        facts: &ViewStyleNodeFacts,
        ancestors: &[ViewStyleNodeFacts],
        parent: Option<&ComputedViewStyle>,
        parent_id: Option<&ViewStyleNodeKey>,
        inherited_axes: ViewInheritedBoxAxes,
        axis_provider_participation: ViewAxisProviderParticipation,
    ) -> Result<ViewStyleResolveResult, ViewStyleResolveError> {
        self.resolver.resolve(
            program,
            &ViewStyleResolveContext {
                node_key: node_id,
                node: facts,
                ancestors,
                applications: &node.applications,
                parent,
                parent_node_key: parent_id,
                inherited_axes,
                axis_provider_participation,
                environment,
                revisions: ViewStyleRevisionSet {
                    sheets: self.program_revision,
                    patches: self.program_revision,
                    tokens: self.program_revision,
                    applications: presentation.revision,
                    interactions: 0,
                    containers: 0,
                },
                trace: ViewStyleTraceMode::Off,
            },
        )
    }
}

fn retain_resolved_node(
    frame: &mut LiveStyleFrameState,
    mount: &BundleViewMountOutput,
    node: &BundleViewStyleNode,
    node_id: ViewStyleNodeKey,
    primary: PrimaryNodeStyle,
) -> Result<(), PlayerFrameError> {
    let PrimaryNodeStyle {
        parent_id: _,
        parent_computed: _,
        ancestors,
        bindings: _,
        facts,
        computed,
        environment_usage,
        projected,
        inherited_axes: _,
    } = primary;
    frame
        .output
        .insert_node(node_id.clone(), projected.clone())?;
    if let BundleViewStyleNodeKind::CallView { view } = &node.kind {
        let key = CallerKey {
            handle: mount.handle.clone(),
            path: node.path.segments().to_vec(),
            instruction: node.instruction,
            child_view: view.clone(),
        };
        if frame.callers.insert(key, node_id.clone()).is_some() {
            return Err(PlayerFrameError::AmbiguousStyleParent {
                mount: mount.mount.get(),
                instruction: node.instruction,
            });
        }
    }
    frame.resolved.insert(
        node_id,
        ResolvedNode {
            facts,
            ancestors,
            computed,
            environment_usage,
        },
    );
    Ok(())
}

fn parent_node_id(
    mount: &BundleViewMountOutput,
    node: &BundleViewStyleNode,
    callers: &BTreeMap<CallerKey, ViewStyleNodeKey>,
) -> Result<Option<ViewStyleNodeKey>, PlayerFrameError> {
    let parent_id = if let Some(parent) = &node.parent {
        Some(parent.style_node_key(mount.mount))
    } else if mount.path.segments().is_empty() {
        None
    } else {
        let (last, caller_path) = mount
            .path
            .segments()
            .split_last()
            .expect("non-empty mount path has a final segment");
        let BundleViewInstancePathSegment::Call { instruction, .. } = last else {
            return Err(PlayerFrameError::MissingStyleParent {
                mount: mount.mount.get(),
                instruction: node.instruction,
            });
        };
        let key = CallerKey {
            handle: mount.handle.clone(),
            path: caller_path.to_vec(),
            instruction: *instruction,
            child_view: mount.view.clone(),
        };
        Some(
            callers
                .get(&key)
                .cloned()
                .ok_or(PlayerFrameError::MissingStyleParent {
                    mount: mount.mount.get(),
                    instruction: node.instruction,
                })?,
        )
    };
    Ok(parent_id)
}

fn node_facts(
    input: &InputController,
    node: &BundleViewStyleNode,
    binding: &NodeBinding,
) -> ViewStyleNodeFacts {
    let element = match node.kind {
        BundleViewStyleNodeKind::Element { element, .. } => Some(element),
        BundleViewStyleNodeKind::Text { .. }
        | BundleViewStyleNodeKind::Image { .. }
        | BundleViewStyleNodeKind::Custom { .. }
        | BundleViewStyleNodeKind::CallView { .. } => None,
    };
    let implementation_part = node.part.clone();
    let exported_part = node.exported_part.clone();
    let interactions = interaction_states(
        input.interaction(),
        binding.target.as_ref(),
        binding.enabled,
    );
    let mut element_states = ViewElementStateSet::default();
    if binding
        .target
        .as_ref()
        .is_some_and(|target| input.focus_visible_for(target))
    {
        element_states = element_states.with(ViewElementState::FocusVisible);
    }
    if binding.composing {
        element_states = element_states.with(ViewElementState::Composing);
    }
    if binding.placeholder_shown {
        element_states = element_states.with(ViewElementState::PlaceholderShown);
    }
    let active_scopes = node
        .applications
        .iter()
        .filter(|application| {
            matches!(
                application.target(),
                ViewStyleApplicationTarget::Named { .. }
            )
        })
        .map(ViewStyleApplication::scope)
        .fold(Vec::new(), |mut scopes, scope| {
            if !scopes.contains(&scope) {
                scopes.push(scope);
            }
            scopes
        });
    ViewStyleNodeFacts::new(element)
        .with_parts(implementation_part, exported_part)
        .with_interactions(interactions)
        .with_element_states(element_states)
        .with_active_scopes(active_scopes)
}

fn interaction_states(
    interaction: &InteractionState,
    target: Option<&InteractionTarget>,
    enabled: bool,
) -> ViewInteractionStateSet {
    ViewInteractionSelector::cascade().into_iter().fold(
        ViewInteractionStateSet::default(),
        |states, selector| {
            if selector.matches(target, enabled, interaction) {
                states.with(selector)
            } else {
                states
            }
        },
    )
}

fn node_bindings(
    presentation: &BundlePresentationSnapshot,
    input: &InputController,
    mount: &BundleViewMountOutput,
    node: &BundleViewStyleNode,
) -> Result<Vec<NodeBinding>, PlayerFrameError> {
    let part_key = node.part.as_ref().map(|part| StyleTargetKey {
        kind: StyleTargetKind::Part,
        id: mount.scoped_id(part.as_public_id().as_str()),
    });
    let mut bindings = match &node.kind {
        BundleViewStyleNodeKind::Element { target, .. } => target
            .as_deref()
            .map(|target| {
                binding_for_target(
                    presentation,
                    input,
                    mount.scoped_id(target),
                    StyleTargetKind::Control,
                )
            })
            .transpose()?
            .into_iter()
            .collect(),
        BundleViewStyleNodeKind::Text { text_source } => mount
            .text
            .iter()
            .filter(|text| text.source_id == *text_source)
            .flat_map(|text| &text.targets)
            .map(|target| {
                binding_for_target(
                    presentation,
                    input,
                    mount.scoped_id(&target.public_id),
                    StyleTargetKind::Text,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        BundleViewStyleNodeKind::Image { image, target } => vec![binding_for_target(
            presentation,
            input,
            mount.scoped_id(target.as_deref().unwrap_or(image)),
            StyleTargetKind::Image,
        )?],
        BundleViewStyleNodeKind::Custom { .. } | BundleViewStyleNodeKind::CallView { .. } => {
            Vec::new()
        }
    };
    if let Some(part_key) = part_key {
        if let Some(binding) = bindings.first_mut() {
            binding.keys.push(part_key);
        } else {
            bindings.push(binding_for_target(
                presentation,
                input,
                part_key.id.clone(),
                StyleTargetKind::Part,
            )?);
        }
    }
    Ok(bindings)
}

fn binding_for_target(
    presentation: &BundlePresentationSnapshot,
    input: &InputController,
    id: String,
    kind: StyleTargetKind,
) -> Result<NodeBinding, PlayerFrameError> {
    let target = PublicId::try_new(id.clone())
        .map(InteractionTarget::new)
        .map_err(|_| PlayerFrameError::InvalidId { value: id.clone() })?;
    let enabled = presentation
        .action_buttons
        .iter()
        .find(|button| button.target == id)
        .is_none_or(|button| button.enabled);
    let placeholder_shown = presentation.text_inputs.iter().any(|control| {
        control.target == id
            && control.value.is_empty()
            && control
                .label
                .as_deref()
                .is_some_and(|label| !label.is_empty())
    });
    let composing = input.ime_composing()
        && input
            .focused_text_editor()
            .is_some_and(|editor| editor.target() == &target);
    Ok(NodeBinding {
        keys: vec![StyleTargetKey { kind, id }],
        target: Some(target),
        enabled,
        composing,
        placeholder_shown,
    })
}

#[cfg(test)]
mod tests;
