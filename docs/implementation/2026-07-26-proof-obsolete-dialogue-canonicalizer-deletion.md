# Proof convergence: obsolete Dialogue canonicalizer deletion

Date: 2026-07-26

Status: implementation and cut-specific validation complete

## Context

The returned Proof-concurrency `v6.1.1.4` archive has SHA-256
`414f95f8ef4c5f3abcce163f0c9b01f124098f0bac856f174af09b5c1e7d564b`.
It is byte-identical to the previously rejected delivery and does not satisfy
the corrective `.1.1.4.1` request. Final HIR leaf payload publication therefore
remains blocked on the corrected archive; this deletion cut does not infer that
missing contract.

The accepted AW-AH-009.4.2/.3 direction independently supersedes the AW-AH-003
Speaker canonicalizer. Parenthesized ordinary calls construct or reconfigure a
typed `CharacterDialogue`; bracket and colon content application produce the
line. `.say`, `Speaker`, `SpeakerPreset`, string callee reconstruction, and a
second Dialogue call AST are not final owners.

## Deleted authority

- sema `CanonicalizationSourceSet`, checked SpeakerLine inventories, project
  canonicalization analyzers, report publication, and candidate rollback state;
- tooling `canonicalize_source`, declaration/path/line sugar planners, the
  general Dialogue tag/ruby/scalar sugar mode, and its fixtures;
- CLI `arcw canonicalize`, its project reload, and its tests;
- LSP project reload/inventory adapter, `arcweft.expandSugar`,
  `arcweft/expandSugar`, semantic sugar code actions, and provisional
  line/SpeakerPreset/character extraction actions;
- the orphaned generic execute-command edit injector and its unrelated
  `ArcweftCommand` aliases; verifier host-command payloads are no longer
  advertised or reinterpreted as arbitrary tooling edits;
- verify-LSP's command-producing semantic source-action adapter.

No alias, dual reader, migration shim, removed-syntax diagnostic, or source
gate replaces these paths.

## Retained final or still-authoritative owners

- ordinary-call lexical identity now lives directly in `LocalCallableId` with
  `SemanticScopeId` and `LexicalBindingIndex`; it no longer depends on a module
  named after canonicalization;
- `fmt --canonical-rich-text` and the edit-bearing
  `arcweft.canonicalRichText` action retain only inferred RichText family and
  typed proxy-object expansion;
- verifier actions, effect upper-bound quick fixes, formatter View/style edits,
  and revision-bound `WorkspaceEdit` projection remain;
- the current executable Speaker/ContentCall/HIR/runtime carriers are frozen,
  not repaired. They are removed only with the later typed
  syntax/HIR/sema/runtime authority switch so production execution does not
  disappear before its replacement exists. Existing registered Character and
  SpeakerPreset classification now obtains its accepted source span directly
  from the checked project instead of depending on the deleted canonicalizer
  inventory.

The freeze includes the current `.say` parser fixtures and the string callee
normalization still used by `arcweft-lang-hir` and `arcweft-lang-sema`. Those
readers receive no new identity, argument, diagnostic, or compatibility
behavior in this cut. They are deletion inventory for the direct
AW-AH-009.4.2/.3 authority switch, not accepted final surface.

## Validation

The audited working change was Jujutsu change `zptlqvltlopw` over parent
`e0a8fcbd`.

- `cargo fmt --all -- --check`: passed;
- `cargo test -p arcweft-lang-sema -p arcweft-tooling
  -p arcweft-verify-lsp -p arcweft-lsp`: passed, including 1,117 sema unit
  tests and the compile-fail API matrix;
- `cargo test -p arcweft-cli --test check
  help_does_not_advertise_removed_semantic_canonicalizer -- --exact`: passed;
- the three `fmt_canonical_rich_text` CLI tests: passed;
- `registered_character_dialogue_keeps_frozen_speaker_classification`: passed;
  it proves the pre-switch Speaker observation survives without a
  canonicalization inventory;
- `capabilities_advertise_full_sync_and_p0_features`: passed and proves that
  the server does not advertise a generic execute-command provider;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features`: the first run found
  two cut-local warnings; both root causes were removed, and the full workspace
  rerun passed without warnings;
- `just test-workspace`: all preceding commands passed, then the final
  `arcw_fixtures_check_run` suite retained the pre-existing two-failure
  `FsError` baseline:
  `spec_should_pass/check/010_capability_fs_read.arcw` and
  `spec_should_pass/run/002_file_read_task.arcw`. The exact suite rerun reported
  3 passed and those same 2 failed;
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-obsolete-dialogue-canonicalizer-deletion-2026-07-26`:
  scanned 3,693 files, 1,932 Rust files, and 902,094 Rust physical LOC; reported
  0 errors and 144 existing warnings. Exact file sizes, physical LOC,
  classifications, dependency edges, public-type duplication, and warnings are
  retained in the
  [structure audit](structure-audits/proof-obsolete-dialogue-canonicalizer-deletion-2026-07-26/violations.md).

The changed warning-level hotspots were reviewed rather than expanded:
`checker.rs` is 85,190 bytes / 2,330 physical LOC,
`registered_call.rs` is 69,641 bytes / 1,689 physical LOC, and the rewritten
LSP `features/actions.rs` is 9,389 bytes / 296 physical LOC. This cut removes
canonicalization responsibility from the first two and reduces the LSP action
module to its remaining typed actions. It adds no crate dependency or facade
re-export.

Tier 2 was not run: this cut removes compiler-tooling/LSP authority but does
not change runtime, rendering, Agent, MCP, or capture behavior.

## Remaining boundary

The next final-HIR cut requires the corrected
`proof-concurrency-v6.1.1.4.1` return. This cut must not be used as evidence that
the final leaf payload schema, public HIR switch, or runtime Dialogue replacement
has been designed or completed.
