# Builtin payload types at plan admission

Date: 2026-09-12.
Inspected base: `cd65ed5a50c3e7ea8eb139ab11363d56edb3d263`, on the preserved
dirty `main` checkout.

## Result

Runtime-plan type admission now checks the complete builtin payload contract
before publishing candidate rows. The existing core case registry supplies
payload presence and Tuple arity; the admitted type declaration supplies child
references. Option and Result additionally require the Tuple child to be the
exact declared item/value/error type ID. Two different semantic declarations
that both project to Bool cannot substitute for each other at this boundary.

`RuntimePlanTypeDeclaration::validate_builtin_payloads` runs inside the
existing candidate-graph validation, after dangling-reference, cycle, and
depth checks. A failure returns `InvalidBuiltinVariantSchema` with the owner
semantic identity, and the transaction publishes no candidate types, locals,
or nominal domains. The existing presence/count gate remains part of that
same transaction.

The old Option/Result checks performed only during `checked_type` projection
are deleted, together with `checked_option_type` and
`RuntimePlanTypeResolutionError::InvalidBuiltinVariantPayload`. The finite
predicate projection now consumes already validated references. No alternate
reader, compatibility path, or version increment was introduced.

This follows the builtin Tuple ABI and atomic runtime-plan admission in the
[accepted structural nominal contract](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier/FINAL_DESIGN.md).
It does not complete nominal graph admission or the convergence goal.

## Validation

- `cargo test -p arcweft-core --all-features --test builtin_payload_admission`:
  **2 passed**. These exercise every registered builtin owner, valid values,
  scalar/zero-item/two-item payload rejection, exact argument identity, and
  atomic preservation of an existing type batch.
- `cargo test -p arcweft-core --all-features --lib plan::type_table::tests`:
  **4 passed**, including graph cycle, depth, capacity, and transaction tests.
- `cargo clippy -p arcweft-core --all-features --test builtin_payload_admission`:
  passed; the library reports 125 warnings, and the new integration test has
  no reported warning.
- `cargo fmt -p arcweft-core`, documentation link checks, and
  `git diff --check`: passed.
- `just structure-audit-gate`: passed; 95 packages, 2,321 Rust files,
  1,280,809 physical Rust LOC, 310 review triggers, zero blocking violations.

The production Rust changes in this cut were already present in the unchanged
working copy used for the immediately preceding
[record-contract validation slice](2026-09-12-record-field-name-contracts.md).
That slice passed the core all-target/all-feature check, core Clippy, and
506 core tests (473 unit plus 33 integration). Its workspace check, workspace
Clippy, workspace tests, and doctests failed at the preserved host-adapter
`opaque_producer` call. Tier 2 also exposed the existing missing Variant
`layout` fields in Dialogue and runtime-accelerator. Those failed gates were
not repeated solely for the subsequent staging boundary; they remain failed,
and their downstream recipes remain not run. The new integration test was
run separately as recorded above. Commands were sequential without Cargo job
counts. Logs use the `builtin-payload-admission-` prefix in
`.arcweft-local/validation/2026-09-11-effect-row-formulas/`.

## Ownership and remaining scope

The working-copy `plan.rs` is 1,364 physical LOC / 47,993 bytes;
`plan/type_table.rs` is 806 LOC / 29,756 bytes; the new integration test is
132 LOC / 4,941 bytes. The existing `plan.rs` size trigger was reviewed: this
change removes diagnostic-projection validation and places the admission rule
on the declaration owner in `type_table.rs`. The builder remains the sole
mutable authority, with no copied type graph or additional dependency. The
core registry still owns case metadata. This is a responsibility-based move,
not an acceptance claim based on line-count reduction.

Only the builtin projection/admission hunks, integration test, and associated
documentation belong to this commit. The new graph-aware value API, canonical
visitor changes, nominal Variant layouts, Rust ADT/host migration, and other
preserved work remain outside it. There is no design deviation; complete
nominal/schema correlation, program-bound acceptance and restore, and the
remaining producer migrations are still required.
