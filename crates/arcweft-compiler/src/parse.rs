use arcweft_lang_syntax::lint::{SyntaxLint, SyntaxLintSeverity};

/// Counts source lints that should be reported as warnings.
pub fn count_warning_lints(lints: &[SyntaxLint]) -> usize {
    lints
        .iter()
        .filter(|lint| matches!(lint.severity(), SyntaxLintSeverity::Warning))
        .count()
}

/// Returns whether any source lint should stop compilation.
pub fn has_error_lints(lints: &[SyntaxLint]) -> bool {
    lints
        .iter()
        .any(|lint| matches!(lint.severity(), SyntaxLintSeverity::Error))
}
