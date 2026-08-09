# Proof source-file owner correction decision

## Recovery context

- Recovered into the Proof public-switch worktree: 2026-08-07
- Inspected Git revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Working tree: dirty Proof public-switch integration
- Validation authority:
  [`2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md`](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md)

This note restores the parser-owner decision only; old tests and file-size
measurements are intentionally omitted.

The source grammar transaction is the sole semantic owner for source-header
ordering, path-root normalization, visibility, and grouped-use member token
families. Attached APIs project those typed decisions from the same syntax
identities; consumers do not reopen source text.

- `ModuleDecl? UseDecl* Item*` determines root roles. Duplicate or late
  headers are ordinary lossless Error items under the current grammar.
- `parent` normalizes once to `Super(depth = 1)`; only authored repeated
  `super` extends the depth. An incomplete explicit root retains one Path owner
  with a typed missing-name child.
- Visibility is `Public | Crate | Super | Recovery` on its existing identity.
  Invalid `pub(...)` creates no alternate reader.
- Grouped-use bindings retain the parser-selected identifier, keyword, or
  lifetime token family, and group-close state comes from the exact attached
  source extent.

Parser cursor decomposition is an internal responsibility split only. It must
not create a re-export alias, public API change, identity change, or second
source owner. Current behavioral and structural evidence belongs to the full
matrix and regenerated task audit.
