use arcweft_lang_syntax::ast::items::TypedSyntaxTree;
use arcweft_lang_syntax::lint::{SyntaxLint, SyntaxLintSeverity, lint_id_policy};
use arcweft_lang_syntax::parser::{ParseOptions, SourceDialect, parse_document, parse_source};
use arcweft_lang_syntax::source::ParsedSource;

/// Parses source text into the shared syntax parser output.
pub fn parse_source_text(source: impl Into<String>) -> ParsedSource {
    parse_source(source)
}

/// Parses Agent dialect source text into the shared syntax parser output.
pub fn parse_agent_source_text(source: impl Into<String>) -> ParsedSource {
    parse_document(
        source,
        ParseOptions {
            source_dialect: SourceDialect::Agent,
        },
    )
}

/// Runs syntax-level source lints on a typed syntax tree.
pub fn lint_source_tree(tree: &TypedSyntaxTree) -> Vec<SyntaxLint> {
    lint_id_policy(tree)
}

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
