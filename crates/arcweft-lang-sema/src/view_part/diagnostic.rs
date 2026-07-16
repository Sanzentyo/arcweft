use arcweft_source::SourceSpan;

use super::CheckedViewId;

/// Stable semantic View-part failure family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewPartDiagnosticCode {
    InvalidOwner,
    InvalidLocalName,
    InvalidPublicName,
    DuplicateLocalTarget,
    MissingLocalTarget,
    DuplicateExportTarget,
    DuplicatePublicName,
    UnsupportedCallViewExport,
    PartIdOverflow,
}

/// Structured semantic diagnostic for one View-part contract violation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPartDiagnostic {
    code: ViewPartDiagnosticCode,
    message: String,
    span: SourceSpan,
    owner: Option<CheckedViewId>,
}

impl ViewPartDiagnostic {
    pub(super) fn new(
        code: ViewPartDiagnosticCode,
        message: impl Into<String>,
        span: SourceSpan,
        owner: Option<CheckedViewId>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            span,
            owner,
        }
    }

    pub const fn code(&self) -> ViewPartDiagnosticCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }

    pub const fn owner(&self) -> Option<&CheckedViewId> {
        self.owner.as_ref()
    }
}
