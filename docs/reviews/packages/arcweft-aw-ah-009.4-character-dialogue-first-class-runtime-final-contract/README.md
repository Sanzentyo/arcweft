# AW-AH-009.4 first-class `CharacterDialogue` runtime final contract

**Status:** `READY_FOR_IMPLEMENTATION`  
**Open questions:** `0`  
**Implementation performed:** `NO`  
**Inspected repository:** `Sanzentyo/arcweft`  
**Inspected `main`:** `f56ed157f8d9070d9d1c607f739d9bd0baa1675d`  
**Governing request SHA-256:** `250a1dc175c5281d79b391cdc0873d75c1ef4b7517f63458bf4b5816a3e23b63`

This archive is the implementation-ready final contract for AW-AH-009.4. It
contains no production patch. The attached request in `SOURCE_REQUEST.md` is the
only requirement authority. The contract closes every required decision,
chooses one runtime and wire model, fixes the complete configuration merge
table, fixes line identity, inventories every real wire, specifies stable
diagnostics and limits, orders the implementation into compiling coherent cuts,
and provides a direct behavior/type/codec/compile-fail test matrix.

## Frozen outcome

The selected model is **`RUNTIME_VALUE`**.

`CharacterDialogue` is one immutable nominal runtime value. It owns a validated
`CharacterId` directly and may be returned, stored, captured, passed through
ordinary functions, placed in supported collections, saved when reachable, and
content-applied after runtime control flow. It is **not** represented as an
ordinary `RuntimeFunctionValue`; the existing function/currying subsystem is
preserved. Parenthesized configuration and bracket/colon content application
are dedicated typed callable surfaces in the shared resolver.

The only generic runtime-value substrate correction is the addition of a
nominal-record carrier in `arcweft-core`. Current AWBC record types already own
a nominal `public_id`, while `RuntimeValue::Record` drops that identity. That
concrete mismatch prevents validated save/restore of `CharacterDialogue`.
Anonymous records remain unchanged.

The `@say.*` family is retained strictly as the stable dialogue-line entity
namespace. Generated line IDs are source-site identities and contain no
character spelling. Character identity is always a separate typed field and is
never reconstructed from a line ID, local name, alias, callee label, or suffix.

## Read order

1. `FINAL_CONTRACT.md` — normative end-to-end model and required decisions.
2. `TYPE_AND_MERGE_TABLE.md` — exact owned types and every configuration field.
3. `GRAMMAR_HIR_SEMA.md` — syntax, recovery, HIR, type rules, resolver, and IDs.
4. `RUNTIME_WIRE_PERSISTENCE.md` — runtime-plan, AWBC, bundle, save, replay,
   hot reload, and Agent observation.
5. `TOOLING_DIAGNOSTICS_LIMITS.md` — tooling behavior, stable diagnostics, and
   production limits.
6. `IMPLEMENTATION_ORDER.md` — coherent compiling cuts and validation gates.
7. `TEST_MATRIX.md` — direct behavior/type/codec/compile-fail test inventory.
8. `DELETION_MATRIX.md` — every required old concept and exact deletion cut.
9. `REPOSITORY_EVIDENCE.md` — current-main evidence and verification honesty.
10. `REQUIREMENTS_TRACEABILITY.md` — request item to frozen decision mapping.
11. `FINAL_STATUS.md` and `OPEN_QUESTIONS.md` — final readiness state.
12. `verification/verify-final-contract.py` — extraction-time artifact verifier.
13. `verification/IMPLEMENTATION_VALIDATION.md` — exact post-implementation gates.
14. `verification/PACKAGE_VERIFICATION.log` — artifact verification scope/result.

## Normative conventions

- **MUST**, **MUST NOT**, **ONLY**, and **EXACTLY** are normative.
- Rust declarations are target declarations unless marked as pseudocode.
- Private fields and validating constructors are intentional.
- Source spellings, aliases, display labels, file names, comments, and local
  variable names are never semantic identity.
- No compatibility shim, dual reader, deprecated alias, source gate,
  `.say`-specific removed-syntax diagnostic, CSS path, or Takumi path is
  permitted.
- AW-AH-009.4.1 consumes the runtime character payload fixed here to define the
  authored `dialogue.character.*` View projection. It must not reopen this
  value or wire model.

## Package integrity

`MANIFEST.txt` lists every archive member in lexical order. Its own digest is
64 ASCII zeroes; every other member has its exact SHA-256 and byte size.
`verification/verify-final-contract.py` verifies membership, hashes, UTF-8/LF
text, status, decision closure, the 260-row test inventory, deletion coverage,
and the exact `OPEN_QUESTIONS=0` result without inspecting repository source.
Run it after extraction with:

```bash
python3 verification/verify-final-contract.py .
```
