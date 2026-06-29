//! Sans I/O LSP helpers for Arcweft verifier diagnostics.
//!
//! This crate does not open sockets or own a language-server transport. It
//! converts verifier reports into `lsp-types` values that a future server,
//! editor plugin, or tests can reuse.

use arcweft_adapter_context::manifest::{
    AdapterEffectCapability, AdapterFunctionSignature, AdapterHostCallId, AdapterManifest,
    AdapterTypeKind,
};
use arcweft_runtime_host::{
    RuntimeHostCapabilities, RuntimeHostConformanceDiagnosticKind, RuntimeHostConformanceReport,
    RuntimeHostRunnerKind,
};
use arcweft_rust_abi::{ArcweftRustTypeDecl, ArcweftRustTypeKind};
use arcweft_verify::{
    Severity as VerifySeverity, ToolAction, ToolActionKind, ToolActionSourceEdit,
    VerificationDiagnostic, VerificationReport,
};
use lsp_types::{
    CodeAction, CodeActionKind, CompletionItem, CompletionItemKind, Diagnostic, DiagnosticSeverity,
    Hover, HoverContents, InlayHint, InlayHintKind, MarkedString, NumberOrString,
    ParameterInformation, ParameterLabel, Position, Range, SignatureHelp, SignatureInformation,
    TextEdit, Uri, WorkspaceEdit,
};
use std::collections::HashMap;

/// Sans I/O LSP context supplied by the caller after resolving profiles.
pub struct ArcweftLspContext<'a> {
    adapter: &'a AdapterManifest,
    runtime_host: Option<RuntimeHostCapabilities>,
}

/// Builder used by transports after resolving a profile and runner.
pub struct ArcweftLspProfileContextBuilder<'a> {
    adapter: &'a AdapterManifest,
    runtime_host: Option<RuntimeHostCapabilities>,
}

/// Adapter-supplied fact required by a document, profile, or runtime plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterManifestRequirement {
    /// A runtime host call must be exported by the active adapter manifest.
    HostCall(AdapterHostCallId),
    /// An effect capability must be granted by the active adapter manifest.
    EffectCapability(AdapterEffectCapability),
}

/// Converts Arcweft byte spans into negotiated LSP ranges.
///
/// The implementation belongs to the transport layer because it needs the
/// current document text and negotiated position encoding.
pub trait LspPositionMapper {
    /// Converts a UTF-8 byte span in the current source document into an LSP range.
    fn range_from_byte_span(&self, start: usize, end: usize) -> Range;
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
    pub fn with_runtime_host(mut self, runtime_host: RuntimeHostCapabilities) -> Self {
        self.runtime_host = Some(runtime_host);
        self
    }

    /// Runtime-host call set supplied by the selected runner, when known.
    pub fn runtime_host(&self) -> Option<&RuntimeHostCapabilities> {
        self.runtime_host.as_ref()
    }
}

impl<'a> ArcweftLspProfileContextBuilder<'a> {
    /// Creates a profile-context builder from the resolved adapter manifest.
    pub const fn new(adapter: &'a AdapterManifest) -> Self {
        Self {
            adapter,
            runtime_host: None,
        }
    }

    /// Adds the selected runner's capabilities.
    #[must_use]
    pub fn with_runtime_host(mut self, runtime_host: RuntimeHostCapabilities) -> Self {
        self.runtime_host = Some(runtime_host);
        self
    }

    /// Selects one standard runner capability preset.
    #[must_use]
    pub fn with_runner_kind(mut self, runner: RuntimeHostRunnerKind) -> Self {
        self.runtime_host = Some(runner.capabilities());
        self
    }

    /// Adds one concrete adapter manifest implemented by the selected runner.
    #[must_use]
    pub fn with_implemented_adapter_manifest(mut self, manifest: &AdapterManifest) -> Self {
        let capabilities = self
            .runtime_host
            .take()
            .unwrap_or_default()
            .with_adapter_manifest(manifest);
        self.runtime_host = Some(capabilities);
        self
    }

    /// Adds concrete adapter manifests implemented by the selected runner.
    #[must_use]
    pub fn with_implemented_adapter_manifests<'b>(
        mut self,
        manifests: impl IntoIterator<Item = &'b AdapterManifest>,
    ) -> Self {
        let capabilities = self
            .runtime_host
            .take()
            .unwrap_or_default()
            .with_adapter_manifests(manifests);
        self.runtime_host = Some(capabilities);
        self
    }

    /// Builds the Sans I/O LSP context.
    pub fn build(self) -> ArcweftLspContext<'a> {
        ArcweftLspContext {
            adapter: self.adapter,
            runtime_host: self.runtime_host,
        }
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

/// Converts a verifier report into LSP diagnostics using source-aware positions.
pub fn diagnostics_from_report_with_mapper(
    report: &VerificationReport,
    mapper: &impl LspPositionMapper,
) -> Vec<Diagnostic> {
    report
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic_from_verify_with_mapper(diagnostic, mapper))
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

/// Diagnoses adapter manifests whose declared host calls are not implemented by
/// the selected runtime host.
pub fn profile_manifest_conformance_diagnostics(
    context: &ArcweftLspContext<'_>,
    manifests: &[AdapterManifest],
) -> Vec<Diagnostic> {
    context
        .runtime_host()
        .map_or_else(Vec::new, |runtime_host| {
            runtime_host_conformance_diagnostics(
                &runtime_host.check_adapter_manifests(manifests.iter()),
            )
        })
}

/// Diagnoses host calls required by source or profile metadata but missing from
/// the selected runtime host implementation.
pub fn runtime_host_requirement_diagnostics(
    runtime_host: &RuntimeHostCapabilities,
    requirements: &[AdapterManifestRequirement],
) -> Vec<Diagnostic> {
    requirements
        .iter()
        .filter_map(|requirement| runtime_host_requirement_diagnostic(runtime_host, requirement))
        .collect()
}

/// Converts runtime-host conformance reports into LSP diagnostics.
pub fn runtime_host_conformance_diagnostics(
    report: &RuntimeHostConformanceReport,
) -> Vec<Diagnostic> {
    report
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            let message = match diagnostic.kind() {
                RuntimeHostConformanceDiagnosticKind::MissingHostCallImplementation => format!(
                    "adapter manifest `{}` declares host call `{}` but the selected runtime host does not implement it",
                    diagnostic.adapter_id(),
                    diagnostic.host_call().as_str()
                ),
            };
            runtime_host_diagnostic("runtime_host.host_call.missing", message)
        })
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
pub fn runtime_host_completions(runtime_host: &RuntimeHostCapabilities) -> Vec<CompletionItem> {
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
pub fn runtime_host_hover(runtime_host: &RuntimeHostCapabilities, name: &str) -> Option<Hover> {
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
                "Rust export: {}\nPackage: {}",
                function.rust_path(),
                function.package()
            ))),
            ..CompletionItem::default()
        });
    let types = context
        .adapter()
        .rust_types()
        .iter()
        .map(|ty| CompletionItem {
            label: ty.decl().name.clone(),
            kind: Some(rust_type_completion_kind(ty.decl())),
            detail: Some(ty.decl().to_string()),
            documentation: Some(lsp_types::Documentation::String(format!(
                "Rust type: {}\nPackage: {}",
                ty.decl().rust_path,
                ty.package()
            ))),
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
                "{}\nRust: {}\nPackage: {}",
                signature_label(function.name(), function.signature()),
                function.rust_path(),
                function.package()
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
                "{}\nRust: {}\nPackage: {}",
                ty.decl(),
                ty.decl().rust_path,
                ty.package()
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
                        label: ParameterLabel::Simple(param.name().to_owned()),
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
                .map(|action| {
                    verifier_command_code_action(uri, diagnostic, action, diagnostic_from_verify)
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Converts verifier tool actions into LSP code actions using source-aware diagnostics.
pub fn code_actions_from_report_with_mapper(
    uri: &Uri,
    report: &VerificationReport,
    mapper: &impl LspPositionMapper,
) -> Vec<CodeAction> {
    report
        .diagnostics
        .iter()
        .flat_map(|diagnostic| {
            diagnostic.actions.iter().map(|action| {
                if let Some(source_edit) = action.source_edit() {
                    verifier_edit_code_action(uri, diagnostic, action, source_edit, mapper)
                } else {
                    verifier_command_code_action(uri, diagnostic, action, |diagnostic| {
                        diagnostic_from_verify_with_mapper(diagnostic, mapper)
                    })
                }
            })
        })
        .collect()
}

fn verifier_edit_code_action(
    uri: &Uri,
    diagnostic: &VerificationDiagnostic,
    action: &ToolAction,
    source_edit: &ToolActionSourceEdit,
    mapper: &impl LspPositionMapper,
) -> CodeAction {
    CodeAction {
        title: action.label.clone(),
        kind: Some(verifier_code_action_kind(action.kind)),
        diagnostics: Some(vec![diagnostic_from_verify_with_mapper(diagnostic, mapper)]),
        edit: Some(workspace_edit_from_tool_action_edit(
            uri,
            source_edit,
            mapper,
        )),
        command: None,
        ..CodeAction::default()
    }
}

fn verifier_command_code_action(
    uri: &Uri,
    diagnostic: &VerificationDiagnostic,
    action: &ToolAction,
    diagnostic_mapper: impl FnOnce(&VerificationDiagnostic) -> Diagnostic,
) -> CodeAction {
    let command = action.host_command();
    CodeAction {
        title: action.label.clone(),
        kind: Some(verifier_code_action_kind(action.kind)),
        diagnostics: Some(vec![diagnostic_mapper(diagnostic)]),
        command: Some(lsp_types::Command {
            title: command.title().to_owned(),
            command: command.id().to_owned(),
            arguments: Some(vec![
                serde_json::json!(uri.to_string()),
                serde_json::json!(diagnostic.obligation),
                serde_json::json!(action.id),
            ]),
        }),
        ..CodeAction::default()
    }
}

fn verifier_code_action_kind(kind: ToolActionKind) -> CodeActionKind {
    match kind {
        ToolActionKind::GenerateProofStub | ToolActionKind::GenerateUnsafeAudit => {
            CodeActionKind::QUICKFIX
        }
        ToolActionKind::ShowObligation
        | ToolActionKind::NavigateToProof
        | ToolActionKind::NavigateToUnsafeAudit => CodeActionKind::REFACTOR,
    }
}

/// Converts source-level Arcweft tooling actions into LSP code actions.
pub fn source_code_actions(uri: &Uri, source: &str) -> Vec<CodeAction> {
    arcweft_tooling::code_actions::source_code_actions(source)
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

/// Converts source-level Arcweft tooling actions into edit-bearing LSP code actions.
pub fn source_code_actions_with_mapper(
    uri: &Uri,
    source: &str,
    mapper: &impl LspPositionMapper,
) -> Vec<CodeAction> {
    arcweft_tooling::code_actions::source_code_actions(source)
        .into_iter()
        .map(|action| {
            let edit = action
                .edit
                .as_ref()
                .map(|edit| workspace_edit_from_tooling_edit(uri, edit, mapper));
            CodeAction {
                title: action.label,
                kind: Some(CodeActionKind::REFACTOR_REWRITE),
                edit,
                command: None,
                ..CodeAction::default()
            }
        })
        .collect()
}

/// Converts one Arcweft tooling edit into an LSP workspace edit.
pub fn workspace_edit_from_tooling_edit(
    uri: &Uri,
    edit: &arcweft_tooling::model::TextEdit,
    mapper: &impl LspPositionMapper,
) -> WorkspaceEdit {
    let text_edit = TextEdit::new(
        mapper.range_from_byte_span(edit.start, edit.end),
        edit.replacement.clone(),
    );
    WorkspaceEdit::new(HashMap::from([(uri.clone(), vec![text_edit])]))
}

/// Converts one verifier-owned source edit into an LSP workspace edit.
pub fn workspace_edit_from_tool_action_edit(
    uri: &Uri,
    edit: &ToolActionSourceEdit,
    mapper: &impl LspPositionMapper,
) -> WorkspaceEdit {
    let span = edit.span();
    let text_edit = TextEdit::new(
        mapper.range_from_byte_span(span.start, span.end),
        edit.replacement().to_owned(),
    );
    WorkspaceEdit::new(HashMap::from([(uri.clone(), vec![text_edit])]))
}

/// Converts inferred Arcweft IDs into LSP inlay hints.
pub fn inferred_id_inlay_hints_with_mapper(
    source: &str,
    mapper: &impl LspPositionMapper,
) -> Vec<InlayHint> {
    arcweft_tooling::id_context::inferred_id_hints(source)
        .into_iter()
        .map(|hint| InlayHint {
            position: mapper
                .range_from_byte_span(hint.position, hint.position)
                .start,
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

fn diagnostic_from_verify_with_mapper(
    diagnostic: &VerificationDiagnostic,
    mapper: &impl LspPositionMapper,
) -> Diagnostic {
    Diagnostic {
        range: diagnostic.source.map_or_else(default_range, |span| {
            mapper.range_from_byte_span(span.start, span.end)
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

fn diagnostic_from_verify(diagnostic: &VerificationDiagnostic) -> Diagnostic {
    Diagnostic {
        range: default_range(),
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
    runtime_host: &RuntimeHostCapabilities,
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

fn signature_label(name: &str, signature: &AdapterFunctionSignature) -> String {
    let params = signature
        .params()
        .iter()
        .map(|param| {
            let name = param.name();
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
    receiver: &AdapterTypeKind,
    name: &str,
    signature: &AdapterFunctionSignature,
) -> String {
    signature_label(&method_label(receiver, name), signature)
}

fn method_label(receiver: &AdapterTypeKind, name: &str) -> String {
    format!("{}.{}", type_kind_label(receiver), name)
}

fn rust_type_completion_kind(decl: &ArcweftRustTypeDecl) -> CompletionItemKind {
    match &decl.kind {
        ArcweftRustTypeKind::Enum { .. } => CompletionItemKind::ENUM,
        ArcweftRustTypeKind::Struct { .. } | ArcweftRustTypeKind::Newtype { .. } => {
            CompletionItemKind::STRUCT
        }
    }
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

fn type_kind_label(ty: &AdapterTypeKind) -> String {
    match ty {
        AdapterTypeKind::Bool => "Bool".to_owned(),
        AdapterTypeKind::I8 => "i8".to_owned(),
        AdapterTypeKind::I16 => "i16".to_owned(),
        AdapterTypeKind::I32 => "i32".to_owned(),
        AdapterTypeKind::I64 => "i64".to_owned(),
        AdapterTypeKind::I128 => "i128".to_owned(),
        AdapterTypeKind::ISize => "isize".to_owned(),
        AdapterTypeKind::U8 => "u8".to_owned(),
        AdapterTypeKind::U16 => "u16".to_owned(),
        AdapterTypeKind::U32 => "u32".to_owned(),
        AdapterTypeKind::U64 => "u64".to_owned(),
        AdapterTypeKind::U128 => "u128".to_owned(),
        AdapterTypeKind::USize => "usize".to_owned(),
        AdapterTypeKind::F32 => "f32".to_owned(),
        AdapterTypeKind::F64 => "f64".to_owned(),
        AdapterTypeKind::String => "String".to_owned(),
        AdapterTypeKind::Char => "Char".to_owned(),
        AdapterTypeKind::Unit => "()".to_owned(),
        AdapterTypeKind::Vec(item) => format!("Vec<{}>", type_kind_label(item)),
        AdapterTypeKind::Seq(item) => format!("Seq<{}>", type_kind_label(item)),
        AdapterTypeKind::Option(item) => format!("Option<{}>", type_kind_label(item)),
        AdapterTypeKind::Result { ok, error } => {
            format!(
                "Result<{}, {}>",
                type_kind_label(ok),
                type_kind_label(error)
            )
        }
        AdapterTypeKind::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(type_kind_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        AdapterTypeKind::Need { ready, error } => {
            format!(
                "Need<{}, {}>",
                type_kind_label(ready),
                type_kind_label(error)
            )
        }
        AdapterTypeKind::Named(name) => name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_adapter_context::manifest::AdapterFunctionParam;
    use arcweft_adapter_context::manifest::{AdapterHostCall, AdapterManifest, AdapterToolingDoc};
    use arcweft_rust_abi::{
        ArcweftRustField, ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage,
        ArcweftRustParam, ArcweftRustPurity, ArcweftRustTypeDecl, ArcweftRustTypeKind,
        ArcweftRustTypeRef, ArcweftRustVariant,
    };
    use arcweft_verify::{
        SourceSpan as VerifySourceSpan, ToolActionApplicability, ToolActionCommand,
        VerificationDiagnostic, VerificationPolicy, VerificationReport,
    };

    struct TestMapper;

    impl LspPositionMapper for TestMapper {
        fn range_from_byte_span(&self, start: usize, end: usize) -> Range {
            Range {
                start: Position::new(0, u32::try_from(start).expect("fixture offset fits")),
                end: Position::new(0, u32::try_from(end).expect("fixture offset fits")),
            }
        }
    }

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
    fn verifier_source_edit_action_becomes_workspace_edit() {
        let uri = "file:///game/routes/opening.arcw"
            .parse::<Uri>()
            .expect("uri");
        let report = VerificationReport {
            policy: VerificationPolicy::default(),
            diagnostics: vec![VerificationDiagnostic {
                id: "d1".to_owned(),
                severity: VerifySeverity::Warning,
                message: "missing proof".to_owned(),
                source: Some(VerifySourceSpan { start: 3, end: 8 }),
                obligation: Some("obligation.0001".to_owned()),
                related_ids: Vec::new(),
                actions: vec![ToolAction {
                    id: "action.generate_proof_stub".to_owned(),
                    label: "Generate proof stub".to_owned(),
                    kind: ToolActionKind::GenerateProofStub,
                    source_edit: Some(arcweft_verify::ToolActionSourceEdit {
                        span: VerifySourceSpan { start: 10, end: 15 },
                        replacement: "proof {}".to_owned(),
                        applicability: ToolActionApplicability::HasPlaceholders,
                    }),
                    command: None,
                }],
            }],
            ..VerificationReport::default()
        };

        let actions = code_actions_from_report_with_mapper(&uri, &report, &TestMapper);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].kind, Some(CodeActionKind::QUICKFIX));
        assert!(actions[0].command.is_none());
        let edit = actions[0].edit.as_ref().expect("workspace edit");
        let text_edit = &edit.changes.as_ref().expect("changes")[&uri][0];
        assert_eq!(text_edit.range.start, Position::new(0, 10));
        assert_eq!(text_edit.range.end, Position::new(0, 15));
        assert_eq!(text_edit.new_text, "proof {}");
    }

    #[test]
    fn verifier_host_action_becomes_command_action() {
        let uri = "file:///game/routes/opening.arcw"
            .parse::<Uri>()
            .expect("uri");
        let report = VerificationReport {
            policy: VerificationPolicy::default(),
            diagnostics: vec![VerificationDiagnostic {
                id: "d1".to_owned(),
                severity: VerifySeverity::Warning,
                message: "inspect obligation".to_owned(),
                source: None,
                obligation: Some("obligation.0001".to_owned()),
                related_ids: Vec::new(),
                actions: vec![ToolAction {
                    id: "action.show_obligation".to_owned(),
                    label: "Show proof obligation".to_owned(),
                    kind: ToolActionKind::ShowObligation,
                    source_edit: None,
                    command: Some(ToolActionCommand::new(
                        "arcweft.verify.showObligation",
                        "Show proof obligation",
                    )),
                }],
            }],
            ..VerificationReport::default()
        };

        let actions = code_actions_from_report(&uri, &report);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].kind, Some(CodeActionKind::REFACTOR));
        assert!(actions[0].edit.is_none());
        assert_eq!(
            actions[0].command.as_ref().expect("command").command,
            "arcweft.verify.showObligation"
        );
    }

    #[test]
    fn exposes_source_actions_and_inlay_hints() {
        let uri = "file:///game/routes/opening.arcw"
            .parse::<Uri>()
            .expect("uri");
        let source = "flow @.opening opening {\n    alice: [.shake amp=2px]hi[/][p]\n}\n";
        let actions = source_code_actions(&uri, source);
        assert!(
            actions
                .iter()
                .any(|action| action.title == "Expand Arcweft sugar")
        );
        assert!(actions.iter().any(|action| {
            action.title == "Canonicalize inferred rich-text tags"
                && action
                    .command
                    .as_ref()
                    .is_some_and(|command| command.command == "arcweft.canonicalRichText")
        }));
        assert!(
            actions
                .iter()
                .any(|action| action.title == "Materialize inferred Arcweft ID")
        );
        let mapped_actions = source_code_actions_with_mapper(&uri, source, &TestMapper);
        assert!(mapped_actions.iter().any(|action| {
            action.title == "Canonicalize inferred rich-text tags" && action.edit.is_some()
        }));
        let hints = inferred_id_inlay_hints_with_mapper(source, &TestMapper);
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
    fn exposes_complex_rust_adapter_type_shapes_from_metadata() {
        let manifest = complex_rust_manifest();
        let adapter = AdapterManifest::new("fixture", "Fixture").with_rust_manifest(&manifest);
        let context = ArcweftLspContext::new(&adapter);

        let completions = rust_adapter_completions(&context);
        let stats = completions
            .iter()
            .find(|item| item.label == "PlayerStats")
            .expect("PlayerStats completion");
        assert_eq!(stats.kind, Some(CompletionItemKind::STRUCT));
        assert!(stats.detail.as_deref().is_some_and(|detail| {
            detail.contains("score: i32")
                && detail.contains("tags: Vec<String>")
                && detail.contains("rank: Option<Rank>")
        }));
        let rank = completions
            .iter()
            .find(|item| item.label == "Rank")
            .expect("Rank completion");
        assert_eq!(rank.kind, Some(CompletionItemKind::ENUM));

        let stats_hover = rust_adapter_hover(&context, "PlayerStats").expect("stats hover");
        assert!(
            matches!(stats_hover.contents, HoverContents::Scalar(MarkedString::String(text)) if text.contains("struct PlayerStats") && text.contains("Package: quest_logic"))
        );
        let rank_hover = rust_adapter_hover(&context, "Rank").expect("rank hover");
        assert!(
            matches!(rank_hover.contents, HoverContents::Scalar(MarkedString::String(text)) if text.contains("enum Rank") && text.contains("Custom { label: String }"))
        );
        let session_hover = rust_adapter_hover(&context, "SessionId").expect("newtype hover");
        assert!(
            matches!(session_hover.contents, HoverContents::Scalar(MarkedString::String(text)) if text.contains("newtype SessionId(u64)"))
        );

        let signature =
            rust_adapter_signature_help(&context, "quest.evaluate").expect("signature help");
        assert_eq!(
            signature.signatures[0].label,
            "quest.evaluate(stats: PlayerStats, seed: Result<(u32, u32), String>) -> Rank"
        );
    }

    fn complex_rust_manifest() -> ArcweftRustManifest {
        ArcweftRustManifest::new(ArcweftRustPackage {
            name: "quest_logic".to_owned(),
            version: "0.1.0".to_owned(),
            metadata_hash: None,
        })
        .with_type(player_stats_type())
        .with_type(rank_type())
        .with_type(session_id_type())
        .with_function(evaluate_function())
    }

    fn player_stats_type() -> ArcweftRustTypeDecl {
        ArcweftRustTypeDecl {
            name: "PlayerStats".to_owned(),
            rust_path: "quest_logic::PlayerStats".to_owned(),
            kind: ArcweftRustTypeKind::Struct {
                fields: vec![
                    ArcweftRustField {
                        name: "score".to_owned(),
                        ty: ArcweftRustTypeRef::I32,
                    },
                    ArcweftRustField {
                        name: "tags".to_owned(),
                        ty: ArcweftRustTypeRef::Vec {
                            item: Box::new(ArcweftRustTypeRef::String),
                        },
                    },
                    ArcweftRustField {
                        name: "rank".to_owned(),
                        ty: ArcweftRustTypeRef::Option {
                            item: Box::new(ArcweftRustTypeRef::Named {
                                name: "Rank".to_owned(),
                            }),
                        },
                    },
                ],
            },
        }
    }

    fn rank_type() -> ArcweftRustTypeDecl {
        ArcweftRustTypeDecl {
            name: "Rank".to_owned(),
            rust_path: "quest_logic::Rank".to_owned(),
            kind: ArcweftRustTypeKind::Enum {
                variants: vec![
                    ArcweftRustVariant {
                        name: "Bronze".to_owned(),
                        fields: Vec::new(),
                    },
                    ArcweftRustVariant {
                        name: "Custom".to_owned(),
                        fields: vec![ArcweftRustField {
                            name: "label".to_owned(),
                            ty: ArcweftRustTypeRef::String,
                        }],
                    },
                ],
            },
        }
    }

    fn session_id_type() -> ArcweftRustTypeDecl {
        ArcweftRustTypeDecl {
            name: "SessionId".to_owned(),
            rust_path: "quest_logic::SessionId".to_owned(),
            kind: ArcweftRustTypeKind::Newtype {
                inner: ArcweftRustTypeRef::U64,
            },
        }
    }

    fn evaluate_function() -> ArcweftRustFunction {
        ArcweftRustFunction {
            name: "quest.evaluate".to_owned(),
            rust_path: "quest_logic::evaluate".to_owned(),
            params: vec![
                ArcweftRustParam {
                    name: "stats".to_owned(),
                    ty: ArcweftRustTypeRef::Named {
                        name: "PlayerStats".to_owned(),
                    },
                },
                ArcweftRustParam {
                    name: "seed".to_owned(),
                    ty: ArcweftRustTypeRef::Result {
                        ok: Box::new(ArcweftRustTypeRef::Tuple {
                            items: vec![ArcweftRustTypeRef::U32, ArcweftRustTypeRef::U32],
                        }),
                        error: Box::new(ArcweftRustTypeRef::String),
                    },
                },
            ],
            return_type: ArcweftRustTypeRef::Named {
                name: "Rank".to_owned(),
            },
            purity: ArcweftRustPurity::Pure,
            effects: Vec::new(),
        }
    }

    #[test]
    fn exposes_adapter_manifest_completions_and_hover() {
        let adapter = AdapterManifest::new("custom", "Custom")
            .with_symbol("custom", AdapterTypeKind::Named("CustomApi".to_owned()))
            .with_method_signature(
                AdapterTypeKind::Named("CustomApi".to_owned()),
                "read",
                AdapterFunctionSignature::new(
                    AdapterTypeKind::String,
                    [AdapterFunctionParam::required(
                        "path",
                        AdapterTypeKind::String,
                    )],
                ),
            )
            .with_function_signature(
                "custom.read",
                AdapterFunctionSignature::new(
                    AdapterTypeKind::String,
                    [AdapterFunctionParam::required(
                        "path",
                        AdapterTypeKind::String,
                    )],
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
        let runtime_host = RuntimeHostCapabilities::standard_native();
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
        let runtime_host = RuntimeHostCapabilities::standard_native();
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
    fn profile_context_builder_accepts_runtime_host_capabilities() {
        let adapter = AdapterManifest::new("sans-io", "Sans I/O");
        let context = ArcweftLspProfileContextBuilder::new(&adapter)
            .with_runner_kind(RuntimeHostRunnerKind::BrowserWeb)
            .build();

        assert_eq!(context.adapter().id().as_str(), "sans-io");
        assert!(context.runtime_host().is_some());
    }

    #[test]
    fn profile_context_wires_adapter_manifest_and_runtime_host_helpers() {
        let adapter = AdapterManifest::new("custom", "Custom")
            .with_effect(AdapterEffectCapability::new("custom.read"))
            .with_host_call(AdapterHostCall::new(
                "custom.read",
                [AdapterEffectCapability::new("custom.read")],
            ));
        let context = ArcweftLspProfileContextBuilder::new(&adapter)
            .with_runner_kind(RuntimeHostRunnerKind::Native)
            .build();

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

    #[test]
    fn profile_context_can_extend_runner_with_implemented_manifest() {
        let adapter = AdapterManifest::new("custom", "Custom")
            .with_host_call(AdapterHostCall::new("custom.read", []));
        let context = ArcweftLspProfileContextBuilder::new(&adapter)
            .with_runner_kind(RuntimeHostRunnerKind::BrowserWeb)
            .with_implemented_adapter_manifest(&adapter)
            .build();

        let diagnostics =
            profile_manifest_conformance_diagnostics(&context, std::slice::from_ref(&adapter));

        assert!(diagnostics.is_empty());
        assert!(
            profile_completions(&context)
                .iter()
                .any(|item| item.label == "custom.read")
        );
    }

    #[test]
    fn profile_manifest_conformance_uses_runtime_host_report() {
        let adapter = AdapterManifest::new("custom", "Custom")
            .with_host_call(AdapterHostCall::new("custom.read", []));
        let context = ArcweftLspProfileContextBuilder::new(&adapter)
            .with_runner_kind(RuntimeHostRunnerKind::BrowserWeb)
            .build();

        let diagnostics =
            profile_manifest_conformance_diagnostics(&context, std::slice::from_ref(&adapter));

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String(
                "runtime_host.host_call.missing".to_owned()
            ))
        );
        assert!(diagnostics[0].message.contains("custom.read"));
    }
}
