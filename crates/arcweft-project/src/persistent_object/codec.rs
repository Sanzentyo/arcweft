use super::payload::{
    BytecodeUnitObject, CompilerObjectPayload, HirBodyFactsObject, HirBodyObject,
    InterfaceSummaryObject, LineTaskEvidenceObject, LinkPlanObject, ParsedSyntaxEvidenceObject,
    ParsedSyntaxObject, PublicSymbolKind, PublicSymbolObject, RuntimePlanUnitObject,
    StableDiagnosticObject, StableDiagnosticSeverity, StableDiagnosticSummaryObject,
    StableRangeObject, StableSourceSpanObject, SyntaxStatsObject, TypecheckGateFactsObject,
    TypecheckGateObject, TypecheckGateReusePolicy,
};
use super::schema::{
    AWBO_MAGIC, AWBO_SCHEMA_VERSION, AwboError, CompilerBuildIdentity,
    CompilerIdentityNamespaceObject, CompilerObjectKey, CompilerObjectKind,
    CompilerObjectStability, CompilerStageInputsObject,
};
use crate::fingerprint::{BuildDigest, NamedDigest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// `.awbo` object envelope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AwboEnvelope {
    pub magic: [u8; 8],
    pub schema_version: u32,
    pub kind: CompilerObjectKind,
    pub stability: CompilerObjectStability,
    pub key_digest: BuildDigest,
    pub payload_digest: BuildDigest,
    pub payload_len: u64,
    pub payload: CompilerObjectPayload,
}

impl AwboEnvelope {
    pub fn new(key: &CompilerObjectKey, payload: CompilerObjectPayload) -> Result<Self, AwboError> {
        let payload_kind = payload.kind();
        if key.kind != payload_kind {
            return Err(AwboError::KindMismatch {
                key: key.kind,
                payload: payload_kind,
            });
        }
        payload.validate_contract_for_key(key)?;
        let payload_bytes = payload.encode_payload_bytes()?;
        let payload_len = encoded_len("payload", payload_bytes.len())?;
        Ok(Self {
            magic: AWBO_MAGIC,
            schema_version: AWBO_SCHEMA_VERSION,
            kind: key.kind,
            stability: key.kind.stability(),
            key_digest: key.digest(),
            payload_digest: BuildDigest::of(&payload_bytes),
            payload_len,
            payload,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, AwboError> {
        self.validate_envelope_shape()?;
        let payload_bytes = self.payload.encode_payload_bytes()?;
        let actual = encoded_len("payload", payload_bytes.len())?;
        if self.payload_len != actual {
            return Err(AwboError::PayloadLengthMismatch {
                expected: self.payload_len,
                actual,
            });
        }
        if self.payload_digest != BuildDigest::of(&payload_bytes) {
            return Err(AwboError::PayloadDigestMismatch);
        }

        let mut writer = BinaryWriter::default();
        writer.put_bytes_raw(&self.magic);
        writer.put_u32(self.schema_version);
        writer.put_u8(self.kind.wire_tag());
        writer.put_u8(self.stability.wire_tag());
        writer.put_digest(self.key_digest);
        writer.put_digest(self.payload_digest);
        writer.put_u64(self.payload_len);
        writer.put_bytes_raw(&payload_bytes);
        Ok(writer.finish())
    }

    pub fn decode_detached(bytes: &[u8]) -> Result<Self, AwboError> {
        let mut reader = BinaryReader::new(bytes);
        let magic = reader.read_array::<8>("magic")?;
        let schema_version = reader.read_u32("schema_version")?;
        let kind = CompilerObjectKind::from_wire_tag(reader.read_u8("kind")?)?;
        let stability = CompilerObjectStability::from_wire_tag(reader.read_u8("stability")?)?;
        let key_digest = reader.read_digest("key_digest")?;
        let payload_digest = reader.read_digest("payload_digest")?;
        let payload_len = reader.read_u64("payload_len")?;
        let payload_bytes = reader.read_exact_len(payload_len, "payload")?;
        reader.finish()?;

        let payload = CompilerObjectPayload::decode_payload_bytes(kind, payload_bytes)?;
        let envelope = Self {
            magic,
            schema_version,
            kind,
            stability,
            key_digest,
            payload_digest,
            payload_len,
            payload,
        };
        envelope.validate_envelope_shape()?;
        Ok(envelope)
    }

    pub fn decode(bytes: &[u8], key: &CompilerObjectKey) -> Result<Self, AwboError> {
        let envelope = Self::decode_detached(bytes)?;
        envelope.validate(key)?;
        Ok(envelope)
    }

    pub fn validate(&self, key: &CompilerObjectKey) -> Result<(), AwboError> {
        self.validate_envelope_shape()?;
        let payload_kind = self.payload.kind();
        if self.kind != key.kind || payload_kind != key.kind {
            return Err(AwboError::KindMismatch {
                key: key.kind,
                payload: payload_kind,
            });
        }
        if self.key_digest != key.digest() {
            return Err(AwboError::KeyDigestMismatch);
        }
        self.payload.validate_contract_for_key(key)
    }

    fn validate_envelope_shape(&self) -> Result<(), AwboError> {
        if self.magic != AWBO_MAGIC {
            return Err(AwboError::BadMagic);
        }
        if self.schema_version != AWBO_SCHEMA_VERSION {
            return Err(AwboError::UnsupportedSchema {
                actual: self.schema_version,
                expected: AWBO_SCHEMA_VERSION,
            });
        }
        let payload_kind = self.payload.kind();
        if self.kind != payload_kind {
            return Err(AwboError::KindMismatch {
                key: self.kind,
                payload: payload_kind,
            });
        }
        let expected_stability = self.kind.stability();
        if self.stability != expected_stability {
            return Err(AwboError::StabilityMismatch {
                kind: self.kind,
                actual: self.stability,
                expected: expected_stability,
            });
        }
        let payload_bytes = self.payload.encode_payload_bytes()?;
        let actual_len = encoded_len("payload", payload_bytes.len())?;
        if self.payload_len != actual_len {
            return Err(AwboError::PayloadLengthMismatch {
                expected: self.payload_len,
                actual: actual_len,
            });
        }
        if self.payload_digest != BuildDigest::of(&payload_bytes) {
            return Err(AwboError::PayloadDigestMismatch);
        }
        Ok(())
    }
}

impl CompilerObjectPayload {
    pub fn encode_payload_bytes(&self) -> Result<Vec<u8>, AwboError> {
        let mut writer = BinaryWriter::default();
        writer.put_string("payload.kind", self.kind().cache_namespace())?;
        match self {
            Self::ParsedSyntax(value) => put_parsed_syntax(&mut writer, value)?,
            Self::InterfaceSummary(value) => put_interface_summary(&mut writer, value)?,
            Self::HirBody(value) => put_hir_body(&mut writer, value)?,
            Self::TypecheckGate(value) => put_typecheck_gate(&mut writer, value)?,
            Self::LineTaskEvidence(value) => put_line_task_evidence(&mut writer, value)?,
            Self::RuntimePlanUnit(value) => put_runtime_plan_unit(&mut writer, value)?,
            Self::BytecodeUnit(value) => put_bytecode_unit(&mut writer, value)?,
            Self::LinkPlan(value) => put_link_plan(&mut writer, value)?,
        }
        Ok(writer.finish())
    }

    pub fn decode_payload_bytes(kind: CompilerObjectKind, bytes: &[u8]) -> Result<Self, AwboError> {
        let mut reader = BinaryReader::new(bytes);
        let namespace = reader.read_string("payload.kind")?;
        if namespace != kind.cache_namespace() {
            return Err(AwboError::KindMismatch {
                key: kind,
                payload: CompilerObjectKind::from_cache_namespace(&namespace)?,
            });
        }
        let payload = match kind {
            CompilerObjectKind::ParsedSyntax => {
                Self::ParsedSyntax(read_parsed_syntax(&mut reader)?)
            }
            CompilerObjectKind::InterfaceSummary => {
                Self::InterfaceSummary(read_interface_summary(&mut reader)?)
            }
            CompilerObjectKind::HirBody => Self::HirBody(read_hir_body(&mut reader)?),
            CompilerObjectKind::TypecheckGate => {
                Self::TypecheckGate(read_typecheck_gate(&mut reader)?)
            }
            CompilerObjectKind::LineTaskEvidence => {
                Self::LineTaskEvidence(read_line_task_evidence(&mut reader)?)
            }
            CompilerObjectKind::RuntimePlanUnit => {
                Self::RuntimePlanUnit(read_runtime_plan_unit(&mut reader)?)
            }
            CompilerObjectKind::BytecodeUnit => {
                Self::BytecodeUnit(read_bytecode_unit(&mut reader)?)
            }
            CompilerObjectKind::LinkPlan => Self::LinkPlan(read_link_plan(&mut reader)?),
        };
        reader.finish()?;
        Ok(payload)
    }

    pub fn digest(&self) -> Result<BuildDigest, AwboError> {
        Ok(BuildDigest::of(&self.encode_payload_bytes()?))
    }

    pub fn payload_len(&self) -> Result<u64, AwboError> {
        encoded_len("payload", self.encode_payload_bytes()?.len())
    }
}

impl CompilerObjectKind {
    fn from_cache_namespace(namespace: &str) -> Result<Self, AwboError> {
        match namespace {
            "parsed-syntax" => Ok(Self::ParsedSyntax),
            "interface-summary" => Ok(Self::InterfaceSummary),
            "hir-body" => Ok(Self::HirBody),
            "typecheck-gate" => Ok(Self::TypecheckGate),
            "line-task-evidence" => Ok(Self::LineTaskEvidence),
            "runtime-plan-unit" => Ok(Self::RuntimePlanUnit),
            "bytecode-unit" => Ok(Self::BytecodeUnit),
            "link-plan" => Ok(Self::LinkPlan),
            other => Err(AwboError::MalformedPayload {
                reason: format!("unknown payload namespace {other}"),
            }),
        }
    }
}

#[derive(Default)]
struct BinaryWriter {
    bytes: Vec<u8>,
}

impl BinaryWriter {
    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn put_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn put_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn put_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn put_bool(&mut self, value: bool) {
        self.put_u8(u8::from(value));
    }

    fn put_digest(&mut self, digest: BuildDigest) {
        self.bytes.extend_from_slice(&digest.as_bytes());
    }

    fn put_string(&mut self, field: &'static str, value: &str) -> Result<(), AwboError> {
        self.put_len(field, value.len())?;
        self.bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn put_string_vec(&mut self, field: &'static str, values: &[String]) -> Result<(), AwboError> {
        self.put_len(field, values.len())?;
        for value in values {
            self.put_string(field, value)?;
        }
        Ok(())
    }

    fn put_named_digests(
        &mut self,
        field: &'static str,
        values: &[NamedDigest],
    ) -> Result<(), AwboError> {
        self.put_len(field, values.len())?;
        for value in values {
            self.put_string(field, value.name())?;
            self.put_digest(value.digest());
        }
        Ok(())
    }

    fn put_bytes(&mut self, field: &'static str, bytes: &[u8]) -> Result<(), AwboError> {
        self.put_len(field, bytes.len())?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn put_bytes_raw(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn put_len(&mut self, field: &'static str, len: usize) -> Result<(), AwboError> {
        let len = u32::try_from(len).map_err(|_| AwboError::PayloadTooLarge { field })?;
        self.put_u32(len);
        Ok(())
    }
}

struct BinaryReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> BinaryReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn finish(&self) -> Result<(), AwboError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(AwboError::MalformedPayload {
                reason: format!(
                    "trailing bytes: consumed {}, total {}",
                    self.offset,
                    self.bytes.len()
                ),
            })
        }
    }

    fn read_u8(&mut self, field: &'static str) -> Result<u8, AwboError> {
        let bytes = self.read_exact(1, field)?;
        Ok(bytes[0])
    }

    fn read_u32(&mut self, field: &'static str) -> Result<u32, AwboError> {
        Ok(u32::from_le_bytes(self.read_array::<4>(field)?))
    }

    fn read_u64(&mut self, field: &'static str) -> Result<u64, AwboError> {
        Ok(u64::from_le_bytes(self.read_array::<8>(field)?))
    }

    fn read_bool(&mut self, field: &'static str) -> Result<bool, AwboError> {
        match self.read_u8(field)? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(AwboError::MalformedPayload {
                reason: format!("{field} has invalid boolean tag {other}"),
            }),
        }
    }

    fn read_digest(&mut self, field: &'static str) -> Result<BuildDigest, AwboError> {
        Ok(BuildDigest::from_bytes(self.read_array::<32>(field)?))
    }

    fn read_string(&mut self, field: &'static str) -> Result<String, AwboError> {
        let len = self.read_u32_len(field)?;
        let bytes = self.read_exact(len, field)?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|error| AwboError::MalformedPayload {
                reason: format!("{field} is not UTF-8: {error}"),
            })
    }

    fn read_string_vec(&mut self, field: &'static str) -> Result<Vec<String>, AwboError> {
        let len = self.read_u32_len(field)?;
        (0..len).map(|_| self.read_string(field)).collect()
    }

    fn read_named_digests(&mut self, field: &'static str) -> Result<Vec<NamedDigest>, AwboError> {
        let len = self.read_u32_len(field)?;
        (0..len)
            .map(|_| {
                let name = self.read_string(field)?;
                let digest = self.read_digest(field)?;
                Ok(NamedDigest::new(name, digest))
            })
            .collect()
    }

    fn read_bytes(&mut self, field: &'static str) -> Result<Vec<u8>, AwboError> {
        let len = self.read_u32_len(field)?;
        Ok(self.read_exact(len, field)?.to_vec())
    }

    fn read_exact_len(&mut self, len: u64, field: &'static str) -> Result<&'a [u8], AwboError> {
        let len = usize::try_from(len).map_err(|_| AwboError::PayloadTooLarge { field })?;
        self.read_exact(len, field)
    }

    fn read_array<const N: usize>(&mut self, field: &'static str) -> Result<[u8; N], AwboError> {
        let mut out = [0; N];
        out.copy_from_slice(self.read_exact(N, field)?);
        Ok(out)
    }

    fn read_exact(&mut self, len: usize, field: &'static str) -> Result<&'a [u8], AwboError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(AwboError::PayloadTooLarge { field })?;
        let Some(bytes) = self.bytes.get(self.offset..end) else {
            return Err(AwboError::MalformedPayload {
                reason: format!(
                    "{field} is truncated at offset {} while reading {len} bytes",
                    self.offset
                ),
            });
        };
        self.offset = end;
        Ok(bytes)
    }

    fn read_u32_len(&mut self, field: &'static str) -> Result<usize, AwboError> {
        usize::try_from(self.read_u32(field)?).map_err(|_| AwboError::PayloadTooLarge { field })
    }
}

fn encoded_len(field: &'static str, len: usize) -> Result<u64, AwboError> {
    u64::try_from(len).map_err(|_| AwboError::PayloadTooLarge { field })
}

fn put_parsed_syntax(
    writer: &mut BinaryWriter,
    value: &ParsedSyntaxObject,
) -> Result<(), AwboError> {
    writer.put_u32(value.schema_version);
    put_identity_namespace(writer, &value.compiler_namespace)?;
    writer.put_string("source_label", &value.source_label)?;
    writer.put_digest(value.source_digest);
    put_source_span(writer, value.source_span);
    put_syntax_stats(writer, &value.stats);
    put_diagnostic_summary(writer, &value.diagnostics)?;
    put_stage_inputs(writer, &value.stage_inputs)?;
    put_parsed_evidence(writer, &value.evidence)?;
    Ok(())
}

fn read_parsed_syntax(reader: &mut BinaryReader<'_>) -> Result<ParsedSyntaxObject, AwboError> {
    Ok(ParsedSyntaxObject {
        schema_version: reader.read_u32("parsed.schema_version")?,
        compiler_namespace: read_identity_namespace(reader)?,
        source_label: reader.read_string("parsed.source_label")?,
        source_digest: reader.read_digest("parsed.source_digest")?,
        source_span: read_source_span(reader)?,
        stats: read_syntax_stats(reader)?,
        diagnostics: read_diagnostic_summary(reader)?,
        stage_inputs: read_stage_inputs(reader)?,
        evidence: read_parsed_evidence(reader)?,
    })
}

fn put_hir_body(writer: &mut BinaryWriter, value: &HirBodyObject) -> Result<(), AwboError> {
    writer.put_u32(value.schema_version);
    put_identity_namespace(writer, &value.compiler_namespace)?;
    writer.put_string("module", &value.module)?;
    writer.put_digest(value.source_digest);
    put_source_span(writer, value.source_span);
    put_diagnostic_summary(writer, &value.diagnostics)?;
    put_stage_inputs(writer, &value.stage_inputs)?;
    writer.put_digest(value.body_digest);
    put_hir_facts(writer, &value.facts);
    Ok(())
}

fn read_hir_body(reader: &mut BinaryReader<'_>) -> Result<HirBodyObject, AwboError> {
    Ok(HirBodyObject {
        schema_version: reader.read_u32("hir.schema_version")?,
        compiler_namespace: read_identity_namespace(reader)?,
        module: reader.read_string("hir.module")?,
        source_digest: reader.read_digest("hir.source_digest")?,
        source_span: read_source_span(reader)?,
        diagnostics: read_diagnostic_summary(reader)?,
        stage_inputs: read_stage_inputs(reader)?,
        body_digest: reader.read_digest("hir.body_digest")?,
        facts: read_hir_facts(reader)?,
    })
}

fn put_interface_summary(
    writer: &mut BinaryWriter,
    value: &InterfaceSummaryObject,
) -> Result<(), AwboError> {
    writer.put_u32(value.schema_version);
    put_identity_namespace(writer, &value.compiler_namespace)?;
    writer.put_string("module", &value.module)?;
    writer.put_digest(value.source_digest);
    put_source_span(writer, value.source_span);
    put_diagnostic_summary(writer, &value.diagnostics)?;
    put_stage_inputs(writer, &value.stage_inputs)?;
    writer.put_digest(value.exports_digest);
    writer.put_digest(value.imports_digest);
    writer.put_len("public_symbols", value.public_symbols.len())?;
    for symbol in &value.public_symbols {
        writer.put_string("symbol.name", &symbol.name)?;
        writer.put_u8(symbol.kind.wire_tag());
        writer.put_digest(symbol.signature_digest);
    }
    Ok(())
}

fn read_interface_summary(
    reader: &mut BinaryReader<'_>,
) -> Result<InterfaceSummaryObject, AwboError> {
    let schema_version = reader.read_u32("interface.schema_version")?;
    let compiler_namespace = read_identity_namespace(reader)?;
    let module = reader.read_string("interface.module")?;
    let source_digest = reader.read_digest("interface.source_digest")?;
    let source_span = read_source_span(reader)?;
    let diagnostics = read_diagnostic_summary(reader)?;
    let stage_inputs = read_stage_inputs(reader)?;
    let exports_digest = reader.read_digest("interface.exports_digest")?;
    let imports_digest = reader.read_digest("interface.imports_digest")?;
    let len = reader.read_u32_len("interface.public_symbols")?;
    let public_symbols = (0..len)
        .map(|_| {
            Ok(PublicSymbolObject {
                name: reader.read_string("interface.symbol.name")?,
                kind: PublicSymbolKind::from_wire_tag(reader.read_u8("interface.symbol.kind")?)?,
                signature_digest: reader.read_digest("interface.symbol.signature_digest")?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InterfaceSummaryObject {
        schema_version,
        compiler_namespace,
        module,
        source_digest,
        source_span,
        diagnostics,
        stage_inputs,
        exports_digest,
        imports_digest,
        public_symbols,
    })
}

fn put_typecheck_gate(
    writer: &mut BinaryWriter,
    value: &TypecheckGateObject,
) -> Result<(), AwboError> {
    writer.put_u32(value.schema_version);
    put_identity_namespace(writer, &value.compiler_namespace)?;
    writer.put_string("typecheck.module", &value.module)?;
    writer.put_digest(value.source_digest);
    put_source_span(writer, value.source_span);
    put_diagnostic_summary(writer, &value.diagnostics)?;
    put_stage_inputs(writer, &value.stage_inputs)?;
    put_typecheck_gate_facts(writer, &value.facts)?;
    writer.put_u8(value.reuse_policy.wire_tag());
    Ok(())
}

fn read_typecheck_gate(reader: &mut BinaryReader<'_>) -> Result<TypecheckGateObject, AwboError> {
    Ok(TypecheckGateObject {
        schema_version: reader.read_u32("typecheck.schema_version")?,
        compiler_namespace: read_identity_namespace(reader)?,
        module: reader.read_string("typecheck.module")?,
        source_digest: reader.read_digest("typecheck.source_digest")?,
        source_span: read_source_span(reader)?,
        diagnostics: read_diagnostic_summary(reader)?,
        stage_inputs: read_stage_inputs(reader)?,
        facts: read_typecheck_gate_facts(reader)?,
        reuse_policy: TypecheckGateReusePolicy::from_wire_tag(
            reader.read_u8("typecheck.reuse_policy")?,
        )?,
    })
}

fn put_typecheck_gate_facts(
    writer: &mut BinaryWriter,
    value: &TypecheckGateFactsObject,
) -> Result<(), AwboError> {
    writer.put_digest(value.interface_exports_digest);
    writer.put_digest(value.interface_imports_digest);
    writer.put_digest(value.dependency_interface_digest_root);
    writer.put_digest(value.body_shape_digest);
    writer.put_digest(value.hir_symbol_digest);
    writer.put_len("typecheck.public_symbols", value.public_symbols.len())?;
    for symbol in &value.public_symbols {
        writer.put_string("typecheck.symbol.name", &symbol.name)?;
        writer.put_u8(symbol.kind.wire_tag());
        writer.put_digest(symbol.signature_digest);
    }
    writer.put_digest(value.type_signature_digest);
    writer.put_digest(value.capability_effect_digest);
    writer.put_digest(value.diagnostic_digest);
    Ok(())
}

fn read_typecheck_gate_facts(
    reader: &mut BinaryReader<'_>,
) -> Result<TypecheckGateFactsObject, AwboError> {
    let interface_exports_digest = reader.read_digest("typecheck.interface_exports_digest")?;
    let interface_imports_digest = reader.read_digest("typecheck.interface_imports_digest")?;
    let dependency_interface_digest_root =
        reader.read_digest("typecheck.dependency_interface_digest_root")?;
    let body_shape_digest = reader.read_digest("typecheck.body_shape_digest")?;
    let hir_symbol_digest = reader.read_digest("typecheck.hir_symbol_digest")?;
    let len = reader.read_u32_len("typecheck.public_symbols")?;
    let public_symbols = (0..len)
        .map(|_| {
            Ok(PublicSymbolObject {
                name: reader.read_string("typecheck.symbol.name")?,
                kind: PublicSymbolKind::from_wire_tag(reader.read_u8("typecheck.symbol.kind")?)?,
                signature_digest: reader.read_digest("typecheck.symbol.signature_digest")?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TypecheckGateFactsObject {
        interface_exports_digest,
        interface_imports_digest,
        dependency_interface_digest_root,
        body_shape_digest,
        hir_symbol_digest,
        public_symbols,
        type_signature_digest: reader.read_digest("typecheck.type_signature_digest")?,
        capability_effect_digest: reader.read_digest("typecheck.capability_effect_digest")?,
        diagnostic_digest: reader.read_digest("typecheck.diagnostic_digest")?,
    })
}

fn put_line_task_evidence(
    writer: &mut BinaryWriter,
    value: &LineTaskEvidenceObject,
) -> Result<(), AwboError> {
    writer.put_string("module", &value.module)?;
    writer.put_digest(value.evidence_digest);
    writer.put_string_vec("task_groups", &value.task_groups)
}

fn read_line_task_evidence(
    reader: &mut BinaryReader<'_>,
) -> Result<LineTaskEvidenceObject, AwboError> {
    Ok(LineTaskEvidenceObject {
        module: reader.read_string("line_task.module")?,
        evidence_digest: reader.read_digest("line_task.evidence_digest")?,
        task_groups: reader.read_string_vec("line_task.task_groups")?,
    })
}

fn put_runtime_plan_unit(
    writer: &mut BinaryWriter,
    value: &RuntimePlanUnitObject,
) -> Result<(), AwboError> {
    writer.put_string("module", &value.module)?;
    writer.put_digest(value.runtime_ir_digest);
    writer.put_bytes("runtime_plan.payload", &value.payload)
}

fn read_runtime_plan_unit(
    reader: &mut BinaryReader<'_>,
) -> Result<RuntimePlanUnitObject, AwboError> {
    Ok(RuntimePlanUnitObject {
        module: reader.read_string("runtime_plan.module")?,
        runtime_ir_digest: reader.read_digest("runtime_plan.runtime_ir_digest")?,
        payload: reader.read_bytes("runtime_plan.payload")?,
    })
}

fn put_bytecode_unit(
    writer: &mut BinaryWriter,
    value: &BytecodeUnitObject,
) -> Result<(), AwboError> {
    writer.put_string("module", &value.module)?;
    writer.put_digest(value.awbc_digest);
    writer.put_bytes("bytecode.payload", &value.payload)
}

fn read_bytecode_unit(reader: &mut BinaryReader<'_>) -> Result<BytecodeUnitObject, AwboError> {
    Ok(BytecodeUnitObject {
        module: reader.read_string("bytecode.module")?,
        awbc_digest: reader.read_digest("bytecode.awbc_digest")?,
        payload: reader.read_bytes("bytecode.payload")?,
    })
}

fn put_link_plan(writer: &mut BinaryWriter, value: &LinkPlanObject) -> Result<(), AwboError> {
    writer.put_string_vec("entrypoints", &value.entrypoints)?;
    writer.put_len("unit_digests", value.unit_digests.len())?;
    for (unit, digest) in &value.unit_digests {
        writer.put_string("unit", unit)?;
        writer.put_digest(*digest);
    }
    writer.put_digest(value.link_digest);
    Ok(())
}

fn read_link_plan(reader: &mut BinaryReader<'_>) -> Result<LinkPlanObject, AwboError> {
    let entrypoints = reader.read_string_vec("link.entrypoints")?;
    let len = reader.read_u32_len("link.unit_digests")?;
    let unit_digests = (0..len)
        .map(|_| {
            let unit = reader.read_string("link.unit")?;
            let digest = reader.read_digest("link.unit_digest")?;
            Ok((unit, digest))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let link_digest = reader.read_digest("link.link_digest")?;
    Ok(LinkPlanObject {
        entrypoints,
        unit_digests,
        link_digest,
    })
}

fn put_identity_namespace(
    writer: &mut BinaryWriter,
    value: &CompilerIdentityNamespaceObject,
) -> Result<(), AwboError> {
    let value = value.clone().canonicalized();
    writer.put_u8(value.object_kind.wire_tag());
    writer.put_string("cache_namespace", &value.cache_namespace)?;
    put_compiler_identity(writer, &value.compiler)
}

fn read_identity_namespace(
    reader: &mut BinaryReader<'_>,
) -> Result<CompilerIdentityNamespaceObject, AwboError> {
    Ok(CompilerIdentityNamespaceObject {
        object_kind: CompilerObjectKind::from_wire_tag(reader.read_u8("namespace.object_kind")?)?,
        cache_namespace: reader.read_string("namespace.cache_namespace")?,
        compiler: read_compiler_identity(reader)?,
    })
}

fn put_compiler_identity(
    writer: &mut BinaryWriter,
    value: &CompilerBuildIdentity,
) -> Result<(), AwboError> {
    let value = value.clone().canonicalized();
    writer.put_string("compiler.package_version", &value.package_version)?;
    writer.put_string("compiler.git_commit", &value.git_commit)?;
    writer.put_string("compiler.rustc", &value.rustc)?;
    writer.put_string("compiler.target", &value.target)?;
    writer.put_string_vec("compiler.enabled_features", &value.enabled_features)
}

fn read_compiler_identity(
    reader: &mut BinaryReader<'_>,
) -> Result<CompilerBuildIdentity, AwboError> {
    Ok(CompilerBuildIdentity {
        package_version: reader.read_string("compiler.package_version")?,
        git_commit: reader.read_string("compiler.git_commit")?,
        rustc: reader.read_string("compiler.rustc")?,
        target: reader.read_string("compiler.target")?,
        enabled_features: reader.read_string_vec("compiler.enabled_features")?,
    }
    .canonicalized())
}

fn put_stage_inputs(
    writer: &mut BinaryWriter,
    value: &CompilerStageInputsObject,
) -> Result<(), AwboError> {
    let value = value.clone().canonicalized();
    writer.put_digest(value.query_options_digest);
    writer.put_named_digests(
        "dependency_interface_digests",
        &value.dependency_interface_digests,
    )?;
    writer.put_named_digests("dependency_body_digests", &value.dependency_body_digests)?;
    writer.put_digest(value.environment_digest);
    Ok(())
}

fn read_stage_inputs(
    reader: &mut BinaryReader<'_>,
) -> Result<CompilerStageInputsObject, AwboError> {
    Ok(CompilerStageInputsObject {
        query_options_digest: reader.read_digest("stage.query_options_digest")?,
        dependency_interface_digests: reader.read_named_digests("stage.interfaces")?,
        dependency_body_digests: reader.read_named_digests("stage.bodies")?,
        environment_digest: reader.read_digest("stage.environment_digest")?,
    }
    .canonicalized())
}

fn put_source_span(writer: &mut BinaryWriter, value: StableSourceSpanObject) {
    writer.put_u32(value.range.start);
    writer.put_u32(value.range.end);
    writer.put_u32(value.start_line);
    writer.put_u32(value.start_column);
    writer.put_u32(value.end_line);
    writer.put_u32(value.end_column);
}

fn read_source_span(reader: &mut BinaryReader<'_>) -> Result<StableSourceSpanObject, AwboError> {
    let span = StableSourceSpanObject {
        range: StableRangeObject {
            start: reader.read_u32("span.range.start")?,
            end: reader.read_u32("span.range.end")?,
        },
        start_line: reader.read_u32("span.start_line")?,
        start_column: reader.read_u32("span.start_column")?,
        end_line: reader.read_u32("span.end_line")?,
        end_column: reader.read_u32("span.end_column")?,
    };
    span.validate()?;
    Ok(span)
}

fn put_syntax_stats(writer: &mut BinaryWriter, value: &SyntaxStatsObject) {
    writer.put_u64(value.bytes);
    writer.put_u64(value.lines);
    writer.put_u64(value.cst_lex_passes);
    writer.put_u64(value.punctuation_scans);
    writer.put_u64(value.punctuation_scan_bytes);
    writer.put_u64(value.line_owned_bytes);
    writer.put_u64(value.block_owned_bytes);
    writer.put_u64(value.raw_owned_bytes);
    writer.put_u64(value.wiki_scan_performed);
    writer.put_u64(value.dot_normalization_owned);
    writer.put_u64(value.dialogue_rescue_expr_parse_attempts);
    writer.put_u64(value.numeric_seq_summaries);
}

fn read_syntax_stats(reader: &mut BinaryReader<'_>) -> Result<SyntaxStatsObject, AwboError> {
    Ok(SyntaxStatsObject {
        bytes: reader.read_u64("stats.bytes")?,
        lines: reader.read_u64("stats.lines")?,
        cst_lex_passes: reader.read_u64("stats.cst_lex_passes")?,
        punctuation_scans: reader.read_u64("stats.punctuation_scans")?,
        punctuation_scan_bytes: reader.read_u64("stats.punctuation_scan_bytes")?,
        line_owned_bytes: reader.read_u64("stats.line_owned_bytes")?,
        block_owned_bytes: reader.read_u64("stats.block_owned_bytes")?,
        raw_owned_bytes: reader.read_u64("stats.raw_owned_bytes")?,
        wiki_scan_performed: reader.read_u64("stats.wiki_scan_performed")?,
        dot_normalization_owned: reader.read_u64("stats.dot_normalization_owned")?,
        dialogue_rescue_expr_parse_attempts: reader
            .read_u64("stats.dialogue_rescue_expr_parse_attempts")?,
        numeric_seq_summaries: reader.read_u64("stats.numeric_seq_summaries")?,
    })
}

fn put_diagnostic_summary(
    writer: &mut BinaryWriter,
    value: &StableDiagnosticSummaryObject,
) -> Result<(), AwboError> {
    value.validate()?;
    writer.put_u32(value.error_count);
    writer.put_u32(value.warning_count);
    writer.put_u32(value.info_count);
    writer.put_u32(value.note_count);
    writer.put_len("diagnostics", value.diagnostics.len())?;
    for diagnostic in &value.diagnostics {
        put_diagnostic(writer, diagnostic)?;
    }
    Ok(())
}

fn read_diagnostic_summary(
    reader: &mut BinaryReader<'_>,
) -> Result<StableDiagnosticSummaryObject, AwboError> {
    let error_count = reader.read_u32("diagnostics.error_count")?;
    let warning_count = reader.read_u32("diagnostics.warning_count")?;
    let info_count = reader.read_u32("diagnostics.info_count")?;
    let note_count = reader.read_u32("diagnostics.note_count")?;
    let len = reader.read_u32_len("diagnostics")?;
    let diagnostics = (0..len)
        .map(|_| read_diagnostic(reader))
        .collect::<Result<Vec<_>, _>>()?;
    let summary = StableDiagnosticSummaryObject {
        error_count,
        warning_count,
        info_count,
        note_count,
        diagnostics,
    };
    summary.validate()?;
    Ok(summary)
}

fn put_diagnostic(
    writer: &mut BinaryWriter,
    value: &StableDiagnosticObject,
) -> Result<(), AwboError> {
    writer.put_string("diagnostic.code", &value.code)?;
    writer.put_u8(value.severity.wire_tag());
    writer.put_string("diagnostic.message", &value.message)?;
    put_optional_span(writer, value.primary_span);
    writer.put_len("diagnostic.related_spans", value.related_spans.len())?;
    for span in &value.related_spans {
        span.validate()?;
        put_source_span(writer, *span);
    }
    Ok(())
}

fn read_diagnostic(reader: &mut BinaryReader<'_>) -> Result<StableDiagnosticObject, AwboError> {
    let code = reader.read_string("diagnostic.code")?;
    let severity = StableDiagnosticSeverity::from_wire_tag(reader.read_u8("diagnostic.severity")?)?;
    let message = reader.read_string("diagnostic.message")?;
    let primary_span = read_optional_span(reader)?;
    let len = reader.read_u32_len("diagnostic.related_spans")?;
    let related_spans = (0..len)
        .map(|_| read_source_span(reader))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(StableDiagnosticObject {
        code,
        severity,
        message,
        primary_span,
        related_spans,
    })
}

fn put_optional_span(writer: &mut BinaryWriter, value: Option<StableSourceSpanObject>) {
    match value {
        Some(span) => {
            writer.put_bool(true);
            put_source_span(writer, span);
        }
        None => writer.put_bool(false),
    }
}

fn read_optional_span(
    reader: &mut BinaryReader<'_>,
) -> Result<Option<StableSourceSpanObject>, AwboError> {
    if reader.read_bool("span.present")? {
        read_source_span(reader).map(Some)
    } else {
        Ok(None)
    }
}

fn put_parsed_evidence(
    writer: &mut BinaryWriter,
    value: &ParsedSyntaxEvidenceObject,
) -> Result<(), AwboError> {
    writer.put_string("parsed_evidence.root_kind", &value.root_kind)?;
    writer.put_digest(value.cst_shape_digest);
    writer.put_digest(value.line_index_digest);
    writer.put_u64(value.cst_node_count);
    writer.put_u64(value.cst_token_count);
    writer.put_u64(value.cst_error_node_count);
    writer.put_u64(value.typed_attribute_count);
    writer.put_u64(value.typed_use_count);
    writer.put_u64(value.typed_item_count);
    writer.put_u64(value.wiki_link_count);
    Ok(())
}

fn read_parsed_evidence(
    reader: &mut BinaryReader<'_>,
) -> Result<ParsedSyntaxEvidenceObject, AwboError> {
    Ok(ParsedSyntaxEvidenceObject {
        root_kind: reader.read_string("parsed_evidence.root_kind")?,
        cst_shape_digest: reader.read_digest("parsed_evidence.cst_shape_digest")?,
        line_index_digest: reader.read_digest("parsed_evidence.line_index_digest")?,
        cst_node_count: reader.read_u64("parsed_evidence.cst_node_count")?,
        cst_token_count: reader.read_u64("parsed_evidence.cst_token_count")?,
        cst_error_node_count: reader.read_u64("parsed_evidence.cst_error_node_count")?,
        typed_attribute_count: reader.read_u64("parsed_evidence.typed_attribute_count")?,
        typed_use_count: reader.read_u64("parsed_evidence.typed_use_count")?,
        typed_item_count: reader.read_u64("parsed_evidence.typed_item_count")?,
        wiki_link_count: reader.read_u64("parsed_evidence.wiki_link_count")?,
    })
}

fn put_hir_facts(writer: &mut BinaryWriter, value: &HirBodyFactsObject) {
    writer.put_u64(value.attribute_count);
    writer.put_u64(value.flow_count);
    writer.put_u64(value.function_count);
    writer.put_u64(value.agent_count);
    writer.put_u64(value.declaration_count);
    writer.put_u64(value.top_level_item_count);
    writer.put_u64(value.flow_item_count);
    writer.put_u64(value.statement_count);
    writer.put_u64(value.dialogue_count);
    writer.put_u64(value.choice_count);
    writer.put_u64(value.loop_count);
    writer.put_u64(value.await_count);
    writer.put_u64(value.thread_count);
    writer.put_u64(value.include_count);
    writer.put_digest(value.symbol_digest);
    writer.put_digest(value.body_shape_digest);
}

fn read_hir_facts(reader: &mut BinaryReader<'_>) -> Result<HirBodyFactsObject, AwboError> {
    Ok(HirBodyFactsObject {
        attribute_count: reader.read_u64("hir_facts.attribute_count")?,
        flow_count: reader.read_u64("hir_facts.flow_count")?,
        function_count: reader.read_u64("hir_facts.function_count")?,
        agent_count: reader.read_u64("hir_facts.agent_count")?,
        declaration_count: reader.read_u64("hir_facts.declaration_count")?,
        top_level_item_count: reader.read_u64("hir_facts.top_level_item_count")?,
        flow_item_count: reader.read_u64("hir_facts.flow_item_count")?,
        statement_count: reader.read_u64("hir_facts.statement_count")?,
        dialogue_count: reader.read_u64("hir_facts.dialogue_count")?,
        choice_count: reader.read_u64("hir_facts.choice_count")?,
        loop_count: reader.read_u64("hir_facts.loop_count")?,
        await_count: reader.read_u64("hir_facts.await_count")?,
        thread_count: reader.read_u64("hir_facts.thread_count")?,
        include_count: reader.read_u64("hir_facts.include_count")?,
        symbol_digest: reader.read_digest("hir_facts.symbol_digest")?,
        body_shape_digest: reader.read_digest("hir_facts.body_shape_digest")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> BuildDigest {
        BuildDigest::of(label.as_bytes())
    }

    fn compiler() -> CompilerBuildIdentity {
        CompilerBuildIdentity {
            package_version: "0.1.0".to_owned(),
            git_commit: "abc123".to_owned(),
            rustc: "rustc-test".to_owned(),
            target: "x86_64-pc-windows-msvc".to_owned(),
            enabled_features: vec!["b".to_owned(), "a".to_owned(), "a".to_owned()],
        }
    }

    fn key(kind: CompilerObjectKind) -> CompilerObjectKey {
        CompilerObjectKey {
            kind,
            compiler: compiler(),
            source_digest: digest("source"),
            query_options_digest: digest("options"),
            dependency_interface_digests: vec![
                NamedDigest::new("z", digest("z-interface")),
                NamedDigest::new("a", digest("a-interface")),
            ],
            dependency_body_digests: vec![NamedDigest::new("body", digest("body"))],
            environment_digest: digest("environment"),
        }
    }

    fn span() -> StableSourceSpanObject {
        StableSourceSpanObject {
            range: StableRangeObject { start: 0, end: 16 },
            start_line: 0,
            start_column: 0,
            end_line: 1,
            end_column: 0,
        }
    }

    fn diagnostic_summary() -> StableDiagnosticSummaryObject {
        StableDiagnosticSummaryObject::new(vec![StableDiagnosticObject {
            code: "syntax.test".to_owned(),
            severity: StableDiagnosticSeverity::Warning,
            message: "synthetic".to_owned(),
            primary_span: Some(span()),
            related_spans: Vec::new(),
        }])
        .expect("diagnostic summary builds")
    }

    fn parsed_payload_for(key: &CompilerObjectKey) -> CompilerObjectPayload {
        CompilerObjectPayload::ParsedSyntax(ParsedSyntaxObject {
            schema_version: AWBO_SCHEMA_VERSION,
            compiler_namespace: key.identity_namespace(),
            source_label: "src/main.arcw".to_owned(),
            source_digest: key.source_digest,
            source_span: span(),
            stats: SyntaxStatsObject {
                bytes: 16,
                lines: 1,
                cst_lex_passes: 1,
                punctuation_scans: 1,
                punctuation_scan_bytes: 16,
                line_owned_bytes: 0,
                block_owned_bytes: 0,
                raw_owned_bytes: 0,
                wiki_scan_performed: 0,
                dot_normalization_owned: 0,
                dialogue_rescue_expr_parse_attempts: 0,
                numeric_seq_summaries: 0,
            },
            diagnostics: diagnostic_summary(),
            stage_inputs: key.stage_inputs(),
            evidence: ParsedSyntaxEvidenceObject {
                root_kind: "root".to_owned(),
                cst_shape_digest: digest("cst-shape"),
                line_index_digest: digest("line-index"),
                cst_node_count: 2,
                cst_token_count: 3,
                cst_error_node_count: 0,
                typed_attribute_count: 0,
                typed_use_count: 0,
                typed_item_count: 1,
                wiki_link_count: 0,
            },
        })
    }

    fn hir_payload_for(key: &CompilerObjectKey) -> CompilerObjectPayload {
        let facts = HirBodyFactsObject {
            attribute_count: 0,
            flow_count: 1,
            function_count: 0,
            agent_count: 0,
            declaration_count: 0,
            top_level_item_count: 0,
            flow_item_count: 2,
            statement_count: 1,
            dialogue_count: 0,
            choice_count: 0,
            loop_count: 0,
            await_count: 0,
            thread_count: 0,
            include_count: 0,
            symbol_digest: digest("symbols"),
            body_shape_digest: digest("hir-shape"),
        };
        CompilerObjectPayload::HirBody(HirBodyObject {
            schema_version: AWBO_SCHEMA_VERSION,
            compiler_namespace: key.identity_namespace(),
            module: "game::main".to_owned(),
            source_digest: key.source_digest,
            source_span: span(),
            diagnostics: StableDiagnosticSummaryObject::empty(),
            stage_inputs: key.stage_inputs(),
            body_digest: facts.body_shape_digest,
            facts,
        })
    }

    fn interface_payload_for(key: &CompilerObjectKey) -> CompilerObjectPayload {
        let stage_inputs = key.stage_inputs();
        let public_symbols = InterfaceSummaryObject::canonical_public_symbols([
            PublicSymbolObject {
                name: "game::opening".to_owned(),
                kind: PublicSymbolKind::Flow,
                signature_digest: digest("opening-signature"),
            },
            PublicSymbolObject {
                name: "game::done".to_owned(),
                kind: PublicSymbolKind::Flow,
                signature_digest: digest("done-signature"),
            },
        ]);
        let imports_digest = stage_inputs.dependency_interface_digest_root();
        CompilerObjectPayload::InterfaceSummary(InterfaceSummaryObject {
            schema_version: AWBO_SCHEMA_VERSION,
            compiler_namespace: key.identity_namespace(),
            module: "game".to_owned(),
            source_digest: key.source_digest,
            source_span: span(),
            diagnostics: StableDiagnosticSummaryObject::empty(),
            stage_inputs,
            exports_digest: InterfaceSummaryObject::exports_digest_for(&public_symbols),
            imports_digest,
            public_symbols,
        })
    }

    fn typecheck_gate_payload_for(object_key: &CompilerObjectKey) -> CompilerObjectPayload {
        let CompilerObjectPayload::InterfaceSummary(interface) =
            interface_payload_for(&key(CompilerObjectKind::InterfaceSummary))
        else {
            panic!("interface helper returns interface summary");
        };
        let CompilerObjectPayload::HirBody(hir) =
            hir_payload_for(&key(CompilerObjectKind::HirBody))
        else {
            panic!("HIR helper returns HIR body");
        };
        let diagnostics = StableDiagnosticSummaryObject::empty();
        let public_symbols = interface.public_symbols.clone();
        let dependency_interface_digest_root =
            object_key.stage_inputs().dependency_interface_digest_root();
        CompilerObjectPayload::TypecheckGate(TypecheckGateObject {
            schema_version: AWBO_SCHEMA_VERSION,
            compiler_namespace: object_key.identity_namespace(),
            module: "game".to_owned(),
            source_digest: object_key.source_digest,
            source_span: span(),
            diagnostics: diagnostics.clone(),
            stage_inputs: object_key.stage_inputs(),
            facts: TypecheckGateFactsObject {
                interface_exports_digest: interface.exports_digest,
                interface_imports_digest: dependency_interface_digest_root,
                dependency_interface_digest_root,
                body_shape_digest: hir.body_digest,
                hir_symbol_digest: hir.facts.symbol_digest,
                public_symbols: public_symbols.clone(),
                type_signature_digest: TypecheckGateObject::type_signature_digest_for(
                    &public_symbols,
                ),
                capability_effect_digest:
                    TypecheckGateObject::conservative_capability_effect_digest(),
                diagnostic_digest: TypecheckGateObject::diagnostic_digest_for(&diagnostics),
            },
            reuse_policy: TypecheckGateReusePolicy::ConservativeRebuild,
        })
    }

    #[test]
    fn persistent_object_bytes_are_deterministic() {
        let key = key(CompilerObjectKind::ParsedSyntax);
        let first = AwboEnvelope::new(&key, parsed_payload_for(&key))
            .expect("envelope builds")
            .encode()
            .expect("envelope encodes");
        let second = AwboEnvelope::new(&key, parsed_payload_for(&key))
            .expect("envelope builds")
            .encode()
            .expect("envelope encodes");

        assert_eq!(first, second);
    }

    #[test]
    fn persistent_object_round_trips_parse_interface_and_hir_payloads() {
        for kind in [
            CompilerObjectKind::ParsedSyntax,
            CompilerObjectKind::InterfaceSummary,
            CompilerObjectKind::HirBody,
            CompilerObjectKind::TypecheckGate,
        ] {
            let key = key(kind);
            let payload = match kind {
                CompilerObjectKind::ParsedSyntax => parsed_payload_for(&key),
                CompilerObjectKind::InterfaceSummary => interface_payload_for(&key),
                CompilerObjectKind::HirBody => hir_payload_for(&key),
                CompilerObjectKind::TypecheckGate => typecheck_gate_payload_for(&key),
                _ => unreachable!("test covers parse/interface/HIR payloads"),
            };
            let envelope = AwboEnvelope::new(&key, payload).expect("envelope builds");
            let bytes = envelope.encode().expect("envelope encodes");
            let decoded = AwboEnvelope::decode(&bytes, &key).expect("envelope decodes");

            assert_eq!(decoded, envelope);
        }
    }

    #[test]
    fn persistent_object_rejects_schema_and_compiler_identity_mismatch() {
        let key = key(CompilerObjectKind::ParsedSyntax);
        let envelope = AwboEnvelope::new(&key, parsed_payload_for(&key)).expect("envelope builds");
        let mut bytes = envelope.encode().expect("envelope encodes");
        bytes[8..12].copy_from_slice(&99_u32.to_le_bytes());
        assert_eq!(
            AwboEnvelope::decode(&bytes, &key),
            Err(AwboError::UnsupportedSchema {
                actual: 99,
                expected: AWBO_SCHEMA_VERSION,
            })
        );

        let mut other_key = key.clone();
        other_key.compiler.git_commit = "other".to_owned();
        let bytes = envelope.encode().expect("envelope encodes");
        assert_eq!(
            AwboEnvelope::decode(&bytes, &other_key),
            Err(AwboError::KeyDigestMismatch)
        );

        let CompilerObjectPayload::ParsedSyntax(parsed) = &envelope.payload else {
            panic!("payload is parsed syntax");
        };
        assert_eq!(
            parsed.compiler_namespace.compiler.enabled_features,
            ["a".to_owned(), "b".to_owned()]
        );
    }

    #[test]
    fn persistent_object_rejects_malformed_payloads() {
        let key = key(CompilerObjectKind::ParsedSyntax);
        let mut bad_payload = parsed_payload_for(&key);
        let CompilerObjectPayload::ParsedSyntax(parsed) = &mut bad_payload else {
            panic!("payload is parsed syntax");
        };
        parsed.source_span.range.start = 8;
        parsed.source_span.range.end = 4;
        assert!(matches!(
            AwboEnvelope::new(&key, bad_payload),
            Err(AwboError::MalformedPayload { .. })
        ));

        let bytes = AwboEnvelope::new(&key, parsed_payload_for(&key))
            .expect("envelope builds")
            .encode()
            .expect("envelope encodes");
        let truncated = &bytes[..bytes.len() - 1];
        assert!(matches!(
            AwboEnvelope::decode(truncated, &key),
            Err(AwboError::MalformedPayload { .. })
        ));
    }

    #[test]
    fn persistent_object_rejects_malformed_diagnostic_summary() {
        let key = key(CompilerObjectKind::ParsedSyntax);
        let mut payload = parsed_payload_for(&key);
        let CompilerObjectPayload::ParsedSyntax(parsed) = &mut payload else {
            panic!("payload is parsed syntax");
        };
        parsed.diagnostics.warning_count = 0;

        assert!(matches!(
            AwboEnvelope::new(&key, payload),
            Err(AwboError::MalformedPayload { .. })
        ));
    }

    #[test]
    fn persistent_object_rejects_malformed_interface_summary() {
        let key = key(CompilerObjectKind::InterfaceSummary);
        let mut payload = interface_payload_for(&key);
        let CompilerObjectPayload::InterfaceSummary(interface) = &mut payload else {
            panic!("payload is interface summary");
        };
        interface.exports_digest = digest("wrong-exports");
        assert!(matches!(
            AwboEnvelope::new(&key, payload),
            Err(AwboError::MalformedPayload { .. })
        ));

        let mut payload = interface_payload_for(&key);
        let CompilerObjectPayload::InterfaceSummary(interface) = &mut payload else {
            panic!("payload is interface summary");
        };
        interface.imports_digest = digest("wrong-imports");
        assert!(matches!(
            AwboEnvelope::new(&key, payload),
            Err(AwboError::MalformedPayload { .. })
        ));
    }

    #[test]
    fn link_plan_payload_digest_is_stable_for_btree_order() {
        let mut units = BTreeMap::new();
        units.insert("b".to_owned(), digest("b"));
        units.insert("a".to_owned(), digest("a"));
        let first = CompilerObjectPayload::LinkPlan(LinkPlanObject {
            entrypoints: vec!["main".to_owned()],
            unit_digests: units.clone(),
            link_digest: digest("link"),
        });
        let second = CompilerObjectPayload::LinkPlan(LinkPlanObject {
            entrypoints: vec!["main".to_owned()],
            unit_digests: units,
            link_digest: digest("link"),
        });

        assert_eq!(
            first.digest().expect("digest builds"),
            second.digest().expect("digest builds")
        );
    }
}
