//! Presentation callable identities.

use super::{CallableSchemaError, CallableSignatureSchema, ResolvedCharacterOwner};
use crate::env::RegisteredTypeCheckEnv;
use crate::types::{EntityKind, TypeKind};

pub use arcweft_interaction_model::presentation::{
    PresentationArgumentRole, PresentationCallableId,
};

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

impl PresentationSchemaContext<'_> {
    pub fn signature_schema(
        &self,
        id: PresentationCallableId,
    ) -> Result<CallableSignatureSchema, CallableSchemaError> {
        super::schema::presentation_schema(id, self.owner, Some(self.environment))
    }

    pub(crate) fn checker_signature_schema(
        id: PresentationCallableId,
    ) -> Result<CallableSignatureSchema, CallableSchemaError> {
        super::schema::presentation_schema(id, None, None)
    }
}
impl PresentationArgumentValuePolicy {
    pub(crate) fn for_role(role: PresentationArgumentRole) -> PresentationArgumentValuePolicy {
        use PresentationArgumentValuePolicy::{Exact, MetadataScalar, TokenScalar, Unchecked};

        match role {
            PresentationArgumentRole::View => Exact(TypeKind::entity_ref(EntityKind::View)),
            PresentationArgumentRole::ViewValue => Exact(TypeKind::ViewValue),
            PresentationArgumentRole::Asset => Exact(TypeKind::entity_ref(EntityKind::Asset)),
            PresentationArgumentRole::Character => {
                Exact(TypeKind::entity_ref(EntityKind::Character))
            }
            PresentationArgumentRole::TargetEntity => {
                Exact(TypeKind::entity_ref(EntityKind::Target))
            }
            PresentationArgumentRole::TargetPublicId => {
                PresentationArgumentValuePolicy::PublicIdReference(
                    arcweft_id::ProjectEntityReferenceFamily::Target,
                    public_id(EntityKind::Target),
                )
            }
            PresentationArgumentRole::LayerPublicId | PresentationArgumentRole::ProxyLayer => {
                PresentationArgumentValuePolicy::PublicIdReference(
                    arcweft_id::ProjectEntityReferenceFamily::Layer,
                    public_id(EntityKind::Layer),
                )
            }
            PresentationArgumentRole::Slot => Exact(TypeKind::entity_ref(EntityKind::Slot)),
            PresentationArgumentRole::Scope => {
                Exact(TypeKind::entity_ref(EntityKind::Other("scope".to_owned())))
            }
            PresentationArgumentRole::Depth | PresentationArgumentRole::ProxyDepth => {
                Exact(TypeKind::I32)
            }
            PresentationArgumentRole::Enabled
            | PresentationArgumentRole::Visible
            | PresentationArgumentRole::ProxyHitTest => Exact(TypeKind::Bool),
            PresentationArgumentRole::Opacity
            | PresentationArgumentRole::AlignmentX
            | PresentationArgumentRole::AlignmentY
            | PresentationArgumentRole::PlaybackRate
            | PresentationArgumentRole::TransformM11
            | PresentationArgumentRole::TransformM12
            | PresentationArgumentRole::TransformM21
            | PresentationArgumentRole::TransformM22 => TokenScalar(ratio_or_milli()),
            PresentationArgumentRole::PlaybackStart
            | PresentationArgumentRole::PlaybackPausedAt
            | PresentationArgumentRole::PlaybackLocalTime => TokenScalar(playback_time()),
            PresentationArgumentRole::Width | PresentationArgumentRole::Height => {
                TokenScalar(viewport_dimension())
            }
            PresentationArgumentRole::Parameter | PresentationArgumentRole::ProxyParameter => {
                MetadataScalar
            }
            PresentationArgumentRole::Source
            | PresentationArgumentRole::Lifetime
            | PresentationArgumentRole::Fade
            | PresentationArgumentRole::Fit
            | PresentationArgumentRole::Id
            | PresentationArgumentRole::Handle
            | PresentationArgumentRole::Key
            | PresentationArgumentRole::Mount
            | PresentationArgumentRole::Action
            | PresentationArgumentRole::Actions
            | PresentationArgumentRole::X
            | PresentationArgumentRole::Y
            | PresentationArgumentRole::Focus
            | PresentationArgumentRole::InputCapture
            | PresentationArgumentRole::Owner
            | PresentationArgumentRole::Drop
            | PresentationArgumentRole::TransformTx
            | PresentationArgumentRole::TransformTy
            | PresentationArgumentRole::ProxyId
            | PresentationArgumentRole::ProxyType
            | PresentationArgumentRole::ProxyRole
            | PresentationArgumentRole::Look => Unchecked,
        }
    }
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
