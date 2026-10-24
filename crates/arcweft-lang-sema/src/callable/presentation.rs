//! Presentation callable identities.

use super::{CallablePath, CallableSchemaError, CallableSignatureSchema, ResolvedCharacterOwner};
use crate::env::RegisteredTypeCheckEnv;
use crate::types::{EntityKind, TypeKind};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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

/// Semantic destination of a presentation argument. The selected schema owns
/// the role independently of a parameter's source label or ordinal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PresentationArgumentRole {
    View,
    Asset,
    Lifetime,
    TargetEntity,
    TargetPublicId,
    LayerPublicId,
    Slot,
    Scope,
    Fade,
    Fit,
    Opacity,
    Id,
    Handle,
    Key,
    Mount,
    Depth,
    Enabled,
    Visible,
    Action,
    Actions,
    X,
    Y,
    Width,
    Height,
    Focus,
    InputCapture,
    Owner,
    Drop,
    AlignmentX,
    AlignmentY,
    PlaybackStart,
    PlaybackPausedAt,
    PlaybackLocalTime,
    PlaybackRate,
    TransformTx,
    TransformTy,
    TransformM11,
    TransformM12,
    TransformM21,
    TransformM22,
    ProxyId,
    ProxyType,
    ProxyRole,
    ProxyLayer,
    ProxyDepth,
    ProxyHitTest,
    Parameter,
    ProxyParameter,
    Look,
    Source,
    Character,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PresentationArgumentValuePolicy {
    Exact(TypeKind),
    PublicIdReference(arcweft_id::ProjectEntityReferenceFamily, TypeKind),
    TokenScalar(TypeKind),
    Unchecked,
    MetadataScalar,
}

#[derive(Clone, Debug)]
pub struct PresentationSchemaContext<'a> {
    pub owner: Option<&'a ResolvedCharacterOwner>,
    pub environment: &'a RegisteredTypeCheckEnv,
}

impl PartialEq for PresentationSchemaContext<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner && std::ptr::eq(self.environment, other.environment)
    }
}

impl Eq for PresentationSchemaContext<'_> {}

impl PresentationCallableId {
    const ALL: [Self; 11] = [
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

    pub fn resolve(path: &CallablePath) -> Option<Self> {
        Self::resolve_surface_name(&path.dotted_name())
    }

    pub(crate) fn resolve_surface_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.surface_name() == name)
    }

    pub(crate) const fn surface_name(self) -> &'static str {
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

    #[allow(
        clippy::needless_pass_by_value,
        reason = "the exact public schema API owns its lightweight context value"
    )]
    pub fn signature_schema(
        self,
        context: PresentationSchemaContext<'_>,
    ) -> Result<CallableSignatureSchema, CallableSchemaError> {
        super::schema::presentation_schema(self, context.owner, Some(context.environment))
    }

    pub(crate) fn checker_signature_schema(
        self,
    ) -> Result<CallableSignatureSchema, CallableSchemaError> {
        super::schema::presentation_schema(self, None, None)
    }
}

impl PresentationArgumentRole {
    pub(crate) const fn semantic_tag(self) -> u8 {
        match self {
            Self::View => 0,
            Self::Asset => 1,
            Self::Lifetime => 2,
            Self::TargetEntity => 3,
            Self::TargetPublicId => 4,
            Self::LayerPublicId => 5,
            Self::Slot => 6,
            Self::Scope => 7,
            Self::Fade => 8,
            Self::Fit => 9,
            Self::Opacity => 10,
            Self::Id => 11,
            Self::Handle => 12,
            Self::Key => 13,
            Self::Mount => 14,
            Self::Depth => 15,
            Self::Enabled => 16,
            Self::Visible => 17,
            Self::Action => 18,
            Self::Actions => 19,
            Self::X => 20,
            Self::Y => 21,
            Self::Width => 22,
            Self::Height => 23,
            Self::Focus => 24,
            Self::InputCapture => 25,
            Self::Owner => 26,
            Self::Drop => 27,
            Self::AlignmentX => 28,
            Self::AlignmentY => 29,
            Self::PlaybackStart => 30,
            Self::PlaybackPausedAt => 31,
            Self::PlaybackLocalTime => 32,
            Self::PlaybackRate => 33,
            Self::TransformTx => 34,
            Self::TransformTy => 35,
            Self::TransformM11 => 36,
            Self::TransformM12 => 37,
            Self::TransformM21 => 38,
            Self::TransformM22 => 39,
            Self::ProxyId => 40,
            Self::ProxyType => 41,
            Self::ProxyRole => 42,
            Self::ProxyLayer => 43,
            Self::ProxyDepth => 44,
            Self::ProxyHitTest => 45,
            Self::Parameter => 46,
            Self::ProxyParameter => 47,
            Self::Look => 48,
            Self::Source => 49,
            Self::Character => 50,
        }
    }

    pub(crate) fn value_policy(self) -> PresentationArgumentValuePolicy {
        use PresentationArgumentValuePolicy::{Exact, MetadataScalar, TokenScalar, Unchecked};

        match self {
            Self::View => Exact(TypeKind::entity_ref(EntityKind::View)),
            Self::Asset => Exact(TypeKind::entity_ref(EntityKind::Asset)),
            Self::Character => Exact(TypeKind::entity_ref(EntityKind::Character)),
            Self::TargetEntity => Exact(TypeKind::entity_ref(EntityKind::Target)),
            Self::TargetPublicId => PresentationArgumentValuePolicy::PublicIdReference(
                arcweft_id::ProjectEntityReferenceFamily::Target,
                public_id(EntityKind::Target),
            ),
            Self::LayerPublicId | Self::ProxyLayer => {
                PresentationArgumentValuePolicy::PublicIdReference(
                    arcweft_id::ProjectEntityReferenceFamily::Layer,
                    public_id(EntityKind::Layer),
                )
            }
            Self::Slot => Exact(TypeKind::entity_ref(EntityKind::Slot)),
            Self::Scope => Exact(TypeKind::entity_ref(EntityKind::Other("scope".to_owned()))),
            Self::Depth | Self::ProxyDepth => Exact(TypeKind::I32),
            Self::Enabled | Self::Visible | Self::ProxyHitTest => Exact(TypeKind::Bool),
            Self::Opacity
            | Self::AlignmentX
            | Self::AlignmentY
            | Self::PlaybackRate
            | Self::TransformM11
            | Self::TransformM12
            | Self::TransformM21
            | Self::TransformM22 => TokenScalar(ratio_or_milli()),
            Self::PlaybackStart | Self::PlaybackPausedAt | Self::PlaybackLocalTime => {
                TokenScalar(playback_time())
            }
            Self::Width | Self::Height => TokenScalar(viewport_dimension()),
            Self::Parameter | Self::ProxyParameter => MetadataScalar,
            Self::Source
            | Self::Lifetime
            | Self::Fade
            | Self::Fit
            | Self::Id
            | Self::Handle
            | Self::Key
            | Self::Mount
            | Self::Action
            | Self::Actions
            | Self::X
            | Self::Y
            | Self::Focus
            | Self::InputCapture
            | Self::Owner
            | Self::Drop
            | Self::TransformTx
            | Self::TransformTy
            | Self::ProxyId
            | Self::ProxyType
            | Self::ProxyRole
            | Self::Look => Unchecked,
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

fn ratio_or_milli() -> TypeKind {
    TypeKind::Choice(vec![TypeKind::I32, TypeKind::F64, TypeKind::String])
}

fn playback_time() -> TypeKind {
    TypeKind::Choice(vec![
        TypeKind::Duration,
        TypeKind::I32,
        TypeKind::F64,
        TypeKind::String,
    ])
}

fn viewport_dimension() -> TypeKind {
    TypeKind::Choice(vec![
        TypeKind::Named("Length".to_owned()),
        TypeKind::I32,
        TypeKind::F64,
        TypeKind::String,
    ])
}

fn public_id(kind: EntityKind) -> TypeKind {
    TypeKind::Choice(vec![TypeKind::entity_ref(kind), TypeKind::String])
}
