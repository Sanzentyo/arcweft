//! Parsed source container and line indexing.

use crate::ast::TypedSyntaxTree;
use crate::cst::SyntaxNode;
use crate::parser::ParseError;
use std::sync::Arc;

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
    line_index: LineIndex,
    source_hash: SourceHash,
}

/// Deterministic hash of source text used for cache keys.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceHash(u64);

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
    ) -> Self {
        let source_hash = SourceHash::new(&source);
        let line_index = LineIndex::new(&source);
        Self {
            source: Arc::from(source),
            syntax,
            typed_tree,
            errors,
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
    fn new(source: &str) -> Self {
        // FNV-1a keeps this crate Sans I/O and avoids adding a hashing crate for
        // a cache key whose only requirement is deterministic stability.
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let hash = source.as_bytes().iter().fold(OFFSET, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
        });
        Self(hash)
    }

    /// Raw stable hash value.
    pub const fn get(self) -> u64 {
        self.0
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
