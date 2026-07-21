# AW-AH-007/008 RichText owner schemas — cut 2

## Outcome and basis

This cut implements the owner-enum and immutable-schema portion of M3 from the
AW-AH-007/008 typed RichText attribute-validation final contract. It is based
on Git `c957a61e4a0b9abf094165c41ef4038ce25324c0`, after the earlier lossless
argument grammar in `240f5bc8fb71532863556efdb668ba78335cf91c` and generic
schema leaf in `91e6687c604528a9fd9348e2c3fd99a4dae45dbb`.

This is a compiling construction cut, not the M3 or package completion point.
No owner is wired into the production semantic or runtime path yet.

## Implemented owner boundaries

`arcweft-dialogue::rich_text` now owns:

- `DialogueRichTextControl` and `DialogueControlProperty`;
- `DialogueHostEventKind` and `DialogueHostProperty`; and
- inherent `from_source_name`, `canonical_name`, and `schema` behavior for
  the complete retained control and host-event inventories.

`arcweft-presentation::rich_text` now owns:

- direct styles and direct-style properties;
- closed style selectors and properties;
- closed layout selectors and properties;
- closed transform selectors and properties; and
- the object selector and canonical object metadata properties.

Each owner exposes immutable `RichTextTagSchema<P>` values in deterministic
owner order. Both domain crates depend one way on the Sans I/O
`arcweft-rich-text-schema` leaf. The leaf still owns no domain inventory,
source-name map, checker, registry, diagnostic, or wire representation.

The direct tests enumerate every new owner and property, round-trip canonical
names, exercise every retained grammar-owned spelling, require reject-only
unknown-property policy, reject removed property/selector aliases, and inspect
the exact defaults, units, limits, closed values, selector contracts, and
checked-output families represented by the static descriptors.

## Grammar spellings are not compatibility aliases

`page`, `wait`, `nl`, `br`, `er`, `cm`, `rb`, `i`, `slant`, `alpha`,
`object_layer`, `z`, `vertical`, `pos`, and `!` are the current grammar-owned
spellings required by the accepted matrix. They are handled only by the
owning enum and canonicalize to one owner name. There is no configuration
alias map or second reader.

Removed property and selector spellings such as `speed` as a property,
`alpha` as a property, `object_layer` as a property, layout `strictness` and
`gap`, transform-origin `start` and `glyph`, object `struct`, `proxy`, `kind`,
`z_index`, and `hit`, and untyped `.meta`/`.metadata`/`.data` have no final
property or selector identity.

## Descriptor boundary and M4 rules

The static schema describes scalar properties and identifies dedicated
payload surfaces; it does not reinterpret syntax or fabricate a scalar value
for a callable/expression payload:

- direct `[call]`, `[!]`, `[if]`, and the `call=` portion of `[at ...]` remain
  `DedicatedPayload` inputs for the shared HIR callable/expression owner;
- the timed-cue scalar schema contains only its positional duration;
- `voice`'s closed `auto` token plus PublicId rule, positional-to-property
  mapping, unitless default units, Signal/Marker leading-dot rule, Move's
  at-least-one-authored rule, and Scale's absent-`y`-from-checked-`x` rule are
  owner-specific M4 checker rules; and
- dot shorthand supplies its selector before static property checking, while
  the schema's required positional selector describes the canonical explicit
  family form.

These are not fallback semantics. M4 must reject malformed authored input
before applying absence-only defaults and must construct the family-specific
checked value without reparsing strings.

## Deliberately remaining work

M2 ordered/ranged RichText HIR remains coupled to the accepted attached-syntax
public switch. The private Proof-concurrency grammar currently has no
identity-bearing RichText tag/argument descendants, so adding detached HIR now
would create the prohibited second reader.

The rest of M3 remains open:

- the original `BuiltinRichTextFxProperty` owner still has the current
  permissive raw-production callers. `CC-001` must delete its alias/no-op
  variants only in a cut that migrates all those callers; adding a parallel
  final builtin schema here would create a mirror inventory;
- `arcweft-lang-sema::checked_rich_text`, the checked owner/action/value IR,
  structured diagnostics, and the schema-driven checker are absent; and
- visible typed text-proxy schema catalog construction remains absent.

M4-M9 checking, total runtime conversion, strict DisplayCatalog/ViewText
codecs, atomic production cutover, raw-reader deletion, formatter/LSP reuse,
corpus, backend/Tier 2 validation, and final documentation remain open. This
cut adds no CSS/Takumi path, compatibility alias, dual reader, raw executable
value, historical diagnostic, or source gate.

## Validation

The final validation passed:

```bash
cargo test -p arcweft-dialogue --test rich_text_schema
cargo test -p arcweft-presentation --test rich_text_authoring_schema
cargo test -p arcweft-dialogue -p arcweft-presentation --all-features
cargo check -p arcweft-dialogue -p arcweft-presentation --all-targets --all-features
cargo clippy -p arcweft-dialogue -p arcweft-presentation --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
git --git-dir=D:\git\arcweft\.git \
  --work-tree=D:\git\arcweft-ws-aw-ah-007-008-owners diff --check \
  c957a61e4a0b9abf094165c41ef4038ce25324c0 --
cargo +nightly -Zscript tools/structure-audit.rs --root . --write \
  docs/implementation/structure-audits/aw-ah-007-008-rich-text-owner-schema-cut-2-2026-07-21
```

- the two focused schema suites passed 4 dialogue and 5 presentation tests;
- the complete dialogue crate passed 24 unit tests, 4 schema integration
  tests, and 4 compile-fail documentation tests;
- the complete presentation crate passed 72 unit tests and 47 integration
  tests;
- all-target/all-feature check and strict Clippy passed;
- formatting and the base-relative whitespace/conflict-marker check passed;
  and
- the structural audit scanned 3,461 files, 1,804 Rust files, 829,611 Rust
  physical lines, and 94 manifests, reporting 0 errors and 131 existing
  warnings.

## Structural evidence

The audit reports are checked in under
`structure-audits/aw-ah-007-008-rich-text-owner-schema-cut-2-2026-07-21/`.
The new dependency leaf has fan-out 0 and fan-in 2, exactly from dialogue and
presentation. Dialogue has fan-out 10 and fan-in 5; presentation has fan-out 7
and fan-in 20. No reverse edge or new higher-layer dependency was introduced.

| Changed Rust file | Bytes | Physical LOC | Classification | Embedded test LOC |
| --- | ---: | ---: | --- | ---: |
| `arcweft-dialogue/src/lib.rs` | 14,282 | 523 | production facade | 2 |
| `arcweft-dialogue/src/rich_text.rs` | 214 | 7 | production facade | 0 |
| `arcweft-dialogue/src/rich_text/control.rs` | 8,133 | 246 | production | 0 |
| `arcweft-dialogue/src/rich_text/host.rs` | 15,664 | 488 | production | 0 |
| `arcweft-dialogue/tests/rich_text_schema.rs` | 6,399 | 192 | integration test | 0 |
| `arcweft-presentation/src/rich_text.rs` | 20,834 | 631 | production | 81 |
| `arcweft-presentation/src/rich_text/authoring_schema.rs` | 474 | 13 | production facade | 0 |
| `arcweft-presentation/src/rich_text/authoring_schema/direct_style.rs` | 8,896 | 271 | production | 0 |
| `arcweft-presentation/src/rich_text/authoring_schema/layout.rs` | 12,054 | 386 | production | 0 |
| `arcweft-presentation/src/rich_text/authoring_schema/object.rs` | 5,278 | 173 | production | 0 |
| `arcweft-presentation/src/rich_text/authoring_schema/style.rs` | 8,262 | 263 | production | 0 |
| `arcweft-presentation/src/rich_text/authoring_schema/transform.rs` | 9,989 | 333 | production | 0 |
| `arcweft-presentation/tests/rich_text_authoring_schema.rs` | 9,225 | 263 | integration test | 0 |

No changed production file crosses the 1,200-line warning threshold. The five
largest non-generated production Rust files in this checkout remain unchanged:
`arcweft-lang-sema/src/checker/module.rs` (2,482 LOC),
`arcweft-core/src/engine/eval/calls.rs` (2,481),
`arcweft-core/src/value.rs` (2,465),
`arcweft-cli/src/toolchain_profile.rs` (2,463), and
`arcweft-bundle/src/container.rs` (2,393).
