//! Display-list generation and canonical Style resolution for retained fragments.

use crate::{
    ComputedViewStyle, ContainerKind, CustomElementId, FragmentKind, ImageId, LayoutBox,
    LayoutResults, NodeId, RichTextSourceId, SemanticSpecId, TextSourceId,
    ViewAxisProviderParticipation, ViewBoxAxisHostSeed, ViewBoxAxisSeedGeneration, ViewElementKind,
    ViewError, ViewFragment, ViewInheritedBoxAxes, ViewInteractionSelector,
    ViewInteractionStateSet, ViewMountId, ViewSemanticFragment, ViewStyleApplication,
    ViewStyleApplicationTarget, ViewStyleBoundaryFacts, ViewStyleNodeFacts, ViewStyleNodeKey,
    ViewStyleProgram, ViewStyleResolveContext, ViewStyleResolver, ViewStyleRevisionSet,
    ViewStyleScopeId, ViewStyleTraceMode,
};
use arcweft_presentation::{
    appearance::PresentationEnvironment, interaction::InteractionState, semantic::SemanticRole,
};
use std::sync::Arc;

/// Frame-local display item identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DisplayItemId(pub u32);

/// Renderer-facing display payload produced from a retained View fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayItemKind {
    Text(TextSourceId),
    RichText(RichTextSourceId),
    Image(ImageId),
    Custom(CustomElementId),
}

/// One ordered display item with its source node and resolved layout box.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayItem {
    node: NodeId,
    kind: DisplayItemKind,
    layout: LayoutBox,
    semantics: Option<SemanticSpecId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DisplayStyleNode {
    key: ViewStyleNodeKey,
    parent: Option<NodeId>,
    ancestors: Vec<NodeId>,
    element: Option<ViewElementKind>,
    semantics: Option<SemanticSpecId>,
    active_scopes: Vec<ViewStyleScopeId>,
    applications: Vec<ViewStyleApplication>,
}

#[derive(Default)]
struct FragmentStyleAllocator {
    next_scope: u64,
    next_application_order: u32,
}

/// Ordered View display list with resolver-ready retained node ancestry.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DisplayList {
    items: Vec<DisplayItem>,
    style_nodes: Vec<DisplayStyleNode>,
    resolution_order: Vec<NodeId>,
}

/// One display item with canonical computed Style for the current frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedDisplayItem {
    item: DisplayItem,
    style: Arc<ComputedViewStyle>,
}

/// Ordered display list after canonical Style resolution.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedDisplayList {
    items: Vec<ResolvedDisplayItem>,
}

impl DisplayItem {
    pub const fn node(self) -> NodeId {
        self.node
    }

    pub const fn kind(self) -> DisplayItemKind {
        self.kind
    }

    pub const fn layout(self) -> LayoutBox {
        self.layout
    }

    pub const fn semantics(self) -> Option<SemanticSpecId> {
        self.semantics
    }
}

impl DisplayList {
    pub fn from_fragment(
        fragment: &ViewFragment,
        layouts: &LayoutResults,
    ) -> Result<Self, ViewError> {
        let parents = fragment_parents(fragment)?;
        let (style_nodes, resolution_order) = retain_style_nodes(fragment, &parents)?;
        let items = fragment
            .nodes()
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                let id = match u32::try_from(index) {
                    Ok(index) => NodeId(index),
                    Err(_) => return Some(Err(ViewError::CapacityExceeded)),
                };
                let kind = match node.kind() {
                    FragmentKind::Text(source) => DisplayItemKind::Text(source),
                    FragmentKind::RichText(source) => DisplayItemKind::RichText(source),
                    FragmentKind::Image(image) => DisplayItemKind::Image(image),
                    FragmentKind::Custom(custom) => DisplayItemKind::Custom(custom),
                    FragmentKind::Container(_) | FragmentKind::View(_) => return None,
                };
                Some(layouts.require(id).map(|layout| DisplayItem {
                    node: id,
                    kind,
                    layout,
                    semantics: node.semantics(),
                }))
            })
            .collect::<Result<Vec<_>, ViewError>>()?;
        Ok(Self {
            items,
            style_nodes,
            resolution_order,
        })
    }

    /// Resolves every retained node parent-first, then projects display-node results.
    pub fn resolve_styles(
        &self,
        semantics: &ViewSemanticFragment,
        program: &ViewStyleProgram,
        interaction: &InteractionState,
        environment: &PresentationEnvironment,
        revisions: ViewStyleRevisionSet,
        resolver: &mut ViewStyleResolver,
    ) -> Result<ResolvedDisplayList, ViewError> {
        let facts = self
            .style_nodes
            .iter()
            .map(|node| style_facts(node, semantics, interaction))
            .collect::<Result<Vec<_>, ViewError>>()?;
        let mut computed: Vec<Option<Arc<ComputedViewStyle>>> = vec![None; self.style_nodes.len()];
        // Detached fragment rendering is projection-only. Mounted player paths
        // supply their root's runtime-owned seed and participate in provider
        // invalidation; this path deliberately does neither.
        let detached_axes = ViewInheritedBoxAxes::for_host_seed(
            ViewMountId::from_raw(0),
            ViewBoxAxisSeedGeneration::INITIAL,
            ViewBoxAxisHostSeed::Default,
        );

        for node in &self.resolution_order {
            let index = node.0 as usize;
            let style_node = self
                .style_nodes
                .get(index)
                .ok_or(ViewError::InvalidFragmentNode(*node))?;
            let ancestors = style_node
                .ancestors
                .iter()
                .map(|ancestor| {
                    facts
                        .get(ancestor.0 as usize)
                        .cloned()
                        .ok_or(ViewError::InvalidFragmentNode(*ancestor))
                })
                .collect::<Result<Vec<_>, ViewError>>()?;
            let parent = style_node
                .parent
                .and_then(|parent| computed.get(parent.0 as usize))
                .and_then(Option::as_ref)
                .map(Arc::as_ref);
            let parent_node_key = style_node
                .parent
                .map(|parent| {
                    self.style_nodes
                        .get(parent.0 as usize)
                        .map(|node| &node.key)
                        .ok_or(ViewError::InvalidFragmentNode(parent))
                })
                .transpose()?;
            let inherited_axes =
                parent.map_or(detached_axes, |parent| parent.axes().inherited_snapshot());
            let resolution = resolver.resolve(
                program,
                &ViewStyleResolveContext {
                    node_key: &style_node.key,
                    node: &facts[index],
                    ancestors: &ancestors,
                    applications: &style_node.applications,
                    parent,
                    parent_node_key,
                    inherited_axes,
                    axis_provider_participation: ViewAxisProviderParticipation::ProjectionOnly,
                    environment,
                    revisions,
                    trace: ViewStyleTraceMode::Off,
                },
            )?;
            computed[index] = Some(resolution.into_computed());
        }

        let items = self
            .items
            .iter()
            .copied()
            .map(|item| {
                let style = computed
                    .get(item.node().0 as usize)
                    .and_then(Option::as_ref)
                    .cloned()
                    .ok_or(ViewError::InvalidFragmentNode(item.node()))?;
                Ok(ResolvedDisplayItem { item, style })
            })
            .collect::<Result<Vec<_>, ViewError>>()?;
        Ok(ResolvedDisplayList { items })
    }

    pub fn as_slice(&self) -> &[DisplayItem] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl ResolvedDisplayItem {
    pub const fn item(&self) -> DisplayItem {
        self.item
    }

    pub fn style(&self) -> &ComputedViewStyle {
        &self.style
    }
}

impl ResolvedDisplayList {
    pub fn as_slice(&self) -> &[ResolvedDisplayItem] {
        &self.items
    }

    pub fn into_vec(self) -> Vec<ResolvedDisplayItem> {
        self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

fn fragment_parents(fragment: &ViewFragment) -> Result<Vec<Option<NodeId>>, ViewError> {
    let mut parents = vec![None; fragment.nodes().len()];
    for (index, _) in fragment.nodes().iter().enumerate() {
        let parent = NodeId(u32::try_from(index).map_err(|_| ViewError::CapacityExceeded)?);
        for child in fragment
            .node_children(parent)
            .ok_or(ViewError::InvalidFragmentNode(parent))?
        {
            let slot = parents
                .get_mut(child.0 as usize)
                .ok_or(ViewError::InvalidFragmentNode(*child))?;
            if slot.replace(parent).is_some() {
                return Err(ViewError::MultipleFragmentParents(*child));
            }
        }
    }
    Ok(parents)
}

fn retain_style_nodes(
    fragment: &ViewFragment,
    parents: &[Option<NodeId>],
) -> Result<(Vec<DisplayStyleNode>, Vec<NodeId>), ViewError> {
    let mut style_nodes: Vec<Option<DisplayStyleNode>> = vec![None; fragment.nodes().len()];
    let mut resolution_order = Vec::with_capacity(fragment.nodes().len());
    let mut roots = parents
        .iter()
        .enumerate()
        .filter_map(|(index, parent)| parent.is_none().then_some(index))
        .map(|index| {
            u32::try_from(index)
                .map(NodeId)
                .map_err(|_| ViewError::CapacityExceeded)
        })
        .collect::<Result<Vec<_>, ViewError>>()?;
    roots.reverse();
    let mut stack = roots;
    let mut allocator = FragmentStyleAllocator::default();

    while let Some(node) = stack.pop() {
        let parent = parents[node.0 as usize];
        let parent_style = parent
            .and_then(|parent| style_nodes.get(parent.0 as usize))
            .and_then(Option::as_ref);
        let retained = retain_style_node(fragment, node, parent, parent_style, &mut allocator)?;
        style_nodes[node.0 as usize] = Some(retained);
        resolution_order.push(node);

        let children = fragment
            .node_children(node)
            .ok_or(ViewError::InvalidFragmentNode(node))?;
        stack.extend(children.iter().rev().copied());
    }

    let style_nodes = style_nodes
        .into_iter()
        .enumerate()
        .map(|(index, node)| {
            node.ok_or_else(|| {
                ViewError::InvalidFragmentNode(NodeId(u32::try_from(index).unwrap_or(u32::MAX)))
            })
        })
        .collect::<Result<Vec<_>, ViewError>>()?;
    Ok((style_nodes, resolution_order))
}

fn retain_style_node(
    fragment: &ViewFragment,
    node: NodeId,
    parent: Option<NodeId>,
    parent_style: Option<&DisplayStyleNode>,
    allocator: &mut FragmentStyleAllocator,
) -> Result<DisplayStyleNode, ViewError> {
    let source = fragment
        .nodes()
        .get(node.0 as usize)
        .ok_or(ViewError::InvalidFragmentNode(node))?;
    let local = fragment
        .node_style_applications(node)
        .ok_or(ViewError::InvalidFragmentNode(node))?;
    let applications = allocator.materialize(parent_style, local)?;
    let active_scopes = named_scope_inventory(&applications);
    let mut ancestors = parent_style.map_or_else(Vec::new, |parent| parent.ancestors.clone());
    if let Some(parent) = parent {
        ancestors.push(parent);
    }
    let mut path = parent_style.map_or_else(Vec::new, |parent| parent.key.path().to_vec());
    path.push(source.key().0);
    Ok(DisplayStyleNode {
        key: ViewStyleNodeKey::new(ViewMountId::from_raw(0), path, node.0),
        parent,
        ancestors,
        element: fragment_element(source.kind()),
        semantics: source.semantics(),
        active_scopes,
        applications,
    })
}

impl FragmentStyleAllocator {
    fn materialize(
        &mut self,
        parent: Option<&DisplayStyleNode>,
        local: &[ViewStyleApplicationTarget],
    ) -> Result<Vec<ViewStyleApplication>, ViewError> {
        let mut applications = parent.map_or_else(Vec::new, |parent| {
            parent
                .applications
                .iter()
                .filter(|application| is_named_application(application))
                .cloned()
                .collect()
        });
        if local.is_empty() {
            return Ok(applications);
        }
        let scope_depth = applications
            .iter()
            .map(ViewStyleApplication::scope_depth)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(ViewError::CapacityExceeded)?;
        let scope = ViewStyleScopeId::new(self.next_scope);
        self.next_scope = self
            .next_scope
            .checked_add(1)
            .ok_or(ViewError::CapacityExceeded)?;
        for target in local {
            applications.push(ViewStyleApplication::new(
                target.clone(),
                scope,
                scope_depth,
                self.next_application_order,
                ViewStyleBoundaryFacts::SAME_VIEW,
            ));
            self.next_application_order = self
                .next_application_order
                .checked_add(1)
                .ok_or(ViewError::CapacityExceeded)?;
        }
        Ok(applications)
    }
}

fn named_scope_inventory(applications: &[ViewStyleApplication]) -> Vec<ViewStyleScopeId> {
    applications
        .iter()
        .filter(|application| is_named_application(application))
        .map(ViewStyleApplication::scope)
        .fold(Vec::new(), |mut scopes, scope| {
            if scopes.last().copied() != Some(scope) {
                scopes.push(scope);
            }
            scopes
        })
}

fn is_named_application(application: &ViewStyleApplication) -> bool {
    matches!(
        application.target(),
        ViewStyleApplicationTarget::Named { .. }
    )
}

fn style_facts(
    node: &DisplayStyleNode,
    semantics: &ViewSemanticFragment,
    interaction: &InteractionState,
) -> Result<ViewStyleNodeFacts, ViewError> {
    let semantic = match node.semantics {
        Some(id) => Some(semantics.get(id).ok_or(ViewError::UnknownDisplaySemantic {
            node: NodeId(node.key.instruction()),
            semantic: id,
        })?),
        None => None,
    };
    let interactions = ViewInteractionSelector::ALL
        .iter()
        .copied()
        .filter(|selector| {
            selector.matches(
                semantic.map(crate::ViewSemanticNode::target),
                semantic.is_none_or(crate::ViewSemanticNode::enabled),
                interaction,
            )
        })
        .fold(ViewInteractionStateSet::default(), |states, state| {
            states.with(state)
        });
    let element = semantic
        .and_then(|semantic| semantic_element(semantic.role()))
        .or(node.element);
    Ok(ViewStyleNodeFacts::new(element)
        .with_interactions(interactions)
        .with_active_scopes(node.active_scopes.clone()))
}

const fn semantic_element(role: SemanticRole) -> Option<ViewElementKind> {
    match role {
        SemanticRole::Button => Some(ViewElementKind::Button),
        SemanticRole::TextField => Some(ViewElementKind::TextField),
        SemanticRole::TextArea => Some(ViewElementKind::TextArea),
        SemanticRole::SecureTextField => Some(ViewElementKind::SecureField),
        SemanticRole::Dialogue
        | SemanticRole::Activity
        | SemanticRole::Image
        | SemanticRole::Debug
        | SemanticRole::Custom => None,
    }
}

const fn fragment_element(kind: FragmentKind) -> Option<ViewElementKind> {
    match kind {
        FragmentKind::Container(ContainerKind::Block | ContainerKind::Inline) => {
            Some(ViewElementKind::Box)
        }
        FragmentKind::Container(ContainerKind::Stack) => Some(ViewElementKind::Stack),
        FragmentKind::Text(_)
        | FragmentKind::RichText(_)
        | FragmentKind::Image(_)
        | FragmentKind::View(_)
        | FragmentKind::Custom(_) => None,
    }
}
