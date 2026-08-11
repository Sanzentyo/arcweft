# Lang-01.3.1.2.2 final contract package

This independently usable package freezes the group-aware compiler/runtime/host
contract for curried external `fn -> Stream<T, E>` operations in Arcweft.

The selected resolution is **to support currying**. A non-final group application
evaluates and captures its group exactly once and emits no Stream-open request. A
final group application joins all earlier captures and current arguments into one
validated canonical product, then emits exactly one open request atomically.

## Authority

1. `FINAL_CONTRACT.md` is the normative behavioral contract.
2. `RUST_TYPES_AND_OWNERS.md` freezes Rust-shaped owners and invariants.
3. `AWBC_AND_WIRE.md` freezes ABI/codec tables, numeric tags, and canonical order.
4. `HOST_JSON.md` freezes the native/Web/Agent JSON projection.
5. `EVALUATION_EFFECT_SNAPSHOT.md` freezes timing, ownership, save, and restore.
6. `FINGERPRINT_AND_HOT_RELOAD.md` freezes hashing and generation compatibility.
7. `DELTA_FROM_LANG-01.3.1.2.1.md` is the exact supersession map.
8. `IMPLEMENTATION_PLAN.md` freezes compile-clean implementation order.
9. `TEST_MATRIX.md` is the acceptance matrix.
10. `REPOSITORY_EVIDENCE.md` and `VALIDATION.md` record the repository-aware basis
    and actual validation scope.

`model/` is a dependency-free reference model of the contract invariants. It is
not a production patch. `host/fixtures/` supplies strict host-wire examples.

## Core decision in one line

Signatures preserve nested groups; runtime values use a canonical coordinate table
plus a parallel value vector and an explicit completed-group count. No source-text
recovery, second resolver, flat compatibility projection, endpoint DTO, or dual
reader exists.
