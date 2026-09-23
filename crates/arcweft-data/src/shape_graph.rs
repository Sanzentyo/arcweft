use std::{
    any::TypeId,
    borrow::Cow,
    collections::{BTreeSet, HashMap},
};

use crate::{DataError, Result, TypeShape};

/// Stable-within-a-graph identity for a reflected shape node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ShapeId(usize);

impl ShapeId {
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Access to the finite set of nodes backing shape references.
pub trait ShapeAccess {
    /// Returns a borrowed node or a shallow owned projection whose children use `Ref` ids.
    fn get_shape(&self, id: ShapeId) -> Option<Cow<'_, TypeShape>>;

    /// Queries the decoding context without executing a default producer.
    /// A plain shape graph carries no authority to generate missing values.
    fn field_defaults(&self) -> Option<&dyn crate::FieldDefaultProvider> {
        None
    }
}

/// Shape access adapter for inline trees that contain no identity references.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmptyShapeAccess;

impl ShapeAccess for EmptyShapeAccess {
    fn get_shape(&self, _id: ShapeId) -> Option<Cow<'_, TypeShape>> {
        None
    }
}

/// A borrowed inline shape or an identity that resolves through a shape graph.
#[derive(Clone, Copy, Debug)]
pub enum ShapeRef<'a> {
    Inline(&'a TypeShape),
    Id(ShapeId),
}

impl<'a> ShapeRef<'a> {
    /// Existing graph coordinate carried by this reference, without resolving
    /// or deriving an identity for an inline node.
    #[must_use]
    pub const fn referenced_id(self) -> Option<ShapeId> {
        match self {
            Self::Id(id) => Some(id),
            Self::Inline(TypeShape::Ref(id)) => Some(*id),
            Self::Inline(_) => None,
        }
    }

    #[must_use]
    pub const fn inline(shape: &'a TypeShape) -> Self {
        Self::Inline(shape)
    }

    #[must_use]
    pub const fn id(id: ShapeId) -> Self {
        Self::Id(id)
    }

    pub fn resolve<'b, A: ShapeAccess + ?Sized>(self, access: &'b A) -> Result<Cow<'b, TypeShape>>
    where
        'a: 'b,
    {
        let mut shape = match self {
            Self::Inline(shape) => Ok(Cow::Borrowed(shape)),
            Self::Id(id) => access.get_shape(id).ok_or_else(|| {
                DataError::unsupported(format!("shape id {} is not present", id.index()))
            }),
        }?;
        let mut followed = BTreeSet::new();
        loop {
            let TypeShape::Ref(id) = shape.as_ref() else {
                return Ok(shape);
            };
            let id = *id;
            if !followed.insert(id) {
                return Err(DataError::new(
                    crate::DataErrorKind::InvalidEncoding,
                    format!(
                        "shape reference cycle reaches id {} without a node",
                        id.index()
                    ),
                ));
            }
            shape = access.get_shape(id).ok_or_else(|| {
                DataError::unsupported(format!("shape id {} is not present", id.index()))
            })?;
        }
    }
}

/// Finite shape-node storage. Recursive shapes use [`TypeShape::Ref`] edges.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ShapeGraph {
    nodes: Vec<TypeShape>,
}

impl ShapeGraph {
    #[must_use]
    pub const fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn get(&self, id: ShapeId) -> Option<&TypeShape> {
        self.nodes.get(id.index())
    }

    #[must_use]
    pub const fn reference(&self, id: ShapeId) -> ShapeRef<'_> {
        ShapeRef::Id(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (ShapeId, &TypeShape)> {
        self.nodes
            .iter()
            .enumerate()
            .map(|(index, shape)| (ShapeId(index), shape))
    }

    pub fn from_root(shape: TypeShape) -> Result<(Self, ShapeId)> {
        let mut builder = ShapeGraphBuilder::new();
        let root = builder.reserve();
        builder.define(root, shape)?;
        Ok((builder.finish()?, root))
    }
}

impl ShapeAccess for ShapeGraph {
    fn get_shape(&self, id: ShapeId) -> Option<Cow<'_, TypeShape>> {
        self.get(id).map(Cow::Borrowed)
    }
}

/// Mutable construction phase that permits forward and recursive references.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ShapeGraphBuilder {
    nodes: Vec<Option<TypeShape>>,
    typed_nodes: HashMap<TypeId, ShapeId>,
}

impl ShapeGraphBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            typed_nodes: HashMap::new(),
        }
    }

    /// Reserves a node identity for self or forward references.
    pub fn reserve(&mut self) -> ShapeId {
        let id = ShapeId(self.nodes.len());
        self.nodes.push(None);
        id
    }

    /// Reserves one node for the exact concrete Rust type within this graph.
    ///
    /// The returned `bool` is true only for the first reservation. Later
    /// recursive or repeated registrations receive the same id while the node
    /// is still being defined or after it has been defined. `TypeId` is used
    /// only for this build-local join; graph ids remain deterministic reserve
    /// order values.
    pub fn reserve_type<T: 'static>(&mut self) -> (ShapeId, bool) {
        let type_id = TypeId::of::<T>();
        if let Some(id) = self.typed_nodes.get(&type_id) {
            return (*id, false);
        }
        let id = self.reserve();
        self.typed_nodes.insert(type_id, id);
        (id, true)
    }

    pub fn define(&mut self, id: ShapeId, shape: TypeShape) -> Result<()> {
        let slot = self.nodes.get_mut(id.index()).ok_or_else(|| {
            DataError::new(
                crate::DataErrorKind::InvalidEncoding,
                format!("shape id {} was not reserved", id.index()),
            )
        })?;
        if slot.is_some() {
            return Err(DataError::new(
                crate::DataErrorKind::DuplicateField,
                format!("shape id {} was defined more than once", id.index()),
            ));
        }
        *slot = Some(shape);
        Ok(())
    }

    pub fn finish(self) -> Result<ShapeGraph> {
        let nodes = self
            .nodes
            .into_iter()
            .enumerate()
            .map(|(index, shape)| {
                shape.ok_or_else(|| {
                    DataError::new(
                        crate::DataErrorKind::InvalidEncoding,
                        format!("shape id {index} was reserved but not defined"),
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;
        for (owner, shape) in nodes.iter().enumerate() {
            if matches!(shape, TypeShape::Ref(_)) {
                return Err(DataError::new(
                    crate::DataErrorKind::InvalidEncoding,
                    format!("shape node {owner} cannot be an unguarded reference"),
                ));
            }
            for target in referenced_ids(shape) {
                if target.index() >= nodes.len() {
                    return Err(DataError::new(
                        crate::DataErrorKind::InvalidEncoding,
                        format!(
                            "shape id {} references missing shape id {}",
                            owner,
                            target.index()
                        ),
                    ));
                }
            }
        }
        Ok(ShapeGraph { nodes })
    }
}

fn referenced_ids(shape: &TypeShape) -> Vec<ShapeId> {
    let mut ids = Vec::new();
    collect_referenced_ids(shape, &mut ids);
    ids
}

fn collect_referenced_ids(shape: &TypeShape, ids: &mut Vec<ShapeId>) {
    match shape {
        TypeShape::Option(inner) | TypeShape::Seq(inner) => collect_referenced_ids(inner, ids),
        TypeShape::Tuple(items) => items
            .iter()
            .for_each(|item| collect_referenced_ids(item, ids)),
        TypeShape::Map { key, value, .. } => {
            collect_referenced_ids(key, ids);
            collect_referenced_ids(value, ids);
        }
        TypeShape::Record { fields, .. } => fields
            .iter()
            .for_each(|field| collect_referenced_ids(&field.shape, ids)),
        TypeShape::Enum { variants, .. } => variants
            .iter()
            .filter_map(|variant| variant.payload.as_ref())
            .for_each(|payload| collect_referenced_ids(payload, ids)),
        TypeShape::Ref(id) => ids.push(*id),
        TypeShape::Unit
        | TypeShape::Bool
        | TypeShape::I8
        | TypeShape::I16
        | TypeShape::I32
        | TypeShape::I64
        | TypeShape::I128
        | TypeShape::Isize
        | TypeShape::U8
        | TypeShape::U16
        | TypeShape::U32
        | TypeShape::U64
        | TypeShape::U128
        | TypeShape::Usize
        | TypeShape::F32
        | TypeShape::F64
        | TypeShape::String
        | TypeShape::Char
        | TypeShape::Bytes { .. } => {}
    }
}
