# Proof-concurrency v6.1.1.4.1.1.1 final correction contract

```text
STATUS=READY_FOR_IMPLEMENTATION
OPEN_QUESTIONS=0
IMPLEMENTATION_PERFORMED=false
PRODUCTION_FILES_CHANGED=false
OUTPUT_LANGUAGE=English
AUDITED_MAIN=66f9bffa0ec3422c14627fcacd0457b28c28e146
AUDITED_DATE=2026-07-28
```

This archive is the standalone, design-only correction requested by Proof-concurrency v6.1.1.4.1.1.1. It closes the only remaining result-changing omissions in the retained v6.1.1.4.1.1 typed `SyntheticOwner` / `SyntheticKey` contract. It does not contain Rust production changes, a patch, an overlay, a branch, or a PR.

## Final result

The contract closes all 21 current `SyntheticRole` entries by fixing:

- the exact accepted typed owner variant or variants;
- the arbitrary-`u32` ordinal predicate;
- deterministic owner-kind-before-ordinal constructor error precedence;
- transaction-owned liveness and `SyntheticDescendantsPerOwner` accounting;
- candidate-only index/dialogue ownership by the source-backed postfix `ExprId`;
- a 51-byte, versioned, session-qualified fingerprint transcript with explicit owner and role tags; and
- direct behavioral, compile-fail, liveness, rollback, exact-limit, and fixed-vector tests.

The final eight-variant `SyntheticOwner`, qualified typed HIR IDs, 21-role vocabulary, source-query contract, AW-AH-009.4.2 candidate meaning, and deletion-driven migration remain unchanged. `SyntheticOwner::Syntax` and a raw-ID owner are not restored. No current role admits `Local` or `Capture`; those accepted owner variants remain typed vocabulary but cannot form a current valid `SyntheticKey`.

## Normative order inside this archive

1. `FINAL_CORRECTION.md`
2. `RUST_SCHEMAS.md`
3. `ROLE_OWNER_ORDINAL_MATRIX.tsv`
4. `CONSTRUCTOR_AND_TRANSACTION_CONTRACT.md`
5. `FINGERPRINT_TRANSCRIPT.md`
6. `IMPLEMENTATION_AND_DELETION_ORDER.md`
7. `TEST_MATRIX.tsv`
8. `REQUIREMENTS_TRACEABILITY.tsv`
9. evidence and validation documents

`SUPERSEDED_PARENT_SCHEMA.md` is provenance only and is non-normative.

## Inputs and hashes

- request bytes: 8552
- request SHA-256: `c4f7d650f2e0674b81ff19d85216868363be47982fa9cf72fa43996d8f16cf53`
- retained v6.1.1.4.1.1 ZIP SHA-256: `2bcd3f78efb76442c2698a24251c4d874f7a941c5a8985649ea157100908a72e`
- base Proof v6.1.1 ZIP SHA-256: `1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`
- AW-AH-009.4.2 ZIP SHA-256: `05e825dde033f308f24fc1f6e504b4c26bba2d61fd33852ce880dc666ba8f2a8`

All sidecars are inside this ZIP. `MANIFEST.txt` intentionally omits itself and records the exact SHA-256 and byte length of every other member.
