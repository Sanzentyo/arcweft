use crate::SourceSpan;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticCode(String);

impl DiagnosticCode {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticLabelStyle {
    Primary,
    Secondary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticLabel {
    style: DiagnosticLabelStyle,
    span: SourceSpan,
    message: Option<String>,
}

impl DiagnosticLabel {
    pub fn primary(span: SourceSpan, message: impl Into<Option<String>>) -> Self {
        Self {
            style: DiagnosticLabelStyle::Primary,
            span,
            message: message.into(),
        }
    }

    pub fn secondary(span: SourceSpan, message: impl Into<Option<String>>) -> Self {
        Self {
            style: DiagnosticLabelStyle::Secondary,
            span,
            message: message.into(),
        }
    }

    pub const fn style(&self) -> DiagnosticLabelStyle {
        self.style
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticApplicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
    Unspecified,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEdit {
    span: SourceSpan,
    replacement: String,
}

impl SourceEdit {
    pub fn new(span: SourceSpan, replacement: impl Into<String>) -> Self {
        Self {
            span,
            replacement: replacement.into(),
        }
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }

    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticSuggestion {
    message: String,
    edits: Vec<SourceEdit>,
    applicability: DiagnosticApplicability,
}

impl DiagnosticSuggestion {
    pub fn new(message: impl Into<String>, applicability: DiagnosticApplicability) -> Self {
        Self {
            message: message.into(),
            edits: Vec::new(),
            applicability,
        }
    }

    #[must_use]
    pub fn with_edit(mut self, edit: SourceEdit) -> Self {
        self.edits.push(edit);
        self
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn edits(&self) -> &[SourceEdit] {
        &self.edits
    }

    pub const fn applicability(&self) -> DiagnosticApplicability {
        self.applicability
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticCommand {
    id: String,
    title: String,
    arguments: Vec<String>,
}

impl DiagnosticCommand {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            arguments: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_argument(mut self, argument: impl Into<String>) -> Self {
        self.arguments.push(argument.into());
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    severity: DiagnosticSeverity,
    code: Option<DiagnosticCode>,
    message: String,
    span: Option<SourceSpan>,
    labels: Vec<DiagnosticLabel>,
    notes: Vec<String>,
    suggestions: Vec<DiagnosticSuggestion>,
    commands: Vec<DiagnosticCommand>,
}

impl Diagnostic {
    pub fn new(severity: DiagnosticSeverity, message: impl Into<String>) -> Self {
        Self {
            severity,
            code: None,
            message: message.into(),
            span: None,
            labels: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
            commands: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(DiagnosticCode::new(code));
        self
    }

    #[must_use]
    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    #[must_use]
    pub fn with_label(mut self, label: DiagnosticLabel) -> Self {
        if label.style == DiagnosticLabelStyle::Primary && self.span.is_none() {
            self.span = Some(label.span.clone());
        }
        self.labels.push(label);
        self
    }

    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    #[must_use]
    pub fn with_suggestion(mut self, suggestion: DiagnosticSuggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    #[must_use]
    pub fn with_command(mut self, command: DiagnosticCommand) -> Self {
        self.commands.push(command);
        self
    }

    pub const fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    pub const fn code(&self) -> Option<&DiagnosticCode> {
        self.code.as_ref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn span(&self) -> Option<&SourceSpan> {
        self.span.as_ref()
    }

    pub fn labels(&self) -> &[DiagnosticLabel] {
        &self.labels
    }

    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    pub fn suggestions(&self) -> &[DiagnosticSuggestion] {
        &self.suggestions
    }

    pub fn commands(&self) -> &[DiagnosticCommand] {
        &self.commands
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiagnosticBag {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticBag {
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Diagnostic> {
        self.diagnostics.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

impl<'a> IntoIterator for &'a DiagnosticBag {
    type IntoIter = std::slice::Iter<'a, Diagnostic>;
    type Item = &'a Diagnostic;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Diagnostic, DiagnosticApplicability, DiagnosticBag, DiagnosticCommand, DiagnosticLabel,
        DiagnosticSeverity, DiagnosticSuggestion, SourceEdit,
    };
    use crate::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

    #[test]
    fn diagnostic_preserves_revision_bound_span() {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://game/game.arcw").expect("id"),
            SourceName::path("game.arcw"),
            "0123456789",
        )
        .expect("document");
        let span = document.span(SourceRange::new(2, 8)).expect("source span");
        let suggestion = DiagnosticSuggestion::new(
            "replace with compact form",
            DiagnosticApplicability::MachineApplicable,
        )
        .with_edit(SourceEdit::new(span.clone(), "flow opening"));
        let diagnostic = Diagnostic::new(DiagnosticSeverity::Warning, "check")
            .with_code("AWF0103")
            .with_label(DiagnosticLabel::primary(
                span.clone(),
                Some("explicit id".to_owned()),
            ))
            .with_note("style lint")
            .with_suggestion(suggestion)
            .with_command(
                DiagnosticCommand::new("arcweft.verify.showObligation", "Show proof obligation")
                    .with_argument("obligation.0001"),
            );
        let mut bag = DiagnosticBag::default();
        bag.push(diagnostic);
        let diagnostic = bag.iter().next().expect("diagnostic");
        assert_eq!(diagnostic.severity(), DiagnosticSeverity::Warning);
        assert_eq!(diagnostic.message(), "check");
        assert_eq!(diagnostic.code().expect("code").as_str(), "AWF0103");
        assert_eq!(
            diagnostic.span().expect("span").source(),
            document.identity()
        );
        assert_eq!(diagnostic.span().expect("span").range().as_range(), 2..8);
        assert_eq!(diagnostic.labels().len(), 1);
        assert_eq!(diagnostic.notes(), &["style lint".to_owned()]);
        assert_eq!(diagnostic.suggestions().len(), 1);
        assert_eq!(diagnostic.commands().len(), 1);
    }
}
