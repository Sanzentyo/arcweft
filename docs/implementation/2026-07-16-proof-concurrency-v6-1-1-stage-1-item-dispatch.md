# Proof-concurrency v6.1.1 Stage 1 top-level item dispatch

## Scope

This cut is based on Git `8984661d5679`. It moves current top-level item
classification out of the full-source cursor/orchestration module and into the
private `parser::item` grammar responsibility. The classifier consumes the
already lexed token slice, so it does not reparse strings, allocate production
syntax identity, or create a second cursor.

The direct fixture covers every current declaration root together with
top-level flow and ordinary error recovery. It proves one item root per source
item and an exact byte-for-byte green-tree round trip. Predicate and proof
entity-reference names continue to use ordinary current-grammar error recovery;
there is no spelling-specific removed-syntax recognizer or diagnostic.

This cut does not claim complete item descendants. It establishes the owner and
exhaustive outer dispatch needed for the remaining Stage 1 item grammars.

## Ownership

- `parser/item.rs` owns top-level classification, declaration-family mapping,
  declaration-root membership, and flow-head fallback;
- `parser/document.rs` retains the one full-source cursor, logical item
  boundaries, event emission, and lossless build orchestration;
- `parser/item_tests.rs` owns direct outer-family coverage without exposing the
  private shadow grammar publicly.

No Cargo dependency, public API, serialized format, compatibility layer, CSS
route, or Takumi route changes in this cut.

## Direct evidence

`every_current_top_level_item_family_has_one_lossless_root` parses one document
containing module/use, all current declaration families, a top-level flow item,
and an ordinary error item. The asserted root sequence is exhaustive and the
built green text equals the authored source exactly.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0`:

```bash
cargo test -p arcweft-lang-syntax parser::item_tests --lib -- --nocapture
cargo test -p arcweft-lang-syntax --lib
cargo clippy -p arcweft-lang-syntax --all-targets -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
git diff --check
```

The focused test passes 1/1 and the syntax library passes 173/173. Workspace
check and Clippy complete without errors or warnings.

`just test-workspace` did not complete. Its first attempt exhausted D: while
linking test artifacts (`os error 112`). `cargo clean` then removed only build
artifacts from the root checkout (23.0 GiB) and the completed AW-AH-009.2.1
(16.2 GiB) and View-export (33.3 GiB) workspaces; source checkouts and the
active environment/geometry workspaces were retained. The retry passed the
workspace suites reached before stopping at the existing
`arcweft-bundle --test view_resource_codecs` fixture
`emit_text_requires_a_one_to_one_owned_text_block_graph`: production returns
`ViewExport(DuplicateStaticTarget)` while the test still expects
`NonCanonicalTable("view_emit_text_block_duplicate_refs")`. The exact focused
test reproduces the same mismatch. This path is outside the four syntax files
in this cut and is being reconciled by the independent environment-condition
integration before its main cut; it is not counted as a passing gate here.

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-item-dispatch-2026-07-16/`.
It scanned 2,961 files, 1,459 Rust files, 681,661 physical Rust LOC, and 90
manifests with zero errors and 129 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser.rs`: 23,757 bytes / 682 physical LOC, production facade,
  hand-maintained, no embedded tests;
- `parser/document.rs`: 17,377 bytes / 550 physical LOC, production
  orchestration, hand-maintained, no embedded tests;
- `parser/item.rs`: 5,468 bytes / 163 physical LOC, production responsibility
  module, hand-maintained, no embedded tests;
- `parser/item_tests.rs`: 4,059 bytes / 126 physical LOC, direct test module,
  hand-maintained.

All in-scope files remain below structural warning thresholds. Dependency
fan-in/fan-out is unchanged because no manifest or public boundary changed.

## Remaining Stage 1 work

Stage 1 remains open. Item-family descendants, dialogue-context expression
ownership, and the remaining malformed/recovery cross-products still require
direct full-source events and tests before the Stage 1 gate can close. Stages
2 through 8 remain outside this cut and are not inferred complete from the
outer dispatch evidence.
