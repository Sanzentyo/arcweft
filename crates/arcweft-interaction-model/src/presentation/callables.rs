//! Executable presentation operation identities and their authored argument roles.

use super::{PresentationArgumentRole, PresentationResourceKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationCallableId {
    View,
    Menu,
    Overlay,
    Background,
    Image,
    PlayerViewport,
    Show,
    RefBackground,
    RefShow,
    ClearBackground,
    Hide,
}

/// Ownership of the value returned by one executable presentation call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationCallableResult {
    Handle(PresentationResourceKind),
    SlotRef(PresentationResourceKind),
    Removed(PresentationResourceKind),
}

impl PresentationCallableId {
    /// Resource family and ownership are selected together, before execution.
    #[must_use]
    pub const fn result(self) -> PresentationCallableResult {
        use PresentationCallableResult::{Handle, Removed, SlotRef};
        use PresentationResourceKind as Resource;
        match self {
            Self::View => Handle(Resource::View),
            Self::Menu => Handle(Resource::Menu),
            Self::Overlay => Handle(Resource::Overlay),
            Self::Background => Handle(Resource::Background),
            Self::Image => Handle(Resource::Image),
            Self::PlayerViewport => Handle(Resource::Viewport),
            Self::Show => Handle(Resource::Character),
            Self::RefBackground => SlotRef(Resource::Background),
            Self::RefShow => SlotRef(Resource::Character),
            Self::ClearBackground => Removed(Resource::Background),
            Self::Hide => Removed(Resource::Character),
        }
    }
    pub const ALL: [Self; 11] = [
        Self::View,
        Self::Menu,
        Self::Overlay,
        Self::Background,
        Self::Image,
        Self::PlayerViewport,
        Self::Show,
        Self::RefBackground,
        Self::RefShow,
        Self::ClearBackground,
        Self::Hide,
    ];

    pub fn resolve_surface_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.surface_name() == name)
    }

    pub const fn surface_name(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Menu => "menu",
            Self::Overlay => "overlay",
            Self::Background => "bg",
            Self::Image => "image",
            Self::PlayerViewport => "player_viewport",
            Self::Show => "show",
            Self::RefBackground => "ref.bg",
            Self::RefShow => "ref.show",
            Self::ClearBackground => "clear.bg",
            Self::Hide => "hide",
        }
    }

    pub fn argument_role(self, name: &str) -> Option<PresentationArgumentRole> {
        match self {
            Self::View | Self::Menu | Self::Overlay => view_argument(name),
            Self::Background => background_argument(name),
            Self::Image => image_argument(name),
            Self::PlayerViewport => viewport_argument(name),
            Self::Show => character_argument(name, true),
            Self::RefShow | Self::Hide => character_argument(name, false),
            Self::RefBackground | Self::ClearBackground => background_reference_argument(name),
        }
    }

    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::View => 0,
            Self::Menu => 1,
            Self::Overlay => 2,
            Self::Background => 3,
            Self::Image => 4,
            Self::PlayerViewport => 5,
            Self::Show => 6,
            Self::RefBackground => 7,
            Self::RefShow => 8,
            Self::ClearBackground => 9,
            Self::Hide => 10,
        }
    }
}

fn view_argument(name: &str) -> Option<PresentationArgumentRole> {
    use PresentationArgumentRole as Argument;

    Some(match name {
        "view" => Argument::View,
        "lifetime" => Argument::Lifetime,
        "target" => Argument::TargetPublicId,
        "layer" => Argument::LayerPublicId,
        "id" => Argument::Id,
        "handle" => Argument::Handle,
        "key" => Argument::Key,
        "mount" => Argument::Mount,
        "depth" => Argument::Depth,
        "visible" => Argument::Visible,
        "enabled" => Argument::Enabled,
        "focus" => Argument::Focus,
        "input_capture" => Argument::InputCapture,
        "owner" => Argument::Owner,
        "drop" => Argument::Drop,
        _ => return None,
    })
}

fn background_argument(name: &str) -> Option<PresentationArgumentRole> {
    use PresentationArgumentRole as Argument;

    Some(match name {
        "asset" => Argument::Asset,
        "target" => Argument::TargetEntity,
        "slot" => Argument::Slot,
        "scope" => Argument::Scope,
        "fade" => Argument::Fade,
        "fit" => Argument::Fit,
        "opacity" => Argument::Opacity,
        "alignment.x" => Argument::AlignmentX,
        "alignment.y" => Argument::AlignmentY,
        "playback.start" => Argument::PlaybackStart,
        "playback.paused_at" => Argument::PlaybackPausedAt,
        "playback.local_time" => Argument::PlaybackLocalTime,
        "playback.rate" => Argument::PlaybackRate,
        _ => return None,
    })
}

fn image_argument(name: &str) -> Option<PresentationArgumentRole> {
    use PresentationArgumentRole as Argument;

    Some(match name {
        "source" => Argument::Source,
        "asset" => Argument::Asset,
        "lifetime" => Argument::Lifetime,
        "target" => Argument::TargetPublicId,
        "layer" => Argument::LayerPublicId,
        "depth" => Argument::Depth,
        "enabled" => Argument::Enabled,
        "visible" => Argument::Visible,
        "id" => Argument::Id,
        "action" => Argument::Action,
        "actions" => Argument::Actions,
        "fit" => Argument::Fit,
        "opacity" => Argument::Opacity,
        "x" => Argument::X,
        "y" => Argument::Y,
        "width" => Argument::Width,
        "height" => Argument::Height,
        "focus" => Argument::Focus,
        "input_capture" => Argument::InputCapture,
        "owner" => Argument::Owner,
        "drop" => Argument::Drop,
        "alignment.x" => Argument::AlignmentX,
        "alignment.y" => Argument::AlignmentY,
        "playback.start" => Argument::PlaybackStart,
        "playback.paused_at" => Argument::PlaybackPausedAt,
        "playback.local_time" => Argument::PlaybackLocalTime,
        "playback.rate" => Argument::PlaybackRate,
        "transform.tx" => Argument::TransformTx,
        "transform.ty" => Argument::TransformTy,
        "transform.m11" => Argument::TransformM11,
        "transform.m12" => Argument::TransformM12,
        "transform.m21" => Argument::TransformM21,
        "transform.m22" => Argument::TransformM22,
        "proxy.id" => Argument::ProxyId,
        "proxy.type" => Argument::ProxyType,
        "proxy.role" => Argument::ProxyRole,
        "proxy.layer" => Argument::ProxyLayer,
        "proxy.depth" => Argument::ProxyDepth,
        "proxy.hit_test" => Argument::ProxyHitTest,
        _ if name.starts_with("param.") => Argument::Parameter,
        _ if name.starts_with("proxy.param.") => Argument::ProxyParameter,
        _ => return None,
    })
}

fn viewport_argument(name: &str) -> Option<PresentationArgumentRole> {
    use PresentationArgumentRole as Argument;

    Some(match name {
        "width" => Argument::Width,
        "height" => Argument::Height,
        "fit" => Argument::Fit,
        _ => return None,
    })
}

fn character_argument(name: &str, include_look: bool) -> Option<PresentationArgumentRole> {
    use PresentationArgumentRole as Argument;

    Some(match name {
        "character" => Argument::Character,
        "look" if include_look => Argument::Look,
        "target" => Argument::TargetEntity,
        "slot" => Argument::Slot,
        "scope" => Argument::Scope,
        _ => return None,
    })
}

fn background_reference_argument(name: &str) -> Option<PresentationArgumentRole> {
    use PresentationArgumentRole as Argument;

    Some(match name {
        "target" => Argument::TargetEntity,
        "slot" => Argument::Slot,
        "scope" => Argument::Scope,
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationHandleMethodId {
    Show,
    Hide,
    Unmount,
    Release,
    Destroy,
    OverlayPop,
}

impl PresentationHandleMethodId {
    /// Universal handle methods also apply before a generic item is closed.
    /// `pop` requires the exact Overlay resource rather than an unknown family.
    pub fn resolve(resource: Option<PresentationResourceKind>, method: &str) -> Option<Self> {
        match method {
            "show" => Some(Self::Show),
            "hide" => Some(Self::Hide),
            "unmount" => Some(Self::Unmount),
            "release" => Some(Self::Release),
            "destroy" => Some(Self::Destroy),
            "pop" if resource == Some(PresentationResourceKind::Overlay) => Some(Self::OverlayPop),
            _ => None,
        }
    }
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Show => 0,
            Self::Hide => 1,
            Self::Unmount => 2,
            Self::Release => 3,
            Self::Destroy => 4,
            Self::OverlayPop => 5,
        }
    }
}
