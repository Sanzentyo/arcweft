# Semantic transcript tag authority

Date: 2026-08-28

Status: implemented

## Established implementation

- `HirExprKind` owns the exact 38 expression shape tags `0x0100..=0x0125`.
  `Error=0x0124` remains a reserved rejecting shape and
  `ForSynthetic=0x0125` remains live.
- `HirPatternKind` owns the exact 13 pattern shape tags
  `0x0500..=0x050C`; `Error=0x050C` is reserved for rejection.
- `HirStmtKind` owns the exact 35 statement shape tags
  `0x0700..=0x0722`; `Error=0x0722` is reserved for rejection.
- The old transcript-local `pattern_kind_tag` and the HIR tests' copied
  expression/statement ordinal tables were deleted.
- Checked expression resolution tags retain `0x0200..=0x021A` unchanged and
  append the live `Closure` family at `0x021B`.
- Checked value resolution tags are `0x0300..=0x0307`.
- The five live select resolutions remain `0x0400..=0x0404`. Removed
  `TupleElement` and `RecordElement` tags remain reserved at `0x0405` and
  `0x0406`; no deleted enum variant was reintroduced.
- The six current checked pattern resolutions are `0x0600..=0x0605`, including
  live `Record` and `TypedBinding`. The stale design-only `Nominal` family was
  not introduced.

The accepted package's semantic tag grammar was retained while its
baseline-pinned source inventory was reconciled to current source authority.

## Validation

- exhaustive owner payload matrices for all 38 expression, 13 pattern, and
  35 statement families: passed;
- checked resolution exact-layout and cross-family uniqueness test: passed;
- `cargo test -p arcweft-lang-sema checked_match_ --lib`: 20 passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo fmt --all -- --check`: passed;
- `git diff --check`: passed.
