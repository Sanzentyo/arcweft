# Lang-01.6 verification-trust signing-policy cut 1a

## Scope

This is the first compiling implementation slice from
`arcweft-lang-01.6-trusted-axiom-surface-final-contract.zip`. It extends the
existing Sans-I/O release-signing owner without changing proof syntax, HIR,
semantic trust propagation, verifier policy, bundle audit, or runtime startup.

Implemented in this cut:

- `SigningSubjectKind::VerificationTrustManifest` and
  `SigningSubjectKind::VerificationTrustRevocations`;
- `VerificationTrustGenerationPolicy`, owned directly by `SigningPolicy`;
- explicit generation-floor input on release-publish and release-consume policy
  construction;
- release policy requirements for both verification-trust signature subjects;
- inherent signing-transcript constructors for trust manifests and revocations;
- exact transcript-shape validation: these subjects carry a manifest digest and
  no unrelated bundle, artifact, archive, or payload fields;
- focused tests for policy ownership, subject-domain separation, and invalid
  extra fields;
- direct updates to current CLI and release-test call sites for the new policy
  constructor.

This is a direct current-contract replacement. No compatibility constructor,
Serde default for a missing generation-policy field, alternate subject enum, or
legacy transcript shape was added.

## Current boundary

The CLI currently supplies zero generation floors when constructing the policy.
That is the neutral typed value needed to keep existing non-authority flows
compiling; it is not the final release admission path. Lang-01.6 Phase 6 must
replace it with the explicitly selected and validated authority document and
must add no environment or project-manifest fallback.

The following package requirements remain open:

- strict authority, trust-manifest, evidence, signature, and revocation wire
  records and byte/work limits;
- signature and generation validation against external trusted keys;
- proof-concurrency Stage 4 ordinary-name typed proof identity cut;
- canonical proof contract digests and typed trusted-root closure;
- `TrustedProofPolicy` and admission matching in `arcweft-verify`;
- mandatory bundle/AWFR verification audit and cache identities;
- release, dependency, player-startup, CLI/LSP/Agent, and revocation integration;
- deletion of the broad verifier boolean and all provisional string trust data.

No claim is made that Lang-01.6 is complete at this cut.

## Validation

Executed with `CARGO_INCREMENTAL=0`:

```text
cargo test -p arcweft-bundle signing_policy --lib
  PASS: 7 passed, 0 failed

cargo test -p arcweft-bundle --lib
  PASS: 93 passed, 0 failed

cargo check -p arcweft-project-loader -p arcweft-cli --all-targets
  PASS

cargo clippy -p arcweft-bundle -p arcweft-project-loader -p arcweft-cli --all-targets
  PASS

cargo +nightly -Zscript tools/structure-audit.rs --root . --write \
  docs/implementation/structure-audits/lang-01-6-verification-trust-signing-policy-cut-1a-2026-07-17
  PASS: 0 errors, 128 warnings
```

`cargo fmt --all` and `git diff --check` were also run successfully. The
remaining final workspace gates belong to later Lang-01.6 cuts.
