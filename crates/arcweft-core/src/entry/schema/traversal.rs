//! Structural schema edges, shared by graph validation and reachability.

use std::fmt;

use super::{RuntimeSchemaValueField, RuntimeTypeSchema};

#[derive(Clone, Copy)]
pub(crate) enum SchemaStep<'a> {
    Argument(usize),
    Field(usize, Option<&'a str>),
    Case(usize, &'a str),
    Tuple(usize),
    Choice(usize),
    Sequence,
    Array,
    MapKey,
    MapValue,
}

impl fmt::Display for SchemaStep<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Argument(index) => write!(formatter, ".arguments[{index}]"),
            Self::Field(index, Some(name)) => write!(formatter, ".fields[{index}:{name}]"),
            Self::Field(index, None) => write!(formatter, ".fields[{index}]"),
            Self::Case(index, name) => write!(formatter, ".cases[{index}:{name}]"),
            Self::Tuple(index) => write!(formatter, ".tuple[{index}]"),
            Self::Choice(index) => write!(formatter, ".choice[{index}]"),
            Self::Sequence => formatter.write_str(".sequence"),
            Self::Array => formatter.write_str(".array"),
            Self::MapKey => formatter.write_str(".map.key"),
            Self::MapValue => formatter.write_str(".map.value"),
        }
    }
}

pub(crate) struct SchemaPath<'a> {
    root: &'a str,
    steps: Vec<SchemaStep<'a>>,
}

impl<'a> SchemaPath<'a> {
    pub(crate) fn root(root: &'a str) -> Self {
        Self {
            root,
            steps: Vec::new(),
        }
    }

    pub(crate) fn new(root: &'a str, step: SchemaStep<'a>) -> Self {
        Self {
            root,
            steps: vec![step],
        }
    }
}

impl fmt::Display for SchemaPath<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.root)?;
        for step in &self.steps {
            write!(formatter, "{step}")?;
        }
        Ok(())
    }
}

/// A context follows the schema's canonical edges while retaining its own
/// typed coordinate. Schema traversal, ordering, and path storage have one owner.
pub(crate) trait SchemaVisitor<'a> {
    type State: Copy;
    type Error;

    fn enter(
        &mut self,
        schema: &'a RuntimeTypeSchema,
        state: Self::State,
        depth: usize,
        path: &SchemaPath<'a>,
    ) -> Result<(), Self::Error>;

    fn child(
        &mut self,
        parent: Self::State,
        step: SchemaStep<'a>,
        path: &SchemaPath<'a>,
    ) -> Result<Self::State, Self::Error>;
}

struct StatelessVisitor<F>(F);

impl<'a, E, F> SchemaVisitor<'a> for StatelessVisitor<F>
where
    F: FnMut(&'a RuntimeTypeSchema, usize, &SchemaPath<'a>) -> Result<(), E>,
{
    type State = ();
    type Error = E;

    fn enter(
        &mut self,
        schema: &'a RuntimeTypeSchema,
        (): (),
        depth: usize,
        path: &SchemaPath<'a>,
    ) -> Result<(), E> {
        self.0(schema, depth, path)
    }

    fn child(&mut self, (): (), _: SchemaStep<'a>, _: &SchemaPath<'a>) -> Result<(), E> {
        Ok(())
    }
}

struct SchemaChildren<'a> {
    schema: &'a RuntimeTypeSchema,
    next: usize,
}

impl<'a> Iterator for SchemaChildren<'a> {
    type Item = (&'a RuntimeTypeSchema, SchemaStep<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        use RuntimeTypeSchema as Schema;
        let index = self.next;
        let child = match self.schema {
            Schema::Seq(inner) if index == 0 => (inner.as_ref(), SchemaStep::Sequence),
            Schema::Array { item, .. } if index == 0 => (item.as_ref(), SchemaStep::Array),
            Schema::Map { key, .. } if index == 0 => (key.as_ref(), SchemaStep::MapKey),
            Schema::Map { value, .. } if index == 1 => (value.as_ref(), SchemaStep::MapValue),
            Schema::Builtin(builtin) => {
                while let Some((case, payload)) = builtin.case(self.next) {
                    let ordinal = self.next;
                    self.next += 1;
                    if let Some(payload) = payload {
                        return Some((payload, SchemaStep::Case(ordinal, case.name())));
                    }
                }
                return None;
            }
            Schema::Tuple(items) => (items.get(index)?, SchemaStep::Tuple(index)),
            Schema::Choice(items) => (items.get(index)?, SchemaStep::Choice(index)),
            Schema::ExactOpaque { arguments, .. } => {
                (arguments.get(index)?, SchemaStep::Argument(index))
            }
            Schema::Record { fields, .. } => {
                let field = fields.get(index)?;
                (
                    &field.schema,
                    SchemaStep::Field(index, Some(&field.rust_name)),
                )
            }
            Schema::RecordValue { fields } => {
                let field = fields.get(index)?;
                (field.schema(), SchemaStep::Field(index, Some(field.name())))
            }
            Schema::Enum { variants, .. } => {
                while let Some(variant) = variants.get(self.next) {
                    let ordinal = self.next;
                    self.next += 1;
                    if let Some(payload) = &variant.payload {
                        return Some((payload, SchemaStep::Case(ordinal, &variant.rust_name)));
                    }
                }
                return None;
            }
            Schema::Unit
            | Schema::Bool
            | Schema::I8
            | Schema::I16
            | Schema::I32
            | Schema::I64
            | Schema::I128
            | Schema::ISize
            | Schema::U8
            | Schema::U16
            | Schema::U32
            | Schema::U64
            | Schema::U128
            | Schema::USize
            | Schema::F32
            | Schema::F64
            | Schema::String
            | Schema::Char
            | Schema::Never
            | Schema::Duration
            | Schema::Progress
            | Schema::EntityReference
            | Schema::AgentValue
            | Schema::Bytes { .. }
            | Schema::Named(_)
            | Schema::NominalRef(_)
            | Schema::Seq(_)
            | Schema::Array { .. }
            | Schema::Map { .. } => return None,
        };
        self.next += 1;
        Some(child)
    }
}

impl RuntimeTypeSchema {
    /// Visits each syntactic node once without following nominal references.
    /// The visitor sees the parent before its children. Only the active path
    /// and one child cursor per ancestor are retained; error paths are rendered
    /// on demand instead of allocating a path string at every node.
    pub(super) fn walk<'a, E>(
        &'a self,
        path: &mut SchemaPath<'a>,
        visit: impl FnMut(&'a Self, usize, &SchemaPath<'a>) -> Result<(), E>,
    ) -> Result<(), E> {
        self.walk_with_state(path, (), &mut StatelessVisitor(visit))
    }

    pub(crate) fn walk_with_state<'a, V: SchemaVisitor<'a>>(
        &'a self,
        path: &mut SchemaPath<'a>,
        state: V::State,
        visitor: &mut V,
    ) -> Result<(), V::Error> {
        visitor.enter(self, state, 1, path)?;
        let initial_path_len = path.steps.len();
        let mut parents = vec![(
            SchemaChildren {
                schema: self,
                next: 0,
            },
            state,
        )];
        while let Some((parent, state)) = parents.last_mut() {
            if let Some((schema, step)) = parent.next() {
                path.steps.push(step);
                let child = visitor.child(*state, step, path)?;
                visitor.enter(schema, child, parents.len() + 1, path)?;
                parents.push((SchemaChildren { schema, next: 0 }, child));
            } else {
                parents.pop();
                if path.steps.len() > initial_path_len {
                    path.steps.pop();
                }
            }
        }
        Ok(())
    }

    /// Dismantles an owned schema without relying on recursive `Box` drops.
    /// Graph admission uses this for both accepted graphs and rejected raw
    /// inputs, whose depth may exceed the selected policy by an arbitrary amount.
    pub(super) fn drop_iteratively(self) {
        let mut pending = vec![self];
        while let Some(schema) = pending.pop() {
            match schema {
                Self::Seq(inner) | Self::Array { item: inner, .. } => pending.push(*inner),
                Self::Map { key, value, .. } => {
                    pending.push(*key);
                    pending.push(*value);
                }
                Self::Builtin(builtin) => pending.extend(builtin.into_payloads()),
                Self::Tuple(items) | Self::Choice(items) => pending.extend(items),
                Self::ExactOpaque { arguments, .. } => pending.extend(arguments),
                Self::Record { fields, .. } => {
                    pending.extend(fields.into_iter().map(|field| field.schema));
                }
                Self::RecordValue { fields } => {
                    pending.extend(fields.into_iter().map(RuntimeSchemaValueField::into_schema));
                }
                Self::Enum { variants, .. } => {
                    pending.extend(variants.into_iter().filter_map(|variant| variant.payload));
                }
                Self::Unit
                | Self::Bool
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
                | Self::Never
                | Self::Duration
                | Self::Progress
                | Self::EntityReference
                | Self::AgentValue
                | Self::Bytes { .. }
                | Self::Named(_)
                | Self::NominalRef(_) => {}
            }
        }
    }
}
