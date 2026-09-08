//! Canonical v1 definition codec and semantic hash transcripts.

use super::{
    CanonicalEncodeError, CanonicalEncoder, CanonicalHashSink, CanonicalLengthSink,
    CanonicalReader, CanonicalSink, CanonicalVecSink, FX_MAX_DEFINITION_CANONICAL_BYTES,
    FX_MAX_DEFINITION_PARAMETER_NAME_BYTES, FX_MAX_FONT_FAMILY_NAME_BYTES,
    FX_MAX_GRAPH_CHILD_EDGES_PER_DEFINITION, FX_MAX_GRAPH_NODES_PER_DEFINITION,
    FX_MAX_PARAMETERS_PER_DEFINITION, FX_MAX_RESOURCE_ID_BYTES, FxAbiHash, FxDefinition,
    FxDefinitionArgumentValue, FxDefinitionError, FxDefinitionGraphContext, FxDefinitionParameter,
    FxDefinitionParameterIndex, FxDefinitionParameterLayout, FxDefinitionParameterLayoutDigest,
    FxDefinitionParameterLayoutRow, FxDefinitionParameterRef, FxDefinitionParameterSchema,
    FxDefinitionParameterSchemaDigest, FxDefinitionParameterType, FxFontFamilyName,
    FxFontFamilyNameError, FxGraph, FxGraphError, FxId, FxIdCanonicalDecodeError, FxNode,
    FxNodeKind, FxParameterStorageSlot, FxPhase, FxProperty, FxPropertyId, FxRendererInterface,
    FxResourceId, FxResourceIdError, FxRuntimeParameterRef, FxRuntimeType, FxRuntimeValue,
    FxRuntimeValueDecodeError, FxSamplerProgram, FxSamplerProgramDecodeError, FxSelectorNameError,
    FxSemanticHash, FxShaderStage, FxStaticType, FxStaticValue, FxTarget,
    FxUniformRecordDecodeError, validate_definition_graph, validate_definition_parameters,
};
use crate::fx::{
    FX_MAX_SELECTOR_NAME_BYTES, FxCanonicalDecodeError, FxSelectorDomain, FxSelectorId,
    FxUniformRecord,
};
use thiserror::Error;

impl From<CanonicalEncodeError> for FxDefinitionError {
    fn from(error: CanonicalEncodeError) -> Self {
        match error {
            CanonicalEncodeError::LengthOverflow => Self::CanonicalLengthOverflow,
            CanonicalEncodeError::AllocationFailed => Self::CanonicalAllocationFailed,
            CanonicalEncodeError::LengthMismatch { actual, expected } => {
                Self::CanonicalLengthMismatch { actual, expected }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxDefinitionDecodeError {
    #[error(transparent)]
    Canonical(#[from] FxCanonicalDecodeError),
    #[error(transparent)]
    Identity(#[from] FxIdCanonicalDecodeError),
    #[error(transparent)]
    RuntimeValue(#[from] FxRuntimeValueDecodeError),
    #[error(transparent)]
    Resource(#[from] FxResourceIdError),
    #[error(transparent)]
    FontFamily(#[from] FxFontFamilyNameError),
    #[error(transparent)]
    Selector(#[from] FxSelectorNameError),
    #[error(transparent)]
    Sampler(#[from] FxSamplerProgramDecodeError),
    #[error(transparent)]
    Uniform(#[from] FxUniformRecordDecodeError),
    #[error(transparent)]
    Graph(#[from] FxGraphError),
    #[error(transparent)]
    Definition(#[from] FxDefinitionError),
    #[error("unknown Fx definition {kind} tag {tag}")]
    UnknownTag { kind: &'static str, tag: u8 },
    #[error("Fx definition {kind} value {value} is out of range")]
    IntegerOutOfRange { kind: &'static str, value: u64 },
    #[error("Fx definition {kind} count {actual} exceeds the owner limit of {limit}")]
    OwnerLimit {
        kind: &'static str,
        actual: u64,
        limit: usize,
    },
    #[error("Fx definition {kind} rows are not in strictly increasing canonical order")]
    NonCanonicalOrder { kind: &'static str },
    #[error("Fx definition allocation failed while decoding {kind}")]
    AllocationFailed { kind: &'static str },
    #[error("stored Fx definition parameter-layout digest does not match decoded parameters")]
    LayoutDigestMismatch,
    #[error("stored Fx definition parameter-schema digest does not match decoded schema")]
    ParameterSchemaDigestMismatch,
    #[error("stored Fx definition ABI hash does not match decoded definition")]
    AbiHashMismatch,
    #[error("stored Fx definition semantic hash does not match decoded definition")]
    SemanticHashMismatch,
}

pub(super) fn derive_parameter_layout_digest(
    abi: &[FxDefinitionParameterLayoutRow],
    abi_count: u16,
    runtime_count: u16,
    static_count: u16,
) -> FxDefinitionParameterLayoutDigest {
    let mut hasher = blake3::Hasher::new();
    {
        let mut encoder = CanonicalEncoder::new(CanonicalHashSink::new(&mut hasher));
        let result = (|| {
            encoder.domain_v1(b"arcweft.fx-definition-parameter-layout")?;
            encoder.unsigned(u64::from(abi_count))?;
            for row in abi {
                encoder.unsigned(u64::from(row.parameter.index.get()))?;
                encode_definition_parameter_type(&mut encoder, row.parameter.ty)?;
                match row.storage {
                    FxParameterStorageSlot::Runtime(slot) => {
                        encoder.tag(0)?;
                        encoder.unsigned(u64::from(slot.get()))?;
                    }
                    FxParameterStorageSlot::Static(slot) => {
                        encoder.tag(1)?;
                        encoder.unsigned(u64::from(slot.get()))?;
                    }
                }
            }
            encoder.unsigned(u64::from(runtime_count))?;
            encoder.unsigned(u64::from(static_count))
        })();
        match result {
            Ok(()) => {}
            Err(error) => match error {},
        }
    }
    FxDefinitionParameterLayoutDigest(*hasher.finalize().as_bytes())
}

impl FxDefinition {
    /// Validates a definition and derives both hashes from its typed contract.
    pub fn new(
        id: FxId,
        parameters: Vec<FxDefinitionParameter>,
        graph: FxGraph,
    ) -> Result<Self, FxDefinitionError> {
        let schema = FxDefinitionParameterSchema::new(id, parameters)?;
        Self::from_parameter_schema(schema, graph)
    }

    /// Seals a graph against the exact parameter schema used while compiling it.
    pub fn from_parameter_schema(
        schema: FxDefinitionParameterSchema,
        graph: FxGraph,
    ) -> Result<Self, FxDefinitionError> {
        validate_definition_graph(&graph, schema.parameter_layout())?;
        let abi_hash =
            FxAbiHash::for_definition(schema.parameters(), schema.parameter_layout(), &graph);
        let semantic_hash = FxSemanticHash::for_graph(&graph);
        let (id, parameters, layout) = schema.into_parts();
        Self::seal(id, parameters, layout, graph, abi_hash, semantic_hash)
    }

    /// Validates a decoded definition and rejects tampered stored hashes.
    pub fn from_parts(
        id: FxId,
        parameters: Vec<FxDefinitionParameter>,
        layout: FxDefinitionParameterLayout,
        graph: FxGraph,
        abi_hash: FxAbiHash,
        semantic_hash: FxSemanticHash,
    ) -> Result<Self, FxDefinitionError> {
        let expected_layout = validate_definition_parameters(&parameters)?;
        if layout != expected_layout {
            return Err(FxDefinitionError::ParameterLayoutMismatch);
        }
        validate_definition_graph(&graph, &layout)?;
        if abi_hash != FxAbiHash::for_definition(&parameters, &layout, &graph) {
            return Err(FxDefinitionError::AbiHashMismatch);
        }
        if semantic_hash != FxSemanticHash::for_graph(&graph) {
            return Err(FxDefinitionError::SemanticHashMismatch);
        }
        Self::seal(id, parameters, layout, graph, abi_hash, semantic_hash)
    }

    fn seal(
        id: FxId,
        parameters: Vec<FxDefinitionParameter>,
        layout: FxDefinitionParameterLayout,
        graph: FxGraph,
        abi_hash: FxAbiHash,
        semantic_hash: FxSemanticHash,
    ) -> Result<Self, FxDefinitionError> {
        let mut encoder = CanonicalEncoder::new(CanonicalLengthSink::default());
        encode_definition_v1(
            &mut encoder,
            &id,
            &parameters,
            &layout,
            &graph,
            abi_hash,
            semantic_hash,
        )?;
        let canonical_len = encoder.into_inner().finish();
        if canonical_len > FX_MAX_DEFINITION_CANONICAL_BYTES {
            return Err(FxDefinitionError::CanonicalTranscriptTooLarge {
                limit: FX_MAX_DEFINITION_CANONICAL_BYTES,
            });
        }
        let canonical_len =
            u32::try_from(canonical_len).map_err(|_| FxDefinitionError::CanonicalLengthOverflow)?;
        Ok(Self {
            id,
            parameters,
            layout,
            graph,
            abi_hash,
            semantic_hash,
            canonical_len,
        })
    }

    pub const fn id(&self) -> &FxId {
        &self.id
    }

    pub fn parameters(&self) -> &[FxDefinitionParameter] {
        &self.parameters
    }

    pub const fn parameter_layout(&self) -> &FxDefinitionParameterLayout {
        &self.layout
    }

    pub const fn graph(&self) -> &FxGraph {
        &self.graph
    }

    pub const fn abi_hash(&self) -> FxAbiHash {
        self.abi_hash
    }

    pub const fn semantic_hash(&self) -> FxSemanticHash {
        self.semantic_hash
    }

    pub const fn canonical_v1_len(&self) -> u32 {
        self.canonical_len
    }

    pub fn canonical_v1_bytes(&self) -> Result<Vec<u8>, FxDefinitionError> {
        let mut output = Vec::new();
        self.append_canonical_v1_bytes(&mut output)?;
        Ok(output)
    }

    pub fn append_canonical_v1_bytes(&self, output: &mut Vec<u8>) -> Result<(), FxDefinitionError> {
        let expected = usize::try_from(self.canonical_len)
            .map_err(|_| FxDefinitionError::CanonicalLengthOverflow)?;
        let sink = CanonicalVecSink::with_preflight(output, expected)?;
        let mut encoder = CanonicalEncoder::new(sink);
        encode_definition_v1(
            &mut encoder,
            &self.id,
            &self.parameters,
            &self.layout,
            &self.graph,
            self.abi_hash,
            self.semantic_hash,
        )?;
        encoder.into_inner().finish()?;
        Ok(())
    }

    pub fn decode_canonical_v1(bytes: &[u8]) -> Result<Self, FxDefinitionDecodeError> {
        if bytes.len() > FX_MAX_DEFINITION_CANONICAL_BYTES {
            return Err(FxDefinitionDecodeError::OwnerLimit {
                kind: "canonical bytes",
                actual: u64::try_from(bytes.len())
                    .map_err(|_| FxCanonicalDecodeError::LengthOverflow)?,
                limit: FX_MAX_DEFINITION_CANONICAL_BYTES,
            });
        }
        let mut reader = CanonicalReader::new(bytes);
        reader.domain_v1(b"arcweft.fx-definition")?;
        let id = FxId::decode_canonical_v1(&mut reader)?;
        let parameter_count =
            decode_count(&mut reader, FX_MAX_PARAMETERS_PER_DEFINITION, "parameter")?;
        let mut parameters = Vec::new();
        parameters
            .try_reserve_exact(parameter_count)
            .map_err(|_| FxDefinitionDecodeError::AllocationFailed { kind: "parameters" })?;
        for ordinal in 0..parameter_count {
            let index = decode_u16(&mut reader, "parameter index")?;
            if usize::from(index) != ordinal {
                return Err(FxDefinitionDecodeError::NonCanonicalOrder { kind: "parameter" });
            }
            let name = decode_bounded_string(
                &mut reader,
                FX_MAX_DEFINITION_PARAMETER_NAME_BYTES,
                "parameter name",
            )?;
            let parameter_type = decode_definition_parameter_type(&mut reader)?;
            let default = if reader.boolean()? {
                Some(decode_definition_argument(&mut reader)?)
            } else {
                None
            };
            parameters.push(FxDefinitionParameter::try_new(
                ordinal,
                name,
                parameter_type,
                default,
            )?);
        }
        let graph = decode_graph(&mut reader)?;
        let layout_digest = FxDefinitionParameterLayoutDigest(reader.digest32()?);
        let abi_hash = FxAbiHash::from_bytes(reader.digest32()?);
        let semantic_hash = FxSemanticHash::from_bytes(reader.digest32()?);
        reader.finish()?;

        let definition = Self::new(id, parameters, graph)?;
        if definition.parameter_layout().digest() != layout_digest {
            return Err(FxDefinitionDecodeError::LayoutDigestMismatch);
        }
        if definition.abi_hash() != abi_hash {
            return Err(FxDefinitionDecodeError::AbiHashMismatch);
        }
        if definition.semantic_hash() != semantic_hash {
            return Err(FxDefinitionDecodeError::SemanticHashMismatch);
        }
        if usize::try_from(definition.canonical_v1_len())
            .map_err(|_| FxCanonicalDecodeError::LengthOverflow)?
            != bytes.len()
        {
            return Err(FxDefinitionDecodeError::NonCanonicalOrder {
                kind: "definition byte length",
            });
        }
        Ok(definition)
    }
}

impl FxDefinitionParameterSchema {
    pub fn new(
        id: FxId,
        parameters: Vec<FxDefinitionParameter>,
    ) -> Result<Self, FxDefinitionError> {
        let layout = validate_definition_parameters(&parameters)?;
        let digest = derive_parameter_schema_digest(&id, &parameters, &layout);
        let mut encoder = CanonicalEncoder::new(CanonicalLengthSink::default());
        encode_parameter_schema_v1(&mut encoder, &id, &parameters, &layout, digest)?;
        let canonical_len = encoder.into_inner().finish();
        if canonical_len > FX_MAX_DEFINITION_CANONICAL_BYTES {
            return Err(FxDefinitionError::CanonicalTranscriptTooLarge {
                limit: FX_MAX_DEFINITION_CANONICAL_BYTES,
            });
        }
        let canonical_len =
            u32::try_from(canonical_len).map_err(|_| FxDefinitionError::CanonicalLengthOverflow)?;
        Ok(Self {
            id,
            parameters,
            layout,
            digest,
            canonical_len,
        })
    }

    pub const fn id(&self) -> &FxId {
        &self.id
    }
    pub fn parameters(&self) -> &[FxDefinitionParameter] {
        &self.parameters
    }
    pub const fn parameter_layout(&self) -> &FxDefinitionParameterLayout {
        &self.layout
    }
    pub const fn digest(&self) -> FxDefinitionParameterSchemaDigest {
        self.digest
    }
    pub const fn canonical_v1_len(&self) -> u32 {
        self.canonical_len
    }

    pub fn canonical_v1_bytes(&self) -> Result<Vec<u8>, FxDefinitionError> {
        let expected = usize::try_from(self.canonical_len)
            .map_err(|_| FxDefinitionError::CanonicalLengthOverflow)?;
        let mut output = Vec::new();
        let sink = CanonicalVecSink::with_preflight(&mut output, expected)?;
        let mut encoder = CanonicalEncoder::new(sink);
        encode_parameter_schema_v1(
            &mut encoder,
            &self.id,
            &self.parameters,
            &self.layout,
            self.digest,
        )?;
        encoder.into_inner().finish()?;
        Ok(output)
    }

    pub fn decode_canonical_v1(bytes: &[u8]) -> Result<Self, FxDefinitionDecodeError> {
        if bytes.len() > FX_MAX_DEFINITION_CANONICAL_BYTES {
            return Err(FxDefinitionDecodeError::OwnerLimit {
                kind: "canonical bytes",
                actual: u64::try_from(bytes.len())
                    .map_err(|_| FxCanonicalDecodeError::LengthOverflow)?,
                limit: FX_MAX_DEFINITION_CANONICAL_BYTES,
            });
        }
        let mut reader = CanonicalReader::new(bytes);
        reader.domain_v1(b"arcweft.fx-definition-parameter-schema")?;
        let id = FxId::decode_canonical_v1(&mut reader)?;
        let parameters = decode_definition_parameters(&mut reader)?;
        let layout_digest = FxDefinitionParameterLayoutDigest(reader.digest32()?);
        let digest = FxDefinitionParameterSchemaDigest(reader.digest32()?);
        reader.finish()?;
        let schema = Self::new(id, parameters)?;
        if schema.parameter_layout().digest() != layout_digest {
            return Err(FxDefinitionDecodeError::LayoutDigestMismatch);
        }
        if schema.digest() != digest {
            return Err(FxDefinitionDecodeError::ParameterSchemaDigestMismatch);
        }
        Ok(schema)
    }

    fn into_parts(
        self,
    ) -> (
        FxId,
        Vec<FxDefinitionParameter>,
        FxDefinitionParameterLayout,
    ) {
        (self.id, self.parameters, self.layout)
    }
}

impl<'a> FxDefinitionGraphContext<'a> {
    /// Creates a graph context over one already-validated outer parameter
    /// schema.
    #[must_use]
    pub const fn new(schema: &'a FxDefinitionParameterSchema) -> Self {
        Self { schema }
    }

    /// Returns the outer parameter schema used by this context.
    #[must_use]
    pub const fn parameter_schema(self) -> &'a FxDefinitionParameterSchema {
        self.schema
    }

    /// Returns the outer definition identity carried by the schema.
    #[must_use]
    pub const fn definition_id(self) -> &'a FxId {
        self.schema.id()
    }

    /// Resolves one outer ABI parameter index to its owner-issued reference.
    #[must_use]
    pub fn parameter_ref(
        self,
        index: FxDefinitionParameterIndex,
    ) -> Option<FxDefinitionParameterRef> {
        self.schema.parameter_layout().parameter_ref(index)
    }

    /// Resolves one outer ABI parameter index to its dense runtime reference.
    #[must_use]
    pub fn runtime_parameter_ref(
        self,
        index: FxDefinitionParameterIndex,
    ) -> Option<FxRuntimeParameterRef> {
        self.schema.parameter_layout().runtime_ref(index)
    }

    /// Returns the complete dense runtime type schema for embedded programs.
    pub fn runtime_types(self) -> impl Iterator<Item = FxRuntimeType> + 'a {
        self.schema
            .parameter_layout()
            .runtime_rows()
            .iter()
            .map(|row| row.reference().runtime_type())
    }

    /// Validates that a symbolic parameter reference belongs to this outer
    /// schema and carries the schema-issued type.
    pub(crate) fn validate_parameter_ref(
        self,
        reference: FxDefinitionParameterRef,
    ) -> Result<FxDefinitionParameterRef, FxDefinitionError> {
        let Some(expected) = self.parameter_ref(reference.index()) else {
            return Err(FxDefinitionError::ParameterReferenceOutOfBounds {
                index: reference.index().get(),
                available: self.schema.parameter_layout().abi_rows().len(),
            });
        };
        if expected.parameter_type() != reference.parameter_type() {
            return Err(FxDefinitionError::ParameterReferenceType {
                index: reference.index().get(),
                expected: expected.parameter_type(),
                actual: reference.parameter_type(),
            });
        }
        Ok(expected)
    }

    /// Validates a graph fragment against the outer schema's graph and
    /// runtime-layout invariants.
    pub(crate) fn validate_graph(self, graph: &FxGraph) -> Result<(), FxDefinitionError> {
        validate_definition_graph(graph, self.schema.parameter_layout())
    }
}

impl FxDefinitionParameterSchemaDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub const fn to_bytes(self) -> [u8; 32] {
        self.0
    }
}

fn derive_parameter_schema_digest(
    id: &FxId,
    parameters: &[FxDefinitionParameter],
    layout: &FxDefinitionParameterLayout,
) -> FxDefinitionParameterSchemaDigest {
    let mut hasher = blake3::Hasher::new();
    {
        let mut encoder = CanonicalEncoder::new(CanonicalHashSink::new(&mut hasher));
        let result = (|| {
            encoder.domain_v1(b"arcweft.fx-definition-parameter-schema-digest")?;
            id.encode_canonical_v1(&mut encoder)?;
            encode_definition_parameters(&mut encoder, parameters)?;
            encoder.digest32(layout.digest().as_bytes())
        })();
        match result {
            Ok(()) => {}
            Err(error) => match error {},
        }
    }
    FxDefinitionParameterSchemaDigest(*hasher.finalize().as_bytes())
}

fn encode_parameter_schema_v1<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    id: &FxId,
    parameters: &[FxDefinitionParameter],
    layout: &FxDefinitionParameterLayout,
    digest: FxDefinitionParameterSchemaDigest,
) -> Result<(), S::Error> {
    encoder.domain_v1(b"arcweft.fx-definition-parameter-schema")?;
    id.encode_canonical_v1(encoder)?;
    encode_definition_parameters(encoder, parameters)?;
    encoder.digest32(layout.digest().as_bytes())?;
    encoder.digest32(digest.as_bytes())
}

fn encode_definition_parameters<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    parameters: &[FxDefinitionParameter],
) -> Result<(), S::Error> {
    let parameter_count = parameters.iter().fold(0_u64, |count, _| count + 1);
    encoder.unsigned(parameter_count)?;
    for parameter in parameters {
        encoder.unsigned(u64::from(parameter.index.get()))?;
        encoder.unsigned(u64::from(parameter.name.byte_len()))?;
        encoder.raw_bytes(parameter.name.as_str().as_bytes())?;
        encode_definition_parameter_type(encoder, parameter.ty)?;
        match &parameter.default {
            Some(default) => {
                encoder.boolean(true)?;
                encode_definition_argument(encoder, default)?;
            }
            None => encoder.boolean(false)?,
        }
    }
    Ok(())
}

fn decode_definition_parameters(
    reader: &mut CanonicalReader<'_>,
) -> Result<Vec<FxDefinitionParameter>, FxDefinitionDecodeError> {
    let parameter_count = decode_count(reader, FX_MAX_PARAMETERS_PER_DEFINITION, "parameter")?;
    let mut parameters = Vec::new();
    parameters
        .try_reserve_exact(parameter_count)
        .map_err(|_| FxDefinitionDecodeError::AllocationFailed { kind: "parameters" })?;
    for ordinal in 0..parameter_count {
        let index = decode_u16(reader, "parameter index")?;
        if usize::from(index) != ordinal {
            return Err(FxDefinitionDecodeError::NonCanonicalOrder { kind: "parameter" });
        }
        let name = decode_bounded_string(
            reader,
            FX_MAX_DEFINITION_PARAMETER_NAME_BYTES,
            "parameter name",
        )?;
        let parameter_type = decode_definition_parameter_type(reader)?;
        let default = if reader.boolean()? {
            Some(decode_definition_argument(reader)?)
        } else {
            None
        };
        parameters.push(FxDefinitionParameter::try_new(
            ordinal,
            name,
            parameter_type,
            default,
        )?);
    }
    Ok(parameters)
}

impl FxAbiHash {
    pub fn for_definition(
        parameters: &[FxDefinitionParameter],
        layout: &FxDefinitionParameterLayout,
        graph: &FxGraph,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        {
            let mut encoder = CanonicalEncoder::new(CanonicalHashSink::new(&mut hasher));
            let result: Result<(), std::convert::Infallible> = (|| {
                encoder.domain_v1(b"arcweft.fx-abi")?;
                encoder.unsigned(u64::from(layout.abi_count()))?;
                for parameter in parameters {
                    encoder.unsigned(u64::from(parameter.index.get()))?;
                    encoder.unsigned(u64::from(parameter.name.byte_len()))?;
                    encoder.raw_bytes(parameter.name.as_str().as_bytes())?;
                    encode_definition_parameter_type(&mut encoder, parameter.ty)?;
                    match &parameter.default {
                        Some(value) => {
                            encoder.boolean(true)?;
                            encode_definition_argument(&mut encoder, value)?;
                        }
                        None => {
                            encoder.boolean(false)?;
                        }
                    }
                }
                encoder.digest32(&layout.digest.0)?;
                let interfaces = graph.renderer_interfaces();
                let interface_count = interfaces.iter().fold(0_u64, |count, _| count + 1);
                encoder.unsigned(interface_count)?;
                for interface in interfaces.iter() {
                    encoder.tag(interface.semantic_tag())?;
                }
                let mut schemas = [false; FX_PROPERTY_REQUIREMENT_CAPACITY];
                collect_property_schemas(graph, &mut schemas);
                let schema_count = schemas
                    .iter()
                    .filter(|present| **present)
                    .fold(0_u64, |count, _| count + 1);
                encoder.unsigned(schema_count)?;
                for interface in FxRendererInterface::ALL {
                    for property in FxPropertyId::ALL {
                        let index = usize::from(interface.semantic_tag()) * FxPropertyId::ALL.len()
                            + usize::from(property.semantic_tag());
                        if schemas[index] {
                            encoder.tag(interface.semantic_tag())?;
                            encoder.tag(property.semantic_tag())?;
                            encode_static_type(&mut encoder, property.value_type())?;
                        }
                    }
                }
                Ok(())
            })();
            match result {
                Ok(()) => {}
                Err(error) => match error {},
            }
        }
        Self::from_bytes(*hasher.finalize().as_bytes())
    }
}

impl FxSemanticHash {
    pub fn for_graph(graph: &FxGraph) -> Self {
        let mut hasher = blake3::Hasher::new();
        {
            let mut encoder = CanonicalEncoder::new(CanonicalHashSink::new(&mut hasher));
            let result = (|| {
                encoder.domain_v1(b"arcweft.fx-semantic")?;
                encode_graph(&mut encoder, graph)
            })();
            match result {
                Ok(()) => {}
                Err(error) => match error {},
            }
        }
        Self::from_bytes(*hasher.finalize().as_bytes())
    }
}

const FX_PROPERTY_REQUIREMENT_CAPACITY: usize = 10 * 26;

fn collect_property_schemas(
    graph: &FxGraph,
    output: &mut [bool; FX_PROPERTY_REQUIREMENT_CAPACITY],
) {
    for node in &graph.nodes {
        if let (Some(interface), Some(properties)) = (node.renderer_interface(), node.properties())
        {
            for property in properties {
                let index = usize::from(interface.semantic_tag()) * FxPropertyId::ALL.len()
                    + usize::from(property.id().semantic_tag());
                output[index] = true;
            }
        }
        match node {
            FxNode::Conditional {
                then_graph,
                else_graph,
                ..
            } => {
                collect_property_schemas(then_graph, output);
                collect_property_schemas(else_graph, output);
            }
            FxNode::Stack { children } => {
                for child in children {
                    collect_property_schemas(child, output);
                }
            }
            _ => {}
        }
    }
}

fn encode_definition_v1<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    id: &FxId,
    parameters: &[FxDefinitionParameter],
    layout: &FxDefinitionParameterLayout,
    graph: &FxGraph,
    abi_hash: FxAbiHash,
    semantic_hash: FxSemanticHash,
) -> Result<(), S::Error> {
    encoder.domain_v1(b"arcweft.fx-definition")?;
    id.encode_canonical_v1(encoder)?;
    encoder.unsigned(u64::from(layout.abi_count()))?;
    for parameter in parameters {
        encoder.unsigned(u64::from(parameter.index.get()))?;
        encoder.unsigned(u64::from(parameter.name.byte_len()))?;
        encoder.raw_bytes(parameter.name.as_str().as_bytes())?;
        encode_definition_parameter_type(encoder, parameter.ty)?;
        match &parameter.default {
            Some(default) => {
                encoder.boolean(true)?;
                encode_definition_argument(encoder, default)?;
            }
            None => encoder.boolean(false)?,
        }
    }
    encode_graph(encoder, graph)?;
    encoder.digest32(layout.digest.as_bytes())?;
    encoder.digest32(abi_hash.as_bytes())?;
    encoder.digest32(semantic_hash.as_bytes())
}

fn encode_graph<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    graph: &FxGraph,
) -> Result<(), S::Error> {
    encoder.unsigned(u64::from(graph.node_count()))?;
    for node in &graph.nodes {
        encoder.tag(node.node_kind().semantic_tag())?;
        if let Some(interface) = node.renderer_interface() {
            encoder.tag(interface as u8)?;
        }
        if let Some(properties) = node.properties() {
            encode_properties(encoder, properties)?;
        }
        match node {
            FxNode::Transform { fx, .. }
            | FxNode::Mask { fx, .. }
            | FxNode::Filter { fx, .. }
            | FxNode::Shader { fx, .. }
            | FxNode::OffscreenPass { fx, .. }
            | FxNode::PostProcess { fx, .. }
            | FxNode::Transition { fx, .. } => {
                fx.encode_canonical_v1(encoder)?;
            }
            FxNode::Conditional {
                condition,
                then_graph,
                else_graph,
            } => {
                encode_static_value(encoder, condition)?;
                encode_graph(encoder, then_graph)?;
                encode_graph(encoder, else_graph)?;
            }
            FxNode::Stack { children } => {
                let child_count = children.iter().fold(0_u64, |count, _| count + 1);
                encoder.unsigned(child_count)?;
                for child in children {
                    encode_graph(encoder, child)?;
                }
            }
            FxNode::Style { .. } | FxNode::Text { .. } | FxNode::Color { .. } => {}
        }
    }
    Ok(())
}

fn encode_properties<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    properties: &[FxProperty],
) -> Result<(), S::Error> {
    let property_count = properties.iter().fold(0_u64, |count, _| count + 1);
    encoder.unsigned(property_count)?;
    for property in properties {
        encoder.tag(property.id.semantic_tag())?;
        encode_static_value(encoder, &property.value)?;
    }
    Ok(())
}

fn encode_static_type<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    value: FxStaticType,
) -> Result<(), S::Error> {
    match value {
        FxStaticType::Runtime(runtime) => {
            encoder.tag(0)?;
            encoder.tag(runtime as u8)?;
        }
        FxStaticType::Resource => encoder.tag(1)?,
        FxStaticType::Selector(domain) => {
            encoder.tag(2)?;
            encoder.tag(domain as u8)?;
        }
        FxStaticType::ShaderStage => encoder.tag(3)?,
        FxStaticType::FontFamily => encoder.tag(4)?,
        FxStaticType::Target => encoder.tag(5)?,
        FxStaticType::Phase => encoder.tag(6)?,
        FxStaticType::UniformRecord => encoder.tag(7)?,
    }
    Ok(())
}

fn encode_definition_parameter_type<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    value: FxDefinitionParameterType,
) -> Result<(), S::Error> {
    match value {
        FxDefinitionParameterType::Runtime(ty) => {
            encoder.tag(0)?;
            encoder.tag(ty as u8)?;
        }
        FxDefinitionParameterType::Resource => encoder.tag(1)?,
        FxDefinitionParameterType::UniformRecord => encoder.tag(2)?,
    }
    Ok(())
}

fn encode_definition_argument<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    value: &FxDefinitionArgumentValue,
) -> Result<(), S::Error> {
    match value {
        FxDefinitionArgumentValue::Runtime(value) => {
            encoder.tag(0)?;
            value.encode_canonical_v1(encoder)?;
        }
        FxDefinitionArgumentValue::Resource(value) => {
            encoder.tag(1)?;
            encoder.unsigned(u64::from(value.byte_len()))?;
            encoder.raw_bytes(value.as_str().as_bytes())?;
        }
        FxDefinitionArgumentValue::UniformRecord(value) => {
            encoder.tag(2)?;
            encoder.unsigned(value.canonical_len())?;
            value.encode_canonical_v1(encoder)?;
        }
    }
    Ok(())
}

pub(super) fn definition_argument_v1_bytes(
    value: &FxDefinitionArgumentValue,
) -> Result<Vec<u8>, FxDefinitionError> {
    let mut counter = CanonicalEncoder::new(CanonicalLengthSink::default());
    counter.domain_v1(b"arcweft.fx-definition-argument")?;
    encode_definition_argument(&mut counter, value)?;
    let length = counter.into_inner().finish();
    if length > FX_MAX_DEFINITION_CANONICAL_BYTES {
        return Err(FxDefinitionError::CanonicalTranscriptTooLarge {
            limit: FX_MAX_DEFINITION_CANONICAL_BYTES,
        });
    }
    let mut output = Vec::new();
    let sink = CanonicalVecSink::with_preflight(&mut output, length)?;
    let mut encoder = CanonicalEncoder::new(sink);
    encoder.domain_v1(b"arcweft.fx-definition-argument")?;
    encode_definition_argument(&mut encoder, value)?;
    encoder.into_inner().finish()?;
    Ok(output)
}

fn encode_static_value<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    value: &FxStaticValue,
) -> Result<(), S::Error> {
    match value {
        FxStaticValue::Runtime(value) => {
            encoder.tag(0)?;
            value.encode_canonical_v1(encoder)?;
        }
        FxStaticValue::Resource(value) => {
            encoder.tag(1)?;
            encoder.unsigned(u64::from(value.byte_len()))?;
            encoder.raw_bytes(value.as_str().as_bytes())?;
        }
        FxStaticValue::Selector(value) => {
            encoder.tag(2)?;
            value.encode_canonical_v1(encoder)?;
        }
        FxStaticValue::ShaderStage(value) => {
            encoder.tag(3)?;
            encoder.unsigned(u64::from(value.tag()))?;
        }
        FxStaticValue::FontFamily(value) => {
            encoder.tag(4)?;
            encoder.unsigned(u64::from(value.byte_len()))?;
            encoder.raw_bytes(value.as_str().as_bytes())?;
        }
        FxStaticValue::Target(value) => {
            encoder.tag(5)?;
            encoder.unsigned(u64::from(value.tag()))?;
        }
        FxStaticValue::Phase(value) => {
            encoder.tag(6)?;
            encoder.unsigned(u64::from(value.tag()))?;
        }
        FxStaticValue::Parameter(value) => {
            encoder.tag(7)?;
            encoder.unsigned(u64::from(value.index.get()))?;
            encode_definition_parameter_type(encoder, value.ty)?;
        }
        FxStaticValue::Sampler(value) => {
            encoder.tag(8)?;
            encoder.unsigned(value.canonical_v1_len_u64())?;
            value.encode_canonical_v1(encoder)?;
        }
        FxStaticValue::UniformRecord(record) => {
            encoder.tag(9)?;
            encoder.unsigned(record.canonical_len())?;
            record.encode_canonical_v1(encoder)?;
        }
    }
    Ok(())
}

fn decode_count(
    reader: &mut CanonicalReader<'_>,
    limit: usize,
    kind: &'static str,
) -> Result<usize, FxDefinitionDecodeError> {
    let actual = reader.unsigned()?;
    if actual > u64::try_from(limit).map_err(|_| FxCanonicalDecodeError::LengthOverflow)? {
        return Err(FxDefinitionDecodeError::OwnerLimit {
            kind,
            actual,
            limit,
        });
    }
    usize::try_from(actual).map_err(|_| FxCanonicalDecodeError::LengthOverflow.into())
}

fn decode_u16(
    reader: &mut CanonicalReader<'_>,
    kind: &'static str,
) -> Result<u16, FxDefinitionDecodeError> {
    let value = reader.unsigned()?;
    u16::try_from(value).map_err(|_| FxDefinitionDecodeError::IntegerOutOfRange { kind, value })
}

fn decode_bounded_string(
    reader: &mut CanonicalReader<'_>,
    limit: usize,
    kind: &'static str,
) -> Result<String, FxDefinitionDecodeError> {
    let actual = reader.unsigned()?;
    if actual > u64::try_from(limit).map_err(|_| FxCanonicalDecodeError::LengthOverflow)? {
        return Err(FxDefinitionDecodeError::OwnerLimit {
            kind,
            actual,
            limit,
        });
    }
    let length = usize::try_from(actual).map_err(|_| FxCanonicalDecodeError::LengthOverflow)?;
    let bytes = reader.raw_bytes(length)?;
    let source = std::str::from_utf8(bytes).map_err(|_| FxCanonicalDecodeError::InvalidUtf8)?;
    let mut value = String::new();
    value
        .try_reserve_exact(length)
        .map_err(|_| FxDefinitionDecodeError::AllocationFailed { kind })?;
    value.push_str(source);
    Ok(value)
}

fn decode_definition_parameter_type(
    reader: &mut CanonicalReader<'_>,
) -> Result<FxDefinitionParameterType, FxDefinitionDecodeError> {
    match reader.tag()? {
        0 => Ok(FxDefinitionParameterType::Runtime(
            FxRuntimeType::decode_canonical_tag(reader.tag()?)?,
        )),
        1 => Ok(FxDefinitionParameterType::Resource),
        2 => Ok(FxDefinitionParameterType::UniformRecord),
        tag => Err(FxDefinitionDecodeError::UnknownTag {
            kind: "parameter type",
            tag,
        }),
    }
}

fn decode_definition_argument(
    reader: &mut CanonicalReader<'_>,
) -> Result<FxDefinitionArgumentValue, FxDefinitionDecodeError> {
    match reader.tag()? {
        0 => Ok(FxDefinitionArgumentValue::Runtime(
            FxRuntimeValue::decode_canonical_v1(reader)?,
        )),
        1 => Ok(FxDefinitionArgumentValue::Resource(FxResourceId::try_new(
            decode_bounded_string(reader, FX_MAX_RESOURCE_ID_BYTES, "resource ID")?,
        )?)),
        2 => {
            let length = reader.length()?;
            Ok(FxDefinitionArgumentValue::UniformRecord(
                FxUniformRecord::decode_canonical_v1_reader(reader, length)?,
            ))
        }
        tag => Err(FxDefinitionDecodeError::UnknownTag {
            kind: "definition argument",
            tag,
        }),
    }
}

fn decode_graph(reader: &mut CanonicalReader<'_>) -> Result<FxGraph, FxDefinitionDecodeError> {
    let node_count = decode_count(reader, FX_MAX_GRAPH_NODES_PER_DEFINITION, "graph node")?;
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(node_count)
        .map_err(|_| FxDefinitionDecodeError::AllocationFailed {
            kind: "graph nodes",
        })?;
    for _ in 0..node_count {
        nodes.push(decode_node(reader)?);
    }
    Ok(FxGraph::try_new(nodes)?)
}

fn decode_node(reader: &mut CanonicalReader<'_>) -> Result<FxNode, FxDefinitionDecodeError> {
    let tag = reader.tag()?;
    let kind = FxNodeKind::ALL
        .get(usize::from(tag))
        .copied()
        .ok_or(FxDefinitionDecodeError::UnknownTag { kind: "node", tag })?;
    if let Some(expected) = kind.renderer_interface() {
        let actual = reader.tag()?;
        if actual != expected.semantic_tag() {
            return Err(FxDefinitionDecodeError::UnknownTag {
                kind: "renderer interface",
                tag: actual,
            });
        }
    }
    Ok(match kind {
        FxNodeKind::Style => FxNode::Style {
            properties: decode_properties(reader, kind)?,
        },
        FxNodeKind::Text => FxNode::Text {
            properties: decode_properties(reader, kind)?,
        },
        FxNodeKind::Color => FxNode::Color {
            properties: decode_properties(reader, kind)?,
        },
        FxNodeKind::Transform => FxNode::Transform {
            properties: decode_properties(reader, kind)?,
            fx: FxId::decode_canonical_v1(reader)?,
        },
        FxNodeKind::Mask => FxNode::Mask {
            properties: decode_properties(reader, kind)?,
            fx: FxId::decode_canonical_v1(reader)?,
        },
        FxNodeKind::Filter => FxNode::Filter {
            properties: decode_properties(reader, kind)?,
            fx: FxId::decode_canonical_v1(reader)?,
        },
        FxNodeKind::Shader => FxNode::Shader {
            properties: decode_properties(reader, kind)?,
            fx: FxId::decode_canonical_v1(reader)?,
        },
        FxNodeKind::OffscreenPass => FxNode::OffscreenPass {
            properties: decode_properties(reader, kind)?,
            fx: FxId::decode_canonical_v1(reader)?,
        },
        FxNodeKind::PostProcess => FxNode::PostProcess {
            properties: decode_properties(reader, kind)?,
            fx: FxId::decode_canonical_v1(reader)?,
        },
        FxNodeKind::Transition => FxNode::Transition {
            properties: decode_properties(reader, kind)?,
            fx: FxId::decode_canonical_v1(reader)?,
        },
        FxNodeKind::Conditional => FxNode::Conditional {
            condition: decode_static_value(reader)?,
            then_graph: decode_graph(reader)?,
            else_graph: decode_graph(reader)?,
        },
        FxNodeKind::Stack => {
            let child_count = decode_count(
                reader,
                FX_MAX_GRAPH_CHILD_EDGES_PER_DEFINITION,
                "stack child",
            )?;
            let mut children = Vec::new();
            children.try_reserve_exact(child_count).map_err(|_| {
                FxDefinitionDecodeError::AllocationFailed {
                    kind: "stack children",
                }
            })?;
            for _ in 0..child_count {
                children.push(decode_graph(reader)?);
            }
            FxNode::Stack { children }
        }
    })
}

fn decode_properties(
    reader: &mut CanonicalReader<'_>,
    node: FxNodeKind,
) -> Result<Vec<FxProperty>, FxDefinitionDecodeError> {
    let count = decode_count(reader, node.property_capacity(), "property")?;
    let mut properties = Vec::new();
    properties
        .try_reserve_exact(count)
        .map_err(|_| FxDefinitionDecodeError::AllocationFailed { kind: "properties" })?;
    let mut prior = None;
    for _ in 0..count {
        let tag = reader.tag()?;
        if prior.is_some_and(|prior| tag <= prior) {
            return Err(FxDefinitionDecodeError::NonCanonicalOrder { kind: "property" });
        }
        prior = Some(tag);
        let id = FxPropertyId::ALL.get(usize::from(tag)).copied().ok_or(
            FxDefinitionDecodeError::UnknownTag {
                kind: "property",
                tag,
            },
        )?;
        properties.push(FxProperty::new(id, decode_static_value(reader)?));
    }
    Ok(properties)
}

fn decode_static_value(
    reader: &mut CanonicalReader<'_>,
) -> Result<FxStaticValue, FxDefinitionDecodeError> {
    Ok(match reader.tag()? {
        0 => FxStaticValue::Runtime(FxRuntimeValue::decode_canonical_v1(reader)?),
        1 => FxStaticValue::Resource(FxResourceId::try_new(decode_bounded_string(
            reader,
            FX_MAX_RESOURCE_ID_BYTES,
            "resource ID",
        )?)?),
        2 => {
            let domain = match reader.tag()? {
                0 => FxSelectorDomain::TransitionKind,
                1 => FxSelectorDomain::TransitionEasing,
                tag => {
                    return Err(FxDefinitionDecodeError::UnknownTag {
                        kind: "selector domain",
                        tag,
                    });
                }
            };
            let name = decode_bounded_string(reader, FX_MAX_SELECTOR_NAME_BYTES, "selector name")?;
            FxStaticValue::Selector(FxSelectorId::try_new(domain, name)?)
        }
        3 => {
            let value = reader.unsigned()?;
            let stage = match value {
                0 => FxShaderStage::GlyphColor,
                1 => FxShaderStage::OffscreenPass,
                2 => FxShaderStage::PostProcess,
                _ => {
                    return Err(FxDefinitionDecodeError::IntegerOutOfRange {
                        kind: "shader stage",
                        value,
                    });
                }
            };
            FxStaticValue::ShaderStage(stage)
        }
        4 => FxStaticValue::FontFamily(FxFontFamilyName::try_new(decode_bounded_string(
            reader,
            FX_MAX_FONT_FAMILY_NAME_BYTES,
            "font-family name",
        )?)?),
        5 => {
            let value = reader.unsigned()?;
            FxStaticValue::Target(match value {
                0 => FxTarget::Node,
                1 => FxTarget::Content,
                2 => FxTarget::Background,
                3 => FxTarget::Line,
                4 => FxTarget::Glyph,
                5 => FxTarget::Viewport,
                _ => {
                    return Err(FxDefinitionDecodeError::IntegerOutOfRange {
                        kind: "target",
                        value,
                    });
                }
            })
        }
        6 => {
            let value = reader.unsigned()?;
            FxStaticValue::Phase(match value {
                0 => FxPhase::BeforeLayout,
                1 => FxPhase::LayoutTransform,
                2 => FxPhase::GlyphTransform,
                3 => FxPhase::GlyphColor,
                4 => FxPhase::GlyphMask,
                5 => FxPhase::OffscreenPass,
                6 => FxPhase::PostProcess,
                7 => FxPhase::Transition,
                _ => {
                    return Err(FxDefinitionDecodeError::IntegerOutOfRange {
                        kind: "phase",
                        value,
                    });
                }
            })
        }
        7 => FxStaticValue::Parameter(FxDefinitionParameterRef {
            index: FxDefinitionParameterIndex(decode_u16(reader, "parameter index")?),
            ty: decode_definition_parameter_type(reader)?,
        }),
        8 => {
            let length = reader.length()?;
            FxStaticValue::Sampler(FxSamplerProgram::decode_canonical_v1_reader(
                reader, length,
            )?)
        }
        9 => {
            let length = reader.length()?;
            FxStaticValue::UniformRecord(FxUniformRecord::decode_canonical_v1_reader(
                reader, length,
            )?)
        }
        tag => {
            return Err(FxDefinitionDecodeError::UnknownTag {
                kind: "static value",
                tag,
            });
        }
    })
}
