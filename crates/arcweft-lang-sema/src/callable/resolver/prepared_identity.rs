//! Resolver-owned callable identities retained until the final callable seal.
//!
//! These types deliberately contain no canonical encoder.  They are the
//! checked, move-only projection of the resolver's typed candidate families;
//! generation-local lookup evidence may still be retained by the surrounding
//! prepared callable, but it is not substituted for one of these identities.

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;

use arcweft_lang_hir::{
    identity::{ExprId, LocalId},
    scope::CaptureAccess,
};

use crate::types::CharacterDialogueCharacterType;

use super::super::{
    AgentIntrinsicSignatureId, BuiltinCallableId, CallConstraintInvariant, CallableCandidateId,
    CallableFamily, CapacityMethodId, CheckedCapacityMethodIdentity,
    CheckedDialogueCallableIdentity, CheckedDomainMethodIdentity, CheckedLanguageCallableIdentity,
    CollectionMethodId, ContentCallableIdentity, DialogueCallableId, DomainMethodId,
    DropCallableId, EnumVariantSignatureId, FunctionValueOrdinal, FunctionValueSignatureId,
    FxSourceConstructor, IntegerMethodId, LanguageCallableFamily, LineContextMethodId,
    LineScheduleCallableId, OptionConstructorKind, PresentationCallableId,
    PresentationHandleMethodId, PromotionCallableId, ResolvedCallableBaseInstantiation,
    ResultConstructorKind, StageMethodId,
};
use super::{
    AcceptedEnumVariantCase, PreparedFunctionValueOriginEvidence,
    PreparedFunctionValueOriginProducer, SignatureOrigin,
};
use crate::callable::CheckedCallableId;

/// The typed language-owned callable family projection used by a prepared
/// candidate.  This is intentionally exhaustive: a new language candidate
/// family must add its stable prepared projection at the same time.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PreparedLanguageCallableIdentity {
    FxConstructor(FxSourceConstructor),
    EnumConstructor(EnumVariantSignatureId),
    Result(ResultConstructorKind),
    Option(OptionConstructorKind),
    Builtin(BuiltinCallableId),
    Agent(AgentIntrinsicSignatureId),
    Presentation(PresentationCallableId),
    Dialogue {
        operation: DialogueCallableId,
        callee: PreparedDialogueCalleeIdentity,
    },
    Content(ContentCallableIdentity),
    Collection(CollectionMethodId),
    PresentationHandle(PresentationHandleMethodId),
    Integer(IntegerMethodId),
    Domain(DomainMethodId),
    Capacity(CapacityMethodId),
    Stage(StageMethodId),
    LineContext(LineContextMethodId),
    LineSchedule(LineScheduleCallableId),
    Drop(DropCallableId),
    Promotion(PromotionCallableId),
}

impl PreparedLanguageCallableIdentity {
    /// Consume the resolver-owned language identity into its one exhaustive
    /// stable checked form.  Raw enum signature/name evidence and capacity
    /// method spelling are validated here and never reach the canonical
    /// encoder.
    pub(crate) fn into_checked(
        self,
        candidate: &CallableCandidateId,
        family: CallableFamily,
        instantiation: &ResolvedCallableBaseInstantiation,
    ) -> Result<CheckedLanguageCallableIdentity, CallConstraintInvariant> {
        let checked = match (self, candidate, family) {
            (
                Self::FxConstructor(id),
                CallableCandidateId::FxConstructor(candidate),
                CallableFamily::FxConstructor,
            ) if &id == candidate => CheckedLanguageCallableIdentity::FxConstructor(id),
            (
                Self::EnumConstructor(signature),
                CallableCandidateId::EnumVariant(candidate),
                CallableFamily::EnumConstructor,
            ) if &signature == candidate
                && matches!(
                    instantiation,
                    ResolvedCallableBaseInstantiation::EnumConstructor
                ) =>
            {
                CheckedLanguageCallableIdentity::EnumConstructor {
                    owner: signature.owner(),
                    case: signature.case(),
                }
            }
            (
                Self::Result(id),
                CallableCandidateId::Result(candidate),
                CallableFamily::ResultConstructor,
            ) if &id == candidate => CheckedLanguageCallableIdentity::Result(id),
            (
                Self::Option(id),
                CallableCandidateId::Option(candidate),
                CallableFamily::OptionConstructor,
            ) if &id == candidate => CheckedLanguageCallableIdentity::Option(id),
            (
                Self::Builtin(id),
                CallableCandidateId::Builtin(candidate),
                CallableFamily::Builtin,
            ) if &id == candidate => CheckedLanguageCallableIdentity::Builtin(id),
            (Self::Agent(id), CallableCandidateId::Agent(candidate), CallableFamily::Agent)
                if &id == candidate =>
            {
                CheckedLanguageCallableIdentity::Agent(id)
            }
            (
                Self::Presentation(id),
                CallableCandidateId::Presentation(candidate),
                CallableFamily::Presentation,
            ) if &id == candidate => CheckedLanguageCallableIdentity::Presentation(id),
            (
                Self::Dialogue { operation, callee },
                CallableCandidateId::Dialogue(candidate),
                CallableFamily::Dialogue,
            ) if &operation == candidate => CheckedLanguageCallableIdentity::Dialogue(
                CheckedDialogueCallableIdentity::seal(operation, callee)?,
            ),
            (
                Self::Content(identity),
                CallableCandidateId::Content(candidate),
                CallableFamily::Content,
            ) if &identity == candidate => CheckedLanguageCallableIdentity::Content(identity),
            (
                Self::Collection(id),
                CallableCandidateId::CollectionMethod(candidate),
                CallableFamily::CollectionMethod,
            ) if &id == candidate => CheckedLanguageCallableIdentity::Collection(id),
            (
                Self::PresentationHandle(id),
                CallableCandidateId::PresentationHandleMethod(candidate),
                CallableFamily::PresentationHandleMethod,
            ) if &id == candidate => CheckedLanguageCallableIdentity::PresentationHandle(id),
            (
                Self::Integer(id),
                CallableCandidateId::IntegerMethod(candidate),
                CallableFamily::IntegerMethod,
            ) if &id == candidate => CheckedLanguageCallableIdentity::Integer(id),
            (
                Self::Domain(id),
                CallableCandidateId::DomainMethod(candidate),
                CallableFamily::DomainMethod,
            ) if &id == candidate => {
                CheckedLanguageCallableIdentity::Domain(id.into_checked_identity()?)
            }
            (
                Self::Capacity(id),
                CallableCandidateId::CapacityMethod(candidate),
                CallableFamily::CapacityMethod,
            ) if &id == candidate => {
                CheckedLanguageCallableIdentity::Capacity(CheckedCapacityMethodIdentity::seal(id)?)
            }
            (
                Self::Stage(id),
                CallableCandidateId::StageMethod(candidate),
                CallableFamily::StageMethod,
            ) if &id == candidate => CheckedLanguageCallableIdentity::Stage(id),
            (
                Self::LineContext(id),
                CallableCandidateId::LineContextMethod(candidate),
                CallableFamily::LineContextMethod,
            ) if &id == candidate => CheckedLanguageCallableIdentity::LineContext(id),
            (
                Self::LineSchedule(id),
                CallableCandidateId::LineSchedule(candidate),
                CallableFamily::LineSchedule,
            ) if &id == candidate => CheckedLanguageCallableIdentity::LineSchedule(id),
            (Self::Drop(id), CallableCandidateId::Drop(candidate), CallableFamily::Drop)
                if &id == candidate =>
            {
                CheckedLanguageCallableIdentity::Drop(id)
            }
            (
                Self::Promotion(id),
                CallableCandidateId::Promotion(candidate),
                CallableFamily::Promotion,
            ) if &id == candidate => CheckedLanguageCallableIdentity::Promotion(id),
            _ => return Err(CallConstraintInvariant::PreparedBaseMismatch),
        };
        Ok(checked)
    }
}

impl DomainMethodId {
    fn into_checked_identity(self) -> Result<CheckedDomainMethodIdentity, CallConstraintInvariant> {
        Ok(match self {
            DomainMethodId::FxSampleOrdinalPhase => {
                CheckedDomainMethodIdentity::FxSampleOrdinalPhase
            }
            DomainMethodId::ObservedObjectRequireRole => {
                CheckedDomainMethodIdentity::ObservedObjectRequireRole
            }
            DomainMethodId::MapGet { key, value } => CheckedDomainMethodIdentity::MapGet {
                key: key.semantic_identity_digest()?,
                value: value.semantic_identity_digest()?,
            },
            DomainMethodId::ProbeCompare { value, operation } => {
                CheckedDomainMethodIdentity::ProbeCompare {
                    value: value.semantic_identity_digest()?,
                    operation: operation.operator(),
                }
            }
            DomainMethodId::DiagnosticsHasError => CheckedDomainMethodIdentity::DiagnosticsHasError,
            DomainMethodId::RagContextPackSummary => {
                CheckedDomainMethodIdentity::RagContextPackSummary
            }
            DomainMethodId::Context => CheckedDomainMethodIdentity::Context,
            DomainMethodId::WithContext => CheckedDomainMethodIdentity::WithContext,
        })
    }
}

/// Resolver-stage projection of every accepted callable identity domain.
///
/// The enum is move-only on purpose.  The prepared callable owns it directly;
/// no replayable pointer or clone of the semantic identity is exposed.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PreparedResolvedCallableIdentity {
    Catalog(CheckedCallableId),
    Language(PreparedLanguageCallableIdentity),
    Lexical {
        local: LocalId,
    },
    FunctionValue {
        producer: PreparedFunctionValueOriginIdentity,
        ordinal: FunctionValueOrdinal,
        captures: Box<[PreparedCaptureIdentityRow]>,
    },
}

/// Prepared function-value origin with the use-site removed from identity.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PreparedFunctionValueOriginIdentity {
    IndependentExpression { producer: ExprId },
}

/// Capture rows retained by a prepared function-value identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCaptureIdentityRow {
    local: LocalId,
    mode: CaptureAccess,
}

impl PreparedCaptureIdentityRow {
    pub(crate) const fn new(local: LocalId, mode: CaptureAccess) -> Self {
        Self { local, mode }
    }

    pub(crate) const fn local(self) -> LocalId {
        self.local
    }

    pub(crate) const fn mode(self) -> CaptureAccess {
        self.mode
    }
}

impl PreparedResolvedCallableIdentity {
    /// Returns whether this stable identity is invoked through a runtime value
    /// callee rather than direct catalog/language dispatch.
    pub(crate) const fn requires_value_callee(&self) -> bool {
        matches!(self, Self::Lexical { .. } | Self::FunctionValue { .. })
    }

    /// Projects one fully validated resolver candidate into its typed
    /// prepared identity.  Catalog candidates must have the checked catalog
    /// proof; language candidates must agree with their origin family.  No
    /// unknown/none/derive-later branch is accepted.
    pub(super) fn try_from_parts(
        id: &CallableCandidateId,
        origin: &SignatureOrigin,
        checked: Option<&CheckedCallableId>,
        instantiation: &super::CallableInstantiation,
        enum_seed: Option<&AcceptedEnumVariantCase>,
        function_origin: Option<&PreparedFunctionValueOriginEvidence>,
    ) -> Option<Self> {
        match origin {
            SignatureOrigin::Project { .. }
            | SignatureOrigin::Standard { .. }
            | SignatureOrigin::Adapter { .. } => checked.cloned().map(Self::Catalog),
            SignatureOrigin::Lexical { id: lexical, local } => match id {
                CallableCandidateId::Local(candidate) if candidate == lexical => {
                    Some(Self::Lexical { local: *local })
                }
                _ => None,
            },
            SignatureOrigin::FunctionValue { id: function } => match id {
                CallableCandidateId::FunctionValue(candidate) if candidate == function => {
                    let (producer, ordinal) = function_value_identity(function, function_origin)?;
                    Some(Self::FunctionValue {
                        producer,
                        ordinal,
                        captures: function_origin?.captures().to_vec().into_boxed_slice(),
                    })
                }
                _ => None,
            },
            SignatureOrigin::Language { family } => {
                language_identity(id, *family, instantiation, enum_seed, None)
            }
            SignatureOrigin::LanguageDialogue { operation, callee } => {
                let CallableCandidateId::Dialogue(candidate) = id else {
                    return None;
                };
                (*candidate == *operation).then(|| {
                    Self::Language(PreparedLanguageCallableIdentity::Dialogue {
                        operation: *operation,
                        callee: PreparedDialogueCalleeIdentity::from_resolved(callee),
                    })
                })
            }
        }
    }
}

fn language_identity(
    id: &CallableCandidateId,
    family: LanguageCallableFamily,
    instantiation: &super::CallableInstantiation,
    enum_seed: Option<&AcceptedEnumVariantCase>,
    dialogue: Option<PreparedDialogueCalleeIdentity>,
) -> Option<PreparedResolvedCallableIdentity> {
    let identity = match (id, family) {
        (CallableCandidateId::FxConstructor(id), LanguageCallableFamily::FxConstructor) => {
            PreparedLanguageCallableIdentity::FxConstructor(*id)
        }
        (CallableCandidateId::EnumVariant(id), LanguageCallableFamily::EnumConstructor) => {
            let seed = enum_seed?;
            let super::CallableInstantiation::EnumConstructor = instantiation else {
                return None;
            };
            if seed.id() != id {
                return None;
            }
            PreparedLanguageCallableIdentity::EnumConstructor(id.clone())
        }
        (CallableCandidateId::Result(id), LanguageCallableFamily::ResultConstructor) => {
            PreparedLanguageCallableIdentity::Result(*id)
        }
        (CallableCandidateId::Option(id), LanguageCallableFamily::OptionConstructor) => {
            PreparedLanguageCallableIdentity::Option(*id)
        }
        (CallableCandidateId::Builtin(id), LanguageCallableFamily::Builtin) => {
            PreparedLanguageCallableIdentity::Builtin(id.clone())
        }
        (CallableCandidateId::Agent(id), LanguageCallableFamily::Agent) => {
            PreparedLanguageCallableIdentity::Agent(*id)
        }
        (CallableCandidateId::Presentation(id), LanguageCallableFamily::Presentation) => {
            PreparedLanguageCallableIdentity::Presentation(*id)
        }
        (CallableCandidateId::Dialogue(id), LanguageCallableFamily::Dialogue) => {
            PreparedLanguageCallableIdentity::Dialogue {
                operation: *id,
                callee: dialogue?,
            }
        }
        (CallableCandidateId::Content(identity), LanguageCallableFamily::Content) => {
            PreparedLanguageCallableIdentity::Content(*identity)
        }
        (CallableCandidateId::CollectionMethod(id), LanguageCallableFamily::CollectionMethod) => {
            PreparedLanguageCallableIdentity::Collection(*id)
        }
        (
            CallableCandidateId::PresentationHandleMethod(id),
            LanguageCallableFamily::PresentationHandleMethod,
        ) => PreparedLanguageCallableIdentity::PresentationHandle(*id),
        (CallableCandidateId::IntegerMethod(id), LanguageCallableFamily::IntegerMethod) => {
            PreparedLanguageCallableIdentity::Integer(*id)
        }
        (CallableCandidateId::DomainMethod(id), LanguageCallableFamily::DomainMethod) => {
            PreparedLanguageCallableIdentity::Domain(id.clone())
        }
        (CallableCandidateId::CapacityMethod(id), LanguageCallableFamily::CapacityMethod) => {
            PreparedLanguageCallableIdentity::Capacity(id.clone())
        }
        (CallableCandidateId::StageMethod(id), LanguageCallableFamily::StageMethod) => {
            PreparedLanguageCallableIdentity::Stage(*id)
        }
        (CallableCandidateId::LineContextMethod(id), LanguageCallableFamily::LineContextMethod) => {
            PreparedLanguageCallableIdentity::LineContext(*id)
        }
        (CallableCandidateId::LineSchedule(id), LanguageCallableFamily::LineSchedule) => {
            PreparedLanguageCallableIdentity::LineSchedule(*id)
        }
        (CallableCandidateId::Drop(id), LanguageCallableFamily::Drop) => {
            PreparedLanguageCallableIdentity::Drop(*id)
        }
        (CallableCandidateId::Promotion(id), LanguageCallableFamily::Promote)
        | (CallableCandidateId::Promotion(id), LanguageCallableFamily::Assume) => {
            PreparedLanguageCallableIdentity::Promotion(*id)
        }
        _ => return None,
    };
    Some(PreparedResolvedCallableIdentity::Language(identity))
}

fn function_value_identity(
    signature: &FunctionValueSignatureId,
    origin: Option<&PreparedFunctionValueOriginEvidence>,
) -> Option<(PreparedFunctionValueOriginIdentity, FunctionValueOrdinal)> {
    let producer = match origin?.producer() {
        PreparedFunctionValueOriginProducer::Call(_)
        | PreparedFunctionValueOriginProducer::PreparedContinuation(_)
        | PreparedFunctionValueOriginProducer::Lexical { .. } => return None,
        PreparedFunctionValueOriginProducer::IndependentExpression { producer } => {
            if signature.expression() != *producer {
                return None;
            }
            PreparedFunctionValueOriginIdentity::IndependentExpression {
                producer: *producer,
            }
        }
    };
    Some((producer, signature.ordinal()))
}

/// Prepared Dialogue callee identity.  Content calls carry their canonical
/// module and callable path in the same enum branch, so a content operation
/// cannot exist without the coordinates needed to identify it.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PreparedDialogueCalleeIdentity {
    Character {
        character: CharacterDialogueCharacterType,
    },
    CharacterDialogue {
        character: CharacterDialogueCharacterType,
    },
    Content {
        module: CanonicalModulePath,
        path: super::super::CallablePath,
    },
}

impl PreparedDialogueCalleeIdentity {
    fn from_resolved(callee: &super::super::ResolvedDialogueCalleeIdentity) -> Self {
        match callee {
            super::super::ResolvedDialogueCalleeIdentity::Character { character } => {
                Self::Character {
                    character: character.clone(),
                }
            }
            super::super::ResolvedDialogueCalleeIdentity::CharacterDialogue { character } => {
                Self::CharacterDialogue {
                    character: character.clone(),
                }
            }
            super::super::ResolvedDialogueCalleeIdentity::Content { module, path } => {
                Self::Content {
                    module: module.clone(),
                    path: path.clone(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_dialogue_identity_retains_module_and_callable_path() {
        let module = CanonicalModulePath::crate_root();
        let path = crate::callable::CallablePath::try_new([
            crate::callable::CallableName::try_new("content").expect("callable name"),
            crate::callable::CallableName::try_new("apply").expect("callable name"),
        ])
        .expect("callable path");
        let identity = PreparedDialogueCalleeIdentity::Content {
            module: module.clone(),
            path: path.clone(),
        };
        let PreparedDialogueCalleeIdentity::Content {
            module: actual_module,
            path: actual_path,
        } = identity
        else {
            panic!("content identity must retain content coordinates");
        };
        assert_eq!(actual_module, module);
        assert_eq!(actual_path, path);
    }

    #[test]
    fn content_candidate_maps_to_checked_content_identity_and_family() {
        let definition =
            arcweft_presentation::rich_text::PresentationContentCallableDefinitionId::Strong;
        let id = ContentCallableIdentity::language(
            definition,
            arcweft_presentation::rich_text::PRESENTATION_CONTENT_CALLABLE_CATALOG
                .get(definition)
                .expect("Strong content row")
                .schema_digest(),
        );
        let candidate = CallableCandidateId::Content(id);
        let checked = PreparedLanguageCallableIdentity::Content(id)
            .into_checked(
                &candidate,
                CallableFamily::Content,
                &ResolvedCallableBaseInstantiation::None,
            )
            .expect("content candidate and family must share one checked identity");
        assert_eq!(checked, CheckedLanguageCallableIdentity::Content(id));
    }
}
