//! Executable presentation operation identities and their authored argument roles.

use super::{PresentationArgumentKey, PresentationArgumentRole, PresentationResourceKind};
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
    Mount,
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
            Self::View | Self::Mount => Handle(Resource::View),
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
    pub const ALL: [Self; 12] = [
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
        Self::Mount,
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
            Self::Mount => "mount",
        }
    }

    pub fn argument_role(self, name: &str) -> Option<PresentationArgumentRole> {
        PresentationArgumentRole::ALL
            .iter()
            .copied()
            .find(|role| self.accepts_argument_role(*role) && role.matches_source_name(name))
    }

    /// Selects an exact typed destination at the source/schema boundary.
    pub fn argument_key(self, name: &str) -> Option<PresentationArgumentKey> {
        let role = self.argument_role(name)?;
        let parameter = role
            .requires_parameter()
            .then(|| name[role.source_name().len()..].to_owned());
        PresentationArgumentKey::try_new(role, parameter).ok()
    }

    /// Closed semantic grammar used by source selection and executable ABI
    /// admission. Public-id targets and entity targets remain distinct roles.
    pub const fn accepts_argument_role(self, role: PresentationArgumentRole) -> bool {
        use PresentationArgumentRole as A;
        match self {
            Self::View | Self::Menu | Self::Overlay | Self::Mount => {
                matches!((self, role), (Self::Mount, A::ViewValue))
                    || (!matches!(self, Self::Mount) && matches!(role, A::View))
                    || matches!(
                        role,
                        A::Lifetime
                            | A::TargetPublicId
                            | A::LayerPublicId
                            | A::Id
                            | A::Handle
                            | A::Key
                            | A::Mount
                            | A::Depth
                            | A::Visible
                            | A::Enabled
                            | A::Focus
                            | A::InputCapture
                            | A::Owner
                            | A::Drop
                    )
            }
            Self::Background => matches!(
                role,
                A::Asset
                    | A::TargetEntity
                    | A::Slot
                    | A::Scope
                    | A::Fade
                    | A::Fit
                    | A::Opacity
                    | A::AlignmentX
                    | A::AlignmentY
                    | A::PlaybackStart
                    | A::PlaybackPausedAt
                    | A::PlaybackLocalTime
                    | A::PlaybackRate
            ),
            Self::Image => matches!(
                role,
                A::Source
                    | A::Asset
                    | A::Lifetime
                    | A::TargetPublicId
                    | A::LayerPublicId
                    | A::Depth
                    | A::Enabled
                    | A::Visible
                    | A::Id
                    | A::Action
                    | A::Actions
                    | A::Fit
                    | A::Opacity
                    | A::X
                    | A::Y
                    | A::Width
                    | A::Height
                    | A::Focus
                    | A::InputCapture
                    | A::Owner
                    | A::Drop
                    | A::AlignmentX
                    | A::AlignmentY
                    | A::PlaybackStart
                    | A::PlaybackPausedAt
                    | A::PlaybackLocalTime
                    | A::PlaybackRate
                    | A::TransformTx
                    | A::TransformTy
                    | A::TransformM11
                    | A::TransformM12
                    | A::TransformM21
                    | A::TransformM22
                    | A::ProxyId
                    | A::ProxyType
                    | A::ProxyRole
                    | A::ProxyLayer
                    | A::ProxyDepth
                    | A::ProxyHitTest
                    | A::Parameter
                    | A::ProxyParameter
            ),
            Self::PlayerViewport => matches!(role, A::Width | A::Height | A::Fit),
            Self::Show => matches!(
                role,
                A::Character | A::Look | A::TargetEntity | A::Slot | A::Scope
            ),
            Self::Hide => matches!(role, A::Character | A::TargetEntity | A::Slot | A::Scope),
            Self::RefShow => matches!(role, A::Character | A::TargetEntity | A::Slot),
            Self::RefBackground => matches!(role, A::TargetEntity | A::Slot),
            Self::ClearBackground => matches!(role, A::TargetEntity | A::Slot | A::Scope),
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
            Self::Mount => 11,
        }
    }
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
