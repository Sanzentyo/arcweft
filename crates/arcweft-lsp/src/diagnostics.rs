use crate::documents::DocumentSnapshot;
use crate::positions::{LineIndex, PositionEncoding};
use crate::profiles::LspProfile;
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::{
    check::{analyze_types, validate_typecheck_ready},
    diagnostics::{TypeCheckError, TypeCheckReadinessError, TypeCheckWarning},
    resolve::{NameResolutionError, registry_from_hir, validate_hir_references},
};
use arcweft_lang_syntax::{
    lint::{SyntaxLint, lint_id_policy},
    parser::parse_source,
};
use arcweft_verify::{BackendKind, VerificationMode, VerificationPolicy, verify_module_with_env};
use arcweft_verify_lsp::{
    LspPositionMapper, diagnostics_from_report_with_mapper,
    profile_manifest_conformance_diagnostics,
};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, NumberOrString, Position, PublishDiagnosticsParams, Range,
};

/// Analyzed document diagnostics plus source index used by feature handlers.
#[derive(Clone, Debug)]
pub struct DocumentAnalysis {
    diagnostics: Vec<Diagnostic>,
    line_index: LineIndex,
}

impl DocumentAnalysis {
    /// Runs syntax, HIR lowering, profile-aware type checking, and verifier diagnostics.
    pub fn analyze(source: &str, encoding: PositionEncoding, profile: &LspProfile) -> Self {
        let line_index = LineIndex::new(source.to_owned(), encoding);
        let parsed = parse_source(source.to_owned());
        let mut diagnostics = parsed
            .errors()
            .iter()
            .enumerate()
            .map(|(index, error)| Diagnostic {
                range: line_index.range_from_byte_span(error.range().start(), error.range().end()),
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String(format!(
                    "syntax.parse.{}",
                    index + 1
                ))),
                source: Some("arcweft-syntax".to_owned()),
                message: error.message().to_owned(),
                ..Diagnostic::default()
            })
            .collect::<Vec<_>>();

        if parsed.errors().is_empty() {
            diagnostics.extend(syntax_lint_diagnostics(
                &lint_id_policy(parsed.typed_tree()),
                &line_index,
            ));
            match lower_to_hir(parsed.typed_tree()) {
                Ok(hir) => {
                    let env = profile.typecheck_env();
                    let resolve = resolve_diagnostics(&hir, &line_index);
                    if resolve.is_empty() {
                        let readiness = readiness_diagnostics(&hir, &line_index);
                        if readiness.is_empty() {
                            let typecheck_report = analyze_types(&hir, &env);
                            diagnostics
                                .extend(typecheck_diagnostics(&typecheck_report.diagnostics));
                            diagnostics.extend(typecheck_warnings(&typecheck_report.warnings));
                            if typecheck_report.diagnostics.is_empty() {
                                let report = verify_module_with_env(
                                    &hir,
                                    &env,
                                    VerificationPolicy {
                                        mode: VerificationMode::Dev,
                                        backend: BackendKind::Emit,
                                    },
                                );
                                diagnostics.extend(diagnostics_from_report_with_mapper(
                                    &report,
                                    &line_index,
                                ));
                            }
                        } else {
                            diagnostics.extend(readiness);
                        }
                    } else {
                        diagnostics.extend(resolve);
                    }
                }
                Err(errors) => {
                    diagnostics.extend(errors.into_iter().enumerate().map(|(index, error)| {
                        let range = error.range().map_or_else(
                            || line_index.range_from_byte_span(0, 0),
                            |range| line_index.range_from_byte_span(range.start(), range.end()),
                        );
                        Diagnostic {
                            range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: Some(NumberOrString::String(format!("hir.lower.{}", index + 1))),
                            source: Some("arcweft-hir".to_owned()),
                            message: error.message().to_owned(),
                            ..Diagnostic::default()
                        }
                    }));
                }
            }
        }

        Self {
            diagnostics,
            line_index,
        }
    }

    /// Diagnostics emitted for the analyzed document.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Line index used for source-aware LSP feature conversion.
    pub const fn line_index(&self) -> &LineIndex {
        &self.line_index
    }
}

fn syntax_lint_diagnostics(lints: &[SyntaxLint], line_index: &LineIndex) -> Vec<Diagnostic> {
    lints
        .iter()
        .map(|lint| Diagnostic {
            range: line_index.range_from_byte_span(lint.range().start(), lint.range().end()),
            severity: Some(match lint.severity() {
                arcweft_lang_syntax::lint::SyntaxLintSeverity::Error => DiagnosticSeverity::ERROR,
                arcweft_lang_syntax::lint::SyntaxLintSeverity::Warning => {
                    DiagnosticSeverity::WARNING
                }
                arcweft_lang_syntax::lint::SyntaxLintSeverity::Information => {
                    DiagnosticSeverity::INFORMATION
                }
                arcweft_lang_syntax::lint::SyntaxLintSeverity::Hint => DiagnosticSeverity::HINT,
            }),
            code: Some(NumberOrString::String(lint.code().stable_code().to_owned())),
            source: Some("arcweft-syntax".to_owned()),
            message: format!("{}: {}", lint.code().domain_name(), lint.message()),
            ..Diagnostic::default()
        })
        .collect()
}

/// Builds a publishDiagnostics notification payload for one open document.
pub fn publish_diagnostics(
    snapshot: &DocumentSnapshot,
    profile: &LspProfile,
) -> PublishDiagnosticsParams {
    let analysis = DocumentAnalysis::analyze(
        snapshot.text(),
        snapshot.line_index().position_encoding(),
        profile,
    );
    let mut diagnostics = analysis.diagnostics;
    diagnostics.extend(profile_diagnostics(profile));
    PublishDiagnosticsParams::new(snapshot.uri().clone(), diagnostics, snapshot.version())
}

fn resolve_diagnostics(
    hir: &arcweft_lang_hir::model::HirModule,
    line_index: &LineIndex,
) -> Vec<Diagnostic> {
    let registry = registry_from_hir(hir);
    validate_hir_references(hir, &registry).map_or_else(
        |errors| {
            errors
                .iter()
                .enumerate()
                .map(|(index, error)| name_resolution_diagnostic(error, index + 1, line_index))
                .collect()
        },
        |()| Vec::new(),
    )
}

fn readiness_diagnostics(
    hir: &arcweft_lang_hir::model::HirModule,
    line_index: &LineIndex,
) -> Vec<Diagnostic> {
    validate_typecheck_ready(hir).map_or_else(
        |errors| {
            errors
                .iter()
                .enumerate()
                .map(|(index, error)| readiness_diagnostic(error, index + 1, line_index))
                .collect()
        },
        |()| Vec::new(),
    )
}

fn name_resolution_diagnostic(
    error: &NameResolutionError,
    index: usize,
    line_index: &LineIndex,
) -> Diagnostic {
    Diagnostic {
        range: line_index.range_from_byte_span(0, 0),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(format!("sema.resolve.{index}"))),
        source: Some("arcweft-sema".to_owned()),
        message: error.message().to_owned(),
        ..Diagnostic::default()
    }
}

fn readiness_diagnostic(
    error: &TypeCheckReadinessError,
    index: usize,
    line_index: &LineIndex,
) -> Diagnostic {
    Diagnostic {
        range: line_index.range_from_byte_span(0, 0),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(format!("sema.readiness.{index}"))),
        source: Some("arcweft-sema".to_owned()),
        message: error.message().to_owned(),
        ..Diagnostic::default()
    }
}

fn typecheck_diagnostics(errors: &[TypeCheckError]) -> Vec<Diagnostic> {
    errors
        .iter()
        .enumerate()
        .map(|(index, error)| Diagnostic {
            range: start_range(),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(format!(
                "sema.typecheck.{}",
                index + 1
            ))),
            source: Some("arcweft-sema".to_owned()),
            message: error.message().to_owned(),
            ..Diagnostic::default()
        })
        .collect()
}

fn typecheck_warnings(warnings: &[TypeCheckWarning]) -> Vec<Diagnostic> {
    warnings
        .iter()
        .enumerate()
        .map(|(index, warning)| Diagnostic {
            range: start_range(),
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(format!(
                "sema.typecheck.warning.{}",
                index + 1
            ))),
            source: Some("arcweft-sema".to_owned()),
            message: warning.message().to_owned(),
            ..Diagnostic::default()
        })
        .collect()
}

fn profile_diagnostics(profile: &LspProfile) -> Vec<Diagnostic> {
    let mut diagnostics = profile
        .diagnostics()
        .iter()
        .map(|diagnostic| Diagnostic {
            range: start_range(),
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(diagnostic.kind().code().to_owned())),
            source: Some("arcweft-lsp-profile".to_owned()),
            message: diagnostic.message().to_owned(),
            ..Diagnostic::default()
        })
        .collect::<Vec<_>>();
    diagnostics.extend(profile_manifest_conformance_diagnostics(
        &profile.context(),
        profile.declared_manifests(),
    ));
    diagnostics
}

fn start_range() -> Range {
    Range::new(Position::new(0, 0), Position::new(0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_adapter_context::manifest::AdapterManifest;
    use arcweft_lang_sema::{
        env::{FunctionParam, FunctionSignature},
        types::TypeKind,
    };
    use arcweft_runtime_host::RuntimeHostRunnerKind;

    #[test]
    fn diagnostics_use_profile_selected_adapter_environment() {
        let source = r#"
flow @.main main {
    let value = custom_echo("hello")
}
"#;
        let default_profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let default_analysis =
            DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &default_profile);
        assert!(default_analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unknown function `custom_echo`")
        }));

        let adapter = AdapterManifest::new("custom", "Custom").with_function_signature(
            "custom_echo",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("value", TypeKind::String)],
            ),
            [],
        );
        let profile = LspProfile::new(adapter, RuntimeHostRunnerKind::Native);
        let profile_analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);

        assert!(!profile_analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unknown function `custom_echo`")
        }));
    }

    #[test]
    fn diagnostics_include_stable_syntax_lint_codes() {
        let source = r"
flow @flow.opening start {
}
";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);
        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == Some(NumberOrString::String("AWF0102".into())))
            .expect("identity mismatch diagnostic");

        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert!(
            diagnostic
                .message
                .contains("identity::decl_binding_mismatch")
        );
    }

    #[test]
    fn diagnostics_map_explicit_decl_id_to_hint() {
        let source = r"
flow @flow.opening {
}
";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);
        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == Some(NumberOrString::String("AWF0103".into())))
            .expect("explicit declaration id diagnostic");

        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::HINT));
        assert!(diagnostic.message.contains("style::explicit_decl_id"));
    }

    #[test]
    fn diagnostics_respect_allow_attribute_for_flow_module_mismatch() {
        let source = r"
mod route::opening

#[allow(id::flow_module_mismatch)]
flow @flow.prologue {
}
";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);

        assert!(!analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == Some(NumberOrString::String("AWF0002".into()))
        }));
    }

    #[test]
    fn diagnostics_respect_source_generated_attribute_for_decl_identity() {
        let source = r"
#![generated(tool)]
flow @flow.opening opening {
}

flow @flow.opening start {
}
";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);

        assert!(analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == Some(NumberOrString::String("AWF0104".into()))
                && diagnostic.severity == Some(DiagnosticSeverity::INFORMATION)
        }));
        assert!(!analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == Some(NumberOrString::String("AWF0101".into()))
        }));
        assert!(analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == Some(NumberOrString::String("AWF0102".into()))
                && diagnostic.severity == Some(DiagnosticSeverity::ERROR)
        }));
    }
}
