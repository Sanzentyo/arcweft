//! Parsed source container and line indexing.

use crate::ast::items::TypedSyntaxTree;
use crate::cst::{SyntaxNode, SyntaxParseStats};
use crate::parser::recovery::ParseError;
use std::{fmt, sync::Arc};

/// Fully parsed source file.
///
/// The lossless syntax tree is always present. `errors` records recoverable
/// syntax failures, while `typed_tree` preserves the current semantic view used
/// by HIR and checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedSource {
    source: Arc<str>,
    syntax: SyntaxNode,
    typed_tree: TypedSyntaxTree,
    errors: Vec<ParseError>,
    syntax_stats: SyntaxParseStats,
    line_index: LineIndex,
    source_hash: SourceHash,
}

/// Deterministic content digest of source text used for cache keys.
///
/// The digest is BLAKE3 over the exact UTF-8 source bytes. It intentionally
/// does not normalize line endings or Unicode forms; callers that need logical
/// source equivalence should normalize before parsing.
///
/// BLAKE3 is heavier than a 64-bit non-cryptographic hash and adds a small
/// dependency/bundle-size cost, including for wasm builds. The tradeoff is
/// intentional: `SourceHash` is meant to be stable enough for cache keys,
/// manifests, and future incremental parsing where accidental collisions are
/// harder to justify than the modest implementation cost.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceHash([u8; Self::LEN]);

/// Byte offsets of line starts for source-coordinate conversion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineIndex {
    starts: Vec<usize>,
}

impl ParsedSource {
    pub(crate) fn new(
        source: String,
        syntax: SyntaxNode,
        typed_tree: TypedSyntaxTree,
        errors: Vec<ParseError>,
        syntax_stats: SyntaxParseStats,
    ) -> Self {
        let source_hash = SourceHash::new(&source);
        let line_index = LineIndex::new(&source);
        Self {
            source: Arc::from(source),
            syntax,
            typed_tree,
            errors,
            syntax_stats,
            line_index,
            source_hash,
        }
    }

    /// Original source text.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Lossless rowan syntax tree.
    pub const fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    /// Typed syntax model used by current HIR lowering.
    pub const fn typed_tree(&self) -> &TypedSyntaxTree {
        &self.typed_tree
    }

    /// Recoverable parse diagnostics.
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// Path-free counters collected by the syntax parser.
    pub const fn syntax_stats(&self) -> SyntaxParseStats {
        self.syntax_stats
    }

    /// Line index for byte-offset diagnostics.
    pub const fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    /// Deterministic source hash.
    pub const fn source_hash(&self) -> SourceHash {
        self.source_hash
    }

    /// True when no parse diagnostics were emitted.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Consumes the parsed source and returns the typed syntax model.
    pub fn into_typed_tree(self) -> TypedSyntaxTree {
        self.typed_tree
    }
}

impl SourceHash {
    /// Number of bytes in a source digest.
    pub const LEN: usize = 32;

    fn new(source: &str) -> Self {
        Self(*blake3::hash(source.as_bytes()).as_bytes())
    }

    /// Raw stable digest bytes.
    pub const fn as_bytes(self) -> [u8; Self::LEN] {
        self.0
    }

    /// Lowercase hexadecimal digest for manifests, logs, and cache filenames.
    pub fn to_hex(self) -> String {
        self.0
            .iter()
            .fold(String::with_capacity(Self::LEN * 2), |mut out, byte| {
                use std::fmt::Write as _;
                write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
                out
            })
    }
}

impl fmt::Display for SourceHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl LineIndex {
    fn new(source: &str) -> Self {
        let mut starts = vec![0];
        for (index, ch) in source.char_indices() {
            if ch == '\n' {
                starts.push(index + ch.len_utf8());
            }
        }
        Self { starts }
    }

    /// Start byte offset for each line.
    pub fn starts(&self) -> &[usize] {
        &self.starts
    }

    /// Converts a byte offset to zero-based line and column.
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let line = self.starts.partition_point(|start| *start <= offset);
        let line = line.saturating_sub(1);
        (line, offset.saturating_sub(self.starts[line]))
    }
}
