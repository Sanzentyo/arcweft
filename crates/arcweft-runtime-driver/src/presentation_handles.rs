use arcweft_bundle::resource_codec::{
    ViewRuntimeActionButton, ViewRuntimeFocusGroup, ViewRuntimeFocusNavigation, ViewRuntimeSurface,
    ViewRuntimeTextControl,
};
use arcweft_id::presentation::PresentationHandleId;
use arcweft_interaction_model::presentation::PresentationResourceKind;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Lifecycle state carried by the portable presentation snapshot.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationResourceState {
    #[default]
    Mounted,
    Hidden,
    Unmounted,
    Released,
    Destroyed,
}

impl PresentationResourceState {
    #[must_use]
    pub const fn is_render_visible(self) -> bool {
        matches!(self, Self::Mounted)
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Released | Self::Destroyed)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mounted => "mounted",
            Self::Hidden => "hidden",
            Self::Unmounted => "unmounted",
            Self::Released => "released",
            Self::Destroyed => "destroyed",
        }
    }
}

/// Inert description of one presentation occurrence. Bundle sessions derive
/// these records from Core custody; decoding never creates a resource.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationHandleRecord {
    pub id: PresentationHandleId,
    pub kind: PresentationResourceKind,
    pub resource_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default)]
    pub state: PresentationResourceState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub depth_milli: i32,
}

impl PresentationHandleRecord {
    #[must_use]
    pub fn new(
        id: PresentationHandleId,
        kind: PresentationResourceKind,
        resource_id: String,
        owner: Option<String>,
        state: PresentationResourceState,
        layer: Option<String>,
        depth_milli: i32,
    ) -> Self {
        Self {
            id,
            kind,
            resource_id,
            owner,
            state,
            layer,
            depth_milli,
        }
    }

    #[must_use]
    pub fn is_render_visible(&self) -> bool {
        self.state.is_render_visible()
    }

    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }

    #[must_use]
    pub fn matches_resource(&self, kind: PresentationResourceKind, resource_id: &str) -> bool {
        self.kind == kind && self.resource_id == resource_id
    }
}

/// Stable diagnostics produced while filtering presentation projections.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationHandleDiagnosticCode {
    HiddenButFocusable,
}

impl PresentationHandleDiagnosticCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HiddenButFocusable => "PH006_HIDDEN_BUT_FOCUSABLE",
        }
    }
}

/// Human- and machine-readable lifecycle diagnostic.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PresentationHandleDiagnostic {
    pub code: PresentationHandleDiagnosticCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handle: Option<PresentationHandleId>,
    pub message: String,
}

impl PresentationHandleDiagnostic {
    #[must_use]
    pub fn new(
        code: PresentationHandleDiagnosticCode,
        handle: Option<PresentationHandleId>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            handle,
            message: message.into(),
        }
    }
}

impl fmt::Display for PresentationHandleDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.handle {
            Some(handle) => write!(
                formatter,
                "{} for `{handle}`: {}",
                self.code.as_str(),
                self.message
            ),
            None => write!(formatter, "{}: {}", self.code.as_str(), self.message),
        }
    }
}

pub(crate) fn filter_presentation_text_inputs(
    controls: Vec<ViewRuntimeTextControl>,
    handles: &[PresentationHandleRecord],
) -> Vec<ViewRuntimeTextControl> {
    controls
        .into_iter()
        .filter(|control| {
            let mut aliases = vec![control.public_id.as_str(), control.target.as_str()];
            let default_visible = control.view.is_none();
            if let Some(view) = control.view.as_deref() {
                aliases.push(view);
            }
            resource_is_render_visible(
                handles,
                &RUNTIME_CONTROL_FAMILIES,
                &aliases,
                default_visible,
            )
        })
        .collect()
}

pub(crate) fn filter_presentation_action_buttons(
    controls: Vec<ViewRuntimeActionButton>,
    handles: &[PresentationHandleRecord],
) -> Vec<ViewRuntimeActionButton> {
    controls
        .into_iter()
        .filter(|control| {
            let mut aliases = vec![control.public_id.as_str(), control.target.as_str()];
            let default_visible = control.view.is_none();
            if let Some(view) = control.view.as_deref() {
                aliases.push(view);
            }
            resource_is_render_visible(
                handles,
                &RUNTIME_CONTROL_FAMILIES,
                &aliases,
                default_visible,
            )
        })
        .collect()
}

pub(crate) fn filter_presentation_surfaces(
    surfaces: Vec<ViewRuntimeSurface>,
    handles: &[PresentationHandleRecord],
) -> Vec<ViewRuntimeSurface> {
    surfaces
        .into_iter()
        .filter(|surface| {
            let mut aliases = vec![surface.public_id.as_str(), surface.target.as_str()];
            let default_visible = surface.view.is_none();
            if let Some(view) = surface.view.as_deref() {
                aliases.push(view);
            }
            resource_is_render_visible(
                handles,
                &RUNTIME_CONTROL_FAMILIES,
                &aliases,
                default_visible,
            )
        })
        .collect()
}

pub(crate) fn filter_presentation_focus_groups(
    groups: Vec<ViewRuntimeFocusGroup>,
    handles: &[PresentationHandleRecord],
) -> Vec<ViewRuntimeFocusGroup> {
    groups
        .into_iter()
        .filter(|group| {
            let mut aliases = vec![group.public_id.as_str()];
            let default_visible = group.view.is_none();
            if let Some(view) = group.view.as_deref() {
                aliases.push(view);
            }
            resource_is_render_visible(
                handles,
                &RUNTIME_CONTROL_FAMILIES,
                &aliases,
                default_visible,
            )
        })
        .collect()
}

pub(crate) fn filter_presentation_focus_navigation(
    navigation: Vec<ViewRuntimeFocusNavigation>,
    handles: &[PresentationHandleRecord],
) -> Vec<ViewRuntimeFocusNavigation> {
    navigation
        .into_iter()
        .filter(|target| {
            let mut aliases = vec![target.public_id.as_str()];
            let default_visible = target.view.is_none();
            if let Some(view) = target.view.as_deref() {
                aliases.push(view);
            }
            resource_is_render_visible(
                handles,
                &RUNTIME_CONTROL_FAMILIES,
                &aliases,
                default_visible,
            ) && target.group.as_ref().is_none_or(|group| {
                resource_is_render_visible(
                    handles,
                    &RUNTIME_CONTROL_FAMILIES,
                    &[group.as_str()],
                    true,
                )
            })
        })
        .collect()
}

pub(crate) fn hidden_focus_diagnostics(
    handles: &[PresentationHandleRecord],
    navigation: &[ViewRuntimeFocusNavigation],
) -> Vec<PresentationHandleDiagnostic> {
    navigation
        .iter()
        .filter_map(|target| {
            let mut aliases = vec![target.public_id.as_str()];
            if let Some(view) = target.view.as_deref() {
                aliases.push(view);
            }
            hidden_matching_handle(handles, &RUNTIME_CONTROL_FAMILIES, &aliases).map(
                |handle| {
                    PresentationHandleDiagnostic::new(
                        PresentationHandleDiagnosticCode::HiddenButFocusable,
                        Some(handle.id.clone()),
                        format!(
                            "hidden presentation resource `{}` still appeared in focus navigation before filtering",
                            target.public_id
                        ),
                    )
                },
            )
        })
        .collect()
}

const RUNTIME_CONTROL_FAMILIES: [PresentationResourceKind; 4] = [
    PresentationResourceKind::View,
    PresentationResourceKind::Menu,
    PresentationResourceKind::Overlay,
    PresentationResourceKind::RuntimeControl,
];

fn resource_is_render_visible(
    handles: &[PresentationHandleRecord],
    kinds: &[PresentationResourceKind],
    aliases: &[&str],
    default_visible: bool,
) -> bool {
    let mut matched = false;
    for handle in handles.iter().filter(|handle| {
        kinds.contains(&handle.kind) && aliases.iter().any(|alias| *alias == handle.resource_id)
    }) {
        matched = true;
        if handle.is_render_visible() {
            return true;
        }
    }
    !matched && default_visible
}

fn hidden_matching_handle<'a>(
    handles: &'a [PresentationHandleRecord],
    kinds: &[PresentationResourceKind],
    aliases: &[&str],
) -> Option<&'a PresentationHandleRecord> {
    let mut hidden = None;
    for handle in handles.iter().filter(|handle| {
        kinds.contains(&handle.kind) && aliases.iter().any(|alias| *alias == handle.resource_id)
    }) {
        if handle.is_render_visible() {
            return None;
        }
        hidden.get_or_insert(handle);
    }
    hidden
}

fn is_default<T>(value: &T) -> bool
where
    T: Default + PartialEq,
{
    value == &T::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::resource_codec::view::{
        CompositionOnBlurPolicy, TextAssistPolicy, TextCapitalization, ViewInputKind,
        ViewSecureInputPolicy, ViewTextSelectionPolicy, ViewTextShortcutPolicy, ViewTextTabPolicy,
        ViewTextVerticalNavigationPolicy,
    };
    use arcweft_bundle::resource_codec::{
        ViewRuntimeControlVisualStyle, ViewRuntimeTextControlBounds,
        ViewRuntimeTextControlHandlers, ViewRuntimeTextControlOptions, ViewRuntimeTextSelection,
    };
    use arcweft_presentation::text_input::{EnterKeyHint, TextInputPurpose};

    fn handle(value: &str) -> PresentationHandleId {
        PresentationHandleId::try_new(value).expect("valid handle id")
    }

    #[test]
    fn invalid_handle_identity_cannot_enter_a_presentation_snapshot() {
        for value in ["", "bad handle", "@handle.bad", "handle.\n", "handle.雪"] {
            let original = view_handle(
                "handle.valid",
                "view.Main",
                PresentationResourceState::Mounted,
            );
            let mut wire = serde_json::to_value(original).expect("presentation record wire");
            wire["id"] = serde_json::Value::String(value.to_owned());
            assert!(
                serde_json::from_value::<PresentationHandleRecord>(wire).is_err(),
                "invalid identity {value:?} must not bypass construction through serde"
            );
        }
        for field in ["created_epoch", "updated_epoch"] {
            let original = view_handle(
                "handle.valid",
                "view.Main",
                PresentationResourceState::Mounted,
            );
            let mut wire = serde_json::to_value(original).unwrap();
            wire[field] = serde_json::json!(1);
            assert!(
                serde_json::from_value::<PresentationHandleRecord>(wire).is_err(),
                "the removed lifecycle reader must not accept {field}"
            );
        }
    }

    fn view_handle(
        id: &str,
        resource_id: &str,
        state: PresentationResourceState,
    ) -> PresentationHandleRecord {
        PresentationHandleRecord::new(
            handle(id),
            PresentationResourceKind::View,
            resource_id.to_owned(),
            Some("flow.test".to_owned()),
            state,
            None,
            0,
        )
    }

    fn text_control(public_id: &str, view: Option<&str>) -> ViewRuntimeTextControl {
        ViewRuntimeTextControl {
            public_id: public_id.to_owned(),
            target: public_id.to_owned(),
            view: view.map(str::to_owned),
            containing_scroll_region: None,
            session: 1,
            value: String::new(),
            selection: ViewRuntimeTextSelection::new(0, 0),
            options: ViewRuntimeTextControlOptions {
                purpose: TextInputPurpose::Text,
                autocorrect: TextAssistPolicy::PlatformDefault,
                spellcheck: TextAssistPolicy::PlatformDefault,
                capitalization: TextCapitalization::None,
                enter_key: EnterKeyHint::Send,
                multiline: false,
                selection_policy: ViewTextSelectionPolicy::Enabled,
                shortcut_policy: ViewTextShortcutPolicy::Enabled,
                tab_policy: ViewTextTabPolicy::FocusNavigation,
                vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
                secure_policy: ViewSecureInputPolicy::Plain,
                composition_on_blur: CompositionOnBlurPolicy::Commit,
            },
            kind: ViewInputKind::TextField,
            bounds: ViewRuntimeTextControlBounds::from_px(0, 0, 100, 24),
            label: None,
            placeholder: None,
            handlers: ViewRuntimeTextControlHandlers::default(),
            style: ViewRuntimeControlVisualStyle::default(),
        }
    }

    #[test]
    fn view_owned_runtime_controls_require_visible_view_handle() {
        let controls = vec![
            text_control("input.global", None),
            text_control("input.name", Some("view.NamePanel")),
            text_control("input.brief", Some("view.BriefPanel")),
        ];

        let without_handles = filter_presentation_text_inputs(controls.clone(), &[]);
        assert_eq!(
            without_handles
                .iter()
                .map(|control| control.public_id.as_str())
                .collect::<Vec<_>>(),
            ["input.global"]
        );

        let handles = [view_handle(
            "handle.flow.test.name",
            "view.NamePanel",
            PresentationResourceState::Mounted,
        )];
        let mounted_name = filter_presentation_text_inputs(controls.clone(), &handles);
        assert_eq!(
            mounted_name
                .iter()
                .map(|control| control.public_id.as_str())
                .collect::<Vec<_>>(),
            ["input.global", "input.name"]
        );

        let handles = [view_handle(
            "handle.flow.test.name",
            "view.NamePanel",
            PresentationResourceState::Released,
        )];
        let released_name = filter_presentation_text_inputs(controls, &handles);
        assert_eq!(
            released_name
                .iter()
                .map(|control| control.public_id.as_str())
                .collect::<Vec<_>>(),
            ["input.global"]
        );
    }

    #[test]
    fn mounted_handle_keeps_resource_visible_after_retained_tombstone() {
        let controls = vec![text_control("input.name", Some("view.NamePanel"))];
        let handles = [
            view_handle(
                "handle.flow.old.name",
                "view.NamePanel",
                PresentationResourceState::Released,
            ),
            view_handle(
                "handle.flow.new.name",
                "view.NamePanel",
                PresentationResourceState::Mounted,
            ),
        ];

        let visible = filter_presentation_text_inputs(controls, &handles);

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].public_id, "input.name");
    }
}
