use super::schema::{
    AWBO_SCHEMA_VERSION, AwboError, CompilerIdentityNamespaceObject, CompilerObjectKey,
    CompilerObjectKind, CompilerStageInputsObject,
};
use crate::fingerprint::BuildDigest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Compiler-private object payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CompilerObjectPayload {
    ParsedSyntax(ParsedSyntaxObject),
    InterfaceSummary(InterfaceSummaryObject),
    HirBody(HirBodyObject),
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
    pub bytes: u64,
    pub lines: u64,
    pub cst_lex_passes: u64,
    pub punctuation_scans: u64,
    pub punctuation_scan_bytes: u64,
    pub line_owned_bytes: u64,
    pub block_owned_bytes: u64,
    pub raw_owned_bytes: u64,
    pub wiki_scan_performed: u64,
    pub dot_normalization_owned: u64,
    pub dialogue_rescue_expr_parse_attempts: u64,
    pub numeric_seq_summaries: u64,
}

/// Compact deterministic parse evidence for later cache validation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ParsedSyntaxEvidenceObject {
    pub root_kind: String,
    pub cst_shape_digest: BuildDigest,
    pub line_index_digest: BuildDigest,
    pub cst_node_count: u64,
    pub cst_token_count: u64,
    pub cst_error_node_count: u64,
    pub typed_attribute_count: u64,
    pub typed_use_count: u64,
    pub typed_item_count: u64,
    pub wiki_link_count: u64,
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
    pub agent_count: u64,
    pub declaration_count: u64,
    pub top_level_item_count: u64,
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
    pub module: String,
    pub exports_digest: BuildDigest,
    pub imports_digest: BuildDigest,
    pub public_symbols: Vec<PublicSymbolObject>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PublicSymbolObject {
    pub name: String,
    pub kind: String,
    pub signature_digest: BuildDigest,
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
pub struct BytecodeUnitObject {
    pub module: String,
    pub awbc_digest: BuildDigest,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LinkPlanObject {
    pub entrypoints: Vec<String>,
    pub unit_digests: BTreeMap<String, BuildDigest>,
    pub link_digest: BuildDigest,
}

impl CompilerObjectPayload {
    pub const fn kind(&self) -> CompilerObjectKind {
        match self {
            Self::ParsedSyntax(_) => CompilerObjectKind::ParsedSyntax,
            Self::InterfaceSummary(_) => CompilerObjectKind::InterfaceSummary,
            Self::HirBody(_) => CompilerObjectKind::HirBody,
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
            Self::HirBody(value) => value.validate_for_key(key),
            Self::InterfaceSummary(_)
            | Self::LineTaskEvidence(_)
            | Self::RuntimePlanUnit(_)
            | Self::BytecodeUnit(_)
            | Self::LinkPlan(_) => Ok(()),
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
