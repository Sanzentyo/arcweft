//! Compiler-private persistent fact builders for safe `.awbo` payloads.
//!
//! This module projects syntax and HIR into stable evidence objects owned by
//! `arcweft-project::persistent_object`. It intentionally does not perform cache
//! reads, cache writes, or semantic/typecheck reuse.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::TypeId,
    item::{
        HirFunctionItem, HirGenericParameter, HirItemFamily, HirItemKind, HirParameter,
        HirParameterKind, HirRequiredName, HirWherePredicate,
    },
    leaf::{HirPath, HirPathRoot, HirPathSegment, HirTypeRegion},
    module::HirModule,
    stmt::{HirConditionalElseBranch, HirContextualStmtBody, HirStmtKind, HirStmtMatchArmBody},
    type_ref::HirTypeKind,
};
use arcweft_lang_syntax::{
    attachment::{SyntaxAccessError, SyntaxNode, TypedItemNode},
    grammar::SyntaxKind,
    incremental::{ParsedSource, SyntaxDiagnostic, SyntaxParseStats},
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
    #[error(transparent)]
    AttachedSyntax(#[from] SyntaxAccessError),
}

/// Builds a parsed-syntax payload object without serializing parser internals.
pub fn parsed_syntax_object(
    input: &ParsedSyntaxFactsInput<'_>,
) -> Result<ParsedSyntaxObject, PersistentFactsError> {
    ensure_key_kind(input.key, CompilerObjectKind::ParsedSyntax)?;
    let source_digest = BuildDigest::from(input.parsed.document().identity().revision());
    let line_index = StableLineIndex::new(input.parsed.source());
    let diagnostics = input
        .parsed
        .diagnostics()
        .iter()
        .map(|diagnostic| syntax_diagnostic(diagnostic, &line_index))
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
        stats: syntax_stats(input.parsed.syntax_stats())?,
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
    let source_digest = BuildDigest::from(input.parsed.document().identity().revision());
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
    let source_digest = BuildDigest::from(input.parsed.document().identity().revision());
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

/// Builds a stable typecheck gate object without serializing the HIR project or a
/// `TypeCheckReport`.
pub fn typecheck_gate_object(
    input: &TypecheckGateFactsInput<'_>,
) -> Result<TypecheckGateObject, PersistentFactsError> {
    ensure_key_kind(input.key, CompilerObjectKind::TypecheckGate)?;
    let source_digest = BuildDigest::from(input.parsed.document().identity().revision());
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
    let source_digest = BuildDigest::from(input.parsed.document().identity().revision());
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
    let source_digest = BuildDigest::from(input.parsed.document().identity().revision());
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
    let source_digest = BuildDigest::from(input.parsed.document().identity().revision());
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
    let source_digest = BuildDigest::from(input.parsed.document().identity().revision());
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

/// Deterministic UTF-8 line projection used only by the persistent codec.
///
/// The incremental parser owns syntax identity; this projection derives
/// presentation coordinates from the same immutable document without adding
/// a second parsed-source authority.
struct StableLineIndex {
    starts: Vec<usize>,
}

impl StableLineIndex {
    fn new(source: &str) -> Self {
        let mut starts = vec![0];
        for (offset, character) in source.char_indices() {
            if character == '\n' {
                starts.push(offset + character.len_utf8());
            }
        }
        Self { starts }
    }

    fn starts(&self) -> &[usize] {
        &self.starts
    }

    fn line_col(&self, offset: usize) -> (usize, usize) {
        let line = self.starts.partition_point(|start| *start <= offset);
        let line = line.saturating_sub(1);
        (line, offset.saturating_sub(self.starts[line]))
    }
}

fn source_span(parsed: &ParsedSource) -> Result<StableSourceSpanObject, PersistentFactsError> {
    stable_span_for_offsets(
        0,
        parsed.source().len(),
        &StableLineIndex::new(parsed.source()),
    )
}

fn stable_span_for_range(
    range: arcweft_source::SourceRange,
    line_index: &StableLineIndex,
) -> Result<StableSourceSpanObject, PersistentFactsError> {
    stable_span_for_offsets(range.start(), range.end(), line_index)
}

fn stable_span_for_offsets(
    start: usize,
    end: usize,
    line_index: &StableLineIndex,
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

fn syntax_stats(stats: SyntaxParseStats) -> Result<SyntaxStatsObject, PersistentFactsError> {
    Ok(SyntaxStatsObject {
        accepted_source_bytes: to_u64("accepted source bytes", stats.accepted_source_bytes())?,
        lexer_tokens: to_u64("lexer tokens", stats.lexer_tokens())?,
        grammar_events: to_u64("grammar events", stats.grammar_events())?,
        top_level_items: to_u64("top-level items", stats.top_level_items())?,
        statements: to_u64("statements", stats.statements())?,
        expressions: to_u64("expressions", stats.expressions())?,
        type_nodes: to_u64("type nodes", stats.type_nodes())?,
        pattern_nodes: to_u64("pattern nodes", stats.pattern_nodes())?,
        identity_bearing_nodes: to_u64("identity-bearing nodes", stats.identity_bearing_nodes())?,
        diagnostic_identities: to_u64("diagnostic identities", stats.diagnostic_identities())?,
    })
}

fn syntax_diagnostic(
    diagnostic: &SyntaxDiagnostic,
    line_index: &StableLineIndex,
) -> Result<StableDiagnosticObject, PersistentFactsError> {
    Ok(StableDiagnosticObject {
        code: diagnostic.code().to_owned(),
        severity: StableDiagnosticSeverity::Error,
        message: diagnostic.message().to_owned(),
        primary_span: Some(stable_span_for_range(
            diagnostic.primary().range(),
            line_index,
        )?),
        related_spans: diagnostic
            .related()
            .map(|span| stable_span_for_range(span.range(), line_index))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn parsed_syntax_evidence(
    parsed: &ParsedSource,
) -> Result<ParsedSyntaxEvidenceObject, PersistentFactsError> {
    let mut counts = SyntaxShapeCounts::default();
    let mut shape_bytes = Vec::new();
    let root = parsed.root_syntax();
    let rowan_root = root.rowan();
    record_syntax_node(rowan_root, &mut counts, &mut shape_bytes)?;
    let attributes = parsed.attributes()?;
    let items = parsed.items()?;
    let use_count = items
        .iter()
        .filter(|item| matches!(item, TypedItemNode::Use(_)))
        .count();
    let item_count = items
        .iter()
        .filter(|item| !matches!(item, TypedItemNode::Module(_) | TypedItemNode::Use(_)))
        .count();
    Ok(ParsedSyntaxEvidenceObject {
        root_kind: u32::from(root.kind() as u16),
        cst_shape_digest: BuildDigest::of(&shape_bytes),
        line_index_digest: line_index_digest(&StableLineIndex::new(parsed.source()))?,
        cst_node_count: counts.nodes,
        cst_token_count: counts.tokens,
        cst_error_node_count: counts.error_nodes,
        typed_attribute_count: to_u64("typed attributes", attributes.len())?,
        typed_use_count: to_u64("typed uses", use_count)?,
        typed_item_count: to_u64("typed items", item_count)?,
    })
}

fn line_index_digest(line_index: &StableLineIndex) -> Result<BuildDigest, PersistentFactsError> {
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
    let kind = node.kind().0;
    if is_error_syntax_kind(kind) {
        counts.error_nodes += 1;
    }
    put_str(bytes, "node")?;
    put_u32(bytes, u32::from(kind));
    let range = node.text_range();
    put_u32(bytes, range.start().into());
    put_u32(bytes, range.end().into());
    for element in node.children_with_tokens() {
        if let Some(child) = element.as_node() {
            record_syntax_node(child, counts, bytes)?;
        } else if let Some(token) = element.as_token() {
            counts.tokens += 1;
            put_str(bytes, "token")?;
            put_u32(bytes, u32::from(token.kind().0));
            let range = token.text_range();
            put_u32(bytes, range.start().into());
            put_u32(bytes, range.end().into());
        }
    }
    Ok(())
}

const fn is_error_syntax_kind(kind: u16) -> bool {
    kind == SyntaxKind::ErrorItem as u16
        || kind == SyntaxKind::ErrorDeclarationMember as u16
        || kind == SyntaxKind::ErrorStatement as u16
        || kind == SyntaxKind::ErrorExpression as u16
        || kind == SyntaxKind::ErrorPattern as u16
        || kind == SyntaxKind::ErrorType as u16
        || kind == SyntaxKind::RichTextInvalidArgument as u16
        || kind == SyntaxKind::RichTextInvalidArgumentIssue as u16
        || kind == SyntaxKind::ErrorNode as u16
}

fn hir_body_facts(
    module: &str,
    hir: &HirModule,
) -> Result<HirBodyFactsObject, PersistentFactsError> {
    let mut counts = HirBodyCounts::default();
    let mut symbols = Vec::new();
    let mut shape = Vec::new();
    let mut item_tags = BTreeMap::new();
    let mut expression_tags = BTreeMap::new();
    let mut statement_tags = BTreeMap::new();
    let mut attribute_count = 0_usize;

    put_str(&mut symbols, module)?;
    for &item_id in hir.source_ordered_items() {
        let item = hir
            .resolve_item(item_id)
            .expect("accepted HIR source item remains live");
        let family = item.family();
        *item_tags.entry(item_family_tag(family)).or_insert(0_usize) += 1;
        attribute_count = attribute_count
            .checked_add(item.prefix().attributes().len())
            .ok_or(PersistentFactsError::CoordinateTooLarge {
                field: "hir attributes",
                value: usize::MAX,
            })?;
        record_item_symbol(hir, item.kind(), item.prefix().attributes(), &mut symbols)?;
        match item.kind() {
            HirItemKind::Flow(flow) => {
                counts.flows += 1;
                counts.flow_items += to_u64("flow body", flow.body().items().len())?;
            }
            HirItemKind::Function(_) => counts.functions += 1,
            kind if is_persistent_declaration(kind.family()) => counts.declarations += 1,
            _ => {}
        }
    }

    for (_, expression) in hir.expressions() {
        let tag = expression_kind_tag(expression.kind());
        *expression_tags.entry(tag).or_insert(0_usize) += 1;
        match expression.kind() {
            HirExprKind::DialogueContentApplication(_) => counts.dialogues += 1,
            HirExprKind::Choice(_) => counts.choices += 1,
            HirExprKind::Await(awaited) => {
                counts.awaits += 1;
                let branch_flow_items = awaited
                    .branches()
                    .iter()
                    .map(|branch| contextual_thread_flow_item_count(branch.body()))
                    .sum::<usize>();
                counts.flow_items += to_u64("Await branch flow items", branch_flow_items)?;
            }
            HirExprKind::Thread(thread) => {
                counts.threads += 1;
                counts.flow_items += to_u64("thread body", thread.body().items().len())?;
            }
            HirExprKind::Loop(_) => counts.loops += 1,
            _ => {}
        }
    }

    counts.statements = to_u64("statements", hir.statements().len())?;
    for (_, statement) in hir.statements() {
        let tag = statement_kind_tag(statement.kind());
        *statement_tags.entry(tag).or_insert(0_usize) += 1;
        counts.flow_items += to_u64(
            "nested thread body",
            immediate_thread_flow_item_count(statement.kind()),
        )?;
        match statement.kind() {
            HirStmtKind::While(_) | HirStmtKind::WhileLet(_) | HirStmtKind::For(_) => {
                counts.loops += 1
            }
            HirStmtKind::Include(_) => counts.includes += 1,
            _ => {}
        }
    }

    // A body dependency digest is deliberately conservative. Binding it to
    // the accepted source revision guarantees that any authored body change
    // invalidates downstream body consumers without reopening source text or
    // serializing qualified arena IDs whose numeric slots are not persistent
    // identities.
    shape.extend_from_slice(hir.provenance().source_identity().revision().as_bytes());
    record_tag_counts(&mut shape, "item kinds", &item_tags)?;
    record_tag_counts(&mut shape, "expression kinds", &expression_tags)?;
    record_tag_counts(&mut shape, "statement kinds", &statement_tags)?;
    put_len(&mut shape, "hir attributes", attribute_count)?;

    Ok(HirBodyFactsObject {
        attribute_count: to_u64("hir attributes", attribute_count)?,
        flow_count: counts.flows,
        function_count: counts.functions,
        declaration_count: counts.declarations,
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
    let mut declaration_ordinal = 0_usize;
    for &item_id in hir.source_ordered_items() {
        let item = hir
            .resolve_item(item_id)
            .expect("accepted HIR source item remains live");
        match item.kind() {
            HirItemKind::Flow(flow) => {
                let Some(name) = flow.identity().name() else {
                    continue;
                };
                symbols.push(public_symbol(
                    PublicSymbolKind::Flow,
                    module,
                    name.as_str(),
                    flow_signature_digest(hir, name.as_str(), flow)?,
                ));
            }
            HirItemKind::Function(function) => {
                let Some(name) = function.name().resolved() else {
                    continue;
                };
                symbols.push(public_symbol(
                    PublicSymbolKind::Function,
                    module,
                    name.as_str(),
                    function_signature_digest(hir, name.as_str(), function)?,
                ));
            }
            kind if is_persistent_declaration(kind.family()) => {
                let tag = item_family_tag(kind.family());
                let name = format!("decl:{declaration_ordinal}:{tag}");
                symbols.push(public_symbol(
                    PublicSymbolKind::Declaration,
                    module,
                    &name,
                    declaration_signature_digest(tag)?,
                ));
                declaration_ordinal += 1;
            }
            _ => {}
        }
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

fn function_signature_digest(
    hir: &HirModule,
    name: &str,
    function: &HirFunctionItem,
) -> Result<BuildDigest, PersistentFactsError> {
    let mut bytes = Vec::new();
    put_str(&mut bytes, "function")?;
    put_str(&mut bytes, name)?;
    record_generic_parameters(hir, function.generic_parameters(), &mut bytes)?;
    put_len(
        &mut bytes,
        "function parameter groups",
        function.parameter_groups().len(),
    )?;
    for group in function.parameter_groups() {
        record_parameters(hir, group.parameters(), &mut bytes)?;
    }
    record_optional_type(hir, function.return_type(), &mut bytes)?;
    record_where_predicates(hir, function.where_predicates(), &mut bytes)?;
    Ok(BuildDigest::of(&bytes))
}

fn flow_signature_digest(
    hir: &HirModule,
    name: &str,
    flow: &arcweft_lang_hir::item::HirFlowItem,
) -> Result<BuildDigest, PersistentFactsError> {
    let mut bytes = Vec::new();
    put_str(&mut bytes, "flow")?;
    put_str(&mut bytes, name)?;
    record_generic_parameters(hir, flow.generic_parameters(), &mut bytes)?;
    put_len(&mut bytes, "flow parameter groups", 1)?;
    record_parameters(hir, flow.parameters(), &mut bytes)?;
    record_optional_type(hir, flow.result().authored_type(), &mut bytes)?;
    record_where_predicates(hir, flow.where_predicates(), &mut bytes)?;
    Ok(BuildDigest::of(&bytes))
}

fn declaration_signature_digest(tag: &str) -> Result<BuildDigest, PersistentFactsError> {
    let mut bytes = Vec::new();
    put_str(&mut bytes, "declaration")?;
    put_str(&mut bytes, tag)?;
    put_str(&mut bytes, "no-signature")?;
    Ok(BuildDigest::of(&bytes))
}

fn record_generic_parameters(
    hir: &HirModule,
    parameters: &[HirGenericParameter],
    bytes: &mut Vec<u8>,
) -> Result<(), PersistentFactsError> {
    put_len(bytes, "generic params", parameters.len())?;
    for parameter in parameters {
        match parameter {
            HirGenericParameter::Lifetime { name } => {
                put_str(bytes, "lifetime")?;
                record_required_name(name, bytes)?;
            }
            HirGenericParameter::Type { name, bounds } => {
                put_str(bytes, "type")?;
                record_required_name(name, bytes)?;
                put_len(bytes, "type bounds", bounds.len())?;
                for &bound in bounds {
                    record_type_ref(hir, bound, bytes)?;
                }
            }
        }
    }
    Ok(())
}

fn record_parameters(
    hir: &HirModule,
    parameters: &[HirParameter],
    bytes: &mut Vec<u8>,
) -> Result<(), PersistentFactsError> {
    put_len(bytes, "params", parameters.len())?;
    for parameter in parameters {
        put_str(
            bytes,
            match parameter.kind() {
                HirParameterKind::Fixed => "fixed",
                HirParameterKind::ExtensionReceiver => "extension-receiver",
                HirParameterKind::RestPositional => "rest-positional",
            },
        )?;
        record_type_ref(hir, parameter.ty(), bytes)?;
        put_bool(bytes, parameter.default().is_some());
    }
    Ok(())
}

fn record_optional_type(
    hir: &HirModule,
    ty: Option<TypeId>,
    bytes: &mut Vec<u8>,
) -> Result<(), PersistentFactsError> {
    if let Some(ty) = ty {
        put_bool(bytes, true);
        record_type_ref(hir, ty, bytes)?;
    } else {
        put_bool(bytes, false);
    }
    Ok(())
}

fn record_where_predicates(
    hir: &HirModule,
    predicates: &[HirWherePredicate],
    bytes: &mut Vec<u8>,
) -> Result<(), PersistentFactsError> {
    put_len(bytes, "where clauses", predicates.len())?;
    for predicate in predicates {
        record_type_ref(hir, predicate.subject(), bytes)?;
        put_len(bytes, "where bounds", predicate.bounds().len())?;
        for &bound in predicate.bounds() {
            record_type_ref(hir, bound, bytes)?;
        }
    }
    Ok(())
}

fn record_type_ref(
    hir: &HirModule,
    ty: TypeId,
    bytes: &mut Vec<u8>,
) -> Result<(), PersistentFactsError> {
    let ty = hir
        .resolve_type(ty)
        .expect("accepted HIR signature type remains live");
    match ty.kind() {
        HirTypeKind::Never => put_str(bytes, "never")?,
        HirTypeKind::ConstInt(value) => {
            put_str(bytes, "const-int")?;
            put_len(bytes, "const int", *value)?;
        }
        HirTypeKind::Path(path) => {
            put_str(bytes, "path")?;
            record_hir_path(path, bytes)?;
        }
        HirTypeKind::Tuple(items) => {
            put_str(bytes, "tuple")?;
            put_len(bytes, "tuple items", items.len())?;
            for &item in items {
                record_type_ref(hir, item, bytes)?;
            }
        }
        HirTypeKind::Function(function) => {
            put_str(bytes, "function")?;
            put_len(bytes, "function params", function.parameters().len())?;
            for &parameter in function.parameters() {
                record_type_ref(hir, parameter, bytes)?;
            }
            record_type_ref(hir, function.return_type(), bytes)?;
            match function.effects() {
                Some(effects) => {
                    put_str(bytes, "effects")?;
                    put_len(bytes, "function effect row", effects.effects().len())?;
                    for effect in effects.effects() {
                        put_str(bytes, effect.as_str())?;
                    }
                }
                None => put_str(bytes, "effects-unknown")?,
            }
        }
        HirTypeKind::Choice(alternatives) => {
            put_str(bytes, "choice")?;
            put_len(bytes, "choice alternatives", alternatives.len())?;
            for &alternative in alternatives {
                record_type_ref(hir, alternative, bytes)?;
            }
        }
        HirTypeKind::Generic(generic) => {
            put_str(bytes, "generic")?;
            record_hir_path(generic.base(), bytes)?;
            put_len(bytes, "generic args", generic.arguments().len())?;
            for &argument in generic.arguments() {
                record_type_ref(hir, argument, bytes)?;
            }
        }
        HirTypeKind::TraitBound(bound) => {
            put_str(bytes, "trait-bound")?;
            record_hir_path(bound.base(), bytes)?;
            put_len(bytes, "trait bound args", bound.arguments().len())?;
            for &argument in bound.arguments() {
                record_type_ref(hir, argument, bytes)?;
            }
            put_len(bytes, "associated type bindings", bound.associated().len())?;
            for binding in bound.associated() {
                put_str(bytes, binding.name().as_str())?;
                record_type_ref(hir, binding.value(), bytes)?;
            }
        }
        HirTypeKind::Projection(projection) => {
            put_str(bytes, "projection")?;
            record_type_ref(hir, projection.subject(), bytes)?;
            put_str(bytes, projection.associated().as_str())?;
        }
        HirTypeKind::Reference(reference) => {
            put_str(bytes, "ref")?;
            put_str(
                bytes,
                match reference.kind() {
                    arcweft_lang_hir::expr::HirBorrowKind::Shared => "shared",
                    arcweft_lang_hir::expr::HirBorrowKind::Mutable => "mutable",
                },
            )?;
            match reference.region() {
                Some(HirTypeRegion::Named(region)) => {
                    put_str(bytes, "named-region")?;
                    put_str(bytes, region.name().as_str())?;
                }
                Some(HirTypeRegion::Elided(_)) => put_str(bytes, "elided-region")?,
                None => put_str(bytes, "no-region")?,
            }
            record_type_ref(hir, reference.referent(), bytes)?;
        }
        HirTypeKind::Slice(inner) => {
            put_str(bytes, "slice")?;
            record_type_ref(hir, *inner, bytes)?;
        }
        HirTypeKind::Recovery(_) => {
            put_str(bytes, "recovery")?;
        }
    }
    Ok(())
}

fn record_hir_path(path: &HirPath, bytes: &mut Vec<u8>) -> Result<(), PersistentFactsError> {
    match path.root() {
        HirPathRoot::ImplicitCrate => put_str(bytes, "implicit-crate")?,
        HirPathRoot::Crate => put_str(bytes, "crate")?,
        HirPathRoot::SelfModule => put_str(bytes, "self")?,
        HirPathRoot::Super { depth } => {
            put_str(bytes, "super")?;
            put_len(bytes, "super depth", depth)?;
        }
    }
    put_len(bytes, "path segments", path.segments().len())?;
    for segment in path.segments() {
        match segment {
            HirPathSegment::Identifier(name) => {
                put_str(bytes, "identifier")?;
                put_str(bytes, name.as_str())?;
            }
            HirPathSegment::ProjectSymbol(name) => {
                put_str(bytes, "project-symbol")?;
                put_str(bytes, name.as_str())?;
            }
        }
    }
    Ok(())
}

fn record_required_name(
    name: &HirRequiredName,
    bytes: &mut Vec<u8>,
) -> Result<(), PersistentFactsError> {
    match name {
        HirRequiredName::Resolved(name) => {
            put_str(bytes, "resolved")?;
            put_str(bytes, name.as_str())
        }
        HirRequiredName::Missing => put_str(bytes, "missing"),
        HirRequiredName::Invalid => put_str(bytes, "invalid"),
    }
}

fn record_item_symbol(
    hir: &HirModule,
    kind: &HirItemKind,
    attributes: &[arcweft_lang_hir::item::HirAttribute],
    symbols: &mut Vec<u8>,
) -> Result<(), PersistentFactsError> {
    let tag = item_family_tag(kind.family());
    put_str(symbols, tag)?;
    put_len(symbols, "item attributes", attributes.len())?;
    for attribute in attributes {
        record_hir_path(attribute.path(), symbols)?;
        put_len(symbols, "attribute arguments", attribute.arguments().len())?;
    }
    match kind {
        HirItemKind::Flow(flow) => {
            put_option_str(
                symbols,
                flow.identity()
                    .name()
                    .map(arcweft_lang_hir::leaf::HirName::as_str),
            )?;
            record_generic_parameters(hir, flow.generic_parameters(), symbols)?;
            record_parameters(hir, flow.parameters(), symbols)?;
            record_optional_type(hir, flow.result().authored_type(), symbols)?;
            record_where_predicates(hir, flow.where_predicates(), symbols)
        }
        HirItemKind::Function(function) => {
            record_required_name(function.name(), symbols)?;
            record_generic_parameters(hir, function.generic_parameters(), symbols)?;
            for group in function.parameter_groups() {
                record_parameters(hir, group.parameters(), symbols)?;
            }
            record_optional_type(hir, function.return_type(), symbols)?;
            record_where_predicates(hir, function.where_predicates(), symbols)
        }
        _ => Ok(()),
    }
}

fn is_persistent_declaration(family: HirItemFamily) -> bool {
    !matches!(
        family,
        HirItemFamily::Module
            | HirItemFamily::Use
            | HirItemFamily::Flow
            | HirItemFamily::Function
            | HirItemFamily::Error
    )
}

const fn item_family_tag(family: HirItemFamily) -> &'static str {
    match family {
        HirItemFamily::Module => "module",
        HirItemFamily::Use => "use",
        HirItemFamily::Flow => "flow",
        HirItemFamily::Function => "function",
        HirItemFamily::Predicate => "predicate",
        HirItemFamily::Proof => "proof",
        HirItemFamily::Trait => "trait",
        HirItemFamily::Impl => "impl",
        HirItemFamily::Enum => "enum",
        HirItemFamily::Struct => "struct",
        HirItemFamily::TypeAlias => "type-alias",
        HirItemFamily::Resource => "resource",
        HirItemFamily::Character => "character",
        HirItemFamily::View => "view",
        HirItemFamily::Action => "action",
        HirItemFamily::Activity => "activity",
        HirItemFamily::Signal => "signal",
        HirItemFamily::Metric => "metric",
        HirItemFamily::Layer => "layer",
        HirItemFamily::Entry => "entry",
        HirItemFamily::ExternCapability => "extern-capability",
        HirItemFamily::Test => "test",
        HirItemFamily::Bench => "bench",
        HirItemFamily::Style => "style",
        HirItemFamily::Error => "error",
    }
}

const fn expression_kind_tag(kind: &HirExprKind) -> &'static str {
    match kind {
        HirExprKind::Unit => "unit",
        HirExprKind::Literal(_) => "literal",
        HirExprKind::EntityReference(_) => "entity-reference",
        HirExprKind::LifetimePath(_) => "lifetime-path",
        HirExprKind::Path(_) => "path",
        HirExprKind::ShortVariant(_) => "short-variant",
        HirExprKind::Placeholder(_) => "placeholder",
        HirExprKind::Tuple(_) => "tuple",
        HirExprKind::BracketSequence(_) => "bracket-sequence",
        HirExprKind::NumericBracketSequence(_) => "numeric-bracket-sequence",
        HirExprKind::ArrayRepeat(_) => "array-repeat",
        HirExprKind::Call(_) => "call",
        HirExprKind::Select(_) => "select",
        HirExprKind::Index(_) => "index",
        HirExprKind::Pipe(_) => "pipe",
        HirExprKind::Try(_) => "try",
        HirExprKind::Await(_) => "await",
        HirExprKind::Thread(_) => "thread",
        HirExprKind::Choice(_) => "choice",
        HirExprKind::Range(_) => "range",
        HirExprKind::Record(_) => "record",
        HirExprKind::RecordLiteral(_) => "record-literal",
        HirExprKind::Binary(_) => "binary",
        HirExprKind::Borrow(_) => "borrow",
        HirExprKind::Dereference(_) => "dereference",
        HirExprKind::Closure(_) => "closure",
        HirExprKind::Unary(_) => "unary",
        HirExprKind::Block(_) => "block",
        HirExprKind::ComputationBlock(_) => "computation-block",
        HirExprKind::NamedBlock(_) => "named-block",
        HirExprKind::Loop(_) => "loop",
        HirExprKind::If(_) => "if",
        HirExprKind::IfLet(_) => "if-let",
        HirExprKind::Match(_) => "match",
        HirExprKind::DialogueContentApplication(_) => "dialogue-content-application",
        HirExprKind::PostfixBracket(_) => "postfix-bracket",
        HirExprKind::Error(_) => "error",
        HirExprKind::ForSynthetic(_) => "for-synthetic",
    }
}

const fn statement_kind_tag(kind: &HirStmtKind) -> &'static str {
    match kind {
        HirStmtKind::Assertion { .. } => "assertion",
        HirStmtKind::Let { .. } => "let",
        HirStmtKind::Assign { .. } => "assign",
        HirStmtKind::LetElse { .. } => "let-else",
        HirStmtKind::LetChoice { .. } => "let-choice",
        HirStmtKind::LetScope { .. } => "let-scope",
        HirStmtKind::LetActionReceive { .. } => "let-action-receive",
        HirStmtKind::Return { .. } => "return",
        HirStmtKind::Out { .. } => "out",
        HirStmtKind::Goto { .. } => "goto",
        HirStmtKind::DeferBlock { .. } => "defer-block",
        HirStmtKind::Defer { .. } => "defer",
        HirStmtKind::Yield { .. } => "yield",
        HirStmtKind::Signal { .. } => "signal",
        HirStmtKind::LifetimeSet { .. } => "lifetime-set",
        HirStmtKind::Wait { .. } => "wait",
        HirStmtKind::On { .. } => "on",
        HirStmtKind::UnsafeLifetime { .. } => "unsafe-lifetime",
        HirStmtKind::Choice { .. } => "choice",
        HirStmtKind::If(_) => "if",
        HirStmtKind::IfLet(_) => "if-let",
        HirStmtKind::Match(_) => "match",
        HirStmtKind::While(_) => "while",
        HirStmtKind::WhileLet(_) => "while-let",
        HirStmtKind::For(_) => "for",
        HirStmtKind::Close { .. } => "close",
        HirStmtKind::Select(_) => "select",
        HirStmtKind::SourceLocale(_) => "source-locale",
        HirStmtKind::Scope(_) => "scope",
        HirStmtKind::Include(_) => "include",
        HirStmtKind::Break { .. } => "break",
        HirStmtKind::Continue { .. } => "continue",
        HirStmtKind::Expression { .. } => "expression",
        HirStmtKind::ProofCall { .. } => "proof-call",
        HirStmtKind::Error => "error",
    }
}

fn immediate_thread_flow_item_count(kind: &HirStmtKind) -> usize {
    match kind {
        HirStmtKind::If(statement) => {
            contextual_thread_flow_item_count(statement.then_body())
                + statement
                    .else_branch()
                    .map_or(0, conditional_else_thread_flow_item_count)
        }
        HirStmtKind::IfLet(statement) => {
            contextual_thread_flow_item_count(statement.then_body())
                + statement
                    .else_branch()
                    .map_or(0, conditional_else_thread_flow_item_count)
        }
        HirStmtKind::Match(statement) => statement
            .arms()
            .iter()
            .map(|arm| match arm.body() {
                HirStmtMatchArmBody::Expression(_) => 0,
                HirStmtMatchArmBody::Body(body) => contextual_thread_flow_item_count(body),
            })
            .sum(),
        HirStmtKind::While(statement) => contextual_thread_flow_item_count(statement.body()),
        HirStmtKind::WhileLet(statement) => contextual_thread_flow_item_count(statement.body()),
        HirStmtKind::For(statement) => contextual_thread_flow_item_count(statement.body()),
        HirStmtKind::Select(statement) => statement
            .branches()
            .iter()
            .map(|branch| contextual_thread_flow_item_count(branch.body()))
            .sum(),
        HirStmtKind::SourceLocale(statement) => contextual_thread_flow_item_count(statement.body()),
        HirStmtKind::Scope(statement) => contextual_thread_flow_item_count(statement.body()),
        _ => 0,
    }
}

fn conditional_else_thread_flow_item_count(branch: &HirConditionalElseBranch) -> usize {
    match branch {
        HirConditionalElseBranch::Body(body) => contextual_thread_flow_item_count(body),
        HirConditionalElseBranch::ElseIf(_) => 0,
    }
}

fn contextual_thread_flow_item_count(body: &HirContextualStmtBody) -> usize {
    body.thread_body().map_or(0, |body| body.items().len())
}

fn record_tag_counts(
    bytes: &mut Vec<u8>,
    field: &'static str,
    tags: &BTreeMap<&'static str, usize>,
) -> Result<(), PersistentFactsError> {
    put_len(bytes, field, tags.len())?;
    for (&tag, &count) in tags {
        put_str(bytes, tag)?;
        put_len(bytes, field, count)?;
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
    declarations: u64,
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
    use arcweft_lang_hir::{
        database::HirDatabase,
        lowering::{HirModuleKey, LoweringRequest},
        proof_return::HirProofReturnSemanticFactSet,
        symbol::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId},
    };
    use arcweft_lang_syntax::{
        ast::module_path::CanonicalModulePath, incremental::SyntaxDatabase, parser::ParseOptions,
    };
    use arcweft_project::{
        fingerprint::NamedDigest,
        persistent_object::{AwboEnvelope, AwboError, CompilerBuildIdentity},
    };
    use arcweft_source::{
        SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId,
    };
    use std::sync::Arc;

    const SOURCE: &str = r#"
pub fn current_route() -> Ref<Flow> {
return @flow.done
}

flow opening {
let route = current_route()
goto @flow.done
goto route
}

flow done {
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
            source_digest: BuildDigest::from(parsed.document().identity().revision()),
            query_options_digest: digest("options"),
            dependency_interface_digests: vec![
                NamedDigest::new("z", digest("z-interface")),
                NamedDigest::new("a", digest("a-interface")),
            ],
            dependency_body_digests: Vec::new(),
            environment_digest: digest("environment"),
        }
    }

    fn parse_attached_document(document: Arc<SourceDocument>) -> ParsedSource {
        let snapshot = SourceSnapshotId::initial(document.display_name().clone());
        SyntaxDatabase::try_new()
            .expect("persistent test syntax database")
            .parse_initial(snapshot, document, ParseOptions::default())
            .expect("persistent test source parses")
    }

    fn lower_attached_hir(parsed: &ParsedSource) -> Arc<HirModule> {
        let package = CallablePackageId::try_new("compiler-persistent-tests")
            .expect("persistent test package ID");
        let path = CanonicalModulePath::crate_root();
        let key = HirModuleKey::new(package.clone(), path, parsed.document().identity().clone());
        let mut database = HirDatabase::try_new().expect("persistent test HIR database");
        let world = ProjectSymbolWorldId::try_new(
            package,
            parsed.document().identity().id().clone(),
            "compiler-persistent-tests",
        )
        .expect("persistent test symbol world");
        let revision = ProjectSymbolRevision::try_for_documents([parsed.document().identity()])
            .expect("persistent test symbol revision");
        let transaction =
            database
                .stage_proof_return_project(
                    [LoweringRequest::try_new(key, parsed)
                        .expect("bound persistent lowering request")],
                    world,
                    revision,
                    [parsed.document().identity()],
                    arcweft_lang_hir::lowering::HirLoweringControl::new(),
                )
                .expect("attached persistent project stages");
        let facts = HirProofReturnSemanticFactSet::try_new(
            Arc::clone(transaction.generation()),
            transaction.headers().cloned(),
            [],
        )
        .expect("persistent fixture has no authored Proof return headers");
        let mut outputs = transaction
            .publish_with_semantic_facts(&mut database, facts)
            .expect("attached persistent project publishes");
        let module = outputs.pop().expect("one persistent fixture module");
        assert!(outputs.is_empty());
        module.into_module()
    }

    #[test]
    fn persistent_parse_facts_encode_deterministically() {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://compiler/persistent/game.arcw")
                    .expect("persistent fixture source ID"),
                SourceName::path("compiler/persistent/game.arcw"),
                SOURCE,
            )
            .expect("persistent fixture source document"),
        );
        let parsed = parse_attached_document(Arc::clone(&document));
        assert!(parsed.diagnostics().is_empty());
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
    fn persistent_facts_are_identical_across_syntax_and_hir_sessions() {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://compiler/persistent/session.arcw")
                    .expect("persistent fixture source ID"),
                SourceName::path("compiler/persistent/session.arcw"),
                SOURCE,
            )
            .expect("persistent fixture source document"),
        );
        let first_parsed = parse_attached_document(Arc::clone(&document));
        let second_parsed = parse_attached_document(Arc::clone(&document));
        assert_ne!(
            first_parsed.snapshot_id().lineage().database(),
            second_parsed.snapshot_id().lineage().database()
        );

        let parse_key = key(CompilerObjectKind::ParsedSyntax, &first_parsed);
        let encode_parse = |parsed: &ParsedSource| {
            AwboEnvelope::new(
                &parse_key,
                parsed_syntax_payload(&ParsedSyntaxFactsInput {
                    key: &parse_key,
                    source_label: "src/session.arcw",
                    parsed,
                })
                .expect("parse payload builds"),
            )
            .expect("parse envelope builds")
            .encode()
            .expect("parse envelope encodes")
        };
        assert_eq!(encode_parse(&first_parsed), encode_parse(&second_parsed));

        let first_hir = lower_attached_hir(&first_parsed);
        let second_hir = lower_attached_hir(&second_parsed);
        assert_ne!(
            first_hir.snapshot_id().module().database(),
            second_hir.snapshot_id().module().database()
        );
        let hir_key = key(CompilerObjectKind::HirBody, &first_parsed);
        let encode_hir = |parsed: &ParsedSource, hir: &HirModule| {
            let object = hir_body_object(&HirBodyFactsInput {
                key: &hir_key,
                module: "session",
                parsed,
                hir,
            })
            .expect("HIR facts build");
            AwboEnvelope::new(&hir_key, CompilerObjectPayload::HirBody(object))
                .expect("HIR envelope builds")
                .encode()
                .expect("HIR envelope encodes")
        };
        assert_eq!(
            encode_hir(&first_parsed, &first_hir),
            encode_hir(&second_parsed, &second_hir)
        );
    }

    #[test]
    fn persistent_hir_body_facts_round_trip_without_hir_serialization() {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://compiler/persistent/game.arcw")
                    .expect("persistent fixture source ID"),
                SourceName::path("compiler/persistent/game.arcw"),
                SOURCE,
            )
            .expect("persistent fixture source document"),
        );
        let parsed = parse_attached_document(Arc::clone(&document));
        assert!(parsed.diagnostics().is_empty());
        let hir = lower_attached_hir(&parsed);
        let key = key(CompilerObjectKind::HirBody, &parsed);
        let object = hir_body_object(&HirBodyFactsInput {
            key: &key,
            module: "game",
            parsed: &parsed,
            hir: &hir,
        })
        .expect("HIR facts build");

        assert_eq!(object.facts.flow_count, 2);
        assert_eq!(object.facts.function_count, 1);
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
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://compiler/persistent/game.arcw")
                    .expect("persistent fixture source ID"),
                SourceName::path("compiler/persistent/game.arcw"),
                SOURCE,
            )
            .expect("persistent fixture source document"),
        );
        let parsed = parse_attached_document(Arc::clone(&document));
        assert!(parsed.diagnostics().is_empty());
        let hir = lower_attached_hir(&parsed);
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
    fn persistent_function_signature_digest_tracks_entity_family_arguments() {
        fn digest_for(source: &str) -> BuildDigest {
            let document = Arc::new(
                SourceDocument::try_new(
                    SourceDocumentId::try_new("arcweft-test://compiler/persistent/signature.arcw")
                        .expect("signature fixture source ID"),
                    SourceName::path("compiler/persistent/signature.arcw"),
                    source,
                )
                .expect("signature fixture source document"),
            );
            let parsed = parse_attached_document(Arc::clone(&document));
            assert!(parsed.diagnostics().is_empty());
            let hir = lower_attached_hir(&parsed);
            let function = hir
                .source_ordered_items()
                .iter()
                .find_map(|&item| match hir.resolve_item(item).ok()?.kind() {
                    HirItemKind::Function(function) => Some(function),
                    _ => None,
                })
                .expect("function is present");
            let name = function.name().resolved().expect("function name resolves");
            function_signature_digest(&hir, name.as_str(), function)
                .expect("signature digest builds")
        }

        let character = r"
pub fn retain(value: Ref<Character>) -> Ref<Character> {
    value
}
";
        let repeated_character = r"
pub fn retain(value: Ref<Character>) -> Ref<Character> {
    value
}
";
        let flow = r"
pub fn retain(value: Ref<Flow>) -> Ref<Flow> {
    value
}
";

        assert_eq!(digest_for(character), digest_for(repeated_character));
        assert_ne!(digest_for(character), digest_for(flow));
    }

    #[test]
    fn persistent_fact_builder_rejects_wrong_key_kind() {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://compiler/persistent/game.arcw")
                    .expect("persistent fixture source ID"),
                SourceName::path("compiler/persistent/game.arcw"),
                SOURCE,
            )
            .expect("persistent fixture source document"),
        );
        let parsed = parse_attached_document(Arc::clone(&document));
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
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://compiler/persistent/game.arcw")
                    .expect("persistent fixture source ID"),
                SourceName::path("compiler/persistent/game.arcw"),
                SOURCE,
            )
            .expect("persistent fixture source document"),
        );
        let parsed = parse_attached_document(Arc::clone(&document));
        assert!(parsed.diagnostics().is_empty());
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

        let rebuilt = parse_attached_document(Arc::clone(&document));
        assert!(rebuilt.diagnostics().is_empty());
        lower_attached_hir(&rebuilt);
    }

    #[test]
    fn persistent_query_actual_bytecode_builder_produces_verified_reusable_payload() {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://compiler/persistent/game.arcw")
                    .expect("persistent fixture source ID"),
                SourceName::path("compiler/persistent/game.arcw"),
                SOURCE,
            )
            .expect("persistent fixture source document"),
        );
        let parsed = parse_attached_document(Arc::clone(&document));
        assert!(parsed.diagnostics().is_empty());
        let hir = lower_attached_hir(&parsed);
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
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://compiler/persistent/game.arcw")
                    .expect("persistent fixture source ID"),
                SourceName::path("compiler/persistent/game.arcw"),
                SOURCE,
            )
            .expect("persistent fixture source document"),
        );
        let parsed = parse_attached_document(Arc::clone(&document));
        assert!(parsed.diagnostics().is_empty());
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
