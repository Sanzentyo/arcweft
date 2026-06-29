use annotate_snippets::{Annotation, AnnotationKind, Group, Level, Patch, Renderer, Snippet};
use arcweft_source::{
    Diagnostic, DiagnosticLabel, DiagnosticLabelStyle, DiagnosticSeverity, SourceName, SourceSpan,
};
use std::io::{self, IsTerminal};
use std::path::Path;

/// Terminal diagnostic renderer for source-backed Arcweft diagnostics.
pub(in crate::app) struct DiagnosticEmitter {
    renderer: Renderer,
}

/// Source text and display path used while rendering one direct-source diagnostic batch.
pub(in crate::app) struct DiagnosticSource<'a> {
    path: String,
    text: &'a str,
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

impl<'a> DiagnosticSource<'a> {
    pub(in crate::app) fn new(path: &Path, text: &'a str) -> Self {
        Self::from_display_path(path.display().to_string(), text)
    }

    pub(in crate::app) fn from_display_path(path: impl Into<String>, text: &'a str) -> Self {
        Self {
            path: path.into(),
            text,
        }
    }

    fn path_for(&self, source_name: &SourceName) -> String {
        match source_name {
            SourceName::Path(path) if path == "<memory>" => self.path.clone(),
            SourceName::Path(path) => path.clone(),
            SourceName::Generated => "<generated>".to_owned(),
        }
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
                group = group.element(snippet_for_span(source, span));
            }
        } else {
            for label in labels {
                group = group.element(snippet_for_label(source, label));
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
                suggestion_group = suggestion_group.element(
                    Snippet::source(source.text)
                        .path(source.path_for(edit.span().source()))
                        .patch(Patch::new(
                            edit.span().range().as_range(),
                            edit.replacement().to_owned(),
                        )),
                );
            }
        } else if !suggestion.edits().is_empty() {
            suggestion_group = suggestion_group.element(
                Level::NOTE.message("source text is unavailable; edit preview omitted".to_owned()),
            );
        }
        groups.push(suggestion_group);
    }
    groups
}

fn snippet_for_span<'source>(
    source: &'source DiagnosticSource<'source>,
    span: &SourceSpan,
) -> Snippet<'source, Annotation<'source>> {
    Snippet::source(source.text)
        .path(source.path_for(span.source()))
        .annotation(AnnotationKind::Primary.span(span.range().as_range()))
}

fn snippet_for_label<'source>(
    source: &'source DiagnosticSource<'source>,
    label: &DiagnosticLabel,
) -> Snippet<'source, Annotation<'source>> {
    let annotation_kind = match label.style() {
        DiagnosticLabelStyle::Primary => AnnotationKind::Primary,
        DiagnosticLabelStyle::Secondary => AnnotationKind::Context,
    };
    let annotation = annotation_kind.span(label.span().range().as_range());
    let snippet = Snippet::source(source.text).path(source.path_for(label.span().source()));
    if let Some(message) = label.message() {
        snippet.annotation(annotation.label(message.to_owned()))
    } else {
        snippet.annotation(annotation)
    }
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
mod tests {
    use super::*;
    use arcweft_source::{
        DiagnosticApplicability, DiagnosticLabel, DiagnosticSuggestion, SourceEdit, SourceRange,
        SourceSpan,
    };

    #[test]
    fn plain_renderer_includes_code_label_and_patch() {
        let source = "flow @flow.opening {\n}\n";
        let span = SourceSpan::new(SourceName::path("game.arcw"), SourceRange::new(5, 18));
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
            );
        let source = DiagnosticSource::new(Path::new("game.arcw"), source);
        let groups = diagnostic_groups(&diagnostic, &source);
        let rendered = Renderer::plain().render(&groups);
        assert!(rendered.contains("hint[AWF0103]: explicit id"));
        assert!(rendered.contains("style::explicit_decl_id"));
        assert!(rendered.contains("replace explicit id with compact form"));
        assert!(rendered.contains("+ flow opening"));
    }
}
