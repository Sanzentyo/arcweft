//! Exhaustive semantic-type shape projection used by every lower phase.

use std::slice;

use arcweft_lang_syntax::reference::BorrowKind;

use crate::effect_row::EffectRow;

use super::super::{
    AcceptedNominalType, ArrayLength, EntityType, GenericTypeParameterId, IteratorStateKind,
    LifetimeScopeKind, MapKind, OpenNominalType, ProjectNominalType, TypeKind,
};
use super::{TypeConstraintError, TypeConstraintRejection};

pub(super) fn is_placeholder_name(name: &str) -> bool {
    name == "_"
}

impl TypeKind {
    pub(crate) fn constraint_shape(&self) -> TypeConstraintShape<'_> {
        match self {
            ty @ (Self::Bool
            | Self::I8
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
            | Self::AgentResourceBody
            | Self::RagContextPack
            | Self::AgentBuiltin(_)
            | Self::Handle { .. }
            | Self::CharacterPatch(_)
            | Self::FocusPatch
            | Self::CharacterDialogue(_)
            | Self::ViewValue
            | Self::CharacterNominal(_)
            | Self::VariantPayload(_)
            | Self::Unit) => TypeConstraintShape::Leaf(ty),
            Self::Never => TypeConstraintShape::Never,
            Self::GenericParam(parameter) => TypeConstraintShape::Generic(parameter),
            Self::Error(_) | Self::Projection { .. } => TypeConstraintShape::Unresolved,
            Self::Named(name) if is_placeholder_name(name) => TypeConstraintShape::Unresolved,
            ty @ Self::Named(_) => TypeConstraintShape::Leaf(ty),
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
                params,
                return_type,
                effects,
            } => TypeConstraintShape::Function {
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

#[derive(Clone, Copy)]
pub(crate) enum TypeConstraintShape<'a> {
    Leaf(&'a TypeKind),
    Never,
    Generic(&'a GenericTypeParameterId),
    Unresolved,
    Unary {
        kind: UnaryShape,
        child: &'a TypeKind,
    },
    Iterator {
        family: &'a IteratorStateKind,
        item: &'a TypeKind,
    },
    Array {
        item: &'a TypeKind,
        len: &'a ArrayLength,
    },
    Ref(&'a EntityType),
    Map {
        kind: &'a MapKind,
        key: &'a TypeKind,
        value: &'a TypeKind,
    },
    Borrow {
        kind: &'a BorrowKind,
        lifetime: &'a Option<LifetimeScopeKind>,
        inner: &'a TypeKind,
    },
    Pair {
        kind: PairShape,
        first: &'a TypeKind,
        second: &'a TypeKind,
    },
    Function {
        params: &'a [TypeKind],
        result: &'a TypeKind,
        effects: &'a EffectRow,
    },
    Nominal {
        nominal: NominalShape<'a>,
        arguments: &'a [TypeKind],
    },
    Tuple(&'a [TypeKind]),
    Choice(&'a [TypeKind]),
}

impl<'a> TypeConstraintShape<'a> {
    pub(crate) fn children(self) -> TypeConstraintChildren<'a> {
        match self {
            Self::Leaf(_) | Self::Never | Self::Generic(_) | Self::Unresolved => {
                TypeConstraintChildren::None
            }
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
            (Self::Leaf(left), Self::Leaf(right)) => left == right,
            (Self::Never, Self::Never) | (Self::Unresolved, Self::Unresolved) => true,
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
            (Self::Function { params: left, .. }, Self::Function { params: right, .. }) => {
                left.len() == right.len()
            }
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

    pub(crate) fn rebuild(self, children: Vec<TypeKind>) -> Result<TypeKind, TypeConstraintError> {
        let mut children = children.into_iter();
        let rebuilt = match self {
            Self::Leaf(ty) => ty.clone(),
            Self::Never => TypeKind::Never,
            Self::Generic(parameter) => TypeKind::GenericParam(parameter.clone()),
            Self::Unresolved => return Err(TypeConstraintRejection::UnresolvedType.into()),
            Self::Unary { kind, .. } => kind.rebuild(next_child(&mut children)),
            Self::Iterator { family, .. } => TypeKind::IteratorState {
                family: *family,
                item: Box::new(next_child(&mut children)),
            },
            Self::Array { len, .. } => TypeKind::Array {
                item: Box::new(next_child(&mut children)),
                len: len.clone(),
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
            Self::Function { effects, .. } => {
                let mut children = children.collect::<Vec<_>>();
                let result = children
                    .pop()
                    .expect("function shape retains one result child");
                TypeKind::Function {
                    params: children,
                    return_type: Box::new(result),
                    effects: effects.clone(),
                }
            }
            Self::Nominal { nominal, .. } => nominal.rebuild(children.collect()),
            Self::Tuple(_) => TypeKind::Tuple(children.collect()),
            Self::Choice(_) => TypeKind::Choice(children.collect()),
        };
        Ok(rebuilt)
    }
}

fn next_child(children: &mut impl Iterator<Item = TypeKind>) -> TypeKind {
    children
        .next()
        .expect("constraint shape child inventory and rebuild agree")
}

pub(crate) enum TypeConstraintChildren<'a> {
    None,
    One(Option<&'a TypeKind>),
    Two {
        first: Option<&'a TypeKind>,
        second: Option<&'a TypeKind>,
    },
    Slice(slice::Iter<'a, TypeKind>),
    Function {
        params: slice::Iter<'a, TypeKind>,
        result: Option<&'a TypeKind>,
    },
}

impl<'a> Iterator for TypeConstraintChildren<'a> {
    type Item = &'a TypeKind;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::None => None,
            Self::One(child) => child.take(),
            Self::Two { first, second } => first.take().or_else(|| second.take()),
            Self::Slice(children) => children.next(),
            Self::Function { params, result } => params.next().or_else(|| result.take()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UnaryShape {
    Range,
    Probe,
    Vec,
    Slice,
    Seq,
    Need,
    Option,
    ThreadHandle,
    Shared,
    DialogueLine,
}

impl UnaryShape {
    fn rebuild(self, child: TypeKind) -> TypeKind {
        let child = Box::new(child);
        match self {
            Self::Range => TypeKind::Range(child),
            Self::Probe => TypeKind::Probe(child),
            Self::Vec => TypeKind::Vec(child),
            Self::Slice => TypeKind::Slice(child),
            Self::Seq => TypeKind::Seq(child),
            Self::Need => TypeKind::Need(child),
            Self::Option => TypeKind::Option(child),
            Self::ThreadHandle => TypeKind::ThreadHandle(child),
            Self::Shared => TypeKind::Shared(child),
            Self::DialogueLine => TypeKind::DialogueLine(child),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PairShape {
    Stream,
    Parser,
    Result,
}

impl PairShape {
    fn rebuild(self, first: TypeKind, second: TypeKind) -> TypeKind {
        match self {
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
pub(crate) enum NominalShape<'a> {
    Project(&'a ProjectNominalType),
    Accepted(&'a AcceptedNominalType),
    Open(&'a OpenNominalType),
}

impl NominalShape<'_> {
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

    pub(crate) fn rebuild(self, arguments: Vec<TypeKind>) -> TypeKind {
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
