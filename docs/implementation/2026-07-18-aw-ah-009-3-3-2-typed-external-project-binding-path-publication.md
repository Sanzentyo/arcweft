# AW-AH-009.3.3.2 typed external project-binding path publication

Date: 2026-07-18

## Source and baseline

This implementation follows
`arcweft-aw-ah-009.3.3.2-typed-external-project-binding-path-publication-final-contract.zip`
as the source of truth. The package was inspected in full before implementation
and reports `OPEN_QUESTIONS=0`.

The isolated implementation was originally created from repository commit
`9a63ac5512cd75947ba70195681e43ab968f9f12`, reconciled onto
`69dc5152510d2511dd44481a81d3f283d9f6ae41`, and finally rebased as the single
Jujutsu change `uyulyzvk` onto current `main`
`715335844401c33fd9f96708ca4417290eb6307f`. The final rebase was conflict-free
and preserved the Proof capability-policy cut, the Seq-06.11d.4.2.3
environment-product source contract, the Lang-01.3.1.2.1 request cut, current
inference callable signatures, the Lang-01.2 split project-test owner, and the
current entry/profile/environment behavior.

## Implemented contract

- `ProjectDirectBinding` now owns the existing typed `ProjectSymbolPath` and
  rejects explicit roots.
- HIR scope rows retain typed paths through direct bindings, imports, aliases,
  groups, globs, re-exports, fixed-point linking, collision evidence, and one
  deterministic public iterator.
- Character registration constructs qualified and compact paths from
  `CharacterId::compact_segments()`.
- Adapter context owns a language-free `AdapterSymbolSegment` /
  `AdapterSymbolPath` model. The unchanged schema-v1 `name` field decodes
  directly into that model.
- Callable catalog publication converts every typed project segment directly
  to `CallableName`, charges row-plus-segment work, preserves existing path
  limits and type projection, and fails closed on missing types or collisions.
- Existing accepted-world construct-then-publish behavior is retained and now
  has collision and malformed-path pointer/generation/cache identity coverage.
- Identical rows at the same typed binding path are explicitly accepted while
  non-identical rows remain a complete-catalog collision.
- Resolver coverage proves that the compact project path `akane` and the
  qualified environment path `character.akane` are independent segmented keys.
- The character producer's exact path-construction boundary rejects a
  test-injected numeric compact segment before either direct binding can be
  constructed.
- A real malformed adapter manifest rebuild through disk discovery, profile
  ownership, and LSP state retains the previously accepted pointer, generation,
  semantic world, and caches.

## Character-path ownership boundary

The loader and semantic registrar both derive qualified and compact character
paths from `CharacterId::compact_segments()`, but they do not share a
syntax-owning convenience wrapper:

- `arcweft-project-loader` is the producer and constructs the two
  `ProjectDirectBinding` values.
- `arcweft-lang-sema` independently reconstructs the two mandatory paths only
  to audit that the producer supplied the complete character publication.

Moving this projection onto `CharacterId` would make the character model depend
on language-syntax path types. Moving it onto HIR would make HIR own character
domain policy. Keeping the small projections at these two distinct ownership
boundaries preserves dependency direction; neither implementation splits raw
strings or accepts an untyped compatibility form.

## Explicit non-goals

The package excludes compatibility wrappers, deprecated aliases, dual readers,
alternate adapter schema versions, a second project-symbol resolver, changes to
callable identities or catalog record shapes, resolver precedence changes,
rollback APIs, CSS, and Takumi work. None were added.

The package also does not redesign syntax parsing or module-path grammar.
`ProjectSymbolPath` and `ProjectSymbolSegment` remain the syntax-owned contract.

## Validation

Completed focused checks:

- `cargo test -p arcweft-adapter-context --lib --tests`
  (`15` unit tests and `1` public API integration test passed)
- `cargo test -p arcweft-lang-sema identical_typed_project_bindings_at_the_same_path_are_accepted --lib`
- `cargo test -p arcweft-lang-sema compact_project_binding_does_not_shadow_a_qualified_environment_callable --lib`
- `cargo test -p arcweft-project-loader malformed_compact_character_path_fails_before_direct_binding_construction --lib`
- `cargo test --offline -p arcweft-lsp malformed_adapter_symbol_path_preserves_the_real_accepted_profile_state --lib`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `just test-workspace`
  (`exit 0`; all tests passed in `745.173` seconds)
- `cargo fmt --all -- --check`
- `cargo metadata --no-deps --format-version 1`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
  (`3,254` files, `1,667` Rust files, `764,457` Rust LOC, `92` manifests,
  `0` errors, `133` threshold warnings)
- `jj resolve --list` reported no conflicts.
- The complete Jujutsu diff passed a reverse `git apply --check` with
  `--whitespace=error-all`.

An additional broad `cargo test --offline -p arcweft-lang-syntax symbol_path`
filter reached the command timeout while traversing unrelated integration-test
binaries and ended with a broken pipe from the terminated harness. It was not a
failed assertion and was not used as acceptance evidence.

The first integration `just test-workspace` process was externally terminated
at its 30-minute command limit while the cold target was still progressing.
The closed output pipe produced `BrokenPipe` errors in the active
`arcweft-runtime-driver` harness; no assertion failed. The exact command was
then rerun on the same unchanged checkout with a sufficient command limit and
completed successfully in `745.173` seconds. Only the repository's existing
unused-patch warnings were emitted.

## Structural audit measurements

Measurements below are from change `uyulyzvk` after the final rebase onto
`715335844401c33fd9f96708ca4417290eb6307f`. The canonical audit scanned
`3,254` files, `1,667` Rust files, `764,457` physical Rust lines, and `92`
package manifests with `0` errors and `133` threshold warnings. Byte and
physical-LOC counts are exact for the checkout used by the package-scope
audit. Embedded test LOC is listed where a production file contains
`#[cfg(test)]`.

| Path | Owner / role | Bytes | LOC | Embedded test LOC |
| --- | --- | ---: | ---: | ---: |
| `arcweft-adapter-context/src/codec.rs` | adapter codec production | 15,689 | 485 | 215 |
| `arcweft-adapter-context/src/lib.rs` | adapter facade | 439 | 13 | 0 |
| `arcweft-adapter-context/src/manifest.rs` | adapter manifest production | 45,294 | 1,304 | 534 |
| `arcweft-adapter-context/src/manifest/registration.rs` | manifest registration production | 7,408 | 208 | 0 |
| `arcweft-adapter-context/src/manifest/registry.rs` | manifest registry production | 2,087 | 74 | 0 |
| `arcweft-adapter-context/src/standard.rs` | standard adapter production | 16,272 | 449 | 103 |
| `arcweft-adapter-context/src/symbol.rs` | typed adapter symbols production | 5,300 | 163 | 56 |
| `arcweft-adapter-context/tests/public_symbol_api.rs` | integration test | 1,376 | 39 | 0 |
| `arcweft-compiler/src/project/tests.rs` | unit test | 13,702 | 401 | 0 |
| `arcweft-lang-hir/src/symbol.rs` | HIR symbol facade | 1,519 | 41 | 0 |
| `arcweft-lang-hir/src/symbol/identity.rs` | HIR identity production | 18,951 | 616 | 66 |
| `arcweft-lang-hir/src/symbol/table.rs` | linked symbol table production | 46,015 | 1,247 | 0 |
| `arcweft-lang-hir/src/symbol/tests.rs` | unit test | 38,396 | 1,131 | 0 |
| `arcweft-lang-hir/tests/project_symbols.rs` | integration test | 2,370 | 77 | 0 |
| `arcweft-lang-sema/src/callable/builder.rs` | callable catalog production | 40,384 | 1,039 | 0 |
| `arcweft-lang-sema/src/callable/resolver_tests.rs` | unit test | 27,285 | 750 | 0 |
| `arcweft-lang-sema/src/callable/tests.rs` | unit test | 62,641 | 1,856 | 0 |
| `arcweft-lang-sema/src/registration/registrar.rs` | semantic registrar production | 36,647 | 941 | 0 |
| `arcweft-lang-sema/src/registration/tests.rs` | unit test | 91,340 | 2,484 | 0 |
| `arcweft-lang-sema/src/test_support/character_project.rs` | test support | 15,619 | 441 | 0 |
| `arcweft-lang-sema/tests/character_manifest_types.rs` | integration test | 12,320 | 348 | 0 |
| `arcweft-lang-syntax/src/ast/symbol_path.rs` | syntax AST production | 15,338 | 464 | 108 |
| `arcweft-lsp/src/profiles/state.rs` | accepted-profile state production | 33,981 | 975 | 486 |
| `arcweft-lsp/src/profiles/tests.rs` | unit test | 14,977 | 471 | 0 |
| `arcweft-project-loader/src/environment.rs` | environment producer production | 29,053 | 853 | 219 |
| `arcweft-verify-lsp/src/lib.rs` | verification facade/production | 72,244 | 1,909 | 798 |

The warning-level changed production files are the existing manifest owner
(`manifest.rs`, whose production portion is about 770 LOC after excluding
tests), the cohesive linked-symbol table (`table.rs`, 1,247 LOC), and the
existing verification facade (`arcweft-verify-lsp/src/lib.rs`, 1,909 LOC with
798 embedded test LOC). This change adds typed boundary behavior to those
owners; it does not introduce a new cross-layer responsibility or an
error-level file.

The largest workspace Rust files were also classified by the canonical audit:
the 12,399-LOC vertical-orientation lookup is generated data; the next largest
files are existing integration suites at 7,984, 6,620, 6,109, 5,850, and 5,249
LOC. Existing unit/integration hotspots include typecheck tests (4,217 LOC),
compiler tests (3,812 LOC), CLI native tests (3,182 LOC), runtime session tests
(2,974 LOC), and LSP session tests (2,732 LOC). No dependency direction,
workspace manifest, feature, or crate boundary changed in this slice.

## Integration note

The implementation remains contract-independent from Lang-01.2. Its overlap
was reconciled by preserving the existing `ProjectSymbolPath` owner,
external-segment grammar, exact range behavior, typed direct-binding/linker
tests, and the split compiler project tests. No adapter or semantic dependency
was introduced into HIR.

There is no remaining implementation or verification TODO within
AW-AH-009.3.3.2. The separate repository-wide Tier 2 harness refresh requested
after this integration was already in validation is tracked as its own
cross-cutting test-policy and fixture slice; it does not change this typed
binding contract or restore an older identifier path.
