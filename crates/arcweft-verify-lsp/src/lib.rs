//! Sans I/O LSP helpers for Arcweft verifier diagnostics.
//!
//! This crate does not open sockets or own a language-server transport. It
//! converts verifier reports into `lsp-types` values that a future server,
//! editor plugin, or tests can reuse.

use arcweft_adapter_context::{
    manifest::{AdapterEffectCapability, AdapterHostCallId, AdapterManifest},
    standard,
};
use arcweft_lang_sema::env::FunctionSignature;
use arcweft_lang_sema::types::TypeKind;
use arcweft_runtime_host::internal_scheduler_manifest;
use arcweft_verify::{
    Severity as VerifySeverity, ToolActionKind, VerificationDiagnostic, VerificationReport,
};
use lsp_types::{
    CodeAction, CodeActionKind, CompletionItem, CompletionItemKind, Diagnostic, DiagnosticSeverity,
    Hover, HoverContents, InlayHint, InlayHintKind, MarkedString, NumberOrString,
    ParameterInformation, ParameterLabel, Position, Range, SignatureHelp, SignatureInformation,
    Uri,
};
use std::collections::BTreeSet;

/// Sans I/O LSP context supplied by the caller after resolving profiles.
pub struct ArcweftLspContext<'a> {
    adapter: &'a AdapterManifest,
    runtime_host: Option<&'a RuntimeHostCallSet>,
}

/// Runtime-host capabilities supplied by the embedding runner.
///
/// Adapter manifests describe the Arcweft-visible surface. This set describes
/// the concrete host calls that the selected native/web runner can actually
/// complete, so tooling can report a profile that type-checks but cannot run
/// with the chosen host.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeHostCallSet {
    host_calls: BTreeSet<AdapterHostCallId>,
}

/// Adapter-supplied fact required by a document, profile, or runtime plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterManifestRequirement {
    /// A runtime host call must be exported by the active adapter manifest.
    HostCall(AdapterHostCallId),
    /// An effect capability must be granted by the active adapter manifest.
    EffectCapability(AdapterEffectCapability),
}

impl<'a> ArcweftLspContext<'a> {
    /// Creates an LSP context from already-resolved adapter metadata.
    pub const fn new(adapter: &'a AdapterManifest) -> Self {
        Self {
            adapter,
            runtime_host: None,
        }
    }

    /// Adapter metadata visible to tooling.
    pub const fn adapter(&self) -> &'a AdapterManifest {
        self.adapter
    }

    /// Creates a context that also includes the selected runner's capabilities.
    #[must_use]
    pub const fn with_runtime_host(mut self, runtime_host: &'a RuntimeHostCallSet) -> Self {
        self.runtime_host = Some(runtime_host);
        self
    }

    /// Runtime-host call set supplied by the selected runner, when known.
    pub const fn runtime_host(&self) -> Option<&'a RuntimeHostCallSet> {
        self.runtime_host
    }
}

impl RuntimeHostCallSet {
    /// Creates an empty runtime-host call set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a runtime-host call set from stable host-call ids.
    pub fn from_host_call_ids(ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            host_calls: ids
                .into_iter()
                .map(AdapterHostCallId::new)
                .collect::<BTreeSet<_>>(),
        }
    }

    /// Creates a runtime-host call set from adapter manifests.
    pub fn from_adapter_manifests<'a>(
        manifests: impl IntoIterator<Item = &'a AdapterManifest>,
    ) -> Self {
        Self::from_host_call_ids(
            manifests
                .into_iter()
                .flat_map(AdapterManifest::host_calls)
                .map(|host_call| host_call.id().to_owned()),
        )
    }

    /// Native runtime-host calls provided by the standard embedding runner.
    pub fn standard_native() -> Self {
        let manifests = [
            standard::native_file_manifest(),
            standard::system_info_manifest(),
            internal_scheduler_manifest(),
        ];
        Self::from_adapter_manifests(&manifests)
    }

    /// Returns true when the runtime host implements this call id.
    pub fn has_host_call(&self, id: &AdapterHostCallId) -> bool {
        self.host_calls.contains(id)
    }

    /// Stable runtime-host call ids visible to tooling.
    pub fn host_call_ids(&self) -> impl Iterator<Item = &str> {
        self.host_calls.iter().map(AdapterHostCallId::as_str)
    }
}

impl AdapterManifestRequirement {
    /// Requires one runtime host-call id.
    pub fn host_call(id: impl Into<String>) -> Self {
        Self::HostCall(AdapterHostCallId::new(id))
    }

    /// Requires one effect capability.
    pub fn effect_capability(id: impl Into<String>) -> Self {
        Self::EffectCapability(AdapterEffectCapability::new(id))
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

/// Diagnoses adapter manifest requirements not provided by the active manifest.
pub fn adapter_manifest_requirement_diagnostics(
    context: &ArcweftLspContext<'_>,
    requirements: &[AdapterManifestRequirement],
) -> Vec<Diagnostic> {
    requirements
        .iter()
        .filter_map(|requirement| adapter_manifest_requirement_diagnostic(context, requirement))
        .collect()
}

/// Diagnoses adapter and runtime-host requirements for the active LSP context.
pub fn profile_requirement_diagnostics(
    context: &ArcweftLspContext<'_>,
    requirements: &[AdapterManifestRequirement],
) -> Vec<Diagnostic> {
    let mut diagnostics = adapter_manifest_requirement_diagnostics(context, requirements);
    if let Some(runtime_host) = context.runtime_host() {
        diagnostics.extend(runtime_host_requirement_diagnostics(
            runtime_host,
            requirements,
        ));
    }
    diagnostics
}

/// Diagnoses host calls required by source or profile metadata but missing from
/// the selected runtime host implementation.
pub fn runtime_host_requirement_diagnostics(
    runtime_host: &RuntimeHostCallSet,
    requirements: &[AdapterManifestRequirement],
) -> Vec<Diagnostic> {
    requirements
        .iter()
        .filter_map(|requirement| runtime_host_requirement_diagnostic(runtime_host, requirement))
        .collect()
}

/// Completes adapter manifest facts and selected runtime-host calls together.
pub fn profile_completions(context: &ArcweftLspContext<'_>) -> Vec<CompletionItem> {
    let mut completions = adapter_manifest_completions(context);
    if let Some(runtime_host) = context.runtime_host() {
        completions.extend(runtime_host_completions(runtime_host));
    }
    completions
}

/// Completes runtime-host calls provided by the selected embedding runner.
pub fn runtime_host_completions(runtime_host: &RuntimeHostCallSet) -> Vec<CompletionItem> {
    runtime_host
        .host_call_ids()
        .map(|id| CompletionItem {
            label: id.to_owned(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some("runtime host call".to_owned()),
            ..CompletionItem::default()
        })
        .collect()
}

/// Builds hover text for adapter facts, Rust exports, or selected runtime-host calls.
pub fn profile_hover(context: &ArcweftLspContext<'_>, name: &str) -> Option<Hover> {
    adapter_manifest_hover(context, name).or_else(|| {
        context
            .runtime_host()
            .and_then(|runtime_host| runtime_host_hover(runtime_host, name))
    })
}

/// Builds hover text for a runtime-host call supplied by the selected runner.
pub fn runtime_host_hover(runtime_host: &RuntimeHostCallSet, name: &str) -> Option<Hover> {
    runtime_host
        .host_call_ids()
        .any(|id| id == name)
        .then(|| string_hover(format!("runtime host call {name}")))
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

/// Completes all adapter manifest facts visible to Arcweft tooling.
pub fn adapter_manifest_completions(context: &ArcweftLspContext<'_>) -> Vec<CompletionItem> {
    let adapter = context.adapter();
    let docs = |subject: &str| tooling_doc(adapter, subject);
    let symbols = adapter.symbols().iter().map(|symbol| CompletionItem {
        label: symbol.name().to_owned(),
        kind: Some(CompletionItemKind::VARIABLE),
        detail: Some(type_kind_label(symbol.ty())),
        documentation: docs(symbol.name()),
        ..CompletionItem::default()
    });
    let methods = adapter.methods().iter().map(|method| {
        let label = method_label(method.receiver(), method.name());
        CompletionItem {
            label: label.clone(),
            kind: Some(CompletionItemKind::METHOD),
            detail: Some(method_signature_label(
                method.receiver(),
                method.name(),
                method.signature(),
            )),
            documentation: docs(&label),
            ..CompletionItem::default()
        }
    });
    let functions = adapter.functions().iter().map(|function| CompletionItem {
        label: function.name().to_owned(),
        kind: Some(CompletionItemKind::FUNCTION),
        detail: Some(signature_label(function.name(), function.signature())),
        documentation: docs(function.name()),
        ..CompletionItem::default()
    });
    let effects = adapter.effects().iter().map(|effect| CompletionItem {
        label: effect.as_str().to_owned(),
        kind: Some(CompletionItemKind::INTERFACE),
        detail: Some("effect capability".to_owned()),
        documentation: docs(effect.as_str()),
        ..CompletionItem::default()
    });
    let host_calls = adapter.host_calls().iter().map(|host_call| CompletionItem {
        label: host_call.id().to_owned(),
        kind: Some(CompletionItemKind::FUNCTION),
        detail: Some("host call".to_owned()),
        documentation: docs(host_call.id()),
        ..CompletionItem::default()
    });
    symbols
        .chain(methods)
        .chain(functions)
        .chain(effects)
        .chain(host_calls)
        .chain(rust_adapter_completions(context))
        .collect()
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

/// Builds hover text for one adapter manifest fact.
pub fn adapter_manifest_hover(context: &ArcweftLspContext<'_>, name: &str) -> Option<Hover> {
    let adapter = context.adapter();
    if let Some(symbol) = adapter
        .symbols()
        .iter()
        .find(|symbol| symbol.name() == name)
    {
        return Some(string_hover(format!(
            "{}: {}{}",
            symbol.name(),
            type_kind_label(symbol.ty()),
            tooling_doc_text(adapter, name)
        )));
    }
    if let Some(method) = adapter
        .methods()
        .iter()
        .find(|method| method_label(method.receiver(), method.name()) == name)
    {
        return Some(string_hover(format!(
            "{}{}",
            method_signature_label(method.receiver(), method.name(), method.signature()),
            tooling_doc_text(adapter, name)
        )));
    }
    if let Some(function) = adapter
        .functions()
        .iter()
        .find(|function| function.name() == name)
    {
        return Some(string_hover(format!(
            "{}{}",
            signature_label(function.name(), function.signature()),
            tooling_doc_text(adapter, name)
        )));
    }
    if adapter
        .effects()
        .iter()
        .any(|effect| effect.as_str() == name)
    {
        return Some(string_hover(format!(
            "effect capability {name}{}",
            tooling_doc_text(adapter, name)
        )));
    }
    if adapter
        .host_calls()
        .iter()
        .any(|host_call| host_call.id() == name)
    {
        return Some(string_hover(format!(
            "host call {name}{}",
            tooling_doc_text(adapter, name)
        )));
    }
    rust_adapter_hover(context, name)
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

fn adapter_manifest_requirement_diagnostic(
    context: &ArcweftLspContext<'_>,
    requirement: &AdapterManifestRequirement,
) -> Option<Diagnostic> {
    match requirement {
        AdapterManifestRequirement::HostCall(id) => {
            (!context.adapter().has_host_call(id)).then(|| {
                adapter_manifest_diagnostic(
                    "adapter.host_call.missing",
                    format!(
                        "adapter manifest `{}` does not provide host call `{}`",
                        context.adapter().id().as_str(),
                        id.as_str()
                    ),
                )
            })
        }
        AdapterManifestRequirement::EffectCapability(capability) => {
            (!context.adapter().has_effect(capability)).then(|| {
                adapter_manifest_diagnostic(
                    "adapter.effect_capability.missing",
                    format!(
                        "adapter manifest `{}` does not grant effect capability `{}`",
                        context.adapter().id().as_str(),
                        capability.as_str()
                    ),
                )
            })
        }
    }
}

fn runtime_host_requirement_diagnostic(
    runtime_host: &RuntimeHostCallSet,
    requirement: &AdapterManifestRequirement,
) -> Option<Diagnostic> {
    match requirement {
        AdapterManifestRequirement::HostCall(id) => (!runtime_host.has_host_call(id)).then(|| {
            runtime_host_diagnostic(
                "runtime_host.host_call.missing",
                format!(
                    "runtime host does not provide implementation for host call `{}`",
                    id.as_str()
                ),
            )
        }),
        AdapterManifestRequirement::EffectCapability(_) => None,
    }
}

fn adapter_manifest_diagnostic(code: impl Into<String>, message: String) -> Diagnostic {
    Diagnostic {
        range: default_range(),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(code.into())),
        source: Some("arcweft-adapter".to_owned()),
        message,
        ..Diagnostic::default()
    }
}

fn runtime_host_diagnostic(code: impl Into<String>, message: String) -> Diagnostic {
    Diagnostic {
        range: default_range(),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(code.into())),
        source: Some("arcweft-runtime-host".to_owned()),
        message,
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

fn method_signature_label(
    receiver: &TypeKind,
    name: &str,
    signature: &FunctionSignature,
) -> String {
    signature_label(&method_label(receiver, name), signature)
}

fn method_label(receiver: &TypeKind, name: &str) -> String {
    format!("{}.{}", type_kind_label(receiver), name)
}

fn string_hover(value: String) -> Hover {
    Hover {
        contents: HoverContents::Scalar(MarkedString::String(value)),
        range: None,
    }
}

fn tooling_doc(adapter: &AdapterManifest, subject: &str) -> Option<lsp_types::Documentation> {
    adapter
        .tooling_docs()
        .iter()
        .find(|doc| doc.subject() == subject)
        .map(|doc| lsp_types::Documentation::String(doc.docs().to_owned()))
}

fn tooling_doc_text(adapter: &AdapterManifest, subject: &str) -> String {
    tooling_doc(adapter, subject).map_or_else(String::new, |doc| match doc {
        lsp_types::Documentation::String(text) => format!("\n{text}"),
        lsp_types::Documentation::MarkupContent(markup) => format!("\n{}", markup.value),
    })
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
    use arcweft_adapter_context::manifest::{AdapterHostCall, AdapterManifest, AdapterToolingDoc};
    use arcweft_lang_sema::env::{FunctionParam, FunctionSignature};
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
        let adapter = AdapterManifest::new("fixture", "Fixture").with_rust_manifest(&manifest);
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

    #[test]
    fn exposes_adapter_manifest_completions_and_hover() {
        let adapter = AdapterManifest::new("custom", "Custom")
            .with_symbol("custom", TypeKind::Named("CustomApi".to_owned()))
            .with_method_signature(
                TypeKind::Named("CustomApi".to_owned()),
                "read",
                FunctionSignature::new(
                    TypeKind::String,
                    [FunctionParam::required("path", TypeKind::String)],
                ),
            )
            .with_function_signature(
                "custom.read",
                FunctionSignature::new(
                    TypeKind::String,
                    [FunctionParam::required("path", TypeKind::String)],
                ),
                [AdapterEffectCapability::new("custom.read")],
            )
            .with_effect(AdapterEffectCapability::new("custom.read"))
            .with_host_call(AdapterHostCall::new(
                "custom.read",
                [AdapterEffectCapability::new("custom.read")],
            ))
            .with_tooling_doc(AdapterToolingDoc::new(
                "custom.read",
                "Read custom content.",
            ));
        let context = ArcweftLspContext::new(&adapter);
        let completions = adapter_manifest_completions(&context);

        for label in ["custom", "CustomApi.read", "custom.read"] {
            assert!(
                completions.iter().any(|item| item.label == label),
                "missing completion {label}"
            );
        }
        let hover = adapter_manifest_hover(&context, "custom.read").expect("manifest hover");
        assert!(
            matches!(hover.contents, HoverContents::Scalar(MarkedString::String(text)) if text.contains("Read custom content."))
        );
    }

    #[test]
    fn diagnoses_missing_adapter_manifest_requirements() {
        let adapter = AdapterManifest::new("native-http", "Native HTTP")
            .with_effect(AdapterEffectCapability::new("http.respond"))
            .with_host_call(AdapterHostCall::new(
                "http.respond",
                [AdapterEffectCapability::new("http.respond")],
            ));
        let context = ArcweftLspContext::new(&adapter);

        let diagnostics = adapter_manifest_requirement_diagnostics(
            &context,
            &[
                AdapterManifestRequirement::host_call("http.respond"),
                AdapterManifestRequirement::host_call("fs.read_text"),
                AdapterManifestRequirement::effect_capability("http.respond"),
                AdapterManifestRequirement::effect_capability("fs.read"),
            ],
        );

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    "adapter.host_call.missing".to_owned(),
                ))
                && diagnostic.message.contains("fs.read_text")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    "adapter.effect_capability.missing".to_owned(),
                ))
                && diagnostic.message.contains("fs.read")
        }));
    }

    #[test]
    fn exposes_runtime_host_completions_and_hover() {
        let runtime_host = RuntimeHostCallSet::standard_native();
        let completions = runtime_host_completions(&runtime_host);

        for label in ["fs.read_text", "system.core_count", "flow_thread.run_child"] {
            assert!(
                completions.iter().any(|item| item.label == label),
                "missing runtime-host completion {label}"
            );
        }
        let hover =
            runtime_host_hover(&runtime_host, "system.core_count").expect("runtime-host hover");
        assert!(
            matches!(hover.contents, HoverContents::Scalar(MarkedString::String(text)) if text.contains("runtime host call system.core_count"))
        );
    }

    #[test]
    fn diagnoses_missing_runtime_host_call_implementation() {
        let runtime_host = RuntimeHostCallSet::standard_native();
        let diagnostics = runtime_host_requirement_diagnostics(
            &runtime_host,
            &[
                AdapterManifestRequirement::host_call("system.core_count"),
                AdapterManifestRequirement::host_call("custom.read"),
                AdapterManifestRequirement::effect_capability("custom.read"),
            ],
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String(
                "runtime_host.host_call.missing".to_owned()
            ))
        );
        assert_eq!(
            diagnostics[0].source,
            Some("arcweft-runtime-host".to_owned())
        );
        assert!(diagnostics[0].message.contains("custom.read"));
    }

    #[test]
    fn profile_context_wires_adapter_manifest_and_runtime_host_helpers() {
        let adapter = AdapterManifest::new("custom", "Custom")
            .with_effect(AdapterEffectCapability::new("custom.read"))
            .with_host_call(AdapterHostCall::new(
                "custom.read",
                [AdapterEffectCapability::new("custom.read")],
            ));
        let runtime_host = RuntimeHostCallSet::standard_native();
        let context = ArcweftLspContext::new(&adapter).with_runtime_host(&runtime_host);

        let diagnostics = profile_requirement_diagnostics(
            &context,
            &[
                AdapterManifestRequirement::host_call("custom.read"),
                AdapterManifestRequirement::effect_capability("custom.read"),
            ],
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String(
                "runtime_host.host_call.missing".to_owned()
            ))
        );
        assert!(diagnostics[0].message.contains("custom.read"));

        let completions = profile_completions(&context);
        assert!(completions.iter().any(|item| item.label == "custom.read"));
        assert!(
            completions
                .iter()
                .any(|item| item.label == "system.core_count")
        );

        assert!(profile_hover(&context, "custom.read").is_some());
        let host_hover = profile_hover(&context, "system.core_count").expect("runtime host hover");
        assert!(
            matches!(host_hover.contents, HoverContents::Scalar(MarkedString::String(text)) if text.contains("runtime host call system.core_count"))
        );
    }
}
