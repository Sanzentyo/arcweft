# Opaque AWBC and native parity implementation evidence

Date: 2026-08-12

Inspected Git baseline:
`b0430bccf717169345846601509a1a8f3486de8e` on `main`, equal to `origin/main`, with a clean working tree before
the A1.3 implementation began.

## Implemented state

The A1.3 opaque AWBC gate is implemented without changing the Arcweft-owned
version markers:

- `AwbcRuntimeType::Opaque` uses runtime-type tag 23 and carries the canonical
  producer string ID, semantic identity, and exact/producer-wide admission;
- `AwbcConstant::Opaque` uses constant tag 18 and carries an exact opaque type
  row plus a recursively materialized payload constant;
- `AwbcRuntimeType` and `AwbcProgram` inherently reify opaque rows through the
  native `RuntimeOpaqueTypeOwner` authority;
- structural verification rejects invalid producer evidence, non-exact opaque
  constants, non-opaque type references, missing payloads, cycles, and excessive
  constant depth;
- the verifier's existing compatibility relation delegates opaque assignments
  to `RuntimeOpaqueTypeOwner::accepts_owner`, covering register, branch,
  argument, return, pattern, and `MakeVariant` consumers without a second
  matcher;
- fiber/VM runtime acceptance delegates to the same native owner and constant
  materialization calls exact-owner `try_wrap`; and
- runtime-plan interning emits complete opaque/composite types, exact opaque
  constants, one complete Result owner for both cases, and exact register types
  for opaque literal values so foreign-producer payloads fail closed; and
- bundle runtime type metadata owns an `Opaque` value family with its final
  encoded discriminant and conservative restart-required compatibility.

`AWBC_ABI_VERSION` and `AWBC_CODEC_VERSION` remain `1`. This is a direct
replacement of an unreleased internal shape; no older codec reader or alternate
wire model exists.

## Validation performed and passed

- `cargo fmt --all` and focused `git diff --check`.
- `cargo check -p arcweft-core -p arcweft-runtime-plan --all-targets
  --all-features --jobs 4`.
- `cargo clippy -p arcweft-core -p arcweft-runtime-plan --all-targets
  --all-features --jobs 4 -- -D warnings`.
- `CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features
  --jobs 4`.
- `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features
  --jobs 4 -- -D warnings`.
- `cargo test -p arcweft-core --all-targets --all-features --jobs 4`: 282 unit
  tests, 1 public-API compile test, 8 direct-suspension tests, 2 assertion tests,
  and 11 runtime-ID boundary tests passed.
- `cargo test -p arcweft-runtime-plan --all-targets --all-features --jobs 4`:
  28 unit tests, 1 public-API compile test, 10 assertion tests, 59 product-parity
  tests, and 3 iterator-witness tests passed.
- Focused new coverage pins type tag 23, constant tag 18, admission bytes 0/1,
  unknown admission rejection, exact/wide/foreign compatibility, exact VM
  materialization, invalid producer evidence, wide/non-opaque/missing/cyclic
  constant rejection, equal interning, complete Result ownership, and correct
  versus foreign opaque `Err` payloads.

## Structural review

The new `arcweft-core::awbc::type_projection` module has one responsibility:
reifying persisted AWBC opaque rows into the existing native checked owner. It
does not own a registry, producer callback, parallel type table, or I/O. The
existing verifier and VM remain the sole consumers of verified AWBC tables.

The touched large owners remain cohesive: AWBC schema owns persisted executable
rows, structural/code verification owns table and dataflow admission, fiber/VM
own execution-time value acceptance/materialization, runtime-plan inventory owns
deterministic interning, and bundle runtime resource codec owns the product
runtime-type metadata grammar. `just structure-audit-gate` scanned 2,160 files,
2,032 Rust files, 1,005,861 Rust LOC, 95 packages, and 184 review triggers with
zero blocking findings.

## Remaining work and non-goals

A1.4 persistence and deletion closure is not credited by this note. Save and
snapshot boundaries must preserve opaque owner/value evidence while their
schema marker remains fixed at `1`. The unrelated pending typed Presentation
command ABI and Tier-2 fixture failures recorded by the preceding evidence note
remain non-goals.
