# Verification scope and evidence boundaries

## Directly verified for this return

- Attached request bytes, byte count, SHA-256, and Git blob identity.
- Latest inspected repository main `c49099fb154d9e3dbb587e1bcd7ee243214da0c4` through the GitHub connector.
- Root, docs, reviews, implementation, and crates `AGENTS.md` instructions.
- Current AWBC schema/codec/wire/VM, task identity, AwaitMany snapshot,
  runtime-plan, semantic TypeKind/pattern/registration/nominal/resource owners,
  maintained timeout/pattern/AWBC docs, and accepted line/Stream contracts at
  the exact ranges in `SOURCE_EVIDENCE.csv`.
- Human/machine artifact consistency, exact allocation tables, required owner
  sets, test cases, request-chain records, safe ZIP shape, and every internal
  SHA-256 manifest entry.
- Directory and archive validator runs plus `unzip -t` and independent hash
  recomputation.

## Design-only boundary

No production patch was applied, so cargo compilation, clippy, fmt, runtime VM
execution, generated fixture regeneration, repository source AST absence, and
Tier-2 property/fuzz/AOT execution are implementation obligations rather than
claims of execution in this return. They are specified concretely in
`TEST_MATRIX.csv` and `COMPILE_CLEAN_SEQUENCE.md`.

The primary, design-validation, and immediate-predecessor request bodies were
verified as exact repository Git objects at the frozen baseline. Their expected
SHA-256 values and byte lengths are imported from their retained package
manifests/SHA256SUMS and are compared by the package validator. This archive
does not claim to have re-downloaded and independently re-hashed those three
private predecessor bodies. The current correction body is included and
independently re-hashed.

## Result

The design package itself is fully machine/human validated. Production
behavior remains unmodified and therefore unexecuted by this design-only
return.
