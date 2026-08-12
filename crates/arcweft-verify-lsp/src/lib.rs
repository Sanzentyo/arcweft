//! Sans I/O LSP helpers for Arcweft verifier diagnostics.
//!
//! This crate does not open sockets or own a language-server transport. It
//! converts verifier reports into `lsp-types` values that a future server,
//! editor plugin, or tests can reuse.

use arcweft_adapter_context::manifest::{
    AdapterCallablePath, AdapterEffectCapability, AdapterFreeCallableKind,
    AdapterFunctionSignature, AdapterHostCallId, AdapterManifest, AdapterToolingSubject,
    AdapterTypeKind,
};
use arcweft_runtime_host::{
    RuntimeHostCapabilities, RuntimeHostConformanceDiagnosticKind, RuntimeHostConformanceReport,
    RuntimeHostRunnerKind,
};
use arcweft_rust_abi::{ArcweftRustTypeDecl, ArcweftRustTypeKind};
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceRange, SourceRevision, SourceSpan, SourceSpanError,
};
use arcweft_verify::{
    Severity as VerifySeverity, SourceSpan as VerifySourceSpan, ToolAction, ToolActionKind,
    ToolActionSourceEdit, VerificationDiagnostic, VerificationReport,
};
use lsp_types::{
    CodeAction, CodeActionKind, CompletionItem, CompletionItemKind, Diagnostic, DiagnosticSeverity,
    Hover, HoverContents, MarkedString, NumberOrString, Position, Range, TextEdit, Uri,
    WorkspaceEdit,
};
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;

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

/// One source edit permanently bound to the exact document revision that
/// produced it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionBoundTextEdit {
    span: SourceSpan,
    replacement: String,
}

impl RevisionBoundTextEdit {
    /// Binds a tooling byte range to an immutable source document.
    pub fn try_from_tooling(
        document: &SourceDocument,
        edit: &arcweft_tooling::model::TextEdit,
    ) -> Result<Self, SourceSpanError> {
        Ok(Self {
            span: document.span(SourceRange::new(edit.start, edit.end))?,
            replacement: edit.replacement.clone(),
        })
    }

    /// Exact revision-bound source span for this edit.
    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }

    /// Replacement text supplied by Arcweft tooling.
    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

/// A revision-bound edit does not belong to the document currently being
/// published.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RevisionBoundWorkspaceEditError {
    #[error("workspace edit belongs to `{actual}`, not current document `{expected}`")]
    WrongDocument {
        expected: SourceDocumentId,
        actual: SourceDocumentId,
    },
    #[error("workspace edit belongs to a stale source revision")]
    WrongRevision {
        expected: SourceRevision,
        actual: SourceRevision,
    },
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
pub fn diagnostics_from_report(
    report: &VerificationReport,
    document: &SourceDocument,
) -> Vec<Diagnostic> {
    report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_belongs_to_document(diagnostic, document))
        .map(diagnostic_from_verify)
        .collect()
}

/// Converts a verifier report into LSP diagnostics using source-aware positions.
pub fn diagnostics_from_report_with_mapper(
    report: &VerificationReport,
    document: &SourceDocument,
    mapper: &impl LspPositionMapper,
) -> Vec<Diagnostic> {
    report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_belongs_to_document(diagnostic, document))
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
    let functions = context.adapter().rust_functions().iter().map(|function| {
        let label = callable_path_label(function.path());
        CompletionItem {
            label: label.clone(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(signature_label(&label, function.signature())),
            documentation: Some(lsp_types::Documentation::String(format!(
                "Rust export: {}\nPackage: {}",
                function.rust_path(),
                function.package().id
            ))),
            ..CompletionItem::default()
        }
    });
    let types = context
        .adapter()
        .rust_types()
        .iter()
        .map(|ty| CompletionItem {
            label: ty.accepted_path().to_string(),
            kind: Some(rust_type_completion_kind(ty.decl())),
            detail: Some(ty.decl().to_string()),
            documentation: Some(lsp_types::Documentation::String(format!(
                "Rust type: {}\nPackage: {}",
                ty.decl().rust_path,
                ty.package().id
            ))),
            ..CompletionItem::default()
        });
    functions.chain(types).collect()
}

/// Completes all adapter manifest facts visible to Arcweft tooling.
pub fn adapter_manifest_completions(context: &ArcweftLspContext<'_>) -> Vec<CompletionItem> {
    let adapter = context.adapter();
    let symbols = adapter.symbols().iter().map(|symbol| CompletionItem {
        label: symbol.path().to_string(),
        kind: Some(CompletionItemKind::VARIABLE),
        detail: Some(type_kind_label(symbol.ty())),
        documentation: None,
        ..CompletionItem::default()
    });
    let methods = adapter.methods().iter().map(|method| {
        let label = method_label(method.receiver(), method.name());
        let subject = AdapterToolingSubject::Method {
            receiver: method.receiver().clone(),
            name: method.callable_name().clone(),
            overload: method.overload(),
        };
        CompletionItem {
            label: label.clone(),
            kind: Some(CompletionItemKind::METHOD),
            detail: Some(method_signature_label(
                method.receiver(),
                method.name(),
                method.signature(),
            )),
            documentation: tooling_doc(adapter, &subject),
            ..CompletionItem::default()
        }
    });
    let functions = adapter.functions().iter().map(|function| {
        let label = callable_path_label(function.path());
        let subject = AdapterToolingSubject::Free {
            kind: AdapterFreeCallableKind::Function,
            path: function.path().clone(),
            overload: function.overload(),
        };
        CompletionItem {
            label: label.clone(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(signature_label(&label, function.signature())),
            documentation: tooling_doc(adapter, &subject),
            ..CompletionItem::default()
        }
    });
    let effects = adapter.effects().iter().map(|effect| CompletionItem {
        label: effect.as_str().to_owned(),
        kind: Some(CompletionItemKind::INTERFACE),
        detail: Some("effect capability".to_owned()),
        documentation: None,
        ..CompletionItem::default()
    });
    let host_calls = adapter.host_calls().iter().map(|host_call| CompletionItem {
        label: host_call.id().to_owned(),
        kind: Some(CompletionItemKind::FUNCTION),
        detail: Some("host call".to_owned()),
        documentation: None,
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
        .find(|function| callable_path_label(function.path()) == name)
    {
        let label = callable_path_label(function.path());
        return Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(format!(
                "{}\nRust: {}\nPackage: {}",
                signature_label(&label, function.signature()),
                function.rust_path(),
                function.package().id
            ))),
            range: None,
        });
    }
    context
        .adapter()
        .rust_types()
        .iter()
        .find(|ty| ty.accepted_path().to_string() == name)
        .map(|ty| Hover {
            contents: HoverContents::Scalar(MarkedString::String(format!(
                "{}\nRust: {}\nPackage: {}",
                ty.decl(),
                ty.decl().rust_path,
                ty.package().id
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
        .find(|symbol| symbol.path().to_string() == name)
    {
        return Some(string_hover(format!(
            "{}: {}",
            symbol.path(),
            type_kind_label(symbol.ty())
        )));
    }
    if let Some(method) = adapter
        .methods()
        .iter()
        .find(|method| method_label(method.receiver(), method.name()) == name)
    {
        let subject = AdapterToolingSubject::Method {
            receiver: method.receiver().clone(),
            name: method.callable_name().clone(),
            overload: method.overload(),
        };
        return Some(string_hover(format!(
            "{}{}",
            method_signature_label(method.receiver(), method.name(), method.signature()),
            tooling_doc_text(adapter, &subject)
        )));
    }
    if let Some(function) = adapter
        .functions()
        .iter()
        .find(|function| callable_path_label(function.path()) == name)
    {
        let subject = AdapterToolingSubject::Free {
            kind: AdapterFreeCallableKind::Function,
            path: function.path().clone(),
            overload: function.overload(),
        };
        return Some(string_hover(format!(
            "{}{}",
            signature_label(name, function.signature()),
            tooling_doc_text(adapter, &subject)
        )));
    }
    if adapter
        .effects()
        .iter()
        .any(|effect| effect.as_str() == name)
    {
        return Some(string_hover(format!("effect capability {name}")));
    }
    if adapter
        .host_calls()
        .iter()
        .any(|host_call| host_call.id() == name)
    {
        return Some(string_hover(format!("host call {name}")));
    }
    rust_adapter_hover(context, name)
}

/// Converts verifier tool actions into LSP code actions.
pub fn code_actions_from_report(
    uri: &Uri,
    document: &SourceDocument,
    report: &VerificationReport,
) -> Vec<CodeAction> {
    report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_belongs_to_document(diagnostic, document))
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
    document: &SourceDocument,
    report: &VerificationReport,
    mapper: &impl LspPositionMapper,
) -> Vec<CodeAction> {
    report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_belongs_to_document(diagnostic, document))
        .flat_map(|diagnostic| {
            diagnostic.actions.iter().filter_map(|action| {
                if let Some(source_edit) = action.source_edit() {
                    verifier_edit_code_action(
                        uri,
                        document,
                        diagnostic,
                        action,
                        source_edit,
                        mapper,
                    )
                    .ok()
                } else {
                    Some(verifier_command_code_action(
                        uri,
                        diagnostic,
                        action,
                        |diagnostic| diagnostic_from_verify_with_mapper(diagnostic, mapper),
                    ))
                }
            })
        })
        .collect()
}

fn verifier_edit_code_action(
    uri: &Uri,
    document: &SourceDocument,
    diagnostic: &VerificationDiagnostic,
    action: &ToolAction,
    source_edit: &ToolActionSourceEdit,
    mapper: &impl LspPositionMapper,
) -> Result<CodeAction, RevisionBoundWorkspaceEditError> {
    Ok(CodeAction {
        title: action.label.clone(),
        kind: Some(verifier_code_action_kind(action.kind)),
        diagnostics: Some(vec![diagnostic_from_verify_with_mapper(diagnostic, mapper)]),
        edit: Some(workspace_edit_from_tool_action_edit(
            uri,
            document,
            source_edit,
            mapper,
        )?),
        command: None,
        ..CodeAction::default()
    })
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

/// Converts source-level Arcweft tooling actions from one exact document lease into LSP edits.
pub fn source_code_actions_with_mapper(
    uri: &Uri,
    document: &Arc<SourceDocument>,
    mapper: &impl LspPositionMapper,
) -> Result<Vec<CodeAction>, arcweft_tooling::model::ToolingError> {
    arcweft_tooling::code_actions::source_code_actions(Arc::clone(document))?
        .into_iter()
        .map(|action| {
            let edit = action
                .edit
                .as_ref()
                .map(|edit| {
                    workspace_edit_from_tooling_edit(uri, edit, document.as_ref(), mapper).map_err(
                        |error| match error {
                            SourceSpanError::Reversed | SourceSpanError::OutOfBounds => {
                                arcweft_tooling::model::ToolingError::RangeOutOfBounds {
                                    start: edit.start,
                                    end: edit.end,
                                    len: document.text().len(),
                                }
                            }
                            SourceSpanError::NotUtf8Boundary => {
                                arcweft_tooling::model::ToolingError::InvalidCharBoundary {
                                    start: edit.start,
                                    end: edit.end,
                                }
                            }
                        },
                    )
                })
                .transpose()?;
            let diagnostics = action
                .diagnostics
                .iter()
                .map(|diagnostic| tooling_diagnostic_with_mapper(diagnostic, mapper))
                .collect::<Vec<_>>();
            Ok(CodeAction {
                title: action.label,
                kind: Some(CodeActionKind::REFACTOR_REWRITE),
                diagnostics: (!diagnostics.is_empty()).then_some(diagnostics),
                edit,
                command: None,
                ..CodeAction::default()
            })
        })
        .collect()
}

fn tooling_diagnostic_with_mapper(
    diagnostic: &arcweft_tooling::model::ToolingDiagnostic,
    mapper: &impl LspPositionMapper,
) -> Diagnostic {
    let mut converted = Diagnostic::new_simple(
        mapper.range_from_byte_span(diagnostic.start, diagnostic.end),
        diagnostic.message.clone(),
    );
    converted.severity = Some(DiagnosticSeverity::WARNING);
    converted.code = Some(NumberOrString::String(diagnostic.code.clone()));
    converted.source = Some("arcweft-tooling".to_owned());
    converted
}

/// Converts one Arcweft tooling edit into an LSP workspace edit.
pub fn workspace_edit_from_tooling_edit(
    uri: &Uri,
    edit: &arcweft_tooling::model::TextEdit,
    document: &SourceDocument,
    mapper: &impl LspPositionMapper,
) -> Result<WorkspaceEdit, SourceSpanError> {
    let edit = RevisionBoundTextEdit::try_from_tooling(document, edit)?;
    let span = edit.span();
    let text_edit = TextEdit::new(
        mapper.range_from_byte_span(span.range().start(), span.range().end()),
        edit.replacement().to_owned(),
    );
    Ok(WorkspaceEdit::new(HashMap::from([(
        uri.clone(),
        vec![text_edit],
    )])))
}

/// Converts an already-bound edit only when it still belongs to the current
/// source document revision.
pub fn workspace_edit_from_revision_bound_edit(
    uri: &Uri,
    edit: &RevisionBoundTextEdit,
    current: &SourceDocument,
    mapper: &impl LspPositionMapper,
) -> Result<WorkspaceEdit, RevisionBoundWorkspaceEditError> {
    if edit.span().source().id() != current.identity().id() {
        return Err(RevisionBoundWorkspaceEditError::WrongDocument {
            expected: current.identity().id().clone(),
            actual: edit.span().source().id().clone(),
        });
    }
    if edit.span().source().revision() != current.identity().revision() {
        return Err(RevisionBoundWorkspaceEditError::WrongRevision {
            expected: current.identity().revision(),
            actual: edit.span().source().revision(),
        });
    }
    let span = edit.span();
    let text_edit = TextEdit::new(
        mapper.range_from_byte_span(span.range().start(), span.range().end()),
        edit.replacement().to_owned(),
    );
    Ok(WorkspaceEdit::new(HashMap::from([(
        uri.clone(),
        vec![text_edit],
    )])))
}

/// Converts one verifier-owned source edit into an LSP workspace edit.
pub fn workspace_edit_from_tool_action_edit(
    uri: &Uri,
    current: &SourceDocument,
    edit: &ToolActionSourceEdit,
    mapper: &impl LspPositionMapper,
) -> Result<WorkspaceEdit, RevisionBoundWorkspaceEditError> {
    let span = edit.span();
    validate_verify_span(span, current)?;
    let text_edit = TextEdit::new(
        mapper.range_from_byte_span(span.start, span.end),
        edit.replacement().to_owned(),
    );
    Ok(WorkspaceEdit::new(HashMap::from([(
        uri.clone(),
        vec![text_edit],
    )])))
}

fn diagnostic_from_verify_with_mapper(
    diagnostic: &VerificationDiagnostic,
    mapper: &impl LspPositionMapper,
) -> Diagnostic {
    Diagnostic {
        range: diagnostic
            .source
            .as_ref()
            .map_or_else(default_range, |span| {
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

fn diagnostic_belongs_to_document(
    diagnostic: &VerificationDiagnostic,
    document: &SourceDocument,
) -> bool {
    diagnostic
        .source
        .as_ref()
        .is_some_and(|span| &span.source == document.identity() && span.validate_for(document))
}

fn validate_verify_span(
    span: &VerifySourceSpan,
    current: &SourceDocument,
) -> Result<(), RevisionBoundWorkspaceEditError> {
    if span.source.id() != current.identity().id() {
        return Err(RevisionBoundWorkspaceEditError::WrongDocument {
            expected: current.identity().id().clone(),
            actual: span.source.id().clone(),
        });
    }
    if span.source.revision() != current.identity().revision() {
        return Err(RevisionBoundWorkspaceEditError::WrongRevision {
            expected: current.identity().revision(),
            actual: span.source.revision(),
        });
    }
    Ok(())
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
    let mut label = String::from(name);
    for group in signature.groups() {
        label.push('(');
        for (index, parameter) in group.parameters().iter().enumerate() {
            if index > 0 {
                label.push_str(", ");
            }
            label.push_str(parameter.name().map_or("_", |name| name.as_str()));
            label.push_str(": ");
            label.push_str(&type_kind_label(parameter.ty()));
        }
        label.push(')');
    }
    label.push_str(" -> ");
    label.push_str(&type_kind_label(signature.return_type()));
    label
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

fn tooling_doc(
    adapter: &AdapterManifest,
    subject: &AdapterToolingSubject,
) -> Option<lsp_types::Documentation> {
    adapter
        .tooling_docs()
        .iter()
        .find(|doc| doc.subject() == subject)
        .map(|doc| {
            lsp_types::Documentation::String(
                doc.summary()
                    .into_iter()
                    .chain(doc.details())
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            )
        })
}

fn tooling_doc_text(adapter: &AdapterManifest, subject: &AdapterToolingSubject) -> String {
    tooling_doc(adapter, subject).map_or_else(String::new, |doc| match doc {
        lsp_types::Documentation::String(text) => format!("\n{text}"),
        lsp_types::Documentation::MarkupContent(markup) => format!("\n{}", markup.value),
    })
}

fn callable_path_label(path: &AdapterCallablePath) -> String {
    path.segments()
        .iter()
        .map(arcweft_adapter_context::callable::AdapterCallableName::as_str)
        .collect::<Vec<_>>()
        .join(".")
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
        AdapterTypeKind::Vec { item } => format!("Vec<{}>", type_kind_label(item)),
        AdapterTypeKind::Seq { item } => format!("Seq<{}>", type_kind_label(item)),
        AdapterTypeKind::Option { item } => format!("Option<{}>", type_kind_label(item)),
        AdapterTypeKind::Result { ok, error } => {
            format!(
                "Result<{}, {}>",
                type_kind_label(ok),
                type_kind_label(error)
            )
        }
        AdapterTypeKind::Tuple { items } => format!(
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
        AdapterTypeKind::Nominal { nominal } => {
            let mut label = nominal
                .path()
                .segments()
                .iter()
                .map(arcweft_adapter_context::manifest::AdapterNominalPathSegment::as_str)
                .collect::<Vec<_>>()
                .join(".");
            if !nominal.arguments().is_empty() {
                label.push('<');
                label.push_str(
                    &nominal
                        .arguments()
                        .iter()
                        .map(type_kind_label)
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                label.push('>');
            }
            label
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_adapter_context::manifest::{
        AdapterCallableGroupIndex, AdapterCallableName, AdapterCallableOverloadIndex,
        AdapterCallableParameterIndex, AdapterEnvironmentOwnerId, AdapterFunctionParam,
        AdapterHostCall, AdapterId, AdapterManifest, AdapterNominalDeclaration,
        AdapterNominalOwner, AdapterNominalPath, AdapterNominalPathPrefix,
        AdapterNominalPathSegment, AdapterNominalTypeRef, AdapterNominalVisibility,
        AdapterOpaqueTypeProducerId, AdapterParameterGroup, AdapterParameterPassing,
        AdapterParameterPresence, AdapterSymbol, AdapterSymbolPath, AdapterSymbolSegment,
        AdapterToolingDoc,
    };
    use arcweft_rust_abi::{
        ArcweftRustField, ArcweftRustFunction, ArcweftRustManifest,
        ArcweftRustOpaqueTypeProducerId, ArcweftRustPackage, ArcweftRustPackageId,
        ArcweftRustParam, ArcweftRustPurity, ArcweftRustStructShape, ArcweftRustTypeDecl,
        ArcweftRustTypeKind, ArcweftRustTypePath, ArcweftRustTypePathSegment, ArcweftRustTypeRef,
        ArcweftRustVariant, ArcweftRustVariantPayload,
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

    fn verifier_document(uri: &Uri) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new(uri.to_string()).expect("source id"),
            arcweft_source::SourceName::path(uri.to_string()),
            "01234567890123456789012345678901",
        )
        .expect("verifier source document")
    }

    fn verifier_span(document: &SourceDocument, start: usize, end: usize) -> VerifySourceSpan {
        VerifySourceSpan {
            source: document.identity().clone(),
            start,
            end,
        }
    }

    #[test]
    fn converts_report_diagnostic() {
        let uri = "file:///game/routes/opening.arcw"
            .parse::<Uri>()
            .expect("uri");
        let document = verifier_document(&uri);
        let report = VerificationReport {
            policy: VerificationPolicy::default(),
            diagnostics: vec![VerificationDiagnostic {
                id: "d1".to_owned(),
                severity: VerifySeverity::Error,
                message: "missing proof".to_owned(),
                source: Some(verifier_span(&document, 3, 8)),
                obligation: Some("obligation.0001".to_owned()),
                related_ids: Vec::new(),
                actions: Vec::new(),
            }],
            ..VerificationReport::default()
        };
        let diagnostics = diagnostics_from_report(&report, &document);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn verifier_source_edit_action_becomes_workspace_edit() {
        let uri = "file:///game/routes/opening.arcw"
            .parse::<Uri>()
            .expect("uri");
        let document = verifier_document(&uri);
        let report = VerificationReport {
            policy: VerificationPolicy::default(),
            diagnostics: vec![VerificationDiagnostic {
                id: "d1".to_owned(),
                severity: VerifySeverity::Warning,
                message: "missing proof".to_owned(),
                source: Some(verifier_span(&document, 3, 8)),
                obligation: Some("obligation.0001".to_owned()),
                related_ids: Vec::new(),
                actions: vec![ToolAction {
                    id: "action.generate_proof_stub".to_owned(),
                    label: "Generate proof stub".to_owned(),
                    kind: ToolActionKind::GenerateProofStub,
                    source_edit: Some(arcweft_verify::ToolActionSourceEdit {
                        span: verifier_span(&document, 10, 15),
                        replacement: "proof {}".to_owned(),
                        applicability: ToolActionApplicability::HasPlaceholders,
                    }),
                    command: None,
                }],
            }],
            ..VerificationReport::default()
        };

        let actions = code_actions_from_report_with_mapper(&uri, &document, &report, &TestMapper);

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
    fn verifier_empty_insertion_action_becomes_workspace_edit() {
        let uri = "file:///game/routes/opening.arcw"
            .parse::<Uri>()
            .expect("uri");
        let document = verifier_document(&uri);
        let report = VerificationReport {
            policy: VerificationPolicy::default(),
            diagnostics: vec![VerificationDiagnostic {
                id: "d1".to_owned(),
                severity: VerifySeverity::Warning,
                message: "missing proof".to_owned(),
                source: Some(verifier_span(&document, 3, 8)),
                obligation: Some("obligation.0001".to_owned()),
                related_ids: Vec::new(),
                actions: vec![ToolAction {
                    id: "action.generate_proof_stub".to_owned(),
                    label: "Generate proof stub".to_owned(),
                    kind: ToolActionKind::GenerateProofStub,
                    source_edit: Some(arcweft_verify::ToolActionSourceEdit {
                        span: verifier_span(&document, 21, 21),
                        replacement: "\n\nproof obligation_0001 {\n    check _\n}\n".to_owned(),
                        applicability: ToolActionApplicability::HasPlaceholders,
                    }),
                    command: None,
                }],
            }],
            ..VerificationReport::default()
        };

        let actions = code_actions_from_report_with_mapper(&uri, &document, &report, &TestMapper);

        let edit = actions[0].edit.as_ref().expect("workspace edit");
        let text_edit = &edit.changes.as_ref().expect("changes")[&uri][0];
        assert_eq!(text_edit.range.start, Position::new(0, 21));
        assert_eq!(text_edit.range.end, Position::new(0, 21));
        assert!(text_edit.new_text.contains("proof obligation_0001"));
    }

    #[test]
    fn verifier_host_action_becomes_command_action() {
        let uri = "file:///game/routes/opening.arcw"
            .parse::<Uri>()
            .expect("uri");
        let document = verifier_document(&uri);
        let report = VerificationReport {
            policy: VerificationPolicy::default(),
            diagnostics: vec![VerificationDiagnostic {
                id: "d1".to_owned(),
                severity: VerifySeverity::Warning,
                message: "inspect obligation".to_owned(),
                source: Some(verifier_span(&document, 3, 8)),
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

        let actions = code_actions_from_report(&uri, &document, &report);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].kind, Some(CodeActionKind::REFACTOR));
        assert!(actions[0].edit.is_none());
        assert_eq!(
            actions[0].command.as_ref().expect("command").command,
            "arcweft.verify.showObligation"
        );
    }

    #[test]
    fn revision_bound_workspace_edit_rejects_stale_and_wrong_documents() {
        let uri = "file:///game/routes/opening.arcw"
            .parse::<Uri>()
            .expect("uri");
        let source_id = SourceDocumentId::try_new(uri.to_string()).expect("source id");
        let producing = SourceDocument::try_new(
            source_id.clone(),
            arcweft_source::SourceName::path(uri.to_string()),
            "alice",
        )
        .expect("producing document");
        let unchanged = SourceDocument::try_new(
            source_id.clone(),
            arcweft_source::SourceName::path(uri.to_string()),
            "alice",
        )
        .expect("unchanged document");
        let changed = SourceDocument::try_new(
            source_id,
            arcweft_source::SourceName::path(uri.to_string()),
            "alicia",
        )
        .expect("changed document");
        let wrong_document = SourceDocument::try_new(
            SourceDocumentId::try_new("file:///game/routes/other.arcw").expect("source id"),
            arcweft_source::SourceName::path("other.arcw"),
            "alice",
        )
        .expect("other document");
        let edit = arcweft_tooling::model::TextEdit {
            start: 1,
            end: 4,
            replacement: "LIC".to_owned(),
        };
        let bound = RevisionBoundTextEdit::try_from_tooling(&producing, &edit)
            .expect("revision-bound edit");

        let workspace =
            workspace_edit_from_revision_bound_edit(&uri, &bound, &unchanged, &TestMapper)
                .expect("same revision edit");
        let converted = &workspace.changes.expect("changes")[&uri][0];
        assert_eq!(converted.range.start, Position::new(0, 1));
        assert_eq!(converted.range.end, Position::new(0, 4));
        assert_eq!(converted.new_text, "LIC");

        assert_eq!(
            workspace_edit_from_revision_bound_edit(&uri, &bound, &changed, &TestMapper),
            Err(RevisionBoundWorkspaceEditError::WrongRevision {
                expected: changed.identity().revision(),
                actual: producing.identity().revision(),
            })
        );
        assert_eq!(
            workspace_edit_from_revision_bound_edit(&uri, &bound, &wrong_document, &TestMapper),
            Err(RevisionBoundWorkspaceEditError::WrongDocument {
                expected: wrong_document.identity().id().clone(),
                actual: producing.identity().id().clone(),
            })
        );
    }

    #[test]
    fn exposes_source_actions() {
        let uri = "file:///game/routes/opening.arcw"
            .parse::<Uri>()
            .expect("uri");
        let source = "flow @.opening opening {\n    alice: [.shake amp=2px]hi[/][p]\n}\n";
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(uri.to_string()).expect("source id"),
                arcweft_source::SourceName::path(uri.to_string()),
                source,
            )
            .expect("source document"),
        );
        let mapped_actions = source_code_actions_with_mapper(&uri, &document, &TestMapper)
            .expect("mapped source code actions");
        assert_eq!(mapped_actions.len(), 1);
        assert_eq!(
            mapped_actions[0].title,
            "Canonicalize inferred rich-text tags"
        );
        assert!(mapped_actions[0].edit.is_some());
        assert!(mapped_actions[0].command.is_none());
    }

    #[test]
    fn exposes_rust_adapter_completion_and_hover() {
        let package = rust_package_id("truck_game");
        let manifest = ArcweftRustManifest::new(ArcweftRustPackage {
            id: package.clone(),
            version: "0.1.0".to_owned(),
            metadata_hash: None,
        })
        .with_type(ArcweftRustTypeDecl {
            path: rust_type_path(["Rank"]),
            rust_path: "truck_game::Rank".to_owned(),
            opaque_producer: fixture_rust_producer(),
            parameters: Vec::new(),
            kind: ArcweftRustTypeKind::Enum {
                variants: Vec::new(),
            },
        })
        .with_function(ArcweftRustFunction {
            name: "score_to_rank".to_owned(),
            rust_path: "truck_game::score_to_rank".to_owned(),
            params: vec![ArcweftRustParam {
                name: "score".to_owned(),
                ty: ArcweftRustTypeRef::I32,
            }],
            return_type: ArcweftRustTypeRef::Nominal {
                package: package.clone(),
                path: rust_type_path(["Rank"]),
                arguments: Vec::new(),
            },
            purity: ArcweftRustPurity::Pure,
            effects: Vec::new(),
        });
        let adapter = AdapterManifest::new("fixture", "Fixture")
            .try_with_rust_package_mount(package, empty_rust_mount())
            .expect("test Rust package mount is unique")
            .try_with_rust_manifest(&manifest)
            .expect("Rust callable metadata is typed");
        let context = ArcweftLspContext::new(&adapter);

        let completions = rust_adapter_completions(&context);
        assert!(completions.iter().any(|item| item.label == "score_to_rank"));
        assert!(completions.iter().any(|item| item.label == "Rank"));
        let hover = rust_adapter_hover(&context, "score_to_rank").expect("hover is available");
        assert!(
            matches!(hover.contents, HoverContents::Scalar(MarkedString::String(text)) if text.contains("score: i32"))
        );
    }

    #[test]
    fn exposes_complex_rust_adapter_type_shapes_from_metadata() {
        let manifest = complex_rust_manifest();
        let adapter = AdapterManifest::new("fixture", "Fixture")
            .try_with_rust_package_mount(manifest.package.id.clone(), empty_rust_mount())
            .expect("test Rust package mount is unique")
            .try_with_rust_manifest(&manifest)
            .expect("Rust callable metadata is typed");
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
                && detail.contains("rank: Option<quest_logic::Rank>")
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
    }

    fn complex_rust_manifest() -> ArcweftRustManifest {
        ArcweftRustManifest::new(ArcweftRustPackage {
            id: rust_package_id("quest_logic"),
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
            path: rust_type_path(["PlayerStats"]),
            rust_path: "quest_logic::PlayerStats".to_owned(),
            opaque_producer: fixture_rust_producer(),
            parameters: Vec::new(),
            kind: ArcweftRustTypeKind::Struct {
                shape: ArcweftRustStructShape::Record {
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
                                item: Box::new(rust_nominal("quest_logic", ["Rank"])),
                            },
                        },
                    ],
                },
            },
        }
    }

    fn rank_type() -> ArcweftRustTypeDecl {
        ArcweftRustTypeDecl {
            path: rust_type_path(["Rank"]),
            rust_path: "quest_logic::Rank".to_owned(),
            opaque_producer: fixture_rust_producer(),
            parameters: Vec::new(),
            kind: ArcweftRustTypeKind::Enum {
                variants: vec![
                    ArcweftRustVariant {
                        name: "Bronze".to_owned(),
                        payload: ArcweftRustVariantPayload::Unit,
                    },
                    ArcweftRustVariant {
                        name: "Custom".to_owned(),
                        payload: ArcweftRustVariantPayload::Record {
                            fields: vec![ArcweftRustField {
                                name: "label".to_owned(),
                                ty: ArcweftRustTypeRef::String,
                            }],
                        },
                    },
                ],
            },
        }
    }

    fn session_id_type() -> ArcweftRustTypeDecl {
        ArcweftRustTypeDecl {
            path: rust_type_path(["SessionId"]),
            rust_path: "quest_logic::SessionId".to_owned(),
            opaque_producer: fixture_rust_producer(),
            parameters: Vec::new(),
            kind: ArcweftRustTypeKind::Newtype {
                inner: ArcweftRustTypeRef::U64,
            },
        }
    }

    fn evaluate_function() -> ArcweftRustFunction {
        ArcweftRustFunction {
            name: "quest_evaluate".to_owned(),
            rust_path: "quest_logic::evaluate".to_owned(),
            params: vec![
                ArcweftRustParam {
                    name: "stats".to_owned(),
                    ty: rust_nominal("quest_logic", ["PlayerStats"]),
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
            return_type: rust_nominal("quest_logic", ["Rank"]),
            purity: ArcweftRustPurity::Pure,
            effects: Vec::new(),
        }
    }

    fn fixture_rust_producer() -> ArcweftRustOpaqueTypeProducerId {
        ArcweftRustOpaqueTypeProducerId::try_new("fixture.project.external-types")
            .expect("fixture producer is valid")
    }

    #[test]
    fn exposes_adapter_manifest_completions_and_hover() {
        let nominal_path = adapter_nominal_path(["CustomApi"]);
        let nominal_type = AdapterTypeKind::Nominal {
            nominal: AdapterNominalTypeRef::try_new(
                AdapterNominalOwner::Environment {
                    owner: AdapterEnvironmentOwnerId::for_adapter(&AdapterId::new("custom")),
                },
                nominal_path.clone(),
                [],
            )
            .expect("test nominal reference is valid"),
        };
        let adapter = AdapterManifest::new("custom", "Custom")
            .try_with_nominal_declaration(
                AdapterNominalDeclaration::try_new(
                    nominal_path,
                    0,
                    AdapterOpaqueTypeProducerId::try_new("fixture.project.external-types")
                        .expect("fixture producer is valid"),
                    AdapterNominalVisibility::Public,
                    "CustomApi",
                )
                .expect("test nominal declaration is valid"),
            )
            .expect("test nominal declaration is unique")
            .with_symbol(AdapterSymbol::new(
                adapter_symbol_path(["custom"]),
                nominal_type.clone(),
            ))
            .with_method_signature(
                nominal_type,
                adapter_name("read"),
                adapter_overload(0),
                adapter_signature([("path", AdapterTypeKind::String)], AdapterTypeKind::String),
                [],
            )
            .with_function_signature(
                adapter_path(["custom", "read"]),
                adapter_overload(0),
                adapter_signature([("path", AdapterTypeKind::String)], AdapterTypeKind::String),
                [AdapterEffectCapability::new("custom.read")],
            )
            .with_effect(AdapterEffectCapability::new("custom.read"))
            .with_host_call(AdapterHostCall::new(
                "custom.read",
                [AdapterEffectCapability::new("custom.read")],
            ))
            .with_tooling_doc(
                AdapterToolingDoc::try_new(
                    AdapterToolingSubject::Free {
                        kind: AdapterFreeCallableKind::Function,
                        path: adapter_path(["custom", "read"]),
                        overload: adapter_overload(0),
                    },
                    Some("Read custom content.".to_owned()),
                    None,
                    Vec::new(),
                )
                .expect("test documentation is typed"),
            );
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

    fn adapter_name(value: &str) -> AdapterCallableName {
        AdapterCallableName::try_new(value).expect("valid test callable name")
    }

    fn adapter_path<const N: usize>(segments: [&str; N]) -> AdapterCallablePath {
        AdapterCallablePath::try_new(segments.into_iter().map(adapter_name))
            .expect("test callable path is non-empty")
    }

    fn adapter_symbol_path<const N: usize>(segments: [&str; N]) -> AdapterSymbolPath {
        AdapterSymbolPath::try_new(segments.map(|segment| {
            AdapterSymbolSegment::try_new(segment).expect("valid test adapter symbol segment")
        }))
        .expect("test adapter symbol path is non-empty")
    }

    fn adapter_nominal_path<const N: usize>(segments: [&str; N]) -> AdapterNominalPath {
        AdapterNominalPath::try_new(segments.map(|segment| {
            AdapterNominalPathSegment::try_new(segment).expect("valid test adapter nominal segment")
        }))
        .expect("test adapter nominal path is non-empty")
    }

    fn empty_rust_mount() -> AdapterNominalPathPrefix {
        AdapterNominalPathPrefix::try_new([]).expect("empty Rust package mount is valid")
    }

    fn rust_package_id(value: &str) -> ArcweftRustPackageId {
        ArcweftRustPackageId::try_new(value).expect("valid test Rust package ID")
    }

    fn rust_type_path<const N: usize>(segments: [&str; N]) -> ArcweftRustTypePath {
        ArcweftRustTypePath::try_new(segments.map(|segment| {
            ArcweftRustTypePathSegment::try_new(segment).expect("valid test Rust path segment")
        }))
        .expect("test Rust type path is non-empty")
    }

    fn rust_nominal<const N: usize>(package: &str, segments: [&str; N]) -> ArcweftRustTypeRef {
        ArcweftRustTypeRef::Nominal {
            package: rust_package_id(package),
            path: rust_type_path(segments),
            arguments: Vec::new(),
        }
    }

    fn adapter_overload(value: usize) -> AdapterCallableOverloadIndex {
        AdapterCallableOverloadIndex::try_from_usize(value).expect("test overload fits")
    }

    fn adapter_signature<const N: usize>(
        parameters: [(&str, AdapterTypeKind); N],
        result: AdapterTypeKind,
    ) -> AdapterFunctionSignature {
        let parameters = parameters
            .into_iter()
            .enumerate()
            .map(|(index, (name, ty))| {
                AdapterFunctionParam::try_new(
                    AdapterCallableParameterIndex::try_from_usize(index)
                        .expect("test parameter index fits"),
                    Some(adapter_name(name)),
                    ty,
                    AdapterParameterPassing::PositionalOrNamed,
                    AdapterParameterPresence::Required,
                )
                .expect("test parameter is valid")
            })
            .collect();
        AdapterFunctionSignature::try_new(
            vec![
                AdapterParameterGroup::try_new(
                    AdapterCallableGroupIndex::try_from_usize(0).expect("initial group fits"),
                    parameters,
                )
                .expect("test parameter group is valid"),
            ],
            result,
        )
        .expect("test adapter signature is valid")
    }
}
