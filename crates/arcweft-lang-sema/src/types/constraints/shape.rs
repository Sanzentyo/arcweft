//! Exhaustive semantic-type shape projection used by every lower phase.

use std::slice;

use arcweft_lang_syntax::reference::BorrowKind;

use super::super::{
    AcceptedNominalType, ArrayLength, CanonicalEffects, EntityType, GenericBinder,
    GenericTypeReference, IteratorStateKind, LifetimeScopeKind, MapKind, OpenNominalType,
    ProjectNominalType, TypeEffectDomain, TypeKind,
};
use super::TypeConstraintError;

pub(super) fn is_placeholder_name(name: &str) -> bool {
    name == "_"
}

impl<D: TypeEffectDomain> TypeKind<D> {
    pub(crate) fn constraint_shape(&self) -> TypeConstraintShape<'_, D> {
        match self {
            Self::Bool => TypeConstraintShape::Leaf(TypeLeaf::Bool),
            Self::I8 => TypeConstraintShape::Leaf(TypeLeaf::I8),
            Self::I16 => TypeConstraintShape::Leaf(TypeLeaf::I16),
            Self::I32 => TypeConstraintShape::Leaf(TypeLeaf::I32),
            Self::I64 => TypeConstraintShape::Leaf(TypeLeaf::I64),
            Self::I128 => TypeConstraintShape::Leaf(TypeLeaf::I128),
            Self::ISize => TypeConstraintShape::Leaf(TypeLeaf::ISize),
            Self::U8 => TypeConstraintShape::Leaf(TypeLeaf::U8),
            Self::U16 => TypeConstraintShape::Leaf(TypeLeaf::U16),
            Self::U32 => TypeConstraintShape::Leaf(TypeLeaf::U32),
            Self::U64 => TypeConstraintShape::Leaf(TypeLeaf::U64),
            Self::U128 => TypeConstraintShape::Leaf(TypeLeaf::U128),
            Self::USize => TypeConstraintShape::Leaf(TypeLeaf::USize),
            Self::F32 => TypeConstraintShape::Leaf(TypeLeaf::F32),
            Self::F64 => TypeConstraintShape::Leaf(TypeLeaf::F64),
            Self::String => TypeConstraintShape::Leaf(TypeLeaf::String),
            Self::Char => TypeConstraintShape::Leaf(TypeLeaf::Char),
            Self::Bytes => TypeConstraintShape::Leaf(TypeLeaf::Bytes),
            Self::TextCluster => TypeConstraintShape::Leaf(TypeLeaf::TextCluster),
            Self::Duration => TypeConstraintShape::Leaf(TypeLeaf::Duration),
            Self::Progress => TypeConstraintShape::Leaf(TypeLeaf::Progress),
            Self::DenseNumeric(value) => TypeConstraintShape::Leaf(TypeLeaf::DenseNumeric(value)),
            Self::StageApi(value) => TypeConstraintShape::Leaf(TypeLeaf::StageApi(value)),
            Self::LineContext => TypeConstraintShape::Leaf(TypeLeaf::LineContext),
            Self::StageActorHandle(value) => {
                TypeConstraintShape::Leaf(TypeLeaf::StageActorHandle(value))
            }
            Self::CueHandle => TypeConstraintShape::Leaf(TypeLeaf::CueHandle),
            Self::VoiceHandle => TypeConstraintShape::Leaf(TypeLeaf::VoiceHandle),
            Self::StatementIngress(value) => {
                TypeConstraintShape::Leaf(TypeLeaf::StatementIngress(value))
            }
            Self::DisplayContext => TypeConstraintShape::Leaf(TypeLeaf::DisplayContext),
            Self::DisplayError => TypeConstraintShape::Leaf(TypeLeaf::DisplayError),
            Self::DebugStatePath => TypeConstraintShape::Leaf(TypeLeaf::DebugStatePath),
            Self::ObservationFieldPath => TypeConstraintShape::Leaf(TypeLeaf::ObservationFieldPath),
            Self::Predicate => TypeConstraintShape::Leaf(TypeLeaf::Predicate),
            Self::Observation => TypeConstraintShape::Leaf(TypeLeaf::Observation),
            Self::ObservedObject => TypeConstraintShape::Leaf(TypeLeaf::ObservedObject),
            Self::AgentBBox => TypeConstraintShape::Leaf(TypeLeaf::AgentBBox),
            Self::ActionName => TypeConstraintShape::Leaf(TypeLeaf::ActionName),
            Self::ActionTarget => TypeConstraintShape::Leaf(TypeLeaf::ActionTarget),
            Self::ActionResult => TypeConstraintShape::Leaf(TypeLeaf::ActionResult),
            Self::AgentValue => TypeConstraintShape::Leaf(TypeLeaf::AgentValue),
            Self::DataFormat => TypeConstraintShape::Leaf(TypeLeaf::DataFormat),
            Self::DataValue => TypeConstraintShape::Leaf(TypeLeaf::DataValue),
            Self::DataError => TypeConstraintShape::Leaf(TypeLeaf::DataError),
            Self::DataErrorKind => TypeConstraintShape::Leaf(TypeLeaf::DataErrorKind),
            Self::DataPath => TypeConstraintShape::Leaf(TypeLeaf::DataPath),
            Self::DataPathSegment => TypeConstraintShape::Leaf(TypeLeaf::DataPathSegment),
            Self::DataMapKind => TypeConstraintShape::Leaf(TypeLeaf::DataMapKind),
            Self::AgentEntityMetadata => TypeConstraintShape::Leaf(TypeLeaf::AgentEntityMetadata),
            Self::AgentSourceAnchor => TypeConstraintShape::Leaf(TypeLeaf::AgentSourceAnchor),
            Self::AgentProjectGraphNeighborhood => {
                TypeConstraintShape::Leaf(TypeLeaf::AgentProjectGraphNeighborhood)
            }
            Self::AgentProjectGraphSymbol => {
                TypeConstraintShape::Leaf(TypeLeaf::AgentProjectGraphSymbol)
            }
            Self::AgentProjectGraphEdge => {
                TypeConstraintShape::Leaf(TypeLeaf::AgentProjectGraphEdge)
            }
            Self::CaptureTarget => TypeConstraintShape::Leaf(TypeLeaf::CaptureTarget),
            Self::CaptureRef => TypeConstraintShape::Leaf(TypeLeaf::CaptureRef),
            Self::AgentResource => TypeConstraintShape::Leaf(TypeLeaf::AgentResource),
            Self::AgentResourceBody => TypeConstraintShape::Leaf(TypeLeaf::AgentResourceBody),
            Self::RagContextPack => TypeConstraintShape::Leaf(TypeLeaf::RagContextPack),
            Self::AgentBuiltin(value) => TypeConstraintShape::Leaf(TypeLeaf::AgentBuiltin(value)),
            Self::PresentationResource(value) => {
                TypeConstraintShape::Leaf(TypeLeaf::PresentationResource(value))
            }
            Self::CharacterPatch(value) => {
                TypeConstraintShape::Leaf(TypeLeaf::CharacterPatch(value))
            }
            Self::FocusPatch => TypeConstraintShape::Leaf(TypeLeaf::FocusPatch),
            Self::CharacterDialogue(value) => {
                TypeConstraintShape::Leaf(TypeLeaf::CharacterDialogue(value))
            }
            Self::CompileTimeCallable(value) => {
                TypeConstraintShape::Leaf(TypeLeaf::CompileTimeCallable(value))
            }
            Self::CompileTimeScalar(value) => {
                TypeConstraintShape::Leaf(TypeLeaf::CompileTimeScalar(value))
            }
            Self::CompileTimeStyle(value) => {
                TypeConstraintShape::Leaf(TypeLeaf::CompileTimeStyle(value))
            }
            Self::CompileTimeEnum(value) => {
                TypeConstraintShape::Leaf(TypeLeaf::CompileTimeEnum(value))
            }
            Self::CompileTimeFx(value) => TypeConstraintShape::Leaf(TypeLeaf::CompileTimeFx(value)),
            Self::ViewValue => TypeConstraintShape::Leaf(TypeLeaf::ViewValue),
            Self::CharacterNominal(value) => {
                TypeConstraintShape::Leaf(TypeLeaf::CharacterNominal(value))
            }
            Self::Unit => TypeConstraintShape::Leaf(TypeLeaf::Unit),
            Self::Never => TypeConstraintShape::Never,
            Self::GenericParam(parameter) => TypeConstraintShape::Generic(parameter),
            Self::VariantPayload(payload) => TypeConstraintShape::Payload(payload),
            Self::Error(poison) => TypeConstraintShape::Unresolved(UnresolvedType::Poison(*poison)),
            Self::Projection {
                subject,
                trait_name,
                assoc,
            } => TypeConstraintShape::Unresolved(UnresolvedType::Projection {
                subject,
                trait_name,
                assoc,
            }),
            Self::Named(name) if is_placeholder_name(name) => {
                TypeConstraintShape::Unresolved(UnresolvedType::Placeholder(name))
            }
            Self::Named(name) => TypeConstraintShape::Leaf(TypeLeaf::Named(name)),
            Self::MetaType(child) => TypeConstraintShape::Unary {
                kind: UnaryShape::MetaType,
                child,
            },
            Self::SlotRef(child) => TypeConstraintShape::Unary {
                kind: UnaryShape::SlotRef,
                child,
            },
            Self::Handle {
                item,
                lifetime,
                state,
                must_drop,
            } => TypeConstraintShape::Unary {
                kind: UnaryShape::Handle {
                    lifetime,
                    state: *state,
                    must_drop: *must_drop,
                },
                child: item,
            },
            Self::FixedVector(vector) => TypeConstraintShape::Unary {
                kind: UnaryShape::FixedVector(vector.dimensions()),
                child: vector.component(),
            },
            Self::Range(child) => TypeConstraintShape::Unary {
                kind: UnaryShape::Range,
                child,
            },
            Self::Probe(child) => TypeConstraintShape::Unary {
                kind: UnaryShape::Probe,
                child,
            },
            Self::Vec(child) => TypeConstraintShape::Unary {
                kind: UnaryShape::Vec,
                child,
            },
            Self::DataShape(child) => TypeConstraintShape::Unary {
                kind: UnaryShape::DataShape,
                child,
            },
            Self::Slice(child) => TypeConstraintShape::Unary {
                kind: UnaryShape::Slice,
                child,
            },
            Self::Seq(child) => TypeConstraintShape::Unary {
                kind: UnaryShape::Seq,
                child,
            },
            Self::Need(child) => TypeConstraintShape::Unary {
                kind: UnaryShape::Need,
                child,
            },
            Self::ThreadHandle(child) => TypeConstraintShape::Unary {
                kind: UnaryShape::ThreadHandle,
                child,
            },
            Self::Shared(child) => TypeConstraintShape::Unary {
                kind: UnaryShape::Shared,
                child,
            },
            Self::DialogueLine(child) => TypeConstraintShape::Unary {
                kind: UnaryShape::DialogueLine,
                child,
            },
            Self::Option(child) => TypeConstraintShape::Unary {
                kind: UnaryShape::Option,
                child,
            },
            Self::IteratorState { family, item } => TypeConstraintShape::Iterator { family, item },
            Self::Array { item, len } => TypeConstraintShape::Array { item, len },
            Self::Ref(entity) => TypeConstraintShape::Ref(entity),
            Self::Map { kind, key, value } => TypeConstraintShape::Map { kind, key, value },
            Self::BorrowRef {
                kind,
                lifetime,
                inner,
            } => TypeConstraintShape::Borrow {
                kind,
                lifetime,
                inner,
            },
            Self::Traversal { source, mapping } => TypeConstraintShape::Pair {
                kind: PairShape::Traversal,
                first: source,
                second: mapping,
            },
            Self::Stream { item, error } => TypeConstraintShape::Pair {
                kind: PairShape::Stream,
                first: item,
                second: error,
            },
            Self::Parser { item, error } => TypeConstraintShape::Pair {
                kind: PairShape::Parser,
                first: item,
                second: error,
            },
            Self::Result { ok, error } => TypeConstraintShape::Pair {
                kind: PairShape::Result,
                first: ok,
                second: error,
            },
            Self::Function {
                binder,
                predicate,
                params,
                return_type,
                effects,
            } => TypeConstraintShape::Function {
                binder,
                predicate,
                params,
                result: return_type,
                effects,
            },
            Self::ProjectNominal(nominal) => TypeConstraintShape::Nominal {
                nominal: NominalShape::Project(nominal),
                arguments: nominal.arguments(),
            },
            Self::AcceptedNominal(nominal) => TypeConstraintShape::Nominal {
                nominal: NominalShape::Accepted(nominal),
                arguments: nominal.arguments(),
            },
            Self::OpenNominal(nominal) => TypeConstraintShape::Nominal {
                nominal: NominalShape::Open(nominal),
                arguments: nominal.arguments(),
            },
            Self::Tuple(items) => TypeConstraintShape::Tuple(items),
            Self::Choice(items) => TypeConstraintShape::Choice(items),
        }
    }
}

#[derive(Clone)]
pub(crate) enum TypeConstraintShape<'a, D: TypeEffectDomain = CanonicalEffects> {
    Payload(&'a super::super::VariantPayloadType<D>),
    Leaf(TypeLeaf<'a>),
    Never,
    Generic(&'a GenericTypeReference),
    Unresolved(UnresolvedType<'a, D>),
    Unary {
        kind: UnaryShape<'a>,
        child: &'a TypeKind<D>,
    },
    Iterator {
        family: &'a IteratorStateKind,
        item: &'a TypeKind<D>,
    },
    Array {
        item: &'a TypeKind<D>,
        len: &'a ArrayLength,
    },
    Ref(&'a EntityType<D>),
    Map {
        kind: &'a MapKind,
        key: &'a TypeKind<D>,
        value: &'a TypeKind<D>,
    },
    Borrow {
        kind: &'a BorrowKind,
        lifetime: &'a Option<LifetimeScopeKind>,
        inner: &'a TypeKind<D>,
    },
    Pair {
        kind: PairShape,
        first: &'a TypeKind<D>,
        second: &'a TypeKind<D>,
    },
    Function {
        binder: &'a GenericBinder,
        predicate: &'a D::Predicate,
        params: &'a [TypeKind<D>],
        result: &'a TypeKind<D>,
        effects: &'a D::Row,
    },
    Nominal {
        nominal: NominalShape<'a, D>,
        arguments: &'a [TypeKind<D>],
    },
    Tuple(&'a [TypeKind<D>]),
    Choice(&'a [TypeKind<D>]),
}

impl<D: TypeEffectDomain> Copy for TypeConstraintShape<'_, D> {}

impl<'a, D: TypeEffectDomain> TypeConstraintShape<'a, D> {
    pub(crate) const fn binder(self) -> GenericBinder {
        match self {
            Self::Function { binder, .. } => *binder,
            _ => GenericBinder::EMPTY,
        }
    }

    pub(crate) fn children(self) -> TypeConstraintChildren<'a, D> {
        match self {
            Self::Payload(payload) => TypeConstraintChildren::Payload(payload.children()),
            Self::Leaf(_) | Self::Never | Self::Generic(_) => TypeConstraintChildren::None,
            Self::Unresolved(unresolved) => TypeConstraintChildren::One(unresolved.child()),
            Self::Unary { child, .. }
            | Self::Iterator { item: child, .. }
            | Self::Array { item: child, .. }
            | Self::Borrow { inner: child, .. } => TypeConstraintChildren::One(Some(child)),
            Self::Ref(entity) => TypeConstraintChildren::One(entity.value()),
            Self::Map { key, value, .. }
            | Self::Pair {
                first: key,
                second: value,
                ..
            } => TypeConstraintChildren::Two {
                first: Some(key),
                second: Some(value),
            },
            Self::Function { params, result, .. } => TypeConstraintChildren::Function {
                params: params.iter(),
                result: Some(result),
            },
            Self::Nominal { arguments, .. } | Self::Tuple(arguments) | Self::Choice(arguments) => {
                TypeConstraintChildren::Slice(arguments.iter())
            }
        }
    }

    pub(crate) fn same_header(self, other: Self) -> bool {
        match (self, other) {
            (Self::Payload(left), Self::Payload(right)) => left.has_same_header(right),
            (Self::Leaf(left), Self::Leaf(right)) => left == right,
            (Self::Never, Self::Never) | (Self::Unresolved(_), Self::Unresolved(_)) => true,
            (Self::Generic(left), Self::Generic(right)) => left == right,
            (Self::Unary { kind: left, .. }, Self::Unary { kind: right, .. }) => left == right,
            (Self::Iterator { family: left, .. }, Self::Iterator { family: right, .. }) => {
                left == right
            }
            (Self::Array { len: left, .. }, Self::Array { len: right, .. }) => left == right,
            (Self::Ref(left), Self::Ref(right)) => {
                left.kind() == right.kind() && left.value().is_some() == right.value().is_some()
            }
            (Self::Map { kind: left, .. }, Self::Map { kind: right, .. }) => left == right,
            (
                Self::Borrow {
                    kind: left_kind,
                    lifetime: left_lifetime,
                    ..
                },
                Self::Borrow {
                    kind: right_kind,
                    lifetime: right_lifetime,
                    ..
                },
            ) => left_kind == right_kind && left_lifetime == right_lifetime,
            (Self::Pair { kind: left, .. }, Self::Pair { kind: right, .. }) => left == right,
            (
                Self::Function {
                    binder: left_binder,
                    params: left,
                    ..
                },
                Self::Function {
                    binder: right_binder,
                    params: right,
                    ..
                },
            ) => left_binder == right_binder && left.len() == right.len(),
            (
                Self::Nominal {
                    nominal: left,
                    arguments: left_arguments,
                },
                Self::Nominal {
                    nominal: right,
                    arguments: right_arguments,
                },
            ) => left.same_owner(right) && left_arguments.len() == right_arguments.len(),
            (Self::Tuple(left), Self::Tuple(right)) | (Self::Choice(left), Self::Choice(right)) => {
                left.len() == right.len()
            }
            _ => false,
        }
    }

    pub(crate) fn rebuild(
        self,
        children: Vec<TypeKind<D>>,
    ) -> Result<TypeKind<D>, TypeConstraintError> {
        self.rebuild_with(
            children,
            &mut (),
            |(), length| Ok(length.clone()),
            |(), row| Ok(row.clone()),
            |(), predicate| Ok(predicate.clone()),
        )
    }

    /// Projects owned scalar children before constructing the result. Type
    /// and scalar projection share this exhaustive structural constructor.
    pub(crate) fn rebuild_with<C, E, T: TypeEffectDomain>(
        self,
        children: Vec<TypeKind<T>>,
        context: &mut C,
        length: impl FnOnce(&mut C, &ArrayLength) -> Result<ArrayLength, E>,
        effects_row: impl FnOnce(&mut C, &D::Row) -> Result<T::Row, E>,
        effect_predicate: impl FnOnce(&mut C, &D::Predicate) -> Result<T::Predicate, E>,
    ) -> Result<TypeKind<T>, E> {
        let mut children = children.into_iter();
        let rebuilt = match self {
            Self::Payload(payload) => {
                TypeKind::VariantPayload(Box::new(payload.map(|_| next_child(&mut children))))
            }
            Self::Leaf(ty) => ty.rebuild(),
            Self::Never => TypeKind::Never,
            Self::Generic(parameter) => TypeKind::GenericParam(parameter.clone()),
            Self::Unresolved(unresolved) => unresolved.rebuild(&mut children),
            Self::Unary { kind, .. } => kind.rebuild(next_child(&mut children)),
            Self::Iterator { family, .. } => TypeKind::IteratorState {
                family: *family,
                item: Box::new(next_child(&mut children)),
            },
            Self::Array { len, .. } => TypeKind::Array {
                item: Box::new(next_child(&mut children)),
                len: length(context, len)?,
            },
            Self::Ref(entity) => TypeKind::Ref(EntityType::new(
                entity.kind().clone(),
                entity.value().map(|_| next_child(&mut children)),
            )),
            Self::Map { kind, .. } => TypeKind::Map {
                kind: *kind,
                key: Box::new(next_child(&mut children)),
                value: Box::new(next_child(&mut children)),
            },
            Self::Borrow { kind, lifetime, .. } => TypeKind::BorrowRef {
                kind: *kind,
                lifetime: lifetime.clone(),
                inner: Box::new(next_child(&mut children)),
            },
            Self::Pair { kind, .. } => {
                kind.rebuild(next_child(&mut children), next_child(&mut children))
            }
            Self::Function {
                binder,
                predicate,
                effects,
                ..
            } => {
                let mut children = children.collect::<Vec<_>>();
                let result = children
                    .pop()
                    .expect("function shape retains one result child");
                TypeKind::Function {
                    binder: *binder,
                    predicate: effect_predicate(context, predicate)?,
                    params: children,
                    return_type: Box::new(result),
                    effects: effects_row(context, effects)?,
                }
            }
            Self::Nominal { nominal, .. } => nominal.rebuild(children.collect()),
            Self::Tuple(_) => TypeKind::Tuple(children.collect()),
            Self::Choice(_) => TypeKind::Choice(children.collect()),
        };
        Ok(rebuilt)
    }
}

fn next_child<D: TypeEffectDomain>(
    children: &mut impl Iterator<Item = TypeKind<D>>,
) -> TypeKind<D> {
    children
        .next()
        .expect("constraint shape child inventory and rebuild agree")
}

pub(crate) enum TypeConstraintChildren<'a, D: TypeEffectDomain = CanonicalEffects> {
    Payload(super::super::VariantPayloadTypeChildren<'a, D>),
    None,
    One(Option<&'a TypeKind<D>>),
    Two {
        first: Option<&'a TypeKind<D>>,
        second: Option<&'a TypeKind<D>>,
    },
    Slice(slice::Iter<'a, TypeKind<D>>),
    Function {
        params: slice::Iter<'a, TypeKind<D>>,
        result: Option<&'a TypeKind<D>>,
    },
}

impl<'a, D: TypeEffectDomain> Iterator for TypeConstraintChildren<'a, D> {
    type Item = &'a TypeKind<D>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Payload(children) => children.next(),
            Self::None => None,
            Self::One(child) => child.take(),
            Self::Two { first, second } => first.take().or_else(|| second.take()),
            Self::Slice(children) => children.next(),
            Self::Function { params, result } => params.next().or_else(|| result.take()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UnaryShape<'a> {
    FixedVector(crate::callable::VectorDimensions),
    Range,
    Probe,
    Vec,
    DataShape,
    Slice,
    Seq,
    Need,
    Option,
    ThreadHandle,
    Shared,
    DialogueLine,
    MetaType,
    SlotRef,
    Handle {
        lifetime: &'a LifetimeScopeKind,
        state: super::super::HandleState,
        must_drop: bool,
    },
}

impl UnaryShape<'_> {
    fn rebuild<D: TypeEffectDomain>(self, child: TypeKind<D>) -> TypeKind<D> {
        let child = Box::new(child);
        match self {
            Self::FixedVector(dimensions) => {
                TypeKind::FixedVector(super::super::FixedVectorType::new(dimensions, *child))
            }
            Self::Range => TypeKind::Range(child),
            Self::Probe => TypeKind::Probe(child),
            Self::Vec => TypeKind::Vec(child),
            Self::DataShape => TypeKind::DataShape(child),
            Self::Slice => TypeKind::Slice(child),
            Self::Seq => TypeKind::Seq(child),
            Self::Need => TypeKind::Need(child),
            Self::Option => TypeKind::Option(child),
            Self::ThreadHandle => TypeKind::ThreadHandle(child),
            Self::Shared => TypeKind::Shared(child),
            Self::DialogueLine => TypeKind::DialogueLine(child),
            Self::MetaType => TypeKind::MetaType(child),
            Self::SlotRef => TypeKind::SlotRef(child),
            Self::Handle {
                lifetime,
                state,
                must_drop,
            } => TypeKind::Handle {
                item: child,
                lifetime: lifetime.clone(),
                state,
                must_drop,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PairShape {
    Traversal,
    Stream,
    Parser,
    Result,
}

impl PairShape {
    fn rebuild<D: TypeEffectDomain>(self, first: TypeKind<D>, second: TypeKind<D>) -> TypeKind<D> {
        match self {
            Self::Traversal => TypeKind::Traversal {
                source: Box::new(first),
                mapping: Box::new(second),
            },
            Self::Stream => TypeKind::Stream {
                item: Box::new(first),
                error: Box::new(second),
            },
            Self::Parser => TypeKind::Parser {
                item: Box::new(first),
                error: Box::new(second),
            },
            Self::Result => TypeKind::Result {
                ok: Box::new(first),
                error: Box::new(second),
            },
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum NominalShape<'a, D: TypeEffectDomain = CanonicalEffects> {
    Project(&'a ProjectNominalType<D>),
    Accepted(&'a AcceptedNominalType<D>),
    Open(&'a OpenNominalType<D>),
}

impl<D: TypeEffectDomain> NominalShape<'_, D> {
    pub(crate) fn same_owner(self, other: Self) -> bool {
        match (self, other) {
            (Self::Project(left), Self::Project(right)) => {
                left.declaration() == right.declaration()
            }
            (Self::Accepted(left), Self::Accepted(right)) => {
                left.declaration() == right.declaration()
            }
            (Self::Open(left), Self::Open(right)) => {
                left.rule() == right.rule() && left.path() == right.path()
            }
            _ => false,
        }
    }

    pub(crate) fn rebuild<T: TypeEffectDomain>(self, arguments: Vec<TypeKind<T>>) -> TypeKind<T> {
        match self {
            Self::Project(nominal) => TypeKind::ProjectNominal(ProjectNominalType::new(
                nominal.declaration().clone(),
                arguments,
            )),
            Self::Accepted(nominal) => TypeKind::AcceptedNominal(AcceptedNominalType::new(
                nominal.declaration().clone(),
                arguments,
            )),
            Self::Open(nominal) => TypeKind::OpenNominal(OpenNominalType::new(
                nominal.rule().clone(),
                nominal.path().clone(),
                arguments,
            )),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum TypeLeaf<'a> {
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    F32,
    F64,
    String,
    Char,
    Bytes,
    TextCluster,
    Duration,
    Progress,
    DenseNumeric(&'a arcweft_core::math::RuntimeDenseNumericType),
    StageApi(&'a super::super::CharacterId),
    LineContext,
    StageActorHandle(&'a super::super::StageActorHandleType),
    CueHandle,
    VoiceHandle,
    StatementIngress(&'a super::super::StandardStatementIngressTypeId),
    DisplayContext,
    DisplayError,
    DebugStatePath,
    ObservationFieldPath,
    Predicate,
    Observation,
    ObservedObject,
    AgentBBox,
    ActionName,
    ActionTarget,
    ActionResult,
    AgentValue,
    DataFormat,
    DataValue,
    DataError,
    DataErrorKind,
    DataPath,
    DataPathSegment,
    DataMapKind,
    AgentEntityMetadata,
    AgentSourceAnchor,
    AgentProjectGraphNeighborhood,
    AgentProjectGraphSymbol,
    AgentProjectGraphEdge,
    CaptureTarget,
    CaptureRef,
    AgentResource,
    AgentResourceBody,
    RagContextPack,
    AgentBuiltin(&'a super::super::AgentBuiltinType),
    PresentationResource(&'a arcweft_interaction_model::presentation::PresentationResourceKind),
    CharacterPatch(&'a super::super::EntityKind),
    FocusPatch,
    CharacterDialogue(&'a super::super::CharacterDialogueType),
    CompileTimeCallable(&'a super::super::CompileTimeCallableType),
    CompileTimeScalar(&'a super::super::CompileTimeScalarType),
    CompileTimeStyle(&'a super::super::CompileTimeStyleType),
    CompileTimeEnum(&'a super::super::CompileTimeEnumType),
    CompileTimeFx(&'a super::super::CompileTimeFxType),
    ViewValue,
    CharacterNominal(&'a super::super::CharacterNominalType),
    Unit,
    Named(&'a str),
}

impl TypeLeaf<'_> {
    fn rebuild<D: TypeEffectDomain>(self) -> TypeKind<D> {
        match self {
            Self::Bool => TypeKind::Bool,
            Self::I8 => TypeKind::I8,
            Self::I16 => TypeKind::I16,
            Self::I32 => TypeKind::I32,
            Self::I64 => TypeKind::I64,
            Self::I128 => TypeKind::I128,
            Self::ISize => TypeKind::ISize,
            Self::U8 => TypeKind::U8,
            Self::U16 => TypeKind::U16,
            Self::U32 => TypeKind::U32,
            Self::U64 => TypeKind::U64,
            Self::U128 => TypeKind::U128,
            Self::USize => TypeKind::USize,
            Self::F32 => TypeKind::F32,
            Self::F64 => TypeKind::F64,
            Self::String => TypeKind::String,
            Self::Char => TypeKind::Char,
            Self::Bytes => TypeKind::Bytes,
            Self::TextCluster => TypeKind::TextCluster,
            Self::Duration => TypeKind::Duration,
            Self::Progress => TypeKind::Progress,
            Self::DenseNumeric(value) => TypeKind::DenseNumeric(*value),
            Self::StageApi(value) => TypeKind::StageApi((*value).clone()),
            Self::LineContext => TypeKind::LineContext,
            Self::StageActorHandle(value) => TypeKind::StageActorHandle((*value).clone()),
            Self::CueHandle => TypeKind::CueHandle,
            Self::VoiceHandle => TypeKind::VoiceHandle,
            Self::StatementIngress(value) => TypeKind::StatementIngress(*value),
            Self::DisplayContext => TypeKind::DisplayContext,
            Self::DisplayError => TypeKind::DisplayError,
            Self::DebugStatePath => TypeKind::DebugStatePath,
            Self::ObservationFieldPath => TypeKind::ObservationFieldPath,
            Self::Predicate => TypeKind::Predicate,
            Self::Observation => TypeKind::Observation,
            Self::ObservedObject => TypeKind::ObservedObject,
            Self::AgentBBox => TypeKind::AgentBBox,
            Self::ActionName => TypeKind::ActionName,
            Self::ActionTarget => TypeKind::ActionTarget,
            Self::ActionResult => TypeKind::ActionResult,
            Self::AgentValue => TypeKind::AgentValue,
            Self::DataFormat => TypeKind::DataFormat,
            Self::DataValue => TypeKind::DataValue,
            Self::DataError => TypeKind::DataError,
            Self::DataErrorKind => TypeKind::DataErrorKind,
            Self::DataPath => TypeKind::DataPath,
            Self::DataPathSegment => TypeKind::DataPathSegment,
            Self::DataMapKind => TypeKind::DataMapKind,
            Self::AgentEntityMetadata => TypeKind::AgentEntityMetadata,
            Self::AgentSourceAnchor => TypeKind::AgentSourceAnchor,
            Self::AgentProjectGraphNeighborhood => TypeKind::AgentProjectGraphNeighborhood,
            Self::AgentProjectGraphSymbol => TypeKind::AgentProjectGraphSymbol,
            Self::AgentProjectGraphEdge => TypeKind::AgentProjectGraphEdge,
            Self::CaptureTarget => TypeKind::CaptureTarget,
            Self::CaptureRef => TypeKind::CaptureRef,
            Self::AgentResource => TypeKind::AgentResource,
            Self::AgentResourceBody => TypeKind::AgentResourceBody,
            Self::RagContextPack => TypeKind::RagContextPack,
            Self::AgentBuiltin(value) => TypeKind::AgentBuiltin(*value),
            Self::PresentationResource(value) => TypeKind::PresentationResource(*value),
            Self::CharacterPatch(value) => TypeKind::CharacterPatch((*value).clone()),
            Self::FocusPatch => TypeKind::FocusPatch,
            Self::CharacterDialogue(value) => TypeKind::CharacterDialogue((*value).clone()),
            Self::CompileTimeCallable(value) => TypeKind::CompileTimeCallable((*value).clone()),
            Self::CompileTimeScalar(value) => TypeKind::CompileTimeScalar((*value).clone()),
            Self::CompileTimeStyle(value) => TypeKind::CompileTimeStyle(*value),
            Self::CompileTimeEnum(value) => TypeKind::CompileTimeEnum(*value),
            Self::CompileTimeFx(value) => TypeKind::CompileTimeFx((*value).clone()),
            Self::ViewValue => TypeKind::ViewValue,
            Self::CharacterNominal(value) => TypeKind::CharacterNominal((*value).clone()),
            Self::Unit => TypeKind::Unit,
            Self::Named(name) => TypeKind::Named(name.to_owned()),
        }
    }
}

/// Recovery evidence retained by structural folds, rejected by constraint policy.
#[derive(Clone, Copy)]
pub(crate) enum UnresolvedType<'a, D: TypeEffectDomain> {
    Poison(super::super::TypePoisonId),
    Placeholder(&'a str),
    Projection {
        subject: &'a TypeKind<D>,
        trait_name: &'a Option<String>,
        assoc: &'a str,
    },
}

impl<'a, D: TypeEffectDomain> UnresolvedType<'a, D> {
    fn child(self) -> Option<&'a TypeKind<D>> {
        match self {
            Self::Projection { subject, .. } => Some(subject),
            Self::Poison(_) | Self::Placeholder(_) => None,
        }
    }

    fn rebuild<T: TypeEffectDomain>(
        self,
        children: &mut impl Iterator<Item = TypeKind<T>>,
    ) -> TypeKind<T> {
        match self {
            Self::Poison(poison) => TypeKind::Error(poison),
            Self::Placeholder(name) => TypeKind::Named(name.to_owned()),
            Self::Projection {
                trait_name, assoc, ..
            } => TypeKind::Projection {
                subject: Box::new(next_child(children)),
                trait_name: trait_name.clone(),
                assoc: assoc.to_owned(),
            },
        }
    }
}
