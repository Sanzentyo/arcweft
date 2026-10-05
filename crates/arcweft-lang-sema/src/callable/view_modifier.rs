//! Closed semantic identities for standard View modifier callables.

use crate::{
    effect_row::EffectRow,
    effects::EffectSet,
    env::{FunctionParam, FunctionSignature},
    types::{
        GenericParameterOwnerId, GenericTypeParameterId, LanguageIntrinsicGenericOwner, TypeKind,
    },
};

use super::{CallableName, CallableValidator, CheckedCallApplication};

/// Exhaustive semantic role of an accepted standard View modifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewModifierId {
    /// Binds platform-independent activation to a checked handler body.
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
    /// Callback construction and extracted execution have closed empty effects.
    /// The result is inferred from the actual handler body; result-role and
    /// retained-place publication are authenticated by the handler boundary.
    pub fn signature(self) -> FunctionSignature {
        match self {
            Self::OnActivate => FunctionSignature::new(
                TypeKind::ViewValue,
                [FunctionParam::required(
                    "handler",
                    TypeKind::function_with_effects(
                        [],
                        TypeKind::generic_parameter(GenericTypeParameterId::new(
                            GenericParameterOwnerId::LanguageIntrinsic(
                                LanguageIntrinsicGenericOwner::ViewHandlerResult,
                            ),
                            0,
                        )),
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

    pub(crate) fn generic_issuer(self) -> super::CallableGenericParameterIssuer {
        match self {
            Self::OnActivate => super::CallableGenericParameterIssuer::language_intrinsic(
                LanguageIntrinsicGenericOwner::ViewHandlerResult,
                1,
                0,
            )
            .expect("the View handler result has one language-owned type parameter"),
            Self::Fx => super::CallableGenericParameterIssuer::empty(),
        }
    }

    pub const fn event(self) -> Option<arcweft_view::EventKind> {
        match self {
            Self::OnActivate => Some(arcweft_view::EventKind::Activate),
            Self::Fx => None,
        }
    }

    /// Authenticates the closed event result domain against its exact runtime
    /// semantic identity. A nominal spelling cannot issue an action role.
    pub fn handler_value_role(
        self,
        ty: &TypeKind,
    ) -> Option<arcweft_view::ViewHandlerTransitionValueRole> {
        if self != Self::OnActivate {
            return None;
        }
        if *ty == TypeKind::Unit {
            return Some(arcweft_view::ViewHandlerTransitionValueRole::Unit);
        }
        let action = crate::dialogue_view::DialogueRuntimeValueRole::Action.exact_owner();
        ty.semantic_identity_digest()
            .ok()
            .filter(|identity| identity.as_bytes() == action.semantic_identity().as_bytes())
            .map(|_| arcweft_view::ViewHandlerTransitionValueRole::DialogueAction)
    }

    /// Issues an identity for the selected modifier and its exact admitted
    /// callable body. Equal call signatures do not identify equal programs.
    pub fn handler_program_id(
        self,
        application: &CheckedCallApplication,
        program: &crate::final_analysis::CheckedDeterministicProgram,
    ) -> Option<arcweft_view::ViewHandlerProgramId> {
        if self != Self::OnActivate {
            return None;
        }
        if !matches!(
            application
                .core()
                .candidates()
                .selected()
                .schema()
                .validator(),
            CallableValidator::ViewModifier(modifier) if *modifier == self
        ) {
            return None;
        }
        let [argument] = application.core().execution().arguments() else {
            return None;
        };
        let [slot] = argument.slots() else {
            return None;
        };
        let (body, intent) = match program.input_abi().source() {
            crate::final_analysis::CheckedExecutionSource::InvokeBody(
                crate::final_analysis::CheckedExecutionBodyOwner::CallableValue(owner),
            ) => (*owner, 0u8),
            crate::final_analysis::CheckedExecutionSource::ExportMutation(
                crate::final_analysis::CheckedExecutionBodyOwner::CallableValue(owner),
            ) => (*owner, 1u8),
            _ => return None,
        };
        if body != slot.source().owner() {
            return None;
        }
        let coordinate = program
            .input_abi()
            .coordinate()
            .path()
            .canonical_bytes()
            .ok()?;
        let mut digest = blake3::Hasher::new();
        digest.update(b"arcweft.view.handler-program.v1\0");
        digest.update(application.digest().as_bytes());
        digest.update(&[intent]);
        digest.update(&u64::try_from(coordinate.len()).ok()?.to_le_bytes());
        digest.update(&coordinate);
        Some(arcweft_view::ViewHandlerProgramId::from_checked_digest(
            *digest.finalize().as_bytes(),
        ))
    }
}
