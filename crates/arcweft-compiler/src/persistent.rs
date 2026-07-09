//! Compiler-private persistent fact builders for safe `.awbo` payloads.
//!
//! This module projects syntax and HIR into stable evidence objects owned by
//! `arcweft-project::persistent_object`. It intentionally does not perform cache
//! reads, cache writes, or semantic/typecheck reuse.

use arcweft_lang_hir::model::{HirAwait, HirChoice, HirFlowItem, HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::{
    ast::common::TextRange,
    cst::{SyntaxKind, SyntaxNode, SyntaxParseStats},
    parser::recovery::ParseError,
    source::{LineIndex, ParsedSource},
    types::{FnParamKind, FnSignature, GenericParam, TypeRef},
};
use arcweft_project::{
    fingerprint::{BuildDigest, NamedDigest},
    persistent_object::{
        AWBO_SCHEMA_VERSION, BytecodeUnitFactsObject, BytecodeUnitIdentityObject,
        BytecodeUnitObject, BytecodeUnitReusePolicy, CompilerObjectKey, CompilerObjectKind,
        CompilerObjectPayload, HirBodyFactsObject, HirBodyObject, InterfaceSummaryObject,
        LinkDescriptorObject, LinkPlanFactsObject, LinkPlanObject, LinkPlanReusePolicy,
        ParsedSyntaxEvidenceObject, ParsedSyntaxObject, PublicSymbolKind, PublicSymbolObject,
        StableDiagnosticObject, StableDiagnosticSeverity, StableDiagnosticSummaryObject,
        StableRangeObject, StableSourceSpanObject, SyntaxStatsObject, TypecheckGateFactsObject,
        TypecheckGateObject, TypecheckGateReusePolicy,
    },
};
use thiserror::Error;

/// Inputs required to project a parsed source into a deterministic payload.
pub struct ParsedSyntaxFactsInput<'a> {
    pub key: &'a CompilerObjectKey,
    pub source_label: &'a str,
    pub parsed: &'a ParsedSource,
}

/// Inputs required to project a lowered HIR module into deterministic facts.
pub struct HirBodyFactsInput<'a> {
    pub key: &'a CompilerObjectKey,
    pub module: &'a str,
    pub parsed: &'a ParsedSource,
    pub hir: &'a HirModule,
}

/// Inputs required to project a lowered HIR module into stable interface facts.
pub struct InterfaceSummaryFactsInput<'a> {
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

/// Inputs required to project a conservative bytecode-unit gate.
pub struct BytecodeUnitFactsInput<'a> {
    pub key: &'a CompilerObjectKey,
    pub module: &'a str,
    pub parsed: &'a ParsedSource,
    pub hir_body: &'a HirBodyObject,
    pub typecheck_gate: &'a TypecheckGateObject,
}

/// Inputs required to project a conservative link-plan gate.
pub struct LinkPlanFactsInput<'a> {
    pub key: &'a CompilerObjectKey,
    pub package: &'a str,
    pub parsed: &'a ParsedSource,
    pub ordered_unit_digests: Vec<NamedDigest>,
    pub product_build_options_digest: BuildDigest,
}

/// Inputs required to project an actual reusable bytecode unit.
pub struct ActualBytecodeUnitFactsInput<'a> {
    pub key: &'a CompilerObjectKey,
    pub module: &'a str,
    pub parsed: &'a ParsedSource,
    pub hir_body: &'a HirBodyObject,
    pub typecheck_gate: &'a TypecheckGateObject,
    pub runtime_plan_unit_digest: BuildDigest,
    pub canonical_awbc_bytes: &'a [u8],
    pub awbc_schema_digest: BuildDigest,
    pub verifier_policy_digest: BuildDigest,
    pub codegen_policy_digest: BuildDigest,
    pub target_profile_digest: BuildDigest,
    pub feature_set_digest: BuildDigest,
    pub relocation_import_table_digest: BuildDigest,
}

/// Inputs required to project an actual reusable link descriptor.
pub struct ActualLinkPlanFactsInput<'a> {
    pub key: &'a CompilerObjectKey,
    pub package: &'a str,
    pub parsed: &'a ParsedSource,
    pub ordered_unit_identities: Vec<NamedDigest>,
    pub entrypoint_digest: BuildDigest,
    pub resource_section_digest: BuildDigest,
    pub adapter_requirements_digest: BuildDigest,
    pub patch_compatibility_digest: BuildDigest,
    pub product_build_options_digest: BuildDigest,
}

/// Failure while projecting compiler internals into stable persistent facts.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum PersistentFactsError {
    #[error("persistent fact builder expected {expected:?} key, got {actual:?}")]
    WrongObjectKind {
        expected: CompilerObjectKind,
        actual: CompilerObjectKind,
    },
    #[error("{field} value {value} does not fit the stable payload coordinate type")]
    CoordinateTooLarge { field: &'static str, value: usize },
    #[error("{field} length does not fit the stable payload count type")]
    CountTooLarge { field: &'static str },
    #[error("{field} source digest does not match typecheck gate key")]
    SourceDigestMismatch { field: &'static str },
}

/// Builds a parsed-syntax payload object without serializing parser internals.
pub fn parsed_syntax_object(
    input: &ParsedSyntaxFactsInput<'_>,
) -> Result<ParsedSyntaxObject, PersistentFactsError> {
    ensure_key_kind(input.key, CompilerObjectKind::ParsedSyntax)?;
    let source_digest = BuildDigest::from_bytes(input.parsed.source_hash().as_bytes());
    let diagnostics = input
        .parsed
        .errors()
        .iter()
        .map(|error| parse_error_diagnostic(error, input.parsed.line_index()))
        .collect::<Result<Vec<_>, _>>()?;
    let diagnostics = StableDiagnosticSummaryObject::new(diagnostics).map_err(|_| {
        PersistentFactsError::CountTooLarge {
            field: "parse diagnostics",
        }
    })?;

    Ok(ParsedSyntaxObject {
        schema_version: AWBO_SCHEMA_VERSION,
        compiler_namespace: input.key.identity_namespace(),
        source_label: input.source_label.to_owned(),
        source_digest,
        source_span: source_span(input.parsed)?,
        stats: syntax_stats(input.parsed.source(), input.parsed.syntax_stats())?,
        diagnostics,
        stage_inputs: input.key.stage_inputs(),
        evidence: parsed_syntax_evidence(input.parsed)?,
    })
}

/// Builds a typed parsed-syntax payload enum for direct envelope construction.
pub fn parsed_syntax_payload(
    input: &ParsedSyntaxFactsInput<'_>,
) -> Result<CompilerObjectPayload, PersistentFactsError> {
    Ok(CompilerObjectPayload::ParsedSyntax(parsed_syntax_object(
        input,
    )?))
}

/// Builds a HIR-body fact payload object without serializing `HirModule`.
pub fn hir_body_object(
    input: &HirBodyFactsInput<'_>,
) -> Result<HirBodyObject, PersistentFactsError> {
    ensure_key_kind(input.key, CompilerObjectKind::HirBody)?;
    let source_digest = BuildDigest::from_bytes(input.parsed.source_hash().as_bytes());
    let facts = hir_body_facts(input.module, input.hir)?;
    Ok(HirBodyObject {
        schema_version: AWBO_SCHEMA_VERSION,
        compiler_namespace: input.key.identity_namespace(),
        module: input.module.to_owned(),
        source_digest,
        source_span: source_span(input.parsed)?,
        diagnostics: StableDiagnosticSummaryObject::empty(),
        stage_inputs: input.key.stage_inputs(),
        body_digest: facts.body_shape_digest,
        facts,
    })
}

/// Builds a typed HIR-body payload enum for direct envelope construction.
pub fn hir_body_payload(
    input: &HirBodyFactsInput<'_>,
) -> Result<CompilerObjectPayload, PersistentFactsError> {
    Ok(CompilerObjectPayload::HirBody(hir_body_object(input)?))
}

/// Builds a stable interface-summary payload object without serializing HIR.
pub fn interface_summary_object(
    input: &InterfaceSummaryFactsInput<'_>,
) -> Result<InterfaceSummaryObject, PersistentFactsError> {
    ensure_key_kind(input.key, CompilerObjectKind::InterfaceSummary)?;
    let source_digest = BuildDigest::from_bytes(input.parsed.source_hash().as_bytes());
    let stage_inputs = input.key.stage_inputs();
    let public_symbols = interface_public_symbols(input.module, input.hir)?;
    Ok(InterfaceSummaryObject {
        schema_version: AWBO_SCHEMA_VERSION,
        compiler_namespace: input.key.identity_namespace(),
        module: input.module.to_owned(),
        source_digest,
        source_span: source_span(input.parsed)?,
        diagnostics: StableDiagnosticSummaryObject::empty(),
        imports_digest: stage_inputs.dependency_interface_digest_root(),
        exports_digest: InterfaceSummaryObject::exports_digest_for(&public_symbols),
        stage_inputs,
        public_symbols,
    })
}

/// Builds a typed interface-summary payload enum for direct envelope construction.
pub fn interface_summary_payload(
    input: &InterfaceSummaryFactsInput<'_>,
) -> Result<CompilerObjectPayload, PersistentFactsError> {
    Ok(CompilerObjectPayload::InterfaceSummary(
        interface_summary_object(input)?,
    ))
}

/// Builds a stable typecheck gate object without serializing linked HIR or a `TypeCheckReport`.
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
    let dependency_interface_digest_root =
        input.key.stage_inputs().dependency_interface_digest_root();
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
    Ok(CompilerObjectPayload::TypecheckGate(typecheck_gate_object(
        input,
    )?))
}

/// Builds a conservative bytecode-unit gate object.
pub fn bytecode_unit_object(
    input: &BytecodeUnitFactsInput<'_>,
) -> Result<BytecodeUnitObject, PersistentFactsError> {
    ensure_key_kind(input.key, CompilerObjectKind::BytecodeUnit)?;
    let source_digest = BuildDigest::from_bytes(input.parsed.source_hash().as_bytes());
    if input.hir_body.source_digest != source_digest {
        return Err(PersistentFactsError::SourceDigestMismatch { field: "hir_body" });
    }
    if input.typecheck_gate.source_digest != source_digest {
        return Err(PersistentFactsError::SourceDigestMismatch {
            field: "typecheck_gate",
        });
    }
    let stage_inputs = input.key.stage_inputs();
    let facts = BytecodeUnitFactsObject {
        identity: BytecodeUnitIdentityObject {
            runtime_plan_unit_digest: BytecodeUnitObject::conservative_runtime_plan_unit_digest(),
            awbc_schema_digest: BytecodeUnitObject::conservative_awbc_schema_digest(),
            verifier_policy_digest: BytecodeUnitObject::conservative_verifier_policy_digest(),
            codegen_policy_digest: BytecodeUnitObject::conservative_codegen_policy_digest(),
            target_profile_digest: input.key.query_options_digest,
            feature_set_digest: BuildDigest::of(
                input.key.compiler.enabled_features.join("\0").as_bytes(),
            ),
            relocation_import_table_digest:
                BytecodeUnitObject::conservative_relocation_import_table_digest(),
        },
        hir_body_digest: input.hir_body.body_digest,
        typecheck_gate_digest: input.typecheck_gate.facts.diagnostic_digest,
        dependency_body_digest_root: stage_inputs.dependency_body_digest_root(),
        canonical_bytecode_digest: BytecodeUnitObject::conservative_canonical_bytecode_digest(),
        bytecode_descriptor_digest: BuildDigest::ZERO,
    };
    let facts = BytecodeUnitFactsObject {
        bytecode_descriptor_digest: BytecodeUnitObject::bytecode_descriptor_digest_for(&facts),
        ..facts
    };
    Ok(BytecodeUnitObject {
        schema_version: AWBO_SCHEMA_VERSION,
        compiler_namespace: input.key.identity_namespace(),
        module: input.module.to_owned(),
        source_digest,
        source_span: source_span(input.parsed)?,
        diagnostics: StableDiagnosticSummaryObject::empty(),
        stage_inputs,
        facts,
        canonical_awbc_bytes: Vec::new(),
        reuse_policy: BytecodeUnitReusePolicy::ConservativeRebuild,
    })
}

/// Builds a typed conservative bytecode-unit payload enum.
pub fn bytecode_unit_payload(
    input: &BytecodeUnitFactsInput<'_>,
) -> Result<CompilerObjectPayload, PersistentFactsError> {
    Ok(CompilerObjectPayload::BytecodeUnit(bytecode_unit_object(
        input,
    )?))
}

/// Builds an actual reusable bytecode-unit payload object.
pub fn actual_bytecode_unit_object(
    input: &ActualBytecodeUnitFactsInput<'_>,
) -> Result<BytecodeUnitObject, PersistentFactsError> {
    ensure_key_kind(input.key, CompilerObjectKind::BytecodeUnit)?;
    let source_digest = BuildDigest::from_bytes(input.parsed.source_hash().as_bytes());
    if input.hir_body.source_digest != source_digest {
        return Err(PersistentFactsError::SourceDigestMismatch { field: "hir_body" });
    }
    if input.typecheck_gate.source_digest != source_digest {
        return Err(PersistentFactsError::SourceDigestMismatch {
            field: "typecheck_gate",
        });
    }
    let stage_inputs = input.key.stage_inputs();
    let facts = BytecodeUnitFactsObject {
        identity: BytecodeUnitIdentityObject {
            runtime_plan_unit_digest: input.runtime_plan_unit_digest,
            awbc_schema_digest: input.awbc_schema_digest,
            verifier_policy_digest: input.verifier_policy_digest,
            codegen_policy_digest: input.codegen_policy_digest,
            target_profile_digest: input.target_profile_digest,
            feature_set_digest: input.feature_set_digest,
            relocation_import_table_digest: input.relocation_import_table_digest,
        },
        hir_body_digest: input.hir_body.body_digest,
        typecheck_gate_digest: input.typecheck_gate.facts.diagnostic_digest,
        dependency_body_digest_root: stage_inputs.dependency_body_digest_root(),
        canonical_bytecode_digest: BuildDigest::of(input.canonical_awbc_bytes),
        bytecode_descriptor_digest: BuildDigest::ZERO,
    };
    let facts = BytecodeUnitFactsObject {
        bytecode_descriptor_digest: BytecodeUnitObject::bytecode_descriptor_digest_for(&facts),
        ..facts
    };
    Ok(BytecodeUnitObject {
        schema_version: AWBO_SCHEMA_VERSION,
        compiler_namespace: input.key.identity_namespace(),
        module: input.module.to_owned(),
        source_digest,
        source_span: source_span(input.parsed)?,
        diagnostics: StableDiagnosticSummaryObject::empty(),
        stage_inputs,
        facts,
        canonical_awbc_bytes: input.canonical_awbc_bytes.to_vec(),
        reuse_policy: BytecodeUnitReusePolicy::VerifiedReusable,
    })
}

/// Builds a typed actual bytecode-unit payload enum.
pub fn actual_bytecode_unit_payload(
    input: &ActualBytecodeUnitFactsInput<'_>,
) -> Result<CompilerObjectPayload, PersistentFactsError> {
    Ok(CompilerObjectPayload::BytecodeUnit(
        actual_bytecode_unit_object(input)?,
    ))
}

/// Builds a conservative link-plan gate object.
pub fn link_plan_object(
    input: &LinkPlanFactsInput<'_>,
) -> Result<LinkPlanObject, PersistentFactsError> {
    ensure_key_kind(input.key, CompilerObjectKind::LinkPlan)?;
    let source_digest = BuildDigest::from_bytes(input.parsed.source_hash().as_bytes());
    let stage_inputs = input.key.stage_inputs();
    let descriptor = LinkDescriptorObject {
        ordered_unit_identities: NamedDigest::canonicalize(input.ordered_unit_digests.clone()),
        entrypoint_digest: LinkPlanObject::conservative_entrypoint_digest(),
        resource_section_digest: LinkPlanObject::conservative_resource_section_digest(),
        adapter_requirements_digest: LinkPlanObject::conservative_adapter_requirements_digest(),
        patch_compatibility_digest: LinkPlanObject::conservative_patch_compatibility_digest(),
        product_build_options_digest: input.product_build_options_digest,
        dependency_body_digest_root: stage_inputs.dependency_body_digest_root(),
    };
    let mut facts = LinkPlanFactsObject {
        descriptor,
        link_descriptor_digest: BuildDigest::ZERO,
    };
    facts.link_descriptor_digest = LinkPlanObject::link_descriptor_digest_for(&facts.descriptor);
    Ok(LinkPlanObject {
        schema_version: AWBO_SCHEMA_VERSION,
        compiler_namespace: input.key.identity_namespace(),
        package: input.package.to_owned(),
        source_digest,
        source_span: source_span(input.parsed)?,
        diagnostics: StableDiagnosticSummaryObject::empty(),
        stage_inputs,
        facts,
        reuse_policy: LinkPlanReusePolicy::ConservativeRebuild,
    })
}

/// Builds a typed conservative link-plan payload enum.
pub fn link_plan_payload(
    input: &LinkPlanFactsInput<'_>,
) -> Result<CompilerObjectPayload, PersistentFactsError> {
    Ok(CompilerObjectPayload::LinkPlan(link_plan_object(input)?))
}

/// Builds an actual reusable link-plan payload object.
pub fn actual_link_plan_object(
    input: &ActualLinkPlanFactsInput<'_>,
) -> Result<LinkPlanObject, PersistentFactsError> {
    ensure_key_kind(input.key, CompilerObjectKind::LinkPlan)?;
    let source_digest = BuildDigest::from_bytes(input.parsed.source_hash().as_bytes());
    let stage_inputs = input.key.stage_inputs();
    let descriptor = LinkDescriptorObject {
        ordered_unit_identities: input.ordered_unit_identities.clone(),
        entrypoint_digest: input.entrypoint_digest,
        resource_section_digest: input.resource_section_digest,
        adapter_requirements_digest: input.adapter_requirements_digest,
        patch_compatibility_digest: input.patch_compatibility_digest,
        product_build_options_digest: input.product_build_options_digest,
        dependency_body_digest_root: stage_inputs.dependency_body_digest_root(),
    };
    let facts = LinkPlanFactsObject {
        link_descriptor_digest: LinkPlanObject::link_descriptor_digest_for(&descriptor),
        descriptor,
    };
    Ok(LinkPlanObject {
        schema_version: AWBO_SCHEMA_VERSION,
        compiler_namespace: input.key.identity_namespace(),
        package: input.package.to_owned(),
        source_digest,
        source_span: source_span(input.parsed)?,
        diagnostics: StableDiagnosticSummaryObject::empty(),
        stage_inputs,
        facts,
        reuse_policy: LinkPlanReusePolicy::VerifiedReusable,
    })
}

/// Builds a typed actual link-plan payload enum.
pub fn actual_link_plan_payload(
    input: &ActualLinkPlanFactsInput<'_>,
) -> Result<CompilerObjectPayload, PersistentFactsError> {
    Ok(CompilerObjectPayload::LinkPlan(actual_link_plan_object(
        input,
    )?))
}

fn ensure_key_kind(
    key: &CompilerObjectKey,
    expected: CompilerObjectKind,
) -> Result<(), PersistentFactsError> {
    if key.kind == expected {
        Ok(())
    } else {
        Err(PersistentFactsError::WrongObjectKind {
            expected,
            actual: key.kind,
        })
    }
}

fn source_span(parsed: &ParsedSource) -> Result<StableSourceSpanObject, PersistentFactsError> {
    stable_span_for_offsets(0, parsed.source().len(), parsed.line_index())
}

fn stable_span_for_range(
    range: &TextRange,
    line_index: &LineIndex,
) -> Result<StableSourceSpanObject, PersistentFactsError> {
    stable_span_for_offsets(range.start(), range.end(), line_index)
}

fn stable_span_for_offsets(
    start: usize,
    end: usize,
    line_index: &LineIndex,
) -> Result<StableSourceSpanObject, PersistentFactsError> {
    let (start_line, start_column) = line_index.line_col(start);
    let (end_line, end_column) = line_index.line_col(end);
    Ok(StableSourceSpanObject {
        range: StableRangeObject {
            start: to_u32("source span start", start)?,
            end: to_u32("source span end", end)?,
        },
        start_line: to_u32("source span start line", start_line)?,
        start_column: to_u32("source span start column", start_column)?,
        end_line: to_u32("source span end line", end_line)?,
        end_column: to_u32("source span end column", end_column)?,
    })
}

fn syntax_stats(
    source: &str,
    stats: SyntaxParseStats,
) -> Result<SyntaxStatsObject, PersistentFactsError> {
    Ok(SyntaxStatsObject {
        bytes: to_u64("source bytes", source.len())?,
        lines: to_u64("source lines", source.lines().count())?,
        cst_lex_passes: to_u64("cst lex passes", stats.cst_lex_passes)?,
        punctuation_scans: to_u64("punctuation scans", stats.punctuation_scans)?,
        punctuation_scan_bytes: to_u64("punctuation scan bytes", stats.punctuation_scan_bytes)?,
        line_owned_bytes: to_u64("line owned bytes", stats.line_owned_bytes)?,
        block_owned_bytes: to_u64("block owned bytes", stats.block_owned_bytes)?,
        raw_owned_bytes: to_u64("raw owned bytes", stats.raw_owned_bytes)?,
        wiki_scan_performed: to_u64("wiki scans", stats.wiki_scan_performed)?,
        dot_normalization_owned: to_u64("dot normalization", stats.dot_normalization_owned)?,
        dialogue_rescue_expr_parse_attempts: to_u64(
            "dialogue rescue parse attempts",
            stats.dialogue_rescue_expr_parse_attempts,
        )?,
        numeric_seq_summaries: to_u64("numeric sequence summaries", stats.numeric_seq_summaries)?,
    })
}

fn parse_error_diagnostic(
    error: &ParseError,
    line_index: &LineIndex,
) -> Result<StableDiagnosticObject, PersistentFactsError> {
    Ok(StableDiagnosticObject {
        code: "syntax.parse".to_owned(),
        severity: StableDiagnosticSeverity::Error,
        message: error.message().to_owned(),
        primary_span: Some(stable_span_for_range(error.range(), line_index)?),
        related_spans: Vec::new(),
    })
}

fn parsed_syntax_evidence(
    parsed: &ParsedSource,
) -> Result<ParsedSyntaxEvidenceObject, PersistentFactsError> {
    let mut counts = SyntaxShapeCounts::default();
    let mut shape_bytes = Vec::new();
    record_syntax_node(parsed.syntax(), &mut counts, &mut shape_bytes)?;
    let typed_tree = parsed.typed_tree();
    Ok(ParsedSyntaxEvidenceObject {
        root_kind: parsed.syntax().kind().cache_fact_tag().to_owned(),
        cst_shape_digest: BuildDigest::of(&shape_bytes),
        line_index_digest: line_index_digest(parsed.line_index())?,
        cst_node_count: counts.nodes,
        cst_token_count: counts.tokens,
        cst_error_node_count: counts.error_nodes,
        typed_attribute_count: to_u64("typed attributes", typed_tree.attrs().len())?,
        typed_use_count: to_u64("typed uses", typed_tree.uses().len())?,
        typed_item_count: to_u64("typed items", typed_tree.items().len())?,
        wiki_link_count: to_u64("wiki links", typed_tree.wiki_links().len())?,
    })
}

fn line_index_digest(line_index: &LineIndex) -> Result<BuildDigest, PersistentFactsError> {
    let mut bytes = Vec::new();
    put_len(&mut bytes, "line starts", line_index.starts().len())?;
    for start in line_index.starts() {
        put_u64(&mut bytes, to_u64("line start", *start)?);
    }
    Ok(BuildDigest::of(&bytes))
}

fn record_syntax_node(
    node: &SyntaxNode,
    counts: &mut SyntaxShapeCounts,
    bytes: &mut Vec<u8>,
) -> Result<(), PersistentFactsError> {
    counts.nodes += 1;
    if node.kind() == SyntaxKind::Error {
        counts.error_nodes += 1;
    }
    put_str(bytes, "node")?;
    put_str(bytes, node.kind().cache_fact_tag())?;
    let range = node.text_range();
    put_u32(bytes, range.start().into());
    put_u32(bytes, range.end().into());
    for element in node.children_with_tokens() {
        if let Some(child) = element.as_node() {
            record_syntax_node(child, counts, bytes)?;
        } else if let Some(token) = element.as_token() {
            counts.tokens += 1;
            put_str(bytes, "token")?;
            put_str(bytes, token.kind().cache_fact_tag())?;
            let range = token.text_range();
            put_u32(bytes, range.start().into());
            put_u32(bytes, range.end().into());
        }
    }
    Ok(())
}

fn hir_body_facts(
    module: &str,
    hir: &HirModule,
) -> Result<HirBodyFactsObject, PersistentFactsError> {
    let mut counts = HirBodyCounts::default();
    let mut symbols = Vec::new();
    let mut shape = Vec::new();

    put_str(&mut symbols, module)?;
    put_len(&mut shape, "hir attributes", hir.attributes().len())?;
    for attribute in hir.attributes() {
        put_str(&mut symbols, "attribute")?;
        put_str(&mut symbols, attribute.name())?;
    }

    for flow in hir.flows() {
        put_str(&mut symbols, "flow")?;
        put_str(&mut symbols, "flow")?;
        put_option_str(&mut symbols, flow.name())?;
        put_len(&mut shape, "flow body", flow.body().len())?;
        counts.flows += 1;
        record_hir_flow_items(flow.body(), &mut counts, &mut shape)?;
    }

    for function in hir.functions() {
        put_str(&mut symbols, "function")?;
        put_str(&mut symbols, function.kind().cache_fact_tag())?;
        put_str(&mut symbols, function.name())?;
        put_len(
            &mut shape,
            "function statements",
            function.statements().len(),
        )?;
        put_bool(&mut shape, function.value().is_some());
        counts.functions += 1;
        counts.statements += to_u64("function statements", function.statements().len())?;
    }

    for agent in hir.agents() {
        put_str(&mut symbols, "agent")?;
        put_str(&mut symbols, agent.item().name())?;
        put_len(
            &mut shape,
            "agent statements",
            agent.item().body_statements().len(),
        )?;
        put_bool(&mut shape, agent.item().body_value().is_some());
        counts.agents += 1;
        counts.statements += to_u64("agent statements", agent.item().body_statements().len())?;
    }

    for declaration in hir.declarations() {
        record_hir_declaration(declaration, &mut symbols, &mut shape)?;
        counts.declarations += 1;
    }

    put_len(&mut shape, "top level items", hir.top_level_items().len())?;
    counts.top_level_items = to_u64("top level items", hir.top_level_items().len())?;
    record_hir_flow_items(hir.top_level_items(), &mut counts, &mut shape)?;

    Ok(HirBodyFactsObject {
        attribute_count: to_u64("hir attributes", hir.attributes().len())?,
        flow_count: counts.flows,
        function_count: counts.functions,
        agent_count: counts.agents,
        declaration_count: counts.declarations,
        top_level_item_count: counts.top_level_items,
        flow_item_count: counts.flow_items,
        statement_count: counts.statements,
        dialogue_count: counts.dialogues,
        choice_count: counts.choices,
        loop_count: counts.loops,
        await_count: counts.awaits,
        thread_count: counts.threads,
        include_count: counts.includes,
        symbol_digest: BuildDigest::of(&symbols),
        body_shape_digest: BuildDigest::of(&shape),
    })
}

fn interface_public_symbols(
    module: &str,
    hir: &HirModule,
) -> Result<Vec<PublicSymbolObject>, PersistentFactsError> {
    let mut symbols = Vec::new();
    for flow in hir.flows() {
        let Some(name) = flow.name() else {
            continue;
        };
        symbols.push(public_symbol(
            PublicSymbolKind::Flow,
            module,
            name,
            signature_digest("flow", name, flow.signature())?,
        ));
    }

    for function in hir.functions() {
        symbols.push(public_symbol(
            PublicSymbolKind::Function,
            module,
            function.name(),
            signature_digest("function", function.name(), Some(function.signature()))?,
        ));
    }

    for agent in hir.agents() {
        let name = agent.item().name();
        symbols.push(public_symbol(
            PublicSymbolKind::Agent,
            module,
            name,
            signature_digest("agent", name, agent.item().signature())?,
        ));
    }

    for (index, declaration) in hir.declarations().iter().enumerate() {
        let name = format!("decl:{index}:{}", declaration.cache_fact_tag());
        symbols.push(public_symbol(
            PublicSymbolKind::Declaration,
            module,
            &name,
            signature_digest("declaration", declaration.cache_fact_tag(), None)?,
        ));
    }

    Ok(InterfaceSummaryObject::canonical_public_symbols(symbols))
}

fn public_symbol(
    kind: PublicSymbolKind,
    module: &str,
    name: &str,
    signature_digest: BuildDigest,
) -> PublicSymbolObject {
    PublicSymbolObject {
        name: format!("{module}::{name}"),
        kind,
        signature_digest,
    }
}

fn signature_digest(
    family: &str,
    name: &str,
    signature: Option<&FnSignature>,
) -> Result<BuildDigest, PersistentFactsError> {
    let mut bytes = Vec::new();
    put_str(&mut bytes, family)?;
    put_str(&mut bytes, name)?;
    if let Some(signature) = signature {
        record_fn_signature(signature, &mut bytes)?;
    } else {
        put_str(&mut bytes, "no-signature")?;
    }
    Ok(BuildDigest::of(&bytes))
}

fn record_fn_signature(
    signature: &FnSignature,
    bytes: &mut Vec<u8>,
) -> Result<(), PersistentFactsError> {
    put_str(bytes, signature.name())?;
    put_len(bytes, "generic params", signature.generic_params().len())?;
    for param in signature.generic_params() {
        match param {
            GenericParam::Lifetime(lifetime) => {
                put_str(bytes, "lifetime")?;
                put_str(bytes, lifetime.name())?;
            }
            GenericParam::Type(param) => {
                put_str(bytes, "type")?;
                put_str(bytes, param.name())?;
                put_len(bytes, "type bounds", param.bounds().len())?;
                for bound in param.bounds() {
                    record_type_ref(bound, bytes)?;
                }
            }
        }
    }

    put_len(bytes, "param groups", signature.param_groups().len())?;
    for group in signature.param_groups() {
        put_len(bytes, "params", group.params().len())?;
        for param in group.params() {
            put_str(
                bytes,
                match param.kind() {
                    FnParamKind::Fixed => "fixed",
                    FnParamKind::Rest => "rest",
                },
            )?;
            record_type_ref(param.ty(), bytes)?;
            put_bool(bytes, param.default().is_some());
        }
    }

    if let Some(return_type) = signature.return_type() {
        put_bool(bytes, true);
        record_type_ref(return_type, bytes)?;
    } else {
        put_bool(bytes, false);
    }

    put_len(bytes, "where clauses", signature.where_clauses().len())?;
    for clause in signature.where_clauses() {
        record_type_ref(clause.subject(), bytes)?;
        put_len(bytes, "where bounds", clause.bounds().len())?;
        for bound in clause.bounds() {
            record_type_ref(bound, bytes)?;
        }
    }
    Ok(())
}

fn record_type_ref(ty: &TypeRef, bytes: &mut Vec<u8>) -> Result<(), PersistentFactsError> {
    match ty {
        TypeRef::Never => put_str(bytes, "never")?,
        TypeRef::ConstInt(value) => {
            put_str(bytes, "const-int")?;
            put_len(bytes, "const int", *value)?;
        }
        TypeRef::Path(path) => {
            put_str(bytes, "path")?;
            put_str(bytes, path)?;
        }
        TypeRef::Tuple(items) => {
            put_str(bytes, "tuple")?;
            put_len(bytes, "tuple items", items.len())?;
            for item in items {
                record_type_ref(item, bytes)?;
            }
        }
        TypeRef::Function {
            params,
            return_type,
            effects,
        } => {
            put_str(bytes, "function")?;
            put_len(bytes, "function params", params.len())?;
            for param in params {
                record_type_ref(param, bytes)?;
            }
            record_type_ref(return_type, bytes)?;
            match effects {
                Some(effects) => {
                    put_str(bytes, "effects")?;
                    put_len(bytes, "function effect row", effects.effects().len())?;
                    for effect in effects.effects() {
                        put_str(bytes, effect)?;
                    }
                }
                None => put_str(bytes, "effects-unknown")?,
            }
        }
        TypeRef::Choice(alternatives) => {
            put_str(bytes, "choice")?;
            put_len(bytes, "choice alternatives", alternatives.len())?;
            for alternative in alternatives {
                record_type_ref(alternative, bytes)?;
            }
        }
        TypeRef::Generic { base, args } => {
            put_str(bytes, "generic")?;
            put_str(bytes, base)?;
            put_len(bytes, "generic args", args.len())?;
            for arg in args {
                record_type_ref(arg, bytes)?;
            }
        }
        TypeRef::TraitBound(bound) => {
            put_str(bytes, "trait-bound")?;
            put_str(bytes, bound.path())?;
            put_len(bytes, "trait bound args", bound.args().len())?;
            for arg in bound.args() {
                record_type_ref(arg, bytes)?;
            }
            put_len(
                bytes,
                "associated type bindings",
                bound.assoc_bindings().len(),
            )?;
            for binding in bound.assoc_bindings() {
                put_str(bytes, binding.name())?;
                record_type_ref(binding.value(), bytes)?;
            }
        }
        TypeRef::Projection { subject, assoc } => {
            put_str(bytes, "projection")?;
            record_type_ref(subject, bytes)?;
            put_str(bytes, assoc)?;
        }
        TypeRef::Ref { lifetime, inner } => {
            put_str(bytes, "ref")?;
            put_option_str(
                bytes,
                lifetime
                    .as_ref()
                    .map(arcweft_lang_syntax::types::LifetimeName::name),
            )?;
            record_type_ref(inner, bytes)?;
        }
        TypeRef::Slice(inner) => {
            put_str(bytes, "slice")?;
            record_type_ref(inner, bytes)?;
        }
    }
    Ok(())
}

fn record_hir_declaration(
    declaration: &HirTopLevelDecl,
    symbols: &mut Vec<u8>,
    shape: &mut Vec<u8>,
) -> Result<(), PersistentFactsError> {
    let tag = declaration.cache_fact_tag();
    put_str(symbols, "decl")?;
    put_str(symbols, tag)?;
    put_str(shape, "decl")?;
    put_str(shape, tag)
}

fn record_hir_flow_items(
    items: &[HirFlowItem],
    counts: &mut HirBodyCounts,
    shape: &mut Vec<u8>,
) -> Result<(), PersistentFactsError> {
    for item in items {
        counts.flow_items += 1;
        put_str(shape, item.cache_fact_tag())?;
        match item {
            HirFlowItem::Stmt(_) => counts.statements += 1,
            HirFlowItem::Dialogue(dialogue) => {
                counts.dialogues += 1;
                put_str(shape, dialogue.callee())?;
            }
            HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
                record_choice(choice, counts, shape)?;
            }
            HirFlowItem::LetScope { scope, .. } => {
                put_option_str(shape, scope.name())?;
                put_len(shape, "scope statements", scope.statements().len())?;
                put_bool(shape, scope.value().is_some());
                counts.statements += to_u64("scope statements", scope.statements().len())?;
            }
            HirFlowItem::LetLoop { block, .. } => {
                counts.loops += 1;
                record_hir_flow_items(block.body(), counts, shape)?;
            }
            HirFlowItem::LetAwait { await_with, .. } | HirFlowItem::Await(await_with) => {
                record_await(await_with, counts, shape)?;
            }
            HirFlowItem::Thread(thread) => {
                counts.threads += 1;
                put_option_str(shape, thread.name())?;
                put_bool(shape, thread.is_detached());
                record_hir_flow_items(thread.body(), counts, shape)?;
            }
            HirFlowItem::If(block) => {
                record_hir_flow_items(block.body(), counts, shape)?;
                record_hir_flow_items(block.else_body(), counts, shape)?;
            }
            HirFlowItem::IfLet(block) => {
                record_hir_flow_items(block.body(), counts, shape)?;
                record_hir_flow_items(block.else_body(), counts, shape)?;
            }
            HirFlowItem::Match(block) => {
                put_len(shape, "match arms", block.arms().len())?;
                for arm in block.arms() {
                    record_hir_flow_items(arm.body(), counts, shape)?;
                }
            }
            HirFlowItem::Loop(block) => {
                counts.loops += 1;
                put_option_str(shape, block.label())?;
                record_hir_flow_items(block.body(), counts, shape)?;
            }
            HirFlowItem::While(block) => {
                counts.loops += 1;
                record_hir_flow_items(block.body(), counts, shape)?;
            }
            HirFlowItem::WhileLet(block) => {
                counts.loops += 1;
                record_hir_flow_items(block.body(), counts, shape)?;
            }
            HirFlowItem::For(block) => {
                counts.loops += 1;
                record_hir_flow_items(block.body(), counts, shape)?;
            }
            HirFlowItem::Select(block) => {
                put_len(shape, "select branches", block.branches().len())?;
                for branch in block.branches() {
                    record_hir_flow_items(branch.body(), counts, shape)?;
                }
            }
            HirFlowItem::Borrow(block) => record_hir_flow_items(block.body(), counts, shape)?,
            HirFlowItem::SourceLocale(block) => {
                put_str(shape, block.locale())?;
                record_hir_flow_items(block.body(), counts, shape)?;
            }
            HirFlowItem::Scope(block) => {
                put_option_str(shape, block.name())?;
                record_hir_flow_items(block.body(), counts, shape)?;
            }
            HirFlowItem::Include(_) => counts.includes += 1,
        }
    }
    Ok(())
}

fn record_choice(
    choice: &HirChoice,
    counts: &mut HirBodyCounts,
    shape: &mut Vec<u8>,
) -> Result<(), PersistentFactsError> {
    counts.choices += 1;
    put_len(shape, "choice items", choice.items().len())?;
    put_len(shape, "choice options", choice.options().len())
}

fn record_await(
    await_with: &HirAwait,
    counts: &mut HirBodyCounts,
    shape: &mut Vec<u8>,
) -> Result<(), PersistentFactsError> {
    counts.awaits += 1;
    put_bool(shape, await_with.applies_try());
    put_len(shape, "await branches", await_with.branches().len())?;
    for branch in await_with.branches() {
        put_str(shape, branch.kind().cache_fact_tag())?;
        record_hir_flow_items(branch.body(), counts, shape)?;
    }
    Ok(())
}

fn to_u32(field: &'static str, value: usize) -> Result<u32, PersistentFactsError> {
    u32::try_from(value).map_err(|_| PersistentFactsError::CoordinateTooLarge { field, value })
}

fn to_u64(field: &'static str, value: usize) -> Result<u64, PersistentFactsError> {
    u64::try_from(value).map_err(|_| PersistentFactsError::CoordinateTooLarge { field, value })
}

fn put_len(
    bytes: &mut Vec<u8>,
    field: &'static str,
    len: usize,
) -> Result<(), PersistentFactsError> {
    put_u64(bytes, to_u64(field, len)?);
    Ok(())
}

fn put_str(bytes: &mut Vec<u8>, value: &str) -> Result<(), PersistentFactsError> {
    put_len(bytes, "string", value.len())?;
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn put_option_str(bytes: &mut Vec<u8>, value: Option<&str>) -> Result<(), PersistentFactsError> {
    if let Some(value) = value {
        bytes.push(1);
        put_str(bytes, value)
    } else {
        bytes.push(0);
        Ok(())
    }
}

fn put_bool(bytes: &mut Vec<u8>, value: bool) {
    bytes.push(u8::from(value));
}

fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

#[derive(Default)]
struct SyntaxShapeCounts {
    nodes: u64,
    tokens: u64,
    error_nodes: u64,
}

#[derive(Default)]
struct HirBodyCounts {
    flows: u64,
    functions: u64,
    agents: u64,
    declarations: u64,
    top_level_items: u64,
    flow_items: u64,
    statements: u64,
    dialogues: u64,
    choices: u64,
    loops: u64,
    awaits: u64,
    threads: u64,
    includes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hir::lower_source_tree, parse::parse_source_text};
    use arcweft_project::{
        fingerprint::NamedDigest,
        persistent_object::{AwboEnvelope, AwboError, CompilerBuildIdentity},
    };

    const SOURCE: &str = r#"
pub reducer current_route() -> Ref<Flow> {
return @flow.done
}

flow @flow.opening opening {
let route = current_route()
goto @flow.done
goto route
}

flow @flow.done done {
return "done"
}
"#;

    fn digest(label: &str) -> BuildDigest {
        BuildDigest::of(label.as_bytes())
    }

    fn compiler() -> CompilerBuildIdentity {
        CompilerBuildIdentity {
            package_version: "0.1.0".to_owned(),
            git_commit: "persistent-test".to_owned(),
            rustc: "rustc-test".to_owned(),
            target: "x86_64-unknown-linux-gnu".to_owned(),
            enabled_features: vec!["b".to_owned(), "a".to_owned(), "a".to_owned()],
        }
    }

    fn key(kind: CompilerObjectKind, parsed: &ParsedSource) -> CompilerObjectKey {
        CompilerObjectKey {
            kind,
            compiler: compiler(),
            source_digest: BuildDigest::from_bytes(parsed.source_hash().as_bytes()),
            query_options_digest: digest("options"),
            dependency_interface_digests: vec![
                NamedDigest::new("z", digest("z-interface")),
                NamedDigest::new("a", digest("a-interface")),
            ],
            dependency_body_digests: Vec::new(),
            environment_digest: digest("environment"),
        }
    }

    #[test]
    fn persistent_parse_facts_encode_deterministically() {
        let parsed = parse_source_text(SOURCE);
        assert!(parsed.errors().is_empty());
        let key = key(CompilerObjectKind::ParsedSyntax, &parsed);
        let input = ParsedSyntaxFactsInput {
            key: &key,
            source_label: "src/game.arcw",
            parsed: &parsed,
        };
        let first = AwboEnvelope::new(
            &key,
            parsed_syntax_payload(&input).expect("parse payload builds"),
        )
        .expect("parse envelope builds")
        .encode()
        .expect("parse envelope encodes");
        let input = ParsedSyntaxFactsInput {
            key: &key,
            source_label: "src/game.arcw",
            parsed: &parsed,
        };
        let second = AwboEnvelope::new(
            &key,
            parsed_syntax_payload(&input).expect("parse payload builds"),
        )
        .expect("parse envelope builds")
        .encode()
        .expect("parse envelope encodes");

        assert_eq!(first, second);
        AwboEnvelope::decode(&first, &key).expect("parse envelope decodes");
    }

    #[test]
    fn persistent_hir_body_facts_round_trip_without_hir_serialization() {
        let parsed = parse_source_text(SOURCE);
        assert!(parsed.errors().is_empty());
        let tree = parsed.clone().into_typed_tree();
        let hir = lower_source_tree(&tree).expect("source lowers to HIR");
        let key = key(CompilerObjectKind::HirBody, &parsed);
        let object = hir_body_object(&HirBodyFactsInput {
            key: &key,
            module: "game",
            parsed: &parsed,
            hir: &hir,
        })
        .expect("HIR facts build");

        assert_eq!(object.facts.flow_count, 2);
        assert_eq!(object.facts.function_count, 0);
        assert_eq!(object.body_digest, object.facts.body_shape_digest);

        let bytes = AwboEnvelope::new(&key, CompilerObjectPayload::HirBody(object))
            .expect("HIR envelope builds")
            .encode()
            .expect("HIR envelope encodes");
        let decoded = AwboEnvelope::decode(&bytes, &key).expect("HIR envelope decodes");
        assert!(matches!(decoded.payload, CompilerObjectPayload::HirBody(_)));
    }

    #[test]
    fn persistent_interface_summary_facts_round_trip_without_hir_serialization() {
        let parsed = parse_source_text(SOURCE);
        assert!(parsed.errors().is_empty());
        let tree = parsed.clone().into_typed_tree();
        let hir = lower_source_tree(&tree).expect("source lowers to HIR");
        let key = key(CompilerObjectKind::InterfaceSummary, &parsed);
        let object = interface_summary_object(&InterfaceSummaryFactsInput {
            key: &key,
            module: "game",
            parsed: &parsed,
            hir: &hir,
        })
        .expect("interface summary facts build");

        assert_eq!(object.source_digest, key.source_digest);
        assert_eq!(
            object.imports_digest,
            object.stage_inputs.dependency_interface_digest_root()
        );
        assert_eq!(
            object.exports_digest,
            InterfaceSummaryObject::exports_digest_for(&object.public_symbols)
        );
        assert!(!object.public_symbols.is_empty());

        let bytes = AwboEnvelope::new(&key, CompilerObjectPayload::InterfaceSummary(object))
            .expect("interface envelope builds")
            .encode()
            .expect("interface envelope encodes");
        let decoded = AwboEnvelope::decode(&bytes, &key).expect("interface envelope decodes");
        assert!(matches!(
            decoded.payload,
            CompilerObjectPayload::InterfaceSummary(_)
        ));
    }

    #[test]
    fn persistent_fact_builder_rejects_wrong_key_kind() {
        let parsed = parse_source_text(SOURCE);
        let key = key(CompilerObjectKind::HirBody, &parsed);
        let error = parsed_syntax_object(&ParsedSyntaxFactsInput {
            key: &key,
            source_label: "src/game.arcw",
            parsed: &parsed,
        })
        .expect_err("wrong key kind rejects");

        assert_eq!(
            error,
            PersistentFactsError::WrongObjectKind {
                expected: CompilerObjectKind::ParsedSyntax,
                actual: CompilerObjectKind::HirBody,
            }
        );
    }

    #[test]
    fn persistent_query_soft_miss_does_not_block_source_rebuild() {
        let parsed = parse_source_text(SOURCE);
        assert!(parsed.errors().is_empty());
        let key = key(CompilerObjectKind::ParsedSyntax, &parsed);
        let bytes = AwboEnvelope::new(
            &key,
            parsed_syntax_payload(&ParsedSyntaxFactsInput {
                key: &key,
                source_label: "src/game.arcw",
                parsed: &parsed,
            })
            .expect("parse payload builds"),
        )
        .expect("parse envelope builds")
        .encode()
        .expect("parse envelope encodes");

        let mut changed_key = key.clone();
        changed_key.compiler.git_commit = "changed-compiler".to_owned();
        assert_eq!(
            AwboEnvelope::decode(&bytes, &changed_key)
                .expect_err("changed compiler identity misses"),
            AwboError::KeyDigestMismatch,
        );

        let rebuilt = parse_source_text(SOURCE);
        assert!(rebuilt.errors().is_empty());
        let tree = rebuilt.clone().into_typed_tree();
        lower_source_tree(&tree).expect("source rebuild still lowers to HIR");
    }

    #[test]
    fn persistent_query_actual_bytecode_builder_produces_verified_reusable_payload() {
        let parsed = parse_source_text(SOURCE);
        assert!(parsed.errors().is_empty());
        let tree = parsed.clone().into_typed_tree();
        let hir = lower_source_tree(&tree).expect("source lowers to HIR");
        let interface_key = key(CompilerObjectKind::InterfaceSummary, &parsed);
        let hir_key = key(CompilerObjectKind::HirBody, &parsed);
        let typecheck_key = key(CompilerObjectKind::TypecheckGate, &parsed);
        let bytecode_key = key(CompilerObjectKind::BytecodeUnit, &parsed);
        let interface_summary = interface_summary_object(&InterfaceSummaryFactsInput {
            key: &interface_key,
            module: "game",
            parsed: &parsed,
            hir: &hir,
        })
        .expect("interface facts build");
        let hir_body = hir_body_object(&HirBodyFactsInput {
            key: &hir_key,
            module: "game",
            parsed: &parsed,
            hir: &hir,
        })
        .expect("HIR facts build");
        let typecheck_gate = typecheck_gate_object(&TypecheckGateFactsInput {
            key: &typecheck_key,
            module: "game",
            parsed: &parsed,
            interface_summary: &interface_summary,
            hir_body: &hir_body,
        })
        .expect("typecheck facts build");
        let object = actual_bytecode_unit_object(&ActualBytecodeUnitFactsInput {
            key: &bytecode_key,
            module: "game",
            parsed: &parsed,
            hir_body: &hir_body,
            typecheck_gate: &typecheck_gate,
            runtime_plan_unit_digest: digest("runtime-plan"),
            canonical_awbc_bytes: b"canonical-awbc",
            awbc_schema_digest: digest("awbc-schema"),
            verifier_policy_digest: digest("verifier-policy"),
            codegen_policy_digest: digest("codegen-policy"),
            target_profile_digest: bytecode_key.query_options_digest,
            feature_set_digest: digest("features"),
            relocation_import_table_digest: digest("relocations"),
        })
        .expect("actual bytecode facts build");

        assert_eq!(
            object.reuse_policy,
            BytecodeUnitReusePolicy::VerifiedReusable
        );
        assert_eq!(object.canonical_awbc_bytes, b"canonical-awbc");
        assert_eq!(
            object.facts.bytecode_descriptor_digest,
            BytecodeUnitObject::bytecode_descriptor_digest_for(&object.facts)
        );
        AwboEnvelope::new(&bytecode_key, CompilerObjectPayload::BytecodeUnit(object))
            .expect("actual bytecode envelope validates");
    }

    #[test]
    fn persistent_query_actual_link_builder_keeps_ordered_descriptor() {
        let parsed = parse_source_text(SOURCE);
        assert!(parsed.errors().is_empty());
        let key = key(CompilerObjectKind::LinkPlan, &parsed);
        let ordered = vec![
            NamedDigest::new("b", digest("b-bytecode")),
            NamedDigest::new("a", digest("a-bytecode")),
        ];
        let object = actual_link_plan_object(&ActualLinkPlanFactsInput {
            key: &key,
            package: "game",
            parsed: &parsed,
            ordered_unit_identities: ordered.clone(),
            entrypoint_digest: digest("entrypoints"),
            resource_section_digest: digest("resources"),
            adapter_requirements_digest: digest("adapter-requirements"),
            patch_compatibility_digest: digest("patch-compatibility"),
            product_build_options_digest: digest("product-build-options"),
        })
        .expect("actual link facts build");

        assert_eq!(object.reuse_policy, LinkPlanReusePolicy::VerifiedReusable);
        assert_eq!(object.facts.descriptor.ordered_unit_identities, ordered);
        assert_eq!(
            object.facts.link_descriptor_digest,
            LinkPlanObject::link_descriptor_digest_for(&object.facts.descriptor)
        );
        AwboEnvelope::new(&key, CompilerObjectPayload::LinkPlan(object))
            .expect("actual link envelope validates");
    }
}
