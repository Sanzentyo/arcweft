# Repository-aware evidence

## Inspected checkout identity

```text
inspected_date=2026-07-25
branch=main
git_commit=0b7e095f4193b9f7fbbc95cc350a626a8a63640a
jujutsu_git_backend_commit=0b7e095f4193b9f7fbbc95cc350a626a8a63640a
jujutsu_exact_revset=commit_id("0b7e095f4193b9f7fbbc95cc350a626a8a63640a")
latest_commit_subject=Make Stream reconciliation request independently throwable
root_AGENTS_blob=e91f99213dde67953beda6aa078c370a8dc4541d
```

The repository was read through the GitHub connector. The final head was
rechecked immediately before package construction. No repository mutation was
performed.

## AGENTS and Rust skill

Root `AGENTS.md` was read in full (482 lines, blob `e91f99213dde67953beda6aa078c370a8dc4541d`). Relevant
binding rules are: change owned enums/types in place; avoid ad hoc matching,
extension-trait detours, compatibility aliases/shims/dual readers, source
gates, and obsolete-syntax compatibility; preserve Sans-I/O layer direction;
use typed evidence and structural validation.

The complete uploaded Rust skill was read (SHA-256 `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665`). This
design uses owned newtypes, inherent checked behavior, deliberate visibility,
no unsafe/unstable/allow workaround, and requires fmt/check/Clippy gates for
implementation. No production Rust was written in this design package.

## Current-main facts that adjudicate the conflict

### `crates/arcweft-core/src/awbc/schema.rs`

Blob `01c0d41efb396db7292b9104b30a035441ca4372` at the inspected commit:

- current `AWBC_ABI_VERSION=1`, `AWBC_CODEC_VERSION=7`;
- current non-terminator tail is
  `0x22 CallTraitMethod`, `0x23 RegisterCleanup`, `0x24 CancelCleanup`,
  `0x25 MakeFunction`, `0x26 ApplyFunction`;
- `AwbcOpcode::from_encoded` has no `0x27`, `0x28`, or `0x29` arm.

Therefore all three proposed bytes are unused, while child rejection of
`0x22/0x23` would corrupt retained current instructions.

### `crates/arcweft-core/src/awbc/codec/code.rs`

Blob `531b5f40be683a1ba3049e27887c591987e6a665` confirms instruction encoding writes
opcode followed by declaration-order fields and rejects unknown instruction
tags.

### `crates/arcweft-core/src/awbc/codec/wire.rs`

Blob `bbdf6f2dc3c624b23f78f8d92730a64d76946440` confirms:

- `u16` is fixed little-endian;
- `u32` and lengths are canonical unsigned base-128 varints;
- `u64` is fixed little-endian;
- noncanonical varints, truncation, trailing bytes, and budgets reject.

### callable substrate

- `crates/arcweft-lang-sema/src/callable/facts.rs`, blob
  `12ab3bbdca5045d53937c5bd49050c715eb4e103`, already retains current/next group
  and exact slot `CallableParameterCoordinate`.
- `crates/arcweft-lang-sema/src/callable/schema.rs`, blob
  `776142ad69e2de6a47bb5c180739d8c679bd20d7`, already owns group-aware
  `CallableSignatureSchema` with contiguous Initial/Curried groups and limits.

No defect was found in this selected resolver/accounting substrate, so it is
consumed rather than redesigned.

### existing core owners

- `crates/arcweft-core/src/entry/identity.rs`, blob
  `4a4c982978cb3079f984b5f7bc0ca05fcd407bef`, owns existing `TypeLayoutHash` and
  `RuntimeValueDigest`.
- `crates/arcweft-core/src/value.rs`, blob
  `25ee59e63f9354d357d283f067ab1123804b0d89`, owns the current
  `RuntimeFunctionValue` struct. The child contract therefore changes this
  owner in place to the closed enum instead of adding a side type.

### accepted intake adjudication

`docs/implementation/2026-07-24-lang-01-3-1-2-2-curried-stream-intake.md`,
blob `470440be82bc5637e32fd7b86d49934cbb235251`, records the same parent identity
mapping and identifies the opcode conflict as design-blocking. This package
closes that blocker.

## Jujutsu identity note

Jujutsu using the Git backend resolves the exact backend commit object by the
same 40-hex commit ID. The package records that exact Jujutsu identity and its
`commit_id(...)` revset. A mutable/local short change-ID display prefix is not
fabricated from GitHub-only evidence and is not needed to identify the
inspected checkout exactly.
