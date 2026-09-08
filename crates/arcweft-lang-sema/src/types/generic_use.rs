//! Exhaustive collection of declaration-owned generic occurrences.
//!
//! Generic ownership is a property of the checked type graph, not of callable
//! source spelling.  This module is deliberately independent of the callable
//! layer: it only knows the complete [`TypeKind`] algebra and records the
//! exact type, constant, and effect identities it encounters. Callable schema construction
//! supplies an opaque occurrence position when it needs first-use rows.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use thiserror::Error;

use crate::effect_row::{EffectRowTail, EffectVar};

use super::{
    ArrayLength, GenericConstParameterId, GenericConstReference, GenericParameterKind,
    GenericParameterOwnerId, GenericScope, GenericScopeError, GenericTypeParameterId,
    GenericTypeReference, TypeKind,
};

#[cfg(test)]
pub(crate) type TypeGenericUseCollector = GenericUseCollector<DeclarationUses>;
pub(crate) type TypeGenericReferenceUseCollector = GenericUseCollector<ReferenceUses>;
pub(crate) type StableGenericReferenceUseCollector = GenericUseCollector<StableReferenceUses>;

/// The same structural visitor serves stable declaration inventories and
/// application-local hint inventories. Only the latter admits inference atoms.
pub(crate) trait GenericUseDomain {
    type TypeKey: Clone + Ord + std::fmt::Debug;
    type ConstKey: Clone + Ord + std::fmt::Debug;
    fn type_key(
        reference: &GenericTypeReference,
        scope: &GenericScope,
        local_depth: usize,
    ) -> Result<Option<Self::TypeKey>, TypeGenericUseError>;
    fn const_key(
        reference: &GenericConstReference,
        scope: &GenericScope,
        local_depth: usize,
    ) -> Result<Option<Self::ConstKey>, TypeGenericUseError>;
}

#[derive(Clone, Debug)]
pub(crate) struct DeclarationUses;

#[derive(Clone, Debug)]
pub(crate) struct ReferenceUses;

#[derive(Clone, Debug)]
pub(crate) struct StableReferenceUses;

impl GenericUseDomain for StableReferenceUses {
    type TypeKey = GenericTypeReference;
    type ConstKey = GenericConstReference;

    fn type_key(
        reference: &GenericTypeReference,
        scope: &GenericScope,
        local_depth: usize,
    ) -> Result<Option<Self::TypeKey>, TypeGenericUseError> {
        if matches!(reference, GenericTypeReference::Inference(_)) {
            return Err(GenericScopeError::EscapedInference {
                kind: GenericParameterKind::Type,
            }
            .into());
        }
        ReferenceUses::type_key(reference, scope, local_depth)
    }

    fn const_key(
        reference: &GenericConstReference,
        scope: &GenericScope,
        local_depth: usize,
    ) -> Result<Option<Self::ConstKey>, TypeGenericUseError> {
        if matches!(reference, GenericConstReference::Inference(_)) {
            return Err(GenericScopeError::EscapedInference {
                kind: GenericParameterKind::Const,
            }
            .into());
        }
        ReferenceUses::const_key(reference, scope, local_depth)
    }
}

impl GenericUseDomain for DeclarationUses {
    type TypeKey = GenericTypeParameterId;
    type ConstKey = GenericConstParameterId;

    fn type_key(
        reference: &GenericTypeReference,
        scope: &GenericScope,
        _local_depth: usize,
    ) -> Result<Option<Self::TypeKey>, TypeGenericUseError> {
        match reference {
            GenericTypeReference::Free(parameter) if valid_type_ordinal(parameter) => {
                Ok(Some(parameter.clone()))
            }
            GenericTypeReference::Free(parameter) => {
                Err(TypeGenericUseError::MalformedTypeParameter {
                    parameter: parameter.clone(),
                })
            }
            GenericTypeReference::Bound(parameter) => {
                scope.bound_type(parameter.depth(), parameter.slot())?;
                Ok(None)
            }
            GenericTypeReference::Inference(_) => Err(GenericScopeError::EscapedInference {
                kind: GenericParameterKind::Type,
            }
            .into()),
        }
    }

    fn const_key(
        reference: &GenericConstReference,
        scope: &GenericScope,
        _local_depth: usize,
    ) -> Result<Option<Self::ConstKey>, TypeGenericUseError> {
        match reference {
            GenericConstReference::Free(parameter) if valid_const_ordinal(parameter) => {
                Ok(Some(parameter.clone()))
            }
            GenericConstReference::Free(parameter) => {
                Err(TypeGenericUseError::MalformedConstParameter {
                    parameter: parameter.clone(),
                })
            }
            GenericConstReference::Bound(parameter) => {
                scope.bound_const(parameter.depth(), parameter.slot())?;
                Ok(None)
            }
            GenericConstReference::Inference(_) => Err(GenericScopeError::EscapedInference {
                kind: GenericParameterKind::Const,
            }
            .into()),
        }
    }
}

impl GenericUseDomain for ReferenceUses {
    type TypeKey = GenericTypeReference;
    type ConstKey = GenericConstReference;

    fn type_key(
        reference: &GenericTypeReference,
        scope: &GenericScope,
        local_depth: usize,
    ) -> Result<Option<Self::TypeKey>, TypeGenericUseError> {
        match reference {
            GenericTypeReference::Inference(_) => Ok(Some(reference.clone())),
            GenericTypeReference::Bound(_) => reference
                .template_key(&scope.without_inner(local_depth)?, scope)
                .map_err(Into::into),
            _ => DeclarationUses::type_key(reference, scope, local_depth)
                .map(|key| key.map(GenericTypeReference::Free)),
        }
    }

    fn const_key(
        reference: &GenericConstReference,
        scope: &GenericScope,
        local_depth: usize,
    ) -> Result<Option<Self::ConstKey>, TypeGenericUseError> {
        match reference {
            GenericConstReference::Inference(_) => Ok(Some(reference.clone())),
            GenericConstReference::Bound(_) => reference
                .template_key(&scope.without_inner(local_depth)?, scope)
                .map_err(Into::into),
            _ => DeclarationUses::const_key(reference, scope, local_depth)
                .map(|key| key.map(GenericConstReference::Free)),
        }
    }
}

/// A malformed generic identity or an inferable array length encountered while
/// walking a schema type.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TypeGenericUseError {
    #[error(transparent)]
    Scope(#[from] GenericScopeError),
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
pub(crate) struct TypeGenericUseInventory<T = GenericTypeParameterId, C = GenericConstParameterId> {
    types: Arc<[T]>,
    consts: Arc<[C]>,
    effects: Arc<[EffectVar]>,
    type_first_use: BTreeMap<T, u32>,
    const_first_use: BTreeMap<C, u32>,
}

impl<T: Ord, C: Ord> TypeGenericUseInventory<T, C> {
    pub(crate) fn types(&self) -> &[T] {
        &self.types
    }

    pub(crate) fn consts(&self) -> &[C] {
        &self.consts
    }

    pub(crate) fn effects(&self) -> &[EffectVar] {
        &self.effects
    }

    pub(crate) fn first_type_use(&self, parameter: &T) -> Option<u32> {
        self.type_first_use.get(parameter).copied()
    }

    pub(crate) fn first_const_use(&self, parameter: &C) -> Option<u32> {
        self.const_first_use.get(parameter).copied()
    }
}

/// Exhaustive, metered-free visitor for generic occurrences in [`TypeKind`].
#[derive(Clone, Debug)]
pub(crate) struct GenericUseCollector<M: GenericUseDomain> {
    scope: GenericScope,
    incoming_depth: usize,
    types: BTreeMap<M::TypeKey, u32>,
    consts: BTreeMap<M::ConstKey, u32>,
    effects: BTreeSet<EffectVar>,
    domain: std::marker::PhantomData<M>,
}

impl<M: GenericUseDomain> GenericUseCollector<M> {
    pub(crate) fn new() -> Self {
        Self {
            scope: GenericScope::default(),
            incoming_depth: 0,
            types: BTreeMap::new(),
            consts: BTreeMap::new(),
            effects: BTreeSet::new(),
            domain: std::marker::PhantomData,
        }
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
            | TypeKind::StatementIngress(_)
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
            | TypeKind::Error(_)
            | TypeKind::CompileTimeCallable(_)
            | TypeKind::CompileTimeScalar(_)
            | TypeKind::CompileTimeEnum(_)
            | TypeKind::CompileTimeFx(_) => Ok(()),
            TypeKind::FixedVector(vector) => self.visit_at(vector.component(), position),
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
            TypeKind::MetaType(inner) => self.visit_at(inner, position),
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
            TypeKind::Stream { item, error }
            | TypeKind::Parser { item, error }
            | TypeKind::Result { ok: item, error } => {
                self.visit_at(item, position)?;
                self.visit_at(error, position)
            }
            TypeKind::Function {
                binder,
                params,
                return_type,
                effects,
            } => {
                if let EffectRowTail::Variable(variable) = effects.tail() {
                    self.effects.insert(variable);
                }
                let nested = self.scope.with_binder(*binder);
                let enclosing = std::mem::replace(&mut self.scope, nested);
                let result = (|| {
                    for parameter in params {
                        self.visit_at(parameter, position)?;
                    }
                    self.visit_at(return_type, position)
                })();
                self.scope = enclosing;
                result
            }
            TypeKind::GenericParam(reference) => {
                if let Some(key) = M::type_key(
                    reference,
                    &self.scope,
                    self.scope.binders().len() - self.incoming_depth,
                )? {
                    self.types
                        .entry(key)
                        .and_modify(|first| *first = (*first).min(position))
                        .or_insert(position);
                }
                Ok(())
            }
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
            TypeKind::VariantPayload(payload) => {
                payload.visit_types(&mut |field| self.visit_at(field, position))
            }
            TypeKind::Handle { .. } => Ok(()),
        }
    }

    pub(crate) fn collect(
        ty: &TypeKind,
    ) -> Result<TypeGenericUseInventory<M::TypeKey, M::ConstKey>, TypeGenericUseError> {
        Self::collect_in_scope(ty, &GenericScope::default())
    }

    pub(crate) fn collect_in_scope(
        ty: &TypeKind,
        scope: &GenericScope,
    ) -> Result<TypeGenericUseInventory<M::TypeKey, M::ConstKey>, TypeGenericUseError> {
        let mut collector = Self::new();
        collector.scope = scope.clone();
        collector.incoming_depth = scope.binders().len();
        collector.visit(ty)?;
        Ok(collector.finish())
    }

    #[cfg(test)]
    pub(crate) fn collect_many<'a>(
        types: impl IntoIterator<Item = (&'a TypeKind, u32)>,
    ) -> Result<TypeGenericUseInventory<M::TypeKey, M::ConstKey>, TypeGenericUseError> {
        Self::collect_many_in_scope(types, &GenericScope::default())
    }

    pub(crate) fn collect_many_in_scope<'a>(
        types: impl IntoIterator<Item = (&'a TypeKind, u32)>,
        scope: &GenericScope,
    ) -> Result<TypeGenericUseInventory<M::TypeKey, M::ConstKey>, TypeGenericUseError> {
        let mut collector = Self::new();
        collector.scope = scope.clone();
        collector.incoming_depth = scope.binders().len();
        for (ty, position) in types {
            collector.visit_at(ty, position)?;
        }
        Ok(collector.finish())
    }

    pub(crate) fn finish(self) -> TypeGenericUseInventory<M::TypeKey, M::ConstKey> {
        let types = self.types.keys().cloned().collect::<Vec<_>>().into();
        let consts = self.consts.keys().cloned().collect::<Vec<_>>().into();
        TypeGenericUseInventory {
            types,
            consts,
            effects: self.effects.into_iter().collect(),
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
            ArrayLength::Generic(reference) => {
                if let Some(key) = M::const_key(
                    reference,
                    &self.scope,
                    self.scope.binders().len() - self.incoming_depth,
                )? {
                    self.consts
                        .entry(key)
                        .and_modify(|first| *first = (*first).min(position))
                        .or_insert(position);
                }
                Ok(())
            }
            ArrayLength::Inferred => Err(TypeGenericUseError::InferableArrayLength),
        }
    }
}

fn valid_type_ordinal(parameter: &GenericTypeParameterId) -> bool {
    match parameter.owner() {
        GenericParameterOwnerId::LanguageIntrinsic(owner) => {
            let (type_count, _) = owner.generic_arity();
            parameter.ordinal() < type_count
        }
        GenericParameterOwnerId::Callable(_)
        | GenericParameterOwnerId::Nominal(_)
        | GenericParameterOwnerId::AcceptedNominal(_)
        | GenericParameterOwnerId::AcceptedSource(_)
        | GenericParameterOwnerId::Detached(_) => true,
    }
}

fn valid_const_ordinal(parameter: &GenericConstParameterId) -> bool {
    match parameter.owner() {
        GenericParameterOwnerId::LanguageIntrinsic(owner) => {
            let (_, const_count) = owner.generic_arity();
            parameter.ordinal() < const_count
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
            key: Box::new(TypeKind::generic_parameter(first.clone())),
            value: Box::new(TypeKind::Array {
                item: Box::new(TypeKind::generic_parameter(second.clone())),
                len: ArrayLength::generic_parameter(length.clone()),
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
            .visit_array_length(&ArrayLength::generic_parameter(constant.clone()), 2)
            .expect("rigid generic length");
        assert_eq!(collector.finish().consts(), &[constant]);
        assert_eq!(
            TypeGenericUseCollector::new().visit_array_length(&ArrayLength::Inferred, 0),
            Err(TypeGenericUseError::InferableArrayLength)
        );
    }

    #[test]
    fn intrinsic_identity_validation_rejects_wrong_ordinal_and_const_namespace() {
        let wrong_type = TypeKind::generic_parameter(GenericTypeParameterId::new(
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
            len: ArrayLength::generic_parameter(GenericConstParameterId::new(
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
            item: Box::new(TypeKind::generic_parameter(parameter.clone())),
            len: ArrayLength::generic_parameter(constant.clone()),
        };
        let inventory = TypeGenericUseCollector::collect_many([
            (&TypeKind::generic_parameter(parameter.clone()), 1),
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

        let generic = |owner| TypeKind::generic_parameter(parameter(owner, 0));
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
            TypeKind::Parser {
                item: Box::new(generic(125)),
                error: Box::new(generic(126)),
            },
            TypeKind::Result {
                ok: Box::new(generic(114)),
                error: Box::new(generic(115)),
            },
            TypeKind::Option(Box::new(generic(116))),
            TypeKind::ThreadHandle(Box::new(generic(117))),
            TypeKind::Shared(Box::new(generic(118))),
            TypeKind::Function {
                binder: crate::types::GenericBinder::EMPTY,
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
        assert_eq!(inventory.types().len(), 27);
        assert_eq!(inventory.first_type_use(&parameter(100, 0)), Some(0));
        assert_eq!(inventory.first_type_use(&parameter(124, 0)), Some(21));
    }

    #[test]
    fn scoped_inventory_distinguishes_outer_references_from_function_local_binders() {
        use crate::{effect_row::EffectRow, effects::EffectSet, types::GenericBinder};
        let binder = GenericBinder::new(1, 1, 0);
        let incoming = GenericScope::default().with_binder(binder);
        let outer_type = incoming.bound_type(0, 0).expect("outer type");
        let outer_length = incoming.bound_const(0, 0).expect("outer length");
        let function = TypeKind::function_with_binder(
            binder,
            [TypeKind::GenericParam(outer_type.clone())],
            TypeKind::Array {
                item: Box::new(TypeKind::GenericParam(outer_type.clone())),
                len: ArrayLength::Generic(outer_length.clone()),
            },
            EffectRow::closed(EffectSet::new()),
        );
        let locals = StableGenericReferenceUseCollector::collect_in_scope(&function, &incoming)
            .expect("function owns its references");
        assert!(locals.types().is_empty() && locals.consts().is_empty());
        assert!(function.semantic_identity_digest().is_ok());

        let mixed = TypeKind::Tuple(vec![
            TypeKind::GenericParam(outer_type.clone()),
            function,
            TypeKind::Array {
                item: Box::new(TypeKind::Bool),
                len: ArrayLength::Generic(outer_length.clone()),
            },
        ]);
        let external = StableGenericReferenceUseCollector::collect_in_scope(&mixed, &incoming)
            .expect("incoming binder supplies the outer references");
        assert_eq!(external.types(), [outer_type]);
        assert_eq!(external.consts(), [outer_length]);
        assert!(StableGenericReferenceUseCollector::collect(&mixed).is_err());
    }

    #[test]
    fn active_hint_references_cannot_become_a_stable_type_inventory() {
        let opening = super::super::generics::OpenedGenericScope::new(
            crate::types::GenericBinder::new(1, 0, 0),
        )
        .expect("fresh application");
        let reference = opening.type_reference(0).expect("inference slot");
        let value = TypeKind::Vec(Box::new(TypeKind::GenericParam(reference.clone())));
        let active =
            TypeGenericReferenceUseCollector::collect(&value).expect("application-local hint");
        assert_eq!(active.types(), [reference]);
        assert!(matches!(
            StableGenericReferenceUseCollector::collect(&value),
            Err(TypeGenericUseError::Scope(
                GenericScopeError::EscapedInference {
                    kind: GenericParameterKind::Type
                }
            ))
        ));
        assert!(value.semantic_identity_digest().is_err());
    }

    #[test]
    fn nested_occurrences_share_the_same_incoming_type_and_const_keys() {
        use crate::{effect_row::EffectRow, effects::EffectSet, types::GenericBinder};
        let binder = GenericBinder::new(1, 1, 0);
        let incoming = GenericScope::default().with_binder(binder);
        let type_key = incoming.bound_type(0, 0).expect("incoming type");
        let const_key = incoming.bound_const(0, 0).expect("incoming length");
        let direct = TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(type_key.clone())),
            len: ArrayLength::Generic(const_key.clone()),
        };
        let inner = incoming.with_binder(binder);
        let nested = TypeKind::function_with_binder(
            binder,
            [TypeKind::GenericParam(
                inner.bound_type(0, 0).expect("local type is excluded"),
            )],
            TypeKind::Array {
                item: Box::new(TypeKind::GenericParam(
                    inner.bound_type(1, 0).expect("outer type under one binder"),
                )),
                len: ArrayLength::Generic(
                    inner
                        .bound_const(1, 0)
                        .expect("outer length under one binder"),
                ),
            },
            EffectRow::closed(EffectSet::new()),
        );
        let uses = StableGenericReferenceUseCollector::collect_many_in_scope(
            [(&direct, 2), (&nested, 1)],
            &incoming,
        )
        .expect("scoped occurrence inventory");
        assert_eq!(uses.types(), &[type_key.clone()]);
        assert_eq!(uses.consts(), &[const_key.clone()]);
        assert_eq!(uses.first_type_use(&type_key), Some(1));
        assert_eq!(uses.first_const_use(&const_key), Some(1));
    }
}
