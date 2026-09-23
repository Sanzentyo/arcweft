//! Validated nominal definitions and their finite, typed schema graph.

use crate::entry::{RuntimeNominalTypeId, TypeLayoutHash};
use crate::pattern::RuntimeSemanticTypeId;
use crate::value::RuntimeRecordFieldId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::traversal::{SchemaPath, SchemaStep};
use super::{
    RuntimeNominalRecordShape, RuntimeNominalRecordShapeError, RuntimeSchemaError,
    RuntimeSchemaLimits, RuntimeTypeSchema,
};

pub(super) mod encoding;
mod validation;

#[cfg(test)]
mod tests;

/// Exact nominal instance identity. Membership is established by a graph.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeNominalSchemaIdentity {
    nominal: RuntimeNominalTypeId,
    semantic_identity: RuntimeSemanticTypeId,
}

impl RuntimeNominalSchemaIdentity {
    #[must_use]
    pub const fn new(
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Self {
        Self {
            nominal,
            semantic_identity,
        }
    }

    #[must_use]
    pub const fn nominal(&self) -> &RuntimeNominalTypeId {
        &self.nominal
    }

    #[must_use]
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        self.semantic_identity
    }
}

/// One ordered field of a structural record schema.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeSchemaValueField {
    field: RuntimeRecordFieldId,
    name: String,
    schema: RuntimeTypeSchema,
}

impl RuntimeSchemaValueField {
    #[must_use]
    pub const fn new(field: RuntimeRecordFieldId, name: String, schema: RuntimeTypeSchema) -> Self {
        Self {
            field,
            name,
            schema,
        }
    }

    #[must_use]
    pub const fn field(&self) -> RuntimeRecordFieldId {
        self.field
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn schema(&self) -> &RuntimeTypeSchema {
        &self.schema
    }

    pub(super) fn into_schema(self) -> RuntimeTypeSchema {
        self.schema
    }
}

/// One source-ordered nominal field, named only for a record struct.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeNominalSchemaField {
    field: RuntimeRecordFieldId,
    name: Option<String>,
    schema: RuntimeTypeSchema,
}

impl RuntimeNominalSchemaField {
    #[must_use]
    pub const fn new(
        field: RuntimeRecordFieldId,
        name: Option<String>,
        schema: RuntimeTypeSchema,
    ) -> Self {
        Self {
            field,
            name,
            schema,
        }
    }

    #[must_use]
    pub const fn field(&self) -> RuntimeRecordFieldId {
        self.field
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    #[must_use]
    pub const fn schema(&self) -> &RuntimeTypeSchema {
        &self.schema
    }
}

/// A source-ordered case; payload presence preserves unit/tuple/record forms.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeNominalSchemaCase {
    ordinal: u32,
    name: String,
    payload: Option<RuntimeTypeSchema>,
}

impl RuntimeNominalSchemaCase {
    #[must_use]
    pub const fn new(ordinal: u32, name: String, payload: Option<RuntimeTypeSchema>) -> Self {
        Self {
            ordinal,
            name,
            payload,
        }
    }

    #[must_use]
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn payload(&self) -> Option<&RuntimeTypeSchema> {
        self.payload.as_ref()
    }
}

/// Complete structural body of one exact nominal instance.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeNominalSchemaBody {
    Record {
        shape: RuntimeNominalRecordShape,
        fields: Box<[RuntimeNominalSchemaField]>,
    },
    Variant {
        cases: Box<[RuntimeNominalSchemaCase]>,
    },
}

/// Definition and ordered arguments, supplied together before graph admission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeNominalSchemaDefinition {
    identity: RuntimeNominalSchemaIdentity,
    arguments: Box<[RuntimeTypeSchema]>,
    body: RuntimeNominalSchemaBody,
    data_codec: Option<super::RuntimeCodecUse>,
}

impl RuntimeNominalSchemaDefinition {
    #[must_use]
    pub fn new(
        identity: RuntimeNominalSchemaIdentity,
        arguments: impl Into<Box<[RuntimeTypeSchema]>>,
        body: RuntimeNominalSchemaBody,
    ) -> Self {
        Self {
            identity,
            arguments: arguments.into(),
            body,
            data_codec: None,
        }
    }

    #[must_use]
    pub const fn identity(&self) -> &RuntimeNominalSchemaIdentity {
        &self.identity
    }

    #[must_use]
    pub fn arguments(&self) -> &[RuntimeTypeSchema] {
        &self.arguments
    }

    #[must_use]
    pub const fn body(&self) -> &RuntimeNominalSchemaBody {
        &self.body
    }

    /// Attaches declaration-owned wire policies to the existing logical body.
    /// Graph admission validates every occurrence against the original schema.
    #[must_use]
    pub fn with_data_codec(mut self, codec: super::RuntimeCodecUse) -> Self {
        self.data_codec = Some(codec);
        self
    }

    #[must_use]
    pub const fn data_codec(&self) -> Option<&super::RuntimeCodecUse> {
        self.data_codec.as_ref()
    }

    pub(crate) fn codec_uses(
        &self,
        limits: RuntimeSchemaLimits,
    ) -> Result<Option<super::RuntimeNominalCodecUses>, super::RuntimeCodecUseError> {
        use super::RuntimeCodecUse as Use;
        let Some(body) = &self.data_codec else {
            return Ok(None);
        };
        let mismatch = || super::RuntimeCodecUseError::Mismatch {
            path: "$body".to_owned(),
        };
        match (&self.body, body) {
            (
                RuntimeNominalSchemaBody::Record {
                    shape: RuntimeNominalRecordShape::Newtype,
                    fields,
                },
                Use::Newtype { inner },
            ) if fields.len() == 1 => {
                inner.validate_schema(fields[0].schema(), limits)?;
            }
            (
                RuntimeNominalSchemaBody::Record {
                    shape: RuntimeNominalRecordShape::Unit,
                    fields,
                },
                Use::Plain,
            ) if fields.is_empty() => {}
            (
                RuntimeNominalSchemaBody::Record {
                    shape: RuntimeNominalRecordShape::Tuple,
                    fields,
                },
                Use::Tuple { items },
            ) if fields.len() == items.len() => {
                for (field, policy) in fields.iter().zip(items) {
                    policy.validate_schema(field.schema(), limits)?;
                }
            }
            (
                RuntimeNominalSchemaBody::Record {
                    shape: RuntimeNominalRecordShape::Record,
                    fields,
                },
                Use::Record {
                    fields: policies, ..
                },
            ) if fields.len() == policies.len() => {
                for (field, policy) in fields.iter().zip(policies) {
                    policy.value.validate_schema(field.schema(), limits)?;
                }
            }
            (
                RuntimeNominalSchemaBody::Variant { cases },
                Use::Enum {
                    cases: policies, ..
                },
            ) if cases.len() == policies.len() => {
                for (case, policy) in cases.iter().zip(policies) {
                    match (case.payload(), policy.payload.as_ref()) {
                        (None, None) => {}
                        (Some(schema), Some(policy)) => policy.validate_schema(schema, limits)?,
                        _ => return Err(mismatch()),
                    }
                }
            }
            _ => return Err(mismatch()),
        }
        let arguments = self
            .arguments
            .iter()
            .map(|schema| Use::from_schema(schema, limits))
            .collect::<Result<_, _>>()?;
        Ok(Some(super::RuntimeNominalCodecUses {
            body: body.clone(),
            arguments,
        }))
    }

    fn walk_schemas<'a, E>(
        &'a self,
        mut visit: impl FnMut(&'a RuntimeTypeSchema, usize, &SchemaPath<'a>) -> Result<(), E>,
    ) -> Result<(), E> {
        let root = self.identity.nominal.as_str();
        for (index, argument) in self.arguments.iter().enumerate() {
            argument.walk(
                &mut SchemaPath::new(root, SchemaStep::Argument(index)),
                &mut visit,
            )?;
        }
        match &self.body {
            RuntimeNominalSchemaBody::Record { fields, .. } => {
                for (index, field) in fields.iter().enumerate() {
                    field.schema.walk(
                        &mut SchemaPath::new(root, SchemaStep::Field(index, field.name())),
                        &mut visit,
                    )?;
                }
            }
            RuntimeNominalSchemaBody::Variant { cases } => {
                for (index, case) in cases.iter().enumerate() {
                    if let Some(payload) = &case.payload {
                        payload.walk(
                            &mut SchemaPath::new(root, SchemaStep::Case(index, &case.name)),
                            &mut visit,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Validated finite nominal graph. Raw serde definitions must pass `try_new`.
/// This admission proof is consumed by plan construction, never persisted as
/// a second runtime type catalog. Limits do not participate in layout identity.
#[derive(Debug)]
pub struct RuntimeNominalSchemaGraph {
    definitions: Box<[RuntimeNominalSchemaDefinition]>,
    limits: RuntimeSchemaLimits,
}

// Admission limits bound construction and use, not the represented source type.
impl PartialEq for RuntimeNominalSchemaGraph {
    fn eq(&self, other: &Self) -> bool {
        self.definitions == other.definitions
    }
}

impl Eq for RuntimeNominalSchemaGraph {}

/// A nominal schema graph cannot be admitted or canonically encoded.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeNominalSchemaGraphError {
    #[error("nominal codec-use admission failed: {0}")]
    CodecUse(#[from] super::RuntimeCodecUseError),
    #[error("source proofs disagree on the definition of {semantic_identity:?}")]
    ConflictingDefinition {
        semantic_identity: RuntimeSemanticTypeId,
    },
    #[error("nominal schema graph exceeds `{budget}` budget")]
    BudgetExceeded { budget: &'static str },
    #[error("invalid schema identity at `{path}`: {source}")]
    InvalidIdentity {
        path: String,
        source: crate::entry::RuntimeIdentityError,
    },
    #[error("nominal schema graph repeats identity {identity:?}")]
    DuplicateIdentity {
        identity: RuntimeNominalSchemaIdentity,
    },
    #[error("nominal identity {nominal:?} has conflicting semantic identities")]
    ConflictingNominalIdentity {
        nominal: RuntimeNominalTypeId,
        first: RuntimeSemanticTypeId,
        second: RuntimeSemanticTypeId,
    },
    #[error("semantic identity {semantic_identity:?} has conflicting nominal identities")]
    ConflictingSemanticIdentity {
        semantic_identity: RuntimeSemanticTypeId,
        first: RuntimeNominalTypeId,
        second: RuntimeNominalTypeId,
    },
    #[error("nominal schema root {root:?} is absent")]
    UnknownRoot { root: RuntimeSemanticTypeId },
    #[error("nominal schema reference {reference:?} at `{path}` is absent or mismatched")]
    DanglingReference {
        path: String,
        reference: RuntimeNominalSchemaIdentity,
    },
    #[error("named schema reference `{name}` at `{path}` requires a tree schema owner")]
    NonNominalReference { path: String, name: String },
    #[error(
        "inline tree nominal definition `{name}` at `{path}` requires a typed graph definition and reference"
    )]
    InlineNominalDefinition { path: String, name: String },
    #[error("field {field} at `{path}` does not have source ordinal {ordinal}")]
    InvalidFieldIdentity {
        path: String,
        ordinal: usize,
        field: RuntimeRecordFieldId,
    },
    #[error("case ordinal {actual} at `{path}` differs from source ordinal {expected}")]
    InvalidCaseOrdinal {
        path: String,
        expected: usize,
        actual: u32,
    },
    #[error("invalid record shape at `{path}`: {source}")]
    RecordShape {
        path: String,
        source: RuntimeNominalRecordShapeError,
    },
    #[error("case {ordinal} at `{path}` has an empty name")]
    EmptyCaseName { path: String, ordinal: usize },
    #[error("case {ordinal} at `{path}` repeats name `{name}`")]
    DuplicateCaseName {
        path: String,
        ordinal: usize,
        name: String,
    },
    #[error("opaque schema at `{path}` requires exact identity admission")]
    NonExactOpaque { path: String },
    #[error("nominal schema encoding failed: {0}")]
    Encoding(#[from] RuntimeSchemaError),
}

impl RuntimeNominalSchemaGraph {
    pub(crate) const fn limits(&self) -> RuntimeSchemaLimits {
        self.limits
    }

    /// Checks the complete definition set and all reference edges atomically.
    pub fn try_new(
        definitions: impl Into<Box<[RuntimeNominalSchemaDefinition]>>,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeNominalSchemaGraphError> {
        validation::admit(definitions.into(), limits)
    }

    /// Joins already admitted source documents without reconstructing their
    /// definitions from executable rows. Shared documents and equal definitions
    /// are retained once; conflicting definitions reject the entire join.
    pub fn try_merge<'a>(
        graphs: impl IntoIterator<Item = &'a Self>,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeNominalSchemaGraphError> {
        let mut seen = std::collections::BTreeSet::new();
        let mut definitions = std::collections::BTreeMap::new();
        let mut work = validation::Work::new(limits);
        for graph in graphs {
            if !seen.insert(std::ptr::from_ref(graph)) {
                continue;
            }
            for definition in graph.definitions() {
                work.definition(definition, |_| Ok(()))?;
                let semantic_identity = definition.identity().semantic_identity();
                if let Some(previous) = definitions.insert(semantic_identity, definition)
                    && previous != definition
                {
                    return Err(RuntimeNominalSchemaGraphError::ConflictingDefinition {
                        semantic_identity,
                    });
                }
            }
        }
        Self::try_new(
            definitions.into_values().cloned().collect::<Vec<_>>(),
            limits,
        )
    }

    #[must_use]
    pub fn definition(
        &self,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Option<&RuntimeNominalSchemaDefinition> {
        self.definitions
            .binary_search_by_key(&semantic_identity, |definition| {
                definition.identity.semantic_identity
            })
            .ok()
            .map(|index| &self.definitions[index])
    }

    pub fn definitions(&self) -> impl ExactSizeIterator<Item = &RuntimeNominalSchemaDefinition> {
        self.definitions.iter()
    }

    /// Hashes only this root's reachable definitions in semantic-identity order.
    pub fn try_layout_hash(
        &self,
        root: RuntimeSemanticTypeId,
    ) -> Result<TypeLayoutHash, RuntimeNominalSchemaGraphError> {
        encoding::layout_hash(self, root)
    }

    pub fn try_layouts(
        &self,
    ) -> Result<Box<[(RuntimeSemanticTypeId, TypeLayoutHash)]>, RuntimeNominalSchemaGraphError>
    {
        encoding::layouts(self)
    }

    /// Validates and hashes a complete value under this graph's exact root.
    /// Layout work shares the graph's admission limits for the operation;
    /// every logical value, including opaque interiors, shares `limits`.
    pub fn accepts_value(
        &self,
        root: RuntimeSemanticTypeId,
        value: &crate::value::RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<crate::entry::RuntimeValueDigest, RuntimeSchemaError> {
        super::value_validation::SchemaValueValidation::graph(self, limits)
            .validate(value, super::value_validation::Expected::GraphRoot(root))
    }
}

impl Drop for RuntimeNominalSchemaGraph {
    fn drop(&mut self) {
        for definition in &mut self.definitions {
            for argument in std::mem::take(&mut definition.arguments) {
                argument.drop_iteratively();
            }
            match &mut definition.body {
                RuntimeNominalSchemaBody::Record { fields, .. } => {
                    for field in fields {
                        std::mem::replace(&mut field.schema, RuntimeTypeSchema::Unit)
                            .drop_iteratively();
                    }
                }
                RuntimeNominalSchemaBody::Variant { cases } => {
                    for case in cases {
                        if let Some(payload) = case.payload.take() {
                            payload.drop_iteratively();
                        }
                    }
                }
            }
        }
    }
}
