# AW-AH-003 sema-backed speaker canonicalization

Date: 2026-07-14

Status: historical; implementation deleted on 2026-07-26

> Superseded implementation record. The sema inventory, tooling canonicalizer,
> `arcw canonicalize`, and LSP sugar action described below were deleted after
> AW-AH-009.4.2/.3 established typed `CharacterDialogue` construction and
> content application as the final owner. See
> [`2026-07-26-proof-obsolete-dialogue-canonicalizer-deletion.md`](2026-07-26-proof-obsolete-dialogue-canonicalizer-deletion.md).
> The remainder of this file is retained as historical evidence, not as a
> current API or implementation contract.

Source package:
`D:\sanze\Downloads\aw-ah-003-sema-backed-speaker-canonicalization.zip`

Repository state measured from Jujutsu change `tuqmpsrwrnnp` over base commit
`52d6fade`. The working copy also contained independent logical-axis and text
layout work; this note records only the AW-AH-003 slice.

## Result

Speaker-line canonicalization now consumes one exact, revision-bound semantic
inventory. Tooling no longer infers `SpeakerPreset` from callee names, local
names, or a handwritten expression walk.

- Parser-owned `SpeakerLineSurface` ranges survive HIR lowering. Ordinary
  content calls deliberately carry no speaker-sugar surface.
- `HirProjectModule` assigns the canonical source module recursively through
  executable flow families.
- `arcweft-lang-sema` owns source identity, source revision, lexical
  scope/binding identity, callable declaration identity, checked speaker-line
  outcomes, and the inherent `TypeKind::speaker_line_classification` rule.
- Project checking resolves constructors, helper/closure returns, block and
  branch results, imported aliases, qualified calls, character aliases, and
  shadowing through the ordinary checker and callable symbol table.
- `arcweft-tooling::canonicalize_source` has one semantic entry point. It
  rejects unavailable or stale input before planning edits, applies only
  semantically proven speaker rewrites, and returns deterministic partial
  diagnostics for individual unresolved, non-speaker, or inconsistent lines.
- `arcw fmt` is syntax-only and no longer accepts `--expand-sugar`.
  `arcw canonicalize` loads the containing project and supplies the selected
  module's exact checked inventory.
- LSP diagnostics and code actions share one cached project-aware analysis per
  open URI. The exact cache key is URI, document version, source revision, and
  profile epoch; open/change replace, close removes, and save/configuration/
  watched-file changes invalidate and rebuild.
- `arcweft-verify` runtime-type fixtures initialize the new report field with
  an empty inventory. These fixtures do not model source canonicalization, so
  this preserves their existing type-evidence meaning while keeping every
  `TypeCheckReport` initializer exhaustive.
- The former heuristic modules `speaker_presets.rs`, `sugar_expansion.rs`, and
  `util.rs` were deleted without a compatibility path.

## Operation boundary

| Surface | Semantic input | Behavior |
|---|---|---|
| `format_source` / `arcw fmt` | none | Syntax-only formatting; `canonical_rich_text` remains syntax-owned |
| `canonicalize_source` | exact `CanonicalizationInput` | Checked semantic and safe syntax rewrites |
| `arcw canonicalize` | containing project snapshot | Uses the same tooling canonicalizer; no write on project/sema failure |
| LSP `Canonicalize Arcweft sugar` | exact cached analysis | Action omitted when semantic analysis is unavailable |

The dependency direction remains `syntax/HIR -> sema -> tooling -> CLI/LSP`.
No syntax or sema crate depends on an adapter.

## Diagnostics and edit policy

- `AWT-CANON-001`: requested semantic data is unavailable; hard error, no
  edit report.
- `AWT-CANON-002`: source hash or length does not match the inventory; hard
  error, no edit report.
- `AWT-CANON-003`: the speaker expression is unresolved or erroneous; leave
  only that line unchanged and return a partial report.
- `AWT-CANON-004`: the expression resolves to a non-speaker type; leave only
  that line unchanged and return a partial report.
- `AWT-CANON-005`: the checked record is missing, duplicated, range-invalid,
  or surface-mismatched; leave only that line unchanged and return a partial
  report.

Diagnostics are sorted by `(start, end, code, message)`. Existing checked
UTF-8, range, and overlap failures remain `ToolingError`s rather than unchanged
success results.

## Shared adapter corpus

The helper-return fixture and expected output live under
`crates/arcweft-tooling/tests/fixtures/canonicalization/`. Tooling, the CLI
binary integration test, and the project-aware LSP session test all consume
the same bytes and assert the same canonical output. Additional focused tests
cover direct presets, character aliases, closures, block/if/if-let/match
results, chained presets, imports and qualified calls, same-spelling module
collisions, shadowing, unresolved partial results, Unicode/CRLF boundaries,
stale/unavailable input, inconsistent record multiplicity/surfaces, and
deterministic ordering.

## Structural audit

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-003-sema-backed-speaker-canonicalization
```

The final audit report is in
[`structure-audits/aw-ah-003-sema-backed-speaker-canonicalization/`](structure-audits/aw-ah-003-sema-backed-speaker-canonicalization/violations.md).
It scanned 2,724 files, 1,298 Rust files, 634,632 Rust physical LOC, and 90
package manifests. It reports zero error-level violations and 127 warnings.
Workspace warning-level hotspots are pre-existing or shared-worktree concerns
and remain visible in the report.

Relevant exact measurements from `file_metrics.csv`:

| Path | Role | Bytes | Physical LOC | Embedded tests |
|---|---|---:|---:|---:|
| `crates/arcweft-lang-sema/src/checker.rs` | production checker orchestration | 86,778 | 2,480 | no |
| `crates/arcweft-lang-sema/src/checker/module.rs` | production module/project checking | 84,131 | 2,206 | no |
| `crates/arcweft-verify-lsp/src/lib.rs` | adapter conversion plus unit tests | 61,452 | 1,637 | yes |
| `crates/arcweft-lang-sema/src/checker/helpers.rs` | checker domain/type helpers | 43,354 | 1,219 | no |
| `crates/arcweft-verify/src/runtime_type.rs` | runtime type-evidence validation and fixtures | 35,545 | 981 | yes |
| `crates/arcweft-lang-sema/src/canonicalization.rs` | semantic inventory contract | 10,474 | 400 | no |
| `crates/arcweft-cli/src/app/tooling.rs` | CLI project adapter | 10,731 | 322 | no |
| `crates/arcweft-tooling/src/canonicalization.rs` | checked edit planner | 10,558 | 297 | no |
| `crates/arcweft-lang-sema/src/checker/canonicalization.rs` | checker evidence capture | 7,643 | 216 | no |
| `crates/arcweft-lsp/src/canonicalization.rs` | LSP project snapshot adapter | 4,690 | 123 | no |

The formerly error-level `checker.rs` size was reduced below 2,500 LOC by
moving generic type-reference responsibilities into the existing checker
helper boundary. New canonicalization responsibilities remain in focused
modules. Further decomposition of the existing warning-level checker and
adapter hotspots is outside AW-AH-003 and is not needed to preserve this
contract.

Normal-dependency fan-in/fan-out measured by the same audit:

| Crate | Fan-in | Fan-out |
|---|---:|---:|
| `arcweft-lang-hir` | 8 | 3 |
| `arcweft-lang-sema` | 7 | 10 |
| `arcweft-tooling` | 4 | 5 |
| `arcweft-verify` | 5 | 7 |
| `arcweft-verify-lsp` | 1 | 7 |
| `arcweft-lsp` | 0 | 25 |
| `arcweft-cli` | 0 | 65 |

## Validation

The focused validation set is:

```bash
cargo test -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-tooling --lib --quiet
cargo test -p arcweft-verify-lsp -p arcweft-lsp --lib --quiet
cargo test -p arcweft-cli --test check canonicalize_ --quiet
cargo test -p arcweft-cli --test check fmt_rejects_removed_expand_sugar_flag --quiet
cargo fmt -p arcweft-verify
cargo test -p arcweft-verify --lib
cargo check -p arcweft-verify --all-targets --all-features
cargo clippy -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-tooling -p arcweft-verify-lsp -p arcweft-lsp --all-targets --all-features -- -D warnings
cargo clippy -p arcweft-cli --all-targets --all-features -- -D warnings
cargo clippy -p arcweft-verify --all-targets --all-features -- -D warnings
git diff --check
```

All commands above complete successfully in the final AW-AH-003 checkout:
HIR 26 tests, sema 518 tests, tooling 64 tests, LSP 97 tests, verify-LSP 16
tests, verifier 37 tests, CLI canonicalization 7 tests, and removed-flag
rejection 1 test. All focused clippy commands are clean with warnings denied.

## Remaining work and deviations

There is no remaining AW-AH-003 implementation TODO and no design deviation.
No persisted semantic cache or codec was introduced, so codec/tamper round-trip
tests are not applicable. Slow Tier 2 MCP stdio and exact visual goldens were
not run because this source-edit-only change does not touch those boundaries.
