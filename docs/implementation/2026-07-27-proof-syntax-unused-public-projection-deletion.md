# Proof convergence: unused syntax public-projection deletion

Date: 2026-07-27

Status: `LANDED_VALIDATED_WITH_EXISTING_WORKSPACE_BASELINE`

## Boundary

This deletion-driven cut removes unreleased syntax surface that had no
workspace consumer while retaining the active bound parse authority.

Public namespaces are narrowed to implementation-only ownership:

- root `cache_facts` is private;
- six CST implementation modules are crate-private; and
- sixteen parser implementation modules are crate-private.

`parser::fragment` and `parser::recovery` remain public. The existing parser
root continues to own `parse_document_with_source`, `parse_fragment`, parse
options/completion, and the other intentional entry points.

The temporary CST event bridge is no longer a downstream contract:

- `CstLine`, `CstLineEvents`, `CstLineKind`, and `FlatFence` plus their
  fields/accessors are crate-private;
- zero-consumer `CstLineEvents::is_empty` and `CstLine::kind` were deleted;
  the unit test now checks parser-visible trivia, documentation, and trimmed
  source behavior instead of the removed accessor; and
- unused rowan forwarding aliases `SyntaxToken`, `RowanTextRange`, and
  `TextSize` were deleted.

The following definition-only typed accessors were also deleted:

- `ModulePathRoot::{is_crate_rooted, super_levels}`;
- `CanonicalModulePath::ancestors_inclusive`;
- `DialogueTagKind::is_point`;
- `Expr::as_select`; and
- `ViewBody::view_calls` together with its sole recursive helper.

The final owners are the public lossless `SyntaxNode`/`SyntaxElement`/
`SyntaxKind` model, bound `ParsedSource`, direct enum matching inside syntax,
and the parser-root entry points. No public alias, wrapper, extension trait,
dual reader, source reparse, source gate, or removed-syntax diagnostic was
introduced.

Two downstream compile-fail rows prove the private/deleted module, type, alias,
and accessor boundaries through Rust visibility and type checking.

## Validation

Completed:

- `cargo fmt --all`;
- `cargo test -p arcweft-lang-syntax --test public_api --all-features --
  --nocapture`: all thirteen compile-fail rows passed;
- `cargo test -p arcweft-lang-syntax --all-targets --all-features`: passed,
  including 494 unit tests and every syntax integration/compile-fail suite;
- `cargo check --workspace --all-targets --all-features`: passed; and
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed; and
- `just test-workspace`: every preceding workspace/CLI stage passed before the
  established `arcw_fixtures_check_run` baseline stopped the recipe. The exact
  suite reported three passes and the same two failures present at the parent
  revision:
  - `spec_should_pass_check_fixtures_pass_after_refactor` for
    `010_capability_fs_read.arcw`; and
  - `spec_should_pass_run_fixtures_pass_after_refactor` for
    `002_file_read_task.arcw`.

Both fixtures require final attached-HIR publication of capability-owned
`FsError`. This deletion cut neither changes that owner nor introduces a
fallback nominal, compatibility reader, fixture bypass, or source gate.

The final design-package ledger compared all 30 retained
`docs/reviews/**/*.zip` archives against package-specific implementation
records: zero unrecorded or changed archives and zero root-inbox ZIPs.

Tier 2 is not applicable. This public-surface deletion does not change runtime,
rendering, Agent, MCP, capture, persistence, or serialized behavior.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-syntax-unused-public-projection-deletion-2026-07-27/`](structure-audits/proof-syntax-unused-public-projection-deletion-2026-07-27/).
The final pass scanned 3,764 files, including 1,954 Rust files and 906,287
physical Rust LOC, and reported zero errors plus 146 existing warnings. Its
146 warning headings are identical to the parent audit; only two existing
size messages changed because `ast/view.rs` fell from 1,846 to 1,799 LOC and
`expr.rs` fell from 1,873 to 1,865 LOC.

Representative changed production metrics are:

| Owner | Bytes | Physical LOC |
| --- | ---: | ---: |
| `arcweft-lang-syntax/src/lib.rs` | 511 | 24 |
| `arcweft-lang-syntax/src/cst.rs` | 12,335 | 420 |
| `arcweft-lang-syntax/src/cst/line.rs` | 25,396 | 770 |
| `arcweft-lang-syntax/src/parser.rs` | 25,713 | 773 |
| `arcweft-lang-syntax/src/ast/module_path.rs` | 9,855 | 309 |
| `arcweft-lang-syntax/src/ast/dialogue.rs` | 26,666 | 936 |
| `arcweft-lang-syntax/src/expr.rs` | 52,820 | 1,865 |
| `arcweft-lang-syntax/src/ast/view.rs` | 48,978 | 1,799 |

The complete audit records every changed Rust/test file, largest workspace
files, classifications, embedded-test markers, and dependency edges. No Cargo
manifest, feature, dependency edge, or crate direction changed.

## Next boundary

This cut deliberately leaves `FragmentKind::Items`, private
`parse_source_with_options`, attached `ParsedSource`, active recovery, and
linked project/HIR readers unchanged. A corrected external Proof `01.1.1.4.1`
archive arrived while this slice was validating; its adjudication and any
replacement of the retained `NOT_READY` transport belong to the next
independent documentation cut. No unadjudicated schema from that archive is
mixed into this deletion.
