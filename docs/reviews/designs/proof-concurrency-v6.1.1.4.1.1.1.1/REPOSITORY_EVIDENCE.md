# Repository and predecessor evidence

## Current GitHub main

- repository: `Sanzentyo/arcweft`
- exact `main`: `5214a4836d5aa13a934ea8cb7037cc3a2a3c8e31`
- commit message: `Reject incomplete Proof synthetic-role return`
- comparison of that SHA to `main`: identical
- `AGENTS.md` blob: `e91f99213dde67953beda6aa078c370a8dc4541d`
- request blob: `abcab3da13ddf2241d4a97ea47437de9a1bb7311`
- rejected intake blob: `b95e44abf4fb7f0f7bafd5c0d91d785ecc932a79`
- `crates/arcweft-lang-hir/src/identity.rs` blob:
  `2c5abea32ca7df642522b449af832064bd1dd1ce`

Current `identity.rs` contains the exact eight-variant typed `SyntheticOwner`, the
21-role vocabulary, and the exact liveness variants copied in `RUST_SCHEMAS.md`.
It intentionally does not contain the final `SyntheticKey` after the rejected return.

## Predecessor archives

| Archive | SHA-256 | Evidence used |
|---|---|---|
| Proof v6.1.1 | `1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef` | complete member inventory checked; exact `PROOF_BLOCK.md` extracted and read, including block-tail anchors and proof/predicate distinctions |
| AW-AH-009.4.2 | `05e825dde033f308f24fc1f6e504b4c26bba2d61fd33852ce880dc666ba8f2a8` | complete member inventory checked; exact `TYPED_HIR_OWNERSHIP.md` extracted and read, including source-backed candidate root, root zero, shared-target exclusion, and per-kind preorder |
| Proof v6.1.1.4.1 | `61e2ee166bff158fe83dcf1484b7b9380a81f60d865377503400d27d238cc708` | all 20 members opened; manifests and final expression/arm records read |
| Proof v6.1.1.4.1.1 | `2bcd3f78efb76442c2698a24251c4d874f7a941c5a8985649ea157100908a72e` | all 24 members opened; source-owner, liveness, role, and lowering rows read |
| rejected Proof v6.1.1.4.1.1.1 | `a9603b3cc758d95dada69310f87a2dc26b7a2ce0ea8b6e0de39de4aa51e75024` | all 18 members opened; 17-row manifest exact; fingerprint copied byte-for-byte; role/tests compared with intake |

The two older package binaries were transported through GitHub as base64; their
central-directory inventories and the exact normative members named above were
reconstructed and verified. Unrelated members were not reinterpreted because this
focused correction explicitly retains them and reopens only tail ownership,
generator evidence, and liveness test wording. This verification boundary is stated
rather than overstated.

## Conflict evidence

The rejected intake establishes three concrete defects:

1. Expr-only tails cannot own predicate/proof block tails because those bodies have
   `ScopeId` but no source-backed body `ExprId`.
2. one shared match root plus exact-zero tail keys collides across missing arm values;
   each arm already has a distinct `ScopeId`.
3. identity admission tests do not prove production lowerer ordering, and the retired
   test named a nonexistent `last_live` field.

The corrected owner set and test matrix directly address those findings without
changing unrelated semantics.

## No repository writes

No production Rust, test, manifest, fixture, stable design chapter, branch, PR,
patch, or overlay was created or edited. The only generated object is the returned
design ZIP.
