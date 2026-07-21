# Lang-01.6 authority-wire acceptance test closure cut 1d

## Scope

This is the smallest Stage-4-independent acceptance-test closure for the
schema-1 verification-trust authority substrate from
`arcweft-lang-01.6-trusted-axiom-surface-final-contract.zip`. It adds direct
behavior evidence to the existing `arcweft-bundle::release` owner without
changing its production wire model or starting proof/HIR/verifier/CLI/bundle
assembly integration.

The tests close the remaining authority-wire evidence gaps for:

- TA-AUT-004: every authority top-level field and both embedded signatures are
  required by strict JSON decoding;
- TA-AUT-002: schema mismatch fails closed for the authority and for each
  nested manifest/signature owner;
- TA-SIG-002: both signed artifacts reject manifest-digest and
  signing-transcript-digest tampering;
- TA-SIG-005: the signing-policy epoch range accepts its inclusive minimum and
  rejects values below it and at or above its exclusive maximum;
- TA-GEN-002/003: a stale revocation generation and a cryptographically valid
  older signed pair replay are rejected by the current external floors;
- TA-REV-005: trust-manifest and revocation policy identities must match;
- TA-DET-002: canonical trust-manifest and revocation JSON survives
  decode/re-encode byte-for-byte with unchanged canonical digests;
- TA-DET-003: both individual artifact decoders reject 16 MiB plus one byte
  before JSON decoding, while exactly 16 MiB reaches the JSON decoder.

All evidence calls crate-owned typed constructors, validators, codecs, and
error enums. No source gate, implementation-text scan, compatibility reader,
or duplicate trust path was added.

## Design boundary

The direct tests did not expose a production-owner defect, so this cut changes
only the existing unit-test module. In particular it does not begin:

- Proof Stage 4 ordinary-name AST/HIR identity;
- canonical proof contract or trusted-root closure;
- verifier admission matching and policy replacement;
- mandatory bundle/AWFR audit ownership;
- release CLI selection, dependency verification, or runtime startup gates.

Those remain in the implementation order recorded by the Lang-01.6 final
contract.

## Structural measurement

Parent revision:
`bc0735cbc271af06624ce656f1171358913d24c6` (`Add bounded native signature cache`).

| Path | Owner / kind | Bytes | Physical LOC | Responsibility |
| --- | --- | ---: | ---: | --- |
| `crates/arcweft-bundle/src/release/verification_trust/tests.rs` | `arcweft-bundle` / unit-test module | 36,073 | 1,021 | authority schema, signature, freshness, policy, canonical JSON, and byte-budget acceptance evidence |

The changed test module remains below the 2,500-LOC integration-test warning
threshold. No production file, Cargo dependency, feature, or public API changed.

## Validation

Executed with `CARGO_INCREMENTAL=0`:

```text
cargo test -p arcweft-bundle verification_trust --lib
  PASS: 27 passed, 0 failed

cargo test -p arcweft-bundle --lib
  PASS: 122 passed, 0 failed

cargo clippy -p arcweft-bundle --all-targets --all-features -- -D warnings
  PASS

cargo fmt --all -- --check
  PASS

cargo +nightly -Zscript tools/structure-audit.rs --root .
  PASS: 3,441 files; 1,792 Rust files; 825,959 physical Rust LOC;
        0 errors, 131 warnings

jj diff --git --color=never |
  git -C D:/git/arcweft apply --check --whitespace=error-all -
  PASS (Jujutsu workspace equivalent of `git diff --check`)
```

This tests-only cut does not qualify as a Tier 2 runtime/render/Agent/MCP/capture
integration cut and does not run `just test-tier2`.
