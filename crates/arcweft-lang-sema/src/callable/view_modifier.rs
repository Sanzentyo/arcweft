//! Closed semantic identities for standard View modifier callables.

use crate::{
    dialogue_view::DIALOGUE_ACTION_TYPE,
    effect_row::EffectRow,
    effects::EffectSet,
    env::{FunctionParam, FunctionSignature},
    types::TypeKind,
};

use super::{CallableName, CallableValidator, CheckedCallApplication};

/// Exhaustive semantic role of an accepted standard View modifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewModifierId {
    /// Binds platform-independent activation to a pure dialogue-action producer.
    OnActivate,
    /// Applies one checked Fx value to the retained receiver node.
    Fx,
}

impl ViewModifierId {
    pub const ALL: [Self; 2] = [Self::OnActivate, Self::Fx];

    /// Collision-free tag used by the callable-schema semantic transcript.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::OnActivate => 0,
            Self::Fx => 1,
        }
    }

    /// Source-visible registry member owned by this standard row.
    pub fn member(self) -> CallableName {
        match self {
            Self::OnActivate => CallableName::try_new("on_click")
                .expect("the standard View modifier member is canonical"),
            Self::Fx => {
                CallableName::try_new("fx").expect("the standard View modifier member is canonical")
            }
        }
    }

    pub const fn receiver(self) -> TypeKind {
        match self {
            Self::OnActivate | Self::Fx => TypeKind::ViewValue,
        }
    }

    /// Exact signature of this standard modifier.
    ///
    /// Callback construction is pure. The body also has a closed empty effect
    /// row because it produces an immutable `DialogueAction`; transition
    /// effects execute only when that typed value is consumed by its runtime
    /// owner.
    pub fn signature(self) -> FunctionSignature {
        match self {
            Self::OnActivate => FunctionSignature::new(
                TypeKind::ViewValue,
                [FunctionParam::required(
                    "handler",
                    TypeKind::function_with_effects(
                        [],
                        TypeKind::Named(DIALOGUE_ACTION_TYPE.to_owned()),
                        EffectRow::closed(EffectSet::new()),
                    ),
                )],
            ),
            Self::Fx => FunctionSignature::new(
                TypeKind::ViewValue,
                [FunctionParam::required(
                    "value",
                    TypeKind::CompileTimeFx(crate::types::CompileTimeFxType::Abstract),
                )],
            ),
        }
    }

    pub const fn event(self) -> Option<arcweft_view::EventKind> {
        match self {
            Self::OnActivate => Some(arcweft_view::EventKind::Activate),
            Self::Fx => None,
        }
    }

    pub const fn handler_result_role(self) -> Option<arcweft_view::ViewHandlerResultRole> {
        match self {
            Self::OnActivate => Some(arcweft_view::ViewHandlerResultRole::DialogueAction),
            Self::Fx => None,
        }
    }

    pub fn handler_result_type(self) -> Option<TypeKind> {
        match self {
            Self::OnActivate => Some(TypeKind::Named(DIALOGUE_ACTION_TYPE.to_owned())),
            Self::Fx => None,
        }
    }

    /// Issues the stable mount-program identity only for an application whose
    /// exact selected callable row carries this modifier role.
    pub fn handler_program_id(
        self,
        application: &CheckedCallApplication,
    ) -> Option<arcweft_view::ViewHandlerProgramId> {
        if self != Self::OnActivate {
            return None;
        }
        matches!(
            application
                .core()
                .candidates()
                .selected()
                .schema()
                .validator(),
            CallableValidator::ViewModifier(modifier) if *modifier == self
        )
        .then(|| {
            arcweft_view::ViewHandlerProgramId::from_checked_digest(
                *application.digest().as_bytes(),
            )
        })
    }
}
