//! Exhaustive language-owned Match-domain classification.
//!
//! Exact project/environment/character shapes remain catalog-owned. This
//! module classifies only the intrinsic structural family or the required
//! catalog kind; it never manufactures cases, fields, or source identities.

use super::{AgentBuiltinType, ArrayLength, HandleState, TypeKind, VariantPayloadType};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MatchDomainInvalidity {
    Poison,
    Unsupported,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum MatchDomainFamily<'a> {
    Empty,
    Unit,
    Bool,
    Option(&'a TypeKind),
    Result {
        ok: &'a TypeKind,
        error: &'a TypeKind,
    },
    ProjectNominal,
    Tuple(&'a [TypeKind]),
    Array {
        item: &'a TypeKind,
        length: usize,
    },
    SymbolicSequence(&'a TypeKind),
    Choice(&'a [TypeKind]),
    VariantPayload(&'a VariantPayloadType),
    RequiresClosedVariant,
    OpenOrOpaque,
}

enum MatchDomainInput<'a> {
    Invalid(MatchDomainInvalidity),
    Immediate(MatchDomainFamily<'a>),
    Option(&'a TypeKind),
    Result(&'a TypeKind, &'a TypeKind),
    ProjectNominal(&'a [TypeKind]),
    Tuple(&'a [TypeKind]),
    Array(&'a TypeKind, &'a ArrayLength),
    SymbolicSequence(&'a TypeKind),
    Choice(&'a [TypeKind]),
    VariantPayload(&'a VariantPayloadType),
    AgentBuiltin(AgentBuiltinType),
    OptionalOpenChild(Option<&'a TypeKind>),
    OpenChild(&'a TypeKind),
    OpenChildren(&'a TypeKind, &'a TypeKind),
    OpenManyThenOne(&'a [TypeKind], &'a TypeKind),
    OpenMany(&'a [TypeKind]),
    Handle(HandleState),
}

const UNSUPPORTED_MATCH_DOMAIN: MatchDomainInput<'static> =
    MatchDomainInput::Invalid(MatchDomainInvalidity::Unsupported);

impl TypeKind {
    /// Returns the intrinsic Match-domain family after recursively rejecting
    /// poison and semantic shapes whose exact owner is unresolved.
    pub(crate) fn match_domain_family(
        &self,
    ) -> Result<MatchDomainFamily<'_>, MatchDomainInvalidity> {
        match self.match_domain_input() {
            MatchDomainInput::Invalid(invalidity) => Err(invalidity),
            MatchDomainInput::Immediate(family) => Ok(family),
            MatchDomainInput::Option(item) => {
                ensure_valid(item)?;
                Ok(MatchDomainFamily::Option(item))
            }
            MatchDomainInput::Result(ok, error) => {
                ensure_valid(ok)?;
                ensure_valid(error)?;
                Ok(MatchDomainFamily::Result { ok, error })
            }
            MatchDomainInput::ProjectNominal(arguments) => {
                ensure_all_valid(arguments)?;
                Ok(MatchDomainFamily::ProjectNominal)
            }
            MatchDomainInput::Tuple(items) => {
                ensure_all_valid(items)?;
                Ok(MatchDomainFamily::Tuple(items))
            }
            MatchDomainInput::Array(item, len) => {
                ensure_valid(item)?;
                match len {
                    ArrayLength::Const(length) => Ok(MatchDomainFamily::Array {
                        item,
                        length: *length,
                    }),
                    ArrayLength::Error(_) => Err(MatchDomainInvalidity::Poison),
                    ArrayLength::Generic(_) | ArrayLength::Inferred => {
                        Err(MatchDomainInvalidity::Unsupported)
                    }
                }
            }
            MatchDomainInput::SymbolicSequence(item) => {
                ensure_valid(item)?;
                Ok(MatchDomainFamily::SymbolicSequence(item))
            }
            MatchDomainInput::Choice(alternatives) => {
                ensure_all_valid(alternatives)?;
                Ok(MatchDomainFamily::Choice(alternatives))
            }
            MatchDomainInput::VariantPayload(payload) => {
                payload.shape().visit_types(&mut ensure_valid)?;
                Ok(MatchDomainFamily::VariantPayload(payload))
            }
            MatchDomainInput::AgentBuiltin(builtin) => {
                Ok(if builtin.match_requires_closed_variant() {
                    MatchDomainFamily::RequiresClosedVariant
                } else {
                    MatchDomainFamily::OpenOrOpaque
                })
            }
            MatchDomainInput::OptionalOpenChild(value) => {
                if let Some(value) = value {
                    ensure_valid(value)?;
                }
                Ok(MatchDomainFamily::OpenOrOpaque)
            }
            MatchDomainInput::OpenChild(child) => {
                ensure_valid(child)?;
                Ok(MatchDomainFamily::OpenOrOpaque)
            }
            MatchDomainInput::OpenChildren(left, right) => {
                ensure_valid(left)?;
                ensure_valid(right)?;
                Ok(MatchDomainFamily::OpenOrOpaque)
            }
            MatchDomainInput::OpenManyThenOne(children, tail) => {
                ensure_all_valid(children)?;
                ensure_valid(tail)?;
                Ok(MatchDomainFamily::OpenOrOpaque)
            }
            MatchDomainInput::OpenMany(children) => {
                ensure_all_valid(children)?;
                Ok(MatchDomainFamily::OpenOrOpaque)
            }
            MatchDomainInput::Handle(state) => {
                if state.match_has_live_value() {
                    Ok(MatchDomainFamily::OpenOrOpaque)
                } else {
                    Err(MatchDomainInvalidity::Unsupported)
                }
            }
        }
    }

    fn match_domain_input(&self) -> MatchDomainInput<'_> {
        match self {
            Self::Error(_) => MatchDomainInput::Invalid(MatchDomainInvalidity::Poison),
            Self::Projection { .. } | Self::DialogueLine(_) => UNSUPPORTED_MATCH_DOMAIN,
            Self::Never => MatchDomainInput::Immediate(MatchDomainFamily::Empty),
            Self::Unit => MatchDomainInput::Immediate(MatchDomainFamily::Unit),
            Self::Bool => MatchDomainInput::Immediate(MatchDomainFamily::Bool),
            Self::Option(item) => MatchDomainInput::Option(item),
            Self::Result { ok, error } => MatchDomainInput::Result(ok, error),
            Self::ProjectNominal(nominal) => MatchDomainInput::ProjectNominal(nominal.arguments()),
            Self::Tuple(items) => MatchDomainInput::Tuple(items),
            Self::Array { item, len } => MatchDomainInput::Array(item, len),
            Self::Vec(item) | Self::Seq(item) | Self::Slice(item) => {
                MatchDomainInput::SymbolicSequence(item)
            }
            Self::Choice(alternatives) => MatchDomainInput::Choice(alternatives),
            Self::VariantPayload(payload) => MatchDomainInput::VariantPayload(payload),
            Self::CharacterNominal(_) | Self::AgentResourceBody => {
                MatchDomainInput::Immediate(MatchDomainFamily::RequiresClosedVariant)
            }
            Self::AgentBuiltin(builtin) => MatchDomainInput::AgentBuiltin(*builtin),
            Self::Ref(entity) => MatchDomainInput::OptionalOpenChild(entity.value()),
            Self::Range(inner)
            | Self::Probe(inner)
            | Self::Need(inner)
            | Self::ThreadHandle(inner)
            | Self::Shared(inner)
            | Self::BorrowRef { inner, .. } => MatchDomainInput::OpenChild(inner),
            Self::IteratorState { item, .. } => MatchDomainInput::OpenChild(item),
            Self::Map { key, value, .. }
            | Self::Stream {
                item: key,
                error: value,
            }
            | Self::Parser {
                item: key,
                error: value,
            } => MatchDomainInput::OpenChildren(key, value),
            Self::Function {
                params,
                return_type,
                ..
            } => MatchDomainInput::OpenManyThenOne(params, return_type),
            Self::AcceptedNominal(nominal) => MatchDomainInput::OpenMany(nominal.arguments()),
            Self::OpenNominal(nominal) => MatchDomainInput::OpenMany(nominal.arguments()),
            Self::Handle { state, .. } => MatchDomainInput::Handle(*state),
            Self::I8
            | Self::I16
            | Self::I32
            | Self::I64
            | Self::I128
            | Self::ISize
            | Self::U8
            | Self::U16
            | Self::U32
            | Self::U64
            | Self::U128
            | Self::USize
            | Self::F32
            | Self::F64
            | Self::String
            | Self::Char
            | Self::Bytes
            | Self::TextCluster
            | Self::Duration
            | Self::Progress
            | Self::StageApi(_)
            | Self::LineContext
            | Self::StageActorHandle(_)
            | Self::CueHandle
            | Self::VoiceHandle
            | Self::DisplayText
            | Self::DebugStatePath
            | Self::ObservationFieldPath
            | Self::Predicate
            | Self::Observation
            | Self::ObservedObject
            | Self::AgentBBox
            | Self::ActionName
            | Self::ActionTarget
            | Self::ActionResult
            | Self::AgentValue
            | Self::DataFormat
            | Self::DataShape
            | Self::AgentEntityMetadata
            | Self::AgentSourceAnchor
            | Self::AgentProjectGraphNeighborhood
            | Self::AgentProjectGraphSymbol
            | Self::AgentProjectGraphEdge
            | Self::CaptureTarget
            | Self::CaptureRef
            | Self::AgentResource
            | Self::RagContextPack
            | Self::GenericParam(_)
            | Self::CharacterPatch(_)
            | Self::FocusPatch
            | Self::CharacterDialogue(_)
            | Self::ViewValue
            | Self::Named(_) => MatchDomainInput::Immediate(MatchDomainFamily::OpenOrOpaque),
        }
    }
}

impl AgentBuiltinType {
    const fn match_requires_closed_variant(self) -> bool {
        match self {
            Self::CaptureFormat
            | Self::CaptureKind
            | Self::PointerButton
            | Self::AgentBinaryEncoding => true,
            Self::ObservedObjectId
            | Self::Diagnostics
            | Self::WaitError
            | Self::ViewportPoint
            | Self::RagError
            | Self::AgentSourcePosition
            | Self::AgentProjectFlowControlSummary
            | Self::AgentProjectGraphSummary
            | Self::AgentBinaryBody
            | Self::AgentBinaryData => false,
        }
    }
}

impl HandleState {
    const fn match_has_live_value(self) -> bool {
        match self {
            Self::Live | Self::Detached => true,
            Self::Dropped | Self::MovedOut => false,
        }
    }
}

fn ensure_valid(ty: &TypeKind) -> Result<(), MatchDomainInvalidity> {
    ty.match_domain_family().map(|_| ())
}

fn ensure_all_valid(types: &[TypeKind]) -> Result<(), MatchDomainInvalidity> {
    types.iter().try_for_each(ensure_valid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{LifetimeScopeKind, TypePoisonId};

    #[test]
    fn agent_builtin_match_domains_are_explicitly_closed_or_opaque() {
        for builtin in [
            AgentBuiltinType::CaptureFormat,
            AgentBuiltinType::CaptureKind,
            AgentBuiltinType::PointerButton,
            AgentBuiltinType::AgentBinaryEncoding,
        ] {
            assert!(matches!(
                TypeKind::AgentBuiltin(builtin).match_domain_family(),
                Ok(MatchDomainFamily::RequiresClosedVariant)
            ));
        }
        for builtin in [
            AgentBuiltinType::ObservedObjectId,
            AgentBuiltinType::Diagnostics,
            AgentBuiltinType::WaitError,
            AgentBuiltinType::ViewportPoint,
            AgentBuiltinType::RagError,
            AgentBuiltinType::AgentSourcePosition,
            AgentBuiltinType::AgentProjectFlowControlSummary,
            AgentBuiltinType::AgentProjectGraphSummary,
            AgentBuiltinType::AgentBinaryBody,
            AgentBuiltinType::AgentBinaryData,
        ] {
            assert!(matches!(
                TypeKind::AgentBuiltin(builtin).match_domain_family(),
                Ok(MatchDomainFamily::OpenOrOpaque)
            ));
        }
    }

    #[test]
    fn unresolved_poison_and_dead_handle_domains_fail_closed() {
        let projection = TypeKind::Projection {
            subject: Box::new(TypeKind::Bool),
            trait_name: None,
            assoc: "Item".to_owned(),
        };
        let dead_handle = |state| TypeKind::Handle {
            name: "fixture".to_owned(),
            lifetime: LifetimeScopeKind::Scene,
            state,
            must_drop: true,
        };

        assert!(matches!(
            projection.match_domain_family(),
            Err(MatchDomainInvalidity::Unsupported)
        ));
        assert!(matches!(
            TypeKind::Array {
                item: Box::new(TypeKind::Bool),
                len: ArrayLength::Inferred,
            }
            .match_domain_family(),
            Err(MatchDomainInvalidity::Unsupported)
        ));
        assert!(matches!(
            TypeKind::Error(TypePoisonId::from_index(1)).match_domain_family(),
            Err(MatchDomainInvalidity::Poison)
        ));
        assert!(matches!(
            dead_handle(HandleState::Dropped).match_domain_family(),
            Err(MatchDomainInvalidity::Unsupported)
        ));
        assert!(matches!(
            dead_handle(HandleState::MovedOut).match_domain_family(),
            Err(MatchDomainInvalidity::Unsupported)
        ));
        assert!(matches!(
            dead_handle(HandleState::Live).match_domain_family(),
            Ok(MatchDomainFamily::OpenOrOpaque)
        ));
        assert!(matches!(
            dead_handle(HandleState::Detached).match_domain_family(),
            Ok(MatchDomainFamily::OpenOrOpaque)
        ));
    }

    #[test]
    fn nested_closed_domains_reject_only_unresolved_or_poisoned_children() {
        assert!(matches!(
            TypeKind::Option(Box::new(TypeKind::Projection {
                subject: Box::new(TypeKind::Bool),
                trait_name: None,
                assoc: "Item".to_owned(),
            }))
            .match_domain_family(),
            Err(MatchDomainInvalidity::Unsupported)
        ));
        assert!(matches!(
            TypeKind::Result {
                ok: Box::new(TypeKind::Bool),
                error: Box::new(TypeKind::Error(TypePoisonId::from_index(2))),
            }
            .match_domain_family(),
            Err(MatchDomainInvalidity::Poison)
        ));
        assert!(matches!(
            TypeKind::Option(Box::new(TypeKind::I64)).match_domain_family(),
            Ok(MatchDomainFamily::Option(TypeKind::I64))
        ));
    }
}
