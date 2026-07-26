# Proof runtime assertion guard seed decision

Date: 2026-07-26

Status: `VALIDATED_WITH_KNOWN_UNRELATED_FIXTURE_FAILURE`

## Scope and authority

This cut fixes the deterministic encoding of the accepted Proof-concurrency
v6.1.1 runtime assertion guard seed. Its design source is:

```text
docs/reviews/packages/arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip
SHA-256: 1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef
```

That package fixes the typed seed fields, BLAKE3 derive-key context, schema
value, output width, and reserved-zero repair, but describes the input only as
a canonical length-prefixed binary encoding. This repository-visible decision
closes the encoding width without changing a package field or adding another
identity model.

The returned Proof `01.1.1.4` archive at SHA-256
`414f95f8ef4c5f3abcce163f0c9b01f124098f0bac856f174af09b5c1e7d564b`
remains rejected. The newly attached copy is byte-identical to that rejected
return. Its pending
[`01.1.1.4.1` correction](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md)
still blocks the public HIR assertion-lowering switch. This cut therefore
derives identity only from already accepted typed boundary values. It does not
publish an assertion inventory, add a guard to the old core carrier, or claim
the runtime assertion stage complete.

## Canonical seed encoding

All integers and lengths use unsigned little-endian encoding. Text lengths are
UTF-8 byte lengths. Sequence lengths count elements. The exact byte stream is:

```text
schema: u16
package: u64 length + UTF-8 bytes
module: u64 segment count + each (u64 length + UTF-8 bytes)
callable.package: u64 length + UTF-8 bytes
callable.module: u64 segment count + each (u64 length + UTF-8 bytes)
callable.owner: u64 length + canonical owner-label UTF-8 bytes
callable.owner_path: u64 segment count + each (u64 length + UTF-8 bytes)
callable.name: u64 length + UTF-8 bytes
assertion_ordinal: u32
condition_index: u8
profile: u8 (`Always = 0`, `DebugOnly = 1`)
```

The fixed `u64` length width matches the runtime-plan derived-hash convention
and makes the bytes independent of host `usize` width. The package and module
fields remain present even though the canonical callable identity also carries
them: the accepted seed lists them separately, so this cut does not normalize
them away.

The stream is hashed with BLAKE3 derive-key context
`arcweft.runtime.assertion-guard.v1`. Schema `1` uses the first 16 digest bytes;
an all-zero result is repaired only by setting the final byte to `1`.

The golden seed

```text
package = story
module = chapter.opening
callable = function story::chapter.opening::scene::run
assertion ordinal = 7
condition index = 0
profile = Always
```

derives:

```text
5f 3b 1c cf ea 6b ac 47 5e ba 86 a0 78 c9 a8 98
```

The focused matrix changes package, module, callable name, callable owner,
callable owner path, assertion ordinal, condition index, and profile one at a
time and requires a different guard for every change.

## Deletion and persistence boundary

The current guardless assertion path remains frozen. In particular, this cut
does not extend the syntax-owned `AssertionStmt` reader, the public-field core
assertion carrier, or the three-string AWBC mapping. The final accepted-HIR
switch must add the typed guard at the production owner and delete those old
readers in the same compiling authority cut; no wrapper, dual reader, string
parser, or compatibility codec is permitted.

The runtime artifact fingerprint will be copied from the existing runtime-plan
`ArtifactKey` digest in that final compiler cut. The session inventory must not
participate in the artifact key. No separate fingerprint derivation is added
here.

AWBC is the persisted assertion-payload owner. Bundle and compile-cache paths
that carry canonical AWBC bytes inherit its typed guard codec rather than
gaining a second assertion codec. Current save envelopes, fiber checkpoints,
session snapshots, and root replay traces do not independently encode a runtime
assertion payload. Their final evidence is typed round-trip and absence of
session-only identity fields, not invented assertion fields.

## Validation

Passed:

- `cargo fmt --all -- --check`;
- `git diff --check`;
- `cargo test -p arcweft-runtime-plan --test assertion_identity`: 1 passed;
- `cargo clippy -p arcweft-runtime-plan --all-targets --all-features -- -D warnings`;
- `cargo check --workspace --all-targets --all-features`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- package inbox audit: all 29 ZIPs under `docs/reviews/` have a
  case-insensitive SHA-256 occurrence in package-specific implementation
  records, with 0 unrecorded archives; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-runtime-assertion-guard-seed-2026-07-26`:
  3,697 files, 1,941 Rust files, 906,932 Rust physical LOC, 95 manifests,
  0 errors, and 146 existing repository warnings.

`just test-workspace` ran for 845.7 seconds. Every preceding suite passed, and
the final `arcw_fixtures_check_run` gate again stopped on the two existing
Proof authority-switch fixtures:

- `tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw`;
- `tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw`.

An exact rerun of
`cargo test -p arcweft-cli --test arcw_fixtures_check_run -- --nocapture`
passed 3 of 5 rows and reproduced only those two failures. They
require the still-missing attached extern-capability `FsError` HIR payload;
this guard encoding neither reaches nor changes that path. No detached reader,
fallback type, or compatibility route was restored to make them pass.

This isolated identity helper does not change a runtime, render, Agent, MCP, or
capture execution path, so it is not a Tier 2 risk cut by itself.
