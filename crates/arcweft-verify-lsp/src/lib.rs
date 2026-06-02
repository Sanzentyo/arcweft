//! Sans I/O LSP helpers for Arcweft verifier diagnostics.
//!
//! This crate does not open sockets or own a language-server transport. It
//! converts verifier reports into `lsp-types` values that a future server,
//! editor plugin, or tests can reuse.

use arcweft_adapter_context::AdapterTypecheckContext;
use arcweft_lang_sema::env::FunctionSignature;
use arcweft_lang_sema::types::TypeKind;
use arcweft_verify::{
    Severity as VerifySeverity, ToolActionKind, VerificationDiagnostic, VerificationReport,
};
use lsp_types::{
    CodeAction, CodeActionKind, CompletionItem, CompletionItemKind, Diagnostic, DiagnosticSeverity,
    Hover, HoverContents, InlayHint, InlayHintKind, MarkedString, NumberOrString,
    ParameterInformation, ParameterLabel, Position, Range, SignatureHelp, SignatureInformation,
    Uri,
};

/// Sans I/O LSP context supplied by the caller after resolving profiles.
pub struct ArcweftLspContext<'a> {
    adapter: &'a AdapterTypecheckContext,
}

impl<'a> ArcweftLspContext<'a> {
    /// Creates an LSP context from already-resolved adapter metadata.
    pub const fn new(adapter: &'a AdapterTypecheckContext) -> Self {
        Self { adapter }
    }

    /// Adapter metadata visible to tooling.
    pub const fn adapter(&self) -> &'a AdapterTypecheckContext {
        self.adapter
    }
}

/// Converts a verifier report into LSP diagnostics for a document.
pub fn diagnostics_from_report(report: &VerificationReport) -> Vec<Diagnostic> {
    report
        .diagnostics
        .iter()
        .map(diagnostic_from_verify)
        .collect()
}

/// Completes Rust adapter functions and exported Rust types visible to Arcweft.
pub fn rust_adapter_completions(context: &ArcweftLspContext<'_>) -> Vec<CompletionItem> {
    let functions = context
        .adapter()
        .rust_functions()
        .iter()
        .map(|function| CompletionItem {
            label: function.name().to_owned(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(signature_label(function.name(), function.signature())),
            documentation: Some(lsp_types::Documentation::String(format!(
                "Rust export: {}",
                function.rust_path()
            ))),
            ..CompletionItem::default()
        });
    let types = context
        .adapter()
        .rust_types()
        .iter()
        .map(|ty| CompletionItem {
            label: ty.decl().name.clone(),
            kind: Some(CompletionItemKind::STRUCT),
            detail: Some(format!("Rust type {}", ty.decl().rust_path)),
            ..CompletionItem::default()
        });
    functions.chain(types).collect()
}

/// Builds hover text for one Rust adapter function or type name.
pub fn rust_adapter_hover(context: &ArcweftLspContext<'_>, name: &str) -> Option<Hover> {
    if let Some(function) = context
        .adapter()
        .rust_functions()
        .iter()
        .find(|function| function.name() == name)
    {
        return Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(format!(
                "{}\nRust: {}",
                signature_label(function.name(), function.signature()),
                function.rust_path()
            ))),
            range: None,
        });
    }
    context
        .adapter()
        .rust_types()
        .iter()
        .find(|ty| ty.decl().name == name)
        .map(|ty| Hover {
            contents: HoverContents::Scalar(MarkedString::String(format!(
                "type {}\nRust: {}",
                ty.decl().name,
                ty.decl().rust_path
            ))),
            range: None,
        })
}

/// Builds signature help for one Rust adapter function name.
pub fn rust_adapter_signature_help(
    context: &ArcweftLspContext<'_>,
    name: &str,
) -> Option<SignatureHelp> {
    let function = context
        .adapter()
        .rust_functions()
        .iter()
        .find(|function| function.name() == name)?;
    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label: signature_label(function.name(), function.signature()),
            documentation: Some(lsp_types::Documentation::String(format!(
                "Rust export: {}",
                function.rust_path()
            ))),
            parameters: Some(
                function
                    .signature()
                    .params()
                    .iter()
                    .map(|param| ParameterInformation {
                        label: ParameterLabel::Simple(param.name().unwrap_or("_").to_owned()),
                        documentation: Some(lsp_types::Documentation::String(type_kind_label(
                            param.ty(),
                        ))),
                    })
                    .collect(),
            ),
            active_parameter: None,
        }],
        active_signature: Some(0),
        active_parameter: Some(0),
    })
}

/// Converts verifier tool actions into LSP code actions.
pub fn code_actions_from_report(uri: &Uri, report: &VerificationReport) -> Vec<CodeAction> {
    report
        .diagnostics
        .iter()
        .flat_map(|diagnostic| {
            diagnostic
                .actions
                .iter()
                .map(|action| CodeAction {
                    title: action.label.clone(),
                    kind: Some(match action.kind {
                        ToolActionKind::GenerateProofStub | ToolActionKind::GenerateUnsafeAudit => {
                            CodeActionKind::QUICKFIX
                        }
                        ToolActionKind::ShowObligation
                        | ToolActionKind::NavigateToProof
                        | ToolActionKind::NavigateToUnsafeAudit => CodeActionKind::REFACTOR,
                    }),
                    diagnostics: Some(vec![diagnostic_from_verify(diagnostic)]),
                    command: Some(lsp_types::Command {
                        title: action.label.clone(),
                        command: format!("arcweft.{}", action.id),
                        arguments: Some(vec![
                            serde_json::json!(uri.to_string()),
                            serde_json::json!(diagnostic.obligation),
                        ]),
                    }),
                    ..CodeAction::default()
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Converts source-level Arcweft tooling actions into LSP code actions.
pub fn source_code_actions(uri: &Uri, source: &str) -> Vec<CodeAction> {
    arcweft_tooling::source_code_actions(source)
        .into_iter()
        .map(|action| CodeAction {
            title: action.label,
            kind: Some(CodeActionKind::REFACTOR_REWRITE),
            command: Some(lsp_types::Command {
                title: action.id.clone(),
                command: action.id,
                arguments: Some(vec![
                    serde_json::json!(uri.to_string()),
                    serde_json::json!(action.edit),
                ]),
            }),
            ..CodeAction::default()
        })
        .collect()
}

/// Converts inferred Arcweft IDs into LSP inlay hints.
pub fn inferred_id_inlay_hints(source: &str) -> Vec<InlayHint> {
    arcweft_tooling::inferred_id_hints(source)
        .into_iter()
        .map(|hint| InlayHint {
            position: offset_position(hint.position),
            label: lsp_types::InlayHintLabel::String(hint.label),
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            tooltip: None,
            padding_left: None,
            padding_right: None,
            data: None,
        })
        .collect()
}

fn diagnostic_from_verify(diagnostic: &VerificationDiagnostic) -> Diagnostic {
    Diagnostic {
        range: diagnostic.source.map_or_else(default_range, |span| Range {
            start: offset_position(span.start),
            end: offset_position(span.end),
        }),
        severity: Some(match diagnostic.severity {
            VerifySeverity::Info => DiagnosticSeverity::INFORMATION,
            VerifySeverity::Warning => DiagnosticSeverity::WARNING,
            VerifySeverity::Error => DiagnosticSeverity::ERROR,
        }),
        code: diagnostic.obligation.clone().map(NumberOrString::String),
        source: Some("arcweft-verify".to_owned()),
        message: diagnostic.message.clone(),
        ..Diagnostic::default()
    }
}

fn default_range() -> Range {
    Range {
        start: Position::new(0, 0),
        end: Position::new(0, 0),
    }
}

fn offset_position(offset: usize) -> Position {
    let character = u32::try_from(offset).unwrap_or(u32::MAX);
    Position::new(0, character)
}

fn signature_label(name: &str, signature: &FunctionSignature) -> String {
    let params = signature
        .params()
        .iter()
        .map(|param| {
            let name = param.name().unwrap_or("_");
            format!("{name}: {}", type_kind_label(param.ty()))
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{name}({params}) -> {}",
        type_kind_label(signature.return_type())
    )
}

fn type_kind_label(ty: &TypeKind) -> String {
    match ty {
        TypeKind::Bool => "Bool".to_owned(),
        TypeKind::I8 => "i8".to_owned(),
        TypeKind::I16 => "i16".to_owned(),
        TypeKind::I32 => "i32".to_owned(),
        TypeKind::I64 => "i64".to_owned(),
        TypeKind::I128 => "i128".to_owned(),
        TypeKind::ISize => "isize".to_owned(),
        TypeKind::U8 => "u8".to_owned(),
        TypeKind::U16 => "u16".to_owned(),
        TypeKind::U32 => "u32".to_owned(),
        TypeKind::U64 => "u64".to_owned(),
        TypeKind::U128 => "u128".to_owned(),
        TypeKind::USize => "usize".to_owned(),
        TypeKind::F32 => "f32".to_owned(),
        TypeKind::F64 => "f64".to_owned(),
        TypeKind::String => "String".to_owned(),
        TypeKind::Char => "Char".to_owned(),
        TypeKind::Unit => "()".to_owned(),
        TypeKind::Never => "Never".to_owned(),
        TypeKind::Vec(item) => format!("Vec<{}>", type_kind_label(item)),
        TypeKind::Seq(item) => format!("Seq<{}>", type_kind_label(item)),
        TypeKind::Option(item) => format!("Option<{}>", type_kind_label(item)),
        TypeKind::Result { ok, error } => {
            format!(
                "Result<{}, {}>",
                type_kind_label(ok),
                type_kind_label(error)
            )
        }
        TypeKind::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(type_kind_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeKind::Named(name) => name.clone(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_adapter_context::AdapterTypecheckContext;
    use arcweft_rust_abi::{
        ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage, ArcweftRustParam,
        ArcweftRustPurity, ArcweftRustTypeDecl, ArcweftRustTypeKind, ArcweftRustTypeRef,
    };
    use arcweft_verify::{VerificationDiagnostic, VerificationPolicy, VerificationReport};

    #[test]
    fn converts_report_diagnostic() {
        let report = VerificationReport {
            policy: VerificationPolicy::default(),
            diagnostics: vec![VerificationDiagnostic {
                id: "d1".to_owned(),
                severity: VerifySeverity::Error,
                message: "missing proof".to_owned(),
                source: None,
                obligation: Some("obligation.0001".to_owned()),
                related_ids: Vec::new(),
                actions: Vec::new(),
            }],
            ..VerificationReport::default()
        };
        let diagnostics = diagnostics_from_report(&report);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn exposes_source_actions_and_inlay_hints() {
        let uri = "file:///game/routes/opening.arcw"
            .parse::<Uri>()
            .expect("uri");
        let source = "flow @.opening opening {\n    alice: hi[p]\n}\n";
        let actions = source_code_actions(&uri, source);
        assert!(
            actions
                .iter()
                .any(|action| action.title == "Expand Arcweft sugar")
        );
        assert!(
            actions
                .iter()
                .any(|action| action.title == "Materialize inferred Arcweft ID")
        );
        let hints = inferred_id_inlay_hints(source);
        assert!(hints.iter().any(|hint| {
            matches!(&hint.label, lsp_types::InlayHintLabel::String(label) if label == "@flow.opening")
        }));
        assert!(hints.iter().any(|hint| {
            matches!(&hint.label, lsp_types::InlayHintLabel::String(label) if label.contains("id=@say.opening.alice.001"))
        }));
    }

    #[test]
    fn exposes_rust_adapter_completion_hover_and_signature_help() {
        let manifest = ArcweftRustManifest::new(ArcweftRustPackage {
            name: "truck_game".to_owned(),
            version: "0.1.0".to_owned(),
            metadata_hash: None,
        })
        .with_type(ArcweftRustTypeDecl {
            name: "Rank".to_owned(),
            rust_path: "truck_game::Rank".to_owned(),
            kind: ArcweftRustTypeKind::Enum {
                variants: Vec::new(),
            },
        })
        .with_function(ArcweftRustFunction {
            name: "mini_games.truck.score_to_rank".to_owned(),
            rust_path: "truck_game::score_to_rank".to_owned(),
            params: vec![ArcweftRustParam {
                name: "score".to_owned(),
                ty: ArcweftRustTypeRef::I32,
            }],
            return_type: ArcweftRustTypeRef::Named {
                name: "Rank".to_owned(),
            },
            purity: ArcweftRustPurity::Pure,
            effects: Vec::new(),
        });
        let adapter = AdapterTypecheckContext::new().with_rust_manifest(&manifest);
        let context = ArcweftLspContext::new(&adapter);

        let completions = rust_adapter_completions(&context);
        assert!(
            completions
                .iter()
                .any(|item| item.label == "mini_games.truck.score_to_rank")
        );
        assert!(completions.iter().any(|item| item.label == "Rank"));
        let hover = rust_adapter_hover(&context, "mini_games.truck.score_to_rank")
            .expect("hover is available");
        assert!(
            matches!(hover.contents, HoverContents::Scalar(MarkedString::String(text)) if text.contains("score: i32"))
        );
        let signature = rust_adapter_signature_help(&context, "mini_games.truck.score_to_rank")
            .expect("signature help is available");
        assert_eq!(
            signature.signatures[0].label,
            "mini_games.truck.score_to_rank(score: i32) -> Rank"
        );
    }
}
