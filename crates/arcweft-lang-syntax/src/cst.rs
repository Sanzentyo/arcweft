//! Shared lexical predicates and counters used by the attached parser.

/// Returns whether `value` is exactly one canonical Arcweft identifier token.
pub fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_alphabetic() || ch.is_ascii_digit())
}

/// Path-free syntax parser counters used by profiling and benchmarks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SyntaxParseStats {
    pub cst_lex_passes: usize,
    pub punctuation_scans: usize,
    pub punctuation_scan_bytes: usize,
    pub line_owned_bytes: usize,
    pub block_owned_bytes: usize,
    pub raw_owned_bytes: usize,
    pub wiki_scan_performed: usize,
    pub dialogue_rescue_expr_parse_attempts: usize,
    pub numeric_seq_summaries: usize,
    pub prefix_depth_limit_failures: usize,
}
