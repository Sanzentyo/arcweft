//! Atomic graph admission, including limits before structural joins.

use std::collections::{BTreeMap, BTreeSet};

use crate::pattern::{RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeProducerId};

use super::{
    RuntimeNominalRecordShape, RuntimeNominalSchemaBody, RuntimeNominalSchemaDefinition,
    RuntimeNominalSchemaGraph, RuntimeNominalSchemaGraphError as Error, RuntimeSchemaError,
    RuntimeSchemaLimits, RuntimeTypeSchema,
};

pub(super) fn admit(
    definitions: Box<[RuntimeNominalSchemaDefinition]>,
    limits: RuntimeSchemaLimits,
) -> Result<RuntimeNominalSchemaGraph, Error> {
    // The candidate owns iterative cleanup before any fallible check. Even an
    // input far beyond the depth limit must be safe to reject and destroy.
    let mut graph = RuntimeNominalSchemaGraph {
        definitions,
        limits,
    };
    let mut work = Work::new(limits);
    work.collection(graph.definitions.len())?;
    for definition in &graph.definitions {
        work.definition(definition, |_| Ok(()))?;
    }
    graph
        .definitions
        .sort_by_key(|definition| definition.identity.semantic_identity);
    let mut nominal_owners = BTreeMap::new();
    let mut semantic_owners = BTreeMap::new();
    for definition in &graph.definitions {
        let identity = &definition.identity;
        super::RuntimeNominalTypeId::try_new(identity.nominal.as_str().to_owned()).map_err(
            |source| Error::InvalidIdentity {
                path: "definition.nominal".to_owned(),
                source,
            },
        )?;
        if let Some(first) = semantic_owners.insert(identity.semantic_identity, &identity.nominal) {
            return Err(if first == &identity.nominal {
                Error::DuplicateIdentity {
                    identity: identity.clone(),
                }
            } else {
                Error::ConflictingSemanticIdentity {
                    semantic_identity: identity.semantic_identity,
                    first: first.clone(),
                    second: identity.nominal.clone(),
                }
            });
        }
        if let Some(first) = nominal_owners.insert(&identity.nominal, identity.semantic_identity) {
            return Err(Error::ConflictingNominalIdentity {
                nominal: identity.nominal.clone(),
                first,
                second: identity.semantic_identity,
            });
        }
    }
    for definition in &graph.definitions {
        definition.validate(&graph)?;
        definition.codec_uses(limits)?;
    }
    super::encoding::validate_document_size(&graph)?;
    Ok(graph)
}

pub(super) struct Work {
    limits: RuntimeSchemaLimits,
    nodes: usize,
}

impl Work {
    pub(super) const fn new(limits: RuntimeSchemaLimits) -> Self {
        Self { limits, nodes: 0 }
    }

    fn node(&mut self, depth: usize) -> Result<(), Error> {
        if !self.limits.permits_depth(depth) {
            return Err(Error::BudgetExceeded { budget: "depth" });
        }
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or(Error::BudgetExceeded { budget: "nodes" })?;
        if !self.limits.permits_nodes(self.nodes) {
            return Err(Error::BudgetExceeded { budget: "nodes" });
        }
        Ok(())
    }

    fn collection(&self, count: usize) -> Result<(), Error> {
        if !self.limits.permits_sequence_items(count) {
            return Err(Error::BudgetExceeded {
                budget: "sequence_items",
            });
        }
        Ok(())
    }

    fn string(&self, text: &str) -> Result<(), Error> {
        if !self.limits.permits_string_bytes(text.len()) {
            return Err(Error::BudgetExceeded {
                budget: "string_bytes",
            });
        }
        u32::try_from(text.len()).map_err(|_| RuntimeSchemaError::SchemaEncodingOverflow)?;
        Ok(())
    }

    pub(super) fn definition<'a>(
        &mut self,
        definition: &'a RuntimeNominalSchemaDefinition,
        mut visit: impl FnMut(&'a RuntimeTypeSchema) -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.node(0)?;
        self.string(definition.identity.nominal.as_str())?;
        self.collection(definition.arguments.len())?;
        if let Some(codec) = &definition.data_codec {
            codec.walk(|codec, depth| {
                use super::super::RuntimeCodecUse as Use;
                self.node(depth)?;
                match codec {
                    Use::Record { name, fields, .. } => {
                        self.string(name)?;
                        self.collection(fields.len())?;
                        for field in fields {
                            self.string(&field.wire_name)?;
                        }
                    }
                    Use::Enum {
                        name, tag, cases, ..
                    } => {
                        self.string(name)?;
                        self.collection(cases.len())?;
                        for case in cases {
                            self.string(&case.wire_name)?;
                        }
                        match tag {
                            super::super::RuntimeEnumTagStyle::External => {}
                            super::super::RuntimeEnumTagStyle::Internal { tag } => {
                                self.string(tag)?
                            }
                            super::super::RuntimeEnumTagStyle::Adjacent { tag, content } => {
                                self.string(tag)?;
                                self.string(content)?;
                            }
                        }
                    }
                    Use::Tuple { items }
                    | Use::RecordFields { fields: items }
                    | Use::Builtin { payloads: items }
                    | Use::Choice {
                        alternatives: items,
                    }
                    | Use::Opaque { arguments: items } => self.collection(items.len())?,
                    _ => {}
                }
                Ok::<_, Error>(())
            })?;
        }
        match &definition.body {
            RuntimeNominalSchemaBody::Record { fields, .. } => {
                self.collection(fields.len())?;
                for field in fields {
                    self.node(0)?;
                    if let Some(name) = &field.name {
                        self.string(name)?;
                    }
                }
            }
            RuntimeNominalSchemaBody::Variant { cases } => {
                self.collection(cases.len())?;
                for case in cases {
                    self.node(0)?;
                    self.string(&case.name)?;
                }
            }
        }
        definition.walk_schemas(|schema, depth, _| {
            self.schema(schema, depth)?;
            visit(schema)
        })
    }

    fn schema(&mut self, schema: &RuntimeTypeSchema, depth: usize) -> Result<(), Error> {
        self.node(depth)?;
        match schema {
            RuntimeTypeSchema::Builtin(builtin) => {
                self.collection(builtin.owner().cases().len())?;
                for case in builtin.owner().cases() {
                    self.node(depth)?;
                    self.string(case.name())?;
                }
            }
            RuntimeTypeSchema::Tuple(items) | RuntimeTypeSchema::Choice(items) => {
                self.collection(items.len())?;
            }
            RuntimeTypeSchema::ExactOpaque { owner, arguments } => {
                self.string(owner.producer().as_str())?;
                self.collection(arguments.len())?;
            }
            RuntimeTypeSchema::RecordValue { fields } => {
                self.collection(fields.len())?;
                for field in fields {
                    self.node(depth)?;
                    self.string(field.name())?;
                }
            }
            RuntimeTypeSchema::Record { name, fields, .. } => {
                self.string(name)?;
                self.collection(fields.len())?;
                for field in fields {
                    self.node(depth)?;
                    self.string(&field.rust_name)?;
                    self.string(&field.wire_name)?;
                }
            }
            RuntimeTypeSchema::Enum {
                name,
                variants,
                tag,
                ..
            } => {
                self.string(name)?;
                self.collection(variants.len())?;
                for variant in variants {
                    self.node(depth)?;
                    self.string(&variant.rust_name)?;
                    self.string(&variant.wire_name)?;
                }
                match tag {
                    super::super::RuntimeEnumTagStyle::External => {}
                    super::super::RuntimeEnumTagStyle::Internal { tag } => self.string(tag)?,
                    super::super::RuntimeEnumTagStyle::Adjacent { tag, content } => {
                        self.string(tag)?;
                        self.string(content)?;
                    }
                }
            }
            RuntimeTypeSchema::Named(name) => self.string(name)?,
            RuntimeTypeSchema::NominalRef(identity) => self.string(identity.nominal.as_str())?,
            RuntimeTypeSchema::Unit
            | RuntimeTypeSchema::Bool
            | RuntimeTypeSchema::I8
            | RuntimeTypeSchema::I16
            | RuntimeTypeSchema::I32
            | RuntimeTypeSchema::I64
            | RuntimeTypeSchema::I128
            | RuntimeTypeSchema::ISize
            | RuntimeTypeSchema::U8
            | RuntimeTypeSchema::U16
            | RuntimeTypeSchema::U32
            | RuntimeTypeSchema::U64
            | RuntimeTypeSchema::U128
            | RuntimeTypeSchema::USize
            | RuntimeTypeSchema::F32
            | RuntimeTypeSchema::F64
            | RuntimeTypeSchema::String
            | RuntimeTypeSchema::Color
            | RuntimeTypeSchema::Char
            | RuntimeTypeSchema::Never
            | RuntimeTypeSchema::Duration
            | RuntimeTypeSchema::Progress
            | RuntimeTypeSchema::EntityReference
            | RuntimeTypeSchema::AgentValue
            | RuntimeTypeSchema::Bytes { .. }
            | RuntimeTypeSchema::Seq(_)
            | RuntimeTypeSchema::Array { .. }
            | RuntimeTypeSchema::Map { .. } => {}
        }
        Ok(())
    }
}

impl RuntimeNominalSchemaDefinition {
    fn validate(&self, graph: &RuntimeNominalSchemaGraph) -> Result<(), Error> {
        let root = self.identity.nominal.as_str();
        match &self.body {
            RuntimeNominalSchemaBody::Record { shape, fields } => {
                shape
                    .validate_field_names(fields.iter().map(|field| field.name()))
                    .map_err(|source| Error::RecordShape {
                        path: root.to_owned(),
                        source,
                    })?;
                for (ordinal, field) in fields.iter().enumerate() {
                    if usize::try_from(field.field.zero_based()) != Ok(ordinal) {
                        return Err(Error::InvalidFieldIdentity {
                            path: root.to_owned(),
                            ordinal,
                            field: field.field,
                        });
                    }
                }
            }
            RuntimeNominalSchemaBody::Variant { cases } => {
                let mut names = BTreeSet::new();
                for (expected, case) in cases.iter().enumerate() {
                    if usize::try_from(case.ordinal) != Ok(expected) {
                        return Err(Error::InvalidCaseOrdinal {
                            path: root.to_owned(),
                            expected,
                            actual: case.ordinal,
                        });
                    }
                    if case.name.is_empty() {
                        return Err(Error::EmptyCaseName {
                            path: root.to_owned(),
                            ordinal: expected,
                        });
                    }
                    if !names.insert(&case.name) {
                        return Err(Error::DuplicateCaseName {
                            path: root.to_owned(),
                            ordinal: expected,
                            name: case.name.clone(),
                        });
                    }
                }
            }
        }
        self.walk_schemas(|schema, _, path| {
            match schema {
                RuntimeTypeSchema::NominalRef(identity) => {
                    if graph
                        .definition(identity.semantic_identity)
                        .is_none_or(|definition| definition.identity != *identity)
                    {
                        return Err(Error::DanglingReference {
                            path: path.to_string(),
                            reference: identity.clone(),
                        });
                    }
                }
                RuntimeTypeSchema::Named(name) => {
                    return Err(Error::NonNominalReference {
                        path: path.to_string(),
                        name: name.clone(),
                    });
                }
                RuntimeTypeSchema::Record { name, .. } | RuntimeTypeSchema::Enum { name, .. } => {
                    return Err(Error::InlineNominalDefinition {
                        path: path.to_string(),
                        name: name.clone(),
                    });
                }
                RuntimeTypeSchema::RecordValue { fields } => {
                    RuntimeNominalRecordShape::Record
                        .validate_field_names(fields.iter().map(|field| Some(field.name())))
                        .map_err(|source| Error::RecordShape {
                            path: path.to_string(),
                            source,
                        })?;
                    for (ordinal, field) in fields.iter().enumerate() {
                        if usize::try_from(field.field().zero_based()) != Ok(ordinal) {
                            return Err(Error::InvalidFieldIdentity {
                                path: path.to_string(),
                                ordinal,
                                field: field.field(),
                            });
                        }
                    }
                }
                RuntimeTypeSchema::ExactOpaque { owner, .. }
                    if owner.admission() != RuntimeOpaqueTypeAdmission::ExactIdentity =>
                {
                    return Err(Error::NonExactOpaque {
                        path: path.to_string(),
                    });
                }
                RuntimeTypeSchema::ExactOpaque { owner, .. } => {
                    RuntimeOpaqueTypeProducerId::try_new(owner.producer().as_str().to_owned())
                        .map_err(|source| Error::InvalidIdentity {
                            path: path.to_string(),
                            source,
                        })?;
                }
                _ => {}
            }
            Ok(())
        })
    }
}
