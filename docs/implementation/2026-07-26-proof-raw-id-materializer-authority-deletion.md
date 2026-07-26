# Proof raw-source ID materializer authority deletion

Date: 2026-07-26

Status: `VALIDATED_WITH_KNOWN_UNRELATED_FIXTURE_FAILURE`

## Outcome

The provisional raw-source ID materializer has been deleted across HIR,
tooling, verify-LSP, LSP transport, and CLI. The removed path was:

```text
source text
  -> parser::parse_source
  -> cst_lines + keyword/string scanning
  -> IdContextEntry
  -> tooling TextEdit / inferred-ID hint
  -> arcw ids materialize / arcweft.materializeId / LSP inlay
```

It was not the final project/source-site identity owner. Keeping it while
AW-AH-009.4.2/.3 establishes typed Dialogue application and accepted line
identity would create a second authority and invite repairs to a scanner that
must disappear.

## Deleted ownership

The cut deletes:

- `arcweft-lang-hir::id_context`, including `collect_id_context`, its raw
  declaration/choice scanner, public entry/materialization records, and unit
  test;
- `arcweft-tooling::id_context`, including edit and inlay projection;
- the materialization branch from ordinary tooling code actions;
- the now-unowned transport-neutral tooling `InlayHint` record;
- verify-LSP's ID-hint mapper;
- LSP `ArcweftCommand::MaterializeId`, command advertisement, source-ID hints,
  and tests whose only edit/hint came from that path;
- CLI `IdsCommand`, `arcw ids materialize` dispatch, and its product test; and
- current CLI/LSP design claims that the provisional command/action exists.

No compatibility command, alias, dual reader, spelling-specific diagnostic,
or source gate replaces these owners. Clap and LSP command parsing expose only
the remaining current command set.

## Retained behavior

The deletion does not remove:

- ordinary function/expression type inlays;
- formatting and current semantic canonicalization actions;
- verifier-owned proof/unsafe code actions;
- typed declaration identity lints and checked semantic identities;
- the language's relative-ID syntax; or
- the accepted AW-AH-009.4.2/.3 final identity design.

Any future materializer must consume the accepted project/source-site identity
inventory. It must not reconstruct parent/choice/dialogue identity by reparsing
text or revive `arcw ids` / `arcweft.materializeId` as compatibility surfaces.

## Dependency boundary

This deletion does not require the pending Proof `01.1.1.4.1` final HIR leaf
payload. It removes a false identity producer without choosing a replacement
expression representation. The loader/compiler parse-once switch remains
separately blocked at the final old HIR lowering boundary until
[`01.1.1.4.1`](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md)
returns.

## Validation

Focused behavior and crate gates passed:

- `cargo test -p arcweft-tooling --lib`: 58 passed;
- `cargo test -p arcweft-tooling --test capability_policy`: 2 passed;
- `cargo test -p arcweft-tooling --test style_environment`: 15 passed;
- `cargo test -p arcweft-tooling --test view_export_part`: 2 passed;
- `cargo test -p arcweft-verify-lsp --lib`: 16 passed;
- `cargo test -p arcweft-lsp --lib`: 225 passed;
- `cargo test -p arcweft-cli --lib`: 200 passed;
- affected five-crate all-target/all-feature check: passed; and
- affected five-crate all-target/all-feature strict Clippy: passed.

Push-cut gates:

- `cargo fmt --all`: passed;
- `git diff --check`: passed;
- `cargo check --workspace --all-targets --all-features`: passed in 49.71s;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed in 54.40s;
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-raw-id-materializer-deletion-2026-07-26`:
  3,700 files, 1,939 Rust files, 906,506 physical Rust LOC, 95 workspace
  package manifests, 0 errors, and 146 pre-existing warnings;
- repository package ledger: 29 ZIP archives, 0 unrecorded hashes;
- one-off production audit for the deleted module, command, dispatch, and hint
  symbols: no Rust matches; and
- `arcw --help`: passed and advertises no `ids` command.

`just test-workspace` ran for 978.9s. All preceding workspace and compile-fail
suites passed; the final `arcw_fixtures_check_run` gate reproduced the same two
pre-existing failures. Its exact rerun passed 3 of 5 tests and failed only:

- `spec_should_pass_check_fixtures_pass_after_refactor` at
  `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass_run_fixtures_pass_after_refactor` at
  `spec_should_pass/run/002_file_read_task.arcw`.

Both direct product reruns report
`sema.nominal.unknown_type: unknown type FsError`. Neither fixture enters the
deleted ID materialization path. This remains a known attached-extern nominal
type fixture gap and is not hidden or reclassified as a successful workspace
gate.

Tier 2 was not run: this cut spans tooling crates and changes public CLI/LSP
command surfaces, but it does not affect a runtime, render, Agent, MCP, or
capture execution path.

## Remaining boundary

The accepted AW-AH-009.4.2/.3 project/source-site inventory must eventually own
typed identity actions and hints. That is a new final-model consumer, not a
reason to restore this scanner or either deleted command. The Proof public HIR
leaf switch remains blocked specifically on the corrected `01.1.1.4.1`
package; no other design deviation was introduced by this deletion.
