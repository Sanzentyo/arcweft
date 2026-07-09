use arcweft_bundle::BundleImageObject;
use arcweft_bundle::resource_codec::{
    ViewRuntimeActionButton, ViewRuntimeFocusGroup, ViewRuntimeFocusNavigation,
    ViewRuntimeScrollRegion, ViewRuntimeSurface, ViewRuntimeTextBlock, ViewRuntimeTextControl,
};
use arcweft_core::effect::{LineEffectRequest, RuntimeCall};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable, serializable identity for a scoped presentation handle.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct PresentationHandleId(String);

impl PresentationHandleId {
    /// Creates a handle id from a runtime/lowering supplied public-id-like value.
    pub fn try_new(value: impl Into<String>) -> Result<Self, PresentationHandleDiagnostic> {
        let value = value.into();
        if is_valid_handle_token(&value) {
            Ok(Self(value))
        } else {
            Err(PresentationHandleDiagnostic::new(
                PresentationHandleDiagnosticCode::InvalidCall,
                None,
                format!("invalid presentation handle id `{value}`"),
            ))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PresentationHandleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<PresentationHandleId> for String {
    fn from(value: PresentationHandleId) -> Self {
        value.0
    }
}

/// Typed runtime family for scoped presentation handles.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationHandleKind {
    Image,
    View,
    Menu,
    Overlay,
    TextBox,
    RuntimeControl,
}

impl PresentationHandleKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::View => "view",
            Self::Menu => "menu",
            Self::Overlay => "overlay",
            Self::TextBox => "text_box",
            Self::RuntimeControl => "runtime_control",
        }
    }

    fn from_arg(value: &str) -> Option<Self> {
        match clean_runtime_arg(value) {
            "image" => Some(Self::Image),
            "view" => Some(Self::View),
            "menu" => Some(Self::Menu),
            "overlay" => Some(Self::Overlay),
            "text_box" => Some(Self::TextBox),
            "runtime_control" => Some(Self::RuntimeControl),
            _ => None,
        }
    }
}

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

/// Serializable runtime record for one scoped presentation resource.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PresentationHandleRecord {
    pub id: PresentationHandleId,
    pub kind: PresentationHandleKind,
    pub resource_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default)]
    pub state: PresentationResourceState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub depth_milli: i32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub created_epoch: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub updated_epoch: u64,
}

impl PresentationHandleRecord {
    #[must_use]
    pub fn new(
        id: PresentationHandleId,
        kind: PresentationHandleKind,
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
            created_epoch: 0,
            updated_epoch: 0,
        }
    }

    #[must_use]
    pub fn with_epochs(mut self, created_epoch: u64, updated_epoch: u64) -> Self {
        self.created_epoch = created_epoch;
        self.updated_epoch = updated_epoch;
        self
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
    pub fn matches_resource(&self, kind: PresentationHandleKind, resource_id: &str) -> bool {
        self.kind == kind && self.resource_id == resource_id
    }
}

/// Deterministic lifecycle operation emitted by lowering/runtime effects.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PresentationHandleOperation {
    Create {
        id: PresentationHandleId,
        kind: PresentationHandleKind,
        resource_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        owner: Option<String>,
        #[serde(default = "default_true")]
        visible: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        layer: Option<String>,
        #[serde(default, skip_serializing_if = "is_default")]
        depth_milli: i32,
    },
    Show {
        id: PresentationHandleId,
    },
    Hide {
        id: PresentationHandleId,
    },
    Unmount {
        id: PresentationHandleId,
    },
    Release {
        id: PresentationHandleId,
    },
    Destroy {
        id: PresentationHandleId,
    },
    Dispose {
        id: PresentationHandleId,
    },
}

impl PresentationHandleOperation {
    /// Parses the canonical runtime call surface used by lowering.
    pub fn from_call(call: &RuntimeCall) -> Option<Result<Self, PresentationHandleDiagnostic>> {
        match call.callee.as_str() {
            "presentation.handle.create" => Some(Self::create_from_call(call)),
            "presentation.handle.show" => Some(Self::unary_from_call(call, |id| Self::Show { id })),
            "presentation.handle.hide" => Some(Self::unary_from_call(call, |id| Self::Hide { id })),
            "presentation.handle.unmount" => {
                Some(Self::unary_from_call(call, |id| Self::Unmount { id }))
            }
            "presentation.handle.release" => {
                Some(Self::unary_from_call(call, |id| Self::Release { id }))
            }
            "presentation.handle.destroy" => {
                Some(Self::unary_from_call(call, |id| Self::Destroy { id }))
            }
            "presentation.handle.dispose" => {
                Some(Self::unary_from_call(call, |id| Self::Dispose { id }))
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn id(&self) -> &PresentationHandleId {
        match self {
            Self::Create { id, .. }
            | Self::Show { id }
            | Self::Hide { id }
            | Self::Unmount { id }
            | Self::Release { id }
            | Self::Destroy { id }
            | Self::Dispose { id } => id,
        }
    }

    fn create_from_call(call: &RuntimeCall) -> Result<Self, PresentationHandleDiagnostic> {
        let id = handle_id_arg(call)?;
        let Some(kind) =
            named_runtime_arg(&call.args, "kind").and_then(PresentationHandleKind::from_arg)
        else {
            return Err(PresentationHandleDiagnostic::new(
                PresentationHandleDiagnosticCode::InvalidCall,
                Some(id),
                "presentation.handle.create requires kind = image|view|menu|overlay|text_box|runtime_control",
            ));
        };
        let resource_id = named_runtime_arg(&call.args, "resource")
            .and_then(public_id_arg)
            .ok_or_else(|| {
                PresentationHandleDiagnostic::new(
                    PresentationHandleDiagnosticCode::InvalidCall,
                    Some(id.clone()),
                    "presentation.handle.create requires a resource argument",
                )
            })?;
        let owner = named_runtime_arg(&call.args, "owner").and_then(public_id_arg);
        let visible = named_runtime_arg(&call.args, "visible")
            .and_then(parse_bool_arg)
            .unwrap_or(true);
        let layer = named_runtime_arg(&call.args, "layer").and_then(public_id_arg);
        let depth_milli = named_runtime_arg(&call.args, "depth")
            .and_then(parse_i32_arg)
            .unwrap_or_default();
        Ok(Self::Create {
            id,
            kind,
            resource_id,
            owner,
            visible,
            layer,
            depth_milli,
        })
    }

    fn unary_from_call(
        call: &RuntimeCall,
        build: impl FnOnce(PresentationHandleId) -> Self,
    ) -> Result<Self, PresentationHandleDiagnostic> {
        handle_id_arg(call).map(build)
    }
}

/// Stable diagnostics produced while applying presentation handle operations.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationHandleDiagnosticCode {
    InvalidCall,
    DuplicateHandle,
    ResourceAlreadyOwned,
    UnknownHandle,
    DoubleDispose,
    TerminalHandle,
    HiddenButFocusable,
}

impl PresentationHandleDiagnosticCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidCall => "PH001_INVALID_CALL",
            Self::DuplicateHandle => "PH002_DUPLICATE_HANDLE",
            Self::ResourceAlreadyOwned => "PH003_RESOURCE_ALREADY_OWNED",
            Self::UnknownHandle => "PH004_UNKNOWN_HANDLE",
            Self::DoubleDispose => "PH005_DOUBLE_DISPOSE",
            Self::TerminalHandle => "PH006_TERMINAL_HANDLE",
            Self::HiddenButFocusable => "PH007_HIDDEN_BUT_FOCUSABLE",
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

pub(crate) fn presentation_handle_operations_from_effects(
    effects: &[LineEffectRequest],
) -> (
    Vec<PresentationHandleOperation>,
    Vec<PresentationHandleDiagnostic>,
) {
    let mut operations = Vec::new();
    let mut diagnostics = Vec::new();
    for effect in effects {
        let LineEffectRequest::Call(call) = effect else {
            continue;
        };
        match PresentationHandleOperation::from_call(call) {
            Some(Ok(operation)) => operations.push(operation),
            Some(Err(diagnostic)) => diagnostics.push(diagnostic),
            None => {}
        }
    }
    (operations, diagnostics)
}

pub(crate) fn apply_presentation_handle_operations(
    handles: &mut Vec<PresentationHandleRecord>,
    operation_epoch: &mut u64,
    operations: &[PresentationHandleOperation],
) -> Vec<PresentationHandleDiagnostic> {
    operations
        .iter()
        .filter_map(|operation| {
            *operation_epoch = operation_epoch.saturating_add(1);
            apply_presentation_handle_operation(handles, *operation_epoch, operation)
        })
        .collect()
}

fn apply_presentation_handle_operation(
    handles: &mut Vec<PresentationHandleRecord>,
    operation_epoch: u64,
    operation: &PresentationHandleOperation,
) -> Option<PresentationHandleDiagnostic> {
    match operation {
        PresentationHandleOperation::Create {
            id,
            kind,
            resource_id,
            owner,
            visible,
            layer,
            depth_milli,
        } => {
            let state = if *visible {
                PresentationResourceState::Mounted
            } else {
                PresentationResourceState::Hidden
            };
            if let Some(existing) = handles.iter_mut().find(|handle| handle.id == *id) {
                if existing.kind == *kind
                    && existing.resource_id == *resource_id
                    && !existing.is_terminal()
                {
                    existing.owner.clone_from(owner);
                    existing.state = state;
                    existing.layer.clone_from(layer);
                    existing.depth_milli = *depth_milli;
                    existing.updated_epoch = operation_epoch;
                    return None;
                }
                return Some(PresentationHandleDiagnostic::new(
                    PresentationHandleDiagnosticCode::DuplicateHandle,
                    Some(id.clone()),
                    "handle ids are stable within one save lineage and cannot be reused",
                ));
            }
            if let Some(existing) = handles.iter().find(|handle| {
                handle.kind == *kind && handle.resource_id == *resource_id && !handle.is_terminal()
            }) {
                return Some(PresentationHandleDiagnostic::new(
                    PresentationHandleDiagnosticCode::ResourceAlreadyOwned,
                    Some(id.clone()),
                    format!(
                        "resource `{}` is already owned by live handle `{}`",
                        existing.resource_id, existing.id
                    ),
                ));
            }
            handles.push(
                PresentationHandleRecord::new(
                    id.clone(),
                    *kind,
                    resource_id.clone(),
                    owner.clone(),
                    state,
                    layer.clone(),
                    *depth_milli,
                )
                .with_epochs(operation_epoch, operation_epoch),
            );
            None
        }
        PresentationHandleOperation::Show { id } => set_live_state(
            handles,
            id,
            operation_epoch,
            PresentationResourceState::Mounted,
            "show terminal presentation handle",
        ),
        PresentationHandleOperation::Hide { id } => set_live_state(
            handles,
            id,
            operation_epoch,
            PresentationResourceState::Hidden,
            "hide terminal presentation handle",
        ),
        PresentationHandleOperation::Unmount { id } => set_live_state(
            handles,
            id,
            operation_epoch,
            PresentationResourceState::Unmounted,
            "unmount terminal presentation handle",
        ),
        PresentationHandleOperation::Release { id }
        | PresentationHandleOperation::Dispose { id } => terminate_handle(
            handles,
            id,
            operation_epoch,
            PresentationResourceState::Released,
        ),
        PresentationHandleOperation::Destroy { id } => terminate_handle(
            handles,
            id,
            operation_epoch,
            PresentationResourceState::Destroyed,
        ),
    }
}

fn set_live_state(
    handles: &mut [PresentationHandleRecord],
    id: &PresentationHandleId,
    operation_epoch: u64,
    next_state: PresentationResourceState,
    terminal_message: &'static str,
) -> Option<PresentationHandleDiagnostic> {
    let Some(handle) = handles.iter_mut().find(|handle| handle.id == *id) else {
        return Some(unknown_handle(id));
    };
    if handle.is_terminal() {
        return Some(PresentationHandleDiagnostic::new(
            PresentationHandleDiagnosticCode::TerminalHandle,
            Some(id.clone()),
            terminal_message,
        ));
    }
    handle.state = next_state;
    handle.updated_epoch = operation_epoch;
    None
}

fn terminate_handle(
    handles: &mut [PresentationHandleRecord],
    id: &PresentationHandleId,
    operation_epoch: u64,
    next_state: PresentationResourceState,
) -> Option<PresentationHandleDiagnostic> {
    let Some(handle) = handles.iter_mut().find(|handle| handle.id == *id) else {
        return Some(unknown_handle(id));
    };
    if handle.is_terminal() {
        return Some(PresentationHandleDiagnostic::new(
            PresentationHandleDiagnosticCode::DoubleDispose,
            Some(id.clone()),
            format!("handle is already {}", handle.state.as_str()),
        ));
    }
    handle.state = next_state;
    handle.updated_epoch = operation_epoch;
    None
}

fn unknown_handle(id: &PresentationHandleId) -> PresentationHandleDiagnostic {
    PresentationHandleDiagnostic::new(
        PresentationHandleDiagnosticCode::UnknownHandle,
        Some(id.clone()),
        "presentation handle operation targets an unknown handle",
    )
}

pub(crate) fn apply_presentation_image_handles(
    active: &mut Vec<BundleImageObject>,
    handles: &[PresentationHandleRecord],
    image_objects: &[BundleImageObject],
) {
    for handle in handles
        .iter()
        .filter(|handle| handle.kind == PresentationHandleKind::Image)
    {
        if handle.is_render_visible() {
            if let Some(object) = image_objects
                .iter()
                .find(|object| object.id == handle.resource_id && object.visible)
            {
                upsert_image_object(active, object.clone());
            }
        } else {
            active.retain(|object| object.id != handle.resource_id);
        }
    }
    active.sort_by(|left, right| (left.depth_milli, &left.id).cmp(&(right.depth_milli, &right.id)));
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

pub(crate) fn filter_presentation_scroll_regions(
    regions: Vec<ViewRuntimeScrollRegion>,
    handles: &[PresentationHandleRecord],
) -> Vec<ViewRuntimeScrollRegion> {
    regions
        .into_iter()
        .filter(|region| {
            let mut aliases = vec![region.public_id.as_str(), region.target.as_str()];
            let default_visible = region.view.is_none();
            if let Some(view) = region.view.as_deref() {
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

pub(crate) fn filter_presentation_text_blocks(
    blocks: Vec<ViewRuntimeTextBlock>,
    handles: &[PresentationHandleRecord],
) -> Vec<ViewRuntimeTextBlock> {
    blocks
        .into_iter()
        .filter(|block| {
            let mut aliases = vec![block.public_id.as_str(), block.target.as_str()];
            let default_visible = block.view.is_none();
            if let Some(view) = block.view.as_deref() {
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

const RUNTIME_CONTROL_FAMILIES: [PresentationHandleKind; 5] = [
    PresentationHandleKind::View,
    PresentationHandleKind::Menu,
    PresentationHandleKind::Overlay,
    PresentationHandleKind::TextBox,
    PresentationHandleKind::RuntimeControl,
];

fn resource_is_render_visible(
    handles: &[PresentationHandleRecord],
    kinds: &[PresentationHandleKind],
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
    kinds: &[PresentationHandleKind],
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

fn upsert_image_object(active: &mut Vec<BundleImageObject>, object: BundleImageObject) {
    if let Some(existing) = active.iter_mut().find(|existing| existing.id == object.id) {
        *existing = object;
    } else {
        active.push(object);
    }
}

fn handle_id_arg(call: &RuntimeCall) -> Result<PresentationHandleId, PresentationHandleDiagnostic> {
    let Some(value) = named_runtime_arg(&call.args, "handle").and_then(public_id_arg) else {
        return Err(PresentationHandleDiagnostic::new(
            PresentationHandleDiagnosticCode::InvalidCall,
            None,
            format!("{} requires handle = @handle...", call.callee),
        ));
    };
    PresentationHandleId::try_new(value)
}

fn named_runtime_arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().find_map(|arg| {
        let (arg_name, value) = arg.split_once(" = ")?;
        (arg_name.trim() == name).then_some(value.trim())
    })
}

fn public_id_arg(arg: &str) -> Option<String> {
    let value = clean_runtime_arg(arg);
    let value = value.strip_prefix('@').unwrap_or(value);
    let normalized = value.split_once(":.").map_or_else(
        || value.to_owned(),
        |(family, suffix)| format!("{family}.{suffix}"),
    );
    is_valid_handle_token(&normalized).then_some(normalized)
}

fn clean_runtime_arg(value: &str) -> &str {
    value.trim().trim_matches('"').trim_matches('\'')
}

fn parse_bool_arg(value: &str) -> Option<bool> {
    match clean_runtime_arg(value) {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_i32_arg(value: &str) -> Option<i32> {
    clean_runtime_arg(value).parse::<i32>().ok()
}

fn is_valid_handle_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/' | ':'))
}

const fn default_true() -> bool {
    true
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
        CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, ViewInputKind,
        ViewInputPurpose, ViewSecureInputPolicy, ViewTextSelectionPolicy, ViewTextShortcutPolicy,
        ViewTextTabPolicy, ViewTextVerticalNavigationPolicy,
    };
    use arcweft_bundle::resource_codec::{
        ViewRuntimeControlStyle, ViewRuntimeTextControlBounds, ViewRuntimeTextControlHandlers,
        ViewRuntimeTextControlOptions, ViewRuntimeTextSelection,
    };

    fn handle(value: &str) -> PresentationHandleId {
        PresentationHandleId::try_new(value).expect("valid handle id")
    }

    fn view_handle(
        id: &str,
        resource_id: &str,
        state: PresentationResourceState,
    ) -> PresentationHandleRecord {
        PresentationHandleRecord::new(
            handle(id),
            PresentationHandleKind::View,
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
                purpose: ViewInputPurpose::Text,
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
            handlers: ViewRuntimeTextControlHandlers::default(),
            style: ViewRuntimeControlStyle::default(),
        }
    }

    #[test]
    fn create_hide_and_release_transitions_are_deterministic() {
        let mut handles = Vec::new();
        let mut operation_epoch = 0;
        let operations = vec![
            PresentationHandleOperation::Create {
                id: handle("handle.flow.menu.background"),
                kind: PresentationHandleKind::Image,
                resource_id: "image.menu.background".to_owned(),
                owner: Some("flow.menu/block.0".to_owned()),
                visible: true,
                layer: Some("layer.background".to_owned()),
                depth_milli: -1_000,
            },
            PresentationHandleOperation::Hide {
                id: handle("handle.flow.menu.background"),
            },
            PresentationHandleOperation::Release {
                id: handle("handle.flow.menu.background"),
            },
        ];

        let diagnostics =
            apply_presentation_handle_operations(&mut handles, &mut operation_epoch, &operations);

        assert!(diagnostics.is_empty());
        assert_eq!(operation_epoch, 3);
        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0].state, PresentationResourceState::Released);
        assert_eq!(handles[0].created_epoch, 1);
        assert_eq!(handles[0].updated_epoch, 3);
    }

    #[test]
    fn double_dispose_reports_structured_diagnostic_without_revival() {
        let mut handles = Vec::new();
        let mut operation_epoch = 0;
        let id = handle("handle.flow.menu.overlay");
        let operations = vec![
            PresentationHandleOperation::Create {
                id: id.clone(),
                kind: PresentationHandleKind::Overlay,
                resource_id: "view.MainMenu".to_owned(),
                owner: Some("flow.menu/block.0".to_owned()),
                visible: true,
                layer: None,
                depth_milli: 0,
            },
            PresentationHandleOperation::Dispose { id: id.clone() },
            PresentationHandleOperation::Dispose { id: id.clone() },
            PresentationHandleOperation::Show { id },
        ];

        let diagnostics =
            apply_presentation_handle_operations(&mut handles, &mut operation_epoch, &operations);

        assert_eq!(operation_epoch, 4);
        assert_eq!(handles[0].state, PresentationResourceState::Released);
        assert_eq!(handles[0].created_epoch, 1);
        assert_eq!(handles[0].updated_epoch, 2);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(
            diagnostics[0].code,
            PresentationHandleDiagnosticCode::DoubleDispose
        );
        assert_eq!(
            diagnostics[1].code,
            PresentationHandleDiagnosticCode::TerminalHandle
        );
    }

    #[test]
    fn live_resource_cannot_have_two_owners() {
        let mut handles = Vec::new();
        let mut operation_epoch = 0;
        let operations = vec![
            PresentationHandleOperation::Create {
                id: handle("handle.flow.a.menu"),
                kind: PresentationHandleKind::View,
                resource_id: "view.MainMenu".to_owned(),
                owner: Some("flow.a".to_owned()),
                visible: true,
                layer: None,
                depth_milli: 0,
            },
            PresentationHandleOperation::Create {
                id: handle("handle.flow.b.menu"),
                kind: PresentationHandleKind::View,
                resource_id: "view.MainMenu".to_owned(),
                owner: Some("flow.b".to_owned()),
                visible: true,
                layer: None,
                depth_milli: 0,
            },
        ];

        let diagnostics =
            apply_presentation_handle_operations(&mut handles, &mut operation_epoch, &operations);

        assert_eq!(operation_epoch, 2);
        assert_eq!(handles.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            PresentationHandleDiagnosticCode::ResourceAlreadyOwned
        );
    }

    #[test]
    fn create_is_idempotent_for_same_live_handle() {
        let mut handles = Vec::new();
        let mut operation_epoch = 0;
        let id = handle("handle.flow.main.mount.view.view.MainMenu");
        let operations = vec![
            PresentationHandleOperation::Create {
                id: id.clone(),
                kind: PresentationHandleKind::View,
                resource_id: "view.MainMenu".to_owned(),
                owner: Some("flow.main".to_owned()),
                visible: true,
                layer: Some("layer.controls".to_owned()),
                depth_milli: 100,
            },
            PresentationHandleOperation::Create {
                id,
                kind: PresentationHandleKind::View,
                resource_id: "view.MainMenu".to_owned(),
                owner: Some("flow.main/reentry".to_owned()),
                visible: false,
                layer: Some("layer.overlay".to_owned()),
                depth_milli: 250,
            },
        ];

        let diagnostics =
            apply_presentation_handle_operations(&mut handles, &mut operation_epoch, &operations);

        assert!(diagnostics.is_empty());
        assert_eq!(operation_epoch, 2);
        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0].state, PresentationResourceState::Hidden);
        assert_eq!(handles[0].owner.as_deref(), Some("flow.main/reentry"));
        assert_eq!(handles[0].layer.as_deref(), Some("layer.overlay"));
        assert_eq!(handles[0].depth_milli, 250);
        assert_eq!(handles[0].created_epoch, 1);
        assert_eq!(handles[0].updated_epoch, 2);
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

    #[test]
    fn handle_table_epoch_survives_serde_roundtrip_and_rollback() {
        let mut handles = Vec::new();
        let mut operation_epoch = 0;
        let id = handle("handle.flow.rollback.panel");
        let operations = vec![
            PresentationHandleOperation::Create {
                id: id.clone(),
                kind: PresentationHandleKind::View,
                resource_id: "view.RollbackPanel".to_owned(),
                owner: Some("flow.rollback/block.0".to_owned()),
                visible: true,
                layer: None,
                depth_milli: 0,
            },
            PresentationHandleOperation::Dispose { id: id.clone() },
        ];
        let diagnostics =
            apply_presentation_handle_operations(&mut handles, &mut operation_epoch, &operations);
        assert!(diagnostics.is_empty());
        let rollback_handles = handles.clone();
        let rollback_epoch = operation_epoch;
        let encoded = serde_json::to_string(&handles).expect("handles serialize");
        let decoded: Vec<PresentationHandleRecord> =
            serde_json::from_str(&encoded).expect("handles deserialize");
        assert_eq!(decoded, handles);

        let stale_show = [PresentationHandleOperation::Show { id }];
        let stale_diagnostics =
            apply_presentation_handle_operations(&mut handles, &mut operation_epoch, &stale_show);
        assert_eq!(
            stale_diagnostics[0].code,
            PresentationHandleDiagnosticCode::TerminalHandle
        );
        assert_eq!(handles[0].state, PresentationResourceState::Released);

        handles = rollback_handles;
        operation_epoch = rollback_epoch;
        let stale_diagnostics =
            apply_presentation_handle_operations(&mut handles, &mut operation_epoch, &stale_show);
        assert_eq!(
            stale_diagnostics[0].code,
            PresentationHandleDiagnosticCode::TerminalHandle
        );
        assert_eq!(operation_epoch, rollback_epoch + 1);
        assert_eq!(handles[0].state, PresentationResourceState::Released);
        assert_eq!(handles[0].updated_epoch, rollback_epoch);
    }
}
