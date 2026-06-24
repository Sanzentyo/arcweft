//! Compiler-private persistent object contracts for `.awbo` cache payloads.
//!
//! `.awbo` objects are not a public compatibility format. They are scoped by
//! cache schema, exact compiler identity where required, query inputs, and
//! dependency digests so corrupt or stale records can be treated as soft cache
//! misses by adapter crates.

use crate::fingerprint::{
    BuildDigest, NamedDigest, put_digest, put_named_digests, put_string, put_string_vec, put_u32,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const AWBO_MAGIC: [u8; 8] = *b"AWBO\r\n\x1a\n";
pub const AWBO_SCHEMA_VERSION: u32 = crate::incremental::CACHE_SCHEMA_VERSION;

/// Persistable compiler-private object family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilerObjectKind {
    ParsedSyntax,
    InterfaceSummary,
    HirBody,
    LineTaskEvidence,
    RuntimePlanUnit,
    BytecodeUnit,
    LinkPlan,
}

/// Whether an object may cross exact compiler identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilerObjectStability {
    CrossCompiler,
    ExactCompilerIdentity,
}

/// Exact compiler identity recorded in compiler-private cache keys.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct CompilerBuildIdentity {
    pub package_version: String,
    pub git_commit: String,
    pub rustc: String,
    pub target: String,
    pub enabled_features: Vec<String>,
}

/// Canonical key material for a compiler object.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct CompilerObjectKey {
    pub kind: CompilerObjectKind,
    pub compiler: CompilerBuildIdentity,
    pub source_digest: BuildDigest,
    pub query_options_digest: BuildDigest,
    pub dependency_interface_digests: Vec<NamedDigest>,
    pub dependency_body_digests: Vec<NamedDigest>,
    pub environment_digest: BuildDigest,
}

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ParsedSyntaxObject {
    pub source_label: String,
    pub source_digest: BuildDigest,
    pub stats: SyntaxStatsObject,
    pub diagnostics: Vec<StableDiagnosticObject>,
    pub cst_bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyntaxStatsObject {
    pub bytes: u64,
    pub lines: u64,
    pub items: u64,
    pub expressions: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StableDiagnosticObject {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub range: Option<StableRangeObject>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StableRangeObject {
    pub start: u32,
    pub end: u32,
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
pub struct HirBodyObject {
    pub module: String,
    pub body_digest: BuildDigest,
    pub private_payload: Vec<u8>,
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

/// `.awbo` validation error.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AwboError {
    #[error("AWBO magic does not match")]
    BadMagic,
    #[error("unsupported AWBO schema version {actual}; expected {expected}")]
    UnsupportedSchema { actual: u32, expected: u32 },
    #[error("AWBO payload kind {payload:?} does not match key kind {key:?}")]
    KindMismatch {
        key: CompilerObjectKind,
        payload: CompilerObjectKind,
    },
    #[error("AWBO key digest mismatch")]
    KeyDigestMismatch,
    #[error("AWBO payload digest mismatch")]
    PayloadDigestMismatch,
    #[error("AWBO payload length mismatch: expected {expected}, actual {actual}")]
    PayloadLengthMismatch { expected: u64, actual: u64 },
    #[error("AWBO payload is too large to encode length")]
    PayloadTooLarge,
}

impl CompilerObjectKind {
    pub const fn stability(self) -> CompilerObjectStability {
        match self {
            Self::InterfaceSummary => CompilerObjectStability::CrossCompiler,
            Self::ParsedSyntax
            | Self::HirBody
            | Self::LineTaskEvidence
            | Self::RuntimePlanUnit
            | Self::BytecodeUnit
            | Self::LinkPlan => CompilerObjectStability::ExactCompilerIdentity,
        }
    }

    pub const fn cache_namespace(self) -> &'static str {
        match self {
            Self::ParsedSyntax => "parsed-syntax",
            Self::InterfaceSummary => "interface-summary",
            Self::HirBody => "hir-body",
            Self::LineTaskEvidence => "line-task-evidence",
            Self::RuntimePlanUnit => "runtime-plan-unit",
            Self::BytecodeUnit => "bytecode-unit",
            Self::LinkPlan => "link-plan",
        }
    }
}

impl CompilerBuildIdentity {
    #[must_use]
    pub fn canonicalized(mut self) -> Self {
        self.enabled_features.sort();
        self.enabled_features.dedup();
        self
    }
}

impl CompilerObjectKey {
    #[must_use]
    pub fn canonicalized(mut self) -> Self {
        self.compiler = self.compiler.canonicalized();
        self.dependency_interface_digests =
            NamedDigest::canonicalize(self.dependency_interface_digests);
        self.dependency_body_digests = NamedDigest::canonicalize(self.dependency_body_digests);
        self
    }

    pub fn digest(&self) -> BuildDigest {
        let key = self.clone().canonicalized();
        let mut bytes = Vec::new();
        put_u32(&mut bytes, AWBO_SCHEMA_VERSION);
        put_string(&mut bytes, key.kind.cache_namespace());
        put_string(&mut bytes, &key.compiler.package_version);
        put_string(&mut bytes, &key.compiler.git_commit);
        put_string(&mut bytes, &key.compiler.rustc);
        put_string(&mut bytes, &key.compiler.target);
        put_string_vec(&mut bytes, &key.compiler.enabled_features);
        put_digest(&mut bytes, key.source_digest);
        put_digest(&mut bytes, key.query_options_digest);
        put_named_digests(&mut bytes, &key.dependency_interface_digests);
        put_named_digests(&mut bytes, &key.dependency_body_digests);
        put_digest(&mut bytes, key.environment_digest);
        BuildDigest::of(&bytes)
    }
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
        let payload_len =
            u64::try_from(payload.payload_len()).map_err(|_| AwboError::PayloadTooLarge)?;
        Ok(Self {
            magic: AWBO_MAGIC,
            schema_version: AWBO_SCHEMA_VERSION,
            kind: key.kind,
            stability: key.kind.stability(),
            key_digest: key.digest(),
            payload_digest: payload.digest(),
            payload_len,
            payload,
        })
    }

    pub fn validate(&self, key: &CompilerObjectKey) -> Result<(), AwboError> {
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
        if self.kind != key.kind || payload_kind != key.kind {
            return Err(AwboError::KindMismatch {
                key: key.kind,
                payload: payload_kind,
            });
        }
        if self.stability != key.kind.stability() {
            return Err(AwboError::KindMismatch {
                key: key.kind,
                payload: payload_kind,
            });
        }
        if self.key_digest != key.digest() {
            return Err(AwboError::KeyDigestMismatch);
        }
        if self.payload_digest != self.payload.digest() {
            return Err(AwboError::PayloadDigestMismatch);
        }
        let actual =
            u64::try_from(self.payload.payload_len()).map_err(|_| AwboError::PayloadTooLarge)?;
        if self.payload_len != actual {
            return Err(AwboError::PayloadLengthMismatch {
                expected: self.payload_len,
                actual,
            });
        }
        Ok(())
    }
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

    pub fn digest(&self) -> BuildDigest {
        let mut bytes = Vec::new();
        self.put_canonical_bytes(&mut bytes);
        BuildDigest::of(&bytes)
    }

    pub fn payload_len(&self) -> usize {
        match self {
            Self::ParsedSyntax(value) => {
                value.cst_bytes.len()
                    + value
                        .diagnostics
                        .iter()
                        .map(|diagnostic| {
                            diagnostic.code.len()
                                + diagnostic.severity.len()
                                + diagnostic.message.len()
                        })
                        .sum::<usize>()
            }
            Self::InterfaceSummary(value) => value
                .public_symbols
                .iter()
                .map(|symbol| symbol.name.len() + symbol.kind.len())
                .sum(),
            Self::HirBody(value) => value.private_payload.len(),
            Self::LineTaskEvidence(value) => value.task_groups.iter().map(String::len).sum(),
            Self::RuntimePlanUnit(value) => value.payload.len(),
            Self::BytecodeUnit(value) => value.payload.len(),
            Self::LinkPlan(value) => {
                value.entrypoints.iter().map(String::len).sum::<usize>()
                    + value.unit_digests.len() * 32
            }
        }
    }

    fn put_canonical_bytes(&self, out: &mut Vec<u8>) {
        put_string(out, self.kind().cache_namespace());
        match self {
            Self::ParsedSyntax(value) => {
                put_string(out, &value.source_label);
                put_digest(out, value.source_digest);
                put_u64(out, value.stats.bytes);
                put_u64(out, value.stats.lines);
                put_u64(out, value.stats.items);
                put_u64(out, value.stats.expressions);
                put_stable_diagnostics(out, &value.diagnostics);
                put_bytes(out, &value.cst_bytes);
            }
            Self::InterfaceSummary(value) => {
                put_string(out, &value.module);
                put_digest(out, value.exports_digest);
                put_digest(out, value.imports_digest);
                put_u32(
                    out,
                    u32::try_from(value.public_symbols.len())
                        .expect("public symbol count fits u32"),
                );
                for symbol in &value.public_symbols {
                    put_string(out, &symbol.name);
                    put_string(out, &symbol.kind);
                    put_digest(out, symbol.signature_digest);
                }
            }
            Self::HirBody(value) => {
                put_string(out, &value.module);
                put_digest(out, value.body_digest);
                put_bytes(out, &value.private_payload);
            }
            Self::LineTaskEvidence(value) => {
                put_string(out, &value.module);
                put_digest(out, value.evidence_digest);
                put_string_vec(out, &value.task_groups);
            }
            Self::RuntimePlanUnit(value) => {
                put_string(out, &value.module);
                put_digest(out, value.runtime_ir_digest);
                put_bytes(out, &value.payload);
            }
            Self::BytecodeUnit(value) => {
                put_string(out, &value.module);
                put_digest(out, value.awbc_digest);
                put_bytes(out, &value.payload);
            }
            Self::LinkPlan(value) => {
                put_string_vec(out, &value.entrypoints);
                put_u32(
                    out,
                    u32::try_from(value.unit_digests.len()).expect("unit digest count fits u32"),
                );
                for (unit, digest) in &value.unit_digests {
                    put_string(out, unit);
                    put_digest(out, *digest);
                }
                put_digest(out, value.link_digest);
            }
        }
    }
}

fn put_stable_diagnostics(out: &mut Vec<u8>, diagnostics: &[StableDiagnosticObject]) {
    put_u32(
        out,
        u32::try_from(diagnostics.len()).expect("diagnostic count fits u32"),
    );
    for diagnostic in diagnostics {
        put_string(out, &diagnostic.code);
        put_string(out, &diagnostic.severity);
        put_string(out, &diagnostic.message);
        match diagnostic.range {
            Some(range) => {
                out.push(1);
                put_u32(out, range.start);
                put_u32(out, range.end);
            }
            None => out.push(0),
        }
    }
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).expect("canonical byte payload length fits u32");
    put_u32(out, len);
    out.extend_from_slice(bytes);
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
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

    fn parsed_payload() -> CompilerObjectPayload {
        CompilerObjectPayload::ParsedSyntax(ParsedSyntaxObject {
            source_label: "src/main.arcw".to_owned(),
            source_digest: digest("source"),
            stats: SyntaxStatsObject {
                bytes: 16,
                lines: 1,
                items: 1,
                expressions: 1,
            },
            diagnostics: vec![StableDiagnosticObject {
                code: "syntax.test".to_owned(),
                severity: "warning".to_owned(),
                message: "synthetic".to_owned(),
                range: Some(StableRangeObject { start: 0, end: 4 }),
            }],
            cst_bytes: b"cst".to_vec(),
        })
    }

    #[test]
    fn compiler_object_kind_reports_stability_and_namespace() {
        assert_eq!(
            CompilerObjectKind::InterfaceSummary.stability(),
            CompilerObjectStability::CrossCompiler
        );
        assert_eq!(
            CompilerObjectKind::ParsedSyntax.cache_namespace(),
            "parsed-syntax"
        );
    }

    #[test]
    fn compiler_object_key_digest_canonicalizes_features_and_dependencies() {
        let first = key(CompilerObjectKind::ParsedSyntax);
        let mut second = first.clone();
        second.compiler.enabled_features = vec!["a".to_owned(), "b".to_owned()];
        second.dependency_interface_digests.reverse();

        assert_eq!(first.digest(), second.digest());
    }

    #[test]
    fn awbo_envelope_validates_key_payload_digest_and_length() {
        let key = key(CompilerObjectKind::ParsedSyntax);
        let envelope = AwboEnvelope::new(&key, parsed_payload()).expect("envelope builds");

        envelope.validate(&key).expect("envelope validates");

        let mut bad_payload = envelope.clone();
        bad_payload.payload_digest = digest("bad-payload");
        assert_eq!(
            bad_payload.validate(&key),
            Err(AwboError::PayloadDigestMismatch)
        );

        let mut bad_len = envelope;
        bad_len.payload_len += 1;
        assert_eq!(
            bad_len.validate(&key),
            Err(AwboError::PayloadLengthMismatch {
                expected: bad_len.payload_len,
                actual: parsed_payload().payload_len() as u64,
            })
        );
    }

    #[test]
    fn awbo_envelope_rejects_kind_mismatch() {
        let error = AwboEnvelope::new(&key(CompilerObjectKind::InterfaceSummary), parsed_payload())
            .expect_err("kind mismatch rejects");

        assert_eq!(
            error,
            AwboError::KindMismatch {
                key: CompilerObjectKind::InterfaceSummary,
                payload: CompilerObjectKind::ParsedSyntax,
            }
        );
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

        assert_eq!(first.digest(), second.digest());
    }
}
