# Lang-01.6 verification-trust wire-model cut 1b

## Scope

This is the second compiling implementation slice from
`arcweft-lang-01.6-trusted-axiom-surface-final-contract.zip`. It completes the
Sans-I/O schema-1 verification-trust authority substrate on top of the signing
subjects and generation policy landed in cut 1a. It does not yet change proof
syntax, HIR, semantic trust propagation, verifier policy, bundle audit, AWFR,
or runtime startup.

Implemented in this cut:

- strict authority, trust-manifest, signature, evidence, admission, revocation,
  declaration, package, profile, module, proof-name, and reason wire types;
- private, validated nominal string storage which preserves accepted bytes;
- exact admission-ID, trust-manifest, and revocation BLAKE3 transcripts using
  checked little-endian `u32` string/count encoding and canonical typed order;
- strict JSON decoding with required fields, duplicate/unknown-field rejection,
  schema-1 fail-closed behavior, and deterministic sorted JSON output;
- 32 MiB authority, 16 MiB artifact, and 65,536 record limits before expensive
  decoding, canonicalization, or signature work;
- opaque `ValidatedVerificationTrustAuthority`, constructed only after exact
  policy/channel/generation/policy-ID/digest/transcript/key-epoch checks;
- Ed25519 verification over the raw existing signing-transcript digest through
  `ReleaseSignaturePolicy` and `ReleaseTrustedPublicKey`, with no parallel key
  store or alternate signature policy;
- typed revocation retention for later verifier and runtime admission checks;
- canonical digest goldens and negative tests for wrong admission IDs,
  duplicate IDs/subjects, evidence mismatch, stale generations, wrong channel,
  wrong algorithm, untrusted/revoked/out-of-window keys, bad signatures,
  malformed JSON, and work limits;
- correction of `ReleaseSignaturePolicy`: `require_awfb_signature` controls
  AWFB only, while its algorithm/key/epoch constraints remain available for
  the unconditionally signed `.awvt` and `.awvr` subjects.

This is a direct unpublished-contract replacement. No legacy schema reader,
Serde default for required authority fields, compatibility alias, detached
side-table, or permissive extension map was added.

## Ownership and structural measurement

Base revision for the final local cut was `2252fe12ed81` on `main`.

| Path | Owner | Kind | Bytes | Physical LOC | Responsibility |
|---|---|---:|---:|---:|---|
| `crates/arcweft-bundle/src/release/verification_trust.rs` | `arcweft-bundle` | production | 38,877 | 1,095 | strict trust wire model, canonical transcripts, authority validation, shared release-key verification |
| `crates/arcweft-bundle/src/release/verification_trust/tests.rs` | `arcweft-bundle` | unit test | 24,735 | 701 | schema, crypto, limits, deterministic digest, and attack-control evidence |

The production module remains below the 1,200-LOC structural warning trigger.
It is a single cohesive release-trust boundary; tests are kept in a separate
module so test fixtures and signing helpers do not inflate production ownership.
No crate dependency or feature was added.

## Current boundary

The following Lang-01.6 requirements remain open and are not counted complete
by this cut:

- proof-concurrency Stage 4 ordinary-name proof AST/HIR identity and
  `#[verify.trusted(reason = ...)]` surface;
- canonical semantic proof-contract digest and direct/effective trusted-root
  closure bound to project world and revision;
- verifier-owned `TrustedProofPolicy`, exact admission matching, and opaque
  `ValidatedBuildVerification`;
- mandatory bundle/AWFR verification audit, cache/watch identities, dependency
  verification, newest-revocation application, and player startup gate;
- release CLI authority selection and elimination of neutral zero generation
  floors from release execution;
- LSP, Agent, CLI, inspector, and documentation migration plus deletion of the
  old verifier boolean/string trust model.

The validated authority intentionally retains revoked IDs rather than silently
removing records. Whether an exact proof admission is blocked is decided by the
verifier/release consumer in later phases, where the full build subject is
available.

## Validation

Executed with `CARGO_INCREMENTAL=0`:

```text
cargo test -p arcweft-bundle verification_trust --lib
  PASS: 20 passed, 0 failed

cargo test -p arcweft-bundle release:: --lib
  PASS: 52 passed, 0 failed

cargo test -p arcweft-bundle --lib
  PASS: 110 passed, 0 failed

cargo clippy -p arcweft-bundle --all-targets --all-features -- -D warnings
  BLOCKED on six View exported-part warnings already present in the newer main
  cut (`resource_codec/view/validated.rs` and `standard_view.rs`); the View
  owner was notified and is correcting them in the active Increment 4-7 cut.
  The trust module itself passed this exact clippy command before that newer
  View cut was rebased underneath it.
```

`cargo fmt --all`, canonical digest goldens, and `git diff --check` also pass.
The structural-audit output for this cut is stored under
`docs/implementation/structure-audits/` with the other reviewable cut records:
3,122 files, 1,563 Rust files, 718,758 physical Rust LOC, 0 errors, and 128
warnings.
