//! Exhaustive collection of declaration-owned generic occurrences.
//!
//! Generic ownership is a property of the checked type graph, not of callable
//! source spelling.  This module is deliberately independent of the callable
//! layer: it only knows the complete [`TypeKind`] algebra and records the
//! exact type/constant identities it encounters.  Callable schema construction
//! supplies an opaque occurrence position when it needs first-use rows.

use std::collections::BTreeMap;
use std::sync::Arc;

use thiserror::Error;

use super::{
    ArrayLength, GenericConstParameterId, GenericParameterOwnerId, GenericTypeParameterId, TypeKind,
};

/// A malformed generic identity or an inferable array length encountered while
/// walking a schema type.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TypeGenericUseError {
    #[error("generic type parameter identity is not valid for its intrinsic owner: {parameter:?}")]
    MalformedTypeParameter { parameter: GenericTypeParameterId },
    #[error(
        "generic constant parameter identity is not valid for a language intrinsic owner: {parameter:?}"
    )]
    MalformedConstParameter { parameter: GenericConstParameterId },
    #[error("array length remains inferable at schema construction")]
    InferableArrayLength,
}

/// Distinct, deterministic generic identities found in one or more checked
/// types.
///
/// The private first-use maps are keyed by the caller-provided opaque position.
/// They let a higher schema owner project the lower collection into its own
/// typed first-use algebra without teaching this layer about callable groups.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeGenericUseInventory {
    types: Arc<[GenericTypeParameterId]>,
    consts: Arc<[GenericConstParameterId]>,
    type_first_use: BTreeMap<GenericTypeParameterId, u32>,
    const_first_use: BTreeMap<GenericConstParameterId, u32>,
}

impl TypeGenericUseInventory {
    pub(crate) fn types(&self) -> &[GenericTypeParameterId] {
        &self.types
    }

    pub(crate) fn consts(&self) -> &[GenericConstParameterId] {
        &self.consts
    }

    pub(crate) fn first_type_use(&self, parameter: &GenericTypeParameterId) -> Option<u32> {
        self.type_first_use.get(parameter).copied()
    }

    pub(crate) fn first_const_use(&self, parameter: &GenericConstParameterId) -> Option<u32> {
        self.const_first_use.get(parameter).copied()
    }
}

/// Exhaustive, metered-free visitor for generic occurrences in [`TypeKind`].
#[derive(Clone, Debug, Default)]
pub(crate) struct TypeGenericUseCollector {
    types: BTreeMap<GenericTypeParameterId, u32>,
    consts: BTreeMap<GenericConstParameterId, u32>,
}

impl TypeGenericUseCollector {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Visits a type at the default position.  This is useful to callers that
    /// only need the deterministic identity inventory.
    pub(crate) fn visit(&mut self, ty: &TypeKind) -> Result<(), TypeGenericUseError> {
        self.visit_at(ty, 0)
    }

    /// Visits a type and records `position` as the first-use coordinate for
    /// every identity first encountered below it.
    pub(crate) fn visit_at(
        &mut self,
        ty: &TypeKind,
        position: u32,
    ) -> Result<(), TypeGenericUseError> {
        match ty {
            TypeKind::Bool
            | TypeKind::I8
            | TypeKind::I16
            | TypeKind::I32
            | TypeKind::I64
            | TypeKind::I128
            | TypeKind::ISize
            | TypeKind::U8
            | TypeKind::U16
            | TypeKind::U32
            | TypeKind::U64
            | TypeKind::U128
            | TypeKind::USize
            | TypeKind::F32
            | TypeKind::F64
            | TypeKind::String
            | TypeKind::Char
            | TypeKind::Bytes
            | TypeKind::TextCluster
            | TypeKind::Duration
            | TypeKind::Progress
            | TypeKind::StageApi(_)
            | TypeKind::LineContext
            | TypeKind::StageActorHandle(_)
            | TypeKind::CueHandle
            | TypeKind::VoiceHandle
            | TypeKind::DisplayText
            | TypeKind::DebugStatePath
            | TypeKind::ObservationFieldPath
            | TypeKind::Predicate
            | TypeKind::Observation
            | TypeKind::ObservedObject
            | TypeKind::AgentBBox
            | TypeKind::ActionName
            | TypeKind::ActionTarget
            | TypeKind::ActionResult
            | TypeKind::AgentValue
            | TypeKind::DataFormat
            | TypeKind::DataShape
            | TypeKind::AgentEntityMetadata
            | TypeKind::AgentSourceAnchor
            | TypeKind::AgentProjectGraphNeighborhood
            | TypeKind::AgentProjectGraphSymbol
            | TypeKind::AgentProjectGraphEdge
            | TypeKind::CaptureTarget
            | TypeKind::CaptureRef
            | TypeKind::AgentResource
            | TypeKind::AgentResourceBody
            | TypeKind::RagContextPack
            | TypeKind::AgentBuiltin(_)
            | TypeKind::FocusPatch
            | TypeKind::CharacterDialogue(_)
            | TypeKind::ViewValue
            | TypeKind::CharacterNominal(_)
            | TypeKind::Named(_)
            | TypeKind::Unit
            | TypeKind::Never
            | TypeKind::Error(_) => Ok(()),
            TypeKind::Range(inner)
            | TypeKind::Probe(inner)
            | TypeKind::Vec(inner)
            | TypeKind::Slice(inner)
            | TypeKind::Seq(inner)
            | TypeKind::Need(inner)
            | TypeKind::Option(inner)
            | TypeKind::ThreadHandle(inner)
            | TypeKind::Shared(inner)
            | TypeKind::DialogueLine(inner) => self.visit_at(inner, position),
            TypeKind::IteratorState { item, .. } => self.visit_at(item, position),
            TypeKind::Array { item, len } => {
                self.visit_at(item, position)?;
                self.visit_array_length(len, position)
            }
            TypeKind::Map { key, value, .. } => {
                self.visit_at(key, position)?;
                self.visit_at(value, position)
            }
            TypeKind::BorrowRef { inner, .. } => self.visit_at(inner, position),
            TypeKind::Stream { item, error } | TypeKind::Result { ok: item, error } => {
                self.visit_at(item, position)?;
                self.visit_at(error, position)
            }
            TypeKind::Function {
                params,
                return_type,
                ..
            } => {
                for parameter in params {
                    self.visit_at(parameter, position)?;
                }
                self.visit_at(return_type, position)
            }
            TypeKind::GenericParam(parameter) => self.visit_type_parameter(parameter, position),
            TypeKind::Ref(entity) => entity
                .value()
                .map_or(Ok(()), |value| self.visit_at(value, position)),
            TypeKind::ProjectNominal(nominal) => {
                for argument in nominal.arguments() {
                    self.visit_at(argument, position)?;
                }
                Ok(())
            }
            TypeKind::AcceptedNominal(nominal) => {
                for argument in nominal.arguments() {
                    self.visit_at(argument, position)?;
                }
                Ok(())
            }
            TypeKind::OpenNominal(nominal) => {
                for argument in nominal.arguments() {
                    self.visit_at(argument, position)?;
                }
                Ok(())
            }
            TypeKind::Projection { subject, .. } => self.visit_at(subject, position),
            TypeKind::CharacterPatch(_) => Ok(()),
            TypeKind::Tuple(items) | TypeKind::Choice(items) => {
                for item in items {
                    self.visit_at(item, position)?;
                }
                Ok(())
            }
            TypeKind::Handle { .. } => Ok(()),
        }
    }

    pub(crate) fn collect(ty: &TypeKind) -> Result<TypeGenericUseInventory, TypeGenericUseError> {
        let mut collector = Self::new();
        collector.visit(ty)?;
        Ok(collector.finish())
    }

    pub(crate) fn collect_many<'a>(
        types: impl IntoIterator<Item = (&'a TypeKind, u32)>,
    ) -> Result<TypeGenericUseInventory, TypeGenericUseError> {
        let mut collector = Self::new();
        for (ty, position) in types {
            collector.visit_at(ty, position)?;
        }
        Ok(collector.finish())
    }

    pub(crate) fn finish(self) -> TypeGenericUseInventory {
        let types = self.types.keys().cloned().collect::<Vec<_>>().into();
        let consts = self.consts.keys().cloned().collect::<Vec<_>>().into();
        TypeGenericUseInventory {
            types,
            consts,
            type_first_use: self.types,
            const_first_use: self.consts,
        }
    }

    pub(crate) fn visit_array_length(
        &mut self,
        length: &ArrayLength,
        position: u32,
    ) -> Result<(), TypeGenericUseError> {
        match length {
            ArrayLength::Const(_) | ArrayLength::Error(_) => Ok(()),
            ArrayLength::Generic(parameter) => self.visit_const_parameter(parameter, position),
            ArrayLength::Inferred => Err(TypeGenericUseError::InferableArrayLength),
        }
    }

    fn visit_type_parameter(
        &mut self,
        parameter: &GenericTypeParameterId,
        position: u32,
    ) -> Result<(), TypeGenericUseError> {
        if !valid_type_ordinal(parameter) {
            return Err(TypeGenericUseError::MalformedTypeParameter {
                parameter: parameter.clone(),
            });
        }
        self.types
            .entry(parameter.clone())
            .and_modify(|first| *first = (*first).min(position))
            .or_insert(position);
        Ok(())
    }

    fn visit_const_parameter(
        &mut self,
        parameter: &GenericConstParameterId,
        position: u32,
    ) -> Result<(), TypeGenericUseError> {
        if matches!(
            parameter.owner(),
            GenericParameterOwnerId::LanguageIntrinsic(_)
        ) {
            return Err(TypeGenericUseError::MalformedConstParameter {
                parameter: parameter.clone(),
            });
        }
        self.consts
            .entry(parameter.clone())
            .and_modify(|first| *first = (*first).min(position))
            .or_insert(position);
        Ok(())
    }
}

fn valid_type_ordinal(parameter: &GenericTypeParameterId) -> bool {
    match parameter.owner() {
        GenericParameterOwnerId::LanguageIntrinsic(owner) => {
            let max = match owner {
                super::LanguageIntrinsicGenericOwner::OptionConstructor
                | super::LanguageIntrinsicGenericOwner::CollectionMap
                | super::LanguageIntrinsicGenericOwner::FxExists
                | super::LanguageIntrinsicGenericOwner::AgentSignal
                | super::LanguageIntrinsicGenericOwner::AgentMetric => 0,
                super::LanguageIntrinsicGenericOwner::ResultConstructor => 1,
            };
            parameter.ordinal() <= max
        }
        GenericParameterOwnerId::Callable(_)
        | GenericParameterOwnerId::Nominal(_)
        | GenericParameterOwnerId::AcceptedNominal(_)
        | GenericParameterOwnerId::AcceptedSource(_)
        | GenericParameterOwnerId::Detached(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        DetachedGenericOwnerId, GenericParameterOwnerId, LanguageIntrinsicGenericOwner,
    };

    fn owner(value: u64) -> GenericParameterOwnerId {
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(value))
    }

    fn parameter(value: u64, ordinal: u16) -> GenericTypeParameterId {
        GenericTypeParameterId::new(owner(value), ordinal)
    }

    fn constant(value: u64, ordinal: u16) -> GenericConstParameterId {
        GenericConstParameterId::new(owner(value), ordinal)
    }

    #[test]
    fn repeated_nested_occurrences_are_sorted_and_coalesced() {
        let first = parameter(1, 1);
        let second = parameter(1, 0);
        let length = constant(2, 0);
        let ty = TypeKind::Map {
            kind: crate::types::MapKind::Ordered,
            key: Box::new(TypeKind::GenericParam(first.clone())),
            value: Box::new(TypeKind::Array {
                item: Box::new(TypeKind::GenericParam(second.clone())),
                len: ArrayLength::Generic(length.clone()),
            }),
        };

        let inventory = TypeGenericUseCollector::collect(&ty).expect("valid generic graph");
        assert_eq!(inventory.types(), &[second, first]);
        assert_eq!(inventory.consts(), &[length]);
    }

    #[test]
    fn array_length_inference_is_rejected_instead_of_dropped() {
        let ty = TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: ArrayLength::Inferred,
        };
        assert_eq!(
            TypeGenericUseCollector::collect(&ty),
            Err(TypeGenericUseError::InferableArrayLength)
        );
    }

    #[test]
    fn every_array_length_constructor_has_typed_behavior() {
        let constant = constant(5, 0);
        let mut collector = TypeGenericUseCollector::new();
        collector
            .visit_array_length(&ArrayLength::Const(3), 0)
            .expect("concrete length");
        collector
            .visit_array_length(
                &ArrayLength::Error(crate::types::TypePoisonId::from_index(2)),
                1,
            )
            .expect("poison length");
        collector
            .visit_array_length(&ArrayLength::Generic(constant.clone()), 2)
            .expect("rigid generic length");
        assert_eq!(collector.finish().consts(), &[constant]);
        assert_eq!(
            TypeGenericUseCollector::new().visit_array_length(&ArrayLength::Inferred, 0),
            Err(TypeGenericUseError::InferableArrayLength)
        );
    }

    #[test]
    fn intrinsic_identity_validation_rejects_wrong_ordinal_and_const_namespace() {
        let wrong_type = TypeKind::GenericParam(GenericTypeParameterId::new(
            GenericParameterOwnerId::LanguageIntrinsic(
                LanguageIntrinsicGenericOwner::OptionConstructor,
            ),
            1,
        ));
        assert!(matches!(
            TypeGenericUseCollector::collect(&wrong_type),
            Err(TypeGenericUseError::MalformedTypeParameter { .. })
        ));

        let wrong_const = TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: ArrayLength::Generic(GenericConstParameterId::new(
                GenericParameterOwnerId::LanguageIntrinsic(
                    LanguageIntrinsicGenericOwner::OptionConstructor,
                ),
                0,
            )),
        };
        assert!(matches!(
            TypeGenericUseCollector::collect(&wrong_const),
            Err(TypeGenericUseError::MalformedConstParameter { .. })
        ));
    }

    #[test]
    fn first_use_position_is_the_first_group_or_result_visit() {
        let parameter = parameter(3, 0);
        let constant = constant(4, 0);
        let result = TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(parameter.clone())),
            len: ArrayLength::Generic(constant.clone()),
        };
        let inventory = TypeGenericUseCollector::collect_many([
            (&TypeKind::GenericParam(parameter.clone()), 1),
            (&result, 2),
        ])
        .expect("valid typed positions");
        assert_eq!(inventory.first_type_use(&parameter), Some(1));
        assert_eq!(inventory.first_const_use(&constant), Some(2));
    }

    #[test]
    fn every_nested_type_constructor_forwards_generic_children() {
        use crate::{
            effect_row::EffectRow,
            effects::EffectSet,
            types::{EntityKind, EntityType, IteratorStateKind, LifetimeScopeKind, MapKind},
        };
        use arcweft_lang_syntax::reference::BorrowKind;

        let generic = |owner| TypeKind::GenericParam(parameter(owner, 0));
        let cases = vec![
            TypeKind::Range(Box::new(generic(100))),
            TypeKind::IteratorState {
                family: IteratorStateKind::Range,
                item: Box::new(generic(101)),
            },
            TypeKind::Ref(EntityType::new(EntityKind::Agent, Some(generic(102)))),
            TypeKind::Probe(Box::new(generic(103))),
            TypeKind::Vec(Box::new(generic(104))),
            TypeKind::Array {
                item: Box::new(generic(105)),
                len: ArrayLength::Const(2),
            },
            TypeKind::Slice(Box::new(generic(106))),
            TypeKind::Seq(Box::new(generic(107))),
            TypeKind::Map {
                kind: MapKind::Ordered,
                key: Box::new(generic(108)),
                value: Box::new(generic(109)),
            },
            TypeKind::BorrowRef {
                kind: BorrowKind::Shared,
                lifetime: Some(LifetimeScopeKind::Frame),
                inner: Box::new(generic(110)),
            },
            TypeKind::Need(Box::new(generic(111))),
            TypeKind::Stream {
                item: Box::new(generic(112)),
                error: Box::new(generic(113)),
            },
            TypeKind::Result {
                ok: Box::new(generic(114)),
                error: Box::new(generic(115)),
            },
            TypeKind::Option(Box::new(generic(116))),
            TypeKind::ThreadHandle(Box::new(generic(117))),
            TypeKind::Shared(Box::new(generic(118))),
            TypeKind::Function {
                params: vec![generic(119)],
                return_type: Box::new(generic(120)),
                effects: EffectRow::closed(EffectSet::new()),
            },
            TypeKind::Projection {
                subject: Box::new(generic(121)),
                trait_name: Some("Trait".to_owned()),
                assoc: "Assoc".to_owned(),
            },
            TypeKind::DialogueLine(Box::new(generic(122))),
            TypeKind::Tuple(vec![generic(123)]),
            TypeKind::Choice(vec![generic(124)]),
        ];
        let positioned = cases
            .iter()
            .enumerate()
            .map(|(position, ty)| (ty, u32::try_from(position).expect("test position fits u32")));
        let inventory = TypeGenericUseCollector::collect_many(positioned)
            .expect("all TypeKind child constructors are traversable");
        assert_eq!(inventory.types().len(), 25);
        assert_eq!(inventory.first_type_use(&parameter(100, 0)), Some(0));
        assert_eq!(inventory.first_type_use(&parameter(124, 0)), Some(20));
    }
}
