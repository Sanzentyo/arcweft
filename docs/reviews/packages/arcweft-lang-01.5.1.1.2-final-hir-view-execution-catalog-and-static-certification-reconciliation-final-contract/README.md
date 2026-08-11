# Lang-01.5.1.1.2 final contract package

This is the standalone, design-only final contract for **final-HIR View execution
catalog and static certification reconciliation**.

- Status: `READY_FOR_IMPLEMENTATION`
- Open result-changing decisions: `0`
- Production baseline: `a6805f7375499e5cce70f84f1531832583474527`
- Request SHA-256: `5f1bf2335fb0c68f8aef66a3e7e63628bcaffdda80a29d131ee0930b24b3fda4`
- Production code overlay: **absent**
- Compatibility layer, source fallback, old AST reader, and dual codec reader:
  **forbidden**

The package selects one final typed path:

`final HIR -> FinalSemanticAnalysis::checked_views -> compiler product transaction
-> existing AWVP/ViewText/Input/Style product owners -> one runtime evaluator`.

Dynamic values execute through the existing ordinary AWBC/`RuntimeValue` owner.
`FxRuntimeValue` remains presentation-only and is reached only through a checked
projection. Resource values use the ordinary nominal runtime representation plus
`ResourceRefValue`'s owning conversion context; no `ViewRuntimeValue`, guessed
`Presentable` trait, source string lookup, or copied endpoint catalog exists.

Static certification is proof attached to the same catalog and instruction model.
It is never admission authority. Absence of a certificate selects dynamic
execution; a valid certificate may remove value-program and constant-normalization
work, but never mount, input, handler, resource-lifetime, observation, save/replay,
or hot-replacement lifecycle work.

Start with `FINAL_CONTRACT.md`, `OWNERS_AND_APIS.md`, `RUST_SCHEMAS.md`,
`PRODUCT_WIRE_AND_SAVE.md`, and `IMPLEMENTATION_PLAN.md`. The full normative test
inventory is in `TEST_MATRIX.csv`; `TEST_MATRIX.md` gives the grouped index.
