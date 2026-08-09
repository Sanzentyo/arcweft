# Proof syntax parse-statistics owner decision

## Recovery context

- Recovered into the Proof public-switch worktree: 2026-08-07
- Inspected Git revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Working tree: dirty Proof public-switch integration
- Validation authority:
  [`2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md`](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md)

The final grammar-owned `SyntaxParseStats` is a crate-private immutable
snapshot of work already accounted by the one grammar transaction. It records
accepted bytes, lexer tokens, grammar events, typed node-family counts, and
structured diagnostic identities after exact stable deduplication.

The detached line-CST statistics owner is not reused for final parsing; its
line-punctuation, rescue-scan, and owned-byte counters describe the reader
deleted by the public switch. Final counters come directly from the lexer/event
builder and `GrammarBudget`; they never rescan source or inspect paths.

Statistics publish with the same tree, diagnostics, identity map, and
attachments. A no-op returns the exact accepted snapshot and statistics;
failure or one-over publishes neither. Per-owner inclusive budgets remain
transaction-local and are not aggregated into a second accounting authority.
No public compatibility view of old statistics is admitted. Current evidence
is owned by the full acceptance matrix.
