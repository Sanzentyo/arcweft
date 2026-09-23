//! Canonical reachable graph documents with one allowance per operation.

use std::collections::{BTreeMap, BTreeSet};

use super::super::{CanonicalBlake3Sink, CanonicalSink, CanonicalWriter, RuntimeSchemaError};
use super::{
    RuntimeNominalSchemaBody, RuntimeNominalSchemaDefinition, RuntimeNominalSchemaGraph,
    RuntimeNominalSchemaGraphError as Error, RuntimeSemanticTypeId, RuntimeTypeSchema,
    TypeLayoutHash, validation::Work,
};

#[derive(Default)]
struct SizeSink(u64);

impl CanonicalSink for SizeSink {
    fn write(&mut self, bytes: &[u8]) -> Result<(), RuntimeSchemaError> {
        self.0 = self
            .0
            .checked_add(u64::try_from(bytes.len()).map_err(|_| {
                RuntimeSchemaError::BudgetExceeded {
                    budget: "encoded_bytes",
                }
            })?)
            .ok_or(RuntimeSchemaError::BudgetExceeded {
                budget: "encoded_bytes",
            })?;
        Ok(())
    }

    fn bytes_written(&self) -> u64 {
        self.0
    }
}

pub(super) fn validate_document_size(graph: &RuntimeNominalSchemaGraph) -> Result<(), Error> {
    let mut sink = SizeSink::default();
    let root = graph
        .definitions
        .first()
        .map_or(RuntimeSemanticTypeId::from_bytes([0; 32]), |definition| {
            definition.identity.semantic_identity
        });
    document(
        graph,
        root,
        graph.definitions.iter(),
        graph.limits.max_encoded_bytes,
        &mut sink,
    )?;
    Ok(())
}

pub(super) fn layout_hash(
    graph: &RuntimeNominalSchemaGraph,
    root: RuntimeSemanticTypeId,
) -> Result<TypeLayoutHash, Error> {
    LayoutOperation::new(graph).hash(root)
}

pub(super) fn layouts(
    graph: &RuntimeNominalSchemaGraph,
) -> Result<Box<[(RuntimeSemanticTypeId, TypeLayoutHash)]>, Error> {
    let mut operation = LayoutOperation::new(graph);
    graph
        .definitions
        .iter()
        .map(|definition| {
            let root = definition.identity.semantic_identity;
            operation.hash(root).map(|layout| (root, layout))
        })
        .collect()
}

pub(in crate::entry::schema) struct LayoutOperation<'a> {
    graph: &'a RuntimeNominalSchemaGraph,
    work: Work,
    remaining_bytes: u64,
    completed: BTreeMap<RuntimeSemanticTypeId, TypeLayoutHash>,
}

impl<'a> LayoutOperation<'a> {
    pub(in crate::entry::schema) fn new(graph: &'a RuntimeNominalSchemaGraph) -> Self {
        Self {
            graph,
            work: Work::new(graph.limits),
            remaining_bytes: graph.limits.max_encoded_bytes,
            completed: BTreeMap::new(),
        }
    }

    pub(in crate::entry::schema) const fn graph(&self) -> &'a RuntimeNominalSchemaGraph {
        self.graph
    }

    pub(in crate::entry::schema) fn hash(
        &mut self,
        root: RuntimeSemanticTypeId,
    ) -> Result<TypeLayoutHash, Error> {
        if let Some(layout) = self.completed.get(&root) {
            return Ok(*layout);
        }
        let mut sink = CanonicalBlake3Sink::default();
        self.write(root, &mut sink)?;
        let layout = TypeLayoutHash::from_bytes(sink.finish());
        self.completed.insert(root, layout);
        Ok(layout)
    }

    fn write<S: CanonicalSink + ?Sized>(
        &mut self,
        root: RuntimeSemanticTypeId,
        sink: &mut S,
    ) -> Result<(), Error> {
        let root_definition = self
            .graph
            .definition(root)
            .ok_or(Error::UnknownRoot { root })?;
        let mut pending = vec![root_definition];
        let mut reached = BTreeSet::new();
        while let Some(definition) = pending.pop() {
            if !reached.insert(definition.identity.semantic_identity) {
                continue;
            }
            self.work.definition(definition, |schema| {
                if let RuntimeTypeSchema::NominalRef(identity) = schema {
                    // The graph's constructor checked the exact pair. No name
                    // resolution or alternative definition can enter this walk.
                    let target = self
                        .graph
                        .definition(identity.semantic_identity)
                        .expect("validated nominal references have a definition");
                    pending.push(target);
                }
                Ok(())
            })?;
        }
        let definitions = self
            .graph
            .definitions
            .iter()
            .filter(|definition| reached.contains(&definition.identity.semantic_identity))
            .collect::<Vec<_>>();
        document(
            self.graph,
            root,
            definitions.into_iter(),
            self.remaining_bytes,
            sink,
        )?;
        self.remaining_bytes -= sink.bytes_written();
        Ok(())
    }
}

fn document<'a, S: CanonicalSink + ?Sized>(
    graph: &RuntimeNominalSchemaGraph,
    root: RuntimeSemanticTypeId,
    definitions: impl ExactSizeIterator<Item = &'a RuntimeNominalSchemaDefinition>,
    max_encoded_bytes: u64,
    sink: &mut S,
) -> Result<(), RuntimeSchemaError> {
    let mut writer = CanonicalWriter {
        sink,
        max_encoded_bytes,
        max_string_bytes: Some(graph.limits.max_string_bytes),
    };
    writer.extend(b"arcweft.nominal-schema-graph\0")?;
    writer.var_u32(1)?;
    writer.extend(root.as_bytes())?;
    writer.len(definitions.len())?;
    for definition in definitions {
        writer.string(definition.identity.nominal.as_str())?;
        writer.extend(definition.identity.semantic_identity.as_bytes())?;
        writer.len(definition.arguments.len())?;
        for argument in &definition.arguments {
            super::super::encoding::schema(argument, &mut writer, Some(graph))?;
        }
        match &definition.body {
            RuntimeNominalSchemaBody::Record { shape, fields } => {
                writer.u8(0)?;
                writer.u8(shape.semantic_tag())?;
                writer.len(fields.len())?;
                for field in fields {
                    writer.var_u32(field.field.get().get())?;
                    writer.option(field.name.as_ref(), |writer, name| writer.string(name))?;
                    super::super::encoding::schema(&field.schema, &mut writer, Some(graph))?;
                }
            }
            RuntimeNominalSchemaBody::Variant { cases } => {
                writer.u8(1)?;
                writer.len(cases.len())?;
                for case in cases {
                    writer.var_u32(case.ordinal)?;
                    writer.string(&case.name)?;
                    writer.option(case.payload.as_ref(), |writer, schema| {
                        super::super::encoding::schema(schema, writer, Some(graph))
                    })?;
                }
            }
        }
        writer.option(definition.data_codec.as_ref(), |writer, codec| {
            super::super::codec_use::encode(codec, writer)
        })?;
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn bytes(
    graph: &RuntimeNominalSchemaGraph,
    root: RuntimeSemanticTypeId,
) -> Result<Vec<u8>, Error> {
    let mut sink = super::super::CanonicalBytesSink::default();
    LayoutOperation::new(graph).write(root, &mut sink)?;
    Ok(sink.finish())
}
