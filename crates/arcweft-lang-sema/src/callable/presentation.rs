//! Presentation callable identities.

use super::{CallablePath, CallableSchemaError, CallableSignatureSchema, ResolvedCharacterOwner};
use crate::env::RegisteredTypeCheckEnv;

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
    pub fn resolve(path: &CallablePath) -> Option<Self> {
        [
            (&["view"][..], Self::View),
            (&["menu"][..], Self::Menu),
            (&["overlay"][..], Self::Overlay),
            (&["bg"][..], Self::Background),
            (&["image"][..], Self::Image),
            (&["player_viewport"][..], Self::PlayerViewport),
            (&["show"][..], Self::Show),
            (&["ref", "bg"][..], Self::RefBackground),
            (&["ref", "show"][..], Self::RefShow),
            (&["clear", "bg"][..], Self::ClearBackground),
            (&["hide"][..], Self::Hide),
        ]
        .into_iter()
        .find_map(|(segments, id)| path.matches(segments).then_some(id))
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "the exact public schema API owns its lightweight context value"
    )]
    pub fn signature_schema(
        self,
        context: PresentationSchemaContext<'_>,
    ) -> Result<CallableSignatureSchema, CallableSchemaError> {
        super::schema::presentation_schema(self, context.owner)
    }
}
