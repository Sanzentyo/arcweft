# Proof concurrency v6.1.1 Stage 1 — source-entry shadow grammar

## Outcome

The private full-document grammar now emits one lossless, typed subtree for
each source `entry` declaration. The declaration owns typed descendants for its
body, role bindings, initial `goto`, server routes, explicit route bindings,
and generic option assignments. Role types, callable paths, and option values
reuse the shared type, path, and expression grammars rather than parsing
detached source strings.

Entry body recovery is local. A malformed role, route, or option remains under
one current-grammar recovery node, and a missing outer `}` synchronizes before
the next unindented declaration without consuming its documentation or
attributes. The recovery tests use a following proof as an independent
declaration boundary; the entry grammar has no production dependency on the
predicate/proof grammar.

## Current grammar reconciliation

Lang-01.2 made typed source-entry roles and a required canonical `@entry.*` ID
the current source contract after the proof-concurrency package was written.
The private grammar inventory therefore adds identity-bearing entry-body and
entry-member kinds that were not named in the package's earlier item-only
inventory. This follows the package rule that current accepted repository
contracts take precedence; it does not preserve the older provisional shape.

The stable entry grammar no longer lists the discarded `start` or `run`
members. The final shadow grammar treats every unsupported member through the
same ordinary invalid-member recovery. It contains no old-spelling
recognizer, dedicated migration diagnostic, compatibility alias, or source
gate.

## Public-switch boundary

This cut remains private and does not change `ParsedSource`, the public
`EntryDeclItem`, HIR, semantic checking, or runtime lowering. In particular,
the provisional public `EntryItem::Raw` and `HirEntryItem::Raw` recovery path
remains until the later atomic syntax/HIR switch. Semantic analysis already
rejects that raw form as non-type-checkable, but complete removal from the
public syntax and HIR models is not claimed by this Stage 1 cut.

## Verification

Run from parent revision `b3427522c9ce`:

- `CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-features --lib
  parser::entry_grammar_tests -- --nocapture`: 9 passed;
- `CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-features --lib
  parser::item_tests::every_current_top_level_item_family_has_one_lossless_root
  -- --exact --nocapture`: 1 passed;
- `CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-features --test
  entry_roles -- --nocapture`: 10 passed;
- `CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-features`:
  passed, including 238 library tests and all syntax integration/doc tests;
- `cargo fmt --all -- --check`: passed;
- `CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lang-syntax --all-targets
  --all-features -- -D warnings`: passed;
- `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features
  -- -D warnings`: passed;
- `CARGO_INCREMENTAL=0 cargo test --workspace --lib --tests --exclude
  arcweft-cli --quiet`: passed;
- `CARGO_INCREMENTAL=0 cargo test -p arcweft-cli --lib --bins --quiet`:
  207 passed;
- the seven selected CLI integration binaries in `just test-workspace`: 22
  passed;
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-concurrency-v6-1-1-stage-1-entry`:
  scanned 3,284 files, 1,685 Rust files, 774,978 Rust physical lines, and 92
  package manifests; reported 0 errors and 132 existing warnings;
- `git diff --check`: passed.

The aggregate `just test-workspace` wrapper crossed the 360-second command
watcher twice while Windows was performing its initial non-incremental links.
The child Cargo jobs were allowed to finish; every underlying command was then
rerun separately against the completed build and passed as recorded above.

Tier 2 was not run for this private parser-only increment because it changes no
runtime, render, Agent, MCP, capture, or public parser path. The new production
responsibility module is 27,257 bytes and 811 physical lines, below the
1,200-line structural-audit warning threshold. The audit report is checked in
under
`docs/implementation/structure-audits/proof-concurrency-v6-1-1-stage-1-entry/`.
