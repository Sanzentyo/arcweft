#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2021"
---
use std::{collections::BTreeMap, env, fs, path::{Path, PathBuf}, process};

struct Edit {
    path: &'static str,
    needle: &'static str,
    replacement: &'static str,
}

fn main() {
    let mut root = PathBuf::from(".");
    let mut apply = false;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => {
                let Some(value) = args.next() else {
                    eprintln!("error: --root requires a path");
                    process::exit(2);
                };
                root = PathBuf::from(value);
            }
            "--apply" | "-a" => apply = true,
            "--help" | "-h" => {
                println!("usage: cargo +nightly -Zscript apply-seq04-6-typecheck-gate.rs --root <repo> [--apply]");
                println!("default mode is dry-run; pass --apply to write files");
                return;
            }
            other => {
                eprintln!("error: unexpected argument {other}");
                process::exit(2);
            }
        }
    }

    let mut file_states: BTreeMap<&'static str, (PathBuf, String, String)> = BTreeMap::new();
    for edit in edits() {
        let state = file_states.entry(edit.path).or_insert_with(|| {
            let path = root.join(edit.path);
            let before = read_utf8(&path);
            (path, before.clone(), before)
        });
        state.2 = replace_once(edit.path, &state.2, edit.needle, edit.replacement);
    }

    let mut changed_paths = Vec::new();
    for (relative_path, (path, before, after)) in file_states {
        if before != after {
            changed_paths.push(relative_path);
            if apply {
                fs::write(&path, after).unwrap_or_else(|error| {
                    eprintln!("error: failed to write {}: {error}", path.display());
                    process::exit(1);
                });
            }
        }
    }

    if apply {
        println!("applied seq04.6 typecheck gate edits to {} files", changed_paths.len());
    } else {
        println!("dry-run ok: {} files would change", changed_paths.len());
        println!("rerun with --apply to write files");
    }
    for path in changed_paths {
        println!("- {path}");
    }
}

fn read_utf8(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| {
        eprintln!("error: failed to read {}: {error}", path.display());
        process::exit(1);
    })
}

fn replace_once(path: &str, source: &str, needle: &str, replacement: &str) -> String {
    let count = source.matches(needle).count();
    if count != 1 {
        eprintln!(
            "error: expected exactly one anchor in {path}, found {count}\nanchor:\n{needle}"
        );
        process::exit(1);
    }
    source.replacen(needle, replacement, 1)
}

fn edits() -> Vec<Edit> {
    vec![
        Edit {
            path: "crates/arcweft-project/src/persistent_object.rs",
            needle: r#"    BytecodeUnitObject, CompilerObjectPayload, HirBodyFactsObject, HirBodyObject,
    InterfaceSummaryObject, LineTaskEvidenceObject, LinkPlanObject, ParsedSyntaxEvidenceObject,
    ParsedSyntaxObject, PublicSymbolKind, PublicSymbolObject, RuntimePlanUnitObject,
    StableDiagnosticObject, StableDiagnosticSeverity, StableDiagnosticSummaryObject,
    StableRangeObject, StableSourceSpanObject, SyntaxStatsObject,
"#,
            replacement: r#"    BytecodeUnitObject, CompilerObjectPayload, HirBodyFactsObject, HirBodyObject,
    InterfaceSummaryObject, LineTaskEvidenceObject, LinkPlanObject, ParsedSyntaxEvidenceObject,
    ParsedSyntaxObject, PublicSymbolKind, PublicSymbolObject, RuntimePlanUnitObject,
    StableDiagnosticObject, StableDiagnosticSeverity, StableDiagnosticSummaryObject,
    StableRangeObject, StableSourceSpanObject, SyntaxStatsObject, TypecheckGateFactsObject,
    TypecheckGateObject, TypecheckGateReusePolicy,
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/schema.rs",
            needle: r#"    incremental::QueryKind,
"#,
            replacement: r#"    incremental::{CacheRecordStatus, InvalidationReason, QueryKind},
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/schema.rs",
            needle: r#"    InterfaceSummary,
    HirBody,
    LineTaskEvidence,
"#,
            replacement: r#"    InterfaceSummary,
    HirBody,
    TypecheckGate,
    LineTaskEvidence,
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/schema.rs",
            needle: r#"            Self::ParsedSyntax
            | Self::HirBody
            | Self::LineTaskEvidence
"#,
            replacement: r#"            Self::ParsedSyntax
            | Self::HirBody
            | Self::TypecheckGate
            | Self::LineTaskEvidence
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/schema.rs",
            needle: r#"            Self::InterfaceSummary => "interface-summary",
            Self::HirBody => "hir-body",
            Self::LineTaskEvidence => "line-task-evidence",
"#,
            replacement: r#"            Self::InterfaceSummary => "interface-summary",
            Self::HirBody => "hir-body",
            Self::TypecheckGate => "typecheck-gate",
            Self::LineTaskEvidence => "line-task-evidence",
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/schema.rs",
            needle: r#"            Self::ParsedSyntax => Some(QueryKind::Parse),
            Self::InterfaceSummary => Some(QueryKind::Interface),
            Self::HirBody => Some(QueryKind::HirBody),
            Self::LineTaskEvidence
"#,
            replacement: r#"            Self::ParsedSyntax => Some(QueryKind::Parse),
            Self::InterfaceSummary => Some(QueryKind::Interface),
            Self::HirBody => Some(QueryKind::HirBody),
            Self::TypecheckGate => Some(QueryKind::TypeCheck),
            Self::LineTaskEvidence
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/schema.rs",
            needle: r#"            Self::ParsedSyntax => Some(ArtifactKind::ParsedSyntax),
            Self::InterfaceSummary => Some(ArtifactKind::InterfaceSummary),
            Self::HirBody => Some(ArtifactKind::HirBody),
            Self::LineTaskEvidence
"#,
            replacement: r#"            Self::ParsedSyntax => Some(ArtifactKind::ParsedSyntax),
            Self::InterfaceSummary => Some(ArtifactKind::InterfaceSummary),
            Self::HirBody => Some(ArtifactKind::HirBody),
            Self::TypecheckGate => Some(ArtifactKind::TypeCheckReport),
            Self::LineTaskEvidence
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/schema.rs",
            needle: r#"    /// Reverse mapping for object families enabled for safe read-through.
    pub const fn from_safe_read_through_artifact_kind(artifact_kind: ArtifactKind) -> Option<Self> {
        match artifact_kind {
            ArtifactKind::ParsedSyntax => Some(Self::ParsedSyntax),
            ArtifactKind::InterfaceSummary => Some(Self::InterfaceSummary),
            ArtifactKind::HirBody => Some(Self::HirBody),
            ArtifactKind::TypeCheckReport
            | ArtifactKind::RuntimePlan
            | ArtifactKind::BytecodeUnit
            | ArtifactKind::AssetMetadata
            | ArtifactKind::AssetPayload
            | ArtifactKind::LinkPlan
            | ArtifactKind::BundleSection
            | ArtifactKind::BundleIndex => None,
        }
    }

    pub const fn wire_tag(self) -> u8 {
"#,
            replacement: r#"    /// Reverse mapping for object families enabled for safe read-through.
    pub const fn from_safe_read_through_artifact_kind(artifact_kind: ArtifactKind) -> Option<Self> {
        match artifact_kind {
            ArtifactKind::ParsedSyntax => Some(Self::ParsedSyntax),
            ArtifactKind::InterfaceSummary => Some(Self::InterfaceSummary),
            ArtifactKind::HirBody => Some(Self::HirBody),
            ArtifactKind::TypeCheckReport => Some(Self::TypecheckGate),
            ArtifactKind::RuntimePlan
            | ArtifactKind::BytecodeUnit
            | ArtifactKind::AssetMetadata
            | ArtifactKind::AssetPayload
            | ArtifactKind::LinkPlan
            | ArtifactKind::BundleSection
            | ArtifactKind::BundleIndex => None,
        }
    }

    pub const TYPECHECK_GATE_CONSERVATIVE_POLICY: &'static str =
        "typecheck-gate-valid-but-linked-sema-rebuilt";

    pub fn read_through_hit_status(self) -> CacheRecordStatus {
        if self.read_through_hit_requires_rebuild() {
            CacheRecordStatus::HitThenRebuilt {
                reason: InvalidationReason::ConservativeInvalidation {
                    policy: Self::TYPECHECK_GATE_CONSERVATIVE_POLICY.to_owned(),
                },
            }
        } else {
            CacheRecordStatus::Hit
        }
    }

    pub const fn read_through_hit_requires_rebuild(self) -> bool {
        matches!(self, Self::TypecheckGate)
    }

    pub const fn conservative_read_through_policy(self) -> Option<&'static str> {
        match self {
            Self::TypecheckGate => Some(Self::TYPECHECK_GATE_CONSERVATIVE_POLICY),
            Self::ParsedSyntax
            | Self::InterfaceSummary
            | Self::HirBody
            | Self::LineTaskEvidence
            | Self::RuntimePlanUnit
            | Self::BytecodeUnit
            | Self::LinkPlan => None,
        }
    }

    pub const fn wire_tag(self) -> u8 {
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/schema.rs",
            needle: r#"            Self::InterfaceSummary => 1,
            Self::HirBody => 2,
            Self::LineTaskEvidence => 3,
"#,
            replacement: r#"            Self::InterfaceSummary => 1,
            Self::HirBody => 2,
            Self::TypecheckGate => 7,
            Self::LineTaskEvidence => 3,
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/schema.rs",
            needle: r#"            1 => Ok(Self::InterfaceSummary),
            2 => Ok(Self::HirBody),
            3 => Ok(Self::LineTaskEvidence),
"#,
            replacement: r#"            1 => Ok(Self::InterfaceSummary),
            2 => Ok(Self::HirBody),
            7 => Ok(Self::TypecheckGate),
            3 => Ok(Self::LineTaskEvidence),
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/payload.rs",
            needle: r#"    HirBody(HirBodyObject),
    LineTaskEvidence(LineTaskEvidenceObject),
"#,
            replacement: r#"    HirBody(HirBodyObject),
    TypecheckGate(TypecheckGateObject),
    LineTaskEvidence(LineTaskEvidenceObject),
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/payload.rs",
            needle: r#"pub struct PublicSymbolObject {
    pub name: String,
    pub kind: PublicSymbolKind,
    pub signature_digest: BuildDigest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LineTaskEvidenceObject {
"#,
            replacement: r#"pub struct PublicSymbolObject {
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
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/payload.rs",
            needle: r#"            Self::InterfaceSummary(_) => CompilerObjectKind::InterfaceSummary,
            Self::HirBody(_) => CompilerObjectKind::HirBody,
            Self::LineTaskEvidence(_) => CompilerObjectKind::LineTaskEvidence,
"#,
            replacement: r#"            Self::InterfaceSummary(_) => CompilerObjectKind::InterfaceSummary,
            Self::HirBody(_) => CompilerObjectKind::HirBody,
            Self::TypecheckGate(_) => CompilerObjectKind::TypecheckGate,
            Self::LineTaskEvidence(_) => CompilerObjectKind::LineTaskEvidence,
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/payload.rs",
            needle: r#"            Self::ParsedSyntax(value) => value.validate_for_key(key),
            Self::InterfaceSummary(value) => value.validate_for_key(key),
            Self::HirBody(value) => value.validate_for_key(key),
            Self::LineTaskEvidence(_)
"#,
            replacement: r#"            Self::ParsedSyntax(value) => value.validate_for_key(key),
            Self::InterfaceSummary(value) => value.validate_for_key(key),
            Self::HirBody(value) => value.validate_for_key(key),
            Self::TypecheckGate(value) => value.validate_for_key(key),
            Self::LineTaskEvidence(_)
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/payload.rs",
            needle: r#"impl PublicSymbolKind {
"#,
            replacement: TYPECHECK_GATE_PAYLOAD_IMPL,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/codec.rs",
            needle: r#"    BytecodeUnitObject, CompilerObjectPayload, HirBodyFactsObject, HirBodyObject,
    InterfaceSummaryObject, LineTaskEvidenceObject, LinkPlanObject, ParsedSyntaxEvidenceObject,
    ParsedSyntaxObject, PublicSymbolKind, PublicSymbolObject, RuntimePlanUnitObject,
    StableDiagnosticObject, StableDiagnosticSeverity, StableDiagnosticSummaryObject,
    StableRangeObject, StableSourceSpanObject, SyntaxStatsObject,
"#,
            replacement: r#"    BytecodeUnitObject, CompilerObjectPayload, HirBodyFactsObject, HirBodyObject,
    InterfaceSummaryObject, LineTaskEvidenceObject, LinkPlanObject, ParsedSyntaxEvidenceObject,
    ParsedSyntaxObject, PublicSymbolKind, PublicSymbolObject, RuntimePlanUnitObject,
    StableDiagnosticObject, StableDiagnosticSeverity, StableDiagnosticSummaryObject,
    StableRangeObject, StableSourceSpanObject, SyntaxStatsObject, TypecheckGateFactsObject,
    TypecheckGateObject, TypecheckGateReusePolicy,
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/codec.rs",
            needle: r#"            Self::InterfaceSummary(value) => put_interface_summary(&mut writer, value)?,
            Self::HirBody(value) => put_hir_body(&mut writer, value)?,
            Self::LineTaskEvidence(value) => put_line_task_evidence(&mut writer, value)?,
"#,
            replacement: r#"            Self::InterfaceSummary(value) => put_interface_summary(&mut writer, value)?,
            Self::HirBody(value) => put_hir_body(&mut writer, value)?,
            Self::TypecheckGate(value) => put_typecheck_gate(&mut writer, value)?,
            Self::LineTaskEvidence(value) => put_line_task_evidence(&mut writer, value)?,
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/codec.rs",
            needle: r#"            CompilerObjectKind::HirBody => Self::HirBody(read_hir_body(&mut reader)?),
            CompilerObjectKind::LineTaskEvidence => {
"#,
            replacement: r#"            CompilerObjectKind::HirBody => Self::HirBody(read_hir_body(&mut reader)?),
            CompilerObjectKind::TypecheckGate => {
                Self::TypecheckGate(read_typecheck_gate(&mut reader)?)
            }
            CompilerObjectKind::LineTaskEvidence => {
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/codec.rs",
            needle: r#"            "interface-summary" => Ok(Self::InterfaceSummary),
            "hir-body" => Ok(Self::HirBody),
            "line-task-evidence" => Ok(Self::LineTaskEvidence),
"#,
            replacement: r#"            "interface-summary" => Ok(Self::InterfaceSummary),
            "hir-body" => Ok(Self::HirBody),
            "typecheck-gate" => Ok(Self::TypecheckGate),
            "line-task-evidence" => Ok(Self::LineTaskEvidence),
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/codec.rs",
            needle: r#"fn put_line_task_evidence(
"#,
            replacement: TYPECHECK_GATE_CODEC_IMPL,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"        CompilerObjectPayload, CompilerStageInputsObject, HirBodyObject, InterfaceSummaryObject,
        ParsedSyntaxObject,
"#,
            replacement: r#"        CompilerObjectPayload, CompilerStageInputsObject, HirBodyObject, InterfaceSummaryObject,
        ParsedSyntaxObject, TypecheckGateObject, TypecheckGateReusePolicy,
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};
"#,
            replacement: r#"use std::{
    collections::BTreeMap,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"    InterfaceSummary(InterfaceSummaryObject),
    HirBody(HirBodyObject),
}
"#,
            replacement: r#"    InterfaceSummary(InterfaceSummaryObject),
    HirBody(HirBodyObject),
    TypecheckGate(TypecheckGateObject),
}
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"    pub soft_miss_reason: Option<PersistentQueryMissReason>,
    pub recovery_action: PersistentQueryRecoveryAction,
}
"#,
            replacement: r#"    pub soft_miss_reason: Option<PersistentQueryMissReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typecheck_gate_reuse_policy: Option<TypecheckGateReusePolicy>,
    pub recovery_action: PersistentQueryRecoveryAction,
}
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"        match self {
            Self::Hit(_) => CacheRecordStatus::Hit,
            Self::Miss(miss) => CacheRecordStatus::Miss {
"#,
            replacement: r#"        match self {
            Self::Hit(hit) => hit.object_kind.read_through_hit_status(),
            Self::Miss(miss) => CacheRecordStatus::Miss {
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"            Self::DependencyInterfaceDigestMismatch { .. } => InvalidationReason::InterfaceChanged,
            Self::DependencyBodyDigestMismatch { .. } => InvalidationReason::BodyChanged,
"#,
            replacement: r#"            Self::DependencyInterfaceDigestMismatch { expected, actual } => {
                dependency_interface_mismatch_invalidation_reason(expected, actual)
            }
            Self::DependencyBodyDigestMismatch { expected, actual } => {
                dependency_body_mismatch_invalidation_reason(expected, actual)
            }
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"impl From<ErrorKind> for PersistentQueryIoKind {
"#,
            replacement: PERSISTENT_QUERY_DEP_HELPERS,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"        other => Err(miss(
"#,
            replacement: r#"        CompilerObjectPayload::TypecheckGate(payload) => {
            Ok(PersistentQueryHitPayload::TypecheckGate(payload.clone()))
        }
        other => Err(miss(
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"            cache_record_status: CacheRecordStatus::Hit,
            soft_miss_reason: None,
            recovery_action: PersistentQueryRecoveryAction::NoneRequired,
"#,
            replacement: r#"            cache_record_status: hit.object_kind.read_through_hit_status(),
            soft_miss_reason: None,
            typecheck_gate_reuse_policy: envelope.and_then(typecheck_gate_reuse_policy),
            recovery_action: if hit.object_kind.read_through_hit_requires_rebuild() {
                PersistentQueryRecoveryAction::RebuildFromSource
            } else {
                PersistentQueryRecoveryAction::NoneRequired
            },
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"                soft_miss_reason: Some(reason),
                recovery_action: PersistentQueryRecoveryAction::RebuildFromSource,
"#,
            replacement: r#"                soft_miss_reason: Some(reason),
                typecheck_gate_reuse_policy: envelope.and_then(typecheck_gate_reuse_policy),
                recovery_action: PersistentQueryRecoveryAction::RebuildFromSource,
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"        soft_miss_reason: Some(reason),
        recovery_action: PersistentQueryRecoveryAction::RebuildFromSource,
"#,
            replacement: r#"        soft_miss_reason: Some(reason),
        typecheck_gate_reuse_policy: None,
        recovery_action: PersistentQueryRecoveryAction::RebuildFromSource,
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"        CompilerObjectPayload::HirBody(payload) => CompilerObjectKey {
            kind: CompilerObjectKind::HirBody,
            compiler: payload.compiler_namespace.compiler.clone(),
            source_digest: payload.source_digest,
            query_options_digest: payload.stage_inputs.query_options_digest,
            dependency_interface_digests: payload.stage_inputs.dependency_interface_digests.clone(),
            dependency_body_digests: payload.stage_inputs.dependency_body_digests.clone(),
            environment_digest: payload.stage_inputs.environment_digest,
        },
        CompilerObjectPayload::LineTaskEvidence(_)
"#,
            replacement: r#"        CompilerObjectPayload::HirBody(payload) => CompilerObjectKey {
            kind: CompilerObjectKind::HirBody,
            compiler: payload.compiler_namespace.compiler.clone(),
            source_digest: payload.source_digest,
            query_options_digest: payload.stage_inputs.query_options_digest,
            dependency_interface_digests: payload.stage_inputs.dependency_interface_digests.clone(),
            dependency_body_digests: payload.stage_inputs.dependency_body_digests.clone(),
            environment_digest: payload.stage_inputs.environment_digest,
        },
        CompilerObjectPayload::TypecheckGate(payload) => CompilerObjectKey {
            kind: CompilerObjectKind::TypecheckGate,
            compiler: payload.compiler_namespace.compiler.clone(),
            source_digest: payload.source_digest,
            query_options_digest: payload.stage_inputs.query_options_digest,
            dependency_interface_digests: payload.stage_inputs.dependency_interface_digests.clone(),
            dependency_body_digests: payload.stage_inputs.dependency_body_digests.clone(),
            environment_digest: payload.stage_inputs.environment_digest,
        },
        CompilerObjectPayload::LineTaskEvidence(_)
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"        CompilerObjectPayload::HirBody(value) => Some(value.schema_version),
        CompilerObjectPayload::LineTaskEvidence(_)
"#,
            replacement: r#"        CompilerObjectPayload::HirBody(value) => Some(value.schema_version),
        CompilerObjectPayload::TypecheckGate(value) => Some(value.schema_version),
        CompilerObjectPayload::LineTaskEvidence(_)
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"        CompilerObjectPayload::HirBody(payload) => {
            validate_hir_body_payload(payload, key)?;
        }
        other => {
"#,
            replacement: r#"        CompilerObjectPayload::HirBody(payload) => {
            validate_hir_body_payload(payload, key)?;
        }
        CompilerObjectPayload::TypecheckGate(payload) => {
            validate_typecheck_gate_payload(payload, key)?;
        }
        other => {
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query.rs",
            needle: r#"fn validate_interface_summary_payload(
"#,
            replacement: PERSISTENT_QUERY_TYPECHECK_VALIDATOR,
        },
        Edit {
            path: "crates/arcweft-compiler/src/persistent.rs",
            needle: r#"        StableDiagnosticSeverity, StableDiagnosticSummaryObject, StableRangeObject,
        StableSourceSpanObject, SyntaxStatsObject,
"#,
            replacement: r#"        StableDiagnosticSeverity, StableDiagnosticSummaryObject, StableRangeObject,
        StableSourceSpanObject, SyntaxStatsObject, TypecheckGateFactsObject, TypecheckGateObject,
        TypecheckGateReusePolicy,
"#,
        },
        Edit {
            path: "crates/arcweft-compiler/src/persistent.rs",
            needle: r#"pub struct InterfaceSummaryFactsInput<'a> {
    pub key: &'a CompilerObjectKey,
    pub module: &'a str,
    pub parsed: &'a ParsedSource,
    pub hir: &'a HirModule,
}

/// Failure while projecting compiler internals into stable persistent facts.
"#,
            replacement: r#"pub struct InterfaceSummaryFactsInput<'a> {
    pub key: &'a CompilerObjectKey,
    pub module: &'a str,
    pub parsed: &'a ParsedSource,
    pub hir: &'a HirModule,
}

/// Inputs required to project stable typecheck gate facts.
pub struct TypecheckGateFactsInput<'a> {
    pub key: &'a CompilerObjectKey,
    pub module: &'a str,
    pub parsed: &'a ParsedSource,
    pub interface_summary: &'a InterfaceSummaryObject,
    pub hir_body: &'a HirBodyObject,
}

/// Failure while projecting compiler internals into stable persistent facts.
"#,
        },
        Edit {
            path: "crates/arcweft-compiler/src/persistent.rs",
            needle: r#"    #[error("{field} length does not fit the stable payload count type")]
    CountTooLarge { field: &'static str },
}
"#,
            replacement: r#"    #[error("{field} length does not fit the stable payload count type")]
    CountTooLarge { field: &'static str },
    #[error("{field} source digest does not match typecheck gate key")]
    SourceDigestMismatch { field: &'static str },
}
"#,
        },
        Edit {
            path: "crates/arcweft-compiler/src/persistent.rs",
            needle: r#"fn ensure_key_kind(
"#,
            replacement: TYPECHECK_GATE_COMPILER_IMPL,
        },
        Edit {
            path: "crates/arcweft-cli/src/app/cache.rs",
            needle: r#"                if let Some(payload_kind) = evidence.payload_kind {
                    println!("  payload kind: {payload_kind:?}");
                }
"#,
            replacement: r#"                if let Some(payload_kind) = evidence.payload_kind {
                    println!("  payload kind: {payload_kind:?}");
                }
                if let Some(policy) = evidence.typecheck_gate_reuse_policy {
                    println!("  typecheck gate policy: {}", policy.as_str());
                }
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/incremental.rs",
            needle: r#"        assert!(
            QueryKind::RuntimePlan
                .dependency_scope()
                .requires_body_digests()
        );
"#,
            replacement: r#"        assert!(
            QueryKind::RuntimePlan
                .dependency_scope()
                .requires_body_digests()
        );
        assert_eq!(QueryKind::TypeCheck.artifact_kind(), crate::artifact::ArtifactKind::TypeCheckReport);
        assert!(
            !QueryKind::TypeCheck
                .dependency_scope()
                .requires_body_digests()
        );
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/codec.rs",
            needle: r#"    fn interface_payload_for(key: &CompilerObjectKey) -> CompilerObjectPayload {
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
"#,
            replacement: CODEC_TYPECHECK_TEST_HELPER,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/codec.rs",
            needle: r#"            CompilerObjectKind::InterfaceSummary,
            CompilerObjectKind::HirBody,
"#,
            replacement: r#"            CompilerObjectKind::InterfaceSummary,
            CompilerObjectKind::HirBody,
            CompilerObjectKind::TypecheckGate,
"#,
        },
        Edit {
            path: "crates/arcweft-project/src/persistent_object/codec.rs",
            needle: r#"                CompilerObjectKind::InterfaceSummary => interface_payload_for(&key),
                CompilerObjectKind::HirBody => hir_payload_for(&key),
"#,
            replacement: r#"                CompilerObjectKind::InterfaceSummary => interface_payload_for(&key),
                CompilerObjectKind::HirBody => hir_payload_for(&key),
                CompilerObjectKind::TypecheckGate => typecheck_gate_payload_for(&key),
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query/tests.rs",
            needle: r#"        PublicSymbolKind, PublicSymbolObject, StableDiagnosticSummaryObject, StableRangeObject,
        StableSourceSpanObject, SyntaxStatsObject,
"#,
            replacement: r#"        PublicSymbolKind, PublicSymbolObject, StableDiagnosticSummaryObject, StableRangeObject,
        StableSourceSpanObject, SyntaxStatsObject, TypecheckGateFactsObject, TypecheckGateObject,
        TypecheckGateReusePolicy,
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query/tests.rs",
            needle: r#"fn interface_request() -> PersistentQueryReadRequest {
    let key = object_key(CompilerObjectKind::InterfaceSummary);
    PersistentQueryReadRequest::new(
        QueryKind::Interface,
        artifact_key(QueryKind::Interface, &key),
        key,
    )
}
"#,
            replacement: LOADER_TYPECHECK_REQUEST_HELPER,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query/tests.rs",
            needle: r#"fn envelope_bytes(key: &CompilerObjectKey) -> Vec<u8> {
    let payload = match key.kind {
        CompilerObjectKind::ParsedSyntax => parsed_payload(key),
        CompilerObjectKind::InterfaceSummary => interface_payload(key),
        CompilerObjectKind::HirBody => hir_payload(key),
        other => panic!("test helper does not support {other:?}"),
    };
"#,
            replacement: r#"fn envelope_bytes(key: &CompilerObjectKey) -> Vec<u8> {
    let payload = match key.kind {
        CompilerObjectKind::ParsedSyntax => parsed_payload(key),
        CompilerObjectKind::InterfaceSummary => interface_payload(key),
        CompilerObjectKind::HirBody => hir_payload(key),
        CompilerObjectKind::TypecheckGate => typecheck_gate_payload(key),
        other => panic!("test helper does not support {other:?}"),
    };
"#,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query/tests.rs",
            needle: r#"fn envelope_bytes(key: &CompilerObjectKey) -> Vec<u8> {
"#,
            replacement: LOADER_TYPECHECK_PAYLOAD_HELPER,
        },
        Edit {
            path: "crates/arcweft-project-loader/src/cache/persistent_query/tests.rs",
            needle: r#"#[test]
fn persistent_query_write_through_rejects_payload_kind_mismatch() {
"#,
            replacement: LOADER_TYPECHECK_TEST,
        },
    ]
}


const CODEC_TYPECHECK_TEST_HELPER: &str = r#"    fn interface_payload_for(key: &CompilerObjectKey) -> CompilerObjectPayload {
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
        let CompilerObjectPayload::HirBody(hir) = hir_payload_for(&key(CompilerObjectKind::HirBody))
        else {
            panic!("HIR helper returns HIR body");
        };
        let diagnostics = StableDiagnosticSummaryObject::empty();
        let public_symbols = interface.public_symbols.clone();
        let dependency_interface_digest_root = object_key.stage_inputs().dependency_interface_digest_root();
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
                type_signature_digest: TypecheckGateObject::type_signature_digest_for(&public_symbols),
                capability_effect_digest: TypecheckGateObject::conservative_capability_effect_digest(),
                diagnostic_digest: TypecheckGateObject::diagnostic_digest_for(&diagnostics),
            },
            reuse_policy: TypecheckGateReusePolicy::ConservativeRebuild,
        })
    }
"#;

const LOADER_TYPECHECK_REQUEST_HELPER: &str = r#"fn interface_request() -> PersistentQueryReadRequest {
    let key = object_key(CompilerObjectKind::InterfaceSummary);
    PersistentQueryReadRequest::new(
        QueryKind::Interface,
        artifact_key(QueryKind::Interface, &key),
        key,
    )
}

fn typecheck_gate_request() -> PersistentQueryReadRequest {
    let key = object_key(CompilerObjectKind::TypecheckGate);
    PersistentQueryReadRequest::new(
        QueryKind::TypeCheck,
        artifact_key(QueryKind::TypeCheck, &key),
        key,
    )
}
"#;

const LOADER_TYPECHECK_PAYLOAD_HELPER: &str = r#"fn typecheck_gate_payload(key: &CompilerObjectKey) -> CompilerObjectPayload {
    let CompilerObjectPayload::InterfaceSummary(interface) =
        interface_payload(&object_key(CompilerObjectKind::InterfaceSummary))
    else {
        panic!("interface helper returns interface summary");
    };
    let CompilerObjectPayload::HirBody(hir) = hir_payload(&object_key(CompilerObjectKind::HirBody))
    else {
        panic!("HIR helper returns HIR body");
    };
    let diagnostics = StableDiagnosticSummaryObject::empty();
    let public_symbols = interface.public_symbols.clone();
    let dependency_interface_digest_root = key.stage_inputs().dependency_interface_digest_root();
    CompilerObjectPayload::TypecheckGate(TypecheckGateObject {
        schema_version: AWBO_SCHEMA_VERSION,
        compiler_namespace: key.identity_namespace(),
        module: "main".to_owned(),
        source_digest: key.source_digest,
        source_span: span(),
        diagnostics: diagnostics.clone(),
        stage_inputs: key.stage_inputs(),
        facts: TypecheckGateFactsObject {
            interface_exports_digest: interface.exports_digest,
            interface_imports_digest: dependency_interface_digest_root,
            dependency_interface_digest_root,
            body_shape_digest: hir.body_digest,
            hir_symbol_digest: hir.facts.symbol_digest,
            public_symbols: public_symbols.clone(),
            type_signature_digest: TypecheckGateObject::type_signature_digest_for(&public_symbols),
            capability_effect_digest: TypecheckGateObject::conservative_capability_effect_digest(),
            diagnostic_digest: TypecheckGateObject::diagnostic_digest_for(&diagnostics),
        },
        reuse_policy: TypecheckGateReusePolicy::ConservativeRebuild,
    })
}

fn envelope_bytes(key: &CompilerObjectKey) -> Vec<u8> {
"#;

const LOADER_TYPECHECK_TEST: &str = r#"#[test]
fn persistent_query_write_through_stores_typecheck_gate_as_valid_but_rebuilt() {
    let store = FilesystemCacheStore::new(temp_root("write-through-typecheck-gate"));
    let request = typecheck_gate_request();
    let receipt = store
        .write_persistent_query(&PersistentQueryWriteRequest::new(
            request.query,
            request.artifact_key,
            request.object_key.clone(),
            "typecheck-gate:main",
            typecheck_gate_payload(&request.object_key),
        ))
        .expect("persistent typecheck gate writes");

    assert_eq!(receipt.query, QueryKind::TypeCheck);
    assert_eq!(receipt.artifact_kind, ArtifactKind::TypeCheckReport);
    assert!(receipt.record_path.is_file());
    assert!(receipt.object_path.is_file());

    let outcome = store.read_persistent_query(&request);
    assert!(outcome.is_hit());
    assert_eq!(
        outcome.cache_record_status(),
        CacheRecordStatus::HitThenRebuilt {
            reason: InvalidationReason::ConservativeInvalidation {
                policy: CompilerObjectKind::TYPECHECK_GATE_CONSERVATIVE_POLICY.to_owned(),
            },
        }
    );
    assert!(matches!(
        outcome,
        PersistentQueryReadOutcome::Hit(hit)
            if matches!(hit.payload, PersistentQueryHitPayload::TypecheckGate(_))
    ));
}

#[test]
fn persistent_query_write_through_rejects_payload_kind_mismatch() {
"#;

const TYPECHECK_GATE_PAYLOAD_IMPL: &str = r#"impl TypecheckGateObject {
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
        let canonical = InterfaceSummaryObject::canonical_public_symbols(
            self.facts.public_symbols.clone(),
        );
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
        if self.facts.interface_exports_digest != InterfaceSummaryObject::exports_digest_for(&canonical) {
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
                reason: "typecheck gate imports digest does not match dependency interfaces".to_owned(),
            });
        }
        if self.facts.type_signature_digest != Self::type_signature_digest_for(&canonical) {
            return Err(AwboError::MalformedPayload {
                reason: "typecheck gate type signature digest mismatch".to_owned(),
            });
        }
        if self.facts.capability_effect_digest != Self::conservative_capability_effect_digest() {
            return Err(AwboError::MalformedPayload {
                reason: "typecheck gate capability/effect digest is not conservative sentinel".to_owned(),
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
"#;

const TYPECHECK_GATE_CODEC_IMPL: &str = r#"fn put_typecheck_gate(
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
"#;

const PERSISTENT_QUERY_DEP_HELPERS: &str = r#"fn dependency_interface_mismatch_invalidation_reason(
    expected: &[NamedDigest],
    actual: &[NamedDigest],
) -> InvalidationReason {
    first_changed_dependency_name(expected, actual).map_or(
        InvalidationReason::InterfaceChanged,
        |module| InvalidationReason::DependencyInterfaceChanged { module },
    )
}

fn dependency_body_mismatch_invalidation_reason(
    expected: &[NamedDigest],
    actual: &[NamedDigest],
) -> InvalidationReason {
    first_changed_dependency_name(expected, actual).map_or(
        InvalidationReason::BodyChanged,
        |module| InvalidationReason::DependencyBodyChanged { module },
    )
}

fn first_changed_dependency_name(expected: &[NamedDigest], actual: &[NamedDigest]) -> Option<String> {
    let actual_by_name = actual
        .iter()
        .map(|value| (value.name(), value.digest()))
        .collect::<BTreeMap<_, _>>();
    expected
        .iter()
        .find(|expected| match actual_by_name.get(expected.name()) {
            Some(actual_digest) => *actual_digest != expected.digest(),
            None => true,
        })
        .map(|value| value.name().to_owned())
        .or_else(|| {
            let expected_by_name = expected
                .iter()
                .map(|value| (value.name(), value.digest()))
                .collect::<BTreeMap<_, _>>();
            actual
                .iter()
                .find(|actual| !expected_by_name.contains_key(actual.name()))
                .map(|value| value.name().to_owned())
        })
}

impl From<ErrorKind> for PersistentQueryIoKind {
"#;

const PERSISTENT_QUERY_TYPECHECK_VALIDATOR: &str = r#"fn validate_typecheck_gate_payload(
    payload: &TypecheckGateObject,
    key: &CompilerObjectKey,
) -> Result<(), PersistentQueryMissReason> {
    validate_payload_schema(payload.schema_version)?;
    validate_namespace(&payload.compiler_namespace, key)?;
    validate_source_digest(payload.source_digest, key)?;
    payload
        .source_span
        .validate()
        .map_err(|error| corrupt_object(&error))?;
    payload
        .diagnostics
        .validate()
        .map_err(|error| corrupt_object(&error))?;
    validate_stage_inputs(&payload.stage_inputs, key)?;
    payload
        .validate_gate_shape()
        .map_err(|error| corrupt_object(&error))
}

fn typecheck_gate_reuse_policy(envelope: &AwboEnvelope) -> Option<TypecheckGateReusePolicy> {
    match &envelope.payload {
        CompilerObjectPayload::TypecheckGate(payload) => Some(payload.reuse_policy),
        CompilerObjectPayload::ParsedSyntax(_)
        | CompilerObjectPayload::InterfaceSummary(_)
        | CompilerObjectPayload::HirBody(_)
        | CompilerObjectPayload::LineTaskEvidence(_)
        | CompilerObjectPayload::RuntimePlanUnit(_)
        | CompilerObjectPayload::BytecodeUnit(_)
        | CompilerObjectPayload::LinkPlan(_) => None,
    }
}

fn validate_interface_summary_payload(
"#;

const TYPECHECK_GATE_COMPILER_IMPL: &str = r#"/// Builds a stable typecheck gate object without serializing linked HIR or a TypeCheckReport.
pub fn typecheck_gate_object(
    input: &TypecheckGateFactsInput<'_>,
) -> Result<TypecheckGateObject, PersistentFactsError> {
    ensure_key_kind(input.key, CompilerObjectKind::TypecheckGate)?;
    let source_digest = BuildDigest::from_bytes(input.parsed.source_hash().as_bytes());
    if input.interface_summary.source_digest != source_digest {
        return Err(PersistentFactsError::SourceDigestMismatch {
            field: "interface_summary",
        });
    }
    if input.hir_body.source_digest != source_digest {
        return Err(PersistentFactsError::SourceDigestMismatch { field: "hir_body" });
    }
    let diagnostics = StableDiagnosticSummaryObject::empty();
    let public_symbols = InterfaceSummaryObject::canonical_public_symbols(
        input.interface_summary.public_symbols.clone(),
    );
    let dependency_interface_digest_root = input.key.stage_inputs().dependency_interface_digest_root();
    let facts = TypecheckGateFactsObject {
        interface_exports_digest: input.interface_summary.exports_digest,
        interface_imports_digest: input.interface_summary.imports_digest,
        dependency_interface_digest_root,
        body_shape_digest: input.hir_body.body_digest,
        hir_symbol_digest: input.hir_body.facts.symbol_digest,
        type_signature_digest: TypecheckGateObject::type_signature_digest_for(&public_symbols),
        capability_effect_digest: TypecheckGateObject::conservative_capability_effect_digest(),
        diagnostic_digest: TypecheckGateObject::diagnostic_digest_for(&diagnostics),
        public_symbols,
    };
    Ok(TypecheckGateObject {
        schema_version: AWBO_SCHEMA_VERSION,
        compiler_namespace: input.key.identity_namespace(),
        module: input.module.to_owned(),
        source_digest,
        source_span: source_span(input.parsed)?,
        diagnostics,
        stage_inputs: input.key.stage_inputs(),
        facts,
        reuse_policy: TypecheckGateReusePolicy::ConservativeRebuild,
    })
}

/// Builds a typed typecheck-gate payload enum for direct envelope construction.
pub fn typecheck_gate_payload(
    input: &TypecheckGateFactsInput<'_>,
) -> Result<CompilerObjectPayload, PersistentFactsError> {
    Ok(CompilerObjectPayload::TypecheckGate(typecheck_gate_object(input)?))
}

fn ensure_key_kind(
"#;
