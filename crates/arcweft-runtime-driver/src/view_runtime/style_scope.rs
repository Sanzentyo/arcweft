//! Ordered Style application scopes retained during View evaluation.

use super::BundleViewInstancePath;
use arcweft_bundle::resource_codec::view::{ViewElementKind, ViewRuntimeGeometryOwner};
use arcweft_view::{
    ViewId, ViewMountId, ViewPartLocalName, ViewPartName, ViewStyleApplication,
    ViewStyleApplicationTarget, ViewStyleBoundaryFacts, ViewStyleNodeKey, ViewStyleScopeId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::owner::ResolvedMountedViewOwner;
use super::part::ViewPartRuntimeCatalog;

/// One node producer whose effective ordered Style applications were retained.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleViewStyleNode {
    pub path: BundleViewInstancePath,
    pub instruction: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<BundleViewStyleNodeId>,
    pub kind: BundleViewStyleNodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<ViewPartLocalName>,
    /// Public part identity exposed to a Style application crossing the
    /// direct owning View boundary. This is distinct from the private
    /// implementation `part` identity above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exported_part: Option<ViewPartName>,
    pub applications: Vec<ViewStyleApplication>,
}

/// Complete identity of a retained parent node inside one mounted View.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleViewStyleNodeId {
    pub path: BundleViewInstancePath,
    pub instruction: u32,
}

/// Closed node-producer inventory used by the runtime Style substrate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BundleViewStyleNodeKind {
    Element {
        element: ViewElementKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },
    Text {
        text_source: String,
    },
    Image {
        image: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },
    Custom {
        element: String,
    },
    CallView {
        view: ViewId,
    },
}

impl BundleViewStyleNodeId {
    #[must_use]
    pub fn style_node_key(&self, mount: ViewMountId) -> ViewStyleNodeKey {
        ViewStyleNodeKey::new(mount, self.path.style_path_words(), self.instruction)
    }
}

impl BundleViewStyleNode {
    #[must_use]
    pub fn style_node_key(&self, mount: ViewMountId) -> ViewStyleNodeKey {
        ViewStyleNodeKey::new(mount, self.path.style_path_words(), self.instruction)
    }
}

impl BundleViewStyleNodeKind {
    #[must_use]
    pub const fn runtime_geometry_owner(&self) -> ViewRuntimeGeometryOwner {
        match self {
            Self::Element { element, .. } => ViewRuntimeGeometryOwner::Element(*element),
            Self::Text { .. } => ViewRuntimeGeometryOwner::Text,
            Self::Image { .. } => ViewRuntimeGeometryOwner::Image,
            Self::Custom { .. } => ViewRuntimeGeometryOwner::Custom,
            Self::CallView { .. } => ViewRuntimeGeometryOwner::CallView,
        }
    }
}

#[derive(Clone, Debug)]
struct ActiveNamedApplication {
    target: ViewStyleApplicationTarget,
    scope: ViewStyleScopeId,
    scope_depth: u16,
    application_order: u32,
    crossed_view_boundaries: u16,
}

#[derive(Clone, Debug, Default)]
struct NamedScopeFrame {
    applications: Vec<ActiveNamedApplication>,
}

/// Named applications active on the current retained-node ancestry.
#[derive(Clone, Debug, Default)]
pub(crate) struct ViewStyleScopeStack {
    frames: Vec<NamedScopeFrame>,
}

/// Deterministic per-frame allocator for Style scope and application order.
#[derive(Debug, Default)]
pub(crate) struct ViewStyleScopeAllocator {
    next_scope: u64,
    next_application_order: u32,
}

/// Style scope state and retained node inventory for one mounted View occurrence.
#[derive(Debug)]
pub(crate) struct ViewStyleScopeRuntime {
    stack: ViewStyleScopeStack,
    nodes: Vec<BundleViewStyleNode>,
    element_frames: Vec<bool>,
    node_ancestry: Vec<BundleViewStyleNodeId>,
}

/// One node-producer evaluation at the Style scope boundary.
pub(crate) struct ViewStyleNodeInput<'a> {
    pub(crate) parts: &'a ViewPartRuntimeCatalog,
    pub(crate) owner: &'a ResolvedMountedViewOwner,
    pub(crate) path: &'a BundleViewInstancePath,
    pub(crate) instruction: u32,
    pub(crate) kind: BundleViewStyleNodeKind,
    pub(crate) part: Option<&'a ViewPartLocalName>,
    pub(crate) local: &'a [ViewStyleApplicationTarget],
    pub(crate) root: bool,
}

#[derive(Debug)]
pub(crate) struct LocalStyleApplications {
    applications: Vec<ViewStyleApplication>,
    named_frame: Option<NamedScopeFrame>,
    node: Option<BundleViewStyleNodeId>,
}

/// Failure to represent the bounded Style scope stack in its public ID widths.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum ViewStyleScopeError {
    #[error("View Style scope depth exceeds u16")]
    ScopeDepth,
    #[error("View Style application order exceeds u32")]
    ApplicationOrder,
    #[error("View Style scope identity exceeds u64")]
    ScopeIdentity,
    #[error("View Style application crosses more than 65,535 nested View boundaries")]
    ViewBoundaryDepth,
    #[error("CloseElement has no matching Style scope frame")]
    UnmatchedElement,
    #[error("CloseElement has no active named Style scope")]
    MissingNamedScope,
    #[error("a View definition root may only establish named Style sheets")]
    InlineDefinitionStyle,
}

impl ViewStyleScopeRuntime {
    pub(crate) fn new(stack: ViewStyleScopeStack) -> Self {
        Self {
            stack,
            nodes: Vec::new(),
            element_frames: Vec::new(),
            node_ancestry: Vec::new(),
        }
    }

    pub(crate) fn retain_node(
        &mut self,
        input: ViewStyleNodeInput<'_>,
        allocator: &mut ViewStyleScopeAllocator,
    ) -> Result<LocalStyleApplications, ViewStyleScopeError> {
        let exported_part = input
            .part
            .and_then(|part| input.parts.public_name(input.owner, part).cloned());
        let mut local = self.stack.applications_for_node(
            input.local,
            input.root,
            exported_part.is_some(),
            allocator,
        )?;
        let node = BundleViewStyleNodeId {
            path: input.path.clone(),
            instruction: input.instruction,
        };
        local.node = Some(node.clone());
        self.nodes.push(BundleViewStyleNode {
            path: input.path.clone(),
            instruction: input.instruction,
            parent: self.node_ancestry.last().cloned(),
            kind: input.kind,
            part: input.part.cloned(),
            exported_part,
            applications: local.applications.clone(),
        });
        Ok(local)
    }

    pub(crate) fn enter_element(&mut self, local: &mut LocalStyleApplications) {
        self.element_frames.push(self.stack.push(local));
        self.node_ancestry.push(
            local
                .node
                .clone()
                .expect("retained element Style scope has a node identity"),
        );
    }

    pub(crate) fn leave_element(&mut self) -> Result<(), ViewStyleScopeError> {
        let pushed = self
            .element_frames
            .pop()
            .ok_or(ViewStyleScopeError::UnmatchedElement)?;
        let _ = self.node_ancestry.pop();
        if pushed && !self.stack.pop() {
            return Err(ViewStyleScopeError::MissingNamedScope);
        }
        Ok(())
    }

    pub(crate) fn for_nested_view(
        &self,
        local: &LocalStyleApplications,
    ) -> Result<ViewStyleScopeStack, ViewStyleScopeError> {
        self.stack.for_nested_view(local)
    }

    pub(crate) fn into_nodes(self) -> Vec<BundleViewStyleNode> {
        self.nodes
    }
}

impl ViewStyleScopeStack {
    pub(crate) fn enter_definition(
        &mut self,
        styles: &[ViewStyleApplicationTarget],
        allocator: &mut ViewStyleScopeAllocator,
    ) -> Result<(), ViewStyleScopeError> {
        if styles
            .iter()
            .any(|style| matches!(style, ViewStyleApplicationTarget::Inline { .. }))
        {
            return Err(ViewStyleScopeError::InlineDefinitionStyle);
        }
        let mut local = self.applications_for_node(styles, true, false, allocator)?;
        let _ = self.push(&mut local);
        Ok(())
    }

    pub(crate) fn applications_for_node(
        &self,
        local: &[ViewStyleApplicationTarget],
        root: bool,
        exported_part: bool,
        allocator: &mut ViewStyleScopeAllocator,
    ) -> Result<LocalStyleApplications, ViewStyleScopeError> {
        let mut applications = self
            .frames
            .iter()
            .flat_map(|frame| frame.applications.iter())
            .map(|application| {
                ViewStyleApplication::new(
                    application.target.clone(),
                    application.scope,
                    application.scope_depth,
                    application.application_order,
                    if application.crossed_view_boundaries != 0 {
                        ViewStyleBoundaryFacts::nested_view(
                            application.crossed_view_boundaries,
                            exported_part,
                            root,
                        )
                    } else {
                        ViewStyleBoundaryFacts::SAME_VIEW
                    },
                )
            })
            .collect::<Vec<_>>();

        if local.is_empty() {
            return Ok(LocalStyleApplications {
                applications,
                named_frame: None,
                node: None,
            });
        }

        let scope_depth = self
            .frames
            .len()
            .checked_add(1)
            .and_then(|depth| u16::try_from(depth).ok())
            .ok_or(ViewStyleScopeError::ScopeDepth)?;
        let count =
            u32::try_from(local.len()).map_err(|_| ViewStyleScopeError::ApplicationOrder)?;
        let next_application_order = allocator
            .next_application_order
            .checked_add(count)
            .ok_or(ViewStyleScopeError::ApplicationOrder)?;
        let next_scope = allocator
            .next_scope
            .checked_add(1)
            .ok_or(ViewStyleScopeError::ScopeIdentity)?;
        let scope = ViewStyleScopeId::new(allocator.next_scope);
        let first_application_order = allocator.next_application_order;
        allocator.next_scope = next_scope;
        allocator.next_application_order = next_application_order;

        let local_applications = local
            .iter()
            .enumerate()
            .map(|(index, target)| {
                let index =
                    u32::try_from(index).expect("local Style application count was checked above");
                ViewStyleApplication::new(
                    target.clone(),
                    scope,
                    scope_depth,
                    first_application_order + index,
                    ViewStyleBoundaryFacts::SAME_VIEW,
                )
            })
            .collect::<Vec<_>>();
        applications.extend(local_applications.iter().cloned());
        let named = local_applications
            .into_iter()
            .filter_map(|application| match application.target() {
                ViewStyleApplicationTarget::Named { .. } => Some(ActiveNamedApplication {
                    target: application.target().clone(),
                    scope: application.scope(),
                    scope_depth: application.scope_depth(),
                    application_order: application.application_order(),
                    crossed_view_boundaries: 0,
                }),
                ViewStyleApplicationTarget::Inline { .. } => None,
            })
            .collect::<Vec<_>>();

        Ok(LocalStyleApplications {
            applications,
            named_frame: (!named.is_empty()).then_some(NamedScopeFrame {
                applications: named,
            }),
            node: None,
        })
    }

    pub(crate) fn push(&mut self, local: &mut LocalStyleApplications) -> bool {
        if let Some(frame) = local.named_frame.take() {
            self.frames.push(frame);
            true
        } else {
            false
        }
    }

    pub(crate) fn pop(&mut self) -> bool {
        self.frames.pop().is_some()
    }

    pub(crate) fn for_nested_view(
        &self,
        local: &LocalStyleApplications,
    ) -> Result<Self, ViewStyleScopeError> {
        let mut nested = self.clone();
        if let Some(frame) = local.named_frame.clone() {
            nested.frames.push(frame);
        }
        for application in nested
            .frames
            .iter_mut()
            .flat_map(|frame| frame.applications.iter_mut())
        {
            application.crossed_view_boundaries = application
                .crossed_view_boundaries
                .checked_add(1)
                .ok_or(ViewStyleScopeError::ViewBoundaryDepth)?;
        }
        Ok(nested)
    }
}
