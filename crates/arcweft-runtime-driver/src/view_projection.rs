//! Projects mount-scoped executable View output into existing scene resources.

use crate::view_runtime::{BundleViewFrame, BundleViewMountOutput, BundleViewStyleNodeKind};
use arcweft_bundle::BundleImageObject;
use arcweft_bundle::resource_codec::view::{
    ViewActionPayloadResource, ViewFocusInitialPolicy, ViewFocusTargetResolution,
    ViewRuntimeActionButtonAction,
};
use arcweft_bundle::resource_codec::{
    ViewRuntimeActionButton, ViewRuntimeFocusGroup, ViewRuntimeFocusNavigation,
    ViewRuntimeScrollRegion, ViewRuntimeSurface, ViewRuntimeTextControl,
};
use arcweft_view::{EventKind, ViewId, ViewMountId};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProjectedViewResources {
    pub(crate) images: Vec<BundleImageObject>,
    pub(crate) text_inputs: Vec<ViewRuntimeTextControl>,
    pub(crate) action_buttons: Vec<ViewRuntimeActionButton>,
    pub(crate) scroll_regions: Vec<ViewRuntimeScrollRegion>,
    pub(crate) surfaces: Vec<ViewRuntimeSurface>,
    pub(crate) focus_groups: Vec<ViewRuntimeFocusGroup>,
    pub(crate) focus_navigation: Vec<ViewRuntimeFocusNavigation>,
}

pub(crate) struct ViewProjectionInput<'a> {
    pub(crate) executable_definitions: &'a BTreeSet<ViewId>,
    pub(crate) current_images: &'a [BundleImageObject],
    pub(crate) current_text_inputs: &'a [ViewRuntimeTextControl],
    pub(crate) images: &'a [BundleImageObject],
    pub(crate) text_inputs: &'a [ViewRuntimeTextControl],
    pub(crate) action_buttons: &'a [ViewRuntimeActionButton],
    pub(crate) scroll_regions: &'a [ViewRuntimeScrollRegion],
    pub(crate) surfaces: &'a [ViewRuntimeSurface],
    pub(crate) focus_groups: &'a [ViewRuntimeFocusGroup],
    pub(crate) focus_navigation: &'a [ViewRuntimeFocusNavigation],
}

pub(crate) fn project_view_resources(
    frame: &BundleViewFrame,
    input: &ViewProjectionInput<'_>,
) -> ProjectedViewResources {
    let mut projected = ProjectedViewResources {
        images: retain_non_executable(input.current_images, input.executable_definitions),
        text_inputs: retain_non_executable(input.text_inputs, input.executable_definitions),
        action_buttons: retain_non_executable(input.action_buttons, input.executable_definitions)
            .into_iter()
            .map(|mut button| {
                button.dialogue_mount = None;
                button
            })
            .collect(),
        scroll_regions: retain_non_executable(input.scroll_regions, input.executable_definitions),
        surfaces: retain_non_executable(input.surfaces, input.executable_definitions),
        focus_groups: retain_non_executable(input.focus_groups, input.executable_definitions),
        focus_navigation: retain_non_executable(
            input.focus_navigation,
            input.executable_definitions,
        ),
    };
    for mount in &frame.mounts {
        project_mount(frame, mount, input, &mut projected);
    }
    projected
}

fn project_mount(
    frame: &BundleViewFrame,
    mount: &BundleViewMountOutput,
    input: &ViewProjectionInput<'_>,
    projected: &mut ProjectedViewResources,
) {
    let active = mount
        .active_targets
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    project_images(mount, input, projected);
    project_text_inputs(mount, input, &active, projected);
    project_action_buttons(frame, mount, input, &active, projected);
    project_layout_resources(mount, input, &active, projected);
    project_focus(mount, input, &active, projected);
}

fn project_images(
    mount: &BundleViewMountOutput,
    input: &ViewProjectionInput<'_>,
    projected: &mut ProjectedViewResources,
) {
    projected.images.extend(
        input
            .images
            .iter()
            .filter(|image| {
                image.view.as_deref() == Some(mount.view.as_str())
                    && mount.active_images.contains(&image.id)
            })
            .cloned()
            .map(|mut image| {
                image.id = scoped_id(mount, &image.id);
                image.target = image.target.map(|target| scoped_id(mount, &target));
                image.view = Some(scoped_id(mount, mount.view.as_str()));
                image.containing_scroll_region = image
                    .containing_scroll_region
                    .map(|region| scoped_id(mount, &region));
                image
            }),
    );
}

fn project_text_inputs(
    mount: &BundleViewMountOutput,
    input: &ViewProjectionInput<'_>,
    active: &BTreeSet<&str>,
    projected: &mut ProjectedViewResources,
) {
    projected.text_inputs.extend(
        input
            .text_inputs
            .iter()
            .filter(|control| {
                owned_and_active(control.view.as_deref(), &control.target, mount, active)
            })
            .cloned()
            .map(|mut control| {
                control.public_id = scoped_id(mount, &control.public_id);
                control.target = scoped_id(mount, &control.target);
                scope_owner(
                    mount,
                    &mut control.view,
                    &mut control.containing_scroll_region,
                );
                if let Some(current) = input.current_text_inputs.iter().find(|current| {
                    current.public_id == control.public_id
                        && current.target == control.target
                        && current.session == control.session
                }) {
                    control.value.clone_from(&current.value);
                    control.selection = current.selection;
                }
                control
            }),
    );
}

fn project_action_buttons(
    frame: &BundleViewFrame,
    mount: &BundleViewMountOutput,
    input: &ViewProjectionInput<'_>,
    active: &BTreeSet<&str>,
    projected: &mut ProjectedViewResources,
) {
    let dialogue_mount = dialogue_root_mount(frame, mount);
    projected.action_buttons.extend(
        input
            .action_buttons
            .iter()
            .filter(|button| {
                owned_and_active(button.view.as_deref(), &button.target, mount, active)
            })
            .cloned()
            .map(|mut button| {
                button.dialogue_mount = matches!(
                    &button.action,
                    ViewRuntimeActionButtonAction::ActionInvoke { .. }
                )
                .then_some(dialogue_mount)
                .flatten();
                let authored_target = button.target.clone();
                let handler = mount
                    .style_nodes
                    .iter()
                    .find(|node| {
                        matches!(
                            &node.kind,
                            BundleViewStyleNodeKind::Element {
                                target: Some(target),
                                ..
                            } if target == &authored_target
                        )
                    })
                    .and_then(|node| {
                        mount.events.iter().find(|binding| {
                            binding.path() == &node.path
                                && binding.instruction() == node.instruction
                                && binding.event() == EventKind::Activate
                        })
                    });
                button.public_id = scoped_id(mount, &button.public_id);
                button.target = scoped_id(mount, &button.target);
                scope_owner(
                    mount,
                    &mut button.view,
                    &mut button.containing_scroll_region,
                );
                if let ViewRuntimeActionButtonAction::ActionInvoke {
                    payload: Some(ViewActionPayloadResource::TextControlProjection { input, .. }),
                    ..
                } = &mut button.action
                {
                    *input = scoped_id(mount, input);
                }
                if matches!(button.action, ViewRuntimeActionButtonAction::Noop)
                    && let Some(handler) = handler
                {
                    button.action = ViewRuntimeActionButtonAction::ViewHandler {
                        event: handler.event(),
                        route: handler.route(),
                    };
                }
                button
            }),
    );
}

fn dialogue_root_mount(
    frame: &BundleViewFrame,
    mount: &BundleViewMountOutput,
) -> Option<ViewMountId> {
    mount.dialogue?;
    let mut roots = frame.mounts.iter().filter(|candidate| {
        candidate.handle == mount.handle
            && candidate.path.segments().is_empty()
            && candidate.dialogue.is_some()
    });
    let root = roots.next()?;
    roots.next().is_none().then_some(root.mount)
}

fn project_layout_resources(
    mount: &BundleViewMountOutput,
    input: &ViewProjectionInput<'_>,
    active: &BTreeSet<&str>,
    projected: &mut ProjectedViewResources,
) {
    projected.scroll_regions.extend(
        input
            .scroll_regions
            .iter()
            .filter(|region| {
                owned_and_active(region.view.as_deref(), &region.target, mount, active)
            })
            .cloned()
            .map(|mut region| {
                region.public_id = scoped_id(mount, &region.public_id);
                region.target = scoped_id(mount, &region.target);
                region.view = Some(scoped_id(mount, mount.view.as_str()));
                region
            }),
    );
    projected.surfaces.extend(
        input
            .surfaces
            .iter()
            .filter(|surface| {
                owned_and_active(surface.view.as_deref(), &surface.target, mount, active)
            })
            .cloned()
            .map(|mut surface| {
                surface.public_id = scoped_id(mount, &surface.public_id);
                surface.target = scoped_id(mount, &surface.target);
                scope_owner(
                    mount,
                    &mut surface.view,
                    &mut surface.containing_scroll_region,
                );
                surface
            }),
    );
}

fn project_focus(
    mount: &BundleViewMountOutput,
    input: &ViewProjectionInput<'_>,
    active: &BTreeSet<&str>,
    projected: &mut ProjectedViewResources,
) {
    projected.focus_groups.extend(
        input
            .focus_groups
            .iter()
            .filter(|group| group.view.as_deref() == Some(mount.view.as_str()))
            .cloned()
            .map(|mut group| {
                group.public_id = scoped_id(mount, &group.public_id);
                group.view = Some(scoped_id(mount, mount.view.as_str()));
                group.parent = group.parent.map(|parent| scoped_id(mount, &parent));
                if let ViewFocusInitialPolicy::Explicit { target } = &mut group.initial {
                    *target = scoped_id(mount, target);
                }
                group
            }),
    );
    projected.focus_navigation.extend(
        input
            .focus_navigation
            .iter()
            .filter(|navigation| {
                owned_and_active(
                    navigation.view.as_deref(),
                    &navigation.public_id,
                    mount,
                    active,
                )
            })
            .cloned()
            .map(|mut navigation| {
                navigation.public_id = scoped_id(mount, &navigation.public_id);
                navigation.view = Some(scoped_id(mount, mount.view.as_str()));
                navigation.group = navigation.group.map(|group| scoped_id(mount, &group));
                for edge in &mut navigation.edges {
                    if let ViewFocusTargetResolution::Explicit { target } = &mut edge.target {
                        *target = scoped_id(mount, target);
                    }
                }
                navigation
            }),
    );
}

fn owned_and_active(
    owner: Option<&str>,
    target: &str,
    mount: &BundleViewMountOutput,
    active: &BTreeSet<&str>,
) -> bool {
    owner == Some(mount.view.as_str()) && active.contains(target)
}

fn scope_owner(
    mount: &BundleViewMountOutput,
    owner: &mut Option<String>,
    scroll_region: &mut Option<String>,
) {
    *owner = Some(scoped_id(mount, mount.view.as_str()));
    *scroll_region = scroll_region.take().map(|region| scoped_id(mount, &region));
}

fn scoped_id(mount: &BundleViewMountOutput, authored: &str) -> String {
    mount.scoped_id(authored)
}

trait ViewOwnedResource {
    fn view_owner(&self) -> Option<&str>;
}

fn retain_non_executable<T>(resources: &[T], executable_definitions: &BTreeSet<ViewId>) -> Vec<T>
where
    T: Clone + ViewOwnedResource,
{
    resources
        .iter()
        .filter(|resource| {
            resource.view_owner().is_none_or(|view| {
                !executable_definitions
                    .iter()
                    .any(|definition| definition.as_str() == view)
            })
        })
        .cloned()
        .collect()
}

macro_rules! impl_view_owned {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ViewOwnedResource for $ty {
                fn view_owner(&self) -> Option<&str> {
                    self.view.as_deref()
                }
            }
        )+
    };
}

impl_view_owned!(
    BundleImageObject,
    ViewRuntimeTextControl,
    ViewRuntimeActionButton,
    ViewRuntimeScrollRegion,
    ViewRuntimeSurface,
    ViewRuntimeFocusGroup,
    ViewRuntimeFocusNavigation,
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogue::{
        DialoguePageIndex, DialogueViewOccurrence, DialogueViewPrimaryAction, DialogueViewReveal,
        DialogueViewStage,
    };
    use crate::presentation_handles::PresentationHandleId;
    use crate::view_runtime::{BundleViewInstancePath, BundleViewMountOutput};
    use arcweft_bundle::resource_codec::{ViewRuntimeButtonBounds, ViewRuntimeControlVisualStyle};
    use arcweft_view::{
        DialogueEntryId, DialogueInstanceId, DialoguePresentationId, DialogueStageIndex, ViewId,
        ViewMountId,
    };

    #[test]
    fn dialogue_button_projection_uses_matching_root_mount_and_overwrites_input_provenance() {
        let handle = PresentationHandleId::try_new("handle.dialogue").unwrap();
        let other_handle = PresentationHandleId::try_new("handle.other").unwrap();
        let root_view = ViewId::try_new("view.dialogue.root").unwrap();
        let child_view = ViewId::try_new("view.dialogue.child").unwrap();
        let other_view = ViewId::try_new("view.other.root").unwrap();
        let root = mount(
            handle.clone(),
            ViewMountId::from_raw(17),
            root_view.clone(),
            BundleViewInstancePath::default(),
            Some(dialogue_state()),
            Vec::new(),
        );
        let other_root = mount(
            other_handle,
            ViewMountId::from_raw(27),
            other_view,
            BundleViewInstancePath::default(),
            Some(dialogue_state()),
            Vec::new(),
        );
        let child_path = serde_json::from_str::<BundleViewInstancePath>(
            r#"[{"kind":"call","instruction":3,"authored_key":null}]"#,
        )
        .unwrap();
        let child = mount(
            handle,
            ViewMountId::from_raw(18),
            child_view.clone(),
            child_path,
            Some(dialogue_state()),
            vec!["button.cancel".to_owned()],
        );
        let frame = BundleViewFrame {
            mounts: vec![other_root, root, child],
            diagnostics: Vec::new(),
        };
        let button = ViewRuntimeActionButton {
            public_id: "button.cancel".to_owned(),
            target: "button.cancel".to_owned(),
            dialogue_mount: Some(ViewMountId::from_raw(99)),
            view: Some(child_view.as_str().to_owned()),
            containing_scroll_region: None,
            label: "Cancel".to_owned(),
            enabled: true,
            bounds: ViewRuntimeButtonBounds::new(0, 0, 100_000, 40_000),
            action: ViewRuntimeActionButtonAction::ActionInvoke {
                action: "action.dialogue.cancel".to_owned(),
                payload: None,
            },
            style: ViewRuntimeControlVisualStyle::default(),
        };
        let executable_definitions = BTreeSet::from([root_view, child_view]);
        let projected = project_view_resources(
            &frame,
            &ViewProjectionInput {
                executable_definitions: &executable_definitions,
                current_images: &[],
                current_text_inputs: &[],
                images: &[],
                text_inputs: &[],
                action_buttons: &[button],
                scroll_regions: &[],
                surfaces: &[],
                focus_groups: &[],
                focus_navigation: &[],
            },
        );

        assert_eq!(projected.action_buttons.len(), 1);
        assert_eq!(
            projected.action_buttons[0].dialogue_mount,
            Some(ViewMountId::from_raw(17))
        );
    }

    #[test]
    fn dialogue_button_projection_fails_closed_without_a_root_mount() {
        let handle = PresentationHandleId::try_new("handle.dialogue").unwrap();
        let child_view = ViewId::try_new("view.dialogue.child").unwrap();
        let child_path = serde_json::from_str::<BundleViewInstancePath>(
            r#"[{"kind":"call","instruction":3,"authored_key":null}]"#,
        )
        .unwrap();
        let frame = BundleViewFrame {
            mounts: vec![mount(
                handle,
                ViewMountId::from_raw(18),
                child_view.clone(),
                child_path,
                Some(dialogue_state()),
                vec!["button.cancel".to_owned()],
            )],
            diagnostics: Vec::new(),
        };
        let button = ViewRuntimeActionButton {
            public_id: "button.cancel".to_owned(),
            target: "button.cancel".to_owned(),
            dialogue_mount: Some(ViewMountId::from_raw(99)),
            view: Some(child_view.as_str().to_owned()),
            containing_scroll_region: None,
            label: "Cancel".to_owned(),
            enabled: true,
            bounds: ViewRuntimeButtonBounds::new(0, 0, 100_000, 40_000),
            action: ViewRuntimeActionButtonAction::ActionInvoke {
                action: "action.dialogue.cancel".to_owned(),
                payload: None,
            },
            style: ViewRuntimeControlVisualStyle::default(),
        };
        let executable_definitions = BTreeSet::from([child_view]);
        let projected = project_view_resources(
            &frame,
            &ViewProjectionInput {
                executable_definitions: &executable_definitions,
                current_images: &[],
                current_text_inputs: &[],
                images: &[],
                text_inputs: &[],
                action_buttons: &[button],
                scroll_regions: &[],
                surfaces: &[],
                focus_groups: &[],
                focus_navigation: &[],
            },
        );

        assert_eq!(projected.action_buttons.len(), 1);
        assert_eq!(projected.action_buttons[0].dialogue_mount, None);
    }

    fn mount(
        handle: PresentationHandleId,
        mount: ViewMountId,
        view: ViewId,
        path: BundleViewInstancePath,
        dialogue: Option<crate::dialogue::DialogueViewState>,
        active_targets: Vec<String>,
    ) -> BundleViewMountOutput {
        BundleViewMountOutput {
            handle,
            mount,
            view,
            path,
            host_axis_seed: None,
            dialogue,
            active_targets,
            active_images: Vec::new(),
            paint: Vec::new(),
            text: Vec::new(),
            fx: Vec::new(),
            events: Vec::new(),
            style_nodes: Vec::new(),
        }
    }

    fn dialogue_state() -> crate::dialogue::DialogueViewState {
        crate::dialogue::DialogueViewState {
            occurrence: DialogueViewOccurrence {
                presentation: DialoguePresentationId::new(1),
                entry: DialogueEntryId::new(2),
                instance: DialogueInstanceId::new(3),
            },
            stage: DialogueViewStage {
                index: DialogueStageIndex::new(4),
                page: DialoguePageIndex::new(0),
                stage_count: 2,
                page_count: 1,
            },
            reveal: DialogueViewReveal::complete(),
            primary_action: DialogueViewPrimaryAction { target: None },
        }
    }
}
