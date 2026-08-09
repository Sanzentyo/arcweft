use super::schema::{
    AWBO_SCHEMA_VERSION, AwboError, CompilerIdentityNamespaceObject, CompilerObjectKey,
    CompilerObjectKind, CompilerStageInputsObject,
};
use crate::fingerprint::{
    BuildDigest, NamedDigest, put_digest, put_named_digests, put_string, put_u32,
};
use serde::{Deserialize, Serialize};

/// Compiler-private object payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CompilerObjectPayload {
    ParsedSyntax(ParsedSyntaxObject),
    InterfaceSummary(InterfaceSummaryObject),
    HirBody(HirBodyObject),
    TypecheckGate(TypecheckGateObject),
    LineTaskEvidence(LineTaskEvidenceObject),
    RuntimePlanUnit(RuntimePlanUnitObject),
    BytecodeUnit(BytecodeUnitObject),
    LinkPlan(LinkPlanObject),
}

/// Deterministic syntax-parse fact payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ParsedSyntaxObject {
    pub schema_version: u32,
    pub compiler_namespace: CompilerIdentityNamespaceObject,
    pub source_label: String,
    pub source_digest: BuildDigest,
    pub source_span: StableSourceSpanObject,
    pub stats: SyntaxStatsObject,
    pub diagnostics: StableDiagnosticSummaryObject,
    pub stage_inputs: CompilerStageInputsObject,
    pub evidence: ParsedSyntaxEvidenceObject,
}

/// Path-free syntax parser counters and source dimensions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyntaxStatsObject {
    pub accepted_source_bytes: u64,
    pub lexer_tokens: u64,
    pub grammar_events: u64,
    pub top_level_items: u64,
    pub statements: u64,
    pub expressions: u64,
    pub type_nodes: u64,
    pub pattern_nodes: u64,
    pub identity_bearing_nodes: u64,
    pub diagnostic_identities: u64,
}

/// Compact deterministic parse evidence for later cache validation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ParsedSyntaxEvidenceObject {
    pub root_kind: u32,
    pub cst_shape_digest: BuildDigest,
    pub line_index_digest: BuildDigest,
    pub cst_node_count: u64,
    pub cst_token_count: u64,
    pub cst_error_node_count: u64,
    pub typed_attribute_count: u64,
    pub typed_use_count: u64,
    pub typed_item_count: u64,
}

/// Deterministic HIR-body exact fact payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HirBodyObject {
    pub schema_version: u32,
    pub compiler_namespace: CompilerIdentityNamespaceObject,
    pub module: String,
    pub source_digest: BuildDigest,
    pub source_span: StableSourceSpanObject,
    pub diagnostics: StableDiagnosticSummaryObject,
    pub stage_inputs: CompilerStageInputsObject,
    pub body_digest: BuildDigest,
    pub facts: HirBodyFactsObject,
}

/// HIR body facts that avoid serializing `HirModule` internals directly.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HirBodyFactsObject {
    pub attribute_count: u64,
    pub flow_count: u64,
    pub function_count: u64,
    pub declaration_count: u64,
    pub flow_item_count: u64,
    pub statement_count: u64,
    pub dialogue_count: u64,
    pub choice_count: u64,
    pub loop_count: u64,
    pub await_count: u64,
    pub thread_count: u64,
    pub include_count: u64,
    pub symbol_digest: BuildDigest,
    pub body_shape_digest: BuildDigest,
}

/// Stable source range represented as half-open UTF-8 byte offsets.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StableRangeObject {
    pub start: u32,
    pub end: u32,
}

/// Stable source span with both byte and line/column coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StableSourceSpanObject {
    pub range: StableRangeObject,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Stable diagnostic severity used by compiler-private payload facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StableDiagnosticSeverity {
    Error,
    Warning,
    Info,
    Note,
}

/// One stable diagnostic fact. Messages are summaries, not reuse authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StableDiagnosticObject {
    pub code: String,
    pub severity: StableDiagnosticSeverity,
    pub message: String,
    pub primary_span: Option<StableSourceSpanObject>,
    pub related_spans: Vec<StableSourceSpanObject>,
}

/// Counts plus ordered diagnostic summaries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StableDiagnosticSummaryObject {
    pub error_count: u32,
    pub warning_count: u32,
    pub info_count: u32,
    pub note_count: u32,
    pub diagnostics: Vec<StableDiagnosticObject>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InterfaceSummaryObject {
    pub schema_version: u32,
    pub compiler_namespace: CompilerIdentityNamespaceObject,
    pub module: String,
    pub source_digest: BuildDigest,
    pub source_span: StableSourceSpanObject,
    pub diagnostics: StableDiagnosticSummaryObject,
    pub stage_inputs: CompilerStageInputsObject,
    pub exports_digest: BuildDigest,
    pub imports_digest: BuildDigest,
    pub public_symbols: Vec<PublicSymbolObject>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicSymbolKind {
    Flow,
    Function,
    Declaration,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct PublicSymbolObject {
    pub name: String,
    pub kind: PublicSymbolKind,
    pub signature_digest: BuildDigest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TypecheckGateObject {
    pub schema_version: u32,
    pub compiler_namespace: CompilerIdentityNamespaceObject,
    pub module: String,
    pub source_digest: BuildDigest,
    pub source_span: StableSourceSpanObject,
    pub diagnostics: StableDiagnosticSummaryObject,
    pub stage_inputs: CompilerStageInputsObject,
    pub facts: TypecheckGateFactsObject,
    pub reuse_policy: TypecheckGateReusePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TypecheckGateFactsObject {
    pub interface_exports_digest: BuildDigest,
    pub interface_imports_digest: BuildDigest,
    pub dependency_interface_digest_root: BuildDigest,
    pub body_shape_digest: BuildDigest,
    pub hir_symbol_digest: BuildDigest,
    pub public_symbols: Vec<PublicSymbolObject>,
    pub type_signature_digest: BuildDigest,
    pub capability_effect_digest: BuildDigest,
    pub diagnostic_digest: BuildDigest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypecheckGateReusePolicy {
    ConservativeRebuild,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LineTaskEvidenceObject {
    pub module: String,
    pub evidence_digest: BuildDigest,
    pub task_groups: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimePlanUnitObject {
    pub module: String,
    pub runtime_ir_digest: BuildDigest,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BytecodeUnitIdentityObject {
    pub runtime_plan_unit_digest: BuildDigest,
    pub awbc_schema_digest: BuildDigest,
    pub verifier_policy_digest: BuildDigest,
    pub codegen_policy_digest: BuildDigest,
    pub target_profile_digest: BuildDigest,
    pub feature_set_digest: BuildDigest,
    pub relocation_import_table_digest: BuildDigest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BytecodeUnitObject {
    pub schema_version: u32,
    pub compiler_namespace: CompilerIdentityNamespaceObject,
    pub module: String,
    pub source_digest: BuildDigest,
    pub source_span: StableSourceSpanObject,
    pub diagnostics: StableDiagnosticSummaryObject,
    pub stage_inputs: CompilerStageInputsObject,
    pub facts: BytecodeUnitFactsObject,
    pub canonical_awbc_bytes: Vec<u8>,
    pub reuse_policy: BytecodeUnitReusePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BytecodeUnitFactsObject {
    pub identity: BytecodeUnitIdentityObject,
    pub hir_body_digest: BuildDigest,
    pub typecheck_gate_digest: BuildDigest,
    pub dependency_body_digest_root: BuildDigest,
    pub canonical_bytecode_digest: BuildDigest,
    pub bytecode_descriptor_digest: BuildDigest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BytecodeUnitReusePolicy {
    ConservativeRebuild,
    VerifiedReusable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LinkDescriptorObject {
    pub ordered_unit_identities: Vec<NamedDigest>,
    pub entrypoint_digest: BuildDigest,
    pub resource_section_digest: BuildDigest,
    pub adapter_requirements_digest: BuildDigest,
    pub patch_compatibility_digest: BuildDigest,
    pub product_build_options_digest: BuildDigest,
    pub dependency_body_digest_root: BuildDigest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LinkPlanObject {
    pub schema_version: u32,
    pub compiler_namespace: CompilerIdentityNamespaceObject,
    pub package: String,
    pub source_digest: BuildDigest,
    pub source_span: StableSourceSpanObject,
    pub diagnostics: StableDiagnosticSummaryObject,
    pub stage_inputs: CompilerStageInputsObject,
    pub facts: LinkPlanFactsObject,
    pub reuse_policy: LinkPlanReusePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LinkPlanFactsObject {
    pub descriptor: LinkDescriptorObject,
    pub link_descriptor_digest: BuildDigest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkPlanReusePolicy {
    ConservativeRebuild,
    VerifiedReusable,
}

impl CompilerObjectPayload {
    pub const fn kind(&self) -> CompilerObjectKind {
        match self {
            Self::ParsedSyntax(_) => CompilerObjectKind::ParsedSyntax,
            Self::InterfaceSummary(_) => CompilerObjectKind::InterfaceSummary,
            Self::HirBody(_) => CompilerObjectKind::HirBody,
            Self::TypecheckGate(_) => CompilerObjectKind::TypecheckGate,
            Self::LineTaskEvidence(_) => CompilerObjectKind::LineTaskEvidence,
            Self::RuntimePlanUnit(_) => CompilerObjectKind::RuntimePlanUnit,
            Self::BytecodeUnit(_) => CompilerObjectKind::BytecodeUnit,
            Self::LinkPlan(_) => CompilerObjectKind::LinkPlan,
        }
    }

    pub fn validate_contract_for_key(&self, key: &CompilerObjectKey) -> Result<(), AwboError> {
        if self.kind() != key.kind {
            return Err(AwboError::KindMismatch {
                key: key.kind,
                payload: self.kind(),
            });
        }
        match self {
            Self::ParsedSyntax(value) => value.validate_for_key(key),
            Self::InterfaceSummary(value) => value.validate_for_key(key),
            Self::HirBody(value) => value.validate_for_key(key),
            Self::TypecheckGate(value) => value.validate_for_key(key),
            Self::BytecodeUnit(value) => value.validate_for_key(key),
            Self::LinkPlan(value) => value.validate_for_key(key),
            Self::LineTaskEvidence(_) | Self::RuntimePlanUnit(_) => Ok(()),
        }
    }
}

impl ParsedSyntaxObject {
    pub fn validate_for_key(&self, key: &CompilerObjectKey) -> Result<(), AwboError> {
        validate_version(self.schema_version)?;
        self.compiler_namespace.validate_for_key(key)?;
        if self.source_digest != key.source_digest {
            return Err(AwboError::PayloadKeyInputMismatch {
                field: "parsed_syntax.source_digest",
            });
        }
        self.source_span.validate()?;
        self.diagnostics.validate()?;
        self.stage_inputs.validate_for_key(key)
    }
}

impl HirBodyObject {
    pub fn validate_for_key(&self, key: &CompilerObjectKey) -> Result<(), AwboError> {
        validate_version(self.schema_version)?;
        self.compiler_namespace.validate_for_key(key)?;
        if self.source_digest != key.source_digest {
            return Err(AwboError::PayloadKeyInputMismatch {
                field: "hir_body.source_digest",
            });
        }
        self.source_span.validate()?;
        self.diagnostics.validate()?;
        if self.body_digest != self.facts.body_shape_digest {
            return Err(AwboError::MalformedPayload {
                reason: "HIR body digest does not match body shape digest".to_owned(),
            });
        }
        self.stage_inputs.validate_for_key(key)
    }
}

impl InterfaceSummaryObject {
    pub fn validate_for_key(&self, key: &CompilerObjectKey) -> Result<(), AwboError> {
        validate_version(self.schema_version)?;
        self.compiler_namespace.validate_for_key(key)?;
        if self.source_digest != key.source_digest {
            return Err(AwboError::PayloadKeyInputMismatch {
                field: "interface_summary.source_digest",
            });
        }
        self.source_span.validate()?;
        self.diagnostics.validate()?;
        self.stage_inputs.validate_for_key(key)?;
        self.validate_summary_shape()
    }

    pub fn validate_summary_shape(&self) -> Result<(), AwboError> {
        if self.module.is_empty() {
            return Err(AwboError::MalformedPayload {
                reason: "interface summary module is empty".to_owned(),
            });
        }
        let canonical = Self::canonical_public_symbols(self.public_symbols.clone());
        if canonical != self.public_symbols {
            return Err(AwboError::MalformedPayload {
                reason: "interface public symbols are not canonical".to_owned(),
            });
        }
        if has_duplicate_public_symbol_descriptor(&canonical) {
            return Err(AwboError::MalformedPayload {
                reason: "interface public symbols contain duplicate descriptors".to_owned(),
            });
        }
        let expected_exports = Self::exports_digest_for(&canonical);
        if self.exports_digest != expected_exports {
            return Err(AwboError::MalformedPayload {
                reason: "interface exports digest does not match public symbols".to_owned(),
            });
        }
        let expected_imports = self.stage_inputs.dependency_interface_digest_root();
        if self.imports_digest != expected_imports {
            return Err(AwboError::MalformedPayload {
                reason: "interface imports digest does not match dependency interfaces".to_owned(),
            });
        }
        Ok(())
    }

    pub fn canonical_public_symbols(
        symbols: impl IntoIterator<Item = PublicSymbolObject>,
    ) -> Vec<PublicSymbolObject> {
        let mut symbols = symbols.into_iter().collect::<Vec<_>>();
        symbols.sort();
        symbols
    }

    pub fn exports_digest_for(symbols: &[PublicSymbolObject]) -> BuildDigest {
        let symbols = Self::canonical_public_symbols(symbols.iter().cloned());
        let mut bytes = Vec::new();
        put_string(&mut bytes, "interface-exports-v1");
        put_u32(&mut bytes, u32::try_from(symbols.len()).unwrap_or(u32::MAX));
        for symbol in symbols {
            put_string(&mut bytes, symbol.kind.as_str());
            put_string(&mut bytes, &symbol.name);
            put_digest(&mut bytes, symbol.signature_digest);
        }
        BuildDigest::of(&bytes)
    }
}

impl TypecheckGateObject {
    pub fn validate_for_key(&self, key: &CompilerObjectKey) -> Result<(), AwboError> {
        validate_version(self.schema_version)?;
        self.compiler_namespace.validate_for_key(key)?;
        if self.source_digest != key.source_digest {
            return Err(AwboError::PayloadKeyInputMismatch {
                field: "typecheck_gate.source_digest",
            });
        }
        self.source_span.validate()?;
        self.diagnostics.validate()?;
        self.stage_inputs.validate_for_key(key)?;
        self.validate_gate_shape()
    }

    pub fn validate_gate_shape(&self) -> Result<(), AwboError> {
        if self.module.is_empty() {
            return Err(AwboError::MalformedPayload {
                reason: "typecheck gate module is empty".to_owned(),
            });
        }
        if self.reuse_policy != TypecheckGateReusePolicy::ConservativeRebuild {
            return Err(AwboError::MalformedPayload {
                reason: "typecheck gate must use conservative rebuild policy".to_owned(),
            });
        }
        let canonical =
            InterfaceSummaryObject::canonical_public_symbols(self.facts.public_symbols.clone());
        if canonical != self.facts.public_symbols {
            return Err(AwboError::MalformedPayload {
                reason: "typecheck gate public symbols are not canonical".to_owned(),
            });
        }
        if has_duplicate_public_symbol_descriptor(&canonical) {
            return Err(AwboError::MalformedPayload {
                reason: "typecheck gate public symbols contain duplicate descriptors".to_owned(),
            });
        }
        if self.facts.interface_exports_digest
            != InterfaceSummaryObject::exports_digest_for(&canonical)
        {
            return Err(AwboError::MalformedPayload {
                reason: "typecheck gate exports digest does not match public symbols".to_owned(),
            });
        }
        let dependency_root = self.stage_inputs.dependency_interface_digest_root();
        if self.facts.dependency_interface_digest_root != dependency_root {
            return Err(AwboError::MalformedPayload {
                reason: "typecheck gate dependency interface root mismatch".to_owned(),
            });
        }
        if self.facts.interface_imports_digest != dependency_root {
            return Err(AwboError::MalformedPayload {
                reason: "typecheck gate imports digest does not match dependency interfaces"
                    .to_owned(),
            });
        }
        if self.facts.type_signature_digest != Self::type_signature_digest_for(&canonical) {
            return Err(AwboError::MalformedPayload {
                reason: "typecheck gate type signature digest mismatch".to_owned(),
            });
        }
        if self.facts.capability_effect_digest != Self::conservative_capability_effect_digest() {
            return Err(AwboError::MalformedPayload {
                reason: "typecheck gate capability/effect digest is not conservative sentinel"
                    .to_owned(),
            });
        }
        if self.facts.diagnostic_digest != Self::diagnostic_digest_for(&self.diagnostics) {
            return Err(AwboError::MalformedPayload {
                reason: "typecheck gate diagnostic digest mismatch".to_owned(),
            });
        }
        Ok(())
    }

    pub fn type_signature_digest_for(symbols: &[PublicSymbolObject]) -> BuildDigest {
        let symbols = InterfaceSummaryObject::canonical_public_symbols(symbols.iter().cloned());
        let mut bytes = Vec::new();
        put_string(&mut bytes, "typecheck-gate-signatures-v1");
        put_u32(&mut bytes, u32::try_from(symbols.len()).unwrap_or(u32::MAX));
        for symbol in symbols {
            put_string(&mut bytes, symbol.kind.as_str());
            put_string(&mut bytes, &symbol.name);
            put_digest(&mut bytes, symbol.signature_digest);
        }
        BuildDigest::of(&bytes)
    }

    pub fn conservative_capability_effect_digest() -> BuildDigest {
        BuildDigest::of(b"typecheck-gate-capability-effect-v1:conservative-rebuild")
    }

    pub fn diagnostic_digest_for(diagnostics: &StableDiagnosticSummaryObject) -> BuildDigest {
        let mut bytes = Vec::new();
        put_string(&mut bytes, "typecheck-gate-diagnostics-v1");
        put_u32(&mut bytes, diagnostics.error_count);
        put_u32(&mut bytes, diagnostics.warning_count);
        put_u32(&mut bytes, diagnostics.info_count);
        put_u32(&mut bytes, diagnostics.note_count);
        put_u32(
            &mut bytes,
            u32::try_from(diagnostics.diagnostics.len()).unwrap_or(u32::MAX),
        );
        for diagnostic in &diagnostics.diagnostics {
            put_string(&mut bytes, &diagnostic.code);
            put_u32(&mut bytes, u32::from(diagnostic.severity.wire_tag()));
            put_string(&mut bytes, &diagnostic.message);
            put_optional_stable_span_digest(&mut bytes, diagnostic.primary_span);
            put_u32(
                &mut bytes,
                u32::try_from(diagnostic.related_spans.len()).unwrap_or(u32::MAX),
            );
            for span in &diagnostic.related_spans {
                put_stable_span_digest(&mut bytes, *span);
            }
        }
        BuildDigest::of(&bytes)
    }
}

impl TypecheckGateReusePolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConservativeRebuild => "conservative_rebuild",
        }
    }

    pub const fn wire_tag(self) -> u8 {
        match self {
            Self::ConservativeRebuild => 0,
        }
    }

    pub const fn from_wire_tag(tag: u8) -> Result<Self, AwboError> {
        match tag {
            0 => Ok(Self::ConservativeRebuild),
            _ => Err(AwboError::UnsupportedWireTag {
                domain: "typecheck gate reuse policy",
                tag,
            }),
        }
    }
}

impl BytecodeUnitObject {
    pub fn validate_for_key(&self, key: &CompilerObjectKey) -> Result<(), AwboError> {
        validate_version(self.schema_version)?;
        self.compiler_namespace.validate_for_key(key)?;
        if self.source_digest != key.source_digest {
            return Err(AwboError::PayloadKeyInputMismatch {
                field: "bytecode_unit.source_digest",
            });
        }
        self.source_span.validate()?;
        self.diagnostics.validate()?;
        self.stage_inputs.validate_for_key(key)?;
        self.validate_gate_shape()
    }

    pub fn validate_gate_shape(&self) -> Result<(), AwboError> {
        if self.module.is_empty() {
            return Err(AwboError::MalformedPayload {
                reason: "bytecode unit module is empty".to_owned(),
            });
        }
        match self.reuse_policy {
            BytecodeUnitReusePolicy::ConservativeRebuild => self.validate_conservative_shape(),
            BytecodeUnitReusePolicy::VerifiedReusable => self.validate_reusable_shape(),
        }
    }

    fn validate_common_shape(&self) -> Result<(), AwboError> {
        if self.facts.dependency_body_digest_root != self.stage_inputs.dependency_body_digest_root()
        {
            return Err(AwboError::MalformedPayload {
                reason: "bytecode unit dependency body root mismatch".to_owned(),
            });
        }
        Ok(())
    }

    fn validate_conservative_shape(&self) -> Result<(), AwboError> {
        self.validate_common_shape()?;
        if self.facts.identity.runtime_plan_unit_digest
            != Self::conservative_runtime_plan_unit_digest()
        {
            return Err(AwboError::MalformedPayload {
                reason: "bytecode unit runtime-plan identity is not the conservative sentinel"
                    .to_owned(),
            });
        }
        if !self.canonical_awbc_bytes.is_empty() {
            return Err(AwboError::MalformedPayload {
                reason: "conservative bytecode unit must not persist AWBC bytes".to_owned(),
            });
        }
        if self.facts.canonical_bytecode_digest != Self::conservative_canonical_bytecode_digest() {
            return Err(AwboError::MalformedPayload {
                reason: "bytecode unit canonical bytecode digest is not the conservative sentinel"
                    .to_owned(),
            });
        }
        if self.facts.bytecode_descriptor_digest
            != Self::bytecode_descriptor_digest_for(&self.facts)
        {
            return Err(AwboError::MalformedPayload {
                reason: "conservative bytecode descriptor digest mismatch".to_owned(),
            });
        }
        Ok(())
    }

    fn validate_reusable_shape(&self) -> Result<(), AwboError> {
        self.validate_common_shape()?;
        if self.canonical_awbc_bytes.is_empty() {
            return Err(AwboError::MalformedPayload {
                reason: "reusable bytecode unit has no canonical AWBC bytes".to_owned(),
            });
        }
        if self.facts.canonical_bytecode_digest != BuildDigest::of(&self.canonical_awbc_bytes) {
            return Err(AwboError::MalformedPayload {
                reason: "bytecode unit canonical AWBC digest mismatch".to_owned(),
            });
        }
        if self.facts.identity.runtime_plan_unit_digest
            == Self::conservative_runtime_plan_unit_digest()
            || self.facts.canonical_bytecode_digest
                == Self::conservative_canonical_bytecode_digest()
            || self.facts.identity.awbc_schema_digest == Self::conservative_awbc_schema_digest()
            || self.facts.identity.verifier_policy_digest
                == Self::conservative_verifier_policy_digest()
            || self.facts.identity.codegen_policy_digest
                == Self::conservative_codegen_policy_digest()
            || self.facts.identity.relocation_import_table_digest
                == Self::conservative_relocation_import_table_digest()
        {
            return Err(AwboError::MalformedPayload {
                reason: "reusable bytecode unit still contains a conservative sentinel".to_owned(),
            });
        }
        if self.facts.bytecode_descriptor_digest
            != Self::bytecode_descriptor_digest_for(&self.facts)
        {
            return Err(AwboError::MalformedPayload {
                reason: "bytecode unit descriptor digest mismatch".to_owned(),
            });
        }
        Ok(())
    }

    pub fn bytecode_descriptor_digest_for(facts: &BytecodeUnitFactsObject) -> BuildDigest {
        let mut bytes = Vec::new();
        put_string(&mut bytes, "bytecode-unit-descriptor-v2");
        put_digest(&mut bytes, facts.identity.runtime_plan_unit_digest);
        put_digest(&mut bytes, facts.hir_body_digest);
        put_digest(&mut bytes, facts.typecheck_gate_digest);
        put_digest(&mut bytes, facts.identity.awbc_schema_digest);
        put_digest(&mut bytes, facts.identity.verifier_policy_digest);
        put_digest(&mut bytes, facts.identity.codegen_policy_digest);
        put_digest(&mut bytes, facts.identity.target_profile_digest);
        put_digest(&mut bytes, facts.identity.feature_set_digest);
        put_digest(&mut bytes, facts.identity.relocation_import_table_digest);
        put_digest(&mut bytes, facts.dependency_body_digest_root);
        put_digest(&mut bytes, facts.canonical_bytecode_digest);
        BuildDigest::of(&bytes)
    }

    pub fn conservative_runtime_plan_unit_digest() -> BuildDigest {
        BuildDigest::of(b"bytecode-unit-gate-v1:runtime-plan-unit-identity-unavailable")
    }

    pub fn conservative_canonical_bytecode_digest() -> BuildDigest {
        BuildDigest::of(b"bytecode-unit-gate-v1:canonical-awbc-bytes-not-reused")
    }

    pub fn conservative_relocation_import_table_digest() -> BuildDigest {
        BuildDigest::of(b"bytecode-unit-gate-v1:relocation-import-table-conservative")
    }

    pub fn conservative_awbc_schema_digest() -> BuildDigest {
        BuildDigest::of(b"bytecode-unit-gate-v1:awbc-schema-conservative")
    }

    pub fn conservative_verifier_policy_digest() -> BuildDigest {
        BuildDigest::of(b"bytecode-unit-gate-v1:verifier-policy-conservative")
    }

    pub fn conservative_codegen_policy_digest() -> BuildDigest {
        BuildDigest::of(b"bytecode-unit-gate-v1:codegen-policy-conservative")
    }
}

impl BytecodeUnitReusePolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConservativeRebuild => "conservative_rebuild",
            Self::VerifiedReusable => "verified_reusable",
        }
    }

    pub const fn wire_tag(self) -> u8 {
        match self {
            Self::ConservativeRebuild => 0,
            Self::VerifiedReusable => 1,
        }
    }

    pub const fn from_wire_tag(tag: u8) -> Result<Self, AwboError> {
        match tag {
            0 => Ok(Self::ConservativeRebuild),
            1 => Ok(Self::VerifiedReusable),
            _ => Err(AwboError::UnsupportedWireTag {
                domain: "bytecode unit reuse policy",
                tag,
            }),
        }
    }
}

impl LinkPlanObject {
    pub fn validate_for_key(&self, key: &CompilerObjectKey) -> Result<(), AwboError> {
        validate_version(self.schema_version)?;
        self.compiler_namespace.validate_for_key(key)?;
        if self.source_digest != key.source_digest {
            return Err(AwboError::PayloadKeyInputMismatch {
                field: "link_plan.source_digest",
            });
        }
        self.source_span.validate()?;
        self.diagnostics.validate()?;
        self.stage_inputs.validate_for_key(key)?;
        self.validate_gate_shape()
    }

    pub fn validate_gate_shape(&self) -> Result<(), AwboError> {
        if self.package.is_empty() {
            return Err(AwboError::MalformedPayload {
                reason: "link plan package is empty".to_owned(),
            });
        }
        if has_duplicate_named_digest(&self.facts.descriptor.ordered_unit_identities) {
            return Err(AwboError::MalformedPayload {
                reason: "link plan ordered unit identities contain duplicate names".to_owned(),
            });
        }
        if self.facts.descriptor.dependency_body_digest_root
            != self.stage_inputs.dependency_body_digest_root()
        {
            return Err(AwboError::MalformedPayload {
                reason: "link plan dependency body root mismatch".to_owned(),
            });
        }
        if self.facts.link_descriptor_digest
            != Self::link_descriptor_digest_for(&self.facts.descriptor)
        {
            return Err(AwboError::MalformedPayload {
                reason: "link plan descriptor digest mismatch".to_owned(),
            });
        }
        if self.reuse_policy == LinkPlanReusePolicy::VerifiedReusable
            && self.facts.descriptor.ordered_unit_identities.is_empty()
        {
            return Err(AwboError::MalformedPayload {
                reason: "reusable link plan has no ordered unit identities".to_owned(),
            });
        }
        if self.reuse_policy == LinkPlanReusePolicy::VerifiedReusable
            && (self.facts.descriptor.entrypoint_digest == Self::conservative_entrypoint_digest()
                || self.facts.descriptor.resource_section_digest
                    == Self::conservative_resource_section_digest()
                || self.facts.descriptor.adapter_requirements_digest
                    == Self::conservative_adapter_requirements_digest()
                || self.facts.descriptor.patch_compatibility_digest
                    == Self::conservative_patch_compatibility_digest())
        {
            return Err(AwboError::MalformedPayload {
                reason: "reusable link plan still contains a conservative descriptor sentinel"
                    .to_owned(),
            });
        }
        Ok(())
    }

    pub fn link_descriptor_digest_for(descriptor: &LinkDescriptorObject) -> BuildDigest {
        let mut bytes = Vec::new();
        put_string(&mut bytes, "link-plan-descriptor-v2");
        put_named_digests(&mut bytes, &descriptor.ordered_unit_identities);
        put_digest(&mut bytes, descriptor.entrypoint_digest);
        put_digest(&mut bytes, descriptor.resource_section_digest);
        put_digest(&mut bytes, descriptor.adapter_requirements_digest);
        put_digest(&mut bytes, descriptor.patch_compatibility_digest);
        put_digest(&mut bytes, descriptor.product_build_options_digest);
        put_digest(&mut bytes, descriptor.dependency_body_digest_root);
        BuildDigest::of(&bytes)
    }

    pub fn conservative_entrypoint_digest() -> BuildDigest {
        BuildDigest::of(b"link-plan-gate-v1:entrypoints-conservative")
    }

    pub fn conservative_resource_section_digest() -> BuildDigest {
        BuildDigest::of(b"link-plan-gate-v1:resources-conservative")
    }

    pub fn conservative_adapter_requirements_digest() -> BuildDigest {
        BuildDigest::of(b"link-plan-gate-v1:adapter-requirements-conservative")
    }

    pub fn conservative_patch_compatibility_digest() -> BuildDigest {
        BuildDigest::of(b"link-plan-gate-v1:patch-compatibility-conservative")
    }
}

impl LinkPlanReusePolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConservativeRebuild => "conservative_rebuild",
            Self::VerifiedReusable => "verified_reusable",
        }
    }

    pub const fn wire_tag(self) -> u8 {
        match self {
            Self::ConservativeRebuild => 0,
            Self::VerifiedReusable => 1,
        }
    }

    pub const fn from_wire_tag(tag: u8) -> Result<Self, AwboError> {
        match tag {
            0 => Ok(Self::ConservativeRebuild),
            1 => Ok(Self::VerifiedReusable),
            _ => Err(AwboError::UnsupportedWireTag {
                domain: "link plan reuse policy",
                tag,
            }),
        }
    }
}

fn put_optional_stable_span_digest(bytes: &mut Vec<u8>, span: Option<StableSourceSpanObject>) {
    if let Some(span) = span {
        put_string(bytes, "some");
        put_stable_span_digest(bytes, span);
    } else {
        put_string(bytes, "none");
    }
}

fn put_stable_span_digest(bytes: &mut Vec<u8>, span: StableSourceSpanObject) {
    put_u32(bytes, span.range.start);
    put_u32(bytes, span.range.end);
    put_u32(bytes, span.start_line);
    put_u32(bytes, span.start_column);
    put_u32(bytes, span.end_line);
    put_u32(bytes, span.end_column);
}

impl PublicSymbolKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flow => "flow",
            Self::Function => "function",
            Self::Declaration => "declaration",
        }
    }

    pub const fn wire_tag(self) -> u8 {
        match self {
            Self::Flow => 0,
            Self::Function => 1,
            Self::Declaration => 2,
        }
    }

    pub const fn from_wire_tag(tag: u8) -> Result<Self, AwboError> {
        match tag {
            0 => Ok(Self::Flow),
            1 => Ok(Self::Function),
            2 => Ok(Self::Declaration),
            _ => Err(AwboError::UnsupportedWireTag {
                domain: "public symbol kind",
                tag,
            }),
        }
    }
}

impl StableRangeObject {
    pub fn validate(self) -> Result<(), AwboError> {
        if self.start <= self.end {
            Ok(())
        } else {
            Err(AwboError::MalformedPayload {
                reason: "source range has start after end".to_owned(),
            })
        }
    }
}

impl StableSourceSpanObject {
    pub fn validate(self) -> Result<(), AwboError> {
        self.range.validate()?;
        if self.start_line > self.end_line {
            return Err(AwboError::MalformedPayload {
                reason: "source span line range has start after end".to_owned(),
            });
        }
        if self.start_line == self.end_line && self.start_column > self.end_column {
            return Err(AwboError::MalformedPayload {
                reason: "source span column range has start after end".to_owned(),
            });
        }
        Ok(())
    }
}

impl StableDiagnosticSeverity {
    pub const fn wire_tag(self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Warning => 1,
            Self::Info => 2,
            Self::Note => 3,
        }
    }

    pub const fn from_wire_tag(tag: u8) -> Result<Self, AwboError> {
        match tag {
            0 => Ok(Self::Error),
            1 => Ok(Self::Warning),
            2 => Ok(Self::Info),
            3 => Ok(Self::Note),
            _ => Err(AwboError::UnsupportedWireTag {
                domain: "stable diagnostic severity",
                tag,
            }),
        }
    }
}

impl StableDiagnosticSummaryObject {
    pub fn new(diagnostics: Vec<StableDiagnosticObject>) -> Result<Self, AwboError> {
        let error_count = count_severity(&diagnostics, StableDiagnosticSeverity::Error)?;
        let warning_count = count_severity(&diagnostics, StableDiagnosticSeverity::Warning)?;
        let info_count = count_severity(&diagnostics, StableDiagnosticSeverity::Info)?;
        let note_count = count_severity(&diagnostics, StableDiagnosticSeverity::Note)?;
        Ok(Self {
            error_count,
            warning_count,
            info_count,
            note_count,
            diagnostics,
        })
    }

    pub fn empty() -> Self {
        Self {
            error_count: 0,
            warning_count: 0,
            info_count: 0,
            note_count: 0,
            diagnostics: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), AwboError> {
        for diagnostic in &self.diagnostics {
            if let Some(span) = diagnostic.primary_span {
                span.validate()?;
            }
            for span in &diagnostic.related_spans {
                span.validate()?;
            }
        }
        let expected = Self::new(self.diagnostics.clone())?;
        if self.error_count != expected.error_count
            || self.warning_count != expected.warning_count
            || self.info_count != expected.info_count
            || self.note_count != expected.note_count
        {
            return Err(AwboError::MalformedPayload {
                reason: "diagnostic summary counts do not match diagnostics".to_owned(),
            });
        }
        Ok(())
    }
}

fn validate_version(actual: u32) -> Result<(), AwboError> {
    if actual == AWBO_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(AwboError::PayloadSchemaMismatch {
            actual,
            expected: AWBO_SCHEMA_VERSION,
        })
    }
}

fn count_severity(
    diagnostics: &[StableDiagnosticObject],
    severity: StableDiagnosticSeverity,
) -> Result<u32, AwboError> {
    u32::try_from(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == severity)
            .count(),
    )
    .map_err(|_| AwboError::PayloadTooLarge {
        field: "diagnostics",
    })
}

fn has_duplicate_public_symbol_descriptor(symbols: &[PublicSymbolObject]) -> bool {
    symbols
        .windows(2)
        .any(|window| window[0].kind == window[1].kind && window[0].name == window[1].name)
}

fn has_duplicate_named_digest(values: &[NamedDigest]) -> bool {
    values
        .windows(2)
        .any(|window| window[0].name() == window[1].name())
}
