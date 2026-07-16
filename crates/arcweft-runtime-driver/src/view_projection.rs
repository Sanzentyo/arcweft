//! Projects mount-scoped executable View output into existing scene resources.

use crate::view_runtime::{BundleViewFrame, BundleViewMountOutput};
use arcweft_bundle::BundleImageObject;
use arcweft_bundle::resource_codec::view::{
    ViewActionPayloadResource, ViewFocusInitialPolicy, ViewFocusTargetResolution,
    ViewRuntimeActionButtonAction,
};
use arcweft_bundle::resource_codec::{
    ViewRuntimeActionButton, ViewRuntimeFocusGroup, ViewRuntimeFocusNavigation,
    ViewRuntimeScrollRegion, ViewRuntimeSurface, ViewRuntimeTextControl,
};
use arcweft_view::ViewId;
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
        action_buttons: retain_non_executable(input.action_buttons, input.executable_definitions),
        scroll_regions: retain_non_executable(input.scroll_regions, input.executable_definitions),
        surfaces: retain_non_executable(input.surfaces, input.executable_definitions),
        focus_groups: retain_non_executable(input.focus_groups, input.executable_definitions),
        focus_navigation: retain_non_executable(
            input.focus_navigation,
            input.executable_definitions,
        ),
    };
    for mount in &frame.mounts {
        project_mount(mount, input, &mut projected);
    }
    projected
}

fn project_mount(
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
    project_action_buttons(mount, input, &active, projected);
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
                image.view = Some(scoped_id(mount, &mount.view));
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
    mount: &BundleViewMountOutput,
    input: &ViewProjectionInput<'_>,
    active: &BTreeSet<&str>,
    projected: &mut ProjectedViewResources,
) {
    projected.action_buttons.extend(
        input
            .action_buttons
            .iter()
            .filter(|button| {
                owned_and_active(button.view.as_deref(), &button.target, mount, active)
            })
            .cloned()
            .map(|mut button| {
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
                if let ViewRuntimeActionButtonAction::DialoguePrimaryAction { target, .. } =
                    &mut button.action
                {
                    *target = mount
                        .dialogue
                        .and_then(|dialogue| dialogue.primary_action.target);
                    button.enabled &= target.is_some();
                }
                button
            }),
    );
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
                region.view = Some(scoped_id(mount, &mount.view));
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
                group.view = Some(scoped_id(mount, &mount.view));
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
                navigation.view = Some(scoped_id(mount, &mount.view));
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
    *owner = Some(scoped_id(mount, &mount.view));
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
