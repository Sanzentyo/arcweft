use annotate_snippets::{Annotation, AnnotationKind, Group, Level, Patch, Renderer, Snippet};
use arcweft_source::{
    Diagnostic, DiagnosticCommand, DiagnosticLabel, DiagnosticLabelStyle, DiagnosticSeverity,
    SourceDocument, SourceDocumentIdentity, SourceSpan,
};
use std::io::{self, IsTerminal};

/// Terminal diagnostic renderer for source-backed Arcweft diagnostics.
pub(in crate::app) struct DiagnosticEmitter {
    renderer: Renderer,
}

/// Source text and display path used while rendering one direct-source diagnostic batch.
pub(in crate::app) struct DiagnosticSource<'a> {
    document: &'a SourceDocument,
}

impl DiagnosticEmitter {
    pub(in crate::app) fn stderr() -> Self {
        let styled = io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        if styled {
            Self {
                renderer: Renderer::styled(),
            }
        } else {
            Self::plain()
        }
    }

    pub(in crate::app) fn plain() -> Self {
        Self {
            renderer: Renderer::plain(),
        }
    }

    pub(in crate::app) fn emit(&self, diagnostic: &Diagnostic, source: &DiagnosticSource<'_>) {
        let groups = diagnostic_groups(diagnostic, source);
        eprintln!("{}", self.renderer.render(&groups));
    }

    pub(in crate::app) fn emit_without_source(&self, diagnostic: &Diagnostic) {
        let groups = diagnostic_groups_without_source(diagnostic);
        eprintln!("{}", self.renderer.render(&groups));
    }

    pub(in crate::app) fn emit_all<'a>(
        &self,
        diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
        source: &DiagnosticSource<'_>,
    ) {
        for diagnostic in diagnostics {
            self.emit(diagnostic, source);
        }
    }
}

pub(in crate::app) fn emit_diagnostics(document: &SourceDocument, diagnostics: &[Diagnostic]) {
    let emitter = DiagnosticEmitter::stderr();
    let source = DiagnosticSource::new(document);
    emitter.emit_all(diagnostics, &source);
}

impl<'a> DiagnosticSource<'a> {
    pub(in crate::app) const fn new(document: &'a SourceDocument) -> Self {
        Self { document }
    }

    fn text(&self) -> &'a str {
        self.document.text()
    }

    fn path_for(&self, identity: &SourceDocumentIdentity) -> Option<String> {
        (identity == self.document.identity())
            .then(|| self.document.display_name().display_name().to_owned())
    }
}

fn diagnostic_groups<'source>(
    diagnostic: &Diagnostic,
    source: &'source DiagnosticSource<'source>,
) -> Vec<Group<'source>> {
    diagnostic_groups_with_optional_source(diagnostic, Some(source))
}

fn diagnostic_groups_without_source(diagnostic: &Diagnostic) -> Vec<Group<'static>> {
    diagnostic_groups_with_optional_source(diagnostic, None)
}

fn diagnostic_groups_with_optional_source<'source>(
    diagnostic: &Diagnostic,
    source: Option<&'source DiagnosticSource<'source>>,
) -> Vec<Group<'source>> {
    let mut groups = Vec::new();
    let mut title = level_for(diagnostic.severity()).primary_title(diagnostic.message().to_owned());
    if let Some(code) = diagnostic.code() {
        title = title.id(code.as_str().to_owned());
    }
    let mut group = Group::with_title(title);
    if let Some(source) = source {
        let labels = diagnostic.labels();
        if labels.is_empty() {
            if let Some(span) = diagnostic.span() {
                if let Some(snippet) = snippet_for_span(source, span) {
                    group = group.element(snippet);
                } else {
                    group = group.element(stale_span_note());
                }
            }
        } else {
            for label in labels {
                if let Some(snippet) = snippet_for_label(source, label) {
                    group = group.element(snippet);
                } else {
                    group = group.element(stale_span_note());
                }
            }
        }
    } else if diagnostic.span().is_some() || !diagnostic.labels().is_empty() {
        group = group.element(
            Level::NOTE.message("source text is unavailable; span labels were omitted".to_owned()),
        );
    }
    for note in diagnostic.notes() {
        group = group.element(Level::NOTE.message(note.to_owned()));
    }
    groups.push(group);

    for suggestion in diagnostic.suggestions() {
        let mut suggestion_group = Group::with_title(
            Level::HELP
                .with_name(Some("help"))
                .secondary_title(suggestion.message().to_owned()),
        );
        if let Some(source) = source {
            for edit in suggestion.edits() {
                if let Some(path) = source.path_for(edit.span().source()) {
                    suggestion_group = suggestion_group.element(
                        Snippet::source(source.text()).path(path).patch(Patch::new(
                            edit.span().range().as_range(),
                            edit.replacement().to_owned(),
                        )),
                    );
                } else {
                    suggestion_group = suggestion_group.element(stale_span_note());
                }
            }
        } else if !suggestion.edits().is_empty() {
            suggestion_group = suggestion_group.element(
                Level::NOTE.message("source text is unavailable; edit preview omitted".to_owned()),
            );
        }
        groups.push(suggestion_group);
    }
    for command in diagnostic.commands() {
        groups.push(Group::with_title(
            Level::HELP
                .with_name(Some("action"))
                .secondary_title(command_title(command)),
        ));
    }
    groups
}

fn command_title(command: &DiagnosticCommand) -> String {
    let title = format!(
        "Run verifier command `{}`: {}",
        command.id(),
        command.title()
    );
    if command.arguments().is_empty() {
        title
    } else {
        format!("{title} (args: {})", command.arguments().join(", "))
    }
}

fn snippet_for_span<'source>(
    source: &'source DiagnosticSource<'source>,
    span: &SourceSpan,
) -> Option<Snippet<'source, Annotation<'source>>> {
    Some(
        Snippet::source(source.text())
            .path(source.path_for(span.source())?)
            .annotation(AnnotationKind::Primary.span(span.range().as_range())),
    )
}

fn snippet_for_label<'source>(
    source: &'source DiagnosticSource<'source>,
    label: &DiagnosticLabel,
) -> Option<Snippet<'source, Annotation<'source>>> {
    let annotation_kind = match label.style() {
        DiagnosticLabelStyle::Primary => AnnotationKind::Primary,
        DiagnosticLabelStyle::Secondary => AnnotationKind::Context,
    };
    let annotation = annotation_kind.span(label.span().range().as_range());
    let snippet = Snippet::source(source.text()).path(source.path_for(label.span().source())?);
    if let Some(message) = label.message() {
        Some(snippet.annotation(annotation.label(message.to_owned())))
    } else {
        Some(snippet.annotation(annotation))
    }
}

fn stale_span_note() -> annotate_snippets::Message<'static> {
    Level::NOTE.message(
        "diagnostic span belongs to a different source revision; source excerpt was omitted"
            .to_owned(),
    )
}

fn level_for(severity: DiagnosticSeverity) -> Level<'static> {
    match severity {
        DiagnosticSeverity::Error => Level::ERROR,
        DiagnosticSeverity::Warning => Level::WARNING,
        DiagnosticSeverity::Info => Level::INFO,
        DiagnosticSeverity::Hint => Level::HELP.with_name(Some("hint")),
    }
}

#[cfg(test)]
mod renderer_tests {
    use super::*;
    use arcweft_lang_syntax::parser::{
        ParseOptions, parse_document_with_source, recovery::ParseErrorKind,
    };
    use arcweft_source::{
        DiagnosticApplicability, DiagnosticCommand, DiagnosticLabel, DiagnosticSuggestion,
        SourceDocumentId, SourceEdit, SourceName, SourceRange,
    };
    use std::sync::Arc;

    fn document(text: &str) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://game/game.arcw")
                .expect("test document id"),
            SourceName::path("game.arcw"),
            text,
        )
        .expect("test source document")
    }

    #[test]
    fn plain_renderer_includes_code_label_and_patch() {
        let source = "flow @flow.opening {\n}\n";
        let document = document(source);
        let span = document.span(SourceRange::new(5, 18)).expect("test span");
        let diagnostic = Diagnostic::new(DiagnosticSeverity::Hint, "explicit id")
            .with_code("AWF0103")
            .with_label(DiagnosticLabel::primary(
                span.clone(),
                Some("style::explicit_decl_id".to_owned()),
            ))
            .with_suggestion(
                DiagnosticSuggestion::new(
                    "replace explicit id with compact form",
                    DiagnosticApplicability::MachineApplicable,
                )
                .with_edit(SourceEdit::new(span, "opening")),
            )
            .with_command(
                DiagnosticCommand::new("arcweft.verify.showObligation", "Show proof obligation")
                    .with_argument("obligation.0001"),
            );
        let source = DiagnosticSource::new(&document);
        let groups = diagnostic_groups(&diagnostic, &source);
        let rendered = Renderer::plain().render(&groups);
        assert!(rendered.contains("hint[AWF0103]: explicit id"));
        assert!(rendered.contains("style::explicit_decl_id"));
        assert!(rendered.contains("replace explicit id with compact form"));
        assert!(rendered.contains("+ flow opening"));
        assert!(rendered.contains("arcweft.verify.showObligation"));
    }

    #[test]
    fn plain_renderer_includes_verifier_proof_stub_patch_preview() {
        let source = "flow opening {\n}\n";
        let document = document(source);
        let span = document
            .span(SourceRange::new(source.len(), source.len()))
            .expect("test insertion span");
        let diagnostic = Diagnostic::new(
            DiagnosticSeverity::Warning,
            "lifetime promotion requires proof",
        )
        .with_code("AWF0703")
        .with_suggestion(
            DiagnosticSuggestion::new(
                "Generate proof stub",
                DiagnosticApplicability::HasPlaceholders,
            )
            .with_edit(SourceEdit::new(
                span,
                "\n\nproof obligation_0001 {\n    // TODO: prove it\n    check _\n}\n",
            )),
        );
        let source = DiagnosticSource::new(&document);
        let groups = diagnostic_groups(&diagnostic, &source);
        let rendered = Renderer::plain().render(&groups);

        assert!(rendered.contains("warning[AWF0703]: lifetime promotion requires proof"));
        assert!(rendered.contains("Generate proof stub"));
        assert!(rendered.contains("+ proof obligation_0001"));
        assert!(rendered.contains("+     check _"));
    }

    #[test]
    fn plain_renderer_includes_verifier_unsafe_audit_patch_preview() {
        let source = "flow audit_demo {\n    unsafe lifetime @unsafe.cache_last_line {\n        let summary = promote_unchecked('flow)\n    }\n}\n";
        let marker = "@unsafe.cache_last_line {";
        let start = source.find(marker).expect("unsafe lifetime marker") + marker.len() - 1;
        let document = document(source);
        let span = document
            .span(SourceRange::new(start, start + 1))
            .expect("test replacement span");
        let diagnostic = Diagnostic::new(
            DiagnosticSeverity::Warning,
            "unsafe lifetime audit `unsafe.cache_last_line` must include string reason and SAFETY docs",
        )
        .with_code("AWF0703")
        .with_suggestion(
            DiagnosticSuggestion::new(
                "Generate unsafe lifetime audit metadata",
                DiagnosticApplicability::HasPlaceholders,
            )
            .with_edit(SourceEdit::new(
                span,
                " reason = _\n{\n    /// SAFETY: TODO: justify this unsafe lifetime block.",
            )),
        );
        let source = DiagnosticSource::new(&document);
        let groups = diagnostic_groups(&diagnostic, &source);
        let rendered = Renderer::plain().render(&groups);

        assert!(rendered.contains(
            "warning[AWF0703]: unsafe lifetime audit `unsafe.cache_last_line` must include string reason and SAFETY docs"
        ));
        assert!(rendered.contains("Generate unsafe lifetime audit metadata"));
        assert!(rendered.contains("reason = _"));
        assert!(rendered.contains("/// SAFETY: TODO"));
    }

    #[test]
    fn plain_renderer_preserves_typed_parser_code_without_injecting_kind_label() {
        let source = "pub view Card() {\n    export part as heading\n    Panel()\n}\n";
        let document = Arc::new(document(source));
        let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
        let error = parsed
            .errors()
            .iter()
            .find(|error| error.kind() == ParseErrorKind::ViewExportPartMissingLocal)
            .expect("typed parser error");
        let diagnostic = error.diagnostic(document.as_ref());
        let source = DiagnosticSource::new(document.as_ref());
        let groups = diagnostic_groups(&diagnostic, &source);
        let rendered = Renderer::plain().render(&groups);

        assert!(rendered.contains(
            "error[view::export_part_missing_local]: View part export needs a private local target before `as`"
        ));
        assert!(rendered.contains("expected: local part name"));
        assert!(rendered.contains("use local part name syntax"));
        assert!(!rendered.contains("Missing local View part name"));
    }
}

#[cfg(test)]
mod tests;
